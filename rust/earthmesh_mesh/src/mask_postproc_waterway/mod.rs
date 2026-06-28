use super::*;

/// Port of `MOD_mask_postproc.F90:narrow_waterway_widen`.
///
/// The helper builds the temporary boundary vertex-to-vertex graph from compact
/// center rows, detects the legacy four-connection narrow-waterway signature,
/// then activates every original center adjacent to the duplicated neighbor.
pub fn widen_narrow_waterway_fortran_indexed(
    is_in_domain: &mut [i32],
    vertex_neighbors: &[Vec<usize>],
    center_neighbors_new: &[Vec<usize>],
    vertex_neighbor_counts: &[usize],
    vertex_neighbor_counts_new: &[usize],
    center_neighbor_counts_new: &[usize],
) -> io::Result<()> {
    if vertex_neighbor_counts.len() < vertex_neighbors.len()
        || vertex_neighbor_counts_new.len() < vertex_neighbors.len()
        || center_neighbor_counts_new.len() < center_neighbors_new.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "neighbor tables and count arrays must have matching Fortran-indexed lengths",
        ));
    }

    let ustr_points = is_in_domain.len().saturating_sub(1);
    let ustr_bounds = vertex_neighbors.len().saturating_sub(1);
    let mut boundary_vertex_neighbors = vec![Vec::<usize>::new(); ustr_bounds + 1];

    for center_id in 2..center_neighbors_new.len() {
        let count = center_neighbor_counts_new[center_id];
        if count > center_neighbors_new[center_id].len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("center {center_id} neighbor count exceeds available row width"),
            ));
        }
        if count == 0 {
            continue;
        }
        for slot in 0..count {
            let left_vertex_id = center_neighbors_new[center_id][slot];
            if left_vertex_id > ustr_bounds {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "center {center_id} references vertex {left_vertex_id}, outside 0..={ustr_bounds}"
                    ),
                ));
            }
            if vertex_neighbor_counts_new[left_vertex_id] == vertex_neighbor_counts[left_vertex_id]
            {
                continue;
            }

            let right_vertex_id = center_neighbors_new[center_id][(slot + 1) % count];
            if right_vertex_id > ustr_bounds {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "center {center_id} references vertex {right_vertex_id}, outside 0..={ustr_bounds}"
                    ),
                ));
            }
            if vertex_neighbor_counts_new[right_vertex_id]
                == vertex_neighbor_counts[right_vertex_id]
            {
                continue;
            }

            push_boundary_neighbor(
                &mut boundary_vertex_neighbors,
                left_vertex_id,
                right_vertex_id,
            )?;
            push_boundary_neighbor(
                &mut boundary_vertex_neighbors,
                right_vertex_id,
                left_vertex_id,
            )?;
            break;
        }
    }

    for vertex_id in 2..=ustr_bounds {
        if boundary_vertex_neighbors[vertex_id].len() != 4 {
            continue;
        }
        let Some(duplicated_neighbor) =
            first_duplicate_neighbor(&boundary_vertex_neighbors[vertex_id])
        else {
            continue;
        };
        let count = vertex_neighbor_counts[duplicated_neighbor];
        if count > vertex_neighbors[duplicated_neighbor].len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("vertex {duplicated_neighbor} neighbor count exceeds available row width"),
            ));
        }
        for &center_id in vertex_neighbors[duplicated_neighbor].iter().take(count) {
            if center_id > ustr_points {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "vertex {duplicated_neighbor} references center {center_id}, outside 0..={ustr_points}"
                    ),
                ));
            }
            is_in_domain[center_id] = 1;
        }
    }

    Ok(())
}

/// Fill point-only ocean contacts around a vertex.
///
/// FVCOM-style ocean cells should not meet only at one vertex.  When the active
/// cells around an original vertex form multiple separated fans, activate the
/// missing cells in that vertex ring so the contact becomes edge-connected.
pub fn fill_vertex_only_ocean_contacts_fortran_indexed(
    is_in_domain: &mut [i32],
    vertex_neighbors: &[Vec<usize>],
    vertex_neighbor_counts: &[usize],
) -> io::Result<usize> {
    if vertex_neighbor_counts.len() < vertex_neighbors.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "vertex neighbor table and count array must have matching Fortran-indexed lengths",
        ));
    }

    let ustr_points = is_in_domain.len().saturating_sub(1);
    let ustr_bounds = vertex_neighbors.len().saturating_sub(1);
    let mut activated = 0;

    for vertex_id in 2..=ustr_bounds {
        let count = vertex_neighbor_counts[vertex_id];
        if count < 3 {
            continue;
        }
        if count > vertex_neighbors[vertex_id].len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("vertex {vertex_id} neighbor count exceeds available row width"),
            ));
        }

        let mut active = Vec::with_capacity(count);
        for &center_id in vertex_neighbors[vertex_id].iter().take(count) {
            if center_id > ustr_points {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "vertex {vertex_id} references center {center_id}, outside 0..={ustr_points}"
                    ),
                ));
            }
            active.push(center_id > 1 && is_in_domain[center_id] == 1);
        }

        let active_count = active.iter().filter(|&&is_active| is_active).count();
        if active_count <= 1 || active_count == count || cyclic_active_runs(&active) <= 1 {
            continue;
        }

        for &center_id in vertex_neighbors[vertex_id].iter().take(count) {
            if center_id > 1 && is_in_domain[center_id] != 1 {
                is_in_domain[center_id] = 1;
                activated += 1;
            }
        }
    }

    Ok(activated)
}

fn cyclic_active_runs(active: &[bool]) -> usize {
    active
        .iter()
        .enumerate()
        .filter(|(index, &is_active)| {
            is_active
                && !active[if *index == 0 {
                    active.len() - 1
                } else {
                    index - 1
                }]
        })
        .count()
}

fn first_duplicate_neighbor(neighbors: &[usize]) -> Option<usize> {
    for (left, &left_value) in neighbors.iter().enumerate() {
        for &right_value in neighbors.iter().skip(left + 1) {
            if left_value == right_value {
                return Some(left_value);
            }
        }
    }
    None
}
