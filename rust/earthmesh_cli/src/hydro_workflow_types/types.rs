use std::collections::BTreeMap;
use std::path::PathBuf;

/// One hydro-mesh QA gate result (faithful to `qa_gates.py::_check`).
#[derive(Debug, Clone)]
pub struct HydroMeshQaCheck {
    pub id: String,
    pub passed: bool,
    pub observed: String,
    pub expected: Option<String>,
}

/// Delivery-package QA report (faithful port of `util/hydro_mesh/qa_gates.py`).
/// Distinct from `earthmesh_quality::coupling` (R7), which validates per-cell coupling;
/// this gates a *delivery package* (mask completeness, known surfaces, non-empty
/// river/coast overlays) before promotion.
#[derive(Debug, Clone)]
pub struct HydroMeshQaReport {
    pub status: String,
    pub background_cell_count: i64,
    pub complete_mask_cell_count: i64,
    pub surface_class_counts: BTreeMap<String, i64>,
    pub river_overlap_cells: i64,
    pub coast_overlap_cells: i64,
    pub min_river_cells: i64,
    pub min_coast_cells: i64,
    pub colm_rows_written: Option<i64>,
    pub checks: Vec<HydroMeshQaCheck>,
}

/// Artifacts + summary produced by [`run_hydro_workflow`](crate::run_hydro_workflow).
#[derive(Clone, Debug)]
pub struct HydroWorkflowReport {
    pub intersection_cells: usize,
    pub coupling_rows: usize,
    pub cells_refined: usize,
    pub refinement_max_level: u8,
    /// R7 verdict (pass/warn/fail), present only when mesh + land-type were supplied.
    pub coupling_quality_verdict: Option<String>,
    pub intersections_path: PathBuf,
    pub coupling_csv_path: PathBuf,
    pub refinement_plan_path: PathBuf,
    pub coupling_quality_path: Option<PathBuf>,
    pub manifest_path: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct HydroBackgroundSummary {
    pub cell_count: usize,
    pub size_km_min: Option<f64>,
    pub size_km_median: Option<f64>,
    pub size_km_max: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct HydroIntersectionSummary {
    pub feature_count: usize,
    pub class_counts: BTreeMap<String, usize>,
    pub fraction_min: Option<f64>,
    pub fraction_median: Option<f64>,
    pub fraction_max: Option<f64>,
    pub area_sum: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct RankedSweepCase {
    pub case_name: String,
    pub status: String,
    pub promotion_status: String,
    pub background_cell_count: i64,
    pub background_median_dx_km: f64,
    pub river_overlap_cells: i64,
    pub coast_overlap_cells: i64,
    pub retained: [i64; 3],
    pub rank: usize,
}
