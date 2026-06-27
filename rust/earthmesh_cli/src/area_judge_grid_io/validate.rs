use std::io;

use earthmesh_mesh::AreaJudgeSourceBounds;

use crate::{matrix_width, require_len};

use super::types::AreaJudgeGridPayload;

pub(crate) fn validate_area_judge_grid_payload(payload: &AreaJudgeGridPayload) -> io::Result<()> {
    let expected_lon = payload
        .bounds
        .maxlon_source
        .checked_sub(payload.bounds.minlon_source)
        .and_then(|span| span.checked_add(1))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "area longitude bounds {}..{} are invalid",
                    payload.bounds.minlon_source, payload.bounds.maxlon_source
                ),
            )
        })?;
    let expected_lat = payload
        .bounds
        .minlat_source
        .checked_sub(payload.bounds.maxlat_source)
        .and_then(|span| span.checked_add(1))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "area latitude bounds {}..{} are invalid",
                    payload.bounds.maxlat_source, payload.bounds.minlat_source
                ),
            )
        })?;
    if payload.longitude.len() != expected_lon {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "longitude length {} must match selected nlons {expected_lon}",
                payload.longitude.len()
            ),
        ));
    }
    if payload.latitude.len() != expected_lat {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "latitude length {} must match selected nlats {expected_lat}",
                payload.latitude.len()
            ),
        ));
    }
    validate_i32_matrix_shape(
        "IsInArea_select",
        &payload.is_in_area_select,
        expected_lon,
        expected_lat,
    )?;
    if let Some(seaorland) = payload.seaorland_select.as_ref() {
        validate_i32_matrix_shape("seaorland_select", seaorland, expected_lon, expected_lat)?;
    }
    Ok(())
}

pub(crate) fn validate_i32_matrix_shape(
    name: &str,
    rows: &[Vec<i32>],
    expected_rows: usize,
    expected_width: usize,
) -> io::Result<()> {
    if rows.len() != expected_rows {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{name} row count {} must match selected nlons {expected_rows}",
                rows.len()
            ),
        ));
    }
    let width = matrix_width(name, rows)?;
    if width != expected_width {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} width {width} must match selected nlats {expected_width}"),
        ));
    }
    Ok(())
}

pub(crate) fn grid_covers_area_judge_bounds_fortran_indexed<T>(
    name: &str,
    grid: &[Vec<T>],
    bounds: AreaJudgeSourceBounds,
) -> io::Result<()> {
    require_len(name, grid.len(), bounds.maxlon_source + 1)?;
    for lon_index in bounds.minlon_source..=bounds.maxlon_source {
        require_len(
            &format!("{name}[{lon_index}]"),
            grid[lon_index].len(),
            bounds.minlat_source + 1,
        )?;
    }
    Ok(())
}
