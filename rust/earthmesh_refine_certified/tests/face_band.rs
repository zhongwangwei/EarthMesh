use earthmesh_refine_certified::coarsen::{
    build_face_band_problem, build_face_band_problem_with_source_face_rings,
    face_band_evidence_json, face_band_plan_json, n6_legacy_mixed_fixture, solve_exact_face_bands,
    AnchorBandPolicy, FaceBandLimits, FaceBandProblem, FaceBandSolveOutcome,
};
use std::{collections::BTreeSet, fs, process::Command};

const FROZEN_BUDGET: u64 = 1_000_000;

#[test]
fn frozen_n6_pinch_free_w2_plan_closes_at_f0() {
    let problem = frozen_problem();
    let FaceBandSolveOutcome::Closed(plan, evidence) = solve(&problem, FROZEN_BUDGET) else {
        panic!("Frozen N6 F0 must close");
    };
    assert_eq!(plan.band_count, 2);
    assert_eq!(plan.band_face_counts, vec![36, 52]);
    assert_eq!(
        plan.interface_edges
            .iter()
            .map(Vec::len)
            .collect::<Vec<_>>(),
        vec![20]
    );
    assert_eq!(evidence.true_pinch_count, 0);
    assert_eq!(evidence.cap_faces, 0);
    assert_eq!(evidence.corridor_faces, 0);
    assert_eq!(evidence.core_faces_sacrificed, 0);
    assert_eq!(evidence.states_examined, 45);
    for face in &problem.coarse_boundary_faces {
        assert_eq!(plan.labels[face], 0);
    }
    for face in &problem.fine_boundary_faces {
        assert_eq!(plan.labels[face], 1);
    }
    let interface_vertices = plan.interface_edges[0]
        .iter()
        .flat_map(|edge| [edge.0, edge.1])
        .collect::<BTreeSet<_>>();
    assert!(interface_vertices.is_disjoint(&problem.coarse_boundary_vertices));
    assert!(interface_vertices.is_disjoint(&problem.fine_boundary_vertices));
    for anchor in problem.anchor_policies.keys() {
        let labels = problem.vertex_incident_faces[anchor]
            .iter()
            .map(|face| plan.labels[face])
            .collect::<BTreeSet<_>>();
        assert_eq!(labels.len(), 1);
    }
}

#[test]
fn exact_state_order_and_json_are_deterministic() {
    let problem = frozen_problem();
    let first = solve(&problem, FROZEN_BUDGET);
    let second = solve(&problem, FROZEN_BUDGET);
    assert_eq!(first, second);
    let FaceBandSolveOutcome::Closed(plan, evidence) = first else {
        panic!("Frozen N6 F0 must close");
    };
    assert_eq!(face_band_plan_json(&plan), face_band_plan_json(&plan));
    assert_eq!(
        face_band_evidence_json(&evidence),
        face_band_evidence_json(&evidence)
    );
}

#[test]
fn budget_exhaustion_is_unknown() {
    assert!(matches!(
        solve(&frozen_problem(), 0),
        FaceBandSolveOutcome::SearchBudgetExhausted { .. }
    ));
}

#[test]
fn family_exhaustion_is_scoped_to_the_supplied_problem() {
    let mut problem = frozen_problem();
    let forced_conflict = problem
        .vertex_incident_faces
        .iter()
        .find_map(|(&vertex, faces)| {
            (faces
                .iter()
                .any(|face| problem.coarse_boundary_faces.contains(face))
                && faces
                    .iter()
                    .any(|face| problem.fine_boundary_faces.contains(face)))
            .then_some(vertex)
        })
        .expect("Frozen F0 has a boundary-touching shared vertex");
    problem
        .anchor_policies
        .insert(forced_conflict, AnchorBandPolicy::InteriorOfSingleBand);
    assert!(matches!(
        solve(&problem, FROZEN_BUDGET),
        FaceBandSolveOutcome::FamilyExhaustedNoSolution { .. }
    ));
}

