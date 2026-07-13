use earthmesh_core::EarthmeshRuntimeState;

use crate::{
    GetContainRuntimeCounts, MkgrdMaskRestartOceanRunReport, MkgrdMaskRestartPatchRunReport,
    MkgrdMaskRestartPlanReport, MkgrdRestartAreaJudgeGlobalSourceRunReport,
};

use super::{MkgrdGridinitRunReport, RefinePipelineRunReport};

/// Branch selected by the current top-level `mkgrd.x` namelist dispatcher.
#[derive(Debug, Clone, PartialEq)]
pub enum MkgrdTopLevelDispatchRunReport {
    /// Normal non-restart grid initialization branch.
    Gridinit(MkgrdGridinitRunReport),
    /// Non-restart refinement generated directly by the Method-C
    /// Delaunay/Voronoi pipeline.
    RefinePipeline(RefinePipelineRunReport),
    /// `mask_restart=.true.` with `mask_patch_on=.true.` runs the current patch `Mask_make`
    /// pre-processing and then hands control back to later restart continuation surfaces.
    MaskRestartPatch(MkgrdMaskRestartPatchRunReport),
    /// `mask_restart=.true.` oceanmesh without mask patches runs the current ocean
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
    /// Default non-restart atmosphere refine generated directly by the Method-C
    /// Delaunay/Voronoi pipeline.
    RefinePipeline(RefinePipelineRunReport),
}

impl MkgrdTopLevelDispatchRunReport {
    pub fn final_domain_contain_runtime_counts(&self) -> Option<&GetContainRuntimeCounts> {
        match self {
            MkgrdTopLevelDispatchRunReport::MaskRestartAreaJudge(report) => {
                report.final_domain_contain_runtime_counts()
            }
            MkgrdTopLevelDispatchRunReport::Gridinit(_)
            | MkgrdTopLevelDispatchRunReport::RefinePipeline(_)
            | MkgrdTopLevelDispatchRunReport::MaskRestartPatch(_)
            | MkgrdTopLevelDispatchRunReport::MaskRestartOcean(_)
            | MkgrdTopLevelDispatchRunReport::MaskRestartPlan(_) => None,
        }
    }

    pub fn runtime_state(&self) -> Option<&EarthmeshRuntimeState> {
        match self {
            MkgrdTopLevelDispatchRunReport::Gridinit(report) => report.runtime_state.as_ref(),
            MkgrdTopLevelDispatchRunReport::RefinePipeline(report) => Some(&report.runtime_state),
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
    pub fn runtime_state(&self) -> Option<&EarthmeshRuntimeState> {
        match self {
            MkgrdTopLevelDefaultRestartRefineRunReport::Dispatch(report) => report.runtime_state(),
            MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(run) => {
                Some(run.runtime_state())
            }
        }
    }

    pub fn final_domain_contain_runtime_counts(&self) -> Option<&GetContainRuntimeCounts> {
        match self {
            MkgrdTopLevelDefaultRestartRefineRunReport::Dispatch(report) => {
                report.final_domain_contain_runtime_counts()
            }
            MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(_) => None,
        }
    }
}
