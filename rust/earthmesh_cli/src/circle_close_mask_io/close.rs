use std::io;
use std::path::Path;

use super::shared::read_nonnegative_refine_netcdf;
use super::types::CloseMask;
use crate::{netcdf_to_io_error, required_dimension_len, required_values_f64, LonLatPoint};

pub fn read_close_refine_netcdf(inputfile: impl AsRef<Path>) -> io::Result<usize> {
    read_nonnegative_refine_netcdf(inputfile, "close_refine")
}

pub fn read_close_mask_netcdf(inputfile: impl AsRef<Path>) -> io::Result<CloseMask> {
    let inputfile = inputfile.as_ref();
    let file = crate::open_netcdf(inputfile).map_err(netcdf_to_io_error)?;
    let close_num = required_dimension_len(&file, "close_num")?;
    let two = required_dimension_len(&file, "two")?;
    if two != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("close two dimension {two} must equal 2"),
        ));
    }
    let refine_degree = read_close_refine_netcdf(inputfile)?;
    let values = required_values_f64(&file, "close_points")?;
    let expected = close_num.checked_mul(2).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("close_points dimensions {close_num}x2 overflow"),
        )
    })?;
    if values.len() != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "close_points contains {} values, expected {expected}",
                values.len()
            ),
        ));
    }
    let points = values
        .chunks_exact(2)
        .map(|row| LonLatPoint {
            lon: row[0],
            lat: row[1],
        })
        .collect::<Vec<_>>();
    let mask = CloseMask {
        refine_degree,
        points,
    };
    validate_close_mask(&mask).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid close mask NetCDF: {err}"),
        )
    })?;
    Ok(mask)
}

pub fn write_close_mask_netcdf(output: impl AsRef<Path>, mask: &CloseMask) -> io::Result<()> {
    let output = output.as_ref();
    crate::ensure_parent_dir(output)?;
    let mut file = crate::create_netcdf(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("close_num", mask.points.len())
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("two", 2).map_err(netcdf_to_io_error)?;
    {
        let mut refine = file
            .add_variable::<i32>("close_refine", &[])
            .map_err(netcdf_to_io_error)?;
        refine
            .put_value(mask.refine_degree as i32, ())
            .map_err(netcdf_to_io_error)?;
    }
    let mut point_values = Vec::with_capacity(mask.points.len() * 2);
    for point in &mask.points {
        point_values.extend([point.lon, point.lat]);
    }
    {
        let mut points = file
            .add_variable::<f64>("close_points", &["close_num", "two"])
            .map_err(netcdf_to_io_error)?;
        points
            .put_values(&point_values, (.., ..))
            .map_err(netcdf_to_io_error)?;
    }
    Ok(())
}

pub(crate) fn validate_close_mask(mask: &CloseMask) -> io::Result<()> {
    if mask.points.len() < 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "close mask must contain at least three points",
        ));
    }
    for (index, point) in mask.points.iter().enumerate() {
        if !point.lon.is_finite() || !point.lat.is_finite() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("close point {} coordinates must be finite", index + 1),
            ));
        }
    }
    Ok(())
}
