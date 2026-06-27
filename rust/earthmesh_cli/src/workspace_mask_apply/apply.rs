use std::io;
use std::path::Path;

use earthmesh_core::MkgrdWorkspacePlan;

use crate::{
    apply_mask_operation, apply_read_nl_workspace_plan, validate_mask_refine_reaches_max_iter_spc,
    MaskCountState, MaskOperationReport, WorkspaceApplyReport,
};

/// Combined report for workspace setup followed by planned mask operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceMaskApplyReport {
    pub workspace: WorkspaceApplyReport,
    pub mask_reports: Vec<MaskOperationReport>,
    pub mask_counts: MaskCountState,
}

/// Apply the Rust `read_nl` workspace plan and then execute every planned
/// `Mask_make` operation in order.
pub fn apply_workspace_and_mask_operations(
    plan: &MkgrdWorkspacePlan,
    namelist_source: &Path,
    workdir: &Path,
    max_iter_spc: usize,
    validate_refine_max_iter: bool,
) -> io::Result<WorkspaceMaskApplyReport> {
    let workspace = apply_read_nl_workspace_plan(plan, namelist_source, workdir)?;
    let mut mask_counts = MaskCountState::default();
    let mut mask_reports = Vec::with_capacity(plan.mask_operations.len());

    for operation in &plan.mask_operations {
        let report =
            apply_mask_operation(operation, &plan.file_dir, max_iter_spc, &mut mask_counts)?;
        mask_reports.push(report);
    }

    if validate_refine_max_iter {
        validate_mask_refine_reaches_max_iter_spc(&mask_counts, max_iter_spc)?;
    }

    Ok(WorkspaceMaskApplyReport {
        workspace,
        mask_reports,
        mask_counts,
    })
}
