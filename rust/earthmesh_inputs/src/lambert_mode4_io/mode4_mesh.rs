use std::io;
use std::path::Path;

use crate::{
    netcdf_to_io_error, required_dimension_len, required_values_f64, required_values_i32,
    required_values_i32_2d, LonLatPoint,
};

use super::{validate_mode4_mesh_for_area_judge, Mode4Mesh};

/// Read the `MOD_file_preprocess.F90:Mode4_Mesh_Read` NetCDF schema.
pub fn read_mode4_mesh_netcdf(inputfile: impl AsRef<Path>) -> io::Result<Mode4Mesh> {
    let file = crate::open_netcdf(inputfile.as_ref()).map_err(netcdf_to_io_error)?;
    let bound_points = required_dimension_len(&file, "bound_points")?;
    let mode_points = required_dimension_len(&file, "mode_points")?;
    let two = required_dimension_len(&file, "two")?;
    let four = required_dimension_len(&file, "four")?;
    if two != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("mode4 two dimension {two} must equal 2"),
        ));
    }
    if four != 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("mode4 four dimension {four} must equal 4"),
        ));
    }

    let lonlat_values = required_values_f64(&file, "lonlat_bound")?;
    let expected_lonlat = bound_points.checked_mul(2).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("mode4 lonlat_bound dimensions {bound_points}x2 overflow"),
        )
    })?;
    if lonlat_values.len() != expected_lonlat {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "lonlat_bound contains {} values, expected {expected_lonlat}",
                lonlat_values.len()
            ),
        ));
    }
    let lonlat_bound = lonlat_values
        .as_chunks::<2>()
        .0
        .iter()
        .map(|row| LonLatPoint {
            lon: row[0],
            lat: row[1],
        })
        .collect::<Vec<_>>();

    let ngr_values = required_values_i32_2d(&file, "ngr_bound")?;
    let expected_ngr = mode_points.checked_mul(4).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("mode4 ngr_bound dimensions {mode_points}x4 overflow"),
        )
    })?;
    if ngr_values.len() != expected_ngr {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "ngr_bound contains {} values, expected {expected_ngr}",
                ngr_values.len()
            ),
        ));
    }
    let ngr_bound = ngr_values
        .as_chunks::<4>()
        .0
        .iter()
        .map(|row| [row[0], row[1], row[2], row[3]])
        .collect::<Vec<_>>();

    let n_ngr = required_values_i32(&file, "n_ngr")?;
    if n_ngr.len() != mode_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "n_ngr contains {} values, expected {mode_points}",
                n_ngr.len()
            ),
        ));
    }

    let mesh = Mode4Mesh {
        lonlat_bound,
        ngr_bound,
        n_ngr,
    };
    validate_mode4_mesh_for_area_judge(&mesh).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid mode4 mesh NetCDF: {err}"),
        )
    })?;
    Ok(mesh)
}

pub fn write_mode4_mesh_netcdf(output: impl AsRef<Path>, mesh: &Mode4Mesh) -> io::Result<()> {
    if mesh.ngr_bound.len() != mesh.n_ngr.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "mode4 ngr_bound and n_ngr lengths must match",
        ));
    }
    let output = output.as_ref();
    crate::ensure_parent_dir(output)?;
    let mut file = crate::create_netcdf(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("bound_points", mesh.bound_points())
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("mode_points", mesh.mode_points())
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("two", 2).map_err(netcdf_to_io_error)?;
    file.add_dimension("four", 4).map_err(netcdf_to_io_error)?;

    let mut lonlat_values = Vec::with_capacity(mesh.bound_points() * 2);
    for point in &mesh.lonlat_bound {
        lonlat_values.extend([point.lon, point.lat]);
    }
    {
        let mut lonlat_bound = file
            .add_variable::<f64>("lonlat_bound", &["bound_points", "two"])
            .map_err(netcdf_to_io_error)?;
        lonlat_bound
            .put_values(&lonlat_values, (.., ..))
            .map_err(netcdf_to_io_error)?;
    }

    let mut ngr_values: Vec<i32> = Vec::with_capacity(mesh.mode_points() * 4);
    for row in &mesh.ngr_bound {
        ngr_values.extend_from_slice(row);
    }
    {
        let mut ngr_bound = file
            .add_variable::<i32>("ngr_bound", &["mode_points", "four"])
            .map_err(netcdf_to_io_error)?;
        ngr_bound
            .put_values(&ngr_values, (.., ..))
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut n_ngr = file
            .add_variable::<i32>("n_ngr", &["mode_points"])
            .map_err(netcdf_to_io_error)?;
        n_ngr
            .put_values(&mesh.n_ngr, ..)
            .map_err(netcdf_to_io_error)?;
    }
    Ok(())
}
