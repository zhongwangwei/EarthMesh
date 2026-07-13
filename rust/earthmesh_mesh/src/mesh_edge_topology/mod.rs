use crate::cells_on_edge_from_neighbor_cells;

/// Port of the shared-cell lookup in `MOD_grid_preprocess:GetArea`.
///
/// Canonical checks all four combinations from `cellsOnEdge(:, edge1)` and
/// `cellsOnEdge(:, edge2)` and keeps the maximum matching positive cell id.
/// Zero is the no-cell sentinel and is returned as `None`.
pub fn shared_cell_for_edge_pair(
    edge1_cells: [usize; 2],
    edge2_cells: [usize; 2],
) -> Option<usize> {
    let mut shared_cell = 0usize;
    for cell1 in edge1_cells {
        for cell2 in edge2_cells {
            if cell1 == cell2 {
                shared_cell = shared_cell.max(cell1);
            }
        }
    }

    (shared_cell > 0).then_some(shared_cell)
}

/// Port of the `cellsOnVertex(:, i)` scan in `MOD_grid_preprocess:GetArea`.
///
/// Returns a zero-based Rust index for the matching Canonical `icv` slot.
pub fn vertex_cell_position(cells_on_vertex: [usize; 3], cell: usize) -> Option<usize> {
    cells_on_vertex
        .iter()
        .position(|candidate| *candidate == cell)
}

/// Output from the core connectivity part of `MOD_grid_preprocess:GetEdge`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetEdgeConnectivity {
    pub cells_on_edge: Vec<[usize; 2]>,
    pub vertices_on_edge: Vec<[usize; 2]>,
    pub edges_on_vertex: Vec<[usize; 3]>,
}

/// Port of the core connectivity loop in `MOD_grid_preprocess:GetEdge`.
///
/// The optional midpoint calculation is intentionally separate; this function
/// ports edge-id creation/reuse, `verticesOnEdge`, `cellsOnEdge`, and
/// `edgesOnVertex` for Canonical-indexed arrays.
pub fn get_edge_connectivity_one_based(
    triangle_neighbors: &[[usize; 3]],
    cells_on_vertex: &[[usize; 3]],
) -> Option<GetEdgeConnectivity> {
    if cells_on_vertex.len() != triangle_neighbors.len() || triangle_neighbors.len() < 2 {
        return None;
    }

    let mut edges_on_vertex = vec![[0usize; 3]; triangle_neighbors.len()];
    let mut cells_on_edge = vec![[0usize; 2]; 2];
    let mut vertices_on_edge = vec![[0usize; 2]; 2];
    let mut triangle_used = vec![false; triangle_neighbors.len()];
    let mut edge_id = 1usize;

    for triangle_id in 2..triangle_neighbors.len() {
        for neighbor_slot in 0..3 {
            let neighbor_id = triangle_neighbors[triangle_id][neighbor_slot];
            if neighbor_id == 0 {
                continue;
            }
            if neighbor_id >= triangle_neighbors.len() {
                return None;
            }

            if triangle_used[neighbor_id] {
                let reuse_slot = triangle_neighbors[neighbor_id]
                    .iter()
                    .position(|candidate| *candidate == triangle_id)?;
                edges_on_vertex[triangle_id][neighbor_slot] =
                    edges_on_vertex[neighbor_id][reuse_slot];
                continue;
            }

            edge_id += 1;
            if cells_on_edge.len() <= edge_id {
                cells_on_edge.resize(edge_id + 1, [0usize; 2]);
                vertices_on_edge.resize(edge_id + 1, [0usize; 2]);
            }

            edges_on_vertex[triangle_id][neighbor_slot] = edge_id;
            vertices_on_edge[edge_id] = [triangle_id, neighbor_id];
            cells_on_edge[edge_id] = cells_on_edge_from_neighbor_cells(
                cells_on_vertex[triangle_id],
                cells_on_vertex[neighbor_id],
            )?;
        }
        triangle_used[triangle_id] = true;
    }

    Some(GetEdgeConnectivity {
        cells_on_edge,
        vertices_on_edge,
        edges_on_vertex,
    })
}
