use std::io;
use std::path::Path;

use super::types::MeritHydroWindowReport;
use crate::area_judge_threshold_inputs::numeric_missing_values;
use crate::{required_values_f64_any, MeritLonLatBbox};

#[derive(Clone, Copy)]
struct AxisWindow {
    start: usize,
    count: usize,
    stride: isize,
}

#[derive(Clone, Copy)]
enum MatrixOrder {
    LonLat,
    LatLon,
}

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
    let sampling_stride = stride;
    let stride = isize::try_from(stride).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "MERIT-Hydro stride exceeds the NetCDF index range",
        )
    })?;
    let tile = tile_path.as_ref().to_path_buf();
    let file = crate::open_netcdf(&tile).map_err(crate::netcdf_to_io_error)?;
    let lon_all = required_values_f64_any(&file, "longitude")?;
    let lat_all = required_values_f64_any(&file, "latitude")?;
    let lon_window = axis_window(&lon_all, bbox.west, bbox.east, stride)?;
    let lat_window = axis_window(&lat_all, bbox.south, bbox.north, stride)?;
    let (lon_window, lat_window) = match (lon_window, lat_window) {
        (Some(lon), Some(lat)) => (lon, lat),
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("bbox does not overlap MERIT-Hydro tile {}", tile.display()),
            ));
        }
    };

    // Read only the selected hyperslab. A production MERIT tile is large enough that
    // materialising all five full-tile matrices here can exhaust process memory.
    let dir = read_i32_window(
        &file,
        "dir",
        lon_all.len(),
        lat_all.len(),
        lon_window,
        lat_window,
    )?;
    let upa_km2 = read_f64_window(
        &file,
        "upa",
        lon_all.len(),
        lat_all.len(),
        lon_window,
        lat_window,
    )?;
    let elv_m = read_f64_window(
        &file,
        "elv",
        lon_all.len(),
        lat_all.len(),
        lon_window,
        lat_window,
    )?;
    let width_m = read_f64_window(
        &file,
        "wth",
        lon_all.len(),
        lat_all.len(),
        lon_window,
        lat_window,
    )?;
    let landtype_igbp = read_i32_window(
        &file,
        "landtype_igbp",
        lon_all.len(),
        lat_all.len(),
        lon_window,
        lat_window,
    )?;

    let tile_name = tile
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| tile.display().to_string());
    Ok(MeritHydroWindowReport {
        tile,
        tile_name,
        lon: selected_axis_values(&lon_all, lon_window),
        lat: selected_axis_values(&lat_all, lat_window),
        width: lon_window.count,
        height: lat_window.count,
        sampling_stride,
        dir,
        upa_km2,
        elv_m,
        width_m,
        landtype_igbp,
    })
}

fn axis_window(
    values: &[f64],
    low: f64,
    high: f64,
    stride: isize,
) -> io::Result<Option<AxisWindow>> {
    let matching = values
        .iter()
        .enumerate()
        .filter_map(|(index, value)| {
            (value.is_finite() && *value >= low && *value <= high).then_some(index)
        })
        .collect::<Vec<_>>();
    if matching.is_empty() {
        return Ok(None);
    }
    if matching.windows(2).any(|pair| pair[1] != pair[0] + 1) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "MERIT-Hydro coordinate selection is not a contiguous NetCDF window",
        ));
    }
    let count = matching.len().div_ceil(stride as usize);
    Ok(Some(AxisWindow {
        start: matching[0],
        count,
        stride,
    }))
}

fn selected_axis_values(values: &[f64], window: AxisWindow) -> Vec<f64> {
    (0..window.count)
        .map(|offset| values[window.start + offset * window.stride as usize])
        .collect()
}

fn read_f64_window(
    file: &netcdf::File,
    name: &str,
    lon_len: usize,
    lat_len: usize,
    lon: AxisWindow,
    lat: AxisWindow,
) -> io::Result<Vec<f64>> {
    let variable = required_variable(file, name)?;
    let order = matrix_order(name, variable.dimensions(), lon_len, lat_len)?;
    let (start, count, stride) = hyperslab(order, lon, lat);
    let values = if let Ok(values) = variable.get_values::<f64, _>((start, count, stride)) {
        values
    } else if let Ok(values) = variable.get_values::<f32, _>((start, count, stride)) {
        values.into_iter().map(f64::from).collect()
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{name} variable must be readable as f64 or f32"),
        ));
    };
    let missing = numeric_missing_values(&variable)?;
    normalize_window(
        values
            .into_iter()
            .map(|value| clean_merit_fill(value, &missing))
            .collect(),
        order,
        lon.count,
        lat.count,
        name,
    )
}

