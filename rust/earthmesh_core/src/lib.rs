//! Typed configuration, runtime state, namelist syntax, and shared constants for
//! the EarthMesh engine.

mod constants;
pub use constants::{
    deg_to_rad, rad_to_deg, EARTH_RADIUS_METERS, JTM_GRID, JTM_INIT, JTM_LBCP, JTM_PROG, JTM_VADJ,
    JTM_WADJ, JTM_WSTN, JTU_GRID, JTU_INIT, JTU_LBCP, JTU_PROG, JTU_WADJ, JTU_WALL, JTU_WSTN,
    JTV_GRID, JTV_INIT, JTV_LBCP, JTV_PROG, JTV_WADJ, JTV_WALL, JTV_WSTN, JTW_GRID, JTW_INIT,
    JTW_LBCP, JTW_PROG, JTW_VADJ, JTW_WADJ, JTW_WSTN, MAX_REMOTE, MLOOPS, NLOOPS_M, NLOOPS_V,
    NLOOPS_W, PATH_LEN, PI2, PI2_R8, PIO180, PIO180_R8, PIU180, PIU180_R8, PI_R8,
};
mod datalayers;
pub use datalayers::{DataLayerConfig, DataLayerRole, DataLayersNamelist, ThresholdVar};
mod datalayer_lowering;
pub use datalayer_lowering::{
    lower_datalayers_namelist, LowerReport, LoweredDatalayers, RefineSwitchArray,
};
mod namelist_syntax;
pub(crate) use namelist_syntax::*;
mod mesh_memory;
pub use mesh_memory::{
    DelaunayMemory, GridMemory, IjTabs, ItabM, ItabMd, ItabUd, ItabV, ItabW, ItabWd,
    MeshMemoryShape, NestUd, NestWd,
};
mod mesh_formats;
pub use mesh_formats::{FvcomMeshConfig, LonLatMeshConfig};
mod markers;
pub use markers::DomainMarker;
mod mkgrd_config;
pub use mkgrd_config::EarthmeshConfig;
mod mkgrd_workspace;
pub use mkgrd_workspace::{MaskOperation, MkgrdWorkspacePlan};
mod quality_namelist;
pub use quality_namelist::QualityNamelist;
mod refine_config;
pub use refine_config::RefineConfig;
mod refine_namelist;
mod refine_validation;
mod runtime_state;
pub use runtime_state::{
    EarthRadii, EarthmeshRuntimeState, MaskCounterState, RuntimeScalarState, SourceGridState,
};

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
