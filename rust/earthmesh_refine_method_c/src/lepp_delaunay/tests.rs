use super::*;
use crate::{lonlat_degrees_to_unit_xyz, LonLatDegrees, MethodCMesh, RefinementRegion};
use earthmesh_boundary::SegmentList;
use earthmesh_mesh::{
    in_circle_on_sphere, InsertionTransactionError, MeshState, Sign, MESH_STATE_FIRST_ID,
};
use std::collections::BTreeSet;

#[derive(Clone)]
struct Fixture {
    vertices: Vec<CartesianPoint>,
    triangles: Vec<[usize; 3]>,
    neighbours: Vec<[usize; 3]>,
}

impl ReadOnlyTriangulation for Fixture {
    fn vertices(&self) -> &[CartesianPoint] {
        &self.vertices
    }

    fn triangles(&self) -> &[[usize; 3]] {
        &self.triangles
    }

    fn neighbours(&self) -> &[[usize; 3]] {
        &self.neighbours
    }
}

fn p(lon: f64, lat: f64) -> CartesianPoint {
    lonlat_degrees_to_unit_xyz(LonLatDegrees::new(lon, lat))
}

fn mesh(vertices: Vec<CartesianPoint>, triangles: Vec<[usize; 3]>) -> MeshState {
    let mut v = vec![CartesianPoint::new(0.0, 0.0, 0.0); 2];
    v.extend(vertices);
    let mut t = vec![[1, 1, 1]; 2];
    t.extend(triangles);
    MeshState::from_parts(v, t).expect("test mesh")
}

fn default_config() -> LeppSearchConfig {
    LeppSearchConfig {
        maximum_path_length: 64,
        ..LeppSearchConfig::default()
    }
}

fn delaunay_violations(state: &MeshState) -> usize {
    let mut violations = 0;
    for triangle in MESH_STATE_FIRST_ID..state.triangles().len() {
        let corners = state.triangles()[triangle];
        for vertex in MESH_STATE_FIRST_ID..state.vertices().len() {
            if corners.contains(&vertex) {
                continue;
            }
            if in_circle_on_sphere(
                state.vertices()[corners[0]],
                state.vertices()[corners[1]],
                state.vertices()[corners[2]],
                state.vertices()[vertex],
            ) == Ok(Sign::Positive)
            {
                violations += 1;
            }
        }
    }
    violations
}

#[test]
fn invalid_config_is_rejected_before_search() {
    let m = mesh(
        vec![p(0.0, 0.0), p(120.0, 0.0), p(60.0, 20.0)],
        vec![[2, 3, 4]],
    );
    for config in [
        LeppSearchConfig {
            maximum_path_length: 0,
            ..LeppSearchConfig::default()
        },
        LeppSearchConfig {
            length_tie_relative_epsilon: -1.0e-12,
            ..default_config()
        },
        LeppSearchConfig {
            length_tie_relative_epsilon: f64::NAN,
            ..default_config()
        },
        LeppSearchConfig {
            length_tie_relative_epsilon: 1.0,
            ..default_config()
        },
    ] {
        assert!(matches!(
            find_lepp(&m, 2, &config),
            Err(LeppSearchError::InvalidConfig { .. })
        ));
    }
}

#[test]
fn nonfinite_coordinate_is_rejected() {
    let f = Fixture {
        vertices: vec![
            CartesianPoint::new(0.0, 0.0, 0.0),
            CartesianPoint::new(0.0, 0.0, 0.0),
            CartesianPoint::new(f64::NAN, 0.0, 0.0),
            p(120.0, 0.0),
            p(60.0, 20.0),
        ],
        triangles: vec![[1, 1, 1], [1, 1, 1], [2, 3, 4]],
        neighbours: vec![[0, 0, 0], [0, 0, 0], [0, 0, 0]],
    };
    assert!(matches!(
        find_lepp_in(&f, 2, &default_config()),
        Err(LeppSearchError::InvalidCoordinate { vertex: 2, .. })
    ));
}

#[test]
fn zero_vector_coordinate_is_rejected() {
    let f = Fixture {
        vertices: vec![
            CartesianPoint::new(0.0, 0.0, 0.0),
            CartesianPoint::new(0.0, 0.0, 0.0),
            CartesianPoint::new(0.0, 0.0, 0.0),
            p(120.0, 0.0),
            p(60.0, 20.0),
        ],
        triangles: vec![[1, 1, 1], [1, 1, 1], [2, 3, 4]],
        neighbours: vec![[0, 0, 0], [0, 0, 0], [0, 0, 0]],
    };
    assert!(matches!(
        find_lepp_in(&f, 2, &default_config()),
        Err(LeppSearchError::InvalidCoordinate { vertex: 2, .. })
    ));
}

#[test]
fn nonpositive_mesh_radius_is_rejected() {
    let f = Fixture {
        vertices: vec![
            CartesianPoint::new(0.0, 0.0, 0.0),
            CartesianPoint::new(0.0, 0.0, 0.0),
        ],
        triangles: vec![[1, 1, 1], [1, 1, 1], [2, 3, 4]],
        neighbours: vec![[0, 0, 0], [0, 0, 0], [0, 0, 0]],
    };
    assert!(matches!(
        find_lepp_in(&f, 2, &default_config()),
        Err(LeppSearchError::InvalidRadius { radius: 0.0, .. })
    ));
}

#[test]
fn zero_edge_length_is_rejected() {
    let f = Fixture {
        vertices: vec![
            CartesianPoint::new(0.0, 0.0, 0.0),
            CartesianPoint::new(0.0, 0.0, 0.0),
            p(0.0, 0.0),
            p(0.0, 0.0),
            p(60.0, 20.0),
        ],
        triangles: vec![[1, 1, 1], [1, 1, 1], [2, 3, 4]],
        neighbours: vec![[0, 0, 0], [0, 0, 0], [0, 0, 0]],
    };
    assert!(matches!(
        find_lepp_in(&f, 2, &default_config()),
        Err(LeppSearchError::InvalidEdgeLength { length: 0.0, .. })
    ));
}

