//! Typed configuration, runtime state, namelist syntax, and shared constants for
//! the EarthMesh engine.

mod constants;
pub use constants::{
    deg_to_rad, rad_to_deg, DEFAULT_HARP_DV_CRITERION_MINIMUM_ANGLE_DEG,
    DEFAULT_HARP_DV_MAXIMUM_CELLS, DEFAULT_HARP_DV_MAXIMUM_NEIGHBOR_SCALE_RATIO,
    DEFAULT_HARP_DV_MAXIMUM_PATCH_CELLS, DEFAULT_HARP_DV_MAXIMUM_VERTEX_DEGREE,
    DEFAULT_HARP_DV_MAX_CYCLES, DEFAULT_HARP_DV_MINIMUM_CANDIDATE_SEPARATION_M,
    DEFAULT_HARP_DV_MINIMUM_CELL_WIDTH_M, DEFAULT_HARP_DV_MINIMUM_TRIANGLE_ANGLE_DEG,
    DEFAULT_METHOD_C_LEPP_MAXIMUM_INSERTIONS_PER_CYCLE,
    DEFAULT_METHOD_C_LEPP_MAXIMUM_NEIGHBOR_SIZE_RATIO, DEFAULT_METHOD_C_LEPP_MAXIMUM_PATH_LENGTH,
    DEFAULT_METHOD_C_LEPP_MAXIMUM_VERTICES, DEFAULT_METHOD_C_LEPP_MAX_CYCLES,
    DEFAULT_METHOD_C_LEPP_MINIMUM_TRIANGLE_ANGLE_DEGREES,
    DEFAULT_METHOD_C_LEPP_STOP_AT_SOURCE_RESOLUTION, DEFAULT_METHOD_C_LEPP_TARGET_SIZE_TOLERANCE,
    DEFAULT_MIN_ANGLE_WARN_DEG, EARTH_RADIUS_METERS, JTM_GRID, JTM_INIT, JTM_LBCP, JTM_PROG,
    JTM_VADJ, JTM_WADJ, JTM_WSTN, JTU_GRID, JTU_INIT, JTU_LBCP, JTU_PROG, JTU_WADJ, JTU_WALL,
    JTU_WSTN, JTV_GRID, JTV_INIT, JTV_LBCP, JTV_PROG, JTV_WADJ, JTV_WALL, JTV_WSTN, JTW_GRID,
    JTW_INIT, JTW_LBCP, JTW_PROG, JTW_VADJ, JTW_WADJ, JTW_WSTN, KM_PER_DEGREE_EQUATOR, MAX_REMOTE,
    MLOOPS, NLOOPS_M, NLOOPS_V, NLOOPS_W, PATH_LEN, PI2, PI2_R8, PIO180, PIO180_R8, PIU180,
    PIU180_R8, PI_R8,
};
mod datalayers;
pub use datalayers::{DataLayerConfig, DataLayerRole, DataLayersNamelist, ThresholdVar};
mod datalayer_lowering;
pub use datalayer_lowering::{
    lower_datalayers_namelist, LowerReport, LoweredDatalayers, RefineSwitchArray,
};
mod namelist_syntax;
pub(crate) use namelist_syntax::{
    canonical_quote, parse_canonical_bool, parse_canonical_string, parse_f64, parse_f64_array,
    parse_i32, parse_i32_canonical_1_based_array,
};
pub use namelist_syntax::{
    namelist_assignments, namelist_has_section, rewrite_namelist_group_fields, NamelistAssignment,
};
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

/// Opt-in progress callback used by long engine loops.
pub mod progress;
/// Diagnostic `run_manifest.json` record for one run.
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
