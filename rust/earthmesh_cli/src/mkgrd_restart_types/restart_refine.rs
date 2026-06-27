use std::path::{Path, PathBuf};

use earthmesh_core::EarthmeshRuntimeState;

use crate::{
    AreaJudgeRestartGridsRunReport, LandtypeDataPreprocessReport,
    MkgrdCompactRestartRefineSourceState, MkgrdRefineLoopExecutionReport,
    MkgrdRefineLoopPrepareReport, MkgrdRefinePrepareSourceGridOptions,
    MkgrdRefineSourceBranchReport,
};

/// Inputs for entering a migrated refine loop from an already-saved
/// `Area_judge` restart grid instead of a fresh non-restart `Area_judge` run.
#[derive(Debug, Clone, Copy)]
pub struct MkgrdAreaJudgeRestartRefineLoopOptions<'a> {
    pub restart_input: &'a Path,
    pub initial_gridfile: &'a Path,
    pub source_grid: MkgrdRefinePrepareSourceGridOptions<'a>,
    pub landtypes_global: &'a [Vec<i32>],
    pub num_vertex: usize,
    pub maxlc: i32,
}

/// Evidence from a restart-grid handoff into the migrated refine-loop stack.
#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdAreaJudgeRestartRefineLoopRunReport {
    pub prepare: MkgrdRefineLoopPrepareReport,
    pub restart: AreaJudgeRestartGridsRunReport,
    pub execution: MkgrdRefineLoopExecutionReport,
}

/// Evidence from the direct restart-refine compact source-state namelist path.
#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdRestartRefineCompactSourceStateNamelistRunReport {
    pub source_bundle: MkgrdCompactRestartRefineSourceState,
    pub report: MkgrdAreaJudgeRestartRefineLoopRunReport,
}

/// Evidence from the direct restart-refine landtype-source namelist path.
#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdRestartRefineLandtypeSourceNamelistRunReport {
    pub preprocess: LandtypeDataPreprocessReport,
    pub report: MkgrdAreaJudgeRestartRefineLoopRunReport,
}

/// Source branch selected by the default top-level `mkgrd.x` restart-refine
/// handoff classifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MkgrdDefaultRestartRefineSource {
    SourceState,
    LandtypeFile,
}

/// Reusable result for the default top-level restart-refine handoff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MkgrdDefaultRestartRefineHandoff {
    pub source: MkgrdDefaultRestartRefineSource,
    pub initial_gridfile: PathBuf,
}

impl MkgrdAreaJudgeRestartRefineLoopRunReport {
    pub fn source_branch_reports(&self) -> &[MkgrdRefineSourceBranchReport] {
        &self.execution.source_branch_reports
    }

    pub fn runtime_state(&self) -> &EarthmeshRuntimeState {
        self.execution
            .runtime_state
            .as_ref()
            .unwrap_or(&self.prepare.runtime_state)
    }
}

impl MkgrdRestartRefineCompactSourceStateNamelistRunReport {
    pub fn source_branch_reports(&self) -> &[MkgrdRefineSourceBranchReport] {
        self.report.source_branch_reports()
    }

    pub fn runtime_state(&self) -> &EarthmeshRuntimeState {
        self.report.runtime_state()
    }
}

impl MkgrdRestartRefineLandtypeSourceNamelistRunReport {
    pub fn source_branch_reports(&self) -> &[MkgrdRefineSourceBranchReport] {
        self.report.source_branch_reports()
    }

    pub fn runtime_state(&self) -> &EarthmeshRuntimeState {
        self.report.runtime_state()
    }
}