#[test]
fn frozen_n6_w3_face_bands_close_at_f2() {
    for rings in 0..2 {
        let FaceBandSolveOutcome::FamilyExhaustedNoSolution { evidence, .. } =
            solve(&frozen_w3_problem(rings), FROZEN_BUDGET)
        else {
            panic!("Frozen N6 W3 F{rings} must close its family as no-solution")
        };
        assert_eq!(evidence.states_examined, 0);
        assert_eq!(evidence.source_face_rings, rings);
    }
    let problem = frozen_w3_problem(2);
    let FaceBandSolveOutcome::Closed(plan, evidence) = solve(&problem, FROZEN_BUDGET) else {
        panic!("Frozen N6 W3 F2 must close")
    };
    assert_eq!(evidence.states_examined, 58);
    assert_eq!(plan.band_face_counts, vec![36, 50, 60]);
    assert_eq!(evidence.interface_edge_counts, vec![20, 26]);
    let interfaces = plan
        .interface_edges
        .iter()
        .map(|edges| {
            edges
                .iter()
                .flat_map(|edge| [edge.0, edge.1])
                .collect::<BTreeSet<_>>()
        })
        .collect::<Vec<_>>();
    assert!(interfaces[0].is_disjoint(&interfaces[1]));
    for vertices in interfaces {
        assert!(vertices.is_disjoint(&problem.coarse_boundary_vertices));
        assert!(vertices.is_disjoint(&problem.fine_boundary_vertices));
    }
    for faces in problem.vertex_incident_faces.values() {
        let labels = faces
            .iter()
            .map(|face| plan.labels[face])
            .collect::<BTreeSet<_>>();
        assert!(labels.last().unwrap() - labels.first().unwrap() <= 1);
    }
    for (&face, neighbours) in &problem.face_adjacency {
        assert!(neighbours
            .iter()
            .all(|neighbour| plan.labels[&face].abs_diff(plan.labels[neighbour]) <= 1));
    }
    for anchor in problem.anchor_policies.keys() {
        assert_eq!(
            problem.vertex_incident_faces[anchor]
                .iter()
                .map(|face| plan.labels[face])
                .collect::<BTreeSet<_>>()
                .len(),
            1
        );
    }
}

#[test]
fn frozen_n6_w3_state_order_is_deterministic_and_budget_is_unknown() {
    let problem = frozen_w3_problem(2);
    assert_eq!(
        solve(&problem, FROZEN_BUDGET),
        solve(&problem, FROZEN_BUDGET)
    );
    assert!(matches!(
        solve(&problem, 0),
        FaceBandSolveOutcome::SearchBudgetExhausted { .. }
    ));
}

#[test]
#[ignore = "explicit Frozen N6 PR56 exact PF-W2 gate"]
fn frozen_n6_pr56_exact_pf_w2_probe() {
    let problem = frozen_problem();
    let FaceBandSolveOutcome::Closed(plan, evidence) = solve(&problem, FROZEN_BUDGET) else {
        panic!("Frozen N6 F0 must close");
    };
    let commit = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
        .map(|value| format!("\"{value}\""))
        .unwrap_or_else(|| "null".into());
    let json = format!(
        "{{\"schema_version\":1,\"probe\":\"FrozenN6Pr56ExactPfW2\",\"commit_sha\":{},\"taskbook_sha256\":\"46d5f8d1ab439ce972186ba50798806b520fe9bdac3f675806d4cd18cff38e2b\",\"plan\":{},\"evidence\":{}}}",
        commit,
        face_band_plan_json(&plan),
        face_band_evidence_json(&evidence),
    );
    if let Ok(path) = std::env::var("EARTHMESH_GEOMETRY_JSON") {
        fs::write(path, &json).unwrap();
    }
    eprintln!("{json}");
}

#[test]
#[ignore = "explicit Frozen N6 PR59 exact W3 face-band gate"]
fn frozen_n6_pr59_exact_w3_probe() {
    let mut attempts = Vec::new();
    for rings in 0..=2 {
        let outcome = solve(&frozen_w3_problem(rings), FROZEN_BUDGET);
        let evidence = match &outcome {
            FaceBandSolveOutcome::Closed(_, evidence)
            | FaceBandSolveOutcome::FamilyExhaustedNoSolution { evidence, .. }
            | FaceBandSolveOutcome::SearchBudgetExhausted { evidence, .. } => evidence,
            FaceBandSolveOutcome::InvalidInput { reason } => panic!("{reason}"),
        };
        let plan = match &outcome {
            FaceBandSolveOutcome::Closed(plan, _) => face_band_plan_json(plan),
            _ => "null".into(),
        };
        attempts.push(format!(
            "{{\"source_face_rings\":{rings},\"plan\":{plan},\"evidence\":{}}}",
            face_band_evidence_json(evidence)
        ));
        if matches!(outcome, FaceBandSolveOutcome::Closed(..)) {
            break;
        }
    }
    let commit = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
        .map(|value| format!("\"{value}\""))
        .unwrap_or_else(|| "null".into());
    let json = format!(
        "{{\"schema_version\":1,\"probe\":\"FrozenN6Pr59ExactW3\",\"commit_sha\":{commit},\"taskbook_sha256\":\"46d5f8d1ab439ce972186ba50798806b520fe9bdac3f675806d4cd18cff38e2b\",\"attempts\":[{}]}}",
        attempts.join(",")
    );
    if let Ok(path) = std::env::var("EARTHMESH_GEOMETRY_JSON") {
        fs::write(path, &json).unwrap();
    }
    eprintln!("{json}");
}

fn frozen_problem() -> FaceBandProblem {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    build_face_band_problem(&source, &component, 2).unwrap()
}

fn frozen_w3_problem(source_face_rings: usize) -> FaceBandProblem {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    build_face_band_problem_with_source_face_rings(&source, &component, 3, source_face_rings)
        .unwrap()
}

fn solve(problem: &FaceBandProblem, maximum_states: u64) -> FaceBandSolveOutcome {
    solve_exact_face_bands(problem, FaceBandLimits { maximum_states })
}
