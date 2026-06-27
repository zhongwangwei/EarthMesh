/// Maximum number of remote send/receive processes in the original Fortran module.
pub const MAX_REMOTE: usize = 30;

/// Maximum path length used by the original Fortran character buffers.
pub const PATH_LEN: usize = 256;

/// Earth radius used by `mkgrd.F90:init_consts`, matching MPAS.
pub const EARTH_RADIUS_METERS: f64 = 6_371_229.0;

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

/// Radians per degree: `atan(1.0_r8) / 45.0_r8` in Fortran.
pub const PIO180: f64 = std::f64::consts::PI / 180.0;
pub const PIO180_R8: f64 = PIO180;

/// Degrees per radian: `45.0_r8 / atan(1.0_r8)` in Fortran.
pub const PIU180: f64 = 180.0 / std::f64::consts::PI;
pub const PIU180_R8: f64 = PIU180;

/// Full turn in radians: `8.0_r8 * atan(1.0_r8)` in Fortran.
pub const PI2: f64 = 2.0 * std::f64::consts::PI;
pub const PI2_R8: f64 = PI2;

/// Pi in double precision: `4.0_r8 * atan(1.0_r8)` in Fortran.
pub const PI_R8: f64 = std::f64::consts::PI;

/// Convert degrees to radians using the migrated Fortran conversion constant.
#[inline]
pub fn deg_to_rad(degrees: f64) -> f64 {
    degrees * PIO180
}

/// Convert radians to degrees using the migrated Fortran conversion constant.
#[inline]
pub fn rad_to_deg(radians: f64) -> f64 {
    radians * PIU180
}
