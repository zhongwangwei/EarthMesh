use crate::cells_on_triangle_one_based_from_mesh;
use crate::lonlat_degrees_from_points;
use crate::n_edges_on_cell_usize_from_mesh;
use crate::read_unstructured_mesh_netcdf;
use crate::triangles_on_cell_one_based_from_mesh;
use crate::SpringjustmentGlobalGridfileReport;
use crate::SpringjustmentGlobalRunOptions;
use crate::UnstructuredMesh;
use earthmesh_mesh::springjustment_global_core_one_based;
use earthmesh_mesh::SpringjustmentGlobalCoreInput;
use std::io;
use std::path::Path;

use super::conversion::unstructured_mesh_from_springjustment_global;
use super::persistence::write_springjustment_global_persistence;

pub fn run_springjustment_global_from_unstructured_gridfile(
    gridfile: impl AsRef<Path>,
    file_dir: impl AsRef<Path>,
    nxp: usize,
    step: usize,
    options: SpringjustmentGlobalRunOptions<'_>,
) -> io::Result<SpringjustmentGlobalGridfileReport> {
    let mesh = read_unstructured_mesh_netcdf(gridfile)?;
    run_springjustment_global_from_unstructured_mesh(&mesh, file_dir, nxp, step, options)
}

pub fn run_springjustment_global_from_unstructured_mesh(
    mesh: &UnstructuredMesh,
    file_dir: impl AsRef<Path>,
    nxp: usize,
    step: usize,
    options: SpringjustmentGlobalRunOptions<'_>,
) -> io::Result<SpringjustmentGlobalGridfileReport> {
    let cells_on_triangle = cells_on_triangle_one_based_from_mesh(mesh)?;
    let triangles_on_cell = triangles_on_cell_one_based_from_mesh(mesh)?;
    let n_edges_on_cell = n_edges_on_cell_usize_from_mesh(mesh)?;
    let triangle_lonlat = lonlat_degrees_from_points(&mesh.m_points);
    let cell_lonlat = lonlat_degrees_from_points(&mesh.w_points);

    let core = springjustment_global_core_one_based(SpringjustmentGlobalCoreInput {
        triangle_lonlat: &triangle_lonlat,
        cell_lonlat: &cell_lonlat,
        cells_on_triangle: &cells_on_triangle,
        triangles_on_cell: &triangles_on_cell,
        n_edges_on_cell: &n_edges_on_cell,
        base_dists_on_edge: options.base_dists_on_edge,
        base_cellwidth: options.base_cellwidth,
        distance_num_rc: options.distance_num_rc,
        distance_spacing: options.distance_spacing,
        distance_steps: options.distance_steps,
        niter_refine: options.niter_refine,
        relax: options.relax,
        radius: options.radius,
        diagnostic_every: options.diagnostic_every,
    })
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "failed to run Springjustment_global core from unstructured mesh",
        )
    })?;

    let persistence =
        write_springjustment_global_persistence(file_dir, nxp, step, &cell_lonlat, &core)?;
    let mesh = unstructured_mesh_from_springjustment_global(mesh, &core)?;

    Ok(SpringjustmentGlobalGridfileReport {
        core,
        persistence,
        mesh,
    })
}
