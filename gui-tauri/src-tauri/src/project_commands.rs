//! Project intent/schema command handlers.

use earthmesh_project::{
    DomainConfig, MeshIntentPreset, ProjectConfig, RegionShape, ResolutionSpec,
};

fn validated_yaml(cfg: ProjectConfig) -> Result<String, String> {
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
    cfg.hydro_coast = base.hydro_coast;
    cfg.coupling = base.coupling;

    if base.target.intent == cfg.target.intent {
        cfg.target.kind = base.target.kind;
        cfg.target.cell = base.target.cell;
        cfg.target.model_format = base.target.model_format;

        for layer in base.data_layers {
            if !cfg.data_layers.iter().any(|l| l.id == layer.id) {
                cfg.data_layers.push(layer);
            }
        }
    }

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
