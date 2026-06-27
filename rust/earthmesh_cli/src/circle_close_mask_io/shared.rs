use std::io;
use std::path::Path;

use crate::netcdf_to_io_error;

pub(super) fn read_nonnegative_refine_netcdf(
    inputfile: impl AsRef<Path>,
    var_name: &str,
) -> io::Result<usize> {
    let file = netcdf::open(inputfile.as_ref()).map_err(netcdf_to_io_error)?;
    let variable = file.variable(var_name).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("NetCDF input is missing {var_name}"),
        )
    })?;
    let refine = variable
        .get_value::<i32, _>(())
        .map_err(netcdf_to_io_error)?;
    if refine < 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{var_name} must be non-negative"),
        ));
    }
    Ok(refine as usize)
}

pub(super) fn parse_float_row(
    line: &str,
    expected: usize,
    label: &str,
    row: usize,
) -> io::Result<Vec<f64>> {
    let values = line
        .split_whitespace()
        .map(|value| {
            value.parse::<f64>().map_err(|err| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid {label} coordinate {value}: {err}"),
                )
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    if values.len() != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} row {row} must contain {expected} values"),
        ));
    }
    Ok(values)
}
