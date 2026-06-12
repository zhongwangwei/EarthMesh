//! Rust-native core constants and typed configuration migrated from
//! `src/consts_coms.F90`.
//!
//! The goal of this crate is to remove hidden Fortran module-global state from
//! downstream mesh kernels while preserving the exact defaults and formulas that
//! existing EarthMesh workflows rely on.

/// Maximum number of remote send/receive processes in the original Fortran module.
pub const MAX_REMOTE: usize = 30;

/// Maximum path length used by the original Fortran character buffers.
pub const PATH_LEN: usize = 256;

/// Earth radius used by `mkgrd.F90:init_consts`, matching MPAS.
pub const EARTH_RADIUS_METERS: f64 = 6_371_229.0;

/// Radians per degree: `atan(1.0_r8) / 45.0_r8` in Fortran.
pub const PIO180: f64 = std::f64::consts::PI / 180.0;

/// Degrees per radian: `45.0_r8 / atan(1.0_r8)` in Fortran.
pub const PIU180: f64 = 180.0 / std::f64::consts::PI;

/// Full turn in radians: `8.0_r8 * atan(1.0_r8)` in Fortran.
pub const PI2: f64 = 2.0 * std::f64::consts::PI;

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

/// Derived Earth radius values initialized by `mkgrd.F90:init_consts`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EarthRadii {
    pub radius_meters: f64,
    pub double_radius_meters: f64,
    pub radius_over_sqrt_five_meters: f64,
    pub inverse_radius_meters: f64,
    pub double_radius_squared_meters: f64,
}

impl EarthRadii {
    /// Build the same secondary radius values that Fortran initializes from `erad`.
    pub fn from_radius_meters(radius_meters: f64) -> Self {
        let double_radius_meters = radius_meters * 2.0;
        Self {
            radius_meters,
            double_radius_meters,
            radius_over_sqrt_five_meters: radius_meters / 5.0_f64.sqrt(),
            inverse_radius_meters: 1.0 / radius_meters,
            double_radius_squared_meters: double_radius_meters * double_radius_meters,
        }
    }
}

impl Default for EarthRadii {
    fn default() -> Self {
        Self::from_radius_meters(EARTH_RADIUS_METERS)
    }
}

/// Typed equivalent of `consts_coms:oname_vars` defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct EarthmeshConfig {
    pub experiment_name: String,
    pub nxp: i32,
    pub base_dir: String,
    pub mesh_type: String,
    pub mode_grid: String,
    pub mode_file_description: String,
    pub mode_file: String,
    pub refine: bool,
    pub openmp: i32,
    pub niter: i32,
    pub gridnum_perdegree: i32,
    pub mask_sea_ratio: f64,
    pub beta: f32,
    pub relax: f32,
    pub isolated_ocean: bool,
    pub mask_restart: bool,
    pub mask_domain_type: String,
    pub landtype_file: String,
    pub mask_domain_fprefix: String,
    pub mask_domain_global: bool,
    pub mask_patch_on: bool,
    pub mask_patch_type: String,
    pub mask_patch_fprefix: String,
    pub output_format: String,
}

impl Default for EarthmeshConfig {
    fn default() -> Self {
        Self {
            experiment_name: "/tmp".to_string(),
            nxp: 0,
            base_dir: " /tmp".to_string(),
            mesh_type: "/tmp".to_string(),
            mode_grid: "/tmp".to_string(),
            mode_file_description: "/tmp".to_string(),
            mode_file: " /tmp".to_string(),
            refine: false,
            openmp: 16,
            niter: 5000,
            gridnum_perdegree: 120,
            mask_sea_ratio: 0.5,
            beta: 1.2,
            relax: 0.04,
            isolated_ocean: false,
            mask_restart: false,
            mask_domain_type: "/tmp".to_string(),
            landtype_file: "/tmp".to_string(),
            mask_domain_fprefix: "/tmp".to_string(),
            mask_domain_global: true,
            mask_patch_on: false,
            mask_patch_type: "/tmp".to_string(),
            mask_patch_fprefix: "/tmp".to_string(),
            output_format: "/tmp".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_radii_use_mpas_radius() {
        let radii = EarthRadii::default();
        assert_eq!(radii.radius_meters, EARTH_RADIUS_METERS);
    }
}

/// Typed equivalent of `lonlatmesh_coms:mesh_vars` defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct LonLatMeshConfig {
    pub definition: String,
    pub lon_start: f64,
    pub lon_end: f64,
    pub lon_grid_interval: f64,
    pub lon_points: i32,
    pub lat_start: f64,
    pub lat_end: f64,
    pub lat_grid_interval: f64,
    pub lat_points: i32,
}

impl Default for LonLatMeshConfig {
    fn default() -> Self {
        Self {
            definition: "center".to_string(),
            lon_start: 0.0,
            lon_end: 359.0,
            lon_grid_interval: 0.0625,
            lon_points: 2880,
            lat_start: 0.0,
            lat_end: 0.0,
            lat_grid_interval: 0.0,
            lat_points: 1440,
        }
    }
}

/// Typed equivalent of `fvcommesh_coms:mesh_vars` defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct FvcomMeshConfig {
    pub case_name: String,
    pub dem_file: String,
    pub lon_name: String,
    pub lat_name: String,
    pub depth_name: String,
    pub min_depth: f64,
    pub max_depth: f64,
    pub limit_slope: f64,
}

impl Default for FvcomMeshConfig {
    fn default() -> Self {
        Self {
            case_name: "CASENAME".to_string(),
            dem_file: "/tmp".to_string(),
            lon_name: "/tmp".to_string(),
            lat_name: "/tmp".to_string(),
            depth_name: "/tmp".to_string(),
            min_depth: 1.0,
            max_depth: 300.0,
            limit_slope: 0.02,
        }
    }
}