#[test]
fn neighbour_must_point_back_across_same_edge() {
    let f = Fixture {
        vertices: vec![
            CartesianPoint::new(0.0, 0.0, 0.0),
            CartesianPoint::new(0.0, 0.0, 0.0),
            p(0.0, 0.0),
            p(120.0, 0.0),
            p(60.0, 45.0),
            p(60.0, -45.0),
        ],
        triangles: vec![[1, 1, 1], [1, 1, 1], [2, 3, 4], [3, 2, 5]],
        neighbours: vec![[0, 0, 0], [0, 0, 0], [0, 0, 3], [0, 0, 0]],
    };
    assert!(matches!(
        find_lepp_in(&f, 2, &default_config()),
        Err(LeppSearchError::AsymmetricNeighbour {
            face: 2,
            neighbour: 3,
            ..
        })
    ));
}

#[test]
fn spherical_edge_length_handles_ninety_degrees() {
    let length = spherical_edge_length(2.0, p(0.0, 0.0), p(90.0, 0.0));
    assert!((length - std::f64::consts::PI).abs() < 1.0e-12);
}

#[test]
fn spherical_edge_length_handles_antimeridian() {
    let length = spherical_edge_length(1.0, p(179.0, 0.0), p(-179.0, 0.0));
    assert!((length - 2.0_f64.to_radians()).abs() < 1.0e-12);
}

#[test]
fn longest_edge_tie_break_is_lexical_edge_id() {
    let m = mesh(
        vec![p(0.0, 0.0), p(90.0, 0.0), p(0.0, 90.0)],
        vec![[2, 3, 4]],
    );
    let path = find_lepp(&m, 2, &default_config()).expect("boundary path");
    assert_eq!(path.edges, vec![LeppEdgeId::new(2, 3)]);
}

#[test]
fn terminal_pair_stops_on_shared_longest_edge() {
    let m = mesh(
        vec![p(0.0, 0.0), p(120.0, 0.0), p(60.0, 45.0), p(60.0, -45.0)],
        vec![[2, 3, 4], [3, 2, 5]],
    );
    let path = find_lepp(&m, 2, &default_config()).expect("interior terminal");
    assert_eq!(path.faces, vec![2, 3]);
    assert_eq!(path.edges, vec![LeppEdgeId::new(2, 3)]);
    assert_eq!(
        path.terminal,
        LeppTerminal::InteriorPair {
            edge: LeppEdgeId::new(2, 3),
            faces: [2, 3]
        }
    );
}

#[test]
fn terminal_pair_cannot_append_past_maximum_path_length() {
    let m = mesh(
        vec![p(0.0, 0.0), p(120.0, 0.0), p(60.0, 45.0), p(60.0, -45.0)],
        vec![[2, 3, 4], [3, 2, 5]],
    );
    let error = find_lepp(
        &m,
        2,
        &LeppSearchConfig {
            maximum_path_length: 1,
            ..LeppSearchConfig::default()
        },
    )
    .expect_err("terminal pair would exceed path limit");
    assert!(matches!(
        error,
        LeppSearchError::MaximumPathLength {
            maximum_path_length: 1,
            ..
        }
    ));
}

#[test]
fn multistep_lepp_walks_until_terminal_pair() {
    let m = mesh(
        vec![
            p(30.0, 20.0),
            p(0.0, 0.0),
            p(60.0, 0.0),
            p(-20.0, 0.0),
            p(150.0, 0.0),
            p(65.0, 0.0),
        ],
        vec![[2, 3, 4], [3, 5, 4], [5, 6, 4], [6, 5, 7]],
    );
    let path = find_lepp(&m, 2, &default_config()).expect("multistep terminal");
    assert_eq!(path.faces, vec![2, 3, 4, 5]);
    assert_eq!(
        path.edges,
        vec![
            LeppEdgeId::new(3, 4),
            LeppEdgeId::new(4, 5),
            LeppEdgeId::new(5, 6)
        ]
    );
    assert_eq!(
        path.terminal,
        LeppTerminal::InteriorPair {
            edge: LeppEdgeId::new(5, 6),
            faces: [4, 5]
        }
    );
}

#[test]
fn boundary_terminal_stops_without_neighbour() {
    let m = mesh(
        vec![p(0.0, 0.0), p(120.0, 0.0), p(60.0, 20.0)],
        vec![[2, 3, 4]],
    );
    let path = find_lepp(&m, 2, &default_config()).expect("boundary terminal");
    assert_eq!(
        path.terminal,
        LeppTerminal::Boundary {
            edge: LeppEdgeId::new(2, 3),
            face: 2
        }
    );
}

#[test]
fn nonmanifold_edge_is_rejected() {
    let f = Fixture {
        vertices: vec![
            CartesianPoint::new(0.0, 0.0, 0.0),
            CartesianPoint::new(0.0, 0.0, 0.0),
            p(0.0, 0.0),
            p(120.0, 0.0),
            p(60.0, 20.0),
            p(60.0, -20.0),
            p(60.0, 60.0),
        ],
        triangles: vec![[1, 1, 1], [1, 1, 1], [2, 3, 4], [3, 2, 5], [2, 3, 6]],
        neighbours: vec![[0, 0, 0], [0, 0, 0], [0, 0, 3], [0, 0, 2], [0, 0, 2]],
    };
    let error = find_lepp_in(&f, 2, &default_config()).expect_err("nonmanifold");
    assert!(
        matches!(error, LeppSearchError::NonManifoldEdge { edge, faces, .. } if edge == LeppEdgeId::new(2, 3) && faces == vec![2, 3, 4])
    );
}

