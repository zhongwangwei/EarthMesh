use std::io;
use std::path::Path;

use crate::{
    build_mpas_mesh_from_unstructured_one_based,
    build_mpas_simple_mesh_from_unstructured_one_based, read_cellwidth_netcdf,
    read_unstructured_mesh_netcdf, write_mpas_graph_info, write_mpas_mesh_netcdf,
    write_mpas_simple_mesh_netcdf, MpasFullMeshPipelineReport,
};

/// File-level replacement for the `MPAS_Mesh_Cal_Simple` path that reads the
/// EarthMesh gridfile plus `cellwidth_NXP####_global.nc4`, builds the simple
/// MPAS payload, and writes `MPASOUT_NXP####_global_Simple.nc4`-compatible
/// NetCDF.
pub fn write_mpas_simple_mesh_from_netcdf_inputs(
    gridfile: impl AsRef<Path>,
    cellwidth_file: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> io::Result<crate::MpasSimpleMeshWriteReport> {
    let mesh = read_unstructured_mesh_netcdf(gridfile)?;
    let cellwidth = read_cellwidth_netcdf(cellwidth_file)?;
    let simple = build_mpas_simple_mesh_from_unstructured_one_based(&mesh, &cellwidth)?;
    write_mpas_simple_mesh_netcdf(output, &simple)
}

/// File-level replacement for the full `MPAS_Mesh_Cal` path that reads the
/// EarthMesh gridfile plus `cellwidth_NXP####_global.nc4`, builds the full MPAS
/// payload, and writes both `MPASOUT_NXP####_global.nc4` and graph.info.
pub fn write_mpas_mesh_from_netcdf_inputs(
    gridfile: impl AsRef<Path>,
    cellwidth_file: impl AsRef<Path>,
    mesh_output: impl AsRef<Path>,
    graph_output: impl AsRef<Path>,
    nxp: usize,
    step: usize,
) -> io::Result<MpasFullMeshPipelineReport> {
    let mesh = read_unstructured_mesh_netcdf(gridfile)?;
    let cellwidth = read_cellwidth_netcdf(cellwidth_file)?;
    let mpas = build_mpas_mesh_from_unstructured_one_based(&mesh, &cellwidth, nxp, step)?;
    let mesh_report = write_mpas_mesh_netcdf(mesh_output, &mpas)?;
    let graph_info = write_mpas_graph_info(
        graph_output,
        10,
        &mpas.cells_on_cell,
        &mpas.cells_on_edge,
        &mpas.n_edges_on_cell,
    )?;
    Ok(MpasFullMeshPipelineReport {
        mesh: mesh_report,
        graph_info,
    })
}
