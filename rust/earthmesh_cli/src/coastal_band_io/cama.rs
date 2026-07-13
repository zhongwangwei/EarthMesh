use std::io;
use std::path::Path;

use crate::{
    read_cama_elevtn_surface_window, read_cama_grid_spec_from_params_file, CamaSurfaceClass,
};

use super::cells::{coastal_band_cells, coastal_band_cells_periodic_x};
use super::writer::{write_coastal_band_cells_geojson, write_coastal_band_dissolve_geojson};

/// End-to-end port of `coastal_band.py::write_coastal_band_geojson`: read a CaMa map
/// directory (`params.txt` + `elevtn.bin`), derive the land mask over the bbox window,
/// select the coastal band, and write it as GeoJSON (dissolved MultiPolygon or per-cell
/// polygons). `y_reversed` matches Python's default `y_reversed_storage=True`.
#[allow(clippy::too_many_arguments)]
pub fn write_coastal_band_geojson_from_cama(
    map_dir: impl AsRef<Path>,
    output_geojson: impl AsRef<Path>,
    west: f64,
    south: f64,
    east: f64,
    north: f64,
    radius_cells: i64,
    y_reversed: bool,
    dissolve: bool,
    undef: f64,
) -> io::Result<usize> {
    let root = map_dir.as_ref();
    let mut grid = read_cama_grid_spec_from_params_file(root.join("params.txt"))?;
    grid.y_reversed_storage = y_reversed;
    let window = grid.window_for_bbox(west, east, south, north)?;
    let report = read_cama_elevtn_surface_window(root.join("elevtn.bin"), grid, window, undef)?;
    let land_mask: Vec<Vec<bool>> = report
        .surface_mask
        .iter()
        .map(|row| row.iter().map(|&c| c == CamaSurfaceClass::Land).collect())
        .collect();
    let wraps_global_lon =
        window.width == grid.nx && (grid.nx as f64 * grid.grid_size_deg).abs() >= 359.0;
    let band = if wraps_global_lon {
        coastal_band_cells_periodic_x(&land_mask, radius_cells, true, true)?
    } else {
        coastal_band_cells(&land_mask, radius_cells, true, true)?
    };
    if dissolve {
        write_coastal_band_dissolve_geojson(
            &band,
            window.x_start as i64,
            window.y_start as i64,
            grid.west,
            grid.south,
            grid.grid_size_deg,
            output_geojson,
        )
    } else {
        write_coastal_band_cells_geojson(
            &band,
            &land_mask,
            window.x_start as i64,
            window.y_start as i64,
            grid.west,
            grid.south,
            grid.grid_size_deg,
            output_geojson,
        )
    }
}
