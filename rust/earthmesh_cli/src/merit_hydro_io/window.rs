use std::io;
use std::path::Path;

use super::types::MeritHydroWindowReport;
use crate::{
    netcdf_to_io_error, required_values_f64_any, required_values_f64_any_matrix,
    required_values_i32_any_matrix, MeritLonLatBbox,
};

/// Read a MERIT-Hydro NetCDF tile into a bbox-selected, lon-major Rust window.
pub fn read_merit_hydro_window(
    tile_path: impl AsRef<Path>,
    bbox: MeritLonLatBbox,
    stride: usize,
) -> io::Result<MeritHydroWindowReport> {
    if stride == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "MERIT-Hydro stride must be positive",
        ));
    }
    let tile = tile_path.as_ref().to_path_buf();
    let file = crate::open_netcdf(&tile).map_err(netcdf_to_io_error)?;
    let lon_all = required_values_f64_any(&file, "longitude")?;
    let lat_all = required_values_f64_any(&file, "latitude")?;
    let lon_indices = indices_between_inclusive(&lon_all, bbox.west, bbox.east, stride);
    let lat_indices = indices_between_inclusive(&lat_all, bbox.south, bbox.north, stride);
    if lon_indices.is_empty() || lat_indices.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("bbox does not overlap MERIT-Hydro tile {}", tile.display()),
        ));
    }
    let lon_len = lon_all.len();
    let lat_len = lat_all.len();
    let dir_all = required_values_i32_any_matrix(&file, "dir", lon_len, lat_len)?;
    let upa_all = required_values_f64_any_matrix(&file, "upa", lon_len, lat_len)?;
    let elv_all = required_values_f64_any_matrix(&file, "elv", lon_len, lat_len)?;
    let wth_all = required_values_f64_any_matrix(&file, "wth", lon_len, lat_len)?;
    let landtype_all = required_values_i32_any_matrix(&file, "landtype_igbp", lon_len, lat_len)?;

    let width = lon_indices.len();
    let height = lat_indices.len();
    let mut dir = Vec::with_capacity(width * height);
    let mut upa_km2 = Vec::with_capacity(width * height);
    let mut elv_m = Vec::with_capacity(width * height);
    let mut width_m = Vec::with_capacity(width * height);
    let mut landtype_igbp = Vec::with_capacity(width * height);
    for &lon_index in &lon_indices {
        for &lat_index in &lat_indices {
            let offset = lon_index
                .checked_mul(lat_len)
                .and_then(|base| base.checked_add(lat_index))
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "MERIT-Hydro index overflow")
                })?;
            dir.push(dir_all[offset]);
            upa_km2.push(clean_merit_fill(upa_all[offset]));
            elv_m.push(clean_merit_fill(elv_all[offset]));
            width_m.push(clean_merit_fill(wth_all[offset]));
            landtype_igbp.push(landtype_all[offset]);
        }
    }
    let tile_name = tile
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| tile.display().to_string());
    Ok(MeritHydroWindowReport {
        tile,
        tile_name,
        lon: lon_indices.iter().map(|&index| lon_all[index]).collect(),
        lat: lat_indices.iter().map(|&index| lat_all[index]).collect(),
        width,
        height,
        dir,
        upa_km2,
        elv_m,
        width_m,
        landtype_igbp,
    })
}

fn indices_between_inclusive(values: &[f64], low: f64, high: f64, stride: usize) -> Vec<usize> {
    values
        .iter()
        .enumerate()
        .filter_map(|(index, value)| {
            if value.is_finite() && *value >= low && *value <= high {
                Some(index)
            } else {
                None
            }
        })
        .step_by(stride)
        .collect()
}

fn clean_merit_fill(value: f64) -> f64 {
    if value <= -9990.0 {
        f64::NAN
    } else {
        value
    }
}
