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
