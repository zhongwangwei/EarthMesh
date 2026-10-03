//! Red-green above the kernels: what one level marks and runs with, and the
//! passes that finish the triangulation once the levels are done.
//!
//! The kernels below refine one marked round. This module runs the levels
//! (`refine_levels`) and finishes them (`finish_levels`). It decides the marking
//! (from any [`earthmesh_refine::TargetLevelField`], so named regions, criteria
//! circles and the h-field mark the same way), the settings each level reads
//! from the engine's per-level arrays, and the finishing passes: an angle-safe
//! Lawson polish and the repair into the published angle window. Everything
//! here reads and writes a [`RedGreenMesh`](crate::RedGreenMesh); turning one
//! into the gridfile's tables is the caller's.

mod finish;
mod level;
mod run;

pub use finish::{
    finalize_redgreen_mesh, legalize_redgreen_mesh, polish_redgreen_mesh,
    repair_redgreen_angle_window, RedGreenPolishReport,
};
pub use level::{
    redgreen_marking, redgreen_marking_from_regions, redgreen_settings_for_level,
    refine_redgreen_level,
};
pub use run::{
    finish_levels, refine_levels, RedGreenCriteria, RedGreenFinish, RedGreenLevels,
    RedGreenRequest, REDGREEN_MAX_CELL_DEGREE,
};
