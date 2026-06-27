use std::fs;
use std::io;
use std::path::Path;

use crate::{netcdf_to_io_error, required_dimension_len, required_values_f64, LonLatPoint};

fn validate_close_mesh_points(points: &[LonLatPoint]) -> io::Result<()> {
    if points.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "close mesh must contain at least one point",
        ));
    }
    for (index, point) in points.iter().enumerate() {
        if !point.lon.is_finite() || !point.lat.is_finite() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("close mesh point {} must be finite", index + 1),
            ));
        }
    }
    Ok(())
}

/// Read the `MOD_file_preprocess.F90:close_Mesh_Read` NetCDF schema.
///
/// Unlike `read_close_mask_netcdf`, this compatibility reader intentionally
/// does not require or read `close_refine`; it is the small close-curve schema
/// written by `close_Mesh_Save` for refinement boundary patches.
pub fn read_close_mesh_netcdf(inputfile: impl AsRef<Path>) -> io::Result<Vec<LonLatPoint>> {
    let file = netcdf::open(inputfile.as_ref()).map_err(netcdf_to_io_error)?;
    let close_num = required_dimension_len(&file, "close_num")?;
    let two = required_dimension_len(&file, "two")?;
    if two != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("close mesh two dimension {two} must equal 2"),
        ));
    }
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
    validate_close_mesh_points(&points)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
    Ok(points)
}

/// Write the `MOD_file_preprocess.F90:close_Mesh_Save` NetCDF schema.
///
/// This is deliberately separate from `write_close_mask_netcdf`: mask inputs
/// include a scalar `close_refine`, while `close_Mesh_Save` writes only
/// `close_num`, `two`, and `close_points`.
pub fn write_close_mesh_netcdf(output: impl AsRef<Path>, points: &[LonLatPoint]) -> io::Result<()> {
    validate_close_mesh_points(points)?;
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = netcdf::create(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("close_num", points.len())
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("two", 2).map_err(netcdf_to_io_error)?;
    let mut point_values = Vec::with_capacity(points.len() * 2);
    for point in points {
        point_values.extend([point.lon, point.lat]);
    }
    {
        let mut variable = file
            .add_variable::<f64>("close_points", &["close_num", "two"])
            .map_err(netcdf_to_io_error)?;
        variable
            .put_values(&point_values, (.., ..))
            .map_err(netcdf_to_io_error)?;
    }
    Ok(())
}
