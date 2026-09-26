use std::fs;
use std::io;
use std::path::Path;

use crate::cama_binary_io::{
    validate_cama_binary_window, CamaBinaryGridSpec, CamaBinaryWindow, CamaMetricKind,
    CamaMetricWindowReport,
};

use super::raw::read_cama_f32_row_window;

/// Read a CaMa float32 metric window such as `uparea.bin`, `width.bin`, or `rivlen.bin`.
///
/// This intentionally stays at the source-data boundary: it preserves the raw
/// metric values in logical south-to-north order and only reports a simple
/// positive/non-positive split used by later reach-inventory assembly.
pub fn read_cama_float32_metric_window(
    path: impl AsRef<Path>,
    grid: CamaBinaryGridSpec,
    window: CamaBinaryWindow,
    kind: CamaMetricKind,
) -> io::Result<CamaMetricWindowReport> {
    validate_cama_binary_window(grid, window)?;
    let item_size = std::mem::size_of::<f32>();
    let row_stride = grid.nx.checked_mul(item_size).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "CaMa metric row stride overflow",
        )
    })?;
    let window_bytes = window.width.checked_mul(item_size).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "CaMa metric window byte width overflow",
        )
    })?;
    let mut handle = fs::File::open(path)?;
    let mut values = Vec::with_capacity(window.height);
    let mut positive_cells = 0_usize;
    let mut non_positive_or_invalid_cells = 0_usize;

    for logical_y in window.y_start..window.y_start + window.height {
        let storage_y = grid.storage_y_index(logical_y);
        let row_offset = storage_y
            .checked_mul(row_stride)
            .and_then(|offset| offset.checked_add(window.x_start * item_size))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "CaMa metric file offset overflow",
                )
            })?;
        let row =
            read_cama_f32_row_window(&mut handle, row_offset, window_bytes, grid.little_endian)?;
        for value in &row {
            if value.is_finite() && *value > 0.0 {
                positive_cells += 1;
            } else {
                non_positive_or_invalid_cells += 1;
            }
        }
        values.push(row);
    }

    Ok(CamaMetricWindowReport {
        grid,
        window,
        kind,
        values,
        positive_cells,
        non_positive_or_invalid_cells,
    })
}