#[test]
fn cycle_is_reported_instead_of_panicking() {
    let f = Fixture {
        vertices: vec![
            CartesianPoint::new(0.0, 0.0, 0.0),
            CartesianPoint::new(0.0, 0.0, 0.0),
            p(0.0, 0.0),
            p(120.0, 0.0),
            p(60.0, 20.0),
        ],
        triangles: vec![[1, 1, 1], [1, 1, 1], [2, 3, 4]],
        neighbours: vec![[0, 0, 0], [0, 0, 0], [0, 0, 2]],
    };
    let error = find_lepp_in(&f, 2, &default_config()).expect_err("cycle");
    assert!(matches!(error, LeppSearchError::Cycle { face: 2, .. }));
}

#[test]
fn maximum_path_length_is_reported() {
    let m = mesh(
        vec![
            p(30.0, 20.0),
            p(0.0, 0.0),
            p(60.0, 0.0),
            p(-20.0, 0.0),
            p(150.0, 0.0),
            p(65.0, 0.0),
        ],
        vec![[2, 3, 4], [3, 5, 4], [5, 6, 4], [6, 5, 7]],
    );
    let error = find_lepp(
        &m,
        2,
        &LeppSearchConfig {
            maximum_path_length: 1,
            ..LeppSearchConfig::default()
        },
    )
    .expect_err("path limit");
    assert!(matches!(
        error,
        LeppSearchError::MaximumPathLength {
            maximum_path_length: 1,
            ..
        }
    ));
}

#[test]
fn repeated_search_is_identical() {
    let m = mesh(
        vec![
            p(30.0, 20.0),
            p(0.0, 0.0),
            p(60.0, 0.0),
            p(-20.0, 0.0),
            p(150.0, 0.0),
            p(65.0, 0.0),
        ],
        vec![[2, 3, 4], [3, 5, 4], [5, 6, 4], [6, 5, 7]],
    );
    let first = find_lepp(&m, 2, &default_config()).expect("first");
    for _ in 0..100 {
        assert_eq!(find_lepp(&m, 2, &default_config()).expect("repeat"), first);
    }
}

#[test]
fn method_c_icosahedron_pseudorandom_faces_all_terminate() {
    let method_c = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 0).expect("base Method-C mesh");
    let state = MeshState::from_triangular_mesh(method_c.mesh()).expect("neutral state");
    let face_count = state.triangles().len() - earthmesh_mesh::MESH_STATE_FIRST_ID;
    let config = LeppSearchConfig {
        maximum_path_length: 256,
        ..LeppSearchConfig::default()
    };
    let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
    for _ in 0..1000 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let face = earthmesh_mesh::MESH_STATE_FIRST_ID + (seed as usize % face_count);
        let path = find_lepp(&state, face, &config).expect("global face terminates");
        assert!(!path.faces.is_empty());
    }
}

#[test]
fn near_antipodal_terminal_midpoint_is_rejected() {
    let state = mesh(
        vec![p(0.0, 0.0), p(180.0, 0.0), p(0.0, 60.0)],
        vec![[2, 3, 4]],
    );
    let edge = LeppEdgeId::new(2, 3);
    assert!(matches!(
        terminal_edge_midpoint(&state, edge),
        Err(LeppInsertionError::NearAntipodalTerminalEdge { edge: found }) if found == edge
    ));
}

#[test]
fn terminal_midpoint_uses_endpoint_directions_when_radii_differ() {
    let state = mesh(
        vec![
            CartesianPoint::new(1.0, 0.0, 0.0),
            CartesianPoint::new(0.0, 2.0, 0.0),
            CartesianPoint::new(0.0, 0.0, 1.0),
        ],
        vec![[2, 3, 4]],
    );
    let midpoint = terminal_edge_midpoint(&state, LeppEdgeId::new(2, 3)).expect("midpoint");
    assert!((midpoint.x - midpoint.y).abs() < 1.0e-12);
    assert!((magnitude(midpoint) - 1.5).abs() < 1.0e-12);
}

#[test]
fn lepp_terminal_midpoint_inserts_transactionally_and_round_trips() {
    let method_c = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 0).expect("base Method-C mesh");
    let original = MeshState::from_triangular_mesh(method_c.mesh()).expect("neutral state");
    let gates = LeppInsertionGates::for_method_c(method_c.mesh().impent);
    let mut blocked = original.clone();
    assert!(matches!(
        insert_lepp_terminal_midpoint(&mut blocked, 2, &default_config(), &gates),
        Err(LeppInsertionError::ProtectedVertexDegreeWouldChange { .. })
            | Err(LeppInsertionError::DegreeLimit { .. })
    ));
    assert_eq!(blocked, original, "a rejected insertion changes nothing");

    let (start, first, report) = (MESH_STATE_FIRST_ID..original.triangles().len())
        .find_map(|start| {
            let mut candidate = original.clone();
            insert_lepp_terminal_midpoint(&mut candidate, start, &default_config(), &gates)
                .ok()
                .map(|report| (start, candidate, report))
        })
        .expect("at least one Method-C-compatible terminal midpoint");
    let terminal_edge = match report.path.terminal {
        LeppTerminal::InteriorPair { edge, .. } => edge,
        LeppTerminal::Boundary { .. } => panic!("the global mesh has no boundary"),
    };

    assert_eq!(first.vertex_count(), original.vertex_count() + 1);
    assert_eq!(first.triangle_count(), original.triangle_count() + 2);
    assert_eq!(first.open_edge_count(), 0);
    first.validate().expect("valid topology");
    assert_eq!(delaunay_violations(&first), 0);
    assert!(first.contains_vertex_id(report.insertion.site_id));
    assert!(report
        .insertion
        .removed_ids
        .iter()
        .all(|&face| !first.contains_face_id(face)));
    assert!(report
        .created_faces
        .iter()
        .all(|&face| first.contains_face_id(face)));
    assert!(report.affected_sites.contains(&report.insertion.site_id));
    let changed: BTreeSet<_> = report.insertion.created.iter().copied().collect();
    let rebuilt_sites: Vec<_> = first
        .voronoi_cells_touching(&changed)
        .expect("local Voronoi rebuild")
        .iter()
        .filter_map(|cell| first.vertex_id(cell.site))
        .collect();
    assert_eq!(rebuilt_sites, report.affected_sites);

    let far_face = (MESH_STATE_FIRST_ID..original.triangles().len())
        .find(|&face| {
            !report.insertion.removed.contains(&face)
                && original.neighbours()[face]
                    .iter()
                    .all(|neighbour| !report.insertion.removed.contains(neighbour))
        })
        .expect("some face is outside the cavity");
    assert_eq!(first.face_id(far_face), original.face_id(far_face));
    assert_eq!(first.triangles()[far_face], original.triangles()[far_face]);
    assert_eq!(
        first.neighbours()[far_face],
        original.neighbours()[far_face]
    );

    let [tail, head] = terminal_edge.vertices;
    assert!(first.triangles()[MESH_STATE_FIRST_ID..]
        .iter()
        .all(|corners| !(corners.contains(&tail) && corners.contains(&head))));
    let new_site = report.insertion.site;
    assert!(first.triangles()[MESH_STATE_FIRST_ID..]
        .iter()
        .any(|corners| corners.contains(&tail) && corners.contains(&new_site)));
    assert!(first.triangles()[MESH_STATE_FIRST_ID..]
        .iter()
        .any(|corners| corners.contains(&head) && corners.contains(&new_site)));

    let rebuilt = first
        .to_triangular_mesh(method_c.mesh().impent, None)
        .expect("Method-C tables");
    rebuilt.validate_topology().expect("valid rebuilt tables");
    let round_trip = MeshState::from_triangular_mesh(&rebuilt).expect("round trip");
    assert_eq!(round_trip.vertices(), first.vertices());
    assert_eq!(round_trip.triangles(), first.triangles());
    assert_eq!(round_trip.neighbours(), first.neighbours());

    let mut second = original;
    let repeated = insert_lepp_terminal_midpoint(&mut second, start, &default_config(), &gates)
        .expect("same insertion");
    assert_eq!(second, first);
    assert_eq!(repeated, report);
}

