use std::fs;
use std::io;
use std::path::Path;

use crate::cama_binary_io::{
    validate_cama_binary_window, CamaBinaryGridSpec, CamaBinaryWindow,
    CamaElevtnSurfaceWindowReport, CamaSurfaceClass,
};

use super::raw::read_cama_f32_row_window;

/// Read a CaMa `elevtn.bin` float32 window and classify valid cells as LAND.
///
/// This mirrors the Python `land_mask_from_elevation` rule: a cell is land when
/// the elevation value is finite and not equal to the supplied CaMa undef value.
pub fn read_cama_elevtn_surface_window(
    path: impl AsRef<Path>,
    grid: CamaBinaryGridSpec,
    window: CamaBinaryWindow,
    undef: f64,
) -> io::Result<CamaElevtnSurfaceWindowReport> {
    validate_cama_binary_window(grid, window)?;
    let item_size = std::mem::size_of::<f32>();
    let row_stride = grid
        .nx
        .checked_mul(item_size)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "CaMa row stride overflow"))?;
    let window_bytes = window.width.checked_mul(item_size).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "CaMa window byte width overflow",
        )
    })?;
    let mut handle = fs::File::open(path)?;
    let mut elevation = Vec::with_capacity(window.height);
    let mut surface_mask = Vec::with_capacity(window.height);
    let mut land_cells = 0_usize;
    let mut ocean_cells = 0_usize;

    for logical_y in window.y_start..window.y_start + window.height {
        let storage_y = grid.storage_y_index(logical_y);
        let row_offset = storage_y
            .checked_mul(row_stride)
            .and_then(|offset| offset.checked_add(window.x_start * item_size))
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "CaMa file offset overflow")
            })?;
        let row =
            read_cama_f32_row_window(&mut handle, row_offset, window_bytes, grid.little_endian)?;
        let mut mask_row = Vec::with_capacity(window.width);
        for value in &row {
            let class = if value.is_finite() && f64::from(*value) != undef {
                land_cells += 1;
                CamaSurfaceClass::Land
            } else {
                ocean_cells += 1;
                CamaSurfaceClass::Ocean
            };
            mask_row.push(class);
        }
        elevation.push(row);
        surface_mask.push(mask_row);
    }

    Ok(CamaElevtnSurfaceWindowReport {
        grid,
        window,
        elevation,
        surface_mask,
        land_cells,
        ocean_cells,
    })
}
