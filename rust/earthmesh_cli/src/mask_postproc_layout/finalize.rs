use std::io;

use super::unstructured_mesh_from_mask_postproc_final;
use crate::{
    lonlat_pairs_from_points, validate_mask_postproc_layout, MaskPostprocFinalizationReport,
    MaskPostprocLayout, UnstructuredMesh,
};

/// Compose the Rust ports of the final `MOD_mask_postproc.F90:mask_postproc_*`
/// compaction steps into the gridfile payload written by `Unstructured_Mesh_Save`.
///
/// This intentionally starts after the domain-specific mask edits are already
/// represented in `IsInDmArea_ustr`; ocean-specific renewal, land patchtype
/// generation, and NetCDF I/O remain separate orchestration layers.
pub fn finalize_mask_postproc_layout_with_reindex_report(
    layout: &MaskPostprocLayout,
    is_in_domain_ustr: &[i32],
    mode_grid: &str,
) -> io::Result<MaskPostprocFinalizationReport> {
    validate_mask_postproc_layout(layout)?;
    let mode_grid = mode_grid.trim();
    let role_points = match mode_grid {
        "tri" => {
            if is_in_domain_ustr.len() < layout.ustr_points {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "IsInDmArea_ustr length {} must cover ustr_points {}",
                        is_in_domain_ustr.len(),
                        layout.ustr_points
                    ),
                ));
            }
            layout.ustr_points
        }
        "hex" => is_in_domain_ustr.len().min(layout.ustr_points),
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "final mask_postproc gridfile supports tri or hex mode_grid only, got {other}"
                ),
            ));
        }
    };

    let active_centers = is_in_domain_ustr
        .iter()
        .take(role_points)
        .map(|&value| value == 1)
        .collect::<Vec<_>>();
    let center_coordinates = lonlat_pairs_from_points(&layout.center_points);
    let vertex_coordinates = lonlat_pairs_from_points(&layout.vertex_points);
    let mut final_data = earthmesh_mesh::finalize_mask_postproc_data_fortran_indexed(
        mode_grid,
        &active_centers,
        &center_coordinates[..role_points],
        &vertex_coordinates,
        &layout.center_neighbors[..role_points],
        &layout.center_neighbor_counts[..role_points],
        layout.ustr_bounds.saturating_sub(1),
    )?;

    let unique_vertices = earthmesh_mesh::extract_unique_vertices_fortran_indexed(
        &final_data.center_neighbors_final,
        &final_data.center_neighbor_counts_final,
        layout.ustr_bounds.saturating_sub(1),
    )?;
    let vertex_reindex =
        earthmesh_mesh::sort_and_reindex_vertices(&unique_vertices, layout.ustr_bounds)?;
    final_data.bounds_final = vertex_reindex.sorted_vertices.len();
    final_data.vertex_coordinates_final = vec![[0.0, 0.0]; final_data.bounds_final + 1];
    for (offset, &source_vertex_id) in vertex_reindex.sorted_vertices.iter().enumerate() {
        let final_vertex_id = offset + 1;
        if final_vertex_id <= 1 || source_vertex_id <= 1 {
            continue;
        }
        let Some(&coordinates) = vertex_coordinates.get(source_vertex_id) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("sorted final vertex {source_vertex_id} outside vertex coordinate table"),
            ));
        };
        final_data.vertex_coordinates_final[final_vertex_id] = coordinates;
    }
    final_data.center_neighbors_final =
        earthmesh_mesh::reindex_final_center_vertices_fortran_indexed(
            &final_data.center_neighbors_final,
            &final_data.center_neighbor_counts_final,
            &vertex_reindex.vertex_mapping,
        )?;
    rebuild_final_vertex_neighbors_from_reindexed_centers(&mut final_data)?;

    let mesh = unstructured_mesh_from_mask_postproc_final(&final_data, mode_grid)?;
    Ok(MaskPostprocFinalizationReport {
        mesh,
        final_data,
        vertex_reindex,
    })
}

/// Compose the Rust ports of the final `MOD_mask_postproc.F90:mask_postproc_*`
/// compaction steps into the gridfile payload written by `Unstructured_Mesh_Save`.
///
/// Use `finalize_mask_postproc_layout_with_reindex_report` when downstream
/// writers need the original-vertex to final-vertex mapping, such as ocean OBC
/// boundary classification.
pub fn finalize_mask_postproc_layout_to_unstructured_mesh(
    layout: &MaskPostprocLayout,
    is_in_domain_ustr: &[i32],
    mode_grid: &str,
) -> io::Result<UnstructuredMesh> {
    Ok(
        finalize_mask_postproc_layout_with_reindex_report(layout, is_in_domain_ustr, mode_grid)?
            .mesh,
    )
}

