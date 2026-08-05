//! Project YAML edit command handlers.

use earthmesh_project::{
    criterion_catalog, default_mask_sea_ratio, threshold_criterion_by_id, CloseBoundaryMode,
    CloseMaskFormat, DomainConfig, HfieldRefinementRecipe, HydroCoastConfig, MeshCellKind,
    MeshDomainKind, ModelFormat, ProjectConfig, ProjectLayerRole, RegionShape,
    SpecifiedBboxRefinement, SpecifiedCircleRefinement, SpecifiedCircleRefinements,
    SpecifiedCloseRefinement,
    ThresholdCriterionConfig, ThresholdField, ViolationPolicy, LANDCOVER_CRITERION_ID,
};
use std::path::{Path, PathBuf};

use crate::project_commands::validated_yaml;

/// Set a data layer's path + enabled flag, returning the updated YAML.
#[tauri::command]
pub(crate) fn set_layer_path(
    yaml: String,
    id: String,
    path: String,
    enabled: bool,
) -> Result<String, String> {
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    let role = {
        let layer = cfg
            .data_layers
            .iter_mut()
            .find(|layer| layer.id == id)
            .ok_or_else(|| format!("no data layer with id '{id}'"))?;
        layer.path = path.clone();
        layer.enabled = enabled;
        layer.role
    };
    if enabled
        && matches!(
            role,
            ProjectLayerRole::Threshold(_) | ProjectLayerRole::LandType
        )
    {
        for sibling in &mut cfg.data_layers {
            if sibling.id != id && sibling.role == role {
                sibling.enabled = false;
            }
        }
    }
    if role == ProjectLayerRole::MeritHydro {
        if enabled && matches!(cfg.domain, DomainConfig::Regional { .. }) {
            let hydro = cfg.hydro_coast.get_or_insert(HydroCoastConfig {
                merit_root: path.clone(),
                cama_root: None,
                merit_stride: 1,
                r3_width_m: 300.0,
                r2_width_m: 50.0,
                r3_upa_km2: 50_000.0,
                r2_upa_km2: 5_000.0,
                river_refinement_enabled: true,
                river_width_refinement_enabled: true,
                river_upstream_area_refinement_enabled: true,
                river_width_threshold_m: Some(300.0),
                river_upstream_area_threshold_km2: Some(50_000.0),
                coast_refinement_enabled: true,
                coast_buffer_km: 50.0,
                coast_land_refinement_enabled: true,
                coast_ocean_refinement_enabled: true,
            });
            hydro.merit_root = path;
        } else {
            cfg.hydro_coast = None;
        }
    }
    validated_yaml(cfg)
}

