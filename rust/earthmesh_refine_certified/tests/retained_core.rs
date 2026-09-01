use earthmesh_refine_certified::{
    coarsen::{
        condense_hierarchy_core, n6_legacy_mixed_fixture, plan_retained_core_subsets,
        retained_core_search_plan_json,
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
