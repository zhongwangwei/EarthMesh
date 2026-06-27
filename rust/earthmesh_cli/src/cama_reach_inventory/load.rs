use std::io;
use std::path::{Path, PathBuf};

use crate::cama_binary_io::{CamaLonLatBbox, CamaMetricKind, CamaReachInventoryReport};
use crate::cama_binary_params::read_cama_grid_spec_from_params_file;
use crate::cama_binary_window_readers::{read_cama_float32_metric_window, read_cama_nextxy_window};

use super::build::build_cama_reach_inventory;

/// Load a CaMa map directory into Rust-owned reach inventory source records.
///
/// The loader reads `params.txt`, selects a bbox window, prefers `rivwth.bin`
/// over `width.bin`, and combines `uparea.bin`, river width, `rivlen.bin`, and
/// `nextxy.bin` through the native Rust readers.
pub fn read_cama_reach_inventory_from_map_dir(
    map_dir: impl AsRef<Path>,
    bbox: CamaLonLatBbox,
    target_dx_km: f64,
    uparea_to_km2: f64,
    y_reversed_storage: bool,
) -> io::Result<CamaReachInventoryReport> {
    let root = map_dir.as_ref();
    let mut grid = read_cama_grid_spec_from_params_file(root.join("params.txt"))?;
    grid.y_reversed_storage = y_reversed_storage;
    let window = grid.window_for_bbox(bbox.west, bbox.east, bbox.south, bbox.north)?;
    let uparea = read_cama_float32_metric_window(
        root.join("uparea.bin"),
        grid,
        window,
        CamaMetricKind::UpstreamArea,
    )?;
    let width_path = preferred_cama_width_path(root);
    let width =
        read_cama_float32_metric_window(width_path, grid, window, CamaMetricKind::RiverWidth)?;
    let rivlen = read_cama_float32_metric_window(
        root.join("rivlen.bin"),
        grid,
        window,
        CamaMetricKind::RiverLength,
    )?;
    let nextxy = read_cama_nextxy_window(root.join("nextxy.bin"), grid, window)?;
    build_cama_reach_inventory(
        grid,
        window,
        target_dx_km,
        uparea_to_km2,
        &uparea,
        &width,
        &rivlen,
        &nextxy,
    )
}

fn preferred_cama_width_path(root: &Path) -> PathBuf {
    let rivwth = root.join("rivwth.bin");
    if rivwth.exists() {
        rivwth
    } else {
        root.join("width.bin")
    }
}
