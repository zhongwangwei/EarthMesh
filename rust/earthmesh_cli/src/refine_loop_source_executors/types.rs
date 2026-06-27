use std::path::Path;

use earthmesh_mesh::AreaJudgeSourceBounds;

use crate::{
    AreaJudgeRefineGridRunReport, GetContainRefineFileRunReport, GetContainRuntimeCounts,
    GetRefAtmosThresholdConfig, GetRefIntegratedFileRunReport, GetRefLandBasicConfig,
    GetRefOceanThresholdConfig, GetRefSpecifiedThresholdWriteReport,
};

/// Runtime inputs for executing the already-migrated specified-refinement
/// source branch inside one `mkgrd.F90` refine-loop step.
#[derive(Debug, Clone, Copy)]
pub struct MkgrdSpecifiedRefineSourceExecutorOptions<'a> {
    pub file_dir: &'a Path,
    pub mesh_type: &'a str,
    pub mask_refine_spc_type: &'a str,
    pub mask_refine_ndm: usize,
    pub mask_refine_ndm_by_iter: &'a [usize; 10],
    pub is_in_domain: &'a [Vec<i32>],
    pub seaorland: &'a [Vec<i32>],
    pub lon_vertex: &'a [f64],
    pub lat_vertex: &'a [f64],
    pub lon_i: &'a [f64],
    pub lat_i: &'a [f64],
    pub gridnum_perdegree: usize,
    pub nlons_source: usize,
    pub nlats_source: usize,
    pub num_vertex: usize,
}

/// Evidence from running `Area_judge_refine(step) -> Get_Contain(step) ->
/// GetRef(step)` for the specified-refinement source branch.
#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdSpecifiedRefineSourceBranchReport {
    pub area: AreaJudgeRefineGridRunReport,
    pub contain: GetContainRefineFileRunReport,
    pub specified_threshold: GetRefSpecifiedThresholdWriteReport,
}

/// Runtime inputs for executing the calculated-refinement source branch inside
/// one `mkgrd.F90` refine-loop step.
#[derive(Debug, Clone, Copy)]
pub struct MkgrdCalculatedRefineSourceExecutorOptions<'a> {
    pub file_dir: &'a Path,
    pub mesh_type: &'a str,
    pub threshold_dir: &'a Path,
    pub calculated_refine: (&'a [Vec<i32>], AreaJudgeSourceBounds),
    pub seaorland: &'a [Vec<i32>],
    pub lon_vertex: &'a [f64],
    pub lat_vertex: &'a [f64],
    pub lon_i: &'a [f64],
    pub lat_i: &'a [f64],
    pub num_vertex: usize,
    pub landtypes_global: &'a [Vec<i32>],
    pub refine_onelayer_lnd: &'a [bool],
    pub th_onelayer_lnd: &'a [f64],
    pub refine_twolayer_lnd: &'a [bool],
    pub th_twolayer_lnd: &'a [[f64; 2]],
    pub refine_onelayer_ocn: &'a [bool],
    pub th_onelayer_ocn: &'a [f64],
    pub refine_onelayer_atmos: &'a [bool],
    pub th_onelayer_atmos: &'a [f64],
    pub land_basic_config: GetRefLandBasicConfig,
    pub ocean_config: GetRefOceanThresholdConfig,
    pub atmos_config: GetRefAtmosThresholdConfig,
}

/// Evidence from running `Area_judge_refine(0) -> Get_Contain(0) ->
/// GetRef(0)` for the calculated-refinement source branch.
#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdCalculatedRefineSourceBranchReport {
    pub area: AreaJudgeRefineGridRunReport,
    pub contain: GetContainRefineFileRunReport,
    pub getref: GetRefIntegratedFileRunReport,
}

/// Runtime inputs for dispatching all currently migrated refine source
/// branches in one `mkgrd.F90` refine-loop execution.
#[derive(Debug, Clone, Copy, Default)]
pub struct MkgrdRefineSourceBranchExecutorOptions<'a> {
    pub calculated: Option<MkgrdCalculatedRefineSourceExecutorOptions<'a>>,
    pub specified: Option<MkgrdSpecifiedRefineSourceExecutorOptions<'a>>,
}

/// Evidence from dispatching one migrated refine source branch.
#[derive(Debug, Clone, PartialEq)]
pub enum MkgrdRefineSourceBranchReport {
    Calculated(MkgrdCalculatedRefineSourceBranchReport),
    Specified(MkgrdSpecifiedRefineSourceBranchReport),
}

impl MkgrdRefineSourceBranchReport {
    pub(crate) fn contain_runtime_counts(&self) -> &GetContainRuntimeCounts {
        match self {
            Self::Calculated(report) => &report.contain.runtime_counts,
            Self::Specified(report) => &report.contain.runtime_counts,
        }
    }
}
