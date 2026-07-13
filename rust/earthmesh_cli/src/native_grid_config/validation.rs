use std::io;

use crate::namelist_reader::namelist_assignments;

use super::limits::{
    METHOD_C_NATIVE_MAX_GRIDS, METHOD_C_NATIVE_MAX_GRID_POINTS,
    METHOD_C_NATIVE_MIN_GRID_SPACING_METERS,
};
use super::parse_namelist_usize;

pub(crate) fn validate_native_grid_optional_usize_bounds(
    contents: &str,
    field: &str,
    min_value: usize,
    max_value: usize,
) -> io::Result<()> {
    for assignment in namelist_assignments(contents, "mkgrd")? {
        if assignment.field == field {
            let value = parse_namelist_usize(&assignment.field, &assignment.value)?;
            if value < min_value || value > max_value {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "native Method-C {field} must be in [{min_value}, {max_value}], got {value}"
                    ),
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_native_grid_assignment_grid_index(
    field: &str,
    grid_index: usize,
    max_grids: usize,
) -> io::Result<()> {
    if grid_index == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("native Method-C {field} index must be at least 1"),
        ));
    }
    if grid_index > max_grids {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "native Method-C {field} index {grid_index} exceeds supported maxgrds {max_grids}"
            ),
        ));
    }
    Ok(())
}

pub(crate) fn validate_native_grid_assignment_grid_point_index(
    field: &str,
    grid_index: usize,
    point_index: usize,
    max_grids: usize,
    max_grid_points: usize,
) -> io::Result<()> {
    validate_native_grid_assignment_grid_index(field, grid_index, max_grids)?;
    if point_index == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("native Method-C {field} point index must be at least 1"),
        ));
    }
    if point_index > max_grid_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "native Method-C {field} index ({grid_index},{point_index}) exceeds supported maxngrdll {max_grid_points}"
            ),
        ));
    }
    Ok(())
}

pub(crate) fn validate_native_grid_lat_lon_radius(
    lat_field: &str,
    lon_field: &str,
    radius_field: &str,
    grid_index: usize,
    point_index: usize,
    lat: f64,
    lon: f64,
    radius: f64,
) -> io::Result<()> {
    if !(-90.0..=90.0).contains(&lat) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "native Method-C {lat_field}({grid_index},{point_index}) must be in [-90, 90], got {lat}"
            ),
        ));
    }
    if !(-180.0..=180.0).contains(&lon) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "native Method-C {lon_field}({grid_index},{point_index}) must be in [-180, 180], got {lon}"
            ),
        ));
    }
    if radius < METHOD_C_NATIVE_MIN_GRID_SPACING_METERS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "native Method-C {radius_field}({grid_index},{point_index}) must be at least {METHOD_C_NATIVE_MIN_GRID_SPACING_METERS}, got {radius}"
            ),
        ));
    }
    let max_radius = earthmesh_core::EARTH_RADIUS_METERS * 2.0;
    if radius > max_radius {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "native Method-C {radius_field}({grid_index},{point_index}) must be no greater than {max_radius}, got {radius}"
            ),
        ));
    }
    Ok(())
}

pub(super) fn max_grids() -> usize {
    METHOD_C_NATIVE_MAX_GRIDS
}

pub(super) fn max_grid_points() -> usize {
    METHOD_C_NATIVE_MAX_GRID_POINTS
}
