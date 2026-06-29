use std::io;
use std::path::Path;

use crate::{
    lat_values, lon_values, netcdf_to_io_error, unstructured_dimc, validate_unstructured_mesh,
    UnstructuredMesh, UnstructuredMeshWriteReport,
};

use super::rows::{flatten_m_to_w, flatten_w_to_m};

pub fn write_unstructured_mesh_netcdf(
    output: impl AsRef<Path>,
    mesh: &UnstructuredMesh,
) -> io::Result<UnstructuredMeshWriteReport> {
    validate_unstructured_mesh(mesh)?;
    let output = output.as_ref();
    crate::ensure_parent_dir(output)?;

    let dimc = unstructured_dimc(mesh);
    let mut file = crate::create_netcdf(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("sjx_points", mesh.m_points.len())
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("lbx_points", mesh.w_points.len())
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("dimb", 3).map_err(netcdf_to_io_error)?;
    file.add_dimension("dimc", dimc)
        .map_err(netcdf_to_io_error)?;

    {
        let mut var = file
            .add_variable::<f64>("GLONM", &["sjx_points"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&lon_values(&mesh.m_points), ..)
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<f64>("GLATM", &["sjx_points"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&lat_values(&mesh.m_points), ..)
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<f64>("GLONW", &["lbx_points"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&lon_values(&mesh.w_points), ..)
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<f64>("GLATW", &["lbx_points"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&lat_values(&mesh.w_points), ..)
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<i32>("itab_m%iw", &["sjx_points", "dimb"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&flatten_m_to_w(&mesh.m_to_w), (.., ..))
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<i32>("itab_w%im", &["lbx_points", "dimc"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&flatten_w_to_m(&mesh.w_to_m, dimc), (.., ..))
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<i32>("n_ngrwm", &["lbx_points"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&mesh.n_w_to_m, ..)
            .map_err(netcdf_to_io_error)?;
    }

    Ok(UnstructuredMeshWriteReport {
        output: output.to_path_buf(),
        sjx_points: mesh.m_points.len(),
        lbx_points: mesh.w_points.len(),
        dimc,
    })
}
