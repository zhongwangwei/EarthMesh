/// Output of `MOD_grid_preprocess:set_weightsOnEdge`.
#[derive(Debug, Clone, PartialEq)]
pub struct WeightsOnEdgeOutput {
    pub weights_on_edge: Vec<Vec<f64>>,
    pub edges_on_edge: Vec<Vec<usize>>,
    pub n_edges_on_edge: Vec<usize>,
    pub error_segment: Vec<f64>,
}

fn find_index_in_prefix(index: usize, indices: &[usize], n_indices: usize) -> Option<usize> {
    indices
        .iter()
        .take(n_indices)
        .position(|candidate| *candidate == index)
}

/// Port of `MOD_grid_preprocess:set_weightsOnEdge`.
///
/// The routine computes MPAS-compatible edge stencils and reconstruction
/// weights for Canonical-indexed mesh arrays. Weight rows are stored compactly per
/// edge rather than in a fixed `maxEdges2 x num_edge` matrix.
pub fn set_weights_on_edge_one_based(
    area_cell: &[f64],
    angle_edge: &[f64],
    dc_edge: &[f64],
    dv_edge: &[f64],
    kite_areas_on_vertex: &[[f64; 3]],
    edges_on_cell: &[Vec<usize>],
    cells_on_vertex: &[[usize; 3]],
    cells_on_edge: &[[usize; 2]],
    vertices_on_cell: &[Vec<usize>],
    vertices_on_edge: &[[usize; 2]],
    n_edges_on_cell: &[usize],
) -> Option<WeightsOnEdgeOutput> {
    let num_edges = cells_on_edge.len();
    if vertices_on_edge.len() != num_edges
        || angle_edge.len() < num_edges
        || dc_edge.len() < num_edges
        || dv_edge.len() < num_edges
    {
        return None;
    }

    let mut weights_on_edge = vec![Vec::new(); num_edges];
    let mut edges_on_edge = vec![Vec::new(); num_edges];
    let mut n_edges_on_edge = vec![0usize; num_edges];
    let mut error_segment = vec![0.0; num_edges];

    for edge_id in 2..num_edges {
        let [cell1, cell2] = cells_on_edge[edge_id];
        let edge_vertices = vertices_on_edge[edge_id];
        if cell1 == 0
            || cell2 == 0
            || edge_vertices[0] == 0
            || edge_vertices[1] == 0
            || cell1 >= n_edges_on_cell.len()
            || cell2 >= n_edges_on_cell.len()
        {
            continue;
        }
        let mut nw1 = 0usize;

        for side in 0..2 {
            let (cell_id, vertex_start, tev2) = if side == 0 {
                (cell1, vertices_on_edge[edge_id][1], -1.0)
            } else {
                (cell2, vertices_on_edge[edge_id][0], 1.0)
            };
            let ne = *n_edges_on_cell.get(cell_id)?;
            if ne == 0
                || vertices_on_cell.get(cell_id)?.len() < ne
                || edges_on_cell.get(cell_id)?.len() < ne
            {
                return None;
            }
            let area = *area_cell.get(cell_id)?;
            if area == 0.0 {
                return None;
            }

            let mut riv_cell = Vec::with_capacity(ne);
            for vertex_id in vertices_on_cell[cell_id].iter().copied().take(ne) {
                let cells_for_vertex = *cells_on_vertex.get(vertex_id)?;
                let kite_slot = cells_for_vertex
                    .iter()
                    .position(|candidate| *candidate == cell_id)?;
                riv_cell.push(kite_areas_on_vertex.get(vertex_id)?[kite_slot] / area);
            }

            let vertex_index = find_index_in_prefix(vertex_start, &vertices_on_cell[cell_id], ne)?;
            let mut riv_wrap = riv_cell.clone();
            riv_wrap.extend_from_slice(&riv_cell);

            for wrapped_index in vertex_index..=(vertex_index + ne - 2) {
                let mut kahan_sum = 0.0;
                let mut kahan_c = 0.0;
                for value in &riv_wrap[vertex_index..=wrapped_index] {
                    let kahan_y = *value - kahan_c;
                    let kahan_t = kahan_sum + kahan_y;
                    kahan_c = (kahan_t - kahan_sum) - kahan_y;
                    kahan_sum = kahan_t;
                }
                weights_on_edge[edge_id].push((kahan_sum - 0.5) * tev2);
            }

            let edge_index_cell = find_index_in_prefix(edge_id, &edges_on_cell[cell_id], ne)?;
            let mut edge_index = edges_on_cell[cell_id][0..ne].to_vec();
            edge_index.extend_from_within(0..ne);
            for local_edge_slot in 0..(ne - 1) {
                let output_slot = nw1 + local_edge_slot;
                let contributing_edge_id = edge_index[edge_index_cell + local_edge_slot + 1];
                edges_on_edge[edge_id].push(contributing_edge_id);
                let factor = *dv_edge.get(contributing_edge_id)? / *dc_edge.get(edge_id)?;
                let mut weight = *weights_on_edge[edge_id].get(output_slot)? * factor;
                if cells_on_edge.get(contributing_edge_id)?[1] == cell_id {
                    weight = -weight;
                }
                weights_on_edge[edge_id][output_slot] = weight;
            }

            nw1 = ne - 1;
            n_edges_on_edge[edge_id] += nw1;
        }
    }

    for edge_id in 2..num_edges {
        let mut v_edge = 0.0;
        for (contributing_edge_id, weight) in edges_on_edge[edge_id]
            .iter()
            .copied()
            .zip(weights_on_edge[edge_id].iter().copied())
        {
            v_edge += angle_edge.get(contributing_edge_id)?.cos() * weight;
        }
        let ve = -angle_edge[edge_id].sin();
        error_segment[edge_id] = (v_edge - ve).abs();
    }

    Some(WeightsOnEdgeOutput {
        weights_on_edge,
        edges_on_edge,
        n_edges_on_edge,
        error_segment,
    })
}
