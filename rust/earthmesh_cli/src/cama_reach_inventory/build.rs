use std::io;

use crate::cama_binary_io::{
    CamaBinaryGridSpec, CamaBinaryWindow, CamaMetricKind, CamaMetricWindowReport,
    CamaNextxyWindowReport, CamaReachInventoryReport, CamaReachRecord,
};

/// Assemble CaMa river reach records from native Rust metric and topology windows.
///
/// This mirrors Python `build_reach_inventory`: only cells with positive
/// upstream area, river width, and river length become records; downstream 0/0
/// is treated as an estuary flag; upstream area can be scaled into km2.
pub fn build_cama_reach_inventory(
    grid: CamaBinaryGridSpec,
    window: CamaBinaryWindow,
    target_dx_km: f64,
    uparea_to_km2: f64,
    uparea: &CamaMetricWindowReport,
    width: &CamaMetricWindowReport,
    rivlen: &CamaMetricWindowReport,
    nextxy: &CamaNextxyWindowReport,
) -> io::Result<CamaReachInventoryReport> {
    if target_dx_km <= 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "target_dx_km must be positive",
        ));
    }
    validate_cama_reach_inventory_inputs(grid, window, uparea, width, rivlen, nextxy)?;

    let mut records = Vec::new();
    let mut skipped_cells = 0_usize;
    for row_offset in 0..window.height {
        for col_offset in 0..window.width {
            let uparea_value = uparea.values[row_offset][col_offset];
            let width_value = width.values[row_offset][col_offset];
            let length_value = rivlen.values[row_offset][col_offset];
            if !(uparea_value > 0.0 && width_value > 0.0 && length_value > 0.0) {
                skipped_cells += 1;
                continue;
            }

            let x_index = window.x_start + col_offset;
            let y_index = window.y_start + row_offset;
            let downstream_x = nextxy.next_x[row_offset][col_offset];
            let downstream_y = nextxy.next_y[row_offset][col_offset];
            records.push(CamaReachRecord {
                reach_id: format!("cama-{y_index}-{x_index}"),
                x_index,
                y_index,
                lon: grid.lon_center(x_index),
                lat: grid.lat_center(y_index),
                upstream_area_km2: f64::from(uparea_value) * uparea_to_km2,
                width_m: f64::from(width_value),
                floodplain_width_m: 0.0,
                target_dx_km,
                is_estuary: downstream_x == 0 && downstream_y == 0,
                river_length_m: f64::from(length_value),
                downstream_x,
                downstream_y,
            });
        }
    }

    let valid_channel_cells = records.len();
    Ok(CamaReachInventoryReport {
        grid,
        window,
        records,
        valid_channel_cells,
        skipped_cells,
    })
}

fn validate_cama_reach_inventory_inputs(
    grid: CamaBinaryGridSpec,
    window: CamaBinaryWindow,
    uparea: &CamaMetricWindowReport,
    width: &CamaMetricWindowReport,
    rivlen: &CamaMetricWindowReport,
    nextxy: &CamaNextxyWindowReport,
) -> io::Result<()> {
    if uparea.kind != CamaMetricKind::UpstreamArea
        || width.kind != CamaMetricKind::RiverWidth
        || rivlen.kind != CamaMetricKind::RiverLength
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "CaMa reach inventory requires upstream-area, river-width, and river-length metric reports",
        ));
    }
    if uparea.grid != grid
        || width.grid != grid
        || rivlen.grid != grid
        || nextxy.grid != grid
        || uparea.window != window
        || width.window != window
        || rivlen.window != window
        || nextxy.window != window
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "all CaMa reach inventory inputs must share the requested grid and window",
        ));
    }
    if !cama_f32_grid_has_shape(&uparea.values, window.height, window.width)
        || !cama_f32_grid_has_shape(&width.values, window.height, window.width)
        || !cama_f32_grid_has_shape(&rivlen.values, window.height, window.width)
        || !cama_i32_grid_has_shape(&nextxy.next_x, window.height, window.width)
        || !cama_i32_grid_has_shape(&nextxy.next_y, window.height, window.width)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "all CaMa reach inventory arrays must match the requested window shape",
        ));
    }
    Ok(())
}

fn cama_f32_grid_has_shape(values: &[Vec<f32>], height: usize, width: usize) -> bool {
    values.len() == height && values.iter().all(|row| row.len() == width)
}

fn cama_i32_grid_has_shape(values: &[Vec<i32>], height: usize, width: usize) -> bool {
    values.len() == height && values.iter().all(|row| row.len() == width)
}
