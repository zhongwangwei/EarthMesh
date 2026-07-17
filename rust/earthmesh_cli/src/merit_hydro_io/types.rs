use std::path::PathBuf;

/// Native Rust MERIT-Hydro tile window read from NetCDF.
#[derive(Debug, Clone, PartialEq)]
pub struct MeritHydroWindowReport {
    pub tile: PathBuf,
    pub tile_name: String,
    pub lon: Vec<f64>,
    pub lat: Vec<f64>,
    pub width: usize,
    pub height: usize,
    pub sampling_stride: usize,
    pub dir: Vec<i32>,
    pub upa_km2: Vec<f64>,
    pub elv_m: Vec<f64>,
    pub width_m: Vec<f64>,
    pub landtype_igbp: Vec<i32>,
}

/// Default MERIT-Hydro mask thresholds used by the Python v3 hydro prototype.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeritMaskThresholds {
    pub r2_width_m: f64,
    pub r3_width_m: f64,
    pub r2_upa_km2: f64,
    pub r3_upa_km2: f64,
    /// Independent Project refinement triggers. Standalone export defaults
    /// them to the corresponding R3 classification thresholds.
    pub river_width_refinement_m: f64,
    pub river_upstream_area_refinement_km2: f64,
}

impl Default for MeritMaskThresholds {
    fn default() -> Self {
        Self {
            r2_width_m: 50.0,
            r3_width_m: 300.0,
            r2_upa_km2: 5_000.0,
            r3_upa_km2: 50_000.0,
            river_width_refinement_m: 300.0,
            river_upstream_area_refinement_km2: 50_000.0,
        }
    }
}

/// Primary classification summary for one native MERIT-Hydro window.
///
/// `classes` contains one precedence-resolved label per native cell (river
/// labels win over surface/coast labels). GeoJSON writers may emit the
/// orthogonal river and surface/coast labels as separate features.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeritHydroMaskClassificationReport {
    pub classes: Vec<String>,
    pub r3_cells: usize,
    pub r2_cells: usize,
    pub coast_land_cells: usize,
    pub coast_ocean_cells: usize,
    pub land_cells: usize,
    pub ocean_cells: usize,
    pub unknown_cells: usize,
}

/// Paths and counts produced by native MERIT-Hydro GeoJSON/layer export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeritHydroGeoJsonLayerWriteReport {
    pub output_dir: PathBuf,
    pub combined_geojson: PathBuf,
    pub river_geojson: PathBuf,
    pub coast_geojson: PathBuf,
    pub surface_geojson: Option<PathBuf>,
    pub summary_json: PathBuf,
    pub window_count: usize,
    pub combined_feature_count: usize,
    pub river_feature_count: usize,
    pub coast_feature_count: usize,
    pub surface_feature_count: usize,
    pub mask_counts: std::collections::BTreeMap<String, usize>,
}
