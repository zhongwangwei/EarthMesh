use std::path::PathBuf;

use earthmesh_mesh::DistanceLayerSpacing;

use crate::MaskPostprocDomainIoPlan;

/// One source branch scheduled inside a `mkgrd.F90` refine-loop step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MkgrdRefineSource {
    /// Fortran calls `Area_judge_refine(0)`, `Get_Contain(0)`, and `GetRef(0)`.
    CalculatedIterZero,
    /// Fortran calls `Area_judge_refine(step)`, `Get_Contain(step)`, and `GetRef(step)`.
    SpecifiedStep,
}

/// Non-destructive schedule for one `mkgrd.F90` refine-loop iteration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MkgrdRefineLoopStepPlan {
    pub step: usize,
    pub max_transition_row: usize,
    pub sources: Vec<MkgrdRefineSource>,
    pub run_refine_loop: bool,
    pub stop_after_step: bool,
}

/// Non-destructive schedule for the `mkgrd.F90` refine loop after the initial
/// gridfile and Area_judge domain setup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MkgrdRefineLoopPlan {
    pub max_iter: usize,
    pub steps: Vec<MkgrdRefineLoopStepPlan>,
    pub final_mask_postproc_step: usize,
}

/// File-level contract for one `Area_judge_refine` -> `Get_Contain` ->
/// `GetRef` source branch inside a `mkgrd.F90` refine-loop step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MkgrdRefineSourceIoPlan {
    pub source: MkgrdRefineSource,
    pub area_judge_iter: usize,
    pub get_contain_iter: usize,
    pub getref_iter: usize,
    pub area_judge_output: PathBuf,
    pub contain_output: PathBuf,
    pub threshold_outputs: Vec<PathBuf>,
    pub specified_threshold_output: Option<PathBuf>,
}

/// File-level contract for one `refine_loop` call and its scratch/final mesh
/// paths in `MOD_Refine.F90`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MkgrdRefineLoopStepIoPlan {
    pub step: usize,
    pub max_transition_row: usize,
    pub sources: Vec<MkgrdRefineSourceIoPlan>,
    pub refine_loop_input_gridfile: PathBuf,
    pub refine_loop_original_tmpfile: PathBuf,
    pub refine_loop_stage2_tmpfile: PathBuf,
    pub refine_loop_stage5_tmpfile: PathBuf,
    pub refine_loop_output_gridfile: PathBuf,
    pub run_refine_loop: bool,
    pub stop_after_step: bool,
}

/// Non-destructive file-level I/O schedule for the top-level `mkgrd.F90` refine
/// loop plus the final `Get_Contain(0)`/`mask_postproc(mesh_type)` handoff.
#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdRefineLoopIoPlan {
    pub file_dir: PathBuf,
    pub nxp: usize,
    pub mesh_type: String,
    pub mode_grid: String,
    pub max_iter: usize,
    pub steps: Vec<MkgrdRefineLoopStepIoPlan>,
    pub final_mask_postproc_step: usize,
    pub final_get_contain_iter: usize,
    pub final_domain_gridfile: PathBuf,
    pub final_result_gridfile: PathBuf,
    pub final_domain_contain_output: PathBuf,
    pub final_quality_check: MkgrdFinalQualityCheckIoPlan,
    pub final_mask_postproc_domain: Option<MaskPostprocDomainIoPlan>,
}

/// Branch selected by `mkgrd.F90:Final_Grid_Quality_Check` before optional
/// global/regional spring adjustment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MkgrdFinalQualitySpringMode {
    SkippedBothDisabled,
    SkippedRegionalEachStep,
    Global,
    RegionalFinal,
}

/// Non-destructive file-level schedule for `Final_Grid_Quality_Check`.
#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdFinalQualityCheckIoPlan {
    pub step: usize,
    pub run_quality_check: bool,
    pub spring_mode: MkgrdFinalQualitySpringMode,
    pub input_gridfile: PathBuf,
    pub original_gridfile: Option<PathBuf>,
    pub quality_before_spring: Option<PathBuf>,
    pub quality_after_spring: Option<PathBuf>,
    pub output_gridfile: Option<PathBuf>,
    pub regional_set_dis: Option<i32>,
    pub global_spring: Option<MkgrdFinalQualityGlobalSpringIoPlan>,
    pub regional_spring: Option<MkgrdFinalQualityRegionalSpringIoPlan>,
    pub regional_source_mask: Option<MkgrdFinalQualityRegionalSourceMaskIoPlan>,
}

/// Source-mask classification inputs for the final regional spring branch.
#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdFinalQualityRegionalSourceMaskIoPlan {
    pub source_lon_vertices: Vec<f64>,
    pub source_lat_vertices: Vec<f64>,
    pub mask_patch: Vec<Vec<bool>>,
    pub first_triangle_id: usize,
}

/// One owned distance-step mask for the final global spring branch.
#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdFinalQualityGlobalDistanceStepIoPlan {
    pub active: bool,
    pub halo: usize,
    pub refinement_flags: Vec<bool>,
    pub num_vertex_in: usize,
    pub num_center_in: usize,
}

/// Resolved namelist-derived controls for the final global spring branch.
#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdFinalQualityGlobalSpringIoPlan {
    pub base_dists_on_edge: f64,
    pub base_cellwidth: Option<f64>,
    pub distance_num_rc: usize,
    pub distance_spacing: DistanceLayerSpacing,
    pub distance_steps: Vec<MkgrdFinalQualityGlobalDistanceStepIoPlan>,
    pub niter_refine: usize,
    pub relax: f64,
    pub radius: f64,
}

/// Resolved namelist-derived controls for the final regional spring branch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MkgrdFinalQualityRegionalSpringIoPlan {
    pub niter_refine: usize,
    pub radius: f64,
}
