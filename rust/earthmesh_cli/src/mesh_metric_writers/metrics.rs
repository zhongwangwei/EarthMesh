use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::{
    lat_values, lon_values, netcdf_to_io_error, require_len, required_dimension_len,
    required_values_f64, write_f64_1d, LonLatPoint,
};

/// Rust data shape written by `MOD_file_preprocess.F90:cellwidth_save`.
#[derive(Debug, Clone, PartialEq)]
pub struct CellwidthMesh {
    pub cell_points: Vec<LonLatPoint>,
    pub cellwidth: Vec<f64>,
}

/// Rust data shape written by `MOD_file_preprocess.F90:distsOnEdge_save`.
#[derive(Debug, Clone, PartialEq)]
pub struct DistsOnEdgeMesh {
    pub edge_points: Vec<LonLatPoint>,
    pub dists_on_edge: Vec<f64>,
}

/// Evidence report from writing `MOD_file_preprocess.F90:cellwidth_save` output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellwidthWriteReport {
    pub output: PathBuf,
    pub num_dbx: usize,
}

/// Evidence report from writing `MOD_file_preprocess.F90:distsOnEdge_save` output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistsOnEdgeWriteReport {
    pub output: PathBuf,
    pub num_edge: usize,
}

/// Write the `distsOnEdge_NXP####_##_global.nc4` schema produced by
/// `MOD_file_preprocess.F90:distsOnEdge_save`.
pub fn write_dists_on_edge_netcdf(
    output: impl AsRef<Path>,
    mesh: &DistsOnEdgeMesh,
) -> io::Result<DistsOnEdgeWriteReport> {
    validate_dists_on_edge_mesh(mesh)?;
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let num_edge = mesh.edge_points.len();
    let mut file = crate::create_netcdf(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("num_edge", num_edge)
        .map_err(netcdf_to_io_error)?;
    write_f64_1d(
        &mut file,
        "lonv",
        "num_edge",
        &lon_values(&mesh.edge_points),
    )?;
    write_f64_1d(
        &mut file,
        "latv",
        "num_edge",
        &lat_values(&mesh.edge_points),
    )?;
    write_f64_1d(&mut file, "distsOnEdge", "num_edge", &mesh.dists_on_edge)?;

    Ok(DistsOnEdgeWriteReport {
        output: output.to_path_buf(),
        num_edge,
    })
}

/// Write the `cellwidth_NXP####_global.nc4` schema produced by
/// `MOD_file_preprocess.F90:cellwidth_save`.
pub fn write_cellwidth_netcdf(
    output: impl AsRef<Path>,
    mesh: &CellwidthMesh,
) -> io::Result<CellwidthWriteReport> {
    validate_cellwidth_mesh(mesh)?;
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let num_dbx = mesh.cell_points.len();
    let mut file = crate::create_netcdf(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("num_dbx", num_dbx)
        .map_err(netcdf_to_io_error)?;
    write_f64_1d(&mut file, "lonw", "num_dbx", &lon_values(&mesh.cell_points))?;
    write_f64_1d(&mut file, "latw", "num_dbx", &lat_values(&mesh.cell_points))?;
    write_f64_1d(&mut file, "cellwidth", "num_dbx", &mesh.cellwidth)?;

    Ok(CellwidthWriteReport {
        output: output.to_path_buf(),
        num_dbx,
    })
}

/// Read the `cellwidth_NXP####_global.nc4` schema produced by
/// `MOD_file_preprocess.F90:cellwidth_save`.
pub fn read_cellwidth_netcdf(input: impl AsRef<Path>) -> io::Result<Vec<f64>> {
    let file = crate::open_netcdf(input.as_ref()).map_err(netcdf_to_io_error)?;
    let num_dbx = required_dimension_len(&file, "num_dbx")?;
    let cellwidth = required_values_f64(&file, "cellwidth")?;
    require_len("cellwidth", cellwidth.len(), num_dbx)?;
    Ok(cellwidth.into_iter().take(num_dbx).collect())
}

fn validate_cellwidth_mesh(mesh: &CellwidthMesh) -> io::Result<()> {
    if mesh.cellwidth.len() != mesh.cell_points.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "cellwidth length {} must match cell_points length {}",
                mesh.cellwidth.len(),
                mesh.cell_points.len()
            ),
        ));
    }
    Ok(())
}

fn validate_dists_on_edge_mesh(mesh: &DistsOnEdgeMesh) -> io::Result<()> {
    if mesh.dists_on_edge.len() != mesh.edge_points.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "dists_on_edge length {} must match edge_points length {}",
                mesh.dists_on_edge.len(),
                mesh.edge_points.len()
            ),
        ));
    }
    Ok(())
}
