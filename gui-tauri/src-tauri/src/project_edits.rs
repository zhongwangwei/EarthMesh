//! Project YAML edit command handlers.

use earthmesh_project::{
    default_mask_sea_ratio, DomainConfig, ProjectConfig, RegionShape, ViolationPolicy,
};

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

/// Set the quality gate (min angle + on-violation policy), returning the YAML.
#[tauri::command]
pub(crate) fn set_quality(yaml: String, min_angle_deg: f64, block: bool) -> Result<String, String> {
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    cfg.quality.min_angle_deg = min_angle_deg;
    cfg.quality.on_violation = if block {
        ViolationPolicy::Block
    } else {
        ViolationPolicy::Warn
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
