use std::io;

use crate::olam_native_parser::olam_namelist_assignments;

use super::limits::OLAM_NATIVE_MIN_GRID_SPACING_METERS;
use super::{parse_olam_native_f64, parse_olam_native_usize};

pub(crate) fn olam_native_grid_count(contents: &str, field: &str) -> io::Result<Option<usize>> {
    for assignment in olam_namelist_assignments(contents, "mkgrd")? {
        if assignment.field == field {
            return parse_olam_native_usize(&assignment.field, &assignment.value).map(Some);
        }
    }
    Ok(None)
}

pub(crate) fn read_olam_native_mdomain(contents: &str) -> io::Result<Option<usize>> {
    let Some(mdomain) = olam_native_grid_count(contents, "mdomain")? else {
        return Ok(None);
    };
    if mdomain > 5 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("native OLAM mdomain must be in [0, 5], got {mdomain}"),
        ));
    }
    Ok(Some(mdomain))
}

pub(crate) fn read_olam_native_deltax(contents: &str) -> io::Result<f64> {
    let mut deltax = 1000.0;
    for assignment in olam_namelist_assignments(contents, "mkgrd")? {
        if assignment.field == "deltax" {
            deltax = parse_olam_native_f64(&assignment.field, &assignment.value)?;
        }
    }
    if deltax < OLAM_NATIVE_MIN_GRID_SPACING_METERS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "native OLAM DELTAX must be at least {OLAM_NATIVE_MIN_GRID_SPACING_METERS}, got {deltax}"
            ),
        ));
    }
    Ok(deltax)
}

pub(crate) fn read_olam_native_sfcgrid_res_factor(contents: &str) -> io::Result<usize> {
    let factor = olam_native_grid_count(contents, "sfcgrid_res_factor")?.unwrap_or(1);
    if factor == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "native OLAM sfcgrid_res_factor must be positive",
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
                "native OLAM sfcgrid_res_factor must be 1 or have prime factors only of 2 and/or 3, got {factor}"
            ),
        ));
    }
    Ok(factor)
}
