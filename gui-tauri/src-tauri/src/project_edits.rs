//! Project YAML edit command handlers.

use earthmesh_project::{
    default_mask_sea_ratio, CloseMaskFormat, DomainConfig, HfieldRefinementRecipe, MeshCellKind,
    ProjectConfig, ProjectLayerRole, RegionShape, SpecifiedBboxRefinement,
    SpecifiedCircleRefinement, SpecifiedCloseRefinement, ThresholdField, ViolationPolicy,
};
use std::path::{Path, PathBuf};

fn validated_yaml(cfg: ProjectConfig) -> Result<String, String> {
    cfg.validate()?;
    cfg.to_yaml()
}

/// Set a data layer's path + enabled flag, returning the updated YAML.
#[tauri::command]
pub(crate) fn set_layer_path(
    yaml: String,
    id: String,
    path: String,
    enabled: bool,
) -> Result<String, String> {
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    let layer = cfg
        .data_layers
        .iter_mut()
        .find(|layer| layer.id == id)
        .ok_or_else(|| format!("no data layer with id '{id}'"))?;
    layer.path = path;
    layer.enabled = enabled;
    validated_yaml(cfg)
}

/// Set or clear a threshold criterion's numeric trigger value.
#[tauri::command]
pub(crate) fn set_threshold_value(
    yaml: String,
    id: String,
    value: Option<f64>,
) -> Result<String, String> {
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    let layer = cfg
        .data_layers
        .iter_mut()
        .find(|layer| layer.id == id)
        .ok_or_else(|| format!("no data layer with id '{id}'"))?;
    if !matches!(
        layer.role,
        ProjectLayerRole::Threshold(_) | ProjectLayerRole::LandType
    ) {
        return Err(format!("data layer '{id}' is not a refinement layer"));
    }
    layer.threshold_value = value;
    validated_yaml(cfg)
}

#[tauri::command]
pub(crate) fn autofill_data_layers_from_folder(
    yaml: String,
    folder: String,
) -> Result<String, String> {
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    let files = nc_files(Path::new(&folder))?;
    for layer in &mut cfg.data_layers {
        let matched = match layer.role {
            ProjectLayerRole::LandType => find_by_stems(
                &files,
                &["landtype_igbp_update", "landtype_usgs_update", "landtype"],
            ),
            ProjectLayerRole::Threshold(field) => find_by_stems(&files, threshold_stems(field)),
            _ => None,
        };
        if let Some(path) = matched {
            layer.path = path.to_string_lossy().into_owned();
            layer.enabled = true;
        }
    }
    validated_yaml(cfg)
}

fn nc_files(folder: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(folder).map_err(|e| format!("read {}: {e}", folder.display()))? {
        let path = entry
            .map_err(|e| format!("read {}: {e}", folder.display()))?
            .path();
        if path
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|s| matches!(s.to_ascii_lowercase().as_str(), "nc" | "nc4"))
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn find_by_stems(files: &[PathBuf], stems: &[&str]) -> Option<PathBuf> {
    stems.iter().find_map(|stem| {
        files.iter().find_map(|path| {
            let found = path.file_stem()?.to_str()?.to_ascii_lowercase();
            (found == *stem || found.starts_with(&format!("{stem}_"))).then(|| path.clone())
        })
    })
}

fn threshold_stems(field: ThresholdField) -> &'static [&'static str] {
    match field {
        ThresholdField::Lai => &["lai", "lai_bnu"],
        ThresholdField::Slope => &["slope_avg"],
        ThresholdField::Dem => &["dem", "topo"],
        ThresholdField::SlopeMax => &["slope_max"],
        ThresholdField::Ks => &["k_s"],
        ThresholdField::KSolids => &["k_solids"],
        ThresholdField::Tkdry => &["tkdry"],
        ThresholdField::Tksatf => &["tksatf"],
        ThresholdField::Tksatu => &["tksatu"],
        ThresholdField::Sst => &["sst"],
        ThresholdField::Ssh => &["ssh"],
        ThresholdField::Eke => &["eke"],
        ThresholdField::SeaSlope => &["sea_slope"],
        ThresholdField::Typhoon => &["typhoon"],
    }
}

