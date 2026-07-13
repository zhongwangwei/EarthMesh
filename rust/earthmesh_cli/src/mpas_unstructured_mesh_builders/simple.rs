use crate::lonlat_degrees_from_points;
use crate::split_cartesian_components;
use crate::validate_mpas_simple_mesh;
use crate::validate_unstructured_mesh;
use crate::MpasSimpleMesh;
use crate::UnstructuredMesh;
use earthmesh_mesh::lonlat_points_to_unit_xyz;
use std::io;

use super::placeholder_rows::normalize_mpas_placeholder_inputs;

pub fn build_mpas_simple_mesh_from_unstructured_one_based(
    mesh: &UnstructuredMesh,
    cellwidth: &[f64],
) -> io::Result<MpasSimpleMesh> {
    let (mesh, cellwidth) = normalize_mpas_placeholder_inputs(mesh, cellwidth)?;
    let mesh = &mesh;
    let cellwidth = cellwidth.as_slice();
    validate_unstructured_mesh(mesh)?;
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
    if cellwidth.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cellwidth must include the Canonical placeholder row",
        ));
    }
    if cellwidth
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cellwidth values must be finite and positive",
        ));
    }

    let vertex_xyz = lonlat_points_to_unit_xyz(&lonlat_degrees_from_points(&mesh.m_points));
    let cell_xyz = lonlat_points_to_unit_xyz(&lonlat_degrees_from_points(&mesh.w_points));
    let (x_vertex, y_vertex, z_vertex) = split_cartesian_components(&vertex_xyz);
    let (x_cell, y_cell, z_cell) = split_cartesian_components(&cell_xyz);

    let cells_on_vertex = mesh
        .m_to_w
        .iter()
        .enumerate()
        .map(|(row_idx, row)| {
            row.iter()
                .map(|&value| {
                    if value < 0 {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("m_to_w row {row_idx} contains negative cell id {value}"),
                        ));
                    }
                    if value <= 1 {
                        return Ok(0);
                    }
                    value.checked_sub(1).ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("m_to_w row {row_idx} contains invalid cell id {value}"),
                        )
                    })
                })
                .collect::<io::Result<Vec<i32>>>()
        })
        .collect::<io::Result<Vec<Vec<i32>>>>()?;

    let min_cellwidth = cellwidth.iter().copied().fold(f64::INFINITY, f64::min);
    let mesh_density = cellwidth
        .iter()
        .map(|width| (min_cellwidth / width).powi(4))
        .collect();

    let simple = MpasSimpleMesh {
        x_cell,
        y_cell,
        z_cell,
        x_vertex,
        y_vertex,
        z_vertex,
        cells_on_vertex,
        mesh_density,
    };
    validate_mpas_simple_mesh(&simple)?;
    Ok(simple)
}
