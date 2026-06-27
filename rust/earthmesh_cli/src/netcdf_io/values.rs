use std::io;

use crate::netcdf_to_io_error;

pub(crate) fn required_values_f64(file: &netcdf::File, name: &str) -> io::Result<Vec<f64>> {
    file.variable(name)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("missing {name} variable"),
            )
        })?
        .get_values::<f64, _>(..)
        .map_err(netcdf_to_io_error)
}

pub(crate) fn required_values_f64_any(file: &netcdf::File, name: &str) -> io::Result<Vec<f64>> {
    let variable = file.variable(name).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("missing {name} variable"),
        )
    })?;
    if let Ok(values) = variable.get_values::<f64, _>(..) {
        return Ok(values);
    }
    if let Ok(values) = variable.get_values::<f32, _>(..) {
        return Ok(values.into_iter().map(f64::from).collect());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{name} variable must be readable as f64 or f32"),
    ))
}

pub(crate) fn required_values_i32(file: &netcdf::File, name: &str) -> io::Result<Vec<i32>> {
    file.variable(name)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("missing {name} variable"),
            )
        })?
        .get_values::<i32, _>(..)
        .map_err(netcdf_to_io_error)
}

pub(crate) fn required_values_i8(file: &netcdf::File, name: &str) -> io::Result<Vec<i8>> {
    file.variable(name)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("missing {name} variable"),
            )
        })?
        .get_values::<i8, _>(..)
        .map_err(netcdf_to_io_error)
}
