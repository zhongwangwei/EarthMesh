use earthmesh_core::{EarthmeshConfig, EarthmeshRuntimeState};

use crate::{
    MaskPostprocOceanDomainReport, MaskRestartRemaskPlan, MkgrdWorkspacePlan,
    WorkspaceMaskApplyReport,
};

/// Report for the migrated top-level `mkgrd.F90` mask-restart branch.
#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdMaskRestartPlanReport {
    pub config: EarthmeshConfig,
    pub runtime_state: EarthmeshRuntimeState,
    pub workspace_plan: MkgrdWorkspacePlan,
    pub remask: MaskRestartRemaskPlan,
}

/// Report for the migrated executable subset of the top-level `mkgrd.F90`
/// mask-restart branch.
#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdMaskRestartOceanRunReport {
    pub plan: MkgrdMaskRestartPlanReport,
    pub runtime_state: EarthmeshRuntimeState,
    pub postproc: MaskPostprocOceanDomainReport,
}

/// Report for executing the `read_nl` mask-restart patch preprocessing branch.
#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdMaskRestartPatchRunReport {
    pub plan: MkgrdMaskRestartPlanReport,
    pub runtime_state: EarthmeshRuntimeState,
    pub workspace_mask: WorkspaceMaskApplyReport,
}
