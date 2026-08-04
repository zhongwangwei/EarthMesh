use super::*;

/// Result of `MOD_mask_postproc.F90:bdy_connection_closed_curve`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryClosedCurves {
    pub num_closed_curve: usize,
    pub num_bdy_long: [usize; 3],
    pub close_curves: Vec<Vec<usize>>,
    pub n_close_curve: Vec<usize>,
}

pub fn push_boundary_neighbor(
    boundary_vertex_neighbors: &mut [Vec<usize>],
    vertex_id: usize,
    neighbor_id: usize,
) -> io::Result<()> {
    if boundary_vertex_neighbors[vertex_id].len() >= 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("boundary vertex {vertex_id} has more than four boundary connections"),
        ));
    }
    boundary_vertex_neighbors[vertex_id].push(neighbor_id);
    Ok(())
}

/// Port of `MOD_mask_postproc.F90:bdy_connection_closed_curve`.
///
/// `boundary_order[0]` and output curve slot `0` are placeholders matching the
/// Canonical convention that useful records start at index `1`/`2` depending on
/// the source array.  `num_bdy_long[0..2]` preserves the compatibility final `+1` on
/// longest/second-longest lengths because downstream allocation expects the
/// extra placeholder space.  Slot `1` (second-longest) deliberately deviates
/// from the compatibility Canonical, whose tracking logic was wrong (no demotion of the
/// old longest, curve 1 excluded); the Canonical canonical has been fixed the
/// same way.
pub fn boundary_closed_curves_one_based(
    boundary_order: &[usize],
    boundary_neighbors: &[Vec<usize>],
) -> io::Result<BoundaryClosedCurves> {
    if boundary_order.len() < 2 {
        return Ok(BoundaryClosedCurves {
            num_closed_curve: 0,
            num_bdy_long: [1, 1, 0],
            close_curves: vec![Vec::new()],
            n_close_curve: vec![0],
        });
    }

    let mut boundary_available = vec![true; boundary_order.len()];
    let mut boundary_position = vec![usize::MAX; boundary_neighbors.len()];
    for (position, &vertex) in boundary_order.iter().enumerate().skip(1) {
        if let Some(slot) = boundary_position.get_mut(vertex) {
            if *slot == usize::MAX {
                *slot = position;
            }
        }
    }
    let mut num_bdy_long = [0usize; 3];
    let mut close_curves = vec![Vec::new()];
    let mut n_close_curve = vec![0usize];

    while boundary_available
        .iter()
        .skip(1)
        .any(|&available| available)
    {
        let start_pos = boundary_available
            .iter()
            .enumerate()
            .skip(1)
            .find_map(|(pos, &available)| available.then_some(pos))
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing boundary start"))?;

        let start_vertex = boundary_order[start_pos];
        require_boundary_neighbor_row(start_vertex, boundary_neighbors)?;
        let mut boundary_queue = vec![start_vertex];
        boundary_available[start_pos] = false;

        let boundary_end = boundary_neighbors[start_vertex][1];
        let mut selected_neighbor = boundary_neighbors[start_vertex][0];
        while selected_neighbor != boundary_end {
            require_boundary_neighbor_row(selected_neighbor, boundary_neighbors)?;
            let previous_vertex = *boundary_queue.last().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "empty boundary queue")
            })?;
            boundary_queue.push(selected_neighbor);
            let selected_pos = boundary_position
                .get(selected_neighbor)
                .copied()
                .filter(|&position| position != usize::MAX)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("boundary vertex {selected_neighbor} not found in boundary_order"),
                    )
                })?;
            boundary_available[selected_pos] = false;

            selected_neighbor = if boundary_neighbors[selected_neighbor][0] == previous_vertex {
                boundary_neighbors[selected_neighbor][1]
            } else {
                boundary_neighbors[selected_neighbor][0]
            };
        }

        boundary_queue.push(boundary_end);
        let end_pos = boundary_position
            .get(boundary_end)
            .copied()
            .filter(|&position| position != usize::MAX)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("boundary end vertex {boundary_end} not found in boundary_order"),
                )
            })?;
        boundary_available[end_pos] = false;
        if boundary_queue.len() < 3 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "boundary closed curve has fewer than three points",
            ));
        }

        let curve_id = close_curves.len();
        let num_points = boundary_queue.len();
        close_curves.push(boundary_queue);
        n_close_curve.push(num_points);

        // Two-slot max tracking. The compatibility Canonical had three flaws here, fixed
        // identically on the Canonical side: it never demoted the previous
        // longest into slot 1, it permanently excluded curve 1 from the
        // second-longest slot, and it excluded curves tying the longest
        // length. Slot 1 has no production consumer, so this cannot change any
        // mesh output.
        if num_points > num_bdy_long[0] {
            num_bdy_long[1] = num_bdy_long[0];
            num_bdy_long[0] = num_points;
            num_bdy_long[2] = curve_id;
        } else if num_points > num_bdy_long[1] {
            num_bdy_long[1] = num_points;
        }
    }

    num_bdy_long[0] += 1;
    num_bdy_long[1] += 1;

    Ok(BoundaryClosedCurves {
        num_closed_curve: close_curves.len() - 1,
        num_bdy_long,
        close_curves,
        n_close_curve,
    })
}

fn require_boundary_neighbor_row(
    vertex_id: usize,
    boundary_neighbors: &[Vec<usize>],
) -> io::Result<()> {
    if vertex_id >= boundary_neighbors.len() || boundary_neighbors[vertex_id].len() < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("boundary vertex {vertex_id} must have two neighbor entries"),
        ));
    }
    Ok(())
}
