use super::*;
use std::collections::BTreeMap;

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

fn ordered_map_from_parts_reference(
    vertices: Vec<CartesianPoint>,
    triangles: Vec<[usize; 3]>,
) -> Result<Vec<[usize; 3]>, Vec<MeshStateError>> {
    let mut errors = Vec::new();
    for (triangle, corners) in triangles.iter().enumerate().skip(MESH_STATE_FIRST_ID) {
        for &corner in corners {
            if !valid_vertex_slot(corner, vertices.len()) {
                errors.push(MeshStateError::UnknownVertex {
                    triangle,
                    vertex: corner,
                });
            }
        }
        if corners[0] == corners[1] || corners[1] == corners[2] || corners[0] == corners[2] {
            errors.push(MeshStateError::DegenerateTriangle {
                triangle,
                corners: *corners,
            });
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    let mut claims: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
    for (triangle, corners) in triangles.iter().enumerate().skip(MESH_STATE_FIRST_ID) {
        for corner in 0..3 {
            claims
                .entry(edge_key(
                    corners[(corner + 1) % 3],
                    corners[(corner + 2) % 3],
                ))
                .or_default()
                .push(triangle);
        }
    }
    for (vertices_of_edge, claimants) in &claims {
        if claimants.len() > 2 {
            errors.push(MeshStateError::NonManifoldEdge {
                vertices: *vertices_of_edge,
                triangles: claimants.len(),
            });
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    let mut neighbours = vec![[0usize; 3]; triangles.len()];
    for (triangle, corners) in triangles.iter().enumerate().skip(MESH_STATE_FIRST_ID) {
        for corner in 0..3 {
            let key = edge_key(corners[(corner + 1) % 3], corners[(corner + 2) % 3]);
            neighbours[triangle][corner] = claims
                .get(&key)
                .and_then(|claimants| claimants.iter().copied().find(|&other| other != triangle))
                .unwrap_or(0);
        }
    }
    Ok(neighbours)
}

fn ordered_map_validate_reference(state: &MeshState) -> Result<(), Vec<MeshStateError>> {
    let mut errors = Vec::new();
    let mut claims: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
    for triangle in state.active_triangle_slots() {
        state.validate_triangle_row(triangle, &mut errors);
        let corners = state.triangles[triangle];
        if corners.iter().all(|corner| state.is_vertex_live(*corner))
            && corners[0] != corners[1]
            && corners[1] != corners[2]
            && corners[0] != corners[2]
        {
            for corner in 0..3 {
                claims
                    .entry(edge_key(
                        corners[(corner + 1) % 3],
                        corners[(corner + 2) % 3],
                    ))
                    .or_default()
                    .push(triangle);
            }
        }
        state.validate_neighbour_edges_for_triangle(triangle, &mut errors);
    }
    for (vertices, claimants) in claims {
        if claimants.len() > 2 {
            errors.push(MeshStateError::NonManifoldEdge {
                vertices,
                triangles: claimants.len(),
            });
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
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

#[test]
fn from_parts_matches_ordered_map_reference_for_closed_and_open_meshes() {
    let cases = [
        tetrahedron(),
        (
            vec![
                point(0.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(0.0, 0.0, 1.0),
            ],
            vec![[1, 1, 1], [1, 1, 1], [2, 3, 4]],
        ),
    ];
    for (vertices, triangles) in cases {
        let expected = ordered_map_from_parts_reference(vertices.clone(), triangles.clone())
            .expect("reference mesh");
        let state = MeshState::from_parts(vertices, triangles).expect("flat mesh");
        assert_eq!(state.neighbours, expected);
    }
}

#[test]
fn from_parts_error_order_matches_ordered_map_reference() {
    let (vertices, mut malformed) = tetrahedron();
    malformed[2] = [1, 3, 4];
    malformed[3] = [2, 4, 99];
    malformed[4] = [2, 2, 3];
    assert_eq!(
        MeshState::from_parts(vertices.clone(), malformed.clone()).unwrap_err(),
        ordered_map_from_parts_reference(vertices, malformed).unwrap_err()
    );

    let mut vertices = vec![point(0.0, 0.0, 0.0); 2];
    vertices.extend((0..8).map(|index| point(index as f64, 0.0, 1.0)));
    let nonmanifold = vec![
        [1, 1, 1],
        [1, 1, 1],
        [2, 3, 4],
        [2, 3, 5],
        [2, 3, 6],
        [4, 5, 7],
        [4, 5, 8],
        [4, 5, 9],
    ];
    assert_eq!(
        MeshState::from_parts(vertices.clone(), nonmanifold.clone()).unwrap_err(),
        ordered_map_from_parts_reference(vertices, nonmanifold).unwrap_err()
    );
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
fn validate_error_order_matches_ordered_map_reference() {
    let mut vertices = vec![point(0.0, 0.0, 0.0); 2];
    vertices.extend((0..8).map(|index| point(index as f64, 0.0, 1.0)));
    let triangles = vec![
        [1, 1, 1],
        [1, 1, 1],
        [2, 3, 4],
        [2, 3, 5],
        [2, 3, 6],
        [4, 5, 7],
        [4, 5, 8],
        [4, 5, 9],
    ];
    let mut state = MeshState {
        neighbours: vec![[0; 3]; triangles.len()],
        vertex_generations: vec![0; vertices.len()],
        triangle_generations: vec![0; triangles.len()],
        vertex_live: vec![true; vertices.len()],
        triangle_live: vec![true; triangles.len()],
        next_vertex_generation: 1,
        next_triangle_generation: 1,
        vertices,
        triangles,
    };
    state.vertex_live[0] = false;
    state.vertex_live[1] = false;
    state.triangle_live[0] = false;
    state.triangle_live[1] = false;
    assert_eq!(
        state.validate().unwrap_err(),
        ordered_map_validate_reference(&state).unwrap_err()
    );

    state.triangles[2] = [1, 3, 4];
    state.triangles[3] = [2, 2, 5];
    state.neighbours[4][0] = 7;

    assert_eq!(
        state.validate().unwrap_err(),
        ordered_map_validate_reference(&state).unwrap_err()
    );
}

#[test]
fn validate_ignores_inactive_faces_in_ordered_reference_equivalence() {
    let (vertices, triangles) = tetrahedron();
    let mut state = MeshState::from_parts(vertices, triangles).expect("mesh");
    state.triangles[2] = [2, 2, 3];
    state.triangle_live[2] = false;
    state.neighbours[3][0] = 2;

    assert_eq!(
        state.validate().unwrap_err(),
        ordered_map_validate_reference(&state).unwrap_err()
    );
    assert!(!state
        .validate()
        .unwrap_err()
        .contains(&MeshStateError::DegenerateTriangle {
            triangle: 2,
            corners: [2, 2, 3],
        }));
}

#[test]
#[ignore = "manual release timing for flat edge claims in MeshState::from_parts/validate"]
fn flat_edge_claims_benchmark() {
    let mesh = TriangularMesh::from_icosahedron(80, 0, 1.0, 0.25).expect("base mesh");
    let vertices = {
        let mut vertices = vec![CartesianPoint::new(0.0, 0.0, 0.0); mesh.nmd + 1];
        vertices[MESH_STATE_FIRST_ID..=mesh.nmd]
            .clone_from_slice(&mesh.m_points[MESH_STATE_FIRST_ID..=mesh.nmd]);
        vertices
    };
    let triangles = {
        let mut triangles = vec![[1usize; 3]; mesh.nwd + 1];
        for iw in MESH_STATE_FIRST_ID..=mesh.nwd {
            triangles[iw] = mesh.w_faces[iw].im;
        }
        triangles
    };

    let started = std::time::Instant::now();
    let legacy_neighbours =
        ordered_map_from_parts_reference(vertices.clone(), triangles.clone()).unwrap();
    let legacy_from_parts = started.elapsed();
    let started = std::time::Instant::now();
    let state = MeshState::from_parts(vertices, triangles).unwrap();
    let from_parts = started.elapsed();
    assert_eq!(legacy_neighbours, state.neighbours);

    let started = std::time::Instant::now();
    ordered_map_validate_reference(&state).unwrap();
    let legacy_validate = started.elapsed();
    let started = std::time::Instant::now();
    state.validate().unwrap();
    let validate = started.elapsed();
    eprintln!(
        "mesh_state edge claims vertices={} triangles={} legacy_from_parts_ms={:.3} flat_from_parts_ms={:.3} legacy_validate_ms={:.3} flat_validate_ms={:.3}",
        state.vertex_count(),
        state.triangle_count(),
        legacy_from_parts.as_secs_f64() * 1000.0,
        from_parts.as_secs_f64() * 1000.0,
        legacy_validate.as_secs_f64() * 1000.0,
        validate.as_secs_f64() * 1000.0
    );
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
