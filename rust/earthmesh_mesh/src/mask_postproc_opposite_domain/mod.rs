use super::*;

/// Port of `MOD_mask_postproc.F90:IsInDmArea_ustr_Renew_v2`.
///
/// For vertices with exactly two missing neighboring triangles, the legacy code
/// checks opposite slots (`j` and `j+3`) and refills both when both are
/// currently outside the active domain.
pub fn renew_mask_postproc_opposite_domain_triangles_fortran_indexed(
    is_in_domain: &mut [i32],
    vertex_neighbors: &[Vec<usize>],
    vertex_neighbor_counts: &[usize],
    vertex_neighbor_counts_new: &[usize],
    points_new: &mut isize,
) -> io::Result<()> {
    if is_in_domain.len() < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "is_in_domain must preserve at least the Fortran placeholder slots",
        ));
    }
    if vertex_neighbor_counts.len() < vertex_neighbors.len()
        || vertex_neighbor_counts_new.len() < vertex_neighbors.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "vertex neighbor table and count arrays must have matching Fortran-indexed lengths",
        ));
    }

    let ustr_points = is_in_domain.len() - 1;
    let ustr_bounds = vertex_neighbors.len().saturating_sub(1);
    for vertex_id in 2..=ustr_bounds {
        let count_original = vertex_neighbor_counts[vertex_id];
        let count_new = vertex_neighbor_counts_new[vertex_id];
        if count_original > vertex_neighbors[vertex_id].len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("vertex {vertex_id} neighbor count exceeds available row width"),
            ));
        }
        if count_original < count_new {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "vertex {vertex_id} renewed count {count_new} exceeds original count {count_original}"
                ),
            ));
        }
        if count_original - count_new != 2 {
            continue;
        }
        for slot in 0..count_original.saturating_sub(3) {
            let left_center_id = vertex_neighbors[vertex_id][slot];
            let right_center_id = vertex_neighbors[vertex_id][slot + 3];
            if left_center_id > ustr_points || right_center_id > ustr_points {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "vertex {vertex_id} references centers {left_center_id}/{right_center_id}, outside 0..={ustr_points}"
                    ),
                ));
            }
            if is_in_domain[left_center_id] != 1 && is_in_domain[right_center_id] != 1 {
                is_in_domain[left_center_id] = 1;
                is_in_domain[right_center_id] = 1;
                *points_new += 2;
            }
        }
    }

    Ok(())
}
