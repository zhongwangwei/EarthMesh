use super::*;
use crate::{normalized_face_center, CartesianPoint, MESH_STATE_FIRST_ID};

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

fn tetrahedron() -> MeshState {
    MeshState::from_parts(
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
    .expect("tetrahedron")
}

fn octahedron_with_degree_three_face_split() -> (MeshState, usize) {
    let base = octahedron();
    let face = 2;
    let [a, b, c] = base.triangles()[face];
    let mut vertices = base.vertices().to_vec();
    let mut triangles = base.triangles().to_vec();
    let child = vertices.len();
    vertices.push(
        normalized_face_center(
            base.vertices()[a],
            base.vertices()[b],
            base.vertices()[c],
            base.sphere_radius(),
        )
        .expect("face centre"),
    );
    triangles[face] = [a, b, child];
    triangles.push([b, c, child]);
    triangles.push([c, a, child]);
    (
        MeshState::from_parts(vertices, triangles).expect("split octahedron face"),
        child,
    )
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
    for (degree, expected) in [(3, 1), (4, 2), (5, 5), (6, 14), (7, 42)] {
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
    let mut state = tetrahedron();
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
fn degree_three_retirement_reuses_one_slot_and_tombstones_two() {
    let (mut state, child) = octahedron_with_degree_three_face_split();
    let before_v = state.vertex_count();
    let before_f = state.triangle_count();

    let report = state
        .retire_vertex_transactionally(child, |state, report| {
            report.replacement_faces.len() == 1 && state.validate().is_ok()
        })
        .expect("retire degree-three face split");

    assert_eq!(state.vertex_count(), before_v - 1);
    assert_eq!(state.triangle_count(), before_f - 2);
    assert_eq!(euler(&state), 2);
    assert_eq!(state.open_edge_count(), 0);
    assert_eq!(report.fan.len(), 3);
    assert_eq!(report.ring.len(), 3);
    assert_eq!(report.reused_faces.len(), 1);
    assert_eq!(report.retired_faces.len(), 2);
    assert_eq!(report.replacement_faces.len(), 1);
    assert_eq!(report.diagonal, None);
    assert!(!state.is_vertex_live(child));
    state
        .validate()
        .expect("valid after degree-three retirement");
}

#[test]
fn degree_three_postcondition_rejection_is_atomic() {
    let (mut state, child) = octahedron_with_degree_three_face_split();
    let before = state.clone();

    let error = state
        .retire_vertex_transactionally(child, |_, _| false)
        .expect_err("postcondition rejects the triangle fill");

    assert_eq!(error, RetirementError::Rejected);
    assert_eq!(state, before);
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

#[test]
fn bounded_retirement_distinguishes_budget_from_complete_infeasibility() {
    let mut state = octahedron();
    let before = state.clone();

    let exhausted = state.retire_vertex_with_budget_transactionally(2, 1, |_, _| false);
    assert_eq!(
        exhausted,
        RetirementSearchOutcome::SearchBudgetExhausted { attempted: 1 }
    );
    assert_eq!(state, before);

    let infeasible = state.retire_vertex_with_budget_transactionally(2, 2, |_, _| false);
    assert!(matches!(
        infeasible,
        RetirementSearchOutcome::ProvenInfeasible {
            attempted: 2,
            last_error: Some(RetirementError::Rejected),
        }
    ));
    assert_eq!(state, before);
}

#[test]
fn zero_budget_retirement_never_mutates_and_a_feasible_search_commits() {
    let mut state = octahedron();
    let before = state.clone();

    assert_eq!(
        state.retire_vertex_with_budget_transactionally(2, 0, |_, _| true),
        RetirementSearchOutcome::SearchBudgetExhausted { attempted: 0 }
    );
    assert_eq!(state, before);

    let committed =
        state.retire_vertex_with_budget_transactionally(2, 2, |state, _| state.validate().is_ok());
    assert!(matches!(
        committed,
        RetirementSearchOutcome::Committed { attempted: 1, .. }
    ));
    assert_eq!(state.vertex_count(), before.vertex_count() - 1);
}

#[test]
fn repairing_start_zero_counts_callback_states_without_rewinding_budget() {
    let mut state = octahedron();
    let before = state.clone();
    let mut callbacks = 0;

    let outcome = state.retire_vertex_with_budget_transactionally_repairing(2, 2, |_, _, _| {
        callbacks += 1;
        RetirementPostconditionOutcome::Rejected { states_examined: 1 }
    });

    assert_eq!(
        outcome,
        RetirementSearchOutcome::SearchBudgetExhausted { attempted: 2 }
    );
    assert_eq!(callbacks, 1);
    assert_eq!(state, before);
}

#[test]
fn repairing_cursor_skips_prior_candidates_without_replay() {
    let mut probe = octahedron();
    let mut first_seen = Vec::new();
    let probe_outcome = probe.retire_vertex_from_cursor_with_budget_transactionally_repairing(
        2,
        0,
        1,
        |_, report, _| {
            first_seen.push(report.replacement_faces.clone());
            RetirementPostconditionOutcome::Rejected { states_examined: 0 }
        },
    );
    assert_eq!(
        probe_outcome,
        RetirementSearchOutcome::SearchBudgetExhausted { attempted: 1 }
    );

    let mut state = octahedron();
    let mut seen = Vec::new();
    let outcome = state.retire_vertex_from_cursor_with_budget_transactionally_repairing(
        2,
        1,
        2,
        |_, report, _| {
            seen.push(report.replacement_faces.clone());
            RetirementPostconditionOutcome::Accepted { states_examined: 0 }
        },
    );

    assert!(matches!(
        outcome,
        RetirementSearchOutcome::Committed { attempted: 2, .. }
    ));
    assert_eq!(seen.len(), 1);
    assert_ne!(seen, first_seen);
    state.validate().expect("valid after cursor retirement");
}

#[test]
fn repairing_cursor_equal_to_budget_exhausts_without_mutating() {
    let mut state = octahedron();
    let before = state.clone();

    let outcome = state.retire_vertex_from_cursor_with_budget_transactionally_repairing(
        2,
        1,
        1,
        |_, _, _| panic!("cursor at budget must not examine candidates"),
    );

    assert_eq!(
        outcome,
        RetirementSearchOutcome::SearchBudgetExhausted { attempted: 1 }
    );
    assert_eq!(state, before);
}

#[test]
fn repairing_cursor_past_finite_space_is_infeasible_without_mutating() {
    let mut state = octahedron();
    let before = state.clone();

    let outcome = state.retire_vertex_from_cursor_with_budget_transactionally_repairing(
        2,
        3,
        10,
        |_, _, _| panic!("cursor past finite space must not examine candidates"),
    );

    assert_eq!(
        outcome,
        RetirementSearchOutcome::ProvenInfeasible {
            attempted: 2,
            last_error: None,
        }
    );
    assert_eq!(state, before);
}
