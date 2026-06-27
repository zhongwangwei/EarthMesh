use earthmesh_core::EarthmeshRuntimeState;

use crate::MkgrdRefineSourceBranchReport;

use super::final_domain::MkgrdRefineLoopFinalDomainHandoffReport;

/// Evidence from dispatching the migrated top-level `mkgrd.F90` refine loop.
#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdRefineLoopExecutionReport {
    pub executed_sources: usize,
    pub source_branch_reports: Vec<MkgrdRefineSourceBranchReport>,
    pub runtime_state: Option<EarthmeshRuntimeState>,
    pub executed_refine_steps: usize,
    pub ran_final_quality_check: bool,
    pub final_handoff: MkgrdRefineLoopFinalDomainHandoffReport,
}
