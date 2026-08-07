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
    let mesh = TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25, 0).expect("base mesh");
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

/// A refined mesh converts too, which is what a backend lifted onto this type
/// would need.
#[test]
fn a_refined_method_c_mesh_also_converts() {
    let mesh = crate::MethodCMesh::from_icosahedron(9, 0, 1.0, 0.25, 0).expect("base mesh");
    let refined = mesh
        .spawn_nest(
            &[crate::RefinementRegion::Circle {
                center: crate::LonLatDegrees::new(0.0, 0.0),
                radius_meters: 2_000_000.0,
                level: 1,
            }],
            1,
        )
        .expect("one level");
    let state = MeshState::from_triangular_mesh(&refined).expect("convert refined");
    assert_eq!(state.open_edge_count(), 0);
    state
        .validate()
        .expect("still a valid state after refining");
    assert!(
        state.triangle_count()
            > MeshState::from_triangular_mesh(&mesh)
                .unwrap()
                .triangle_count()
    );
}
