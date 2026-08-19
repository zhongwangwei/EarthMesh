use super::*;

fn point(x: f64, y: f64, z: f64) -> CartesianPoint {
    CartesianPoint::new(x, y, z)
}

/// A tetrahedron: the smallest closed triangulation, so every edge is shared.
fn tetrahedron() -> (Vec<CartesianPoint>, Vec<[usize; 3]>) {
    let vertices = vec![
        point(0.0, 0.0, 0.0),
        point(0.0, 0.0, 0.0),
        point(1.0, 1.0, 1.0),
        point(1.0, -1.0, -1.0),
        point(-1.0, 1.0, -1.0),
        point(-1.0, -1.0, 1.0),
    ];
    let triangles = vec![
        [1, 1, 1],
        [1, 1, 1],
        [2, 3, 4],
        [2, 4, 5],
        [2, 5, 3],
        [3, 5, 4],
    ];
    (vertices, triangles)
}

/// Adjacency is derived, and over a closed surface nothing is left open.
#[test]
fn a_closed_triangulation_has_no_open_edge() {
    let (vertices, triangles) = tetrahedron();
    let state = MeshState::from_parts(vertices, triangles).expect("a tetrahedron is a mesh");

    assert_eq!(state.vertex_count(), 4);
    assert_eq!(state.triangle_count(), 4);
    assert_eq!(
        state.open_edge_count(),
        0,
        "every edge of a closed surface has a triangle across it"
    );
    state.validate().expect("adjacency is symmetric");
}

/// One triangle alone has three edges and nothing across any of them.
#[test]
fn a_single_triangle_is_open_on_every_edge() {
    let vertices = vec![
        point(0.0, 0.0, 0.0),
        point(0.0, 0.0, 0.0),
        point(1.0, 0.0, 0.0),
        point(0.0, 1.0, 0.0),
        point(0.0, 0.0, 1.0),
    ];
    let state = MeshState::from_parts(vertices, vec![[1, 1, 1], [1, 1, 1], [2, 3, 4]])
        .expect("one triangle is a mesh");
    assert_eq!(state.open_edge_count(), 3);
    state.validate().expect("nothing to be asymmetric about");
}

/// A triangle naming one corner twice encloses nothing, and is refused.
#[test]
fn a_triangle_with_a_repeated_corner_is_refused() {
    let (vertices, mut triangles) = tetrahedron();
    triangles[2] = [3, 3, 4];
    let errors = MeshState::from_parts(vertices, triangles)
        .expect_err("a repeated corner is not a triangle");
    assert!(errors.contains(&MeshStateError::DegenerateTriangle {
        triangle: 2,
        corners: [3, 3, 4]
    }));
}

/// A corner the mesh does not carry is refused, rather than indexed.
#[test]
fn a_triangle_naming_an_absent_vertex_is_refused() {
    let (vertices, mut triangles) = tetrahedron();
    triangles[3] = [2, 4, 99];
    let errors = MeshState::from_parts(vertices, triangles)
        .expect_err("a corner outside the mesh is not a corner");
    assert!(errors.contains(&MeshStateError::UnknownVertex {
        triangle: 3,
        vertex: 99
    }));
}

/// Three triangles on one edge is not a surface.
#[test]
fn an_edge_claimed_by_three_triangles_is_refused() {
    let mut vertices = vec![point(0.0, 0.0, 0.0); 2];
    vertices.extend([
        point(1.0, 0.0, 0.0),
        point(0.0, 1.0, 0.0),
        point(0.0, 0.0, 1.0),
        point(-1.0, 0.0, 0.0),
        point(0.0, -1.0, 0.0),
    ]);
    let triangles = vec![[1, 1, 1], [1, 1, 1], [2, 3, 4], [2, 3, 5], [2, 3, 6]];
    let errors = MeshState::from_parts(vertices, triangles)
        .expect_err("an edge with three triangles is not manifold");
    assert!(errors
        .iter()
        .any(|error| matches!(error, MeshStateError::NonManifoldEdge { triangles: 3, .. })));
}

/// The real thing: a production mesh converts, and loses only Method-C's
/// bookkeeping.
#[test]
fn a_method_c_mesh_converts_to_the_neutral_state_and_closes() {
    let mesh = TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25).expect("base mesh");
    let state = MeshState::from_triangular_mesh(&mesh).expect("convert");

    assert_eq!(state.vertex_count(), mesh.nmd - 1);
    assert_eq!(state.triangle_count(), mesh.nwd - 1);
    assert_eq!(
        state.open_edge_count(),
        0,
        "a global icosahedral mesh is a closed sphere"
    );
    state
        .validate()
        .expect("a production mesh is a valid state");

    // Euler over a closed sphere: V - E + F = 2, with each triangle carrying
    // three half edges. This is the check that says the conversion kept the
    // topology and not only the numbers.
    let faces = state.triangle_count();
    let edges = faces * 3 / 2;
    let vertices = state.vertex_count();
    assert_eq!(
        vertices as isize - edges as isize + faces as isize,
        2,
        "V {vertices} - E {edges} + F {faces}"
    );
}

