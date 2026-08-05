use serde::{Deserialize, Serialize};

use crate::quality::MeshQuality;

/// A refinement criterion, flattened for the data-layer / quality UI.
#[derive(Serialize)]
pub(crate) struct CriterionInfo {
    pub(crate) id: String,
    pub(crate) source_stem: String,
    pub(crate) statistic: String,
    pub(crate) physical_process: String,
    pub(crate) label: String,
    pub(crate) help: String,
    pub(crate) unit: String,
    pub(crate) range_min: f64,
    pub(crate) range_max: f64,
    pub(crate) default_value: f64,
}

/// Backend-owned project defaults and refinement limits consumed at GUI startup.
#[derive(Serialize)]
pub(crate) struct ProjectCapabilities {
    pub(crate) intent_ids: Vec<String>,
    pub(crate) target_presets: Vec<TargetPresetInfo>,
    pub(crate) target_compatibility: Vec<TargetCompatibilityInfo>,
    pub(crate) default_sea_ratio: f64,
    pub(crate) default_min_angle_deg: f64,
    pub(crate) method_c_min_base_nxp: i32,
    pub(crate) method_c_max_refinement_level: u8,
    pub(crate) default_openmp: i32,
    pub(crate) default_niter: i32,
    pub(crate) default_beta: f64,
    pub(crate) default_relax: f64,
    pub(crate) default_hfield_g: f64,
    pub(crate) method_c_spring_nxp1_km: f64,
    pub(crate) km_per_degree_equator: f64,
}

/// Canonical target defaults attached to one intent preset.
#[derive(Serialize)]
pub(crate) struct TargetPresetInfo {
    pub(crate) intent: String,
    pub(crate) kind: String,
    pub(crate) cell: String,
    pub(crate) model_format: String,
}

/// Specialized writer support for one output model.
#[derive(Serialize)]
pub(crate) struct TargetCompatibilityInfo {
    pub(crate) model_format: String,
    pub(crate) specialized_cells: Vec<String>,
}

/// A loaded project: canonical YAML plus the path it came from.
#[derive(Serialize)]
pub(crate) struct OpenedProject {
    pub(crate) path: String,
    pub(crate) yaml: String,
}

/// One data layer, flattened for the live layer panel.
#[derive(Serialize)]
pub(crate) struct LayerSummary {
    pub(crate) id: String,
    pub(crate) role_kind: String,
    pub(crate) source_field: Option<String>,
    pub(crate) role: String,
    pub(crate) path: String,
    pub(crate) enabled: bool,
    pub(crate) threshold_value: Option<f64>,
    /// True for tiled inputs (MERIT-Hydro, CaMa) that are directories of tiles,
    /// so the UI offers a folder picker instead of a file picker.
    pub(crate) wants_folder: bool,
}

/// One effective threshold criterion. Continuous sources share the path in
/// `LayerSummary`; categorical landcover uses the same independent switch/value
/// shape while its source remains available separately as a mask layer.
#[derive(Serialize)]
pub(crate) struct ThresholdCriterionSummary {
    pub(crate) id: String,
    pub(crate) source_id: String,
    pub(crate) statistic: String,
    pub(crate) source_enabled: bool,
    pub(crate) enabled: bool,
    pub(crate) value: f64,
}

