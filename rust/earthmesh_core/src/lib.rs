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

impl EarthmeshConfig {
    /// Derive `file_dir = trim(base_dir) // trim(expnme) // '/'` as in
    /// `mkgrd.F90:read_nl`.
    pub fn file_dir(&self) -> String {
        format!(
            "{}{}/",
            self.base_dir.trim_end(),
            self.experiment_name.trim()
        )
    }

    /// Parse the Fortran `/mkgrd/ NL` namelist shape consumed by
    /// `mkgrd.F90:read_nl` into the typed Rust configuration.
    ///
    /// This is intentionally non-destructive: it mirrors assignment parsing and
    /// validation, but does not create/remove the working directories that the
    /// Fortran driver manages after `read_nl`.
    pub fn from_mkgrd_namelist(input: &str) -> Result<Self, String> {
        let mut config = Self::default();
        let mut in_mkgrd = false;

        for raw_line in input.lines() {
            let line = strip_fortran_comment(raw_line).trim().trim_end_matches(',');
            if line.is_empty() {
                continue;
            }
            if line.starts_with('&') {
                in_mkgrd = line.eq_ignore_ascii_case("&mkgrd");
                continue;
            }
            if line == "/" {
                in_mkgrd = false;
                continue;
            }
            if !in_mkgrd {
                continue;
            }

            let Some((left, right)) = line.split_once('=') else {
                continue;
            };
            let Some(field) = left.trim().split_once('%').map(|(_, field)| field.trim()) else {
                continue;
            };
            let value = right.trim().trim_end_matches(',');

            match field.to_ascii_lowercase().as_str() {
                "expnme" => config.experiment_name = parse_fortran_string(value),
                "nxp" => config.nxp = parse_i32(field, value)?,
                "base_dir" => config.base_dir = parse_fortran_string(value),
                "mesh_type" => config.mesh_type = parse_fortran_string(value),
                "mode_grid" => config.mode_grid = parse_fortran_string(value),
                "mode_file_description" => {
                    config.mode_file_description = parse_fortran_string(value)
                }
                "mode_file" => config.mode_file = parse_fortran_string(value),
                "refine" => config.refine = parse_fortran_bool(field, value)?,
                "openmp" => config.openmp = parse_i32(field, value)?,
                "niter" => config.niter = parse_i32(field, value)?,
                "gridnum_perdegree" => config.gridnum_perdegree = parse_i32(field, value)?,
                "mask_sea_ratio" => config.mask_sea_ratio = parse_f64(field, value)?,
                "beta" => config.beta = parse_f32(field, value)?,
                "relax" => config.relax = parse_f32(field, value)?,
                "isolated_ocean" => config.isolated_ocean = parse_fortran_bool(field, value)?,
                "mask_restart" => config.mask_restart = parse_fortran_bool(field, value)?,
                "mask_domain_type" => config.mask_domain_type = parse_fortran_string(value),
                "landtype_file" => config.landtype_file = parse_fortran_string(value),
                "mask_domain_fprefix" => config.mask_domain_fprefix = parse_fortran_string(value),
                "mask_domain_global" => {
                    config.mask_domain_global = parse_fortran_bool(field, value)?
                }
                "mask_patch_on" => config.mask_patch_on = parse_fortran_bool(field, value)?,
                "mask_patch_type" => config.mask_patch_type = parse_fortran_string(value),
                "mask_patch_fprefix" => config.mask_patch_fprefix = parse_fortran_string(value),
                "output_format" => config.output_format = parse_fortran_string(value),
                _ => {}
            }
        }

        config.validate_like_read_nl()?;
        Ok(config)
    }

    fn validate_like_read_nl(&self) -> Result<(), String> {
        match self.gridnum_perdegree {
            120 | 240 => {}
            other => {
                return Err(format!(
                    "gridnum_perdegree must be 120 or 240 like mkgrd.F90:read_nl, got {other}"
                ));
            }
        }

        match (self.mesh_type.as_str(), self.output_format.as_str()) {
            ("landmesh", "CoLM")
            | ("oceanmesh", "FVCOM")
            | ("atmosmesh", "MPAS")
            | ("atmosmesh", "MPAS-Simple")
            | ("LOCmesh", "CoLM") => Ok(()),
            ("landmesh", _) => Err(format!(
                "landmesh output_format must be CoLM like mkgrd.F90:read_nl, got {}",
                self.output_format
            )),
            ("oceanmesh", _) => Err(format!(
                "oceanmesh output_format must be FVCOM like mkgrd.F90:read_nl, got {}",
                self.output_format
            )),
            ("atmosmesh", _) => Err(format!(
                "atmosmesh output_format must be MPAS or MPAS-Simple like mkgrd.F90:read_nl, got {}",
                self.output_format
            )),
            ("LOCmesh", _) => Err(format!(
                "LOCmesh output_format must be CoLM like mkgrd.F90:read_nl, got {}",
                self.output_format
            )),
            (mesh_type, _) => Err(format!(
                "unsupported mesh_type {mesh_type} like mkgrd.F90:read_nl"
            )),
        }
    }
}

fn strip_fortran_comment(line: &str) -> &str {
    let mut quote: Option<char> = None;
    for (index, ch) in line.char_indices() {
        match ch {
            '\'' | '"' if quote == Some(ch) => quote = None,
            '\'' | '"' if quote.is_none() => quote = Some(ch),
            '!' if quote.is_none() => return &line[..index],
            _ => {}
        }
    }
    line
}

fn parse_fortran_string(value: &str) -> String {
    value
        .trim()
        .trim_end_matches(',')
        .trim()
        .trim_matches(|ch| ch == '\'' || ch == '"')
        .to_string()
}

fn parse_i32(field: &str, value: &str) -> Result<i32, String> {
    value
        .trim()
        .trim_end_matches(',')
        .parse()
        .map_err(|err| format!("invalid integer for {field}: {value} ({err})"))
}

fn parse_f32(field: &str, value: &str) -> Result<f32, String> {
    value
        .trim()
        .trim_end_matches(',')
        .parse()
        .map_err(|err| format!("invalid real for {field}: {value} ({err})"))
}

fn parse_f64(field: &str, value: &str) -> Result<f64, String> {
    value
        .trim()
        .trim_end_matches(',')
        .parse()
        .map_err(|err| format!("invalid real for {field}: {value} ({err})"))
}

fn parse_fortran_bool(field: &str, value: &str) -> Result<bool, String> {
    match value
        .trim()
        .trim_end_matches(',')
        .to_ascii_lowercase()
        .as_str()
    {
        ".true." | "true" | "t" => Ok(true),
        ".false." | "false" | "f" => Ok(false),
        other => Err(format!("invalid logical for {field}: {other}")),
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