/// Configure the MERIT-Hydro river/coast refinement demand without changing
/// whether those classes remain available for coupling and map output.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(crate) fn set_hydro_refinement(
    yaml: String,
    river_width_enabled: bool,
    river_upstream_area_enabled: bool,
    coast_enabled: bool,
    coast_buffer_km: f64,
    coast_land_enabled: bool,
    coast_ocean_enabled: bool,
    river_width_threshold_m: f64,
    river_upstream_area_threshold_km2: f64,
) -> Result<String, String> {
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    if let Some(hydro) = cfg.hydro_coast.as_mut() {
        hydro.river_refinement_enabled = river_width_enabled || river_upstream_area_enabled;
        hydro.river_width_refinement_enabled = river_width_enabled;
        hydro.river_upstream_area_refinement_enabled = river_upstream_area_enabled;
        hydro.coast_refinement_enabled = coast_enabled;
        hydro.coast_buffer_km = coast_buffer_km;
        hydro.coast_land_refinement_enabled = coast_land_enabled;
        hydro.coast_ocean_refinement_enabled = coast_ocean_enabled;
        hydro.river_width_threshold_m = Some(river_width_threshold_m);
        hydro.river_upstream_area_threshold_km2 = Some(river_upstream_area_threshold_km2);
    }
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

/// Set one mean/std criterion without changing its shared source-layer path or
/// the sibling statistic. A blank explicit value restores this criterion's
/// catalog default instead of falling back to a legacy shared threshold.
#[tauri::command]
pub(crate) fn set_threshold_criterion(
    yaml: String,
    id: String,
    enabled: bool,
    value: Option<f64>,
) -> Result<String, String> {
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    let is_landcover = id == LANDCOVER_CRITERION_ID;
    let source_role = if is_landcover {
        ProjectLayerRole::LandType
    } else {
        let criterion = threshold_criterion_by_id(&id)
            .ok_or_else(|| format!("unknown threshold criterion '{id}'"))?;
        ProjectLayerRole::Threshold(criterion.source_field)
    };
    if !cfg
        .data_layers
        .iter()
        .any(|layer| layer.role == source_role)
    {
        return Err(format!(
            "threshold criterion '{id}' has no matching data source"
        ));
    }
    cfg.refinement
        .threshold_criteria
        .retain(|entry| entry.id != id);
    cfg.refinement
        .threshold_criteria
        .push(ThresholdCriterionConfig { id, enabled, value });
    validated_yaml(cfg)
}

#[tauri::command]
pub(crate) fn autofill_data_layers_from_folder(
    yaml: String,
    folder: String,
) -> Result<String, String> {
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    let files = nc_files(Path::new(&folder))?;
    if let Some(path) = find_by_stems(
        &files,
        &["landtype_igbp_update", "landtype_usgs_update", "landtype"],
    ) {
        set_autofilled_role(&mut cfg, ProjectLayerRole::LandType, "landcover", path);
    }
    for source in criterion_catalog() {
        if let Some(path) = find_by_stems(&files, threshold_stems(source.field)) {
            set_autofilled_role(
                &mut cfg,
                ProjectLayerRole::Threshold(source.field),
                source.field.stem(),
                path,
            );
        }
    }
    validated_yaml(cfg)
}

fn set_autofilled_role(
    cfg: &mut ProjectConfig,
    role: ProjectLayerRole,
    canonical_id: &str,
    path: PathBuf,
) {
    let selected = cfg
        .data_layers
        .iter()
        .position(|layer| layer.role == role && layer.enabled)
        .or_else(|| {
            cfg.data_layers
                .iter()
                .position(|layer| layer.role == role && layer.id == canonical_id)
        })
        .or_else(|| cfg.data_layers.iter().position(|layer| layer.role == role));
    let Some(selected) = selected else {
        return;
    };
    let path = path.to_string_lossy().into_owned();
    for (index, layer) in cfg.data_layers.iter_mut().enumerate() {
        if layer.role == role {
            layer.enabled = index == selected;
            if index == selected {
                layer.path = path.clone();
            }
        }
    }
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

/// Update the canonical physical target while preserving project intent as the
/// one-shot preset that originally scaffolded the project.
#[tauri::command]
pub(crate) fn set_project_target(
    yaml: String,
    kind: String,
    model_format: String,
) -> Result<String, String> {
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    cfg.target.kind = match kind.trim().to_ascii_lowercase().as_str() {
        "land" => MeshDomainKind::Land,
        "ocean" => MeshDomainKind::Ocean,
        "atmosphere" => MeshDomainKind::Atmosphere,
        "coupled" => MeshDomainKind::Coupled,
        "earth" => MeshDomainKind::Earth,
        other => return Err(format!("unknown target kind '{other}'")),
    };
    cfg.target.model_format = match model_format.trim().to_ascii_lowercase().as_str() {
        "colm" => ModelFormat::CoLM,
        "fvcom" => ModelFormat::Fvcom,
        "icon" => ModelFormat::Icon,
        "mpas" => ModelFormat::Mpas,
        "mpas-ocean" | "mpasocean" => ModelFormat::MpasOcean,
        "mpas-simple" | "mpassimple" => ModelFormat::MpasSimple,
        other => return Err(format!("unknown target model format '{other}'")),
    };
    if cfg.target.kind != MeshDomainKind::Coupled {
        cfg.coupling = None;
    }
    validated_yaml(cfg)
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
        shape: RegionShape::Close {
            path,
            format,
            boundary: CloseBoundaryMode::Polyline,
        },
        sea_ratio: Some(sea_ratio.unwrap_or_else(default_mask_sea_ratio)),
    };
    validated_yaml(cfg)
}

/// Set the quality gate and bounded AutoRefine repair batch, returning the YAML.
#[tauri::command]
pub(crate) fn set_quality(
    yaml: String,
    min_angle_deg: f64,
    policy: String,
    auto_refine_batch_cells: usize,
) -> Result<String, String> {
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    cfg.quality.min_angle_deg = min_angle_deg;
    cfg.quality.auto_refine_batch_cells = auto_refine_batch_cells;
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
/// uniform mesh (no source data needed); enabled runs must fit Method-C levels 1..=5.
#[tauri::command]
pub(crate) fn set_refinement(
    yaml: String,
    enabled: bool,
    threshold_enabled: bool,
    max_passes: u8,
) -> Result<String, String> {
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    cfg.refinement.enabled = enabled;
    cfg.refinement.threshold_enabled = threshold_enabled;
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
    let existing_circle_count = cfg
        .refinement
        .specified_circle
        .as_ref()
        .map(|circles| circles.as_slice().len())
        .unwrap_or(0);
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
        // Writing a single circle over a chain would drop every member past the
        // first, and the panel that sends this has no way to show that. Refuse
        // instead of quietly deleting the rest of a coastline.
        if existing_circle_count > 1 {
            return Err(format!(
                "this project refines with a chain of {existing_circle_count} circles, which the single-circle control cannot edit; edit specified_circle in the project file, or switch the refinement source"
            ));
        }
        cfg.refinement.specified_circle = Some(SpecifiedCircleRefinements::One(
            SpecifiedCircleRefinement {
                lon: lon.unwrap_or(0.0),
                lat: lat.unwrap_or(0.0),
                radius_km: radius_km.unwrap_or(100.0),
            },
        ));
    } else if enabled && kind == "close" {
        cfg.refinement.specified_close = Some(SpecifiedCloseRefinement {
            path: path.unwrap_or_default(),
            boundary: CloseBoundaryMode::Polyline,
        });
    } else if enabled {
        return Err("specified refinement kind must be radius, bbox, or close".to_string());
    }
    validated_yaml(cfg)
}

/// Set the geometry semantics for a close domain or specified close refinement.
/// The UI exposes this only in expert mode, while the option stays attached to
/// the close source in the project schema for reproducibility.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(crate) fn set_close_boundary(
    yaml: String,
    target: String,
    mode: String,
    iterations: Option<u8>,
    margin_km: Option<f64>,
    max_radius_deg: Option<f64>,
    max_segment_angle_deg: Option<f64>,
) -> Result<String, String> {
    let boundary = match mode.trim() {
        "" | "polyline" => CloseBoundaryMode::Polyline,
        "spherical_chaikin" => CloseBoundaryMode::SphericalChaikin {
            iterations: iterations.unwrap_or(2),
            max_segment_angle_deg: max_segment_angle_deg.unwrap_or(0.25),
        },
        "enclosing_cap" => CloseBoundaryMode::EnclosingCap {
            margin_km: margin_km.unwrap_or(0.0),
            max_radius_deg: max_radius_deg.unwrap_or(80.0),
            max_segment_angle_deg: max_segment_angle_deg.unwrap_or(0.25),
        },
        other => return Err(format!("unknown close boundary mode '{other}'")),
    };
    boundary.validate()?;

    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    match target.trim() {
        "domain" => match &mut cfg.domain {
            DomainConfig::Regional {
                shape: RegionShape::Close { boundary: slot, .. },
                ..
            } => *slot = boundary,
            _ => return Err("domain close boundary requires a close domain".to_string()),
        },
        "specified" => {
            let close = cfg.refinement.specified_close.as_mut().ok_or_else(|| {
                "specified close boundary requires a specified close refinement".to_string()
            })?;
            close.boundary = boundary;
        }
        other => {
            return Err(format!(
                "close boundary target must be domain or specified; got {other:?}"
            ))
        }
    }
    validated_yaml(cfg)
}

