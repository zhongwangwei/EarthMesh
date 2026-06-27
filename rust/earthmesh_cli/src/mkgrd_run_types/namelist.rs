use earthmesh_core::EarthmeshRuntimeState;

use crate::{GetContainRuntimeCounts, MkgrdCompactSourceState, MkgrdDataPreprocessSourceState};

use super::{MkgrdGridinitRunReport, MkgrdRefineLoopNamelistRunReport};

/// Evidence from the migrated top-level `mkgrd.x` namelist path.
#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdTopLevelNamelistRunReport {
    pub gridinit: MkgrdGridinitRunReport,
    pub refine: Option<MkgrdRefineLoopNamelistRunReport>,
}

/// Evidence from the direct `--run-refine-landtype-source` migrated namelist
/// path, including the owned data_preprocess source state that replaces
/// Fortran module globals.
#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdRefineLandtypeSourceNamelistRunReport {
    pub source_state: MkgrdDataPreprocessSourceState,
    pub gridinit: MkgrdGridinitRunReport,
    pub refine: Option<MkgrdRefineLoopNamelistRunReport>,
}

/// Evidence from the direct compact source-state migrated namelist path.
#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdRefineCompactSourceStateNamelistRunReport {
    pub source_state: MkgrdCompactSourceState,
    pub gridinit: MkgrdGridinitRunReport,
    pub refine: Option<MkgrdRefineLoopNamelistRunReport>,
}

impl MkgrdTopLevelNamelistRunReport {
    pub fn source_branch_reports(&self) -> &[crate::MkgrdRefineSourceBranchReport] {
        self.refine
            .as_ref()
            .map(MkgrdRefineLoopNamelistRunReport::source_branch_reports)
            .unwrap_or(&[])
    }

    pub fn runtime_state(&self) -> Option<&EarthmeshRuntimeState> {
        self.refine
            .as_ref()
            .map(MkgrdRefineLoopNamelistRunReport::runtime_state)
            .or(self.gridinit.runtime_state.as_ref())
    }

    pub fn final_domain_contain_runtime_counts(&self) -> Option<&GetContainRuntimeCounts> {
        self.refine
            .as_ref()
            .and_then(MkgrdRefineLoopNamelistRunReport::final_domain_contain_runtime_counts)
    }
}

impl MkgrdRefineLandtypeSourceNamelistRunReport {
    pub fn source_branch_reports(&self) -> &[crate::MkgrdRefineSourceBranchReport] {
        self.refine
            .as_ref()
            .map(MkgrdRefineLoopNamelistRunReport::source_branch_reports)
            .unwrap_or(&[])
    }

    pub fn runtime_state(&self) -> Option<&EarthmeshRuntimeState> {
        self.refine
            .as_ref()
            .map(MkgrdRefineLoopNamelistRunReport::runtime_state)
            .or(self.gridinit.runtime_state.as_ref())
    }

    pub fn final_domain_contain_runtime_counts(&self) -> Option<&GetContainRuntimeCounts> {
        self.refine
            .as_ref()
            .and_then(MkgrdRefineLoopNamelistRunReport::final_domain_contain_runtime_counts)
    }
}

impl MkgrdRefineCompactSourceStateNamelistRunReport {
    pub fn source_branch_reports(&self) -> &[crate::MkgrdRefineSourceBranchReport] {
        self.refine
            .as_ref()
            .map(MkgrdRefineLoopNamelistRunReport::source_branch_reports)
            .unwrap_or(&[])
    }

    pub fn runtime_state(&self) -> Option<&EarthmeshRuntimeState> {
        self.refine
            .as_ref()
            .map(MkgrdRefineLoopNamelistRunReport::runtime_state)
            .or(self.gridinit.runtime_state.as_ref())
    }

    pub fn final_domain_contain_runtime_counts(&self) -> Option<&GetContainRuntimeCounts> {
        self.refine
            .as_ref()
            .and_then(MkgrdRefineLoopNamelistRunReport::final_domain_contain_runtime_counts)
    }
}
