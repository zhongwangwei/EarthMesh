use super::*;

/// Result of the pure classification part of `MOD_mask_postproc.F90:bdy_calculation`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryOrders {
    pub bdy_order: Vec<usize>,
    pub obc_order: Vec<usize>,
    pub ibc_order: Vec<usize>,
    pub rotation_start: Option<usize>,
}

/// Pure-data port of `MOD_mask_postproc.F90:bdy_calculation`.
///
/// This helper classifies the retained longest boundary into OBC/IBC order
/// arrays and performs the compatibility order rotation. Writing `obc.nc4` is kept in
/// the adapter layer.
pub fn classify_boundary_orders_one_based(
    num_bdy_long: [usize; 3],
    bdy_long_order: &[usize],
    vertex_neighbors: &[Vec<usize>],
    vertex_neighbor_counts: &[usize],
    vertex_mapping: &[usize],
    is_in_domain: &[i32],
) -> io::Result<BoundaryOrders> {
    let bdy_num = num_bdy_long[0];
    if bdy_num == 0 || bdy_long_order.len() < bdy_num {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "bdy_long_order must cover num_bdy_long[0]",
        ));
    }

    let mut bdy_order = bdy_long_order[..bdy_num].to_vec();
    let mut obc_order = vec![1usize; bdy_num];
    let mut ibc_order = vec![1usize; bdy_num];

    for idx in 1..bdy_num {
        let vertex_id = bdy_long_order[idx];
        require_vertex_count(vertex_id, vertex_neighbor_counts)?;
        if vertex_id >= vertex_neighbors.len() || vertex_id >= vertex_mapping.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("boundary vertex {vertex_id} is outside vertex tables"),
            ));
        }
        let count = vertex_neighbor_counts[vertex_id];
        if count > vertex_neighbors[vertex_id].len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("boundary vertex {vertex_id} neighbor count exceeds row width"),
            ));
        }

        let mut all_adjacent_centers_active = true;
        for &center_id in vertex_neighbors[vertex_id].iter().take(count) {
            if center_id >= is_in_domain.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "boundary vertex {vertex_id} canonicals center {center_id}, outside is_in_domain"
                    ),
                ));
            }
            if is_in_domain[center_id] != 1 {
                all_adjacent_centers_active = false;
                break;
            }
        }

        bdy_order[idx] = vertex_mapping[vertex_id];
        if all_adjacent_centers_active {
            obc_order[idx] = bdy_order[idx];
        } else {
            ibc_order[idx] = bdy_order[idx];
        }
    }

    if bdy_num >= 4 {
        for idx in 2..bdy_num - 1 {
            if obc_order[idx] != 1 && obc_order[idx - 1] == 1 && obc_order[idx + 1] == 1 {
                ibc_order[idx] = obc_order[idx];
                obc_order[idx] = 1;
            }
        }
    }

    let mut rotation_start = None;
    if bdy_num >= 4 {
        for idx in 1..=bdy_num - 3 {
            if obc_order[idx] == 1 {
                continue;
            }
            if obc_order[idx + 1] != 1 && obc_order[idx + 2] == 1 {
                rotate_boundary_order_like_canonical(&mut bdy_order, idx);
                rotate_boundary_order_like_canonical(&mut obc_order, idx);
                rotate_boundary_order_like_canonical(&mut ibc_order, idx);
                rotation_start = Some(idx + 1);
                break;
            }
        }
    }

    Ok(BoundaryOrders {
        bdy_order,
        obc_order,
        ibc_order,
        rotation_start,
    })
}

fn rotate_boundary_order_like_canonical(values: &mut [usize], split_idx: usize) {
    let original = values.to_vec();
    let mut write_idx = 1;
    for &value in original.iter().skip(split_idx + 1) {
        values[write_idx] = value;
        write_idx += 1;
    }
    for &value in original[1..=split_idx].iter().rev() {
        values[write_idx] = value;
        write_idx += 1;
    }
}