/// Set the target cell shape, returning the updated YAML.
#[tauri::command]
pub(crate) fn set_target_cell(yaml: String, cell: String) -> Result<String, String> {
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    cfg.target.cell = match cell.trim().to_ascii_lowercase().as_str() {
        "hex" => MeshCellKind::Hex,
        "tri" => MeshCellKind::Tri,
        other => return Err(format!("unknown cell shape '{other}'")),
    };
    validated_yaml(cfg)
}

/// Set the domain to global, returning the updated YAML.
#[tauri::command]
pub(crate) fn set_domain_global(yaml: String) -> Result<String, String> {
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    cfg.domain = DomainConfig::Global;
    validated_yaml(cfg)
}

/// Set the domain to a regional bounding box, returning the updated YAML.
#[tauri::command]
pub(crate) fn set_domain_bbox(
    yaml: String,
    w: f64,
    e: f64,
    s: f64,
    n: f64,
    sea_ratio: Option<f64>,
) -> Result<String, String> {
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    cfg.domain = DomainConfig::Regional {
        shape: RegionShape::Bbox { w, e, n, s },
        sea_ratio: Some(sea_ratio.unwrap_or_else(default_mask_sea_ratio)),
    };
    validated_yaml(cfg)
}

/// Set the domain to a watershed shapefile, returning the updated YAML.
#[tauri::command]
pub(crate) fn set_domain_shapefile(
    yaml: String,
    path: String,
    sea_ratio: Option<f64>,
) -> Result<String, String> {
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    cfg.domain = DomainConfig::Regional {
        shape: RegionShape::Shapefile { path },
        sea_ratio: Some(sea_ratio.unwrap_or_else(default_mask_sea_ratio)),
    };
    validated_yaml(cfg)
}

/// Set the domain to a close boundary source, returning the updated YAML.
#[tauri::command]
pub(crate) fn set_domain_close(
    yaml: String,
    path: String,
    format: String,
    sea_ratio: Option<f64>,
) -> Result<String, String> {
    let format = match format.as_str() {
        "polygon_shp" => CloseMaskFormat::PolygonShp,
        "nml" => CloseMaskFormat::Nml,
        "netcdf" => CloseMaskFormat::Netcdf,
        "lonlat_text" => CloseMaskFormat::LonLatText,
        other => return Err(format!("unknown close format '{other}'")),
    };
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    cfg.domain = DomainConfig::Regional {
        shape: RegionShape::Close { path, format },
        sea_ratio: Some(sea_ratio.unwrap_or_else(default_mask_sea_ratio)),
    };
    validated_yaml(cfg)
}

/// Set the quality gate (min angle + on-violation policy), returning the YAML.
#[tauri::command]
pub(crate) fn set_quality(
    yaml: String,
    min_angle_deg: f64,
    policy: String,
) -> Result<String, String> {
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    cfg.quality.min_angle_deg = min_angle_deg;
    cfg.quality.on_violation = match policy.trim() {
        "warn" => ViolationPolicy::Warn,
        "block" => ViolationPolicy::Block,
        "auto_refine" => ViolationPolicy::AutoRefine,
        other => {
            return Err(format!(
                "quality on_violation must be warn, block, or auto_refine; got {other:?}"
            ));
        }
    };
    validated_yaml(cfg)
}