fn read_i32_window(
    file: &netcdf::File,
    name: &str,
    lon_len: usize,
    lat_len: usize,
    lon: AxisWindow,
    lat: AxisWindow,
) -> io::Result<Vec<i32>> {
    let variable = required_variable(file, name)?;
    let order = matrix_order(name, variable.dimensions(), lon_len, lat_len)?;
    let (start, count, stride) = hyperslab(order, lon, lat);
    let values = if let Ok(values) = variable.get_values::<i32, _>((start, count, stride)) {
        values
    } else if let Ok(values) = variable.get_values::<i16, _>((start, count, stride)) {
        values.into_iter().map(i32::from).collect()
    } else if let Ok(values) = variable.get_values::<i8, _>((start, count, stride)) {
        values.into_iter().map(i32::from).collect()
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{name} variable must be readable as i32, i16, or i8"),
        ));
    };
    let missing = numeric_missing_values(&variable)?;
    normalize_window(
        values
            .into_iter()
            .map(|value| {
                if missing.contains(&f64::from(value)) {
                    i32::MIN
                } else {
                    value
                }
            })
            .collect(),
        order,
        lon.count,
        lat.count,
        name,
    )
}

fn required_variable<'a>(file: &'a netcdf::File, name: &str) -> io::Result<netcdf::Variable<'a>> {
    file.variable(name).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("missing {name} variable"),
        )
    })
}

fn matrix_order(
    name: &str,
    dimensions: &[netcdf::Dimension<'_>],
    lon_len: usize,
    lat_len: usize,
) -> io::Result<MatrixOrder> {
    if dimensions.len() != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{name} must be 2-D"),
        ));
    }
    let names = dimensions
        .iter()
        .map(|dimension| dimension.name())
        .collect::<Vec<_>>();
    let lengths = dimensions
        .iter()
        .map(|dimension| dimension.len())
        .collect::<Vec<_>>();
    let order = if is_lon_dim(&names[0]) && is_lat_dim(&names[1]) {
        (lengths == [lon_len, lat_len]).then_some(MatrixOrder::LonLat)
    } else if is_lat_dim(&names[0]) && is_lon_dim(&names[1]) {
        (lengths == [lat_len, lon_len]).then_some(MatrixOrder::LatLon)
    } else if lon_len != lat_len && lengths == [lon_len, lat_len] {
        Some(MatrixOrder::LonLat)
    } else if lon_len != lat_len && lengths == [lat_len, lon_len] {
        Some(MatrixOrder::LatLon)
    } else {
        None
    };
    order.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{name} dimensions {:?} with lengths {:?} do not match expected longitude x latitude",
                names, lengths
            ),
        )
    })
}

fn is_lon_dim(name: &str) -> bool {
    is_axis_dim(name, &["lon", "longitude"], "x")
}

fn is_lat_dim(name: &str) -> bool {
    is_axis_dim(name, &["lat", "latitude"], "y")
}

fn is_axis_dim(name: &str, aliases: &[&str], short_axis: &str) -> bool {
    let normalized = name.to_ascii_lowercase();
    normalized == short_axis
        || aliases.contains(&normalized.as_str())
        || normalized
            .split(|ch: char| !ch.is_ascii_alphanumeric())
            .any(|token| aliases.contains(&token))
}

fn hyperslab(
    order: MatrixOrder,
    lon: AxisWindow,
    lat: AxisWindow,
) -> ([usize; 2], [usize; 2], [isize; 2]) {
    match order {
        MatrixOrder::LonLat => (
            [lon.start, lat.start],
            [lon.count, lat.count],
            [lon.stride, lat.stride],
        ),
        MatrixOrder::LatLon => (
            [lat.start, lon.start],
            [lat.count, lon.count],
            [lat.stride, lon.stride],
        ),
    }
}

fn normalize_window<T: Copy + Default>(
    values: Vec<T>,
    order: MatrixOrder,
    width: usize,
    height: usize,
    name: &str,
) -> io::Result<Vec<T>> {
    let expected = width.checked_mul(height).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "MERIT-Hydro window size overflow",
        )
    })?;
    if values.len() != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{name} hyperslab returned {} values; expected {expected}",
                values.len()
            ),
        ));
    }
    if matches!(order, MatrixOrder::LonLat) {
        return Ok(values);
    }
    let mut transposed = vec![T::default(); expected];
    for lat in 0..height {
        for lon in 0..width {
            transposed[lon * height + lat] = values[lat * width + lon];
        }
    }
    Ok(transposed)
}

fn clean_merit_fill(value: f64, missing: &[f64]) -> f64 {
    if !value.is_finite() || value <= -9990.0 || missing.contains(&value) {
        f64::NAN
    } else {
        value
    }
}
