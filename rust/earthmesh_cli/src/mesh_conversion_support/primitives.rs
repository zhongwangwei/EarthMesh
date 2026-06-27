use std::io;

use earthmesh_mesh::{CartesianPoint, LonLatDegrees};

use crate::{LonLatPoint, MaskPostprocLayout};

pub(crate) fn usize_from_i32_connectivity(value: i32, name: &str) -> io::Result<usize> {
    usize::try_from(value).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{name} contains negative connectivity value {value}"),
        )
    })
}

pub(crate) fn usize_from_i32_nonnegative(value: i32, name: &str) -> io::Result<usize> {
    usize::try_from(value).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} contains negative value {value}"),
        )
    })
}

pub(crate) fn usize_from_i32_positive(value: i32, name: &str) -> io::Result<usize> {
    let converted = usize_from_i32_nonnegative(value, name)?;
    if converted == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} must be positive"),
        ));
    }
    Ok(converted)
}

pub(crate) fn patchtype_indices(
    lon_source: i32,
    lat_source: i32,
    minlon_dm_area: i32,
    maxlat_dm_area: i32,
    nlons_dm_select: usize,
    nlats_dm_select: usize,
) -> io::Result<(usize, usize)> {
    let lon_idx = lon_source - minlon_dm_area;
    let lat_idx = lat_source - maxlat_dm_area;
    if lon_idx < 0
        || lat_idx < 0
        || usize::try_from(lon_idx)
            .ok()
            .is_none_or(|idx| idx >= nlons_dm_select)
        || usize::try_from(lat_idx)
            .ok()
            .is_none_or(|idx| idx >= nlats_dm_select)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "source pixel ({lon_source}, {lat_source}) is outside patchtype grid from minlon {minlon_dm_area}, maxlat {maxlat_dm_area}, shape {nlons_dm_select}x{nlats_dm_select}"
            ),
        ));
    }
    Ok((lon_idx as usize, lat_idx as usize))
}

pub(crate) fn lookup_f64(values: &[f64], index: usize, name: &str) -> io::Result<f64> {
    values.get(index).copied().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{name} length {} does not cover source index {index}",
                values.len()
            ),
        )
    })
}

pub(crate) fn m_to_w_as_usize_rows(rows: &[[i32; 3]]) -> io::Result<Vec<Vec<usize>>> {
    rows.iter()
        .map(|row| {
            row.iter()
                .map(|&value| usize_from_i32_connectivity(value, "itab_m%iw"))
                .collect()
        })
        .collect()
}

pub(crate) fn i32_rows_as_usize(rows: &[Vec<i32>], name: &str) -> io::Result<Vec<Vec<usize>>> {
    rows.iter()
        .map(|row| {
            row.iter()
                .map(|&value| usize_from_i32_connectivity(value, name))
                .collect()
        })
        .collect()
}

pub(crate) fn i32_counts_as_usize(values: &[i32], name: &str) -> io::Result<Vec<usize>> {
    values
        .iter()
        .map(|&value| usize_from_i32_connectivity(value, name))
        .collect()
}

pub(crate) fn validate_mask_postproc_layout(layout: &MaskPostprocLayout) -> io::Result<()> {
    for (name, actual, required) in [
        (
            "center_points",
            layout.center_points.len(),
            layout.ustr_points,
        ),
        (
            "center_neighbors",
            layout.center_neighbors.len(),
            layout.ustr_points,
        ),
        (
            "center_neighbor_counts",
            layout.center_neighbor_counts.len(),
            layout.ustr_points,
        ),
        (
            "vertex_points",
            layout.vertex_points.len(),
            layout.ustr_bounds,
        ),
        (
            "vertex_neighbors",
            layout.vertex_neighbors.len(),
            layout.ustr_bounds,
        ),
        (
            "vertex_neighbor_counts",
            layout.vertex_neighbor_counts.len(),
            layout.ustr_bounds,
        ),
    ] {
        if actual != required {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{name} length {actual} must match required {required}"),
            ));
        }
    }
    Ok(())
}

