use crate::spring_global_debug;

pub(crate) fn boundary_cells_from_triangle_flags(
    num_center_in: usize,
    triangles_on_cell: &[Vec<usize>],
    triangle_flags: &[bool],
) -> Option<Vec<bool>> {
    let mut boundary = vec![false; triangles_on_cell.len()];
    for cell_id in (num_center_in + 1)..triangles_on_cell.len() {
        let triangles = &triangles_on_cell[cell_id];
        if triangles.is_empty() {
            continue;
        }
        let mut flagged = 0usize;
        let mut active_triangles = 0usize;
        for &triangle_id in triangles {
            if triangle_id <= 1 {
                continue;
            }
            active_triangles += 1;
            if *triangle_flags.get(triangle_id)? {
                flagged += 1;
            }
        }
        boundary[cell_id] = flagged != 0 && flagged != active_triangles;
    }
    Some(boundary)
}

/// Port of the edge-length update rule in
/// `MOD_grid_preprocess:distsOnEdge_layers_make`.
///
/// The arrays preserve current Canonical indexing: slots `0` and `1` are
/// placeholders, triangle ids and edge ids are used directly, and the caller
/// provides `num_vertex_in`/`num_center_in` from `num_mp_step(iter)` and
/// `num_wp_step(iter)`.
pub fn dists_on_edge_layers_one_based(
    num_vertex_in: usize,
    num_center_in: usize,
    num_rc: usize,
    dist_len: usize,
    triangles_on_cell: &[Vec<usize>],
    edges_on_vertex: &[[usize; 3]],
    cells_on_edge: &[[usize; 2]],
    dist_layers: &[f64],
    refinement_flags: &[bool],
    initial_dists_on_edge: &[f64],
) -> Option<Vec<f64>> {
    if dist_len == 0
        || dist_layers.len() < 2 * dist_len
        || refinement_flags.len() > edges_on_vertex.len()
        || initial_dists_on_edge.len() > cells_on_edge.len()
    {
        return None;
    }

    let mut triangle_flags = refinement_flags.to_vec();
    let mut triangle_in = vec![false; triangle_flags.len()];
    let mut dists_on_edge = initial_dists_on_edge.to_vec();
    let mut edge_moved = vec![false; initial_dists_on_edge.len()];
    let mindist00 = dist_layers[2 * dist_len - 1] / 2.0;

    for _ in 0..num_rc {
        let boundary =
            boundary_cells_from_triangle_flags(num_center_in, triangles_on_cell, &triangle_flags)?;
        for cell_id in (num_center_in + 1)..triangles_on_cell.len() {
            if !boundary[cell_id] {
                continue;
            }
            for &triangle_id in &triangles_on_cell[cell_id] {
                if *triangle_flags.get(triangle_id)? {
                    triangle_flags[triangle_id] = false;
                }
            }
        }
    }
    spring_global_debug(&format!(
        "dists layers after_rc active_after_vertex={}",
        triangle_flags
            .iter()
            .enumerate()
            .filter(|(idx, flag)| **flag && *idx > num_vertex_in)
            .count()
    ));

    let mut direct_candidate_edges = 0usize;
    for triangle_id in (num_vertex_in + 1)..triangle_flags.len() {
        if !triangle_flags[triangle_id] {
            continue;
        }
        for &edge_id in edges_on_vertex.get(triangle_id)? {
            if edge_id == 0 {
                continue;
            }
            direct_candidate_edges += 1;
            *dists_on_edge.get_mut(edge_id)? = mindist00;
            *edge_moved.get_mut(edge_id)? = true;
        }
    }
    spring_global_debug(&format!(
        "dists layers direct_candidate_edges={direct_candidate_edges}"
    ));

    for layer_id in 0..=dist_len {
        let boundary =
            boundary_cells_from_triangle_flags(num_center_in, triangles_on_cell, &triangle_flags)?;
        if layer_id == dist_len {
            break;
        }

        for cell_id in (num_center_in + 1)..triangles_on_cell.len() {
            if !boundary[cell_id] {
                continue;
            }
            for &triangle_id in &triangles_on_cell[cell_id] {
                if *triangle_flags.get(triangle_id)? {
                    continue;
                }
                *triangle_flags.get_mut(triangle_id)? = true;
                *triangle_in.get_mut(triangle_id)? = true;
            }
        }

        for triangle_id in 2..triangle_in.len() {
            if !triangle_in[triangle_id] {
                continue;
            }
            for &edge_id in edges_on_vertex.get(triangle_id)? {
                if edge_id == 0 || *edge_moved.get(edge_id)? {
                    continue;
                }
                let cells = *cells_on_edge.get(edge_id)?;
                let boundary_sum =
                    usize::from(*boundary.get(cells[0])?) + usize::from(*boundary.get(cells[1])?);
                let layer_index = if boundary_sum == 1 {
                    2 * layer_id
                } else {
                    2 * layer_id + 1
                };
                *dists_on_edge.get_mut(edge_id)? = *dist_layers.get(layer_index)?;
                *edge_moved.get_mut(edge_id)? = true;
            }
        }
        triangle_in.fill(false);
    }

    Some(dists_on_edge)
}