fn post_quality_config(
    _method_c: &MethodCMesh,
    maximum_insertions: usize,
) -> LeppPostQualityConfig {
    LeppPostQualityConfig {
        maximum_edge_length: Some(1_300_000.0),
        minimum_spherical_triangle_angle_degrees: None,
        maximum_insertions,
        search: default_config(),
        gates: LeppInsertionGates::for_method_c(_method_c.mesh().impent),
    }
}

#[test]
fn post_quality_config_requires_a_trigger_and_positive_limit() {
    let method_c = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 0).expect("base Method-C mesh");
    let mut state = MeshState::from_triangular_mesh(method_c.mesh()).expect("neutral state");
    assert!(matches!(
        improve_lepp_post_quality(&mut state, &LeppPostQualityConfig::default()),
        Err(LeppPostQualityError::InvalidConfig { .. })
    ));
    let config = post_quality_config(&method_c, 0);
    assert!(matches!(
        improve_lepp_post_quality(&mut state, &config),
        Err(LeppPostQualityError::InvalidConfig { .. })
    ));

    let mut config = post_quality_config(&method_c, 1);
    config.gates.protected_vertices.push(state.vertices().len());
    assert!(matches!(
        improve_lepp_post_quality(&mut state, &config),
        Err(LeppPostQualityError::InvalidConfig { .. })
    ));
    config.maximum_insertions = 1;
    config.maximum_edge_length = Some(f64::NAN);
    assert!(matches!(
        improve_lepp_post_quality(&mut state, &config),
        Err(LeppPostQualityError::InvalidConfig { .. })
    ));
}

#[test]
fn post_quality_no_violations_does_not_touch_the_mesh() {
    let method_c = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 0).expect("base Method-C mesh");
    let original = MeshState::from_triangular_mesh(method_c.mesh()).expect("neutral state");
    let mut state = original.clone();
    let mut config = post_quality_config(&method_c, 3);
    config.maximum_edge_length = Some(f64::MAX);
    let report = improve_lepp_post_quality(&mut state, &config).expect("quality pass");
    assert_eq!(report.stop_reason, LeppPostQualityStopReason::NoViolations);
    assert_eq!(report.attempted, 0);
    assert_eq!(report.committed, 0);
    assert_eq!(state, original);
}

#[test]
fn post_quality_is_deterministic_and_round_trips_after_a_commit() {
    let method_c = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 0).expect("base Method-C mesh");
    let original = MeshState::from_triangular_mesh(method_c.mesh()).expect("neutral state");
    let config = post_quality_config(&method_c, 1);
    let mut first = original.clone();
    let first_report = improve_lepp_post_quality(&mut first, &config).expect("post quality");
    let mut second = original;
    let second_report =
        improve_lepp_post_quality(&mut second, &config).expect("post quality repeat");

    assert_eq!(first_report, second_report);
    assert_eq!(first, second);
    assert_eq!(first_report.committed, 1);
    assert_eq!(first_report.insertions.len(), 1);
    assert!(
        first_report.after.worst_violation < first_report.before.worst_violation
            || first_report.after.total_violation < first_report.before.total_violation
    );
    assert_eq!(first.open_edge_count(), 0);
    first.validate().expect("valid topology");
    assert_eq!(delaunay_violations(&first), 0);

    let rebuilt = first
        .to_triangular_mesh(method_c.mesh().impent, None)
        .expect("Method-C tables");
    rebuilt.validate_topology().expect("valid rebuilt tables");
}

fn post_quality_in_threads(
    original: &MeshState,
    config: &LeppPostQualityConfig,
    threads: usize,
) -> (MeshState, LeppPostQualityReport) {
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("local rayon pool")
        .install(|| {
            let mut state = original.clone();
            let report = improve_lepp_post_quality(&mut state, config).expect("post quality");
            (state, report)
        })
}

