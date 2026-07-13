use crate::cells_on_triangle_one_based_from_mesh;
use crate::lonlat_degrees_from_points;
use crate::n_edges_on_cell_usize_from_mesh;
use crate::read_unstructured_mesh_netcdf;
use crate::triangles_on_cell_one_based_from_mesh;
use crate::write_unstructured_mesh_netcdf;
use crate::SpringjustmentRegionalGridfileReport;
use crate::SpringjustmentRegionalRunOptions;
use crate::UnstructuredMesh;
use crate::UnstructuredMeshWriteReport;
use earthmesh_mesh::springjustment_regional_core_one_based;
use earthmesh_mesh::SpringjustmentRegionalCoreInput;
use std::io;
use std::path::Path;

use super::conversion::unstructured_mesh_from_springjustment_regional;

pub fn run_springjustment_regional_from_unstructured_gridfile(
    gridfile: impl AsRef<Path>,
    options: SpringjustmentRegionalRunOptions<'_>,
) -> io::Result<SpringjustmentRegionalGridfileReport> {
    let mesh = read_unstructured_mesh_netcdf(gridfile)?;
    run_springjustment_regional_from_unstructured_mesh(&mesh, options)
}

pub fn run_springjustment_regional_from_unstructured_mesh(
    mesh: &UnstructuredMesh,
    options: SpringjustmentRegionalRunOptions<'_>,
) -> io::Result<SpringjustmentRegionalGridfileReport> {
    let cells_on_triangle = cells_on_triangle_one_based_from_mesh(mesh)?;
    let triangles_on_cell = triangles_on_cell_one_based_from_mesh(mesh)?;
    let n_edges_on_cell = n_edges_on_cell_usize_from_mesh(mesh)?;
    let triangle_lonlat = lonlat_degrees_from_points(&mesh.m_points);
    let cell_lonlat = lonlat_degrees_from_points(&mesh.w_points);

    let core = springjustment_regional_core_one_based(SpringjustmentRegionalCoreInput {
        triangle_lonlat: &triangle_lonlat,
        cell_lonlat: &cell_lonlat,
        cells_on_triangle: &cells_on_triangle,
        triangles_on_cell: &triangles_on_cell,
        n_edges_on_cell: &n_edges_on_cell,
        move_mask: options.move_mask,
        niter_refine: options.niter_refine,
        radius: options.radius,
        diagnostic_every: options.diagnostic_every,
    })
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "failed to run Springjustment_regional_step core from unstructured mesh",
        )
    })?;

    let mesh = unstructured_mesh_from_springjustment_regional(mesh, &core)?;
    Ok(SpringjustmentRegionalGridfileReport { core, mesh })
}

pub fn write_springjustment_regional_gridfile(
    output: impl AsRef<Path>,
    report: &SpringjustmentRegionalGridfileReport,
) -> io::Result<UnstructuredMeshWriteReport> {
    write_unstructured_mesh_netcdf(output, &report.mesh)
}
