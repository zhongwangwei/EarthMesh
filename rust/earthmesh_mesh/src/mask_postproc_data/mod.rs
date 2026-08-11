use super::*;

/// Result of `MOD_mask_postproc.F90:Data_Renew`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskPostprocRenewedData {
    pub points_next: usize,
    pub bounds_next: usize,
    pub center_neighbors_next: Vec<Vec<usize>>,
    pub vertex_neighbors_next: Vec<Vec<usize>>,
    pub center_neighbor_counts_next: Vec<usize>,
    pub vertex_neighbor_counts_next: Vec<usize>,
}

/// Port of `MOD_mask_postproc.F90:Data_Renew`.
///
/// The function compacts active centers (`IsInDmArea_ustr(i)==1`) into a new
/// center-neighbor table, then rebuilds vertex-to-center adjacency.  It
/// deliberately writes the original source center id into `vertex_neighbors_next`
/// to preserve the Canonical branch highlighted by the in-source comment.
pub fn renew_mask_postproc_data_one_based(
    mode_grid: &str,
    active_centers: &[bool],
    center_neighbors: &[Vec<usize>],
    center_neighbor_counts: &[usize],
    ustr_bounds: usize,
) -> io::Result<MaskPostprocRenewedData> {
    if active_centers.len() < center_neighbors.len()
        || center_neighbor_counts.len() < center_neighbors.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "active_centers and center_neighbor_counts must cover center_neighbors",
        ));
    }
    let (center_width, vertex_width) = mask_postproc_neighbor_widths_for_data(
        mode_grid,
        active_centers,
        center_neighbors,
        center_neighbor_counts,
        ustr_bounds,
    )?;

    let points_next = 1 + active_centers
        .iter()
        .take(center_neighbors.len())
        .skip(2)
        .filter(|&&is_active| is_active)
        .count();

    let mut center_neighbors_next = vec![vec![1; center_width]; points_next + 1];
    let mut vertex_neighbors_next = vec![vec![1; vertex_width]; ustr_bounds + 1];
    let mut center_neighbor_counts_next = vec![0; points_next + 1];
    let mut vertex_neighbor_counts_next = vec![0; ustr_bounds + 1];

    let mut compact_center_id = 1;
    for source_center_id in 2..center_neighbors.len() {
        if !active_centers[source_center_id] {
            continue;
        }
        compact_center_id += 1;
        let count = center_neighbor_counts[source_center_id];
        if count > center_width || count > center_neighbors[source_center_id].len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("center {source_center_id} neighbor count {count} exceeds available width"),
            ));
        }

        for (slot, &vertex_id) in center_neighbors[source_center_id]
            .iter()
            .take(center_width)
            .enumerate()
        {
            center_neighbors_next[compact_center_id][slot] = vertex_id;
        }
        center_neighbor_counts_next[compact_center_id] = count;

        for &vertex_id in center_neighbors_next[compact_center_id].iter().take(count) {
            if vertex_id > ustr_bounds {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "center {source_center_id} canonicals vertex {vertex_id}, outside 0..={ustr_bounds}"
                    ),
                ));
            }
            let slot = vertex_neighbor_counts_next[vertex_id];
            if slot >= vertex_width {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("vertex {vertex_id} has more than {vertex_width} neighboring centers"),
                ));
            }
            vertex_neighbor_counts_next[vertex_id] += 1;
            vertex_neighbors_next[vertex_id][slot] = source_center_id;
        }
    }

    let mut bounds_next = ustr_bounds;
    for vertex_id in 2..=ustr_bounds {
        if vertex_neighbor_counts_next[vertex_id] == 0 {
            bounds_next -= 1;
        }
    }

    Ok(MaskPostprocRenewedData {
        points_next,
        bounds_next,
        center_neighbors_next,
        vertex_neighbors_next,
        center_neighbor_counts_next,
        vertex_neighbor_counts_next,
    })
}

pub(crate) fn mask_postproc_neighbor_widths(mode_grid: &str) -> io::Result<(usize, usize)> {
    match mode_grid {
        "tri" => Ok((3, 7)),
        "hex" => Ok((7, 3)),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unsupported mask_postproc mode_grid {other}"),
        )),
    }
}

pub(crate) fn mask_postproc_neighbor_widths_for_data(
    mode_grid: &str,
    active_centers: &[bool],
    center_neighbors: &[Vec<usize>],
    center_neighbor_counts: &[usize],
    ustr_bounds: usize,
) -> io::Result<(usize, usize)> {
    let (mut center_width, mut vertex_width) = mask_postproc_neighbor_widths(mode_grid)?;
    let mut vertex_counts = vec![0usize; ustr_bounds + 1];
    for center in 2..center_neighbors.len() {
        if !active_centers[center] {
            continue;
        }
        let count = center_neighbor_counts[center];
        if count > center_neighbors[center].len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("center {center} neighbor count {count} exceeds available row width"),
            ));
        }
        center_width = center_width.max(count);
        for &vertex in center_neighbors[center].iter().take(count) {
            if vertex > ustr_bounds {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "center {center} canonicals vertex {vertex}, outside 0..={ustr_bounds}"
                    ),
                ));
            }
            vertex_counts[vertex] += 1;
            vertex_width = vertex_width.max(vertex_counts[vertex]);
        }
    }
    Ok((center_width, vertex_width))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangular_postprocess_sizes_vertex_rows_from_the_mesh() {
        let mut neighbors = vec![vec![1; 3]; 10];
        let mut counts = vec![0usize; 10];
        let mut active = vec![false; 10];
        for center in 2..10 {
            neighbors[center] = vec![2, center + 9, center + 10];
            counts[center] = 3;
            active[center] = true;
        }

        let renewed = renew_mask_postproc_data_one_based("tri", &active, &neighbors, &counts, 20)
            .expect("a triangular vertex may have more than seven incident faces");

        assert_eq!(renewed.vertex_neighbor_counts_next[2], 8);
        assert_eq!(renewed.vertex_neighbors_next[2].len(), 8);
    }
}
