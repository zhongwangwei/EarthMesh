use crate::cells_on_triangle_one_based_from_mesh;
use crate::lonlat_degrees_from_points;
use crate::n_edges_on_cell_usize_from_mesh;
use crate::read_unstructured_mesh_netcdf;
use crate::triangles_on_cell_one_based_from_mesh;
use crate::UnstructuredMesh;
use earthmesh_mesh::{
    get_area_production_one_based, get_edge_production_one_based, lonlat_points_to_unit_xyz,
    triangle_neighbors_from_cell_membership_one_based, GetAreaProductionOutput, GetAreaUnitInput,
    GetEdgeProductionOutput,
};
use std::io;
use std::path::Path;

/// Read an `Unstructured_Mesh_Read` gridfile and run the current
/// `MOD_grid_preprocess.F90:GetEdge` production adapter.
pub fn get_edge_from_unstructured_gridfile(
    gridfile: impl AsRef<Path>,
) -> io::Result<GetEdgeProductionOutput> {
    let mesh = read_unstructured_mesh_netcdf(gridfile)?;
    get_edge_from_unstructured_mesh(&mesh)
}

/// Run the current `GetEdge` production adapter from a Rust unstructured mesh.
pub fn get_edge_from_unstructured_mesh(
    mesh: &UnstructuredMesh,
) -> io::Result<GetEdgeProductionOutput> {
    let cells_on_triangle = cells_on_triangle_one_based_from_mesh(mesh)?;
    let triangles_on_cell = triangles_on_cell_one_based_from_mesh(mesh)?;
    let n_edges_on_cell = n_edges_on_cell_usize_from_mesh(mesh)?;
    let triangle_neighbors = triangle_neighbors_from_cell_membership_one_based(
        &cells_on_triangle,
        &triangles_on_cell,
        &n_edges_on_cell,
    )
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "failed to derive triangle neighbors for GetEdge adapter",
        )
    })?;
    let triangle_lonlat = lonlat_degrees_from_points(&mesh.m_points);
    let cell_lonlat = lonlat_degrees_from_points(&mesh.w_points);
    get_edge_production_one_based(
        &triangle_neighbors,
        &cells_on_triangle,
        &triangle_lonlat,
        &cell_lonlat,
    )
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "failed to run GetEdge production adapter from unstructured mesh",
        )
    })
}

/// Read an `Unstructured_Mesh_Read` gridfile and run the current
/// `MOD_grid_preprocess.F90:GetArea` production adapter.
pub fn get_area_from_unstructured_gridfile(
    gridfile: impl AsRef<Path>,
) -> io::Result<GetAreaProductionOutput> {
    let mesh = read_unstructured_mesh_netcdf(gridfile)?;
    get_area_from_unstructured_mesh(&mesh)
}

/// Run the current `GetArea` production adapter from a Rust unstructured mesh.
pub fn get_area_from_unstructured_mesh(
    mesh: &UnstructuredMesh,
) -> io::Result<GetAreaProductionOutput> {
    let edge_output = get_edge_from_unstructured_mesh(mesh)?;
    let cells_on_vertex = cells_on_triangle_one_based_from_mesh(mesh)?;
    let triangle_lonlat = lonlat_degrees_from_points(&mesh.m_points);
    let cell_lonlat = lonlat_degrees_from_points(&mesh.w_points);
    let edge_lonlat = edge_output.edge_points.clone();
    let vertices = lonlat_points_to_unit_xyz(&triangle_lonlat);
    let cell_points = lonlat_points_to_unit_xyz(&cell_lonlat);
    let edge_points = lonlat_points_to_unit_xyz(&edge_lonlat);
    let vertices_on_cell = triangles_on_cell_one_based_from_mesh(mesh)?;
    let n_edges_on_cell = n_edges_on_cell_usize_from_mesh(mesh)?;

    get_area_production_one_based(GetAreaUnitInput {
        vertices: &vertices,
        edge_points: &edge_points,
        cell_points: &cell_points,
        cells_on_vertex: &cells_on_vertex,
        edges_on_vertex: &edge_output.edges_on_vertex,
        cells_on_edge: &edge_output.cells_on_edge,
        vertices_on_cell: &vertices_on_cell,
        n_edges_on_cell: &n_edges_on_cell,
    })
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "failed to run GetArea production adapter from unstructured mesh",
        )
    })
}
