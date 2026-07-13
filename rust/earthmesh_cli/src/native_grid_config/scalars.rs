use std::io;

use crate::namelist_reader::namelist_assignments;

use super::limits::METHOD_C_NATIVE_MIN_GRID_SPACING_METERS;
use super::{parse_namelist_f64, parse_namelist_usize};

pub(crate) fn native_grid_grid_count(contents: &str, field: &str) -> io::Result<Option<usize>> {
    for assignment in namelist_assignments(contents, "mkgrd")? {
        if assignment.field == field {
            return parse_namelist_usize(&assignment.field, &assignment.value).map(Some);
        }
    }
    Ok(None)
}

pub(crate) fn read_native_grid_mdomain(contents: &str) -> io::Result<Option<usize>> {
    let Some(mdomain) = native_grid_grid_count(contents, "mdomain")? else {
        return Ok(None);
    };
    if mdomain > 5 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("native Method-C mdomain must be in [0, 5], got {mdomain}"),
        ));
    }
    Ok(Some(mdomain))
}

pub(crate) fn read_native_grid_deltax(contents: &str) -> io::Result<f64> {
    let mut deltax = 1000.0;
    for assignment in namelist_assignments(contents, "mkgrd")? {
        if assignment.field == "deltax" {
            deltax = parse_namelist_f64(&assignment.field, &assignment.value)?;
        }
    }
    if deltax < METHOD_C_NATIVE_MIN_GRID_SPACING_METERS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "native Method-C DELTAX must be at least {METHOD_C_NATIVE_MIN_GRID_SPACING_METERS}, got {deltax}"
            ),
        ));
    }
    Ok(deltax)
}

pub(crate) fn read_native_grid_sfcgrid_res_factor(contents: &str) -> io::Result<usize> {
    let factor = native_grid_grid_count(contents, "sfcgrid_res_factor")?.unwrap_or(1);
    if factor == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "native Method-C sfcgrid_res_factor must be positive",
        ));
    }
    let mut remaining_factor = factor;
    while remaining_factor % 2 == 0 {
        remaining_factor /= 2;
    }
    while remaining_factor % 3 == 0 {
        remaining_factor /= 3;
    }
    if remaining_factor != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "native Method-C sfcgrid_res_factor must be 1 or have prime factors only of 2 and/or 3, got {factor}"
            ),
        ));
    }
    Ok(factor)
}
