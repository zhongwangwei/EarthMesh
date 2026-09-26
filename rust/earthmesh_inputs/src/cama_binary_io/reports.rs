use std::path::PathBuf;

use super::geometry::{CamaBinaryGridSpec, CamaBinaryWindow};

/// LAND/OCEAN class derived from CaMa `elevtn.bin` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CamaSurfaceClass {
    Land,
    Ocean,
}

/// Lon/lat bounding box used to select a logical CaMa window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CamaLonLatBbox {
    pub west: f64,
    pub east: f64,
    pub south: f64,
    pub north: f64,
}

/// CaMa float32 metric grids used to identify and size river reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CamaMetricKind {
    UpstreamArea,
    RiverWidth,
    RiverLength,
}

/// One Rust-owned CaMa river reach source record assembled from native binary windows.
#[derive(Debug, Clone, PartialEq)]
pub struct CamaReachRecord {
    pub reach_id: String,
    pub x_index: usize,
    pub y_index: usize,
    pub lon: f64,
    pub lat: f64,
    pub upstream_area_km2: f64,
    pub width_m: f64,
    pub floodplain_width_m: f64,
    pub target_dx_km: f64,
    pub is_estuary: bool,
    pub river_length_m: f64,
    pub downstream_x: i32,
    pub downstream_y: i32,
}

/// Conservative default thresholds for promoting CaMa reaches into R0/R1/R2/R3 classes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CamaReachClassificationThresholds {
    pub explicit_2d_width_fraction: f64,
    pub refine_width_fraction: f64,
    pub explicit_2d_upstream_area_km2: f64,
    pub refine_upstream_area_km2: f64,
    pub keep_1d_upstream_area_km2: f64,
}

impl Default for CamaReachClassificationThresholds {
    fn default() -> Self {
        Self {
            explicit_2d_width_fraction: 0.25,
            refine_width_fraction: 0.10,
            explicit_2d_upstream_area_km2: 50_000.0,
            refine_upstream_area_km2: 10_000.0,
            keep_1d_upstream_area_km2: 1_000.0,
        }
    }
}

/// Point-level river classification attached to exported CaMa reach source records.
#[derive(Debug, Clone, PartialEq)]
pub struct CamaReachClassification {
    pub reach_id: String,
    pub river_class: String,
    pub effective_width_m: f64,
    pub reasons: Vec<String>,
}

/// Report from exporting Rust-owned CaMa reach inventory records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CamaReachInventoryJsonlWriteReport {
    pub output: PathBuf,
    pub record_count: usize,
}

/// Report from exporting Rust-owned CaMa reach inventory records to point GeoJSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CamaReachInventoryGeoJsonWriteReport {
    pub output: PathBuf,
    pub feature_count: usize,
}

/// Rust-owned CaMa reach inventory assembled from metric and topology windows.
#[derive(Debug, Clone, PartialEq)]
pub struct CamaReachInventoryReport {
    pub grid: CamaBinaryGridSpec,
    pub window: CamaBinaryWindow,
    pub records: Vec<CamaReachRecord>,
    pub valid_channel_cells: usize,
    pub skipped_cells: usize,
}

/// Native Rust report for one CaMa float32 river metric window.
#[derive(Debug, Clone, PartialEq)]
pub struct CamaMetricWindowReport {
    pub grid: CamaBinaryGridSpec,
    pub window: CamaBinaryWindow,
    pub kind: CamaMetricKind,
    pub values: Vec<Vec<f32>>,
    pub positive_cells: usize,
    pub non_positive_or_invalid_cells: usize,
}

/// Native Rust report for a CaMa `nextxy.bin` downstream-topology window.
#[derive(Debug, Clone, PartialEq)]
pub struct CamaNextxyWindowReport {
    pub grid: CamaBinaryGridSpec,
    pub window: CamaBinaryWindow,
    pub next_x: Vec<Vec<i32>>,
    pub next_y: Vec<Vec<i32>>,
    pub terminal_or_ocean: Vec<Vec<bool>>,
    pub valid_downstream_links: usize,
    pub terminal_or_ocean_links: usize,
}

/// Native Rust report for a CaMa `elevtn.bin` surface-mask window.
#[derive(Debug, Clone, PartialEq)]
pub struct CamaElevtnSurfaceWindowReport {
    pub grid: CamaBinaryGridSpec,
    pub window: CamaBinaryWindow,
    pub elevation: Vec<Vec<f32>>,
    pub surface_mask: Vec<Vec<CamaSurfaceClass>>,
    pub land_cells: usize,
    pub ocean_cells: usize,
}