#[test]
fn post_quality_parallel_scan_is_thread_count_deterministic() {
    let method_c = MethodCMesh::from_icosahedron(7, 0, 1.0, 0.25, 0).expect("base Method-C mesh");
    let original = MeshState::from_triangular_mesh(method_c.mesh()).expect("neutral state");
    let config = post_quality_config(&method_c, 2);

    let (single_state, single_report) = post_quality_in_threads(&original, &config, 1);
    let (parallel_state, parallel_report) = post_quality_in_threads(&original, &config, 4);

    assert_eq!(single_report, parallel_report);
    assert_eq!(single_state, parallel_state);
}

#[test]
fn post_quality_rejects_unimproving_attempts_without_mutating() {
    let method_c = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 0).expect("base Method-C mesh");
    let original = MeshState::from_triangular_mesh(method_c.mesh()).expect("neutral state");
    let mut state = original.clone();
    let mut config = post_quality_config(&method_c, 2);
    config.gates.maximum_vertex_degree = 3;
    let report = improve_lepp_post_quality(&mut state, &config).expect("all attempts rejected");
    assert_eq!(report.committed, 0);
    assert!(report.rejected > 0);
    assert_eq!(report.rejections.len(), report.rejected);
    assert_eq!(report.attempted, report.rejected);
    assert!(report.attempted <= original.triangle_count());
    assert_eq!(
        report.stop_reason,
        LeppPostQualityStopReason::NoCommittableInsertion
    );
    assert_eq!(state, original);
}

#[test]
fn constrained_lepp_refuses_unlisted_boundary_terminal_without_changes() {
    let original = mesh(
        vec![p(0.0, 0.0), p(120.0, 0.0), p(60.0, 20.0)],
        vec![[2, 3, 4]],
    );
    let mut state = original.clone();
    let mut segments = SegmentList::from_pairs([(3, 4)]);

    let error = insert_lepp_terminal_midpoint_constrained(
        &mut state,
        &mut segments,
        2,
        &default_config(),
        &LeppInsertionGates::default(),
    )
    .expect_err("unprotected terminal");

    assert!(matches!(
        error,
        LeppInsertionError::UnprotectedBoundaryTerminal {
            edge,
            face: 2
        } if edge == LeppEdgeId::new(2, 3)
    ));
    assert_eq!(state, original);
    assert!(segments.contains(3, 4));
    assert!(!segments.contains(2, 3));
}

#[test]
fn constrained_lepp_splits_protected_boundary_terminal_and_segment_marker() {
    let mut state = mesh(
        vec![p(0.0, 0.0), p(120.0, 0.0), p(60.0, 20.0)],
        vec![[2, 3, 4]],
    );
    let mut segments = SegmentList::from_marked_pairs([(2, 3, 42)]);

    let report = insert_lepp_terminal_midpoint_constrained(
        &mut state,
        &mut segments,
        2,
        &default_config(),
        &LeppInsertionGates::default(),
    )
    .expect("protected boundary split");

    assert_eq!(state.vertex_count(), 4);
    assert_eq!(state.triangle_count(), 2);
    assert_eq!(state.open_edge_count(), 4);
    state.validate().expect("valid open mesh");
    assert!(!state.triangles()[MESH_STATE_FIRST_ID..]
        .iter()
        .any(|corners| corners.contains(&2) && corners.contains(&3)));
    assert_eq!(report.requested_edge, LeppEdgeId::new(2, 3));
    assert_eq!(report.split_edge, LeppEdgeId::new(2, 3));
    assert_eq!(report.split_reason, LeppInsertionSplitReason::TerminalEdge);
    assert_eq!(segments.marker(2, report.insertion.site), Some(42));
    assert_eq!(segments.marker(report.insertion.site, 3), Some(42));
    assert_eq!(segments.marker(2, 3), None);
    assert!(matches!(
        report.path.terminal,
        LeppTerminal::Boundary { .. }
    ));
}

#[test]
fn constrained_lepp_rejects_stale_segment_before_mutating() {
    let original = mesh(
        vec![p(0.0, 0.0), p(120.0, 0.0), p(60.0, 20.0)],
        vec![[2, 3, 4]],
    );
    let mut state = original.clone();
    let mut segments = SegmentList::from_pairs([(2, 4), (3, 5)]);
    let before_segments = segments.clone();

    let error = insert_lepp_terminal_midpoint_constrained(
        &mut state,
        &mut segments,
        2,
        &default_config(),
        &LeppInsertionGates::default(),
    )
    .expect_err("stale protected segment");

    assert!(matches!(
        error,
        LeppInsertionError::StaleProtectedSegment { edge } if edge == LeppEdgeId::new(3, 5)
    ));
    assert_eq!(state, original);
    assert_eq!(segments, before_segments);
}

#[test]
fn encroachment_scan_accepts_explicit_segments_outside_path_region() {
    let state = mesh(
        vec![p(0.0, 0.0), p(120.0, 0.0), p(60.0, 20.0)],
        vec![[2, 3, 4]],
    );
    let point = terminal_edge_midpoint(&state, LeppEdgeId::new(3, 4)).expect("midpoint");
    let found = state
        .encroached_segment_edges(point, SegmentList::from_pairs([(3, 4)]).iter())
        .expect("encroaches explicit segment");
    assert_eq!(
        LeppEdgeId::new(found.tail, found.head),
        LeppEdgeId::new(3, 4)
    );
}

#[test]
fn constrained_lepp_rejection_rolls_back_mesh_and_segment_marker() {
    let original = mesh(
        vec![p(0.0, 0.0), p(120.0, 0.0), p(60.0, 20.0)],
        vec![[2, 3, 4]],
    );
    let mut state = original.clone();
    let mut segments = SegmentList::from_marked_pairs([(2, 3, 42)]);
    let original_segments = segments.clone();
    let gates = LeppInsertionGates {
        protected_vertices: vec![4],
        ..LeppInsertionGates::default()
    };

    insert_lepp_terminal_midpoint_constrained(
        &mut state,
        &mut segments,
        2,
        &default_config(),
        &gates,
    )
    .expect_err("protected degree rejects boundary split");

    assert_eq!(state, original);
    assert_eq!(segments, original_segments);
}

