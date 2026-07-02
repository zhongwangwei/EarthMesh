/// Port of `MOD_grid_preprocess:IsNgrmm`.
///
/// Returns the one-based Fortran code for the vertex in `a` opposite the shared
/// edge with `b`: `1`, `2`, or `3`. Non-neighbor triangles return `None`
/// instead of Fortran's `0` sentinel.
pub fn is_ngrmm(a: [usize; 3], b: [usize; 3]) -> Option<usize> {
    if b.contains(&a[0]) {
        if b.contains(&a[1]) {
            Some(3)
        } else if b.contains(&a[2]) {
            Some(2)
        } else {
            None
        }
    } else if b.contains(&a[1]) && b.contains(&a[2]) {
        Some(1)
    } else {
        None
    }
}

/// Port of the `GetEdge` `cellsOnEdge(:, k)` mapping after `IsNgrmm`.
///
/// The two shared polygon-cell ids are selected from `a` according to the
/// Fortran opposite-vertex code and sorted ascending before return.
pub fn cells_on_edge_from_neighbor_cells(a: [usize; 3], b: [usize; 3]) -> Option<[usize; 2]> {
    let mut cells = match is_ngrmm(a, b)? {
        1 => [a[1], a[2]],
        2 => [a[2], a[0]],
        3 => [a[0], a[1]],
        _ => return None,
    };
    if cells[0] > cells[1] {
        cells.swap(0, 1);
    }
    Some(cells)
}

/// Port of `MOD_grid_preprocess:set_ngrmm`.
///
/// Builds triangle-neighbor slots from triangle-to-cell membership
/// (`cells_on_triangle`) and the inverse cell-to-triangle membership
/// (`triangles_on_cell`). Slots preserve the Fortran `IsNgrmm` meaning:
/// neighbor slot `0`, `1`, or `2` is opposite the corresponding triangle cell.
pub fn triangle_neighbors_from_cell_membership_fortran_indexed(
    cells_on_triangle: &[[usize; 3]],
    triangles_on_cell: &[Vec<usize>],
    triangle_counts_on_cell: &[usize],
) -> Option<Vec<[usize; 3]>> {
    if triangles_on_cell.len() != triangle_counts_on_cell.len() {
        return None;
    }

    let mut triangle_neighbors = vec![[0usize; 3]; cells_on_triangle.len()];
    for triangle_id in 2..cells_on_triangle.len() {
        let mut neighbor_count = 0usize;
        for &cell_id in &cells_on_triangle[triangle_id] {
            if cell_id == 0 {
                continue;
            }
            let count = *triangle_counts_on_cell.get(cell_id)?;
            let cell_triangles = triangles_on_cell.get(cell_id)?;
            if count > cell_triangles.len() {
                return None;
            }
            if neighbor_count == 3 {
                break;
            }
            for &candidate_triangle_id in cell_triangles.iter().take(count) {
                // Stop as soon as all three neighbor slots are filled; checked
                // here (not only in the outer loop) so a spurious later match
                // on malformed input cannot overwrite an already-filled slot.
                if neighbor_count == 3 {
                    break;
                }
                if candidate_triangle_id == 0 || candidate_triangle_id == triangle_id {
                    continue;
                }
                let candidate_cells = *cells_on_triangle.get(candidate_triangle_id)?;
                let Some(opposite_slot) = is_ngrmm(cells_on_triangle[triangle_id], candidate_cells)
                else {
                    continue;
                };
                // Count distinct slot fills, not raw writes: on a valid mesh
                // each neighbor is rediscovered via its second shared cell, and
                // counting rewrites made the "== 3" early exit unreachable.
                if triangle_neighbors[triangle_id][opposite_slot - 1] == 0 {
                    neighbor_count += 1;
                }
                triangle_neighbors[triangle_id][opposite_slot - 1] = candidate_triangle_id;
            }
        }
    }

    Some(triangle_neighbors)
}
