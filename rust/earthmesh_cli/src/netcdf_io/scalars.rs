use std::io;

use crate::{netcdf_to_io_error, required_values_i32};

fn required_scalar_i32(file: &netcdf::File, name: &str) -> io::Result<i32> {
    let values = required_values_i32(file, name)?;
    match values.as_slice() {
        [value] => Ok(*value),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{name} scalar must contain exactly one value, found {}",
                values.len()
            ),
        )),
    }
}

pub(crate) fn required_scalar_usize_i32(file: &netcdf::File, name: &str) -> io::Result<usize> {
    let value = required_scalar_i32(file, name)?;
    usize::try_from(value).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{name} value {value} must be non-negative"),
        )
    })
}

pub(crate) fn write_i32_scalar(
    file: &mut netcdf::FileMut,
    name: &str,
    value: i32,
) -> io::Result<()> {
    let mut var = file
        .add_variable::<i32>(name, &[])
        .map_err(netcdf_to_io_error)?;
    var.put_values(&[value], ..).map_err(netcdf_to_io_error)
}

pub(crate) fn write_f64_scalar(
    file: &mut netcdf::FileMut,
    name: &str,
    value: f64,
) -> io::Result<()> {
    let mut var = file
        .add_variable::<f64>(name, &[])
        .map_err(netcdf_to_io_error)?;
    var.put_values(&[value], ..).map_err(netcdf_to_io_error)
}
