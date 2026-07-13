use std::{io, path::Path};

use earthmesh_core::{GridMemory, IjTabs};

use crate::{
    gridfile_mesh_from_one_based_state, gridfile_mesh_from_state, gridfile_output_path,
    write_unstructured_mesh_netcdf, UnstructuredMeshWriteReport,
};

pub fn write_gridfile_from_state(
    file_dir: impl AsRef<Path>,
    nxp: usize,
    step: usize,
    mode_grid: &str,
    grid: &GridMemory,
    tabs: &IjTabs,
) -> io::Result<UnstructuredMeshWriteReport> {
    let mesh = gridfile_mesh_from_state(grid, tabs)?;
    let output = gridfile_output_path(file_dir, nxp, step, mode_grid);
    write_unstructured_mesh_netcdf(output, &mesh)
}

pub fn write_gridfile_from_one_based_state(
    file_dir: impl AsRef<Path>,
    nxp: usize,
    step: usize,
    mode_grid: &str,
    grid: &GridMemory,
    tabs: &IjTabs,
) -> io::Result<UnstructuredMeshWriteReport> {
    let mesh = gridfile_mesh_from_one_based_state(grid, tabs)?;
    let output = gridfile_output_path(file_dir, nxp, step, mode_grid);
    write_unstructured_mesh_netcdf(output, &mesh)
}
