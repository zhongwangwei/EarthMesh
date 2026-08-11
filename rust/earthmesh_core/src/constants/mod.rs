/// Maximum number of remote send/receive processes in the original Canonical module.
pub const MAX_REMOTE: usize = 30;

/// Maximum path length used by the original Canonical character buffers.
pub const PATH_LEN: usize = 256;

/// Earth radius used by `mkgrd.F90:init_consts`, matching MPAS.
pub const EARTH_RADIUS_METERS: f64 = 6_371_229.0;

/// Shared default warning threshold for minimum mesh angles.
pub const DEFAULT_MIN_ANGLE_WARN_DEG: f64 = 25.0;

/// Canonical refinement-spring budgets when `RL%niter_refine` is omitted.
pub const DEFAULT_SURFACE_REFINE_SPRING_ITERATIONS: usize = 2_000;
pub const DEFAULT_ATMOSPHERE_REFINE_SPRING_ITERATIONS: usize = 5_000;

/// One source of truth for the bounded Method-C LEPP AdaptiveHybrid defaults.
pub const DEFAULT_METHOD_C_LEPP_MAX_CYCLES: usize = 8;
pub const DEFAULT_METHOD_C_LEPP_TARGET_SIZE_TOLERANCE: f64 = 1.20;
pub const DEFAULT_METHOD_C_LEPP_MAXIMUM_NEIGHBOR_SIZE_RATIO: f64 = 1.75;
pub const DEFAULT_METHOD_C_LEPP_MAXIMUM_VERTICES: usize = 5_000_000;
pub const DEFAULT_METHOD_C_LEPP_MAXIMUM_INSERTIONS_PER_CYCLE: usize = 500_000;
pub const DEFAULT_METHOD_C_LEPP_MAXIMUM_PATH_LENGTH: usize = 100_000;
pub const DEFAULT_METHOD_C_LEPP_MINIMUM_TRIANGLE_ANGLE_DEGREES: f64 = 0.0;
pub const DEFAULT_METHOD_C_LEPP_STOP_AT_SOURCE_RESOLUTION: bool = true;

/// One source of truth for the production HARP-DV controls exposed through
/// project files, namelists, and the GUI.
pub const DEFAULT_HARP_DV_MAX_CYCLES: u32 = 20;
pub const DEFAULT_HARP_DV_MINIMUM_CELL_WIDTH_M: f64 = 1_000.0;
pub const DEFAULT_HARP_DV_MAXIMUM_CELLS: usize = 5_000_000;
pub const DEFAULT_HARP_DV_MAXIMUM_PATCH_CELLS: usize = 10_000;
pub const DEFAULT_HARP_DV_MAXIMUM_NEIGHBOR_SCALE_RATIO: f64 = 1.75;
pub const DEFAULT_HARP_DV_MINIMUM_CANDIDATE_SEPARATION_M: f64 = 1.0;
pub const DEFAULT_HARP_DV_MAXIMUM_VERTEX_DEGREE: usize = 7;
pub const DEFAULT_HARP_DV_MINIMUM_TRIANGLE_ANGLE_DEG: f64 = DEFAULT_MIN_ANGLE_WARN_DEG;
pub const DEFAULT_HARP_DV_CRITERION_MINIMUM_ANGLE_DEG: f64 = 0.0;

/// Equatorial kilometers per degree on EarthMesh's configured sphere.
pub const KM_PER_DEGREE_EQUATOR: f64 =
    2.0 * std::f64::consts::PI * (EARTH_RADIUS_METERS / 1000.0) / 360.0;

/// Maximum number of non-parallel M/V/W loops in `mem_ijtabs`.
pub const MLOOPS: usize = 7;
pub const NLOOPS_M: usize = MLOOPS + MAX_REMOTE;
pub const NLOOPS_V: usize = MLOOPS + MAX_REMOTE;
pub const NLOOPS_W: usize = MLOOPS + MAX_REMOTE;

pub const JTM_GRID: usize = 1;
pub const JTU_GRID: usize = 1;
pub const JTV_GRID: usize = 1;
pub const JTW_GRID: usize = 1;
pub const JTM_INIT: usize = 2;
pub const JTU_INIT: usize = 2;
pub const JTV_INIT: usize = 2;
pub const JTW_INIT: usize = 2;
pub const JTM_PROG: usize = 3;
pub const JTU_PROG: usize = 3;
pub const JTV_PROG: usize = 3;
pub const JTW_PROG: usize = 3;
pub const JTM_WADJ: usize = 4;
pub const JTU_WADJ: usize = 4;
pub const JTV_WADJ: usize = 4;
pub const JTW_WADJ: usize = 4;
pub const JTM_WSTN: usize = 5;
pub const JTU_WSTN: usize = 5;
pub const JTV_WSTN: usize = 5;
pub const JTW_WSTN: usize = 5;
pub const JTM_LBCP: usize = 6;
pub const JTU_LBCP: usize = 6;
pub const JTV_LBCP: usize = 6;
pub const JTW_LBCP: usize = 6;
pub const JTM_VADJ: usize = 7;
pub const JTU_WALL: usize = 7;
pub const JTV_WALL: usize = 7;
pub const JTW_VADJ: usize = 7;

/// Radians per degree: `atan(1.0_r8) / 45.0_r8` in Canonical.
pub const PIO180: f64 = std::f64::consts::PI / 180.0;
pub const PIO180_R8: f64 = PIO180;

/// Degrees per radian: `45.0_r8 / atan(1.0_r8)` in Canonical.
pub const PIU180: f64 = 180.0 / std::f64::consts::PI;
pub const PIU180_R8: f64 = PIU180;

/// Full turn in radians: `8.0_r8 * atan(1.0_r8)` in Canonical.
pub const PI2: f64 = 2.0 * std::f64::consts::PI;
pub const PI2_R8: f64 = PI2;

/// Pi in double precision: `4.0_r8 * atan(1.0_r8)` in Canonical.
pub const PI_R8: f64 = std::f64::consts::PI;

/// Convert degrees to radians using the current Canonical conversion constant.
#[inline]
pub fn deg_to_rad(degrees: f64) -> f64 {
    degrees * PIO180
}

/// Convert radians to degrees using the current Canonical conversion constant.
#[inline]
pub fn rad_to_deg(radians: f64) -> f64 {
    radians * PIU180
}
