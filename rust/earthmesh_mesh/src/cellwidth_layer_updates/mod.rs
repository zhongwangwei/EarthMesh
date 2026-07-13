use crate::boundary_cells_from_triangle_flags;

/// Port of the cell-width update rule in
/// `MOD_grid_preprocess:cellwidth_layers_make`.
///
/// `cells_on_triangle` corresponds to Canonical `ngrmw(:, i)` for triangle `i`,
/// while `triangles_on_cell` corresponds to `ngrwm(:, k)` for cell `k`.
pub fn cellwidth_layers_one_based(
    num_vertex_in: usize,
    num_center_in: usize,
    num_rc: usize,
    dist_len: usize,
    cells_on_triangle: &[[usize; 3]],
    triangles_on_cell: &[Vec<usize>],
    dist_layers: &[f64],
    refinement_flags: &[bool],
    initial_cellwidth: &[f64],
) -> Option<Vec<f64>> {
    if dist_len == 0
        || dist_layers.len() < dist_len
        || refinement_flags.len() > cells_on_triangle.len()
        || initial_cellwidth.len() < triangles_on_cell.len()
    {
        return None;
    }

    let mut triangle_flags = refinement_flags.to_vec();
    let mut triangle_in = vec![false; triangle_flags.len()];
    let mut cellwidth = initial_cellwidth.to_vec();

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

    let inner_cellwidth = dist_layers[dist_len - 1] / 2.0;
    for triangle_id in (num_vertex_in + 1)..triangle_flags.len() {
        if !triangle_flags[triangle_id] {
            continue;
        }
        for &cell_id in cells_on_triangle.get(triangle_id)? {
            if cell_id == 0 {
                continue;
            }
            *cellwidth.get_mut(cell_id)? = inner_cellwidth;
        }
    }

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
            for &cell_id in cells_on_triangle.get(triangle_id)? {
                if cell_id == 0 || *boundary.get(cell_id)? {
                    continue;
                }
                *cellwidth.get_mut(cell_id)? = *dist_layers.get(layer_id)?;
            }
        }
        triangle_in.fill(false);
    }

    Some(cellwidth)
}