fn adaptive_config(
    method_c: &MethodCMesh,
    cycles: usize,
    per_cycle: usize,
) -> AdaptiveHybridConfig {
    AdaptiveHybridConfig {
        max_cycles: cycles,
        target_size_tolerance: 1.0,
        stop_at_source_resolution: true,
        maximum_neighbor_size_ratio: 10.0,
        maximum_vertices: usize::MAX,
        maximum_insertions_per_cycle: per_cycle,
        minimum_triangle_angle: 0.0,
        search: default_config(),
        gates: LeppInsertionGates {
            maximum_vertex_degree: 8,
            protected_vertices: method_c.mesh().impent.to_vec(),
        },
    }
}

fn test_face_center(state: &MeshState, face: usize) -> CartesianPoint {
    let corners = state.triangles()[face];
    let sum = CartesianPoint::new(
        state.vertices()[corners[0]].x
            + state.vertices()[corners[1]].x
            + state.vertices()[corners[2]].x,
        state.vertices()[corners[0]].y
            + state.vertices()[corners[1]].y
            + state.vertices()[corners[2]].y,
        state.vertices()[corners[0]].z
            + state.vertices()[corners[1]].z
            + state.vertices()[corners[2]].z,
    );
    let norm = magnitude(sum);
    CartesianPoint::new(sum.x / norm, sum.y / norm, sum.z / norm)
}

fn adaptive_circle(method_c: &MethodCMesh, level: usize) -> RefinementRegion {
    let state = MeshState::from_triangular_mesh(method_c.mesh()).expect("neutral state");
    RefinementRegion::Circle {
        center: crate::xyz_to_lonlat_degrees(test_face_center(&state, MESH_STATE_FIRST_ID)),
        radius_meters: 3_000_000.0,
        level,
    }
}

#[test]
fn adaptive_hybrid_target_level_converts_from_local_median_edge() {
    let method_c = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 0).expect("base Method-C mesh");
    let state = MeshState::from_triangular_mesh(method_c.mesh()).expect("neutral state");
    let level0 = RefinementRegion::Bbox {
        west_degrees: -180.0,
        east_degrees: 180.0,
        south_degrees: -90.0,
        north_degrees: 90.0,
        level: 0,
    };
    let level2 = RefinementRegion::Bbox {
        west_degrees: -180.0,
        east_degrees: 180.0,
        south_degrees: -90.0,
        north_degrees: 90.0,
        level: 2,
    };

    let h0 = adaptive_hybrid_target_edge_from_level(&state, &level0).expect("level 0 target");
    let h2 = adaptive_hybrid_target_edge_from_level(&state, &level2).expect("level 2 target");

    assert!((h0 / 4.0 - h2).abs() < 1.0e-9);
}

#[test]
fn adaptive_hybrid_respects_explicit_target_edge() {
    let method_c = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 0).expect("base Method-C mesh");
    let original = MeshState::from_triangular_mesh(method_c.mesh()).expect("neutral state");
    let mut state = original.clone();
    let mut demand = AdaptiveHybridDemand::user_region("explicit", adaptive_circle(&method_c, 5));
    demand.target_edge_m = Some(f64::MAX);

    let report = refine_adaptive_hybrid(&mut state, &[demand], &adaptive_config(&method_c, 2, 1))
        .expect("explicit target");

    assert_eq!(state, original);
    assert_eq!(report.stop_reason, AdaptiveHybridStopReason::Satisfied);
    assert_eq!(report.path_stats.committed, 0);
}

#[test]
fn adaptive_hybrid_rejects_invalid_demand_region() {
    let method_c = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 0).expect("base Method-C mesh");
    let mut state = MeshState::from_triangular_mesh(method_c.mesh()).expect("neutral state");
    let demand = AdaptiveHybridDemand::user_region(
        "bad-level",
        RefinementRegion::Circle {
            center: LonLatDegrees::new(0.0, 0.0),
            radius_meters: 1_000.0,
            level: 0,
        },
    );

    assert!(matches!(
        refine_adaptive_hybrid(&mut state, &[demand], &adaptive_config(&method_c, 1, 1)),
        Err(AdaptiveHybridError::InvalidConfig { .. })
    ));
}

#[test]
fn adaptive_hybrid_refines_region_and_reports_physical_insertions() {
    let method_c = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 0).expect("base Method-C mesh");
    let original = MeshState::from_triangular_mesh(method_c.mesh()).expect("neutral state");
    let mut state = original.clone();
    let demand = AdaptiveHybridDemand::physical_region("circle", adaptive_circle(&method_c, 1));
    assert!(matches!(
        &demand.cause,
        earthmesh_refine::RefinementCause::PhysicalCriterion { criterion_id }
            if criterion_id == "circle"
    ));

    let report = refine_adaptive_hybrid(&mut state, &[demand], &adaptive_config(&method_c, 3, 2))
        .expect("adaptive hybrid");

    assert!(report.insertion_counts.physical > 0, "{report:?}");
    assert_eq!(report.path_stats.committed, report.insertions.len());
    assert!(state.vertex_count() > original.vertex_count());
    assert_eq!(state.open_edge_count(), 0);
    state.validate().expect("valid adaptive state");
}

#[test]
fn adaptive_hybrid_stops_after_multiple_limited_cycles_without_dropping_hard_demands() {
    let method_c = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 0).expect("base Method-C mesh");
    let mut state = MeshState::from_triangular_mesh(method_c.mesh()).expect("neutral state");
    let demand = AdaptiveHybridDemand::user_region("hard", adaptive_circle(&method_c, 3));

    let report = refine_adaptive_hybrid(&mut state, &[demand], &adaptive_config(&method_c, 2, 1))
        .expect("adaptive hybrid");

    assert_eq!(report.cycles, 2);
    assert_eq!(report.stop_reason, AdaptiveHybridStopReason::MaxCycles);
    assert_eq!(report.path_stats.committed, 2);
    assert!(report.unresolved_demands.iter().any(|d| d.hard));
}

