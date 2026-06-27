use std::io;

use crate::matrix_width;

use super::types::{ContainMesh, FlatContainMesh};

pub(crate) fn validate_contain_mesh(contain: &ContainMesh) -> io::Result<()> {
    matrix_width("ustr_id", &contain.ustr_id)?;
    matrix_width("ustr_ii", &contain.ustr_ii)?;
    if contain.is_in_area_ustr.len() != contain.ustr_id.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "IsInArea_ustr length {} must match num_ustr {}",
                contain.is_in_area_ustr.len(),
                contain.ustr_id.len()
            ),
        ));
    }
    Ok(())
}

pub(crate) fn validate_flat_contain_mesh(contain: &FlatContainMesh) -> io::Result<()> {
    if contain.ustr_id_width == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "flat ustr_id width must be positive",
        ));
    }
    if !contain
        .ustr_id_values
        .len()
        .is_multiple_of(contain.ustr_id_width)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "flat ustr_id length must be a multiple of row width",
        ));
    }
    if contain.ustr_ii_width == 0 && !contain.ustr_ii_values.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "flat ustr_ii width must be positive when values are present",
        ));
    }
    if contain.ustr_ii_width != 0
        && !contain
            .ustr_ii_values
            .len()
            .is_multiple_of(contain.ustr_ii_width)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "flat ustr_ii length must be a multiple of row width",
        ));
    }
    if contain.is_in_area_ustr.len() != contain.num_ustr() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "IsInArea_ustr length {} must match num_ustr {}",
                contain.is_in_area_ustr.len(),
                contain.num_ustr()
            ),
        ));
    }
    Ok(())
}