/// A project at a glance — used to reflect a loaded YAML back into the UI.
#[derive(Serialize)]
pub(crate) struct ProjectSummary {
    pub(crate) name: String,
    pub(crate) authors: Vec<String>,
    pub(crate) description: String,
    pub(crate) intent: String,
    pub(crate) target_kind: String,
    pub(crate) cell: String,
    pub(crate) quality_mode: String,
    pub(crate) model_format: String,
    pub(crate) delivery_status: String,
    pub(crate) skipped_adapter_reason: Option<String>,
    pub(crate) domain: String,
    pub(crate) domain_shape: String,
    pub(crate) nxp: Option<i32>,
    pub(crate) approx_km: Option<f64>,
    pub(crate) approx_degree: Option<f64>,
    pub(crate) effective_nxp: i32,
    /// `[w, e, s, n]` when the domain is a regional bounding box, else `None`.
    pub(crate) bbox: Option<[f64; 4]>,
    pub(crate) watershed_path: Option<String>,
    pub(crate) close_format: Option<String>,
    pub(crate) domain_close_boundary: Option<earthmesh_project::CloseBoundaryMode>,
    pub(crate) sea_ratio: Option<f64>,
    pub(crate) min_angle_deg: f64,
    pub(crate) auto_refine_batch_cells: usize,
    pub(crate) on_violation: String,
    pub(crate) refine_enabled: bool,
    pub(crate) threshold_refine_enabled: bool,
    pub(crate) threshold_criteria: Vec<ThresholdCriterionSummary>,
    pub(crate) hydro_river_refine_enabled: bool,
    pub(crate) hydro_river_width_refine_enabled: bool,
    pub(crate) hydro_river_upstream_area_refine_enabled: bool,
    pub(crate) hydro_river_width_threshold_m: Option<f64>,
    pub(crate) hydro_river_upstream_area_threshold_km2: Option<f64>,
    pub(crate) hydro_coast_refine_enabled: bool,
    pub(crate) hydro_coast_buffer_km: Option<f64>,
    pub(crate) hydro_coast_land_refine_enabled: bool,
    pub(crate) hydro_coast_ocean_refine_enabled: bool,
    pub(crate) hydro_r2_width_m: Option<f64>,
    pub(crate) hydro_r2_upa_km2: Option<f64>,
    pub(crate) hydro_r3_width_m: Option<f64>,
    pub(crate) hydro_r3_upa_km2: Option<f64>,
    pub(crate) max_passes: u8,
    pub(crate) specified_refine_enabled: bool,
    pub(crate) specified_refine_kind: String,
    pub(crate) specified_refine_lon: Option<f64>,
    pub(crate) specified_refine_lat: Option<f64>,
    pub(crate) specified_refine_radius_km: Option<f64>,
    pub(crate) specified_refine_bbox: Option<[f64; 4]>,
    pub(crate) specified_refine_path: Option<String>,
    pub(crate) specified_refine_close_boundary: Option<earthmesh_project::CloseBoundaryMode>,
    pub(crate) hfield_enabled: bool,
    pub(crate) hfield_g: Option<f64>,
    pub(crate) hfield_max_level: Option<u8>,
    pub(crate) hfield_base_m: Option<f64>,
    pub(crate) expert_nxp: Option<i32>,
    pub(crate) expert_openmp: Option<i32>,
    pub(crate) expert_niter: Option<i32>,
    pub(crate) expert_niter_refine: Option<i32>,
    pub(crate) expert_max_iter_spc: Option<i32>,
    pub(crate) expert_max_iter_cal: Option<i32>,
    pub(crate) expert_halo: Option<Vec<i32>>,
    pub(crate) expert_max_transition_row: Option<Vec<i32>>,
    pub(crate) expert_set_dis_type: Option<String>,
    pub(crate) expert_num_rc: Option<i32>,
    pub(crate) expert_vertex_pretect_layers: Option<i32>,
    pub(crate) expert_spring_global_type: Option<i32>,
    pub(crate) expert_spring_regional_type: Option<i32>,
    pub(crate) expert_beta: Option<f64>,
    pub(crate) expert_relax: Option<f64>,
    pub(crate) expert_weak_concav_eliminate: Option<bool>,
    pub(crate) layers: Vec<LayerSummary>,
}

/// Outcome of a mesh run: exit status + where the outputs landed.
#[derive(Serialize)]
pub(crate) struct RunResult {
    pub(crate) ok: bool,
    pub(crate) code: Option<i32>,
    pub(crate) outdir: String,
    /// The gridfile the engine reported (`gridfile=<path>` on stdout), so the GUI
    /// can run quality + draw the mesh without re-globbing. None if not seen.
    pub(crate) gridfile: Option<String>,
    pub(crate) delivery: Option<String>,
    pub(crate) specialized_outputs: Vec<String>,
    pub(crate) skipped_adapter_reason: Option<String>,
    /// Authoritative Project-aware quality report emitted by the engine.
    pub(crate) final_quality: Option<MeshQuality>,
    /// Every candidate-selection decision produced by the shared AutoRefine
    /// loop, ordered by pass and artifact path. Empty for non-AutoRefine runs.
    pub(crate) auto_refine_decisions: Vec<AutoRefineDecision>,
}

/// One guarded metric that made an AutoRefine candidate non-acceptable.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct AutoRefineRegression {
    pub(crate) metric: String,
    pub(crate) preferred: String,
    pub(crate) baseline: Option<f64>,
    pub(crate) candidate: Option<f64>,
    pub(crate) delta: Option<f64>,
}

/// Machine-readable candidate-selection record emitted by the CLI.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct AutoRefineDecision {
    #[serde(default)]
    pub(crate) schema_version: Option<u32>,
    pub(crate) kind: String,
    pub(crate) pass: u8,
    pub(crate) decision: String,
    pub(crate) reason: String,
    #[serde(default)]
    pub(crate) regressions: Vec<AutoRefineRegression>,
    pub(crate) baseline_gridfile: Option<String>,
    pub(crate) candidate_gridfile: String,
    pub(crate) selected_gridfile: String,
    pub(crate) baseline_quality_report: Option<String>,
    pub(crate) candidate_quality_report: String,
    pub(crate) selected_quality_report: String,
    pub(crate) baseline_verdict: Option<String>,
    pub(crate) candidate_verdict: String,
    pub(crate) selected_verdict: String,
    /// Full path to the decision JSON, added by the GUI scanner rather than
    /// stored inside the artifact itself.
    #[serde(default)]
    pub(crate) artifact_path: String,
}
