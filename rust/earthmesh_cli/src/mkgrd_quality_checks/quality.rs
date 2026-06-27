use std::io;

use crate::*;

pub(super) fn grid_quality_global_from_unstructured_mesh(
    mesh: &UnstructuredMesh,
) -> io::Result<earthmesh_mesh::GridQualityGlobalOutput> {
    let cells_on_triangle = cells_on_triangle_fortran_indexed_from_mesh(mesh)?;
    let triangles_on_cell = triangles_on_cell_fortran_indexed_from_mesh(mesh)?;
    let n_edges_on_cell = n_edges_on_cell_usize_from_mesh(mesh)?;
    let triangle_lonlat = lonlat_degrees_from_points(&mesh.m_points);
    let cell_lonlat = lonlat_degrees_from_points(&mesh.w_points);

    earthmesh_mesh::grid_quality_check_global_fortran_indexed(
        &cell_lonlat,
        &cells_on_triangle,
        &triangle_lonlat,
        &triangles_on_cell,
        &n_edges_on_cell,
    )
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "failed to run Grid_Quality_Check_Global from unstructured mesh",
        )
    })
}
