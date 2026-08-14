use super::*;
use crate::{CartesianPoint, MESH_STATE_FIRST_ID};

fn point(x: f64, y: f64, z: f64) -> CartesianPoint {
    CartesianPoint::new(x, y, z)
}

fn octahedron() -> MeshState {
    let ring_radius = 0.99_f64.sqrt();
    MeshState::from_parts(
        vec![
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 1.0),
            point(ring_radius, 0.0, 0.1),
            point(0.0, ring_radius, 0.1),
            point(-ring_radius, 0.0, 0.1),
            point(0.0, -ring_radius, 0.1),
            point(0.0, 0.0, -1.0),
        ],
        vec![
            [1, 1, 1],
            [1, 1, 1],
            [2, 3, 4],
            [2, 4, 5],
            [2, 5, 6],
            [2, 6, 3],
            [7, 4, 3],
            [7, 5, 4],
            [7, 6, 5],
            [7, 3, 6],
        ],
    )
    .expect("octahedron")
}

fn pentagonal_bipyramid() -> MeshState {
    let ring_radius = 0.99_f64.sqrt();
    let mut vertices = vec![
        point(0.0, 0.0, 0.0),
        point(0.0, 0.0, 0.0),
        point(0.0, 0.0, 1.0),
    ];
    for i in 0..5 {
        let angle = std::f64::consts::TAU * i as f64 / 5.0;
        vertices.push(point(
            ring_radius * angle.cos(),
            ring_radius * angle.sin(),
            0.1,
        ));
    }
    vertices.push(point(0.0, 0.0, -1.0));
    MeshState::from_parts(
        vertices,
        vec![
            [1, 1, 1],
            [1, 1, 1],
            [2, 3, 4],
            [2, 4, 5],
            [2, 5, 6],
            [2, 6, 7],
            [2, 7, 3],
            [8, 4, 3],
            [8, 5, 4],
            [8, 6, 5],
            [8, 7, 6],
            [8, 3, 7],
        ],
    )
    .expect("pentagonal bipyramid")
}

fn euler(state: &MeshState) -> isize {
    state.vertex_count() as isize - (state.triangle_count() * 3 / 2) as isize
        + state.triangle_count() as isize
}

#[test]
fn polygon_triangulation_enumeration_is_complete() {
    for (degree, expected) in [(4, 2), (5, 5), (6, 14), (7, 42)] {
        let ring = (0..degree).collect::<Vec<_>>();
        let candidates = triangulations(&ring);
        assert_eq!(candidates.len(), expected, "degree {degree}");
        assert!(candidates
            .iter()
            .all(|candidate| candidate.len() == degree - 2));
    }
}

#[test]
fn a_degenerate_replacement_face_is_rejected() {
    let state = octahedron();
    let corners = [2, 3, 5];

    assert_eq!(
        oriented(&state, Sign::Positive, corners),
        Err(RetirementError::DegenerateCandidate { corners })
    );
}

#[test]
fn degree_four_retirement_closes_the_quad_and_keeps_euler() {
    let mut state = octahedron();
    let before_v = state.vertex_count();
    let before_f = state.triangle_count();

    let report = state
        .retire_degree_four_vertex_transactionally(2, |_, _| true)
        .expect("retire top vertex");

    assert_eq!(state.vertex_count(), before_v - 1);
    assert_eq!(state.triangle_count(), before_f - 2);
    assert_eq!(
        state.vertices().len(),
        8,
        "no compaction or appended vertex"
    );
    assert_eq!(
        state.triangles().len(),
        10,
        "no compaction or appended face"
    );
    assert_eq!(euler(&state), 2);
    assert_eq!(state.open_edge_count(), 0);
    assert_eq!(report.reused_faces.len(), 2);
    assert_eq!(report.retired_faces.len(), 2);
    assert_eq!(report.retired_face_ids.len(), 2);
    assert!(!state.is_vertex_live(2));
    assert!(!state.contains_vertex_id(report.vertex_id));
    for id in &report.retired_face_ids {
        assert!(!state.contains_face_id(*id));
    }
    for face in report.retired_faces {
        assert!(!state.is_triangle_live(face));
    }
    state.validate().expect("valid after retirement");
}