#[test]
fn stable_ids_track_slot_and_generation() {
    let (vertices, triangles) = tetrahedron();
    let mut state = MeshState::from_parts(vertices, triangles).expect("mesh");
    let vertex = state.vertex_id(2).expect("vertex id");
    let face = state.face_id(2).expect("face id");
    let edge = state.edge_id(2, 3).expect("edge id");

    assert!(state.contains_vertex_id(vertex));
    assert!(state.contains_face_id(face));
    assert_eq!(edge, EdgeId::new(state.vertex_id(3).unwrap(), vertex));

    state.set_triangle(2, [2, 3, 5]);
    assert!(!state.contains_face_id(face));
    assert!(state.contains_vertex_id(vertex));
}

#[test]
fn validation_requires_reverse_neighbour_on_the_same_edge() {
    let (vertices, triangles) = tetrahedron();
    let mut state = MeshState::from_parts(vertices, triangles).expect("mesh");
    state.neighbours[2][0] = 3;

    let errors = state
        .validate()
        .expect_err("wrong-edge reciprocal neighbour is asymmetric");
    assert!(errors.iter().any(|error| matches!(
        error,
        MeshStateError::AsymmetricNeighbour {
            triangle: 2,
            neighbour: 3
        }
    )));
}

#[test]
fn reserved_vertex_slots_are_rejected_as_unknown_vertices() {
    let (vertices, mut triangles) = tetrahedron();
    triangles[2] = [1, 3, 4];

    let errors = MeshState::from_parts(vertices, triangles)
        .expect_err("reserved slots are not active vertices");
    assert!(errors.contains(&MeshStateError::UnknownVertex {
        triangle: 2,
        vertex: 1
    }));

    let (vertices, triangles) = tetrahedron();
    let mut state = MeshState::from_parts(vertices, triangles).expect("mesh");
    state.triangles[2] = [0, 3, 4];
    let errors = state
        .validate()
        .expect_err("validation also rejects reserved slots");
    assert!(errors.contains(&MeshStateError::UnknownVertex {
        triangle: 2,
        vertex: 0
    }));
}

#[test]
fn triangular_mesh_conversion_rejects_short_tables_before_indexing() {
    let mesh = TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25).expect("base mesh");

    let mut short_points = mesh.clone();
    short_points.m_points.truncate(short_points.nmd);
    let error =
        MeshState::from_triangular_mesh(&short_points).expect_err("short point table is malformed");
    assert!(error.to_string().contains("m_points"));

    let mut short_faces = mesh;
    short_faces.w_faces.truncate(short_faces.nwd);
    let error =
        MeshState::from_triangular_mesh(&short_faces).expect_err("short face table is malformed");
    assert!(error.to_string().contains("w_faces"));
}

#[test]
fn retired_rows_are_not_active_entities() {
    let (vertices, triangles) = tetrahedron();
    let mut state = MeshState::from_parts(vertices, triangles).expect("mesh");
    let vertex = state.vertex_id(2).expect("vertex id");
    let face = state.face_id(2).expect("face id");

    let region: std::collections::BTreeSet<_> = state.active_triangle_slots().collect();
    state.retire_triangle_in_region_for_test(2, &region);
    assert_eq!(state.face_id(2), None);
    assert!(!state.contains_face_id(face));
    assert_eq!(state.triangle_count(), 3);
    assert_eq!(state.open_edge_count(), 3);
    state.validate().expect("retired faces are ignored");

    state.retire_vertex_for_test(2);
    assert_eq!(state.vertex_id(2), None);
    assert!(!state.contains_vertex_id(vertex));
    assert_eq!(state.vertex_count(), 3);
    let errors = state
        .validate()
        .expect_err("live faces may not name retired vertices");
    assert!(errors.contains(&MeshStateError::UnknownVertex {
        triangle: 3,
        vertex: 2
    }));
}

#[test]
fn active_slot_iterators_skip_retired_rows() {
    let (vertices, triangles) = tetrahedron();
    let mut state = MeshState::from_parts(vertices, triangles).expect("mesh");
    state.retire_vertex_for_test(5);
    let region: std::collections::BTreeSet<_> = state.active_triangle_slots().collect();
    state.retire_triangle_in_region_for_test(5, &region);

    assert_eq!(
        state.active_vertex_slots().collect::<Vec<_>>(),
        vec![2, 3, 4]
    );
    assert_eq!(
        state.active_triangle_slots().collect::<Vec<_>>(),
        vec![2, 3, 4]
    );
}
