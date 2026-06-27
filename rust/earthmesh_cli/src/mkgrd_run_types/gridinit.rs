use earthmesh_core::{EarthmeshConfig, EarthmeshRuntimeState};

use crate::{UnstructuredMeshWriteReport, WorkspaceMaskApplyReport};

/// Report for the migrated initial-grid branch of the `mkgrd.x` driver.
#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdGridinitRunReport {
    pub config: EarthmeshConfig,
    pub runtime_state: Option<EarthmeshRuntimeState>,
    pub workspace_mask: WorkspaceMaskApplyReport,
    pub gridfile: UnstructuredMeshWriteReport,
}
