use super::*;

/// Result of `MOD_mask_postproc.F90:sort_and_reindex`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VertexReindex {
    pub sorted_vertices: Vec<usize>,
    pub vertex_mapping: Vec<usize>,
}

/// Port of `MOD_mask_postproc.F90:extract_unique_vertices`.
///
/// The input is Rust row-major by center id: `center_neighbors[j][i]` mirrors
/// Canonical `ustr_ngr_center_f(i, j)`. Slot `1` is preserved as the compatibility empty
/// vertex placeholder and the scan starts at center id `2`.
pub fn extract_unique_vertices_one_based(
    center_neighbors: &[Vec<usize>],
    neighbor_counts: &[usize],
    max_vertex_id: usize,
) -> io::Result<Vec<usize>> {
    if neighbor_counts.len() < center_neighbors.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "neighbor_counts length must cover center_neighbors",
        ));
    }

    let mut is_selected = vec![true; max_vertex_id + 1];
    let mut unique_vertices = vec![1];
    for center_id in 2..center_neighbors.len() {
        let count = neighbor_counts[center_id];
        if count > center_neighbors[center_id].len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "neighbor count {count} exceeds center {center_id} row length {}",
                    center_neighbors[center_id].len()
                ),
            ));
        }
        for &vertex_id in center_neighbors[center_id].iter().take(count) {
            if vertex_id > max_vertex_id {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "center {center_id} canonicals vertex {vertex_id}, outside 0..={max_vertex_id}"
                    ),
                ));
            }
            if is_selected[vertex_id] {
                unique_vertices.push(vertex_id);
                is_selected[vertex_id] = false;
            }
        }
    }

    Ok(unique_vertices)
}

/// Port of `MOD_mask_postproc.F90:sort_and_reindex`.
///
/// Returns the sorted unique vertex list and the Canonical-style old vertex id to
/// new compact id mapping. Mapping slot `0` is retained but unused.
pub fn sort_and_reindex_vertices(
    unique_vertices: &[usize],
    max_vertex_id: usize,
) -> io::Result<VertexReindex> {
    let mut sorted_vertices = unique_vertices.to_vec();
    sorted_vertices.sort_unstable();

    let mut vertex_mapping = vec![0; max_vertex_id + 1];
    for (new_id, &old_vertex_id) in sorted_vertices.iter().enumerate() {
        if old_vertex_id > max_vertex_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("vertex {old_vertex_id} outside 0..={max_vertex_id}"),
            ));
        }
        vertex_mapping[old_vertex_id] = new_id + 1;
    }

    Ok(VertexReindex {
        sorted_vertices,
        vertex_mapping,
    })
}

/// Port of the final `ustr_ngr_center_f = vertex_mapping(ustr_ngr_center_f)`
/// loop in `MOD_mask_postproc.F90:mask_postproc_*`.
///
/// The scan preserves Canonical indexing by leaving rows `0` and `1` untouched
/// and only remapping slots covered by `center_neighbor_counts`.
pub fn reindex_final_center_vertices_one_based(
    center_neighbors_final: &[Vec<usize>],
    center_neighbor_counts_final: &[usize],
    vertex_mapping: &[usize],
) -> io::Result<Vec<Vec<usize>>> {
    if center_neighbor_counts_final.len() < center_neighbors_final.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "center_neighbor_counts_final must cover center_neighbors_final",
        ));
    }

    let mut reindexed = center_neighbors_final.to_vec();
    for center_id in 2..center_neighbors_final.len() {
        let count = center_neighbor_counts_final[center_id];
        if count > center_neighbors_final[center_id].len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("center {center_id} neighbor count exceeds available row width"),
            ));
        }
        for slot in 0..count {
            let old_vertex_id = center_neighbors_final[center_id][slot];
            let Some(&new_vertex_id) = vertex_mapping.get(old_vertex_id) else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "center {center_id} canonicals vertex {old_vertex_id}, outside vertex_mapping"
                    ),
                ));
            };
            if new_vertex_id == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("center {center_id} canonicals unmapped vertex {old_vertex_id}"),
                ));
            }
            reindexed[center_id][slot] = new_vertex_id;
        }
    }

    Ok(reindexed)
}
