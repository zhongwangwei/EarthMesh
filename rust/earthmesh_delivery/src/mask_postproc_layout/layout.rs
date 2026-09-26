use std::io;

use crate::{
    i32_counts_as_usize, i32_rows_as_usize, lonlat_points_from_pairs, m_to_w_as_usize_rows,
    rows_to_triangle_connectivity, usize_rows_to_i32, usize_values_to_i32,
    validate_unstructured_mesh, MaskPostprocLayout, UnstructuredMesh,
};

/// Port of the repeated `mode_grid == 'tri'/'hex'` setup in
/// `MOD_mask_postproc.F90:mask_postproc_Earth/Lnd/Ocn`.
pub fn mask_postproc_layout_from_unstructured_mesh(
    mesh: &UnstructuredMesh,
    mode_grid: &str,
) -> io::Result<MaskPostprocLayout> {
    validate_unstructured_mesh(mesh)?;
    match mode_grid.trim() {
        "tri" => Ok(MaskPostprocLayout {
            ustr_points: mesh.m_points.len(),
            ustr_bounds: mesh.w_points.len(),
            center_points: mesh.m_points.clone(),
            vertex_points: mesh.w_points.clone(),
            center_neighbors: m_to_w_as_usize_rows(&mesh.m_to_w)?,
            vertex_neighbors: i32_rows_as_usize(&mesh.w_to_m, "itab_w%im")?,
            center_neighbor_counts: vec![3; mesh.m_points.len()],
            vertex_neighbor_counts: i32_counts_as_usize(&mesh.n_w_to_m, "n_ngrwm")?,
        }),
        "hex" => Ok(MaskPostprocLayout {
            ustr_points: mesh.w_points.len(),
            ustr_bounds: mesh.m_points.len(),
            center_points: mesh.w_points.clone(),
            vertex_points: mesh.m_points.clone(),
            center_neighbors: i32_rows_as_usize(&mesh.w_to_m, "itab_w%im")?,
            vertex_neighbors: m_to_w_as_usize_rows(&mesh.m_to_w)?,
            center_neighbor_counts: i32_counts_as_usize(&mesh.n_w_to_m, "n_ngrwm")?,
            vertex_neighbor_counts: vec![3; mesh.m_points.len()],
        }),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("mask_postproc layout supports tri or hex mode_grid only, got {other}"),
        )),
    }
}

/// Build the `Unstructured_Mesh_Save` payload used at the end of
/// `MOD_mask_postproc.F90:mask_postproc_*`.
///
/// For `tri`, the final center/vertex arrays are written directly.  For `hex`,
/// the Canonical call swaps center and vertex arguments so the compatibility gridfile
/// still stores triangles in `m*` variables and polygons in `w*` variables.
pub fn unstructured_mesh_from_mask_postproc_final(
    final_data: &earthmesh_mesh::MaskPostprocFinalData,
    mode_grid: &str,
) -> io::Result<UnstructuredMesh> {
    match mode_grid.trim() {
        "tri" => Ok(UnstructuredMesh {
            m_points: lonlat_points_from_pairs(
                "center_coordinates_final",
                &final_data.center_coordinates_final,
                final_data.points_final,
            )?,
            w_points: lonlat_points_from_pairs(
                "vertex_coordinates_final",
                &final_data.vertex_coordinates_final,
                final_data.bounds_final,
            )?,
            m_to_w: rows_to_triangle_connectivity(
                "center_neighbors_final",
                &final_data.center_neighbors_final,
                final_data.points_final,
            )?,
            w_to_m: usize_rows_to_i32(
                "vertex_neighbors_final",
                &final_data.vertex_neighbors_final,
            )?,
            n_w_to_m: usize_values_to_i32(
                "vertex_neighbor_counts_final",
                &final_data.vertex_neighbor_counts_final,
            )?,
        }),
        "hex" => Ok(UnstructuredMesh {
            m_points: lonlat_points_from_pairs(
                "vertex_coordinates_final",
                &final_data.vertex_coordinates_final,
                final_data.bounds_final,
            )?,
            w_points: lonlat_points_from_pairs(
                "center_coordinates_final",
                &final_data.center_coordinates_final,
                final_data.points_final,
            )?,
            m_to_w: rows_to_triangle_connectivity(
                "vertex_neighbors_final",
                &final_data.vertex_neighbors_final,
                final_data.bounds_final,
            )?,
            w_to_m: usize_rows_to_i32(
                "center_neighbors_final",
                &final_data.center_neighbors_final,
            )?,
            n_w_to_m: usize_values_to_i32(
                "center_neighbor_counts_final",
                &final_data.center_neighbor_counts_final,
            )?,
        }),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("final mask_postproc gridfile supports tri or hex mode_grid only, got {other}"),
        )),
    }
}
