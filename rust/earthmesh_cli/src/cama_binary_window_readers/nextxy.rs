use std::fs;
use std::io;
use std::path::Path;

use crate::cama_binary_io::{
    validate_cama_binary_window, CamaBinaryGridSpec, CamaBinaryWindow, CamaNextxyWindowReport,
};

use super::raw::read_cama_i32_row_window;

/// Read CaMa `nextxy.bin` planar int32 downstream topology into logical indices.
///
/// The file is two full-grid planes: raw one-based downstream x, followed by
/// raw one-based downstream y. Positive values are converted to zero-based
/// logical indices. Non-positive values are preserved as terminal/ocean links.
pub fn read_cama_nextxy_window(
    path: impl AsRef<Path>,
    grid: CamaBinaryGridSpec,
    window: CamaBinaryWindow,
) -> io::Result<CamaNextxyWindowReport> {
    validate_cama_binary_window(grid, window)?;
    let item_size = std::mem::size_of::<i32>();
    let row_stride = grid.nx.checked_mul(item_size).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "CaMa nextxy row stride overflow",
        )
    })?;
    let plane_stride = row_stride.checked_mul(grid.ny).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "CaMa nextxy plane stride overflow",
        )
    })?;
    let window_bytes = window.width.checked_mul(item_size).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "CaMa nextxy window byte width overflow",
        )
    })?;
    let mut handle = fs::File::open(path)?;
    let mut next_x = Vec::with_capacity(window.height);
    let mut next_y = Vec::with_capacity(window.height);
    let mut valid_downstream_links = 0_usize;
    let mut terminal_or_ocean_links = 0_usize;

    for logical_y in window.y_start..window.y_start + window.height {
        let storage_y = grid.storage_y_index(logical_y);
        let row_base = storage_y
            .checked_mul(row_stride)
            .and_then(|offset| offset.checked_add(window.x_start * item_size))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "CaMa nextxy file offset overflow",
                )
            })?;
        let raw_x_row =
            read_cama_i32_row_window(&mut handle, row_base, window_bytes, grid.little_endian)?;
        let y_row_base = plane_stride.checked_add(row_base).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "CaMa nextxy y-plane file offset overflow",
            )
        })?;
        let raw_y_row =
            read_cama_i32_row_window(&mut handle, y_row_base, window_bytes, grid.little_endian)?;
        let mut converted_x = Vec::with_capacity(window.width);
        let mut converted_y = Vec::with_capacity(window.width);
        for (raw_x, raw_y) in raw_x_row.into_iter().zip(raw_y_row) {
            if raw_x > 0 && raw_y > 0 {
                valid_downstream_links += 1;
            } else {
                terminal_or_ocean_links += 1;
            }
            converted_x.push(convert_cama_nextxy_x(raw_x));
            converted_y.push(convert_cama_nextxy_y(raw_y, grid));
        }
        next_x.push(converted_x);
        next_y.push(converted_y);
    }

    Ok(CamaNextxyWindowReport {
        grid,
        window,
        next_x,
        next_y,
        valid_downstream_links,
        terminal_or_ocean_links,
    })
}

fn convert_cama_nextxy_x(raw_x: i32) -> i32 {
    if raw_x <= 0 {
        raw_x
    } else {
        raw_x - 1
    }
}

fn convert_cama_nextxy_y(raw_y: i32, grid: CamaBinaryGridSpec) -> i32 {
    if raw_y <= 0 {
        raw_y
    } else if grid.y_reversed_storage {
        grid.ny as i32 - raw_y
    } else {
        raw_y - 1
    }
}
