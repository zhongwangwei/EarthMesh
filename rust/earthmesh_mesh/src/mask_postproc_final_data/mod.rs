use super::*;

/// Result of `MOD_mask_postproc.F90:Data_Finial`.
#[derive(Debug, Clone, PartialEq)]
pub struct MaskPostprocFinalData {
    pub points_final: usize,
    pub bounds_final: usize,
    pub center_coordinates_final: Vec<[f64; 2]>,
    pub vertex_coordinates_final: Vec<[f64; 2]>,
    pub center_neighbors_final: Vec<Vec<usize>>,
    pub vertex_neighbors_final: Vec<Vec<usize>>,
    pub center_neighbor_counts_final: Vec<usize>,
    pub vertex_neighbor_counts_final: Vec<usize>,
}

/// Port of `MOD_mask_postproc.F90:Data_Finial`.
///
/// This is the final placeholder-preserving compaction after domain-mask edits:
/// active centers are copied to compact ids, vertex adjacency is rebuilt using
/// those compact center ids (`k` in the Fortran comment), then only vertices
/// that still have adjacent centers are copied to the final vertex arrays.
pub fn finalize_mask_postproc_data_fortran_indexed(
    mode_grid: &str,
    active_centers: &[bool],
    center_coordinates: &[[f64; 2]],
    vertex_coordinates: &[[f64; 2]],
    center_neighbors: &[Vec<usize>],
    center_neighbor_counts: &[usize],
    ustr_bounds: usize,
) -> io::Result<MaskPostprocFinalData> {
    let (center_width, vertex_width) = mask_postproc_neighbor_widths(mode_grid)?;
    if active_centers.len() < center_neighbors.len()
        || center_coordinates.len() < center_neighbors.len()
        || center_neighbor_counts.len() < center_neighbors.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "active_centers, center_coordinates, and center_neighbor_counts must cover center_neighbors",
        ));
    }
    if vertex_coordinates.len() <= ustr_bounds {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "vertex_coordinates length {} must cover Fortran vertex ids 1..={ustr_bounds}",
                vertex_coordinates.len()
            ),
        ));
    }

    let points_final = 1 + active_centers
        .iter()
        .take(center_neighbors.len())
        .skip(2)
        .filter(|&&is_active| is_active)
        .count();

    let mut center_coordinates_final = vec![[0.0, 0.0]; points_final + 1];
    let mut center_neighbors_final = vec![vec![1; center_width]; points_final + 1];
    let mut center_neighbor_counts_final = vec![0; points_final + 1];
    let mut vertex_neighbors_work = vec![vec![1; vertex_width]; ustr_bounds + 1];
    let mut vertex_neighbor_counts_work = vec![0; ustr_bounds + 1];

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

        center_coordinates_final[compact_center_id] = center_coordinates[source_center_id];
        for (slot, &vertex_id) in center_neighbors[source_center_id]
            .iter()
            .take(center_width)
            .enumerate()
        {
            center_neighbors_final[compact_center_id][slot] = vertex_id;
        }
        center_neighbor_counts_final[compact_center_id] = count;

        for &vertex_id in center_neighbors_final[compact_center_id].iter().take(count) {
            if vertex_id > ustr_bounds {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "center {source_center_id} references vertex {vertex_id}, outside 0..={ustr_bounds}"
                    ),
                ));
            }
            let slot = vertex_neighbor_counts_work[vertex_id];
            if slot >= vertex_width {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("vertex {vertex_id} has more than {vertex_width} neighboring centers"),
                ));
            }
            vertex_neighbor_counts_work[vertex_id] += 1;
            vertex_neighbors_work[vertex_id][slot] = compact_center_id;
        }
    }

    let bounds_final = 1 + vertex_neighbor_counts_work
        .iter()
        .take(ustr_bounds + 1)
        .skip(2)
        .filter(|&&count| count > 0)
        .count();

    let mut vertex_coordinates_final = vec![[0.0, 0.0]; bounds_final + 1];
    let mut vertex_neighbors_final = vec![vec![1; vertex_width]; bounds_final + 1];
    let mut vertex_neighbor_counts_final = vec![0; bounds_final + 1];

    let mut compact_vertex_id = 1;
    for source_vertex_id in 2..=ustr_bounds {
        if vertex_neighbor_counts_work[source_vertex_id] == 0 {
            continue;
        }
        compact_vertex_id += 1;
        vertex_coordinates_final[compact_vertex_id] = vertex_coordinates[source_vertex_id];
        vertex_neighbors_final[compact_vertex_id] = vertex_neighbors_work[source_vertex_id].clone();
        vertex_neighbor_counts_final[compact_vertex_id] =
            vertex_neighbor_counts_work[source_vertex_id];
    }

    Ok(MaskPostprocFinalData {
        points_final,
        bounds_final,
        center_coordinates_final,
        vertex_coordinates_final,
        center_neighbors_final,
        vertex_neighbors_final,
        center_neighbor_counts_final,
        vertex_neighbor_counts_final,
    })
}
