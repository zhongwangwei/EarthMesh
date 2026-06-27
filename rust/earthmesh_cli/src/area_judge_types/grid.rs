use std::path::{Path, PathBuf};

use earthmesh_mesh::AreaJudgeSourceBounds;

use super::{
    AreaJudgeCalculatedRefineConfig, AreaJudgeNonRestartReport, AreaJudgePatchConfig,
    AreaJudgeRefineStepReport, AreaJudgeRestartReport,
};

/// Runtime inputs for a non-restart Area_judge grid-file orchestration run.
#[derive(Debug, Clone, Copy)]
pub struct AreaJudgeGridRunConfig<'a> {
    pub file_dir: &'a Path,
    pub mask_domain_global: bool,
    pub mask_domain_type: &'a str,
    pub mask_domain_ndm: usize,
    pub mask_patch: Option<AreaJudgePatchConfig<'a>>,
    pub refine: bool,
    pub calculated_refine: Option<AreaJudgeCalculatedRefineConfig<'a>>,
    pub landtypes_global: &'a [Vec<i32>],
    pub mesh_type: &'a str,
    pub lon_vertex: &'a [f64],
    pub lat_vertex: &'a [f64],
    pub lon_i: &'a [f64],
    pub lat_i: &'a [f64],
    pub gridnum_perdegree: usize,
    pub nlons_source: usize,
    pub nlats_source: usize,
    pub domain_output: Option<&'a Path>,
    pub refine_output: Option<&'a Path>,
}

/// Evidence from writing a selected Area_judge grid payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaJudgeGridWriteReport {
    pub output: PathBuf,
    pub bounds: AreaJudgeSourceBounds,
    pub nlons_select: usize,
    pub nlats_select: usize,
    pub selected_cells: usize,
    pub has_seaorland: bool,
}

/// Evidence from a non-restart Area_judge run that writes selected grid files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaJudgeGridRunReport {
    pub area: AreaJudgeNonRestartReport,
    pub refine_step: Option<AreaJudgeRefineStepReport>,
    pub domain_write: Option<AreaJudgeGridWriteReport>,
    pub refine_write: Option<AreaJudgeGridWriteReport>,
}

/// Runtime inputs for a restart Area_judge grid-file orchestration run.
#[derive(Debug, Clone, Copy)]
pub struct AreaJudgeRestartGridsRunConfig<'a> {
    pub file_dir: &'a Path,
    pub restart_input: &'a Path,
    pub mask_patch: Option<AreaJudgePatchConfig<'a>>,
    pub refine: bool,
    pub calculated_refine: Option<AreaJudgeCalculatedRefineConfig<'a>>,
    pub lon_vertex: &'a [f64],
    pub lat_vertex: &'a [f64],
    pub lon_i: &'a [f64],
    pub lat_i: &'a [f64],
    pub gridnum_perdegree: usize,
    pub nlons_source: usize,
    pub nlats_source: usize,
    pub domain_output: Option<&'a Path>,
    pub refine_output: Option<&'a Path>,
}

/// Evidence from a restart Area_judge run that writes selected grid files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaJudgeRestartGridsRunReport {
    pub area: AreaJudgeRestartReport,
    pub refine_step: Option<AreaJudgeRefineStepReport>,
    pub domain_write: Option<AreaJudgeGridWriteReport>,
    pub refine_write: Option<AreaJudgeGridWriteReport>,
}

/// Runtime inputs for writing one `Area_judge_refine(iter)` selected grid.
#[derive(Debug, Clone, Copy)]
pub struct AreaJudgeRefineGridRunConfig<'a> {
    pub file_dir: &'a Path,
    pub iter: usize,
    pub calculated_refine: Option<(&'a [Vec<i32>], AreaJudgeSourceBounds)>,
    pub mask_refine_spc_type: &'a str,
    pub mask_refine_ndm: usize,
    pub is_in_domain: &'a [Vec<i32>],
    pub lon_vertex: &'a [f64],
    pub lat_vertex: &'a [f64],
    pub lon_i: &'a [f64],
    pub lat_i: &'a [f64],
    pub gridnum_perdegree: usize,
    pub nlons_source: usize,
    pub nlats_source: usize,
    pub refine_output: &'a Path,
}

/// Evidence from an `Area_judge_refine(iter)` grid-file run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaJudgeRefineGridRunReport {
    pub refine_step: AreaJudgeRefineStepReport,
    pub refine_write: AreaJudgeGridWriteReport,
}
