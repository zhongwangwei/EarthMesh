use earthmesh_core::{EarthmeshRuntimeState, RefineConfig};

use crate::{
    GetContainRuntimeCounts, MkgrdMaskRestartOceanRunReport, MkgrdMaskRestartPatchRunReport,
    MkgrdMaskRestartPlanReport, MkgrdRefineLoopExecutionReport, MkgrdRefineLoopIoPlan,
    MkgrdRefineSourceBranchReport, MkgrdRestartAreaJudgeGlobalSourceRunReport,
};

use super::{MkgrdGridinitRunReport, MkgrdOlamSpecifiedRefineRunReport};

/// Source-grid geometry needed to complete prepare-time refine-loop
/// enrichments that Fortran derives from module state after `read_nl`.
#[derive(Debug, Clone, Copy)]
pub struct MkgrdRefinePrepareSourceGridOptions<'a> {
    pub lon_vertex: &'a [f64],
    pub lat_vertex: &'a [f64],
    pub lon_i: &'a [f64],
    pub lat_i: &'a [f64],
    pub gridnum_perdegree: usize,
    pub nlons_source: usize,
    pub nlats_source: usize,
    pub first_triangle_id: usize,
}

/// Prepared state for the migrated `mkgrd.F90` refine path after namelist
/// parsing and `read_nl` workspace/mask side effects.
#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdRefineLoopPrepareReport {
    pub config: earthmesh_core::EarthmeshConfig,
    pub refine: RefineConfig,
    pub runtime_state: EarthmeshRuntimeState,
    pub workspace_mask: crate::WorkspaceMaskApplyReport,
    pub plan: MkgrdRefineLoopIoPlan,
    pub final_source_mask_injected: bool,
}

/// Evidence from running the namelist-level migrated refine path with a
/// supplied refine-loop executor.
#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdRefineLoopNamelistRunReport {
    pub prepare: MkgrdRefineLoopPrepareReport,
    pub execution: MkgrdRefineLoopExecutionReport,
}

impl MkgrdRefineLoopNamelistRunReport {
    pub fn source_branch_reports(&self) -> &[MkgrdRefineSourceBranchReport] {
        &self.execution.source_branch_reports
    }

    pub fn final_domain_contain_runtime_counts(&self) -> Option<&GetContainRuntimeCounts> {
        self.execution
            .final_handoff
            .generated_contain
            .as_ref()
            .map(|contain| &contain.runtime_counts)
    }

    pub fn runtime_state(&self) -> &EarthmeshRuntimeState {
        self.execution
            .runtime_state
            .as_ref()
            .unwrap_or(&self.prepare.runtime_state)
    }
}

/// Branch selected by the migrated top-level `mkgrd.x` namelist dispatcher.
#[derive(Debug, Clone, PartialEq)]
pub enum MkgrdTopLevelDispatchRunReport {
    /// Normal non-restart grid initialization branch.
    Gridinit(MkgrdGridinitRunReport),
    /// Non-restart refinement generated directly by the OLAM
    /// Delaunay/Voronoi pipeline.
    OlamRefineGlobalSource(MkgrdOlamSpecifiedRefineRunReport),
    /// `mask_restart=.true.` with `mask_patch_on=.true.` runs the migrated patch `Mask_make`
    /// pre-processing and then hands control back to later restart continuation surfaces.
    MaskRestartPatch(MkgrdMaskRestartPatchRunReport),
    /// `mask_restart=.true.` oceanmesh without mask patches runs the migrated ocean
    /// `mask_postproc` branch directly.
    MaskRestartOcean(MkgrdMaskRestartOceanRunReport),
    /// `mask_restart=.true.` non-ocean continuation without patch preprocessing
    /// can reconstruct configured global source axes and rerun restarted
    /// `Area_judge` without caller-supplied CLI geometry overrides.
    MaskRestartAreaJudge(MkgrdRestartAreaJudgeGlobalSourceRunReport),
    /// Restart branches that need caller-supplied postprocess/source-grid options are planned
    /// but not executed by the option-free dispatcher.
    MaskRestartPlan(MkgrdMaskRestartPlanReport),
}

/// Evidence from the top-level `mkgrd.x` dispatcher after applying the
/// default restart-refine handoff rules used by the CLI front-end.
#[derive(Debug, Clone, PartialEq)]
pub enum MkgrdTopLevelDefaultRestartRefineRunReport {
    /// No default restart-refine handoff was selected; normal top-level
    /// dispatch handled the namelist.
    Dispatch(MkgrdTopLevelDispatchRunReport),
    /// Default non-restart atmosphere refine generated directly by the OLAM
    /// Delaunay/Voronoi pipeline.
    OlamRefineGlobalSource(MkgrdOlamSpecifiedRefineRunReport),
}

impl MkgrdTopLevelDispatchRunReport {
    pub fn source_branch_reports(&self) -> &[MkgrdRefineSourceBranchReport] {
        &[]
    }

    pub fn final_domain_contain_runtime_counts(&self) -> Option<&GetContainRuntimeCounts> {
        match self {
            MkgrdTopLevelDispatchRunReport::MaskRestartAreaJudge(report) => {
                report.final_domain_contain_runtime_counts()
            }
            MkgrdTopLevelDispatchRunReport::Gridinit(_)
            | MkgrdTopLevelDispatchRunReport::OlamRefineGlobalSource(_)
            | MkgrdTopLevelDispatchRunReport::MaskRestartPatch(_)
            | MkgrdTopLevelDispatchRunReport::MaskRestartOcean(_)
            | MkgrdTopLevelDispatchRunReport::MaskRestartPlan(_) => None,
        }
    }

    pub fn runtime_state(&self) -> Option<&EarthmeshRuntimeState> {
        match self {
            MkgrdTopLevelDispatchRunReport::Gridinit(report) => report.runtime_state.as_ref(),
            MkgrdTopLevelDispatchRunReport::OlamRefineGlobalSource(report) => {
                Some(&report.runtime_state)
            }
            MkgrdTopLevelDispatchRunReport::MaskRestartPatch(report) => Some(&report.runtime_state),
            MkgrdTopLevelDispatchRunReport::MaskRestartOcean(report) => Some(&report.runtime_state),
            MkgrdTopLevelDispatchRunReport::MaskRestartAreaJudge(report) => {
                Some(report.runtime_state())
            }
            MkgrdTopLevelDispatchRunReport::MaskRestartPlan(report) => Some(&report.runtime_state),
        }
    }
}

impl MkgrdTopLevelDefaultRestartRefineRunReport {
    pub fn source_branch_reports(&self) -> &[MkgrdRefineSourceBranchReport] {
        match self {
            MkgrdTopLevelDefaultRestartRefineRunReport::Dispatch(report) => {
                report.source_branch_reports()
            }
            MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(run) => {
                run.source_branch_reports()
            }
        }
    }

    pub fn runtime_state(&self) -> Option<&EarthmeshRuntimeState> {
        match self {
            MkgrdTopLevelDefaultRestartRefineRunReport::Dispatch(report) => report.runtime_state(),
            MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(run) => {
                Some(run.runtime_state())
            }
        }
    }

    pub fn final_domain_contain_runtime_counts(&self) -> Option<&GetContainRuntimeCounts> {
        match self {
            MkgrdTopLevelDefaultRestartRefineRunReport::Dispatch(report) => {
                report.final_domain_contain_runtime_counts()
            }
            MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(_) => None,
        }
    }
}