#[test]
fn postcondition_rejection_leaves_the_mesh_byte_for_byte_equal() {
    let mut state = octahedron();
    let before = state.clone();

    let error = state
        .retire_degree_four_vertex_transactionally(2, |_, _| false)
        .expect_err("postcondition rejects both diagonals");

    assert_eq!(error, RetirementError::Rejected);
    assert_eq!(state, before);
}

#[test]
fn non_degree_four_vertices_are_refused_atomically() {
    let mut state = MeshState::from_parts(
        vec![
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(1.0, 1.0, 1.0),
            point(1.0, -1.0, -1.0),
            point(-1.0, 1.0, -1.0),
            point(-1.0, -1.0, 1.0),
        ],
        vec![
            [1, 1, 1],
            [1, 1, 1],
            [2, 3, 4],
            [2, 4, 5],
            [2, 5, 3],
            [3, 5, 4],
        ],
    )
    .expect("tetrahedron");
    let before = state.clone();

    let error = state
        .retire_degree_four_vertex_transactionally(2, |_, _| true)
        .expect_err("tetrahedron vertices have degree three");

    assert!(matches!(
        error,
        RetirementError::NotDegreeFour {
            vertex: 2,
            degree: 3
        }
    ));
    assert_eq!(state, before);
}

#[test]
fn slots_outside_the_local_patch_do_not_change() {
    let mut state = octahedron();
    let before = state.clone();
    let report = state
        .retire_degree_four_vertex_transactionally(2, |_, _| true)
        .expect("retire");
    let changed: std::collections::BTreeSet<_> = report
        .reused_faces
        .iter()
        .chain(report.retired_faces.iter())
        .copied()
        .collect();

    for vertex in MESH_STATE_FIRST_ID..before.vertices().len() {
        if vertex != 2 {
            assert_eq!(state.vertices()[vertex], before.vertices()[vertex]);
            assert_eq!(state.vertex_id(vertex), before.vertex_id(vertex));
        }
    }
    for face in MESH_STATE_FIRST_ID..before.triangles().len() {
        if !changed.contains(&face) {
            assert_eq!(state.triangles()[face], before.triangles()[face]);
            assert_eq!(state.face_id(face), before.face_id(face));
        }
    }
}

#[test]
fn degree_five_retirement_reuses_three_slots_and_tombstones_two() {
    let mut state = pentagonal_bipyramid();
    let before_v = state.vertex_count();
    let before_f = state.triangle_count();

    let report = state
        .retire_vertex_transactionally(2, |state, report| {
            report.replacement_faces.len() == 3 && state.validate().is_ok()
        })
        .expect("retire degree-five top vertex");

    assert_eq!(state.vertex_count(), before_v - 1);
    assert_eq!(state.triangle_count(), before_f - 2);
    assert_eq!(euler(&state), 2);
    assert_eq!(state.open_edge_count(), 0);
    assert_eq!(report.fan.len(), 5);
    assert_eq!(report.ring.len(), 5);
    assert_eq!(report.reused_faces.len(), 3);
    assert_eq!(report.retired_faces.len(), 2);
    assert_eq!(report.replacement_faces.len(), 3);
    assert_eq!(report.diagonal, None);
    assert!(!state.is_vertex_live(2));
    state
        .validate()
        .expect("valid after degree-five retirement");
}

#[test]
fn degree_five_postcondition_rejection_is_atomic() {
    let mut state = pentagonal_bipyramid();
    let before = state.clone();

    let error = state
        .retire_vertex_transactionally(2, |_, _| false)
        .expect_err("postcondition rejects every triangulation");

    assert_eq!(error, RetirementError::Rejected);
    assert_eq!(state, before);
}
