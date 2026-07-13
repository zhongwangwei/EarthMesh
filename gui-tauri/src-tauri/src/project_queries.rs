//! Read-only project catalog and summary command handlers.

use crate::dto::{CriterionInfo, LayerSummary, ProjectSummary};
use earthmesh_project::{
    criterion_catalog, CloseMaskFormat, DomainConfig, MeshDomainKind, ProjectConfig, RegionShape,
    ResolutionSpec,
};

/// List every registered refinement criterion (self-describing GUI specs).
#[tauri::command]
pub(crate) fn list_criteria() -> Vec<CriterionInfo> {
    criterion_catalog()
        .iter()
        .map(|c| CriterionInfo {
            physical_process: c.physical_process.to_string(),
            label: c.gui.label.to_string(),
            help: c.gui.help.to_string(),
            unit: c.gui.unit.to_string(),
            range_min: c.gui.range.0,
            range_max: c.gui.range.1,
            default_value: c.gui.default,
            stem: c.field.stem().to_string(),
        })
        .collect()
}

/// Summarize a project YAML for the UI (name, intent, resolution, data layers).
#[tauri::command]
pub(crate) fn project_summary(yaml: String) -> Result<ProjectSummary, String> {
    let cfg = ProjectConfig::from_yaml(&yaml)?;
    let (nxp, approx_km, approx_degree) = match cfg.target.resolution {
        ResolutionSpec::Nxp(n) => (Some(n), None, None),
        ResolutionSpec::ApproxKm(k) => (None, Some(k), None),
        ResolutionSpec::ApproxDegree(d) => (None, None, Some(d)),
    };
    let (domain, domain_shape, bbox, watershed_path, close_format, sea_ratio) = match &cfg.domain {
        DomainConfig::Global => ("global", "global", None, None, None, None),
        DomainConfig::Regional {
            shape: RegionShape::Bbox { w, e, n, s },
            sea_ratio,
        } => (
            "regional",
            "bbox",
            Some([*w, *e, *s, *n]),
            None,
            None,
            *sea_ratio,
        ),
        DomainConfig::Regional {
            shape: RegionShape::Circle { .. },
            ..
        } => ("regional", "circle", None, None, None, None),
        DomainConfig::Regional {
            shape: RegionShape::Shapefile { path },
            sea_ratio,
        } => (
            "regional",
            "shapefile",
            None,
            Some(path.clone()),
            None,
            *sea_ratio,
        ),
        DomainConfig::Regional {
            shape: RegionShape::Close { path, format, .. },
            sea_ratio,
        } => (
            "regional",
            "close",
            None,
            Some(path.clone()),
            Some(close_format_id(*format).to_string()),
            *sea_ratio,
        ),
    };
    let cell = cfg.target.cell.engine_str().to_string();
    let quality_mode = if cell == "tri" {
        "tri-strict"
    } else {
        "hex-cgrid"
    }
    .to_string();
    let on_violation = cfg.quality.on_violation.as_str().to_string();
    let specified_circle = cfg.refinement.specified_circle.as_ref();
    let specified_bbox = cfg.refinement.specified_bbox.as_ref();
    let specified_close = cfg.refinement.specified_close.as_ref();
    let domain_close_boundary = match &cfg.domain {
        DomainConfig::Regional {
            shape: RegionShape::Close { boundary, .. },
            ..
        } => Some(boundary.clone()),
        _ => None,
    };
    let hfield_effective = cfg.refinement.hfield.clone().unwrap_or_default();
    let layers = cfg
        .data_layers
        .iter()
        .map(|l| LayerSummary {
            id: l.id.clone(),
            role_kind: l.role.role_kind().to_string(),
            role: l.role.label(),
            path: l.path.clone(),
            enabled: l.enabled,
            threshold_value: l.threshold_value,
            wants_folder: l.role.wants_folder(),
        })
        .collect();
    Ok(ProjectSummary {
        name: cfg.metadata.name.clone(),
        authors: cfg.metadata.authors.clone(),
        description: cfg.metadata.description.clone(),
        intent: cfg.target.intent.id().to_string(),
        target_kind: target_kind_id(cfg.target.kind).to_string(),
        cell,
        quality_mode,
        model_format: cfg.target.model_format.engine_str().to_string(),
        domain: domain.to_string(),
        domain_shape: domain_shape.to_string(),
        nxp,
        approx_km,
        approx_degree,
        effective_nxp: cfg.try_lower()?.mkgrd.nxp,
        bbox,
        watershed_path,
        close_format,
        domain_close_boundary,
        sea_ratio,
        min_angle_deg: cfg.quality.min_angle_deg,
        auto_refine_batch_cells: cfg.quality.auto_refine_batch_cells,
        on_violation,
        refine_enabled: cfg.refinement.enabled,
        max_passes: cfg.refinement.max_passes,
        specified_refine_enabled: specified_circle.is_some()
            || specified_bbox.is_some()
            || specified_close.is_some(),
        specified_refine_kind: if specified_close.is_some() {
            "close"
        } else if specified_bbox.is_some() {
            "bbox"
        } else {
            "radius"
        }
        .to_string(),
        specified_refine_lon: specified_circle.map(|c| c.lon),
        specified_refine_lat: specified_circle.map(|c| c.lat),
        specified_refine_radius_km: specified_circle.map(|c| c.radius_km),
        specified_refine_bbox: specified_bbox.map(|b| [b.w, b.e, b.s, b.n]),
        specified_refine_path: specified_close.map(|c| c.path.clone()),
        specified_refine_close_boundary: specified_close.map(|c| c.boundary.clone()),
        hfield_enabled: hfield_effective.enabled,
        hfield_g: Some(hfield_effective.g),
        hfield_max_level: Some(hfield_effective.max_level),
        hfield_base_m: hfield_effective.base_m,
        expert_nxp: cfg.expert.nxp,
        expert_openmp: cfg.expert.openmp,
        expert_niter: cfg.expert.niter,
        expert_niter_refine: cfg.expert.niter_refine,
        expert_max_iter_spc: cfg.expert.max_iter_spc,
        expert_max_iter_cal: cfg.expert.max_iter_cal,
        expert_halo: cfg.expert.halo.clone(),
        expert_max_transition_row: cfg.expert.max_transition_row.clone(),
        expert_set_dis_type: cfg.expert.set_dis_type.clone(),
        expert_num_rc: cfg.expert.num_rc,
        expert_vertex_pretect_layers: cfg.expert.vertex_pretect_layers,
        expert_spring_global_type: cfg.expert.spring_global_type,
        expert_spring_regional_type: cfg.expert.spring_regional_type,
        expert_beta: cfg.expert.beta,
        expert_relax: cfg.expert.relax,
        expert_weak_concav_eliminate: cfg.expert.weak_concav_eliminate,
        layers,
    })
}

fn target_kind_id(kind: MeshDomainKind) -> &'static str {
    match kind {
        MeshDomainKind::Land => "land",
        MeshDomainKind::Ocean => "ocean",
        MeshDomainKind::Atmosphere => "atmosphere",
        MeshDomainKind::Coupled => "coupled",
        MeshDomainKind::Earth => "earth",
    }
}

fn close_format_id(format: CloseMaskFormat) -> &'static str {
    match format {
        CloseMaskFormat::PolygonShp => "polygon_shp",
        CloseMaskFormat::Nml => "nml",
        CloseMaskFormat::Netcdf => "netcdf",
        CloseMaskFormat::LonLatText => "lonlat_text",
    }
}
