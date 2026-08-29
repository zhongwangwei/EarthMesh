//! Project intent/schema command handlers.

use earthmesh_project::{
    DomainConfig, MeshDomainKind, MeshIntentPreset, MethodCAlgorithm, ProjectConfig, RegionShape,
    ResolutionSpec,
};

pub(crate) fn validated_yaml(cfg: ProjectConfig) -> Result<String, String> {
    cfg.validate()?;
    cfg.to_yaml()
}

/// Scaffold a project from an intent preset and serialize it to YAML.
/// The frontend applies the visible domain with `set_domain_*` after scaffolding.
#[tauri::command]
pub(crate) fn scaffold_project(
    name: String,
    intent: String,
    nxp: Option<i32>,
    approx_km: Option<f64>,
    approx_degree: Option<f64>,
) -> Result<String, String> {
    let resolution = match (approx_degree, approx_km) {
        (Some(degrees), _) => ResolutionSpec::ApproxDegree(degrees),
        (None, Some(km)) => ResolutionSpec::ApproxKm(km),
        (None, None) => ResolutionSpec::Nxp(nxp.unwrap_or(80)),
    };
    let cfg = ProjectConfig::scaffold(
        &name,
        MeshIntentPreset::from_id(&intent)
            .ok_or_else(|| format!("unknown mesh intent '{intent}'"))?,
        DomainConfig::Global,
        resolution,
    );
    validated_yaml(cfg)
}

/// Validate a project YAML — returns the canonical re-serialized YAML on success,
/// or a human-readable parse error. Used by the GUI summary/open validation path.
#[tauri::command]
pub(crate) fn validate_project(yaml: String) -> Result<String, String> {
    ProjectConfig::from_yaml(&yaml)?.to_yaml()
}

/// Update name/authors/description only; used by project metadata forms.
#[tauri::command]
pub(crate) fn set_project_metadata(
    yaml: String,
    name: String,
    authors: Vec<String>,
    description: String,
) -> Result<String, String> {
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    cfg.metadata.name = name;
    cfg.metadata.authors = authors
        .into_iter()
        .map(|author| author.trim().to_string())
        .filter(|author| !author.is_empty())
        .collect();
    cfg.metadata.description = description;
    validated_yaml(cfg)
}

/// Preserve schema fields the current Studio UI does not expose yet.
///
/// The frontend composes YAML from visible controls. When the user opens an
/// existing project and saves/runs without editing advanced sections, we should
/// not silently drop carried-but-hidden project fields such as hydro/coupling
/// options, expert overrides, unsupported domain shapes, or custom data layers.
#[tauri::command]
pub(crate) fn preserve_unexposed_project_fields(
    base_yaml: String,
    yaml: String,
    preserve_domain: bool,
) -> Result<String, String> {
    let base = ProjectConfig::from_yaml(&base_yaml)?;
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;

    cfg.expert = base.expert;
    cfg.refinement.threshold_criteria = base.refinement.threshold_criteria.clone();
    cfg.refinement.method_c = base.refinement.method_c.clone();
    cfg.refinement.harp_dv = base.refinement.harp_dv.clone();
    cfg.refinement.certified = base.refinement.certified.clone();
    if cfg.refinement.specified_circle.is_none()
        && cfg.refinement.specified_bbox.is_none()
        && cfg.refinement.specified_close.is_none()
    {
        cfg.refinement.specified_circle = base.refinement.specified_circle.clone();
        cfg.refinement.specified_bbox = base.refinement.specified_bbox.clone();
        cfg.refinement.specified_close = base.refinement.specified_close.clone();
    }

    if let Some(base_adaptive) = base.refinement.adaptive.as_ref() {
        let adaptive = cfg.refinement.adaptive.get_or_insert_with(Default::default);
        adaptive.base_m = base_adaptive.base_m;
    }

    if let Some(base_hfield) = base.refinement.hfield.as_ref() {
        let hfield = cfg.refinement.hfield.get_or_insert_with(Default::default);
        hfield.origin_lon = base_hfield.origin_lon;
        hfield.origin_lat = base_hfield.origin_lat;
        hfield.nlon = base_hfield.nlon;
        hfield.nlat = base_hfield.nlat;
    }

    if base.target.intent == cfg.target.intent {
        cfg.target.kind = base.target.kind;
        cfg.target.cell = base.target.cell;
        cfg.target.model_format = base.target.model_format;
    }

    for layer in base.data_layers {
        if !cfg
            .data_layers
            .iter()
            .any(|candidate| candidate.id == layer.id)
        {
            if layer.enabled
                && matches!(
                    layer.role,
                    earthmesh_project::ProjectLayerRole::Threshold(_)
                        | earthmesh_project::ProjectLayerRole::LandType
                )
            {
                for sibling in &mut cfg.data_layers {
                    if sibling.role == layer.role {
                        sibling.enabled = false;
                    }
                }
            }
            cfg.data_layers.push(layer);
        }
    }

    cfg.hydro_coast = matches!(cfg.domain, DomainConfig::Regional { .. })
        .then_some(base.hydro_coast)
        .flatten();
    cfg.coupling = (cfg.target.kind == MeshDomainKind::Coupled)
        .then_some(base.coupling)
        .flatten();

    let preserves_unexposed_shape = matches!(
        &base.domain,
        DomainConfig::Regional {
            shape: RegionShape::Circle { .. }
                | RegionShape::Shapefile { .. }
                | RegionShape::Close { .. },
            ..
        }
    ) && matches!(cfg.domain, DomainConfig::Global);
    if preserve_domain && preserves_unexposed_shape {
        cfg.domain = base.domain;
    }

    validated_yaml(cfg)
}

/// Preserve hidden quality settings after visible route/backend/domain setters have
/// made the intermediate project valid.
#[tauri::command]
pub(crate) fn preserve_unexposed_quality_fields(
    base_yaml: String,
    yaml: String,
) -> Result<String, String> {
    let base = ProjectConfig::from_yaml(&base_yaml)?;
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    let Some(lepp) = base.quality.lepp_post_quality else {
        return validated_yaml(cfg);
    };
    if cfg.refinement.enabled
        && cfg.refinement.backend == earthmesh_project::RefinementBackend::MethodC
        && cfg.refinement.method_c.algorithm != MethodCAlgorithm::LeppDelaunay
        && matches!(cfg.domain, DomainConfig::Global)
    {
        cfg.quality.lepp_post_quality = Some(lepp);
    }
    validated_yaml(cfg)
}
