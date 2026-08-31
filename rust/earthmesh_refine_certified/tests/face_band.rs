use earthmesh_refine_certified::coarsen::{
    build_face_band_problem, face_band_evidence_json, face_band_plan_json, n6_legacy_mixed_fixture,
    solve_exact_face_bands, AnchorBandPolicy, FaceBandLimits, FaceBandProblem,
    FaceBandSolveOutcome,
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
fn pr56_rejects_w3_until_its_exact_constraints_exist() {
    let mut problem = frozen_problem();
    problem.band_count = 3;
    assert!(matches!(
        solve(&problem, FROZEN_BUDGET),
        FaceBandSolveOutcome::InvalidInput { .. }
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

fn frozen_problem() -> FaceBandProblem {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    build_face_band_problem(&source, &component, 2).unwrap()
}

fn solve(problem: &FaceBandProblem, maximum_states: u64) -> FaceBandSolveOutcome {
    solve_exact_face_bands(problem, FaceBandLimits { maximum_states })
}