/// Toggle h-field refinement (continuous cell-width field driving Method-C).
/// `enabled=false` stores an explicit discrete mask override; absent hfield
/// recipes default to h-field during lowering.
/// Choose the point+radius route and its settings.
///
/// A run refines one way or the other, so turning this on clears any h-field
/// request rather than leaving two backends both claiming the mesh.
#[tauri::command]
pub(crate) fn set_adaptive_refinement(
    yaml: String,
    enabled: bool,
    max_level: Option<u8>,
    coastline: Option<bool>,
) -> Result<String, String> {
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    let max_level = max_level.unwrap_or(0);
    if max_level > earthmesh_project::METHOD_C_MAX_AUTO_REFINE_LEVEL {
        return Err(format!(
            "adaptive max_level must be 0..={}",
            earthmesh_project::METHOD_C_MAX_AUTO_REFINE_LEVEL
        ));
    }
    let base_m = cfg
        .refinement
        .adaptive
        .as_ref()
        .and_then(|recipe| recipe.base_m);
    cfg.refinement.adaptive = Some(earthmesh_project::AdaptiveRefinementRecipe {
        enabled,
        max_level,
        base_m,
        coastline: coastline.unwrap_or(true),
    });
    if enabled {
        cfg.refinement.hfield = None;
    }
    validated_yaml(cfg)
}

#[tauri::command]
pub(crate) fn set_hfield_refinement(
    yaml: String,
    enabled: bool,
    g: Option<f64>,
    max_level: Option<u8>,
    base_m: Option<f64>,
) -> Result<String, String> {
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    // The GUI does not expose these; carry whatever the project file already had
    // so editing an unrelated h-field setting cannot silently drop them. Raster
    // size is derived during lowering when unset, and the derivation is what the
    // GUI relies on.
    let (origin_lon, origin_lat, nlon, nlat) = cfg
        .refinement
        .hfield
        .as_ref()
        .map(|recipe| {
            (
                recipe.origin_lon,
                recipe.origin_lat,
                recipe.nlon,
                recipe.nlat,
            )
        })
        .unwrap_or((None, None, None, None));
    if matches!(base_m, Some(base) if !base.is_finite() || base <= 0.0) {
        return Err("h-field base_m must be positive when set".to_string());
    }
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
            base_m,
            origin_lon,
            origin_lat,
            nlon,
            nlat,
        })
    } else {
        Some(HfieldRefinementRecipe {
            enabled: false,
            origin_lon,
            origin_lat,
            nlon,
            nlat,
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
    beta: Option<f64>,
    relax: Option<f64>,
    weak_concav_eliminate: Option<bool>,
    isolated_ocean: Option<bool>,
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
    cfg.expert.isolated_ocean = isolated_ocean;
    validated_yaml(cfg)
}
