use super::*;

/// Single-vertex output from `orderVertexArrays`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderedVertexArrays {
    pub edges_on_vertex: [usize; 3],
    pub cells_on_vertex: [usize; 3],
}

/// Array-level output from the Fortran-indexed `orderVertexArrays` port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedVertexArraysOutput {
    pub edges_on_vertex: Vec<[usize; 3]>,
    pub cells_on_vertex: Vec<[usize; 3]>,
}

/// Port of the per-vertex mutation/rebuild workflow in `MOD_grid_preprocess:orderVertexArrays`.
///
/// This preserves the Fortran algorithm: mutate `edgesOnVertex` by repeatedly
/// swapping the next smallest positive-CCW edge into the following slot, then
/// rebuild `cellsOnVertex` from `verticesOnEdge` and `cellsOnEdge`.
pub fn order_vertex_arrays_for_vertex(
    vertex_id: usize,
    vertex: CartesianPoint,
    edges_on_vertex: [usize; 3],
    edge_points: &[CartesianPoint],
    vertices_on_edge: &[[usize; 2]],
    cells_on_edge: &[[usize; 2]],
) -> Option<OrderedVertexArrays> {
    let mut ordered_edges = edges_on_vertex;

    for j in 0..3 {
        let edge1 = ordered_edges[j];
        if edge1 == 0 {
            continue;
        }
        let reference_edge = *edge_points.get(edge1)?;
        let candidate_slots = ((j + 1)..3)
            .filter(|slot| ordered_edges[*slot] > 0)
            .collect::<Vec<_>>();
        let candidate_points = candidate_slots
            .iter()
            .map(|slot| edge_points.get(ordered_edges[*slot]).copied())
            .collect::<Option<Vec<_>>>()?;
        let Some(relative_slot) =
            next_ccw_edge_candidate_slot(vertex, reference_edge, &candidate_points)
        else {
            continue;
        };
        let swap_slot = candidate_slots[relative_slot];
        if swap_slot != j + 1 {
            ordered_edges.swap(j + 1, swap_slot);
        }
    }

    let mut ordered_cells = [0usize; 3];
    for j in 0..3 {
        let edge = ordered_edges[j];
        if edge == 0 {
            continue;
        }
        let vertices = *vertices_on_edge.get(edge)?;
        let cells = *cells_on_edge.get(edge)?;
        ordered_cells[j] = if vertex_id == vertices[0] {
            cells[0]
        } else {
            cells[1]
        };
    }

    Some(OrderedVertexArrays {
        edges_on_vertex: ordered_edges,
        cells_on_vertex: ordered_cells,
    })
}

/// Fortran-indexed array wrapper for `MOD_grid_preprocess:orderVertexArrays`.
///
/// Indices `0` and `1` are preserved/skipped so existing Fortran-style ids can
/// be used directly while the rest of the mesh workflow is migrated.
pub fn order_vertex_arrays_fortran_indexed(
    vertex_points: &[CartesianPoint],
    edge_points: &[CartesianPoint],
    edges_on_vertex: &[[usize; 3]],
    vertices_on_edge: &[[usize; 2]],
    cells_on_edge: &[[usize; 2]],
) -> Option<OrderedVertexArraysOutput> {
    if edges_on_vertex.len() < vertex_points.len() {
        return None;
    }

    let mut ordered_edges = edges_on_vertex.to_vec();
    let mut ordered_cells = vec![[0usize; 3]; vertex_points.len()];

    for vertex_id in 2..vertex_points.len() {
        let ordered = order_vertex_arrays_for_vertex(
            vertex_id,
            vertex_points[vertex_id],
            ordered_edges[vertex_id],
            edge_points,
            vertices_on_edge,
            cells_on_edge,
        )?;
        ordered_edges[vertex_id] = ordered.edges_on_vertex;
        ordered_cells[vertex_id] = ordered.cells_on_vertex;
    }

    Some(OrderedVertexArraysOutput {
        edges_on_vertex: ordered_edges,
        cells_on_vertex: ordered_cells,
    })
}
