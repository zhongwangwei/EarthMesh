use super::*;

/// Production-facing `GetEdge` output after the same post-processing sequence
/// used by the global mesh workflow.
#[derive(Debug, Clone, PartialEq)]
pub struct GetEdgeProductionOutput {
    pub cells_on_edge: Vec<[usize; 2]>,
    pub vertices_on_edge: Vec<[usize; 2]>,
    pub edges_on_vertex: Vec<[usize; 3]>,
    pub cells_on_vertex: Vec<[usize; 3]>,
    pub edge_points: Vec<LonLatDegrees>,
}

/// Production wrapper for `MOD_grid_preprocess:GetEdge` plus the immediate
/// post-processing used before MPAS-style mesh outputs are consumed.
///
/// The sequence matches the migrated workflow surfaces:
/// `GetEdge`, `GetSort_verticesOnEdge`, optional `vp` midpoint generation, and
/// `orderVertexArrays`.
pub fn get_edge_production_fortran_indexed(
    triangle_neighbors: &[[usize; 3]],
    cells_on_vertex: &[[usize; 3]],
    triangle_lonlat: &[LonLatDegrees],
    cell_lonlat: &[LonLatDegrees],
) -> Option<GetEdgeProductionOutput> {
    let connectivity = get_edge_connectivity_fortran_indexed(triangle_neighbors, cells_on_vertex)?;
    let vertices_on_edge = order_vertices_on_edge_fortran_indexed(
        triangle_lonlat,
        cell_lonlat,
        &connectivity.cells_on_edge,
        &connectivity.vertices_on_edge,
    )?;
    let edge_points =
        edge_midpoints_from_cells_fortran_indexed(&connectivity.cells_on_edge, cell_lonlat)?;
    let triangle_points = triangle_lonlat
        .iter()
        .copied()
        .map(lonlat_degrees_to_unit_xyz)
        .collect::<Vec<_>>();
    let edge_points_cartesian = edge_points
        .iter()
        .copied()
        .map(lonlat_degrees_to_unit_xyz)
        .collect::<Vec<_>>();
    let ordered_vertex_arrays = order_vertex_arrays_fortran_indexed(
        &triangle_points,
        &edge_points_cartesian,
        &connectivity.edges_on_vertex,
        &vertices_on_edge,
        &connectivity.cells_on_edge,
    )?;

    Some(GetEdgeProductionOutput {
        cells_on_edge: connectivity.cells_on_edge,
        vertices_on_edge,
        edges_on_vertex: ordered_vertex_arrays.edges_on_vertex,
        cells_on_vertex: ordered_vertex_arrays.cells_on_vertex,
        edge_points,
    })
}

/// Port of the optional `vp` midpoint calculation in `MOD_grid_preprocess:GetEdge`.
///
/// For each Fortran-indexed edge id from `2..`, the edge point is the spherical
/// centroid of the two neighboring polygon cell centers `wp(cellsOnEdge(:, k), :)`.
pub fn edge_midpoints_from_cells_fortran_indexed(
    cells_on_edge: &[[usize; 2]],
    cell_lonlat: &[LonLatDegrees],
) -> Option<Vec<LonLatDegrees>> {
    let mut midpoints = vec![LonLatDegrees::new(0.0, 0.0); cells_on_edge.len()];
    for edge_id in 2..cells_on_edge.len() {
        let cells = cells_on_edge[edge_id];
        let cell1 = *cell_lonlat.get(cells[0])?;
        let cell2 = *cell_lonlat.get(cells[1])?;
        midpoints[edge_id] = spherical_centroid_degrees(&[cell1, cell2])?;
    }
    Some(midpoints)
}
