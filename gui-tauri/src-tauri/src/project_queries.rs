//! Read-only project catalog and summary command handlers.

use crate::dto::{CriterionInfo, LayerSummary, ProjectSummary};
use earthmesh_project::{
    criterion_catalog, DomainConfig, ProjectConfig, ProjectLayerRole, RegionShape, ResolutionSpec,
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
            stem: c.field.stem().to_string(),
        })
        .collect()
}

/// Summarize a project YAML for the UI (name, intent, resolution, data layers).
#[tauri::command]
pub(crate) fn project_summary(yaml: String) -> Result<ProjectSummary, String> {
    let cfg = ProjectConfig::from_yaml(&yaml)?;
    let (nxp, approx_km) = match cfg.target.resolution {
        ResolutionSpec::Nxp(n) => (Some(n), None),
        ResolutionSpec::ApproxKm(k) => (None, Some(k)),
    };
    let (domain, domain_shape, bbox, sea_ratio) = match &cfg.domain {
        DomainConfig::Global => ("global", "global", None, None),
        DomainConfig::Regional {
            shape: RegionShape::Bbox { w, e, n, s },
            sea_ratio,
        } => ("regional", "bbox", Some([*w, *e, *s, *n]), *sea_ratio),
        DomainConfig::Regional {
            shape: RegionShape::Circle { .. },
            ..
        } => ("regional", "circle", None, None),
    };
    let on_violation = cfg.quality.on_violation.as_str().to_string();
    let layers = cfg
        .data_layers
        .iter()
        .map(|l| LayerSummary {
            id: l.id.clone(),
            role_kind: l.role.role_kind().to_string(),
            role: l.role.label(),
            path: l.path.clone(),
            enabled: l.enabled,
            wants_folder: matches!(
                l.role,
                ProjectLayerRole::MeritHydro | ProjectLayerRole::Cama
            ),
        })
        .collect();
    Ok(ProjectSummary {
        name: cfg.metadata.name.clone(),
        authors: cfg.metadata.authors.clone(),
        description: cfg.metadata.description.clone(),
        intent: cfg.target.intent.id().to_string(),
        cell: cfg.target.cell.engine_str().to_string(),
        model_format: cfg.target.model_format.try_engine_str()?.to_string(),
        domain: domain.to_string(),
        domain_shape: domain_shape.to_string(),
        nxp,
        approx_km,
        effective_nxp: cfg.try_lower()?.mkgrd.nxp,
        bbox,
        sea_ratio,
        min_angle_deg: cfg.quality.min_angle_deg,
        on_violation,
        refine_enabled: cfg.refinement.enabled,
        max_passes: cfg.refinement.max_passes,
        layers,
    })
}
