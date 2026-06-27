use std::fs;
use std::io;
use std::path::Path;

use crate::{netcdf_to_io_error, required_dimension_len, required_values_f64};

use super::types::{BBoxMask, BBoxMesh, BBoxPoint};
use super::validation::{validate_bbox_mask, validate_bbox_mesh};

/// Read `bbox_refine` from a bbox NetCDF source used by `bbox_mask_make`.
pub fn read_bbox_refine_netcdf(inputfile: impl AsRef<Path>) -> io::Result<usize> {
    let inputfile = inputfile.as_ref();
    let file = netcdf::open(inputfile).map_err(netcdf_to_io_error)?;
    let variable = file.variable("bbox_refine").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "bbox NetCDF input is missing bbox_refine",
        )
    })?;
    let refine = variable
        .get_value::<i32, _>(())
        .map_err(netcdf_to_io_error)?;
    if refine < 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "bbox_refine must be non-negative",
        ));
    }
    Ok(refine as usize)
}

/// Read `MOD_file_preprocess.F90:bbox_Mesh_Read` NetCDF schema.
pub fn read_bbox_mesh_netcdf(inputfile: impl AsRef<Path>) -> io::Result<BBoxMesh> {
    let file = netcdf::open(inputfile.as_ref()).map_err(netcdf_to_io_error)?;
    let bbox_num = required_dimension_len(&file, "bbox_num")?;
    let four = required_dimension_len(&file, "four")?;
    if four != 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("bbox four dimension {four} must equal 4"),
        ));
    }
    let values = required_values_f64(&file, "bbox_points")?;
    let expected = bbox_num.checked_mul(4).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("bbox_points dimensions {bbox_num}x4 overflow"),
        )
    })?;
    if values.len() != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "bbox_points contains {} values, expected {expected}",
                values.len()
            ),
        ));
    }
    let mesh = BBoxMesh {
        points: values
            .chunks_exact(4)
            .map(|row| BBoxPoint {
                west: row[0],
                east: row[1],
                north: row[2],
                south: row[3],
            })
            .collect(),
    };
    validate_bbox_mesh(&mesh).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid bbox mesh NetCDF: {err}"),
        )
    })?;
    Ok(mesh)
}

/// Write `MOD_file_preprocess.F90:bbox_Mesh_Save` NetCDF schema.
pub fn write_bbox_mesh_netcdf(output: impl AsRef<Path>, mesh: &BBoxMesh) -> io::Result<()> {
    validate_bbox_mesh(mesh).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid bbox mesh: {err}"),
        )
    })?;
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = netcdf::create(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("bbox_num", mesh.points.len())
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("four", 4).map_err(netcdf_to_io_error)?;
    let mut values = Vec::with_capacity(mesh.points.len() * 4);
    for point in &mesh.points {
        values.extend([point.west, point.east, point.north, point.south]);
    }
    {
        let mut bbox_points = file
            .add_variable::<f64>("bbox_points", &["bbox_num", "four"])
            .map_err(netcdf_to_io_error)?;
        bbox_points
            .put_values(&values, (.., ..))
            .map_err(netcdf_to_io_error)?;
    }
    Ok(())
}

/// Read bbox mask points from the NetCDF schema produced by `bbox_mask_make`.
pub fn read_bbox_mask_netcdf(inputfile: impl AsRef<Path>) -> io::Result<BBoxMask> {
    let inputfile = inputfile.as_ref();
    let file = netcdf::open(inputfile).map_err(netcdf_to_io_error)?;
    let bbox_num = required_dimension_len(&file, "bbox_num")?;
    let four = required_dimension_len(&file, "four")?;
    if four != 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("bbox four dimension {four} must equal 4"),
        ));
    }
    let refine_degree = read_bbox_refine_netcdf(inputfile)?;
    let values = required_values_f64(&file, "bbox_points")?;
    let expected = bbox_num.checked_mul(4).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("bbox_points dimensions {bbox_num}x4 overflow"),
        )
    })?;
    if values.len() != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "bbox_points contains {} values, expected {expected}",
                values.len()
            ),
        ));
    }
    let points = values
        .chunks_exact(4)
        .map(|row| BBoxPoint {
            west: row[0],
            east: row[1],
            north: row[2],
            south: row[3],
        })
        .collect::<Vec<_>>();
    let mask = BBoxMask {
        refine_degree,
        points,
    };
    validate_bbox_mask(&mask).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid bbox mask NetCDF: {err}"),
        )
    })?;
    Ok(mask)
}

/// Write bbox mask points to a NetCDF file using the bbox schema consumed by
/// EarthMesh mask preprocessing.
pub fn write_bbox_mask_netcdf(output: impl AsRef<Path>, mask: &BBoxMask) -> io::Result<()> {
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = netcdf::create(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("bbox_num", mask.points.len())
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("four", 4).map_err(netcdf_to_io_error)?;
    {
        let mut refine = file
            .add_variable::<i32>("bbox_refine", &[])
            .map_err(netcdf_to_io_error)?;
        refine
            .put_value(mask.refine_degree as i32, ())
            .map_err(netcdf_to_io_error)?;
    }
    let mut values = Vec::with_capacity(mask.points.len() * 4);
    for point in &mask.points {
        values.extend([point.west, point.east, point.north, point.south]);
    }
    {
        let mut bbox_points = file
            .add_variable::<f64>("bbox_points", &["bbox_num", "four"])
            .map_err(netcdf_to_io_error)?;
        bbox_points
            .put_values(&values, (.., ..))
            .map_err(netcdf_to_io_error)?;
    }
    Ok(())
}
