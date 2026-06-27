/// Port of `MOD_grid_preprocess:set_edgesOnEdge_tri`.
///
/// For each edge, returns the two cyclic neighboring edges at the first
/// endpoint followed by the two cyclic neighboring edges at the second endpoint.
/// Indices preserve the Fortran convention that edge ids start at `2`.
pub fn edges_on_edge_tri_fortran_indexed(
    vertices_on_edge: &[[usize; 2]],
    edges_on_vertex: &[[usize; 3]],
) -> Option<Vec<[usize; 4]>> {
    let mut edges_on_edge = vec![[0usize; 4]; vertices_on_edge.len()];

    for edge_id in 2..vertices_on_edge.len() {
        let vertices = vertices_on_edge[edge_id];
        for (endpoint_slot, vertex_id) in vertices.iter().copied().enumerate() {
            let vertex_edges = *edges_on_vertex.get(vertex_id)?;
            let edge_slot = vertex_edges
                .iter()
                .position(|candidate_edge| *candidate_edge == edge_id)?;
            let adjacent_slots = match edge_slot {
                0 => [1, 2],
                1 => [2, 0],
                2 => [0, 1],
                _ => return None,
            };
            edges_on_edge[edge_id][endpoint_slot * 2] = vertex_edges[adjacent_slots[0]];
            edges_on_edge[edge_id][endpoint_slot * 2 + 1] = vertex_edges[adjacent_slots[1]];
        }
    }

    Some(edges_on_edge)
}