#[test]
fn adaptive_hybrid_reports_source_resolution_unresolved() {
    let method_c = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 0).expect("base Method-C mesh");
    let original = MeshState::from_triangular_mesh(method_c.mesh()).expect("neutral state");
    let mut state = original.clone();
    let mut demand = AdaptiveHybridDemand::user_region("source", adaptive_circle(&method_c, 2));
    demand.source_resolution_m = Some(f64::MAX);

    let report = refine_adaptive_hybrid(&mut state, &[demand], &adaptive_config(&method_c, 2, 1))
        .expect("adaptive hybrid");

    assert_eq!(state, original);
    assert!(report
        .unresolved_demands
        .iter()
        .any(|d| d.reason == AdaptiveHybridUnresolvedReason::SourceResolution && d.hard));
}

#[test]
fn adaptive_hybrid_does_not_report_source_floor_for_an_already_satisfied_face() {
    let method_c = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 0).expect("base Method-C mesh");
    let original = MeshState::from_triangular_mesh(method_c.mesh()).expect("neutral state");
    let mut state = original.clone();
    let mut demand = AdaptiveHybridDemand::user_region("source", adaptive_circle(&method_c, 1));
    demand.source_resolution_m = Some(f64::MAX);
    demand.target_edge_m = Some(f64::MAX);
    let mut config = adaptive_config(&method_c, 2, 1);
    config.target_size_tolerance = 1.0e6;

    let report = refine_adaptive_hybrid(&mut state, &[demand], &config).expect("adaptive hybrid");

    assert_eq!(state, original);
    assert_eq!(report.stop_reason, AdaptiveHybridStopReason::Satisfied);
    assert!(report.unresolved_demands.is_empty());
}

fn adaptive_hybrid_in_threads(
    original: &MeshState,
    demand: &AdaptiveHybridDemand,
    config: &AdaptiveHybridConfig,
    threads: usize,
) -> (MeshState, AdaptiveHybridReport) {
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("local rayon pool")
        .install(|| {
            let mut state = original.clone();
            let report = refine_adaptive_hybrid(&mut state, std::slice::from_ref(demand), config)
                .expect("adaptive hybrid");
            (state, report)
        })
}

#[test]
fn adaptive_hybrid_parallel_scan_is_thread_count_deterministic() {
    let method_c = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 0).expect("base Method-C mesh");
    let original = MeshState::from_triangular_mesh(method_c.mesh()).expect("neutral state");
    let demand = AdaptiveHybridDemand::user_region("repeat", adaptive_circle(&method_c, 1));
    let config = adaptive_config(&method_c, 2, 2);

    let (single_state, single_report) = adaptive_hybrid_in_threads(&original, &demand, &config, 1);
    let (parallel_state, parallel_report) =
        adaptive_hybrid_in_threads(&original, &demand, &config, 4);

    assert_eq!(single_report, parallel_report);
    assert_eq!(single_state, parallel_state);
}

#[test]
fn constrained_lepp_splits_encroached_segment_before_terminal_boundary() {
    let mut state = mesh(
        vec![p(0.0, 0.0), p(60.0, 0.0), p(15.0, 5.0)],
        vec![[2, 3, 4]],
    );
    let mut segments = SegmentList::from_marked_pairs([(3, 4, 7)]);

    let report = insert_lepp_terminal_midpoint_constrained(
        &mut state,
        &mut segments,
        2,
        &default_config(),
        &LeppInsertionGates::default(),
    )
    .expect("encroached segment split");

    assert_eq!(report.insertion.site, 5);
    assert!(
        matches!(report.path.terminal, LeppTerminal::Boundary { edge, face: 2 } if edge == LeppEdgeId::new(2, 3))
    );
    assert!(state.triangles()[MESH_STATE_FIRST_ID..]
        .iter()
        .any(|corners| corners.contains(&3) && corners.contains(&report.insertion.site)));
    assert!(state.triangles()[MESH_STATE_FIRST_ID..]
        .iter()
        .any(|corners| corners.contains(&4) && corners.contains(&report.insertion.site)));
    assert_eq!(segments.marker(3, report.insertion.site), Some(7));
    assert_eq!(segments.marker(report.insertion.site, 4), Some(7));
    assert_eq!(segments.marker(3, 4), None);
    assert!(segments.contains(3, report.insertion.site));
    assert!(
        !segments.contains(2, 3),
        "terminal demand waits for the next pass"
    );
}

#[test]
fn constrained_lepp_restores_segments_when_encroached_split_is_rejected() {
    let original = mesh(
        vec![p(0.0, 0.0), p(60.0, 0.0), p(15.0, 5.0)],
        vec![[2, 3, 4]],
    );
    let mut state = original.clone();
    let mut segments = SegmentList::from_marked_pairs([(3, 4, 7)]);
    let original_segments = segments.clone();

    let error = super::insertion::insert_lepp_terminal_midpoint_constrained_with_postcondition(
        &mut state,
        &mut segments,
        2,
        &default_config(),
        &LeppInsertionGates::default(),
        |_, _| false,
    )
    .expect_err("forced rejection");

    assert!(matches!(
        error,
        LeppInsertionError::Transaction(InsertionTransactionError::Rejected)
    ));
    assert_eq!(state, original);
    assert_eq!(segments, original_segments);
}

#[test]
fn adaptive_hybrid_tiny_circle_uses_representative_face_when_no_centroid_is_inside() {
    let method_c = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 0).expect("base Method-C mesh");
    let original = MeshState::from_triangular_mesh(method_c.mesh()).expect("neutral state");
    let center = crate::xyz_to_lonlat_degrees(test_face_center(&original, MESH_STATE_FIRST_ID));
    let mut state = original.clone();
    let demand = AdaptiveHybridDemand::user_region(
        "tiny",
        RefinementRegion::Circle {
            center,
            radius_meters: 1.0,
            level: 1,
        },
    );

    let mut config = adaptive_config(&method_c, 1, 1);
    config.gates.protected_vertices.clear();
    let report =
        refine_adaptive_hybrid(&mut state, &[demand], &config).expect("adaptive tiny circle");

    assert_eq!(report.path_stats.committed, 1, "{report:?}");
    assert!(state.vertex_count() > original.vertex_count());
}

