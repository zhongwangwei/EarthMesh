use super::*;

/// Result of `MOD_mask_postproc.F90:bdy_connection` before NetCDF output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryConnection {
    pub bdy_num_in: usize,
    pub boundary_order: Vec<usize>,
    pub boundary_neighbors: Vec<Vec<usize>>,
    pub curves: BoundaryClosedCurves,
}

/// Pure-data port of `MOD_mask_postproc.F90:bdy_connection`.
///
/// NetCDF writing of `obcv2.nc4` remains an adapter concern; this helper
/// returns the boundary order, the two-neighbor boundary graph, and the
/// closed-curve metadata needed by isolated-ocean removal.
pub fn boundary_connection_one_based(
    center_neighbors_new: &[Vec<usize>],
    center_neighbor_counts_new: &[usize],
    vertex_neighbor_counts: &[usize],
    vertex_neighbor_counts_new: &[usize],
) -> io::Result<BoundaryConnection> {
    if center_neighbor_counts_new.len() < center_neighbors_new.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "center_neighbor_counts_new must cover center_neighbors_new",
        ));
    }
    if vertex_neighbor_counts_new.len() != vertex_neighbor_counts.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "vertex neighbor count arrays must have matching lengths",
        ));
    }

    let ustr_bounds = vertex_neighbor_counts.len().saturating_sub(1);
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
                        "center {center_id} canonicals vertex {left_vertex_id}, outside 0..={ustr_bounds}"
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
                        "center {center_id} canonicals vertex {right_vertex_id}, outside 0..={ustr_bounds}"
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

    let mut boundary_order = vec![1usize];
    let mut boundary_neighbors = vec![vec![1usize, 1usize]; ustr_bounds + 1];
    for vertex_id in 2..=ustr_bounds {
        match boundary_vertex_neighbors[vertex_id].len() {
            0 => {}
            2 => {
                boundary_order.push(vertex_id);
                boundary_neighbors[vertex_id][0] = boundary_vertex_neighbors[vertex_id][0];
                boundary_neighbors[vertex_id][1] = boundary_vertex_neighbors[vertex_id][1];
            }
            count => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("boundary vertex {vertex_id} has {count} connections, expected 0 or 2"),
                ));
            }
        }
    }

    let curves = boundary_closed_curves_one_based(&boundary_order, &boundary_neighbors)?;
    Ok(BoundaryConnection {
        bdy_num_in: boundary_order.len(),
        boundary_order,
        boundary_neighbors,
        curves,
    })
}