fn rebuild_final_vertex_neighbors_from_reindexed_centers(
    final_data: &mut earthmesh_mesh::MaskPostprocFinalData,
) -> io::Result<()> {
    let vertex_width = final_data
        .vertex_neighbors_final
        .first()
        .map(|row| row.len())
        .unwrap_or(0);
    if vertex_width == 0 && final_data.bounds_final > 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "final vertex neighbor rows must have positive width",
        ));
    }
    if final_data.center_neighbors_final.len() <= final_data.points_final
        || final_data.center_neighbor_counts_final.len() <= final_data.points_final
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "final center neighbor tables must cover points_final",
        ));
    }

    let mut vertex_neighbors_final = vec![vec![1; vertex_width]; final_data.bounds_final + 1];
    let mut vertex_neighbor_counts_final = vec![0usize; final_data.bounds_final + 1];
    for center_id in 2..=final_data.points_final {
        let count = final_data.center_neighbor_counts_final[center_id];
        if count > final_data.center_neighbors_final[center_id].len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("final center {center_id} neighbor count exceeds row width"),
            ));
        }
        for &vertex_id in final_data.center_neighbors_final[center_id]
            .iter()
            .take(count)
        {
            if vertex_id > final_data.bounds_final {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "final center {center_id} references vertex {vertex_id}, outside bounds_final {}",
                        final_data.bounds_final
                    ),
                ));
            }
            let slot = vertex_neighbor_counts_final[vertex_id];
            if slot >= vertex_width {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "final vertex {vertex_id} has more than {vertex_width} neighboring centers"
                    ),
                ));
            }
            vertex_neighbors_final[vertex_id][slot] = center_id;
            vertex_neighbor_counts_final[vertex_id] += 1;
        }
    }
    for vertex_id in 2..=final_data.bounds_final {
        let count = vertex_neighbor_counts_final[vertex_id];
        if count == 0 || count >= vertex_width {
            continue;
        }
        let fill = vertex_neighbors_final[vertex_id][0];
        for slot in count..vertex_width {
            vertex_neighbors_final[vertex_id][slot] = fill;
        }
    }
    final_data.vertex_neighbors_final = vertex_neighbors_final;
    final_data.vertex_neighbor_counts_final = vertex_neighbor_counts_final;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LonLatPoint;

    #[test]
    fn tri_finalization_preserves_active_triangles_like_v2() {
        let zeros = LonLatPoint { lon: 0.0, lat: 0.0 };
        let layout = MaskPostprocLayout {
            ustr_points: 5,
            ustr_bounds: 8,
            center_points: vec![
                zeros,
                zeros,
                LonLatPoint { lon: 0.0, lat: 0.5 },
                LonLatPoint { lon: 0.5, lat: 0.5 },
                LonLatPoint {
                    lon: -1.0,
                    lat: 0.5,
                },
            ],
            vertex_points: vec![
                zeros,
                zeros,
                LonLatPoint { lon: 0.0, lat: 0.0 },
                LonLatPoint { lon: 1.0, lat: 0.0 },
                LonLatPoint { lon: 0.0, lat: 1.0 },
                LonLatPoint { lon: 1.0, lat: 1.0 },
                LonLatPoint {
                    lon: -1.0,
                    lat: 0.0,
                },
                LonLatPoint {
                    lon: -1.0,
                    lat: 1.0,
                },
            ],
            center_neighbors: vec![
                vec![1; 3],
                vec![1; 3],
                vec![2, 3, 4],
                vec![4, 3, 5],
                vec![2, 6, 7],
            ],
            vertex_neighbors: vec![vec![]; 8],
            center_neighbor_counts: vec![0, 0, 3, 3, 3],
            vertex_neighbor_counts: vec![0; 8],
        };
        let active = vec![0, 0, 1, 1, 1];

        let report =
            finalize_mask_postproc_layout_with_reindex_report(&layout, &active, "tri").unwrap();

        assert_eq!(report.final_data.points_final, 4);
    }
}
