use std::io;
use std::path::Path;

use crate::*;

/// Build a standard MPAS mesh NetCDF (+ `graph.info`) straight from a base
/// `gridfile`, without the spring/refine pipeline or a cellwidth file. A uniform
/// cellwidth is synthesized: for an unrefined mesh every cell has the same width,
/// so `meshDensity == 1`, which is exactly correct. Reuses the same validated
/// builder/writer as the full pipeline, so the output is the standard MPAS schema.
pub fn write_standard_mpas_from_gridfile(
    gridfile: impl AsRef<Path>,
    mesh_output: impl AsRef<Path>,
    graph_output: impl AsRef<Path>,
    nxp: usize,
) -> io::Result<MpasFullMeshPipelineReport> {
    let mesh = read_unstructured_mesh_netcdf(gridfile)?;
    let base_width = if nxp > 0 { 7680.0 / nxp as f64 } else { 1.0 };
    let cellwidth = vec![base_width; mesh.w_points.len()];
    let mpas = build_mpas_mesh_from_unstructured_fortran_indexed(&mesh, &cellwidth, nxp, 1)?;
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

/// Write a regional (limited-area) MPAS mesh + graph.info from a global hex
/// gridfile, keeping only the cells whose centre falls inside `region`.
///
/// Builds the full global MPAS mesh (the validated path), then [`subset_mpas_mesh`]
/// re-indexes it to the region: every kept cell is geometrically complete and its
/// geometry is preserved exactly, while connectivity to dropped cells collapses to
/// the MPAS `0` no-neighbour marker (boundary cells/edges). Returns the report and
/// the number of cells kept.
pub fn write_regional_mpas_from_gridfile(
    gridfile: impl AsRef<Path>,
    mesh_output: impl AsRef<Path>,
    graph_output: impl AsRef<Path>,
    region: &GridRegion,
    nxp: usize,
) -> io::Result<(MpasFullMeshPipelineReport, usize)> {
    let mesh = read_unstructured_mesh_netcdf(gridfile)?;
    let base_width = if nxp > 0 { 7680.0 / nxp as f64 } else { 1.0 };
    let cellwidth = vec![base_width; mesh.w_points.len()];
    let global = build_mpas_mesh_from_unstructured_fortran_indexed(&mesh, &cellwidth, nxp, 1)?;

    let n_cells = global.lat_cell.len();
    let mut keep_cell = vec![false; n_cells];
    let mut kept = 0usize;
    for c in 1..n_cells {
        let lon_deg = global.lon_cell[c].to_degrees();
        let lat_deg = global.lat_cell[c].to_degrees();
        if region.contains(lon_deg, lat_deg) {
            keep_cell[c] = true;
            kept += 1;
        }
    }
    if kept == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "region contains no cells",
        ));
    }

    let regional = subset_mpas_mesh(&global, &keep_cell)?;
    let mesh_report = write_mpas_mesh_netcdf(mesh_output, &regional)?;
    let graph_info = write_mpas_graph_info(
        graph_output,
        10,
        &regional.cells_on_cell,
        &regional.cells_on_edge,
        &regional.n_edges_on_cell,
    )?;
    Ok((
        MpasFullMeshPipelineReport {
            mesh: mesh_report,
            graph_info,
        },
        kept,
    ))
}
