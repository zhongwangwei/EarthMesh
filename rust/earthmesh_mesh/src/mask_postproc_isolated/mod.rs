use super::*;

/// Result of `MOD_mask_postproc.F90:Isolated_Ocean_Renew`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsolatedOceanRenewal {
    pub num_bdy_long: [usize; 3],
    pub bdy_long_order: Vec<usize>,
    pub removed_curve_ids: Vec<usize>,
    pub n_close_curve_after: Vec<usize>,
}

/// Pure-data port of `MOD_mask_postproc.F90:Isolated_Ocean_Renew`.
///
/// The caller supplies the already-built boundary connection so this helper can
/// focus on the compatibility closed-curve classification and inward peeling rule.
pub fn remove_isolated_ocean_one_based(
    is_in_domain: &mut [i32],
    center_neighbors: &[Vec<usize>],
    center_neighbor_counts: &[usize],
    vertex_neighbors_new: &[Vec<usize>],
    vertex_neighbor_counts: &[usize],
    vertex_neighbor_counts_new: &mut [usize],
    boundary: &BoundaryConnection,
) -> io::Result<IsolatedOceanRenewal> {
    if center_neighbor_counts.len() < center_neighbors.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "center_neighbor_counts must cover center_neighbors",
        ));
    }
    if vertex_neighbor_counts_new.len() != vertex_neighbor_counts.len()
        || vertex_neighbors_new.len() > vertex_neighbor_counts.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "vertex neighbor tables/counts must share a compatible Canonical-indexed domain",
        ));
    }

    let curves = &boundary.curves;
    let longest_curve_id = curves.num_bdy_long[2];
    let mut bdy_long_order = vec![1usize; curves.num_bdy_long[0]];
    if longest_curve_id > 0 {
        let longest_curve = curves.close_curves.get(longest_curve_id).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("longest curve id {longest_curve_id} is missing"),
            )
        })?;
        if longest_curve.len() + 1 > bdy_long_order.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "longest boundary curve does not fit bdy_long_order",
            ));
        }
        for (offset, &vertex_id) in longest_curve.iter().enumerate() {
            bdy_long_order[offset + 1] = vertex_id;
        }
    }

    let mut close_curves = curves.close_curves.clone();
    let mut n_close_curve = curves.n_close_curve.clone();
    let mut removed_curve_ids = Vec::new();
    for curve_id in 1..=curves.num_closed_curve {
        if curve_id == longest_curve_id {
            continue;
        }
        let curve = curves.close_curves.get(curve_id).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("closed curve id {curve_id} is missing"),
            )
        })?;
        let mut num_diff = 0isize;
        for &vertex_id in curve {
            require_vertex_count(vertex_id, vertex_neighbor_counts)?;
            num_diff += 2 * vertex_neighbor_counts_new[vertex_id] as isize
                - vertex_neighbor_counts[vertex_id] as isize;
        }
        if num_diff >= 0 {
            continue;
        }

        removed_curve_ids.push(curve_id);
        let mut num_add = 1usize;
        while num_add != 0 {
            let isolated_order = close_curves[curve_id].clone();
            let isolated_count = n_close_curve[curve_id];
            close_curves[curve_id].clear();
            n_close_curve[curve_id] = 0;

            for &boundary_vertex_id in isolated_order.iter().take(isolated_count) {
                let adjacent_center_count = vertex_neighbor_counts_new[boundary_vertex_id];
                vertex_neighbor_counts_new[boundary_vertex_id] = 0;
                let center_row = vertex_neighbors_new
                    .get(boundary_vertex_id)
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("vertex {boundary_vertex_id} missing vertex_neighbors_new row"),
                        )
                    })?;
                if adjacent_center_count > center_row.len() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "vertex {boundary_vertex_id} renewed count exceeds available row width"
                        ),
                    ));
                }
                for &center_id in center_row.iter().take(adjacent_center_count) {
                    if center_id >= is_in_domain.len() {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!(
                                "vertex {boundary_vertex_id} canonicals center {center_id}, outside is_in_domain"
                            ),
                        ));
                    }
                    is_in_domain[center_id] = -1;
                    let center_count = *center_neighbor_counts.get(center_id).ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("center {center_id} missing neighbor count"),
                        )
                    })?;
                    let center_row = center_neighbors.get(center_id).ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("center {center_id} missing neighbor row"),
                        )
                    })?;
                    if center_count > center_row.len() {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("center {center_id} neighbor count exceeds row width"),
                        ));
                    }
                    for &next_boundary_vertex_id in center_row.iter().take(center_count) {
                        require_vertex_count(next_boundary_vertex_id, vertex_neighbor_counts)?;
                        if vertex_neighbor_counts_new[next_boundary_vertex_id]
                            != vertex_neighbor_counts[next_boundary_vertex_id]
                        {
                            continue;
                        }
                        if !close_curves[curve_id].contains(&next_boundary_vertex_id) {
                            close_curves[curve_id].push(next_boundary_vertex_id);
                            n_close_curve[curve_id] += 1;
                        }
                    }
                }
            }

            num_add = n_close_curve[curve_id];
            if num_add == 1 {
                num_add = 0;
            }
        }
    }

    Ok(IsolatedOceanRenewal {
        num_bdy_long: curves.num_bdy_long,
        bdy_long_order,
        removed_curve_ids,
        n_close_curve_after: n_close_curve,
    })
}