pub(crate) fn lonlat_pairs_from_points(points: &[LonLatPoint]) -> Vec<[f64; 2]> {
    points.iter().map(|point| [point.lon, point.lat]).collect()
}

pub(crate) fn lonlat_points_from_pairs(
    name: &str,
    values: &[[f64; 2]],
    expected_final_id: usize,
) -> io::Result<Vec<LonLatPoint>> {
    if values.len() <= expected_final_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{name} length {} must include final id {expected_final_id}",
                values.len()
            ),
        ));
    }
    Ok(values
        .iter()
        .map(|point| LonLatPoint {
            lon: point[0],
            lat: point[1],
        })
        .collect())
}

pub(crate) fn rows_to_triangle_connectivity(
    name: &str,
    rows: &[Vec<usize>],
    expected_final_id: usize,
) -> io::Result<Vec<[i32; 3]>> {
    if rows.len() <= expected_final_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{name} length {} must include final id {expected_final_id}",
                rows.len()
            ),
        ));
    }
    rows.iter()
        .enumerate()
        .map(|(row_idx, row)| {
            if row.len() < 3 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("{name} row {row_idx} must contain at least three connectivity slots"),
                ));
            }
            Ok([
                usize_to_i32(name, row[0])?,
                usize_to_i32(name, row[1])?,
                usize_to_i32(name, row[2])?,
            ])
        })
        .collect()
}

pub(crate) fn usize_rows_to_i32(name: &str, rows: &[Vec<usize>]) -> io::Result<Vec<Vec<i32>>> {
    rows.iter()
        .map(|row| usize_values_to_i32(name, row))
        .collect()
}

pub(crate) fn usize_to_i32(name: &str, value: usize) -> io::Result<i32> {
    i32::try_from(value).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} contains value {value} that does not fit NetCDF INT"),
        )
    })
}

pub(crate) fn lonlat_degrees_from_points(points: &[LonLatPoint]) -> Vec<LonLatDegrees> {
    points
        .iter()
        .map(|point| LonLatDegrees {
            lon_degrees: point.lon,
            lat_degrees: point.lat,
        })
        .collect()
}

pub(crate) fn split_cartesian_components(
    points: &[CartesianPoint],
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let mut x = Vec::with_capacity(points.len());
    let mut y = Vec::with_capacity(points.len());
    let mut z = Vec::with_capacity(points.len());
    for point in points {
        x.push(point.x);
        y.push(point.y);
        z.push(point.z);
    }
    (x, y, z)
}

pub(crate) fn scale_cartesian_points_by_earth_radius(points: &mut [CartesianPoint]) {
    for point in points {
        point.x *= earthmesh_core::EARTH_RADIUS_METERS;
        point.y *= earthmesh_core::EARTH_RADIUS_METERS;
        point.z *= earthmesh_core::EARTH_RADIUS_METERS;
    }
}

pub(crate) fn rad_to_deg(radians: f64) -> f64 {
    radians * 180.0 / std::f64::consts::PI
}

pub(crate) fn normalize_degrees(mut degrees: f64) -> f64 {
    if degrees > 180.0 {
        degrees -= 360.0;
    }
    if degrees < -180.0 {
        degrees += 360.0;
    }
    degrees
}

pub(crate) fn require_len(name: &str, actual: usize, required: usize) -> io::Result<()> {
    if actual < required {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} length {actual} is shorter than required {required}"),
        ));
    }
    Ok(())
}

pub(crate) fn lon_values(points: &[LonLatPoint]) -> Vec<f64> {
    points.iter().map(|point| point.lon).collect()
}

pub(crate) fn lat_values(points: &[LonLatPoint]) -> Vec<f64> {
    points.iter().map(|point| point.lat).collect()
}

pub(crate) fn usize_values_to_i32(name: &str, values: &[usize]) -> io::Result<Vec<i32>> {
    values
        .iter()
        .map(|&value| usize_to_i32(name, value))
        .collect()
}
