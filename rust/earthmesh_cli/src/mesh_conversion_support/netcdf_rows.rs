use std::io;

use crate::netcdf_to_io_error;

pub(crate) fn f64_matrix_width(name: &str, rows: &[Vec<f64>]) -> io::Result<usize> {
    let width = rows.first().map(Vec::len).unwrap_or(0);
    if rows.iter().any(|row| row.len() != width) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} rows must have uniform width"),
        ));
    }
    Ok(width)
}

pub(crate) fn matrix_width(name: &str, rows: &[Vec<i32>]) -> io::Result<usize> {
    let width = rows.first().map(Vec::len).unwrap_or(0);
    if rows.iter().any(|row| row.len() != width) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} rows must have uniform width"),
        ));
    }
    Ok(width)
}

pub(crate) fn flatten_i32_rows(rows: &[Vec<i32>]) -> Vec<i32> {
    rows.iter().flat_map(|row| row.iter().copied()).collect()
}

fn flatten_i32_pairs(rows: &[[i32; 2]]) -> Vec<i32> {
    rows.iter().flat_map(|row| row.iter().copied()).collect()
}

fn flatten_f64_rows(rows: &[Vec<f64>]) -> Vec<f64> {
    rows.iter().flat_map(|row| row.iter().copied()).collect()
}

pub(crate) fn write_i32_1d(
    file: &mut netcdf::FileMut,
    name: &str,
    dim: &str,
    values: &[i32],
) -> io::Result<()> {
    let mut var = file
        .add_variable::<i32>(name, &[dim])
        .map_err(netcdf_to_io_error)?;
    var.put_values(values, ..).map_err(netcdf_to_io_error)
}

pub(crate) fn write_i32_matrix_rows(
    file: &mut netcdf::FileMut,
    name: &str,
    dims: &[&str],
    rows: &[Vec<i32>],
) -> io::Result<()> {
    let mut var = file
        .add_variable::<i32>(name, dims)
        .map_err(netcdf_to_io_error)?;
    var.put_values(&flatten_i32_rows(rows), (.., ..))
        .map_err(netcdf_to_io_error)
}

pub(crate) fn write_i32_pair_rows(
    file: &mut netcdf::FileMut,
    name: &str,
    dims: &[&str],
    rows: &[[i32; 2]],
) -> io::Result<()> {
    let mut var = file
        .add_variable::<i32>(name, dims)
        .map_err(netcdf_to_io_error)?;
    var.put_values(&flatten_i32_pairs(rows), (.., ..))
        .map_err(netcdf_to_io_error)
}

pub(crate) fn write_f64_matrix_rows(
    file: &mut netcdf::FileMut,
    name: &str,
    dims: &[&str],
    rows: &[Vec<f64>],
) -> io::Result<()> {
    let mut var = file
        .add_variable::<f64>(name, dims)
        .map_err(netcdf_to_io_error)?;
    var.put_values(&flatten_f64_rows(rows), (.., ..))
        .map_err(netcdf_to_io_error)
}

pub(crate) fn one_to_n_i32(n: usize, name: &str) -> io::Result<Vec<i32>> {
    (1..=n).map(|value| usize_to_i32(name, value)).collect()
}

pub(crate) fn write_f64_1d(
    file: &mut netcdf::FileMut,
    name: &str,
    dim: &str,
    values: &[f64],
) -> io::Result<()> {
    let mut var = file
        .add_variable::<f64>(name, &[dim])
        .map_err(netcdf_to_io_error)?;
    var.put_values(values, ..).map_err(netcdf_to_io_error)
}

pub(crate) fn i32_matrix_from_flat(
    name: &str,
    values: Vec<i32>,
    rows: usize,
    width: usize,
) -> io::Result<Vec<Vec<i32>>> {
    let expected = rows.checked_mul(width).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{name} dimensions {rows}x{width} overflow"),
        )
    })?;
    if values.len() != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{name} contains {} values, expected {expected}",
                values.len()
            ),
        ));
    }
    Ok(rows_from_flat_i32(&values, width))
}

pub(crate) fn rows_from_flat_i32(values: &[i32], width: usize) -> Vec<Vec<i32>> {
    if width == 0 {
        return Vec::new();
    }
    values.chunks_exact(width).map(|row| row.to_vec()).collect()
}

pub(crate) fn usize_values_to_i32(name: &str, values: &[usize]) -> io::Result<Vec<i32>> {
    values
        .iter()
        .map(|&value| usize_to_i32(name, value))
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
