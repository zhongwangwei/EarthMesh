//! Between the red-green refinement tables and the gridfile's mesh.
//!
//! They are the same five tables under different names -- the writer's
//! `m_to_w`/`w_to_m` are the pipeline's `ngrmw`/`ngrwm` -- so this is a
//! conversion rather than a translation. What it does have to do is police the
//! width: ids are `usize` on one side and `i32` on the other, and a mesh too
//! large to address in `i32` has to say so here rather than wrap silently into
//! a gridfile that reads as valid.

use std::io;

use earthmesh_refine_redgreen::RedGreenMesh;

use crate::coordinate_types::LonLatPoint;
use crate::unstructured_mesh_support::UnstructuredMesh;

fn narrow(id: usize, role: &str) -> io::Result<i32> {
    i32::try_from(id).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{role} id {id} exceeds what a gridfile can address"),
        )
    })
}

/// The refined mesh, in the shape the gridfile writer takes.
pub fn unstructured_mesh_from_redgreen(mesh: &RedGreenMesh) -> io::Result<UnstructuredMesh> {
    let point = |p: &earthmesh_mesh::LonLatDegrees| LonLatPoint {
        lon: p.lon_degrees,
        lat: p.lat_degrees,
    };
    let mut m_to_w = Vec::with_capacity(mesh.cells_on_triangle.len());
    for corners in &mesh.cells_on_triangle {
        m_to_w.push([
            narrow(corners[0], "cell")?,
            narrow(corners[1], "cell")?,
            narrow(corners[2], "cell")?,
        ]);
    }
    let mut w_to_m = Vec::with_capacity(mesh.triangles_on_cell.len());
    for row in &mesh.triangles_on_cell {
        let mut converted = Vec::with_capacity(row.len());
        for &triangle in row {
            converted.push(narrow(triangle, "triangle")?);
        }
        w_to_m.push(converted);
    }
    let mut n_w_to_m = Vec::with_capacity(mesh.n_triangles_on_cell.len());
    for &count in &mesh.n_triangles_on_cell {
        n_w_to_m.push(narrow(count, "triangle count")?);
    }

    Ok(UnstructuredMesh {
        m_points: mesh.triangle_points.iter().map(point).collect(),
        w_points: mesh.cell_points.iter().map(point).collect(),
        m_to_w,
        w_to_m,
        n_w_to_m,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refined_mesh_arrives_with_every_table_intact() {
        // The tables are the same five under different names, so the test that
        // matters is that none of them loses a row or a slot on the way.
        let mesh = earthmesh_mesh::MethodCDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 0)
            .expect("base mesh");
        let neighbors = mesh.m_neighbors.clone();
        let redgreen = earthmesh_refine_redgreen::redgreen_mesh_from_method_c(&mesh, &neighbors)
            .expect("bridge in");

        let written = unstructured_mesh_from_redgreen(&redgreen).expect("bridge out");

        assert_eq!(written.m_points.len(), redgreen.triangle_points.len());
        assert_eq!(written.w_points.len(), redgreen.cell_points.len());
        assert_eq!(written.m_to_w.len(), redgreen.cells_on_triangle.len());
        assert_eq!(written.w_to_m.len(), redgreen.triangles_on_cell.len());
        assert_eq!(
            written.m_to_w[2],
            redgreen.cells_on_triangle[2].map(|id| id as i32),
            "a triangle's corners survive the narrowing"
        );
        assert_eq!(
            written.n_w_to_m[2] as usize,
            redgreen.n_triangles_on_cell[2]
        );
    }

    #[test]
    fn an_id_too_wide_for_the_gridfile_is_refused_rather_than_wrapped() {
        // Silently wrapping would produce a gridfile that reads as valid and
        // points at the wrong cells.
        let mut mesh = RedGreenMesh {
            num_vertex: 1,
            num_center: 1,
            triangle_points: vec![earthmesh_mesh::LonLatDegrees::new(0.0, 0.0); 3],
            cell_points: vec![earthmesh_mesh::LonLatDegrees::new(0.0, 0.0); 3],
            cells_on_triangle: vec![[1, 1, 1]; 3],
            triangles_on_cell: vec![Vec::new(); 3],
            n_triangles_on_cell: vec![0; 3],
        };
        mesh.cells_on_triangle[2] = [1, 1, i32::MAX as usize + 1];

        let error = unstructured_mesh_from_redgreen(&mesh)
            .expect_err("an unaddressable id must not wrap into a gridfile");
        assert!(
            error.to_string().contains("exceeds what a gridfile"),
            "{error}"
        );
    }
}
