use super::*;

/// Port of `MOD_grid_preprocess:orderVerticesOnCell`.
///
/// Preserves the Canonical selection-sort approach: for each fixed vertex slot,
/// choose the remaining vertex with positive `cross(vec1, vec2) . normal` and
/// the smallest angle to the current canonical vector.
pub fn order_vertices_on_cell_one_based(
    cell_points: &[CartesianPoint],
    vertex_points: &[CartesianPoint],
    vertices_on_cell: &[Vec<usize>],
    n_edges_on_cell: &[usize],
) -> Option<Vec<Vec<usize>>> {
    if n_edges_on_cell.len() < vertices_on_cell.len() || cell_points.len() < vertices_on_cell.len()
    {
        return None;
    }

    let mut ordered = vertices_on_cell.to_vec();
    for cell_id in 2..vertices_on_cell.len() {
        let ne = n_edges_on_cell[cell_id];
        if ne == 0 {
            continue;
        }
        if ordered[cell_id].len() < ne {
            return None;
        }

        let cell_center = *cell_points.get(cell_id)?;
        let normal_mag = magnitude(cell_center);
        if normal_mag == 0.0 {
            return None;
        }
        let normal = CartesianPoint::new(
            cell_center.x / normal_mag,
            cell_center.y / normal_mag,
            cell_center.z / normal_mag,
        );

        for slot in 0..(ne - 1) {
            let vertex1_id = ordered[cell_id][slot];
            if vertex1_id == 0 {
                continue;
            }
            let vertex1 = *vertex_points.get(vertex1_id)?;
            let vec1 = vector_between(cell_center, vertex1);
            let mag1 = magnitude(vec1);
            if mag1 == 0.0 {
                continue;
            }

            let mut min_angle = std::f64::consts::PI * 2.0;
            let mut swap_slot = None;
            for candidate_slot in (slot + 1)..ne {
                let vertex2_id = ordered[cell_id][candidate_slot];
                if vertex2_id == 0 {
                    continue;
                }
                let vertex2 = *vertex_points.get(vertex2_id)?;
                let vec2 = vector_between(cell_center, vertex2);
                let mag2 = magnitude(vec2);
                if mag2 == 0.0 {
                    continue;
                }

                let cross_product = cross(vec1, vec2);
                if dot(cross_product, normal) <= 0.0 {
                    continue;
                }
                let angle = (dot(vec1, vec2) / (mag1 * mag2)).clamp(-1.0, 1.0).acos();
                if angle < min_angle {
                    min_angle = angle;
                    swap_slot = Some(candidate_slot);
                }
            }

            if let Some(candidate_slot) = swap_slot {
                if candidate_slot != slot + 1 {
                    ordered[cell_id].swap(slot + 1, candidate_slot);
                }
            }
        }
    }

    Some(ordered)
}
