//! Rust-native core constants and typed configuration migrated from
//! `src/consts_coms.F90`.
//!
//! The goal of this crate is to remove hidden Fortran module-global state from
//! downstream mesh kernels while preserving the exact defaults and formulas that
//! existing EarthMesh workflows rely on.

mod constants;
pub use constants::*;
mod datalayers;
pub use datalayers::*;
mod datalayer_lowering;
pub use datalayer_lowering::*;
mod fortran_namelist;
pub(crate) use fortran_namelist::*;
mod mesh_memory;
pub use mesh_memory::*;
mod mesh_formats;
pub use mesh_formats::*;
mod mkgrd_config;
pub use mkgrd_config::*;
mod mkgrd_workspace;
pub use mkgrd_workspace::*;
mod quality_namelist;
pub use quality_namelist::*;
mod refine_config;
pub use refine_config::*;
mod refine_namelist;
mod refine_validation;
mod runtime_state;
pub use runtime_state::*;

/// Unified path resolution + pre-run input checks shared by CLI and GUI.
pub mod paths;
/// Opt-in progress callback used by long engine loops.
pub mod progress;
/// Reproducible `run_manifest.json` record for one run.
pub mod run_manifest;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_radii_use_mpas_radius() {
        let radii = EarthRadii::default();
        assert_eq!(radii.radius_meters, EARTH_RADIUS_METERS);
    }
}
