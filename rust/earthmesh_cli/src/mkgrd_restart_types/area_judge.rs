use earthmesh_core::EarthmeshRuntimeState;

use crate::{
    AreaJudgeGridWriteReport, AreaJudgeRestartReport, GetContainRefineFileRunReport,
    GetContainRuntimeCounts, MaskPostprocEarthDomainReport, MaskPostprocLandDomainReport,
    MaskPostprocOceanDomainReport, MpasFullMeshPipelineReport, MpasSimpleMeshWriteReport,
    WorkspaceMaskApplyReport,
};

use super::MkgrdMaskRestartPlanReport;

/// Final domain postprocessing result used by the active mask-restart path.
#[derive(Debug, Clone, PartialEq)]
pub enum MkgrdFinalDomainPostprocReport {
    Earth(MaskPostprocEarthDomainReport),
    Land(MaskPostprocLandDomainReport),
    Ocean(MaskPostprocOceanDomainReport),
    /// Native atmosphere mesh retained while a caller defers model export.
    AtmosNative(crate::UnstructuredMeshWriteReport),
    Atmos(MpasSimpleMeshWriteReport),
    AtmosFull(MpasFullMeshPipelineReport),
}

pub use crate::global_source_axes::MkgrdRestartAreaJudgeOptions;

/// Report for the restarted `Area_judge` continuation of the top-level
/// `mkgrd.F90` mask-restart branch.
#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdRestartAreaJudgeRunReport {
    pub plan: MkgrdMaskRestartPlanReport,
    pub runtime_state: EarthmeshRuntimeState,
    pub workspace_mask: WorkspaceMaskApplyReport,
    pub area: AreaJudgeRestartReport,
    pub area_write: AreaJudgeGridWriteReport,
    pub refine_write: Option<AreaJudgeGridWriteReport>,
}

/// Runtime options for composing restarted `Area_judge` with the final
/// `Get_Contain(0)` + `mask_postproc(mesh_type)` handoff.
#[derive(Debug, Clone, Copy)]
pub struct MkgrdRestartAreaJudgePostprocOptions<'a> {
    pub area_judge: MkgrdRestartAreaJudgeOptions<'a>,
    pub num_vertex: usize,
}

/// Evidence from the mask-restart ContinueMkgrd branch after the restarted
/// `Area_judge` selected grid is handed directly to final domain postprocess.
#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdRestartAreaJudgePostprocRunReport {
    pub restart: MkgrdRestartAreaJudgeRunReport,
    pub contain: GetContainRefineFileRunReport,
    pub postproc: MkgrdFinalDomainPostprocReport,
}

/// Evidence for the restart `Area_judge` branch when the CLI/user supplies the
/// global regular-source grid dimensions instead of pre-expanded coordinate
/// arrays.
#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdRestartAreaJudgeGlobalSourceRunReport {
    pub restart: MkgrdRestartAreaJudgeRunReport,
    pub postproc: Option<MkgrdRestartAreaJudgePostprocRunReport>,
}

impl MkgrdRestartAreaJudgePostprocRunReport {
    pub fn final_domain_contain_runtime_counts(&self) -> &GetContainRuntimeCounts {
        &self.contain.runtime_counts
    }

    pub fn runtime_state(&self) -> &EarthmeshRuntimeState {
        &self.restart.runtime_state
    }
}

impl MkgrdRestartAreaJudgeGlobalSourceRunReport {
    pub fn final_domain_contain_runtime_counts(&self) -> Option<&GetContainRuntimeCounts> {
        self.postproc
            .as_ref()
            .map(MkgrdRestartAreaJudgePostprocRunReport::final_domain_contain_runtime_counts)
    }

    pub fn runtime_state(&self) -> &EarthmeshRuntimeState {
        self.postproc
            .as_ref()
            .map(MkgrdRestartAreaJudgePostprocRunReport::runtime_state)
            .unwrap_or(&self.restart.runtime_state)
    }
}
