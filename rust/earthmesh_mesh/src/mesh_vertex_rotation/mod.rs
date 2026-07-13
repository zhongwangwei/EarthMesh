/// Port of one-vertex rotation logic from `MOD_grid_preprocess:normalizeRotation`.
///
/// The minimum positive cell id is rotated into slot 0, and the edge slots are
/// rotated in lockstep. If no positive cell id exists, arrays are unchanged.
pub fn normalize_vertex_rotation(
    cells_on_vertex: [usize; 3],
    edges_on_vertex: [usize; 3],
) -> ([usize; 3], [usize; 3]) {
    let mut min_cell = cells_on_vertex[0];
    let mut min_pos = 0usize;

    for pos in 1..3 {
        let cell = cells_on_vertex[pos];
        if cell > 0 && (min_cell == 0 || cell < min_cell) {
            min_cell = cell;
            min_pos = pos;
        }
    }

    if min_pos == 1 && min_cell > 0 {
        (
            [cells_on_vertex[1], cells_on_vertex[2], cells_on_vertex[0]],
            [edges_on_vertex[1], edges_on_vertex[2], edges_on_vertex[0]],
        )
    } else if min_pos == 2 && min_cell > 0 {
        (
            [cells_on_vertex[2], cells_on_vertex[0], cells_on_vertex[1]],
            [edges_on_vertex[2], edges_on_vertex[0], edges_on_vertex[1]],
        )
    } else {
        (cells_on_vertex, edges_on_vertex)
    }
}

/// Port of `MOD_grid_preprocess:standardizeVerticesOnCellRotation`.
///
/// Cell ids preserve the current Canonical indexing convention: slot `1` is
/// skipped and valid cells are visited from id `2`. Only the first
/// `n_edges_on_cell[cell_id]` entries are rotated; any storage tail is kept in
/// place, matching Canonical's fixed-width `verticesOnCell(:, i)` arrays.
pub fn standardize_vertices_on_cell_rotation_one_based(
    vertices_on_cell: &[Vec<usize>],
    n_edges_on_cell: &[usize],
) -> Option<Vec<Vec<usize>>> {
    if n_edges_on_cell.len() < vertices_on_cell.len() {
        return None;
    }

    let mut standardized = vertices_on_cell.to_vec();
    for cell_id in 2..vertices_on_cell.len() {
        let ne = n_edges_on_cell[cell_id];
        if ne == 0 {
            continue;
        }
        if standardized[cell_id].len() < ne {
            return None;
        }

        let mut min_vertex_id = usize::MAX;
        let mut min_pos = 0usize;
        for pos in 0..ne {
            let vertex_id = standardized[cell_id][pos];
            if vertex_id > 0 && vertex_id < min_vertex_id {
                min_vertex_id = vertex_id;
                min_pos = pos;
            }
        }

        if min_vertex_id != usize::MAX && min_pos != 0 {
            let current = standardized[cell_id][0..ne].to_vec();
            let rotated = current[min_pos..]
                .iter()
                .chain(current[..min_pos].iter())
                .copied()
                .collect::<Vec<_>>();
            standardized[cell_id][0..ne].copy_from_slice(&rotated);
        }
    }

    Some(standardized)
}