/// Set whether refinement runs and how many passes. `enabled=false` yields a
/// uniform mesh (no source data needed); enabled runs must fit engine levels 1..=9.
#[tauri::command]
pub(crate) fn set_refinement(
    yaml: String,
    enabled: bool,
    max_passes: u8,
) -> Result<String, String> {
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    cfg.refinement.enabled = enabled;
    cfg.refinement.max_passes = if enabled { max_passes } else { 0 };
    validated_yaml(cfg)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(crate) fn set_specified_refinement(
    yaml: String,
    enabled: bool,
    kind: Option<String>,
    lon: Option<f64>,
    lat: Option<f64>,
    radius_km: Option<f64>,
    w: Option<f64>,
    e: Option<f64>,
    s: Option<f64>,
    n: Option<f64>,
    path: Option<String>,
) -> Result<String, String> {
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    cfg.refinement.specified_circle = None;
    cfg.refinement.specified_bbox = None;
    cfg.refinement.specified_close = None;
    let kind = kind.as_deref().unwrap_or("radius");
    if enabled && kind == "bbox" {
        cfg.refinement.specified_bbox = Some(SpecifiedBboxRefinement {
            w: w.unwrap_or(0.0),
            e: e.unwrap_or(1.0),
            s: s.unwrap_or(0.0),
            n: n.unwrap_or(1.0),
        });
    } else if enabled && kind == "radius" {
        cfg.refinement.specified_circle = Some(SpecifiedCircleRefinement {
            lon: lon.unwrap_or(0.0),
            lat: lat.unwrap_or(0.0),
            radius_km: radius_km.unwrap_or(100.0),
        });
    } else if enabled && kind == "close" {
        cfg.refinement.specified_close = Some(SpecifiedCloseRefinement {
            path: path.unwrap_or_default(),
        });
    } else if enabled {
        return Err("specified refinement kind must be radius, bbox, or close".to_string());
    }
    validated_yaml(cfg)
}

/// Toggle h-field refinement (continuous cell-width field driving Method-C).
/// `enabled=false` stores an explicit legacy hard-mask override; absent hfield
/// recipes default to h-field during lowering.
#[tauri::command]
pub(crate) fn set_hfield_refinement(
    yaml: String,
    enabled: bool,
    g: Option<f64>,
    max_level: Option<u8>,
    base_m: Option<f64>,
) -> Result<String, String> {
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    cfg.refinement.hfield = if enabled {
        let g = g.unwrap_or(0.2);
        if !g.is_finite() || g <= 0.0 {
            return Err("h-field gradation g must be positive".to_string());
        }
        let max_level = max_level.unwrap_or(0);
        if max_level > 5 {
            return Err("h-field max_level must be in 0..=5 (0 = auto)".to_string());
        }
        Some(HfieldRefinementRecipe {
            enabled: true,
            g,
            max_level,
            base_m: base_m.filter(|base| base.is_finite() && *base > 0.0),
        })
    } else {
        Some(HfieldRefinementRecipe {
            enabled: false,
            ..HfieldRefinementRecipe::default()
        })
    };
    validated_yaml(cfg)
}

/// Set expert overrides. Nulls clear the override and keep template/default values.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(crate) fn set_expert(
    yaml: String,
    nxp: Option<i32>,
    openmp: Option<i32>,
    niter: Option<i32>,
    niter_refine: Option<i32>,
    max_iter_spc: Option<i32>,
    max_iter_cal: Option<i32>,
    halo: Option<Vec<i32>>,
    max_transition_row: Option<Vec<i32>>,
    set_dis_type: Option<String>,
    num_rc: Option<i32>,
    vertex_pretect_layers: Option<i32>,
    spring_global_type: Option<i32>,
    spring_regional_type: Option<i32>,
    beta: Option<f32>,
    relax: Option<f32>,
    weak_concav_eliminate: Option<bool>,
) -> Result<String, String> {
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    cfg.expert.nxp = nxp;
    cfg.expert.openmp = openmp;
    cfg.expert.niter = niter;
    cfg.expert.niter_refine = niter_refine;
    cfg.expert.max_iter_spc = max_iter_spc;
    cfg.expert.max_iter_cal = max_iter_cal;
    cfg.expert.halo = halo;
    cfg.expert.max_transition_row = max_transition_row;
    cfg.expert.set_dis_type = set_dis_type;
    cfg.expert.num_rc = num_rc;
    cfg.expert.vertex_pretect_layers = vertex_pretect_layers;
    cfg.expert.spring_global_type = spring_global_type;
    cfg.expert.spring_regional_type = spring_regional_type;
    cfg.expert.beta = beta;
    cfg.expert.relax = relax;
    cfg.expert.weak_concav_eliminate = weak_concav_eliminate;
    validated_yaml(cfg)
}
