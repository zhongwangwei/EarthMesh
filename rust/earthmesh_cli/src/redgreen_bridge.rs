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
    use earthmesh_mesh::{LonLatDegrees, RefinementRegion};

    #[test]
    fn a_refined_mesh_arrives_with_every_table_intact() {
        // The tables are the same five under different names, so the test that
        // matters is that none of them loses a row or a slot on the way.
        let mesh =
            earthmesh_mesh::TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25).expect("base mesh");
        let neighbors = mesh.m_neighbors.clone();
        let redgreen = earthmesh_refine_redgreen::redgreen_mesh_from_triangular(&mesh, &neighbors)
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
            green_parents: Vec::new(),
            refinement_levels: Vec::new(),
        };
        mesh.cells_on_triangle[2] = [1, 1, i32::MAX as usize + 1];

        let error = unstructured_mesh_from_redgreen(&mesh)
            .expect_err("an unaddressable id must not wrap into a gridfile");
        assert!(
            error.to_string().contains("exceeds what a gridfile"),
            "{error}"
        );
    }

    #[test]
    fn a_named_circle_refines_and_arrives_as_a_writable_mesh() {
        // The chain end to end: regions -> marking -> round -> gridfile tables.
        // Every link has its own test; this is the one that says they compose.
        let base =
            earthmesh_mesh::TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25).expect("base mesh");
        let neighbors = base.m_neighbors.clone();
        let mesh = earthmesh_refine_redgreen::redgreen_mesh_from_triangular(&base, &neighbors)
            .expect("bridge in");
        let before = mesh.triangle_count();

        let outcome = earthmesh_refine_redgreen::refine_redgreen_level(
            &mesh,
            &earthmesh_refine::RegionTargets::new(&[RefinementRegion::Circle {
                center: LonLatDegrees::new(0.0, 0.0),
                radius_meters: 3_000_000.0,
                level: 1,
            }]),
            &earthmesh_core::RefineConfig::default(),
            1,
            None,
            false,
            false,
        )
        .expect("one red-green level");
        let written = unstructured_mesh_from_redgreen(&outcome.mesh).expect("bridge out");

        assert!(
            outcome.refined_triangle_count > 0,
            "the circle asked for triangles: {outcome:?}"
        );
        assert!(
            outcome.mesh.triangle_count() > before,
            "a refined mesh has more triangles: {} vs {before}",
            outcome.mesh.triangle_count()
        );
        assert_eq!(
            written.m_to_w.len(),
            outcome.mesh.cells_on_triangle.len(),
            "and arrives whole"
        );
    }

    /// A refined mesh has to reach the writer with both arrays on the same row
    /// layout.
    ///
    /// A gridfile reader picks between the compact layout (id = row + 1) and the
    /// two-placeholder one (id = row) by whether rows 0 and 1 sit at the origin
    /// -- and it picks per array. The renewal left slot 0 of the cell points at
    /// its 9999 "no vertex here yet" sentinel, so the cell array read as compact
    /// while the triangle array read as two-placeholder. The file opened, passed
    /// its checks, and resolved every connectivity id one row off.
    #[test]
    fn a_refined_mesh_keeps_both_arrays_on_the_same_row_layout() {
        let base =
            earthmesh_mesh::TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25).expect("base mesh");
        let neighbors = base.m_neighbors.clone();
        let mesh = earthmesh_refine_redgreen::redgreen_mesh_from_triangular(&base, &neighbors)
            .expect("bridge in");

        let outcome = earthmesh_refine_redgreen::refine_redgreen_level(
            &mesh,
            &earthmesh_refine::RegionTargets::new(&[RefinementRegion::Circle {
                center: LonLatDegrees::new(0.0, 0.0),
                radius_meters: 3_000_000.0,
                level: 1,
            }]),
            // With the transition rows off the round leaves hanging nodes by
            // design -- only the hexagonal dual reads such a mesh -- so the
            // triangular view is only a mesh to check when they are on.
            &earthmesh_core::RefineConfig {
                is_transition: true,
                ..earthmesh_core::RefineConfig::default()
            },
            1,
            None,
            false,
            false,
        )
        .expect("one red-green level");
        let written = unstructured_mesh_from_redgreen(&outcome.mesh).expect("bridge out");

        let topology = crate::unstructured_mesh_support::check_unstructured_mesh_topology(&written);
        assert!(
            topology.is_consistent(),
            "a refined red-green mesh must reach the writer as one mesh: {:?}",
            &topology.violations[..topology.violations.len().min(4)]
        );
    }
}