#[test]
fn adaptive_hybrid_constrained_refines_a_fully_protected_open_boundary() {
    let mut state = mesh(
        vec![p(0.0, 0.0), p(120.0, 0.0), p(60.0, 20.0)],
        vec![[2, 3, 4]],
    );
    let center = crate::xyz_to_lonlat_degrees(test_face_center(&state, MESH_STATE_FIRST_ID));
    let mut demand = AdaptiveHybridDemand::user_region(
        "regional-boundary",
        RefinementRegion::Circle {
            center,
            radius_meters: 10.0,
            level: 1,
        },
    );
    demand.cause = earthmesh_refine::RefinementCause::BoundaryResolution;
    let mut segments = SegmentList::from_marked_pairs([(2, 3, 9), (3, 4, 9), (4, 2, 9)]);
    let config = AdaptiveHybridConfig {
        max_cycles: 1,
        target_size_tolerance: 1.0,
        maximum_neighbor_size_ratio: 10.0,
        maximum_insertions_per_cycle: 1,
        minimum_triangle_angle: 0.0,
        search: default_config(),
        ..AdaptiveHybridConfig::default()
    };

    let report = refine_adaptive_hybrid_constrained(&mut state, &mut segments, &[demand], &config)
        .expect("constrained adaptive hybrid");

    assert_eq!(report.insertion_counts.boundary, 1, "{report:?}");
    assert_eq!(state.vertex_count(), 4);
    assert_eq!(state.open_edge_count(), 4);
    assert_eq!(segments.iter().count(), 4);
    state.validate().expect("valid refined open mesh");
}

#[test]
fn adaptive_hybrid_constrained_rejects_an_unprotected_open_boundary() {
    let original = mesh(
        vec![p(0.0, 0.0), p(120.0, 0.0), p(60.0, 20.0)],
        vec![[2, 3, 4]],
    );
    let mut state = original.clone();
    let mut segments = SegmentList::from_pairs([(2, 3)]);
    let error = refine_adaptive_hybrid_constrained(
        &mut state,
        &mut segments,
        &[],
        &AdaptiveHybridConfig::default(),
    )
    .expect_err("unprotected open edge");

    assert!(matches!(error, AdaptiveHybridError::InvalidMesh { .. }));
    assert_eq!(state, original);
}

#[test]
fn adaptive_hybrid_constrained_rejects_a_phantom_segment() {
    let original = mesh(
        vec![p(0.0, 0.0), p(120.0, 0.0), p(60.0, 20.0)],
        vec![[2, 3, 4]],
    );
    let mut state = original.clone();
    let mut segments = SegmentList::from_pairs([(2, 3), (3, 4), (4, 2), (2, 99)]);

    let error = refine_adaptive_hybrid_constrained(
        &mut state,
        &mut segments,
        &[],
        &AdaptiveHybridConfig::default(),
    )
    .expect_err("phantom segment");

    assert!(matches!(error, AdaptiveHybridError::InvalidMesh { .. }));
    assert_eq!(state, original);
}

#[test]
fn adaptive_hybrid_report_bounds_details_but_keeps_the_exact_count() {
    let empty_report = || AdaptiveHybridReport {
        cycles: 0,
        insertions: Vec::new(),
        insertion_counts: AdaptiveHybridInsertionCounts::default(),
        path_stats: AdaptiveHybridPathStats::default(),
        initial_vertices: 0,
        final_vertices: 0,
        initial_faces: 0,
        final_faces: 0,
        target_satisfaction: AdaptiveHybridTargetSatisfaction::default(),
        unresolved_demand_count: 0,
        unresolved_demands: Vec::new(),
        rejections: Vec::new(),
        stop_reason: AdaptiveHybridStopReason::Satisfied,
    };
    let mut report = empty_report();
    let mut reverse = empty_report();

    for index in 0..=1024 {
        let detail = AdaptiveHybridUnresolvedDemand {
            criterion_id: format!("demand-{index}"),
            face: None,
            hard: true,
            reason: AdaptiveHybridUnresolvedReason::Limit,
            message: "test".to_string(),
        };
        report.add_unresolved_demand(detail);
    }
    for index in (0..=1024).rev() {
        reverse.add_unresolved_demand(AdaptiveHybridUnresolvedDemand {
            criterion_id: format!("demand-{index}"),
            face: None,
            hard: true,
            reason: AdaptiveHybridUnresolvedReason::Limit,
            message: "test".to_string(),
        });
    }

    assert_eq!(report.unresolved_demand_count, 1025);
    assert_eq!(report.unresolved_demands.len(), 1024);
    assert_eq!(report.unresolved_demands, reverse.unresolved_demands);
}

#[test]
fn adaptive_hybrid_quality_insertions_strictly_improve_the_quality_objective() {
    let method_c = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 0).expect("base Method-C mesh");
    let mut state = MeshState::from_triangular_mesh(method_c.mesh()).expect("neutral state");
    let original = state.clone();
    let mut config = adaptive_config(&method_c, 1, 1);
    config.minimum_triangle_angle = 59.0;
    let quality = LeppPostQualityConfig {
        minimum_spherical_triangle_angle_degrees: Some(config.minimum_triangle_angle),
        ..LeppPostQualityConfig::default()
    };
    let before = super::post_quality::quality_snapshot(&state, &quality).expect("before quality");

    let report = refine_adaptive_hybrid(&mut state, &[], &config).expect("quality-only adaptive");
    let after = super::post_quality::quality_snapshot(&state, &quality).expect("after quality");

    assert!(report.path_stats.attempted > 0, "{report:?}");
    if report.insertion_counts.quality > 0 {
        assert!(super::post_quality::strictly_improves_quality_snapshot(
            after, before
        ));
    } else {
        assert_eq!(state, original);
        assert!(report.path_stats.rejected > 0, "{report:?}");
    }
}
