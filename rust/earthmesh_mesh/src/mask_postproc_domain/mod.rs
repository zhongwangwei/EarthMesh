use super::*;

/// Port of `MOD_mask_postproc.F90:IsInDmArea_ustr_Renew`.
///
/// `is_in_domain` mirrors the global Canonical `IsInDmArea_ustr` array: `1` is
/// active/ocean, negative values are inactive/land, and slot `1` is the compatibility
/// placeholder.  The routine first removes triangles whose three vertices are
/// all solid-boundary vertices (`n_ustr_ngr == 6`), then applies the compatibility
/// one-missing-triangle refill rule and updates `points_new` with the same
/// per-vertex increments/decrements as the Canonical code.
pub fn renew_mask_postproc_domain_triangles_one_based(
    is_in_domain: &mut [i32],
    vertex_neighbors: &[Vec<usize>],
    vertex_neighbors_new: &[Vec<usize>],
    vertex_neighbor_counts: &[usize],
    vertex_neighbor_counts_new: &[usize],
    points_new: &mut isize,
) -> io::Result<()> {
    if is_in_domain.len() < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "is_in_domain must preserve at least the Canonical placeholder slots",
        ));
    }
    if vertex_neighbor_counts.len() < vertex_neighbors.len()
        || vertex_neighbor_counts_new.len() < vertex_neighbors.len()
        || vertex_neighbors_new.len() < vertex_neighbors.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "vertex neighbor tables and count arrays must have matching Canonical-indexed lengths",
        ));
    }
    let ustr_points = is_in_domain.len() - 1;
    let ustr_bounds = vertex_neighbors.len().saturating_sub(1);
    let mut solid_boundary_vertex_tally = vec![3usize; is_in_domain.len()];

    for vertex_id in 2..=ustr_bounds {
        let count_new = vertex_neighbor_counts_new[vertex_id];
        let count_original = vertex_neighbor_counts[vertex_id];
        if count_new > vertex_neighbors_new[vertex_id].len()
            || count_original > vertex_neighbors[vertex_id].len()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("vertex {vertex_id} neighbor count exceeds available row width"),
            ));
        }
        if count_new == 0 || count_new == count_original {
            continue;
        }
        for &center_id in vertex_neighbors_new[vertex_id].iter().take(count_new) {
            if center_id > ustr_points {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "vertex {vertex_id} canonicals center {center_id}, outside 0..={ustr_points}"
                    ),
                ));
            }
            solid_boundary_vertex_tally[center_id] += 1;
        }
    }

    for center_id in 2..=ustr_points {
        if is_in_domain[center_id] != 1 {
            continue;
        }
        if solid_boundary_vertex_tally[center_id] == 6 {
            is_in_domain[center_id] = -1;
            *points_new -= 1;
        }
    }

    for vertex_id in 2..=ustr_bounds {
        let count_original = vertex_neighbor_counts[vertex_id];
        let count_new = vertex_neighbor_counts_new[vertex_id];
        if count_original < count_new {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "vertex {vertex_id} renewed count {count_new} exceeds original count {count_original}"
                ),
            ));
        }
        if count_original - count_new != 1 {
            continue;
        }
        for &center_id in vertex_neighbors[vertex_id].iter().take(count_original) {
            if center_id > ustr_points {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "vertex {vertex_id} canonicals center {center_id}, outside 0..={ustr_points}"
                    ),
                ));
            }
            is_in_domain[center_id] = 1;
        }
        *points_new += 1;
    }

    Ok(())
}
