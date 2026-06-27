use std::io;

use crate::netcdf_to_io_error;

pub(crate) fn write_colm_i32_var(
    file: &mut netcdf::FileMut,
    name: &str,
    values: &[i32],
) -> io::Result<()> {
    let mut var = file
        .add_variable::<i32>(name, &["cell"])
        .map_err(netcdf_to_io_error)?;
    var.put_values(values, ..).map_err(netcdf_to_io_error)
}

pub(crate) fn write_colm_i8_var(
    file: &mut netcdf::FileMut,
    name: &str,
    values: &[i8],
) -> io::Result<()> {
    let mut var = file
        .add_variable::<i8>(name, &["cell"])
        .map_err(netcdf_to_io_error)?;
    var.put_values(values, ..).map_err(netcdf_to_io_error)
}

pub(crate) fn write_colm_f64_var(
    file: &mut netcdf::FileMut,
    name: &str,
    values: &[f64],
    units: Option<&str>,
) -> io::Result<()> {
    let mut var = file
        .add_variable::<f64>(name, &["cell"])
        .map_err(netcdf_to_io_error)?;
    if let Some(units) = units {
        var.put_attribute("units", units)
            .map_err(netcdf_to_io_error)?;
    }
    var.put_values(values, ..).map_err(netcdf_to_io_error)
}
