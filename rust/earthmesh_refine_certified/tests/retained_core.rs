use earthmesh_refine_certified::{
    coarsen::{
        condense_hierarchy_core, n6_legacy_mixed_fixture, plan_retained_core_subsets,
        remaining_connected_retained_core_candidates, retained_core_ladder_required,
        retained_core_search_plan_json, retained_core_topology_evidence_json,
        solve_retained_core_topology, RetainedCoreCorridorFamily, RetainedCoreTopologyLimits,
        RetainedCoreTopologyOutcomeKind,
    },
    mesh_fingerprint,
};
use std::collections::BTreeSet;

const RCR_TASKBOOK_SHA256: &str =
    "f5a4509d879de425108bdff0c58bed8f9d64bf5c76297a097af7eca8d361d0ce";

fn frozen_plan() -> earthmesh_refine_certified::coarsen::RetainedCoreSearchPlan {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let violation_parents = BTreeSet::from([component.core_parents[0]]);
    plan_retained_core_subsets(
        &source,
        &component.core_parents.iter().copied().collect(),
        &violation_parents,
    )
    .unwrap()
}

#[test]
fn n6_has_expected_initial_coarse_parent_count() {
    assert_eq!(frozen_plan().initial_coarse_parents.len(), 10);
}

#[test]
fn single_release_candidates_are_complete() {
    let plan = frozen_plan();
    let candidates = plan
        .candidates
        .iter()
        .filter(|candidate| candidate.released_parents.len() == 1)
        .collect::<Vec<_>>();
    assert_eq!(candidates.len(), 10);
    assert_eq!(
        candidates
            .iter()
            .flat_map(|candidate| candidate.released_parents.iter().copied())
            .collect::<BTreeSet<_>>(),
        plan.initial_coarse_parents
    );
}

#[test]
fn single_release_order_uses_violation_distance() {
    let plan = frozen_plan();
    let first = plan
        .candidates
        .iter()
        .find(|candidate| candidate.released_parents.len() == 1)
        .unwrap();
    assert_eq!(first.violation_influence_score, 1.0);
}

#[test]
fn pair_release_candidates_are_complete() {
    let plan = frozen_plan();
    let released_pairs = plan
        .candidates
        .iter()
        .filter(|candidate| candidate.released_parents.len() == 2)
        .map(|candidate| candidate.released_parents.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(released_pairs.len(), 45);
}

#[test]
fn remaining_connected_subsets_are_complete() {
    let plan = frozen_plan();
    let mut by_cardinality = std::collections::BTreeMap::new();
    for candidate in remaining_connected_retained_core_candidates(&plan) {
        *by_cardinality
            .entry(candidate.retained_parents.len())
            .or_insert(0usize) += 1;
    }
    assert_eq!(
        by_cardinality,
        std::collections::BTreeMap::from([
            (1, 10),
            (2, 11),
            (3, 14),
            (4, 20),
            (5, 30),
            (6, 35),
            (7, 34),
        ])
    );
    assert_eq!(by_cardinality.values().sum::<usize>(), 154);
}

#[test]
fn pr79_trigger_depends_on_strict_success_not_candidate_existence() {
    assert!(!frozen_plan().candidates.is_empty());
    assert!(retained_core_ladder_required(false));
    assert!(!retained_core_ladder_required(true));
}

#[test]
fn retain_one_candidate_is_tested() {
    let plan = frozen_plan();
    assert_eq!(
        remaining_connected_retained_core_candidates(&plan)
            .into_iter()
            .filter(|candidate| candidate.retained_parents.len() == 1)
            .count(),
        10
    );
    assert_eq!(RetainedCoreCorridorFamily::ALL.len(), 6);
}

#[test]
fn connected_subset_order_is_stable() {
    let first = frozen_plan();
    let second = frozen_plan();
    assert_eq!(first, second);
    assert_eq!(
        retained_core_search_plan_json(&first),
        retained_core_search_plan_json(&second)
    );
    assert!(first
        .candidates
        .windows(2)
        .all(|pair| pair[0].retained_parents.len() >= pair[1].retained_parents.len()));
}

#[test]
fn empty_retained_set_equals_safe_mother() {
    let (source, _) = n6_legacy_mixed_fixture().unwrap();
    let plan = frozen_plan();
    let empty = plan
        .candidates
        .iter()
        .find(|candidate| candidate.retained_parents.is_empty())
        .unwrap();
    assert_eq!(empty.released_parents, plan.initial_coarse_parents);
    let rebuilt = condense_hierarchy_core(&source, &[]).unwrap();
    assert_eq!(
        mesh_fingerprint(&rebuilt.mesh.mesh),
        mesh_fingerprint(&source.mesh)
    );
}

#[test]
fn candidate_rebuild_never_calls_sector_promotion_provenance() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let candidate = frozen_plan()
        .candidates
        .into_iter()
        .find(|candidate| candidate.released_parents.len() == 1)
        .unwrap();
    let outcome = solve_retained_core_topology(
        &source,
        &component,
        &candidate,
        RetainedCoreTopologyLimits {
            face_band_states: 1,
            topology_states: 1,
        },
    );
    assert_ne!(
        outcome.evidence().topology_outcome,
        RetainedCoreTopologyOutcomeKind::InvalidInput,
        "{outcome:?}"
    );
}

