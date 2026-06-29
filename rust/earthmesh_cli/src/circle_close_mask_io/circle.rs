use std::io;
use std::path::Path;

use super::shared::read_nonnegative_refine_netcdf;
use super::types::{CircleMask, CircleMesh};
use crate::{netcdf_to_io_error, required_dimension_len, required_values_f64, LonLatPoint};

pub fn read_circle_refine_netcdf(inputfile: impl AsRef<Path>) -> io::Result<usize> {
    read_nonnegative_refine_netcdf(inputfile, "circle_refine")
}

/// Read `MOD_file_preprocess.F90:circle_Mesh_Read` NetCDF schema.
pub fn read_circle_mesh_netcdf(inputfile: impl AsRef<Path>) -> io::Result<CircleMesh> {
    let file = crate::open_netcdf(inputfile.as_ref()).map_err(netcdf_to_io_error)?;
    let circle_num = required_dimension_len(&file, "circle_num")?;
    let two = required_dimension_len(&file, "two")?;
    if two != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("circle two dimension {two} must equal 2"),
        ));
    }
    let point_values = required_values_f64(&file, "circle_points")?;
    let expected_points = circle_num.checked_mul(2).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("circle_points dimensions {circle_num}x2 overflow"),
        )
    })?;
    if point_values.len() != expected_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "circle_points contains {} values, expected {expected_points}",
                point_values.len()
            ),
        ));
    }
    let radius_km = required_values_f64(&file, "circle_radius")?;
    if radius_km.len() != circle_num {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "circle_radius contains {} values, expected {circle_num}",
                radius_km.len()
            ),
        ));
    }
    let mesh = CircleMesh {
        points: point_values
            .chunks_exact(2)
            .map(|row| LonLatPoint {
                lon: row[0],
                lat: row[1],
            })
            .collect(),
        radius_km,
    };
    validate_circle_mesh(&mesh).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid circle mesh NetCDF: {err}"),
        )
    })?;
    Ok(mesh)
}

/// Write `MOD_file_preprocess.F90:circle_Mesh_Save` NetCDF schema.
pub fn write_circle_mesh_netcdf(output: impl AsRef<Path>, mesh: &CircleMesh) -> io::Result<()> {
    validate_circle_mesh(mesh).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid circle mesh: {err}"),
        )
    })?;
    let output = output.as_ref();
    crate::ensure_parent_dir(output)?;
    let mut file = crate::create_netcdf(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("circle_num", mesh.points.len())
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("two", 2).map_err(netcdf_to_io_error)?;
    let mut point_values = Vec::with_capacity(mesh.points.len() * 2);
    for point in &mesh.points {
        point_values.extend([point.lon, point.lat]);
    }
    {
        let mut points = file
            .add_variable::<f64>("circle_points", &["circle_num", "two"])
            .map_err(netcdf_to_io_error)?;
        points
            .put_values(&point_values, (.., ..))
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut radius = file
            .add_variable::<f64>("circle_radius", &["circle_num"])
            .map_err(netcdf_to_io_error)?;
        radius
            .put_values(&mesh.radius_km, ..)
            .map_err(netcdf_to_io_error)?;
    }
    Ok(())
}

pub fn read_circle_mask_netcdf(inputfile: impl AsRef<Path>) -> io::Result<CircleMask> {
    let inputfile = inputfile.as_ref();
    let file = crate::open_netcdf(inputfile).map_err(netcdf_to_io_error)?;
    let circle_num = required_dimension_len(&file, "circle_num")?;
    let two = required_dimension_len(&file, "two")?;
    if two != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("circle two dimension {two} must equal 2"),
        ));
    }
    let refine_degree = read_circle_refine_netcdf(inputfile)?;
    let point_values = required_values_f64(&file, "circle_points")?;
    let expected_points = circle_num.checked_mul(2).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("circle_points dimensions {circle_num}x2 overflow"),
        )
    })?;
    if point_values.len() != expected_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "circle_points contains {} values, expected {expected_points}",
                point_values.len()
            ),
        ));
    }
    let radius_km = required_values_f64(&file, "circle_radius")?;
    if radius_km.len() != circle_num {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "circle_radius contains {} values, expected {circle_num}",
                radius_km.len()
            ),
        ));
    }
    let points = point_values
        .chunks_exact(2)
        .map(|row| LonLatPoint {
            lon: row[0],
            lat: row[1],
        })
        .collect::<Vec<_>>();
    let mask = CircleMask {
        refine_degree,
        points,
        radius_km,
    };
    validate_circle_mask(&mask).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid circle mask NetCDF: {err}"),
        )
    })?;
    Ok(mask)
}

pub fn write_circle_mask_netcdf(output: impl AsRef<Path>, mask: &CircleMask) -> io::Result<()> {
    if mask.points.len() != mask.radius_km.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "circle points and radius arrays must have the same length",
        ));
    }
    let output = output.as_ref();
    crate::ensure_parent_dir(output)?;
    let mut file = crate::create_netcdf(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("circle_num", mask.points.len())
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("two", 2).map_err(netcdf_to_io_error)?;
    {
        let mut refine = file
            .add_variable::<i32>("circle_refine", &[])
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
            .add_variable::<f64>("circle_points", &["circle_num", "two"])
            .map_err(netcdf_to_io_error)?;
        points
            .put_values(&point_values, (.., ..))
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut radius = file
            .add_variable::<f64>("circle_radius", &["circle_num"])
            .map_err(netcdf_to_io_error)?;
        radius
            .put_values(&mask.radius_km, ..)
            .map_err(netcdf_to_io_error)?;
    }
    Ok(())
}

fn validate_circle_mesh(mesh: &CircleMesh) -> io::Result<()> {
    if mesh.points.len() != mesh.radius_km.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "circle points and radius arrays must have the same length",
        ));
    }
    for (index, (point, radius)) in mesh.points.iter().zip(mesh.radius_km.iter()).enumerate() {
        if !point.lon.is_finite() || !point.lat.is_finite() || !radius.is_finite() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "circle point {} coordinates and radius must be finite",
                    index + 1
                ),
            ));
        }
        if *radius < 0.0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("circle point {} radius must be non-negative", index + 1),
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_circle_mask(mask: &CircleMask) -> io::Result<()> {
    validate_circle_mesh(&CircleMesh {
        points: mask.points.clone(),
        radius_km: mask.radius_km.clone(),
    })
}
