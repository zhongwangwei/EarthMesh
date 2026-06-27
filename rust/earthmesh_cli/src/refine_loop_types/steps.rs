use std::path::PathBuf;

use crate::{RefineArrayLengthCalculationRunReport, RefineLoopWorkingState};

use super::operations::{
    NgrRenewReport, OnedivideFourConnectionReport, OnedivideFourRenewReport, OnedivideTwoReport,
};

/// Evidence from the file-backed `MOD_refine.F90:refine_loop` prologue:
/// read the current unstructured grid and save the `_ori` snapshot before any
/// geometry refinement mutates the mesh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MkgrdRefineLoopPrologueSnapshotReport {
    pub input_gridfile: PathBuf,
    pub original_tmpfile: PathBuf,
    pub copied_bytes: u64,
    pub sjx_points: usize,
    pub lbx_points: usize,
}

/// Evidence from the same prologue once the gridfile has also been converted
/// into the Rust `refine_loop` working arrays.
#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdRefineLoopWorkingStatePrologueReport {
    pub snapshot: MkgrdRefineLoopPrologueSnapshotReport,
    pub state: RefineLoopWorkingState,
}

/// Evidence from the conservative Rust working-state executor for one
/// file-backed `MOD_refine.F90:refine_loop` step.
#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdRefineLoopWorkingStateStepReport {
    pub prologue: MkgrdRefineLoopWorkingStatePrologueReport,
    pub state: RefineLoopWorkingState,
    pub output_gridfile: PathBuf,
    pub loaded_ref_sjx: Option<Vec<i32>>,
    pub onedivide_four_connection: Option<OnedivideFourConnectionReport>,
    pub array_length: Option<RefineArrayLengthCalculationRunReport>,
    pub onedivide_four_renew: Option<OnedivideFourRenewReport>,
    pub onedivide_two: Option<OnedivideTwoReport>,
    pub ngr_renew: Option<NgrRenewReport>,
    pub post_refine_counts: Option<(usize, usize)>,
}