#[test]
#[ignore = "explicit Frozen N6 PR70 single-release topology matrix"]
fn frozen_n6_pr70_single_release_topology_probe() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let plan = frozen_plan();
    let outcomes = plan
        .candidates
        .iter()
        .filter(|candidate| candidate.released_parents.len() == 1)
        .map(|candidate| {
            solve_retained_core_topology(
                &source,
                &component,
                candidate,
                RetainedCoreTopologyLimits {
                    face_band_states: 1_000_000,
                    topology_states: 10_000,
                },
            )
        })
        .collect::<Vec<_>>();
    assert!(outcomes.iter().all(|outcome| {
        outcome.evidence().topology_outcome != RetainedCoreTopologyOutcomeKind::InvalidInput
    }));
    let matrix = outcomes
        .iter()
        .map(|outcome| retained_core_topology_evidence_json(outcome.evidence()))
        .collect::<Vec<_>>()
        .join(",");
    let json = format!(
        "{{\"schema_version\":1,\"probe\":\"FrozenN6Pr70SingleReleaseTopology\",\"taskbook_sha256\":\"{RCR_TASKBOOK_SHA256}\",\"matrix\":[{matrix}]}}"
    );
    if let Ok(path) = std::env::var("EARTHMESH_RETAINED_CORE_JSON") {
        std::fs::write(path, &json).unwrap();
    }
    eprintln!("{json}");
}

#[test]
#[ignore = "explicit Frozen N6 PR71 pair-release topology matrix"]
fn frozen_n6_pr71_pair_release_topology_probe() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let candidates = frozen_plan()
        .candidates
        .into_iter()
        .filter(|candidate| candidate.released_parents.len() == 2)
        .collect::<Vec<_>>();
    let chunk_size = candidates.len().div_ceil(4);
    let outcomes = std::thread::scope(|scope| {
        let handles = candidates
            .chunks(chunk_size)
            .map(|chunk| {
                let source = &source;
                let component = &component;
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(|candidate| {
                            solve_retained_core_topology(
                                source,
                                component,
                                candidate,
                                RetainedCoreTopologyLimits {
                                    face_band_states: 1_000_000,
                                    topology_states: 10_000,
                                },
                            )
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(outcomes.len(), 45);
    assert!(candidates
        .iter()
        .zip(&outcomes)
        .all(|(candidate, outcome)| {
            outcome.evidence().topology_outcome != RetainedCoreTopologyOutcomeKind::InvalidInput
                || candidate.retained_components > 1
        }));
    let exact_no_solution = outcomes
        .iter()
        .filter(|outcome| {
            outcome.evidence().topology_outcome
                == RetainedCoreTopologyOutcomeKind::TopologyFamilyExhaustedNoSolution
        })
        .count();
    let search_incomplete = outcomes
        .iter()
        .filter(|outcome| {
            outcome.evidence().topology_outcome
                == RetainedCoreTopologyOutcomeKind::SearchBudgetExhausted
        })
        .count();
    let closed = outcomes
        .iter()
        .filter(|outcome| {
            outcome.evidence().topology_outcome == RetainedCoreTopologyOutcomeKind::Closed
        })
        .count();
    assert_eq!((exact_no_solution, search_incomplete, closed), (41, 4, 0));
    let matrix = outcomes
        .iter()
        .map(|outcome| retained_core_topology_evidence_json(outcome.evidence()))
        .collect::<Vec<_>>()
        .join(",");
    let json = format!(
        "{{\"schema_version\":1,\"probe\":\"FrozenN6Pr71PairReleaseTopology\",\"taskbook_sha256\":\"{RCR_TASKBOOK_SHA256}\",\"exact_no_solution\":{exact_no_solution},\"search_incomplete\":{search_incomplete},\"topology_closed\":{closed},\"geometry_attempted\":0,\"matrix\":[{matrix}]}}"
    );
    if let Ok(path) = std::env::var("EARTHMESH_RETAINED_CORE_JSON") {
        std::fs::write(path, &json).unwrap();
    }
    eprintln!("{json}");
}

#[test]
#[ignore = "explicit Frozen N6 PR69 retained-core subset matrix"]
fn frozen_n6_pr69_retained_core_subset_probe() {
    let plan = frozen_plan();
    let json = format!(
        "{{\"schema_version\":1,\"probe\":\"FrozenN6Pr69RetainedCoreSubsets\",\"taskbook_sha256\":\"{RCR_TASKBOOK_SHA256}\",\"gate\":\"CompleteDeterministicSubsetLadder\",\"plan\":{}}}",
        retained_core_search_plan_json(&plan),
    );
    if let Ok(path) = std::env::var("EARTHMESH_RETAINED_CORE_JSON") {
        std::fs::write(path, &json).unwrap();
    }
    eprintln!("{json}");
}
