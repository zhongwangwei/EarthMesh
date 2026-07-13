use crate::LonLatDegrees;

/// Output of `MOD_grid_preprocess:edgeIDSort`.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeIdSortOutput {
    pub cells_on_edge: Vec<[usize; 2]>,
    pub vertices_on_edge: Vec<[usize; 2]>,
    pub edges_on_vertex: Vec<[usize; 3]>,
    pub edge_points: Vec<LonLatDegrees>,
}

/// Port of `MOD_grid_preprocess:edgeIDSort`.
///
/// Edges from the current mesh are reordered to match
/// `cells_on_edge_canonical`; `edges_on_vertex` is then rebuilt from the sorted
/// `vertices_on_edge` arrays.
pub fn edge_id_sort_one_based(
    num_vertices: usize,
    cells_on_edge_canonical: &[[usize; 2]],
    cells_on_edge: &[[usize; 2]],
    vertices_on_edge: &[[usize; 2]],
    edge_points: &[LonLatDegrees],
) -> Option<EdgeIdSortOutput> {
    let num_edges = cells_on_edge_canonical.len();
    if cells_on_edge.len() != num_edges
        || vertices_on_edge.len() != num_edges
        || edge_points.len() != num_edges
    {
        return None;
    }

    let mut sorted_cells_on_edge = vec![[0usize; 2]; num_edges];
    let mut sorted_vertices_on_edge = vec![[0usize; 2]; num_edges];
    let mut sorted_edge_points = vec![LonLatDegrees::new(0.0, 0.0); num_edges];

    for target_edge_id in 2..num_edges {
        let canonical_cells = cells_on_edge_canonical[target_edge_id];
        let source_edge_id = (2..num_edges).find(|&candidate| {
            cells_on_edge[candidate][0] == canonical_cells[0]
                && cells_on_edge[candidate][1] == canonical_cells[1]
        })?;
        sorted_cells_on_edge[target_edge_id] = cells_on_edge[source_edge_id];
        sorted_vertices_on_edge[target_edge_id] = vertices_on_edge[source_edge_id];
        sorted_edge_points[target_edge_id] = edge_points[source_edge_id];
    }

    let mut edges_on_vertex = vec![[0usize; 3]; num_vertices];
    let mut edge_counts = vec![0usize; num_vertices];
    for edge_id in 2..num_edges {
        for &vertex_id in &sorted_vertices_on_edge[edge_id] {
            if vertex_id == 0 {
                continue;
            }
            let count = edge_counts.get_mut(vertex_id)?;
            if *count >= 3 {
                return None;
            }
            edges_on_vertex.get_mut(vertex_id)?[*count] = edge_id;
            *count += 1;
        }
    }

    Some(EdgeIdSortOutput {
        cells_on_edge: sorted_cells_on_edge,
        vertices_on_edge: sorted_vertices_on_edge,
        edges_on_vertex,
        edge_points: sorted_edge_points,
    })
}
