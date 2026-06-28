use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::{flatten_i32_rows, netcdf_to_io_error, validate_mpas_simple_mesh, write_f64_1d};

/// Rust data shape written by `MOD_file_preprocess.F90:MPAS_Mesh_Simple_Save`.
#[derive(Debug, Clone, PartialEq)]
pub struct MpasSimpleMesh {
    pub x_cell: Vec<f64>,
    pub y_cell: Vec<f64>,
    pub z_cell: Vec<f64>,
    pub x_vertex: Vec<f64>,
    pub y_vertex: Vec<f64>,
    pub z_vertex: Vec<f64>,
    pub cells_on_vertex: Vec<Vec<i32>>,
    pub mesh_density: Vec<f64>,
}

/// Evidence report from writing `MOD_file_preprocess.F90:MPAS_Mesh_Simple_Save` output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MpasSimpleMeshWriteReport {
    pub output: PathBuf,
    pub n_cells: usize,
    pub n_vertices: usize,
}

/// Write the simple MPAS mesh schema produced by
/// `MOD_file_preprocess.F90:MPAS_Mesh_Simple_Save`.
pub fn write_mpas_simple_mesh_netcdf(
    output: impl AsRef<Path>,
    mesh: &MpasSimpleMesh,
) -> io::Result<MpasSimpleMeshWriteReport> {
    validate_mpas_simple_mesh(mesh)?;
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let n_cells = mesh.x_cell.len() - 1;
    let n_vertices = mesh.x_vertex.len() - 1;

    let mut file = crate::create_netcdf(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("nCells", n_cells)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("nVertices", n_vertices)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("vertexDegree", 3)
        .map_err(netcdf_to_io_error)?;

    write_f64_1d(&mut file, "xCell", "nCells", &mesh.x_cell[1..])?;
    write_f64_1d(&mut file, "yCell", "nCells", &mesh.y_cell[1..])?;
    write_f64_1d(&mut file, "zCell", "nCells", &mesh.z_cell[1..])?;
    write_f64_1d(&mut file, "xVertex", "nVertices", &mesh.x_vertex[1..])?;
    write_f64_1d(&mut file, "yVertex", "nVertices", &mesh.y_vertex[1..])?;
    write_f64_1d(&mut file, "zVertex", "nVertices", &mesh.z_vertex[1..])?;
    {
        let mut var = file
            .add_variable::<i32>("cellsOnVertex", &["nVertices", "vertexDegree"])
            .map_err(netcdf_to_io_error)?;
        var.put_attribute("units", "-")
            .map_err(netcdf_to_io_error)?;
        var.put_attribute("long_name", "IDs of the cells that meet at a vertex")
            .map_err(netcdf_to_io_error)?;
        var.put_values(&flatten_i32_rows(&mesh.cells_on_vertex[1..]), (.., ..))
            .map_err(netcdf_to_io_error)?;
    }
    write_f64_1d(&mut file, "meshDensity", "nCells", &mesh.mesh_density[1..])?;

    file.add_attribute("on_a_sphere", "YES")
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("sphere_radius", 1.0_f64)
        .map_err(netcdf_to_io_error)?;

    Ok(MpasSimpleMeshWriteReport {
        output: output.to_path_buf(),
        n_cells,
        n_vertices,
    })
}
