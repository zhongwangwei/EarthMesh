use crate::validate_unstructured_mesh;
use crate::LonLatPoint;
use crate::MpasMesh;
use crate::UnstructuredMesh;
use std::io;

pub(crate) fn normalize_unstructured_mesh_placeholder_rows(
    mesh: &UnstructuredMesh,
) -> io::Result<UnstructuredMesh> {
    validate_unstructured_mesh(mesh)?;
    let max_cell_id = mesh
        .m_to_w
        .iter()
        .flat_map(|row| row.iter().copied())
        .filter_map(|value| usize::try_from(value).ok())
        .max()
        .unwrap_or(0);
    let max_triangle_id = mesh
        .w_to_m
        .iter()
        .flat_map(|row| row.iter().copied())
        .filter_map(|value| usize::try_from(value).ok())
        .max()
        .unwrap_or(0);
    let missing_cell_placeholder = max_cell_id >= mesh.w_points.len();
    let missing_triangle_placeholder = max_triangle_id >= mesh.m_points.len();

    if !missing_cell_placeholder && !missing_triangle_placeholder {
        return Ok(mesh.clone());
    }

    let mut normalized = mesh.clone();
    let placeholder = LonLatPoint { lon: 0.0, lat: 0.0 };
    if has_single_placeholder_row(mesh) {
        if missing_triangle_placeholder {
            normalized.m_points.insert(0, placeholder);
            normalized.m_to_w.insert(0, [1, 1, 1]);
        }
        if missing_cell_placeholder {
            normalized.w_points.insert(0, placeholder);
            normalized.w_to_m.insert(0, vec![1]);
            normalized.n_w_to_m.insert(0, 1);
        }
        return Ok(normalized);
    }

    if missing_triangle_placeholder {
        for row in &mut normalized.w_to_m {
            for value in row {
                if *value > 0 {
                    *value = value.checked_add(1).ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "triangle id overflow while inserting Canonical placeholder rows",
                        )
                    })?;
                }
            }
        }
    }
    if missing_cell_placeholder {
        for row in &mut normalized.m_to_w {
            for value in row {
                if *value > 0 {
                    *value = value.checked_add(1).ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "cell id overflow while inserting Canonical placeholder rows",
                        )
                    })?;
                }
            }
        }
    }
    if missing_triangle_placeholder {
        normalized.m_points.splice(0..0, [placeholder, placeholder]);
        normalized.m_to_w.splice(0..0, [[1, 1, 1], [1, 1, 1]]);
    }
    if missing_cell_placeholder {
        normalized.w_points.splice(0..0, [placeholder, placeholder]);
        normalized.w_to_m.splice(0..0, [vec![1], vec![1]]);
        normalized.n_w_to_m.splice(0..0, [1, 1]);
    }
    Ok(normalized)
}

pub(super) fn normalize_mpas_placeholder_inputs(
    mesh: &UnstructuredMesh,
    cellwidth: &[f64],
) -> io::Result<(UnstructuredMesh, Vec<f64>)> {
    if cellwidth.len() != mesh.w_points.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "cellwidth length {} must match w_points length {}",
                cellwidth.len(),
                mesh.w_points.len()
            ),
        ));
    }

    let normalized = normalize_unstructured_mesh_placeholder_rows(mesh)?;
    let mut normalized_cellwidth = cellwidth.to_vec();
    if normalized.w_points.len() > cellwidth.len() {
        let missing_placeholders = normalized.w_points.len() - cellwidth.len();
        let placeholder_cellwidth = normalized_cellwidth.first().copied().unwrap_or(1.0);
        for _ in 0..missing_placeholders {
            normalized_cellwidth.insert(0, placeholder_cellwidth);
        }
    }
    if normalized_cellwidth.len() != normalized.w_points.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "cellwidth length {} must match normalized w_points length {}",
                normalized_cellwidth.len(),
                normalized.w_points.len()
            ),
        ));
    }

    Ok((normalized, normalized_cellwidth))
}

pub(super) fn trim_mpas_inserted_placeholder_rows(
    mesh: &mut MpasMesh,
    extra_cell_rows: usize,
    extra_vertex_rows: usize,
) {
    if extra_cell_rows > 0 {
        drain_prefix(&mut mesh.lat_cell, extra_cell_rows);
        drain_prefix(&mut mesh.lon_cell, extra_cell_rows);
        drain_prefix(&mut mesh.x_cell, extra_cell_rows);
        drain_prefix(&mut mesh.y_cell, extra_cell_rows);
        drain_prefix(&mut mesh.z_cell, extra_cell_rows);
        drain_prefix(&mut mesh.n_edges_on_cell, extra_cell_rows);
        drain_prefix(&mut mesh.cells_on_cell, extra_cell_rows);
        drain_prefix(&mut mesh.vertices_on_cell, extra_cell_rows);
        drain_prefix(&mut mesh.edges_on_cell, extra_cell_rows);
        drain_prefix(&mut mesh.area_cell, extra_cell_rows);
        drain_prefix(&mut mesh.mesh_density, extra_cell_rows);
    }
    if extra_vertex_rows > 0 {
        drain_prefix(&mut mesh.lat_vertex, extra_vertex_rows);
        drain_prefix(&mut mesh.lon_vertex, extra_vertex_rows);
        drain_prefix(&mut mesh.x_vertex, extra_vertex_rows);
        drain_prefix(&mut mesh.y_vertex, extra_vertex_rows);
        drain_prefix(&mut mesh.z_vertex, extra_vertex_rows);
        drain_prefix(&mut mesh.cells_on_vertex, extra_vertex_rows);
        drain_prefix(&mut mesh.edges_on_vertex, extra_vertex_rows);
        drain_prefix(&mut mesh.area_triangle, extra_vertex_rows);
        drain_prefix(&mut mesh.kite_areas_on_vertex, extra_vertex_rows);
    }
    let extra_edge_rows = extra_cell_rows.max(extra_vertex_rows);
    if extra_edge_rows > 0 {
        drain_prefix(&mut mesh.lat_edge, extra_edge_rows);
        drain_prefix(&mut mesh.lon_edge, extra_edge_rows);
        drain_prefix(&mut mesh.x_edge, extra_edge_rows);
        drain_prefix(&mut mesh.y_edge, extra_edge_rows);
        drain_prefix(&mut mesh.z_edge, extra_edge_rows);
        drain_prefix(&mut mesh.cells_on_edge, extra_edge_rows);
        drain_prefix(&mut mesh.vertices_on_edge, extra_edge_rows);
        drain_prefix(&mut mesh.n_edges_on_edge, extra_edge_rows);
        drain_prefix(&mut mesh.edges_on_edge, extra_edge_rows);
        drain_prefix(&mut mesh.dv_edge, extra_edge_rows);
        drain_prefix(&mut mesh.dc_edge, extra_edge_rows);
        drain_prefix(&mut mesh.angle_edge, extra_edge_rows);
        drain_prefix(&mut mesh.weights_on_edge, extra_edge_rows);
        drain_prefix(&mut mesh.error_segment, extra_edge_rows);
    }
}

fn drain_prefix<T>(values: &mut Vec<T>, count: usize) {
    values.drain(0..count);
}

fn has_single_placeholder_row(mesh: &UnstructuredMesh) -> bool {
    mesh.m_to_w
        .first()
        .is_some_and(|row| row.iter().all(|value| *value == 1))
        && mesh
            .w_to_m
            .first()
            .is_some_and(|row| !row.is_empty() && row.iter().all(|value| *value == 1))
        && mesh
            .n_w_to_m
            .first()
            .is_some_and(|value| matches!(*value, 0 | 1))
}
