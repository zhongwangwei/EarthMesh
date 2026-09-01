use earthmesh_refine_certified::coarsen::{
    build_face_band_problem_with_source_face_rings, essential_cycle_problem_key,
    face_band_problem_identity_report_json, n6_legacy_mixed_fixture,
    profile_frozen_n6_face_band_problems, AnchorBandPolicy, RetainedCoreCorridorFamily,
};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn anchor_policy_changes_key() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let problem =
        build_face_band_problem_with_source_face_rings(&source, &component, 2, 0).unwrap();
    let mut changed = problem.clone();
    let anchor = *problem.anchor_policies.keys().next().unwrap();
    changed
        .anchor_policies
        .insert(anchor, AnchorBandPolicy::OnSingleInterface);
    assert_ne!(key(&source, &problem), key(&source, &changed));
}

#[test]
fn source_face_ring_changes_key() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let first = build_face_band_problem_with_source_face_rings(&source, &component, 2, 0).unwrap();
    let second = build_face_band_problem_with_source_face_rings(&source, &component, 2, 1).unwrap();
    assert_ne!(key(&source, &first), key(&source, &second));
}

#[test]
fn retained_parent_changes_key() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let problem =
        build_face_band_problem_with_source_face_rings(&source, &component, 2, 0).unwrap();
    let first = essential_cycle_problem_key(
        &source,
        &problem,
        component.core_parents.iter().copied(),
        RetainedCoreCorridorFamily::F0CurrentSourceFaceCorridor,
    )
    .unwrap();
    let second = essential_cycle_problem_key(
        &source,
        &problem,
        component.core_parents.iter().copied().skip(1),
        RetainedCoreCorridorFamily::F0CurrentSourceFaceCorridor,
    )
    .unwrap();
    assert_ne!(first, second);
}

#[test]
fn runtime_slots_do_not_change_key() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let problem =
        build_face_band_problem_with_source_face_rings(&source, &component, 2, 0).unwrap();
    let remapped = remap_face_slots(&problem, 10_000);
    assert_eq!(key(&source, &problem), key(&source, &remapped));
}

#[test]
fn different_graphs_never_share_exact_key() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let problem =
        build_face_band_problem_with_source_face_rings(&source, &component, 2, 0).unwrap();
    let mut changed = problem.clone();
    let left = *changed.face_adjacency.keys().next().unwrap();
    let right = changed.face_adjacency[&left][0];
    changed
        .face_adjacency
        .get_mut(&left)
        .unwrap()
        .retain(|neighbour| *neighbour != right);
    changed
        .face_adjacency
        .get_mut(&right)
        .unwrap()
        .retain(|neighbour| *neighbour != left);
    assert_ne!(key(&source, &problem), key(&source, &changed));
}

#[test]
fn bounded_profile_canonicalizes_all_924_attempts() {
    let report = profile_frozen_n6_face_band_problems(0).unwrap();
    assert_eq!(report.attempts, 924);
    assert_eq!(report.canonicalized_attempts, 924);
    assert_eq!(report.corridor_attempts.values().sum::<usize>(), 924);
    assert_eq!(
        report
            .transition_face_count_histogram
            .values()
            .sum::<usize>(),
        924
    );
    assert_eq!(report.frozen_pr84_unknown_attempts, 659);
    assert_eq!(
        report
            .frozen_pr84_unknown_transition_face_count_histogram
            .values()
            .sum::<usize>(),
        659
    );
    assert!(face_band_problem_identity_report_json(&report).contains("\"attempts\":924"));
}

#[test]
fn frozen_full_profile_snapshot_records_the_659_unknowns() {
    let snapshot = include_str!("fixtures/frozen_n6_problem_profile.json");
    assert!(snapshot.contains("\"attempts\":924"));
    assert!(snapshot.contains("\"unique_exact_problem_keys\":924"));
    assert!(snapshot.contains("\"frozen_pr84_unknown_attempts\":659"));
    assert!(snapshot.contains("\"SearchBudgetExhausted\":275"));
}

#[test]
#[ignore = "Alpha6 PR89 full 16,384-state face-band profile"]
fn print_full_frozen_n6_problem_profile() {
    let report = profile_frozen_n6_face_band_problems(16_384).unwrap();
    println!("{}", face_band_problem_identity_report_json(&report));
}

fn key(
    source: &earthmesh_refine_certified::MotherGrid,
    problem: &earthmesh_refine_certified::coarsen::FaceBandProblem,
) -> earthmesh_refine_certified::coarsen::EssentialCycleProblemKey {
    essential_cycle_problem_key(
        source,
        problem,
        [],
        RetainedCoreCorridorFamily::F0CurrentSourceFaceCorridor,
    )
    .unwrap()
}

fn remap_face_slots(
    problem: &earthmesh_refine_certified::coarsen::FaceBandProblem,
    offset: usize,
) -> earthmesh_refine_certified::coarsen::FaceBandProblem {
    let remap = |slot: usize| slot + offset;
    let set = |values: &BTreeSet<usize>| values.iter().map(|slot| remap(*slot)).collect();
    let map_faces = |values: &BTreeMap<usize, Vec<usize>>| {
        values
            .iter()
            .map(|(face, neighbours)| {
                (
                    remap(*face),
                    neighbours.iter().map(|slot| remap(*slot)).collect(),
                )
            })
            .collect()
    };
    let mut out = problem.clone();
    out.transition_faces = problem
        .transition_faces
        .iter()
        .map(|slot| remap(*slot))
        .collect();
    out.coarse_boundary_faces = set(&problem.coarse_boundary_faces);
    out.fine_boundary_faces = set(&problem.fine_boundary_faces);
    out.face_adjacency = map_faces(&problem.face_adjacency);
    out.vertex_incident_faces = problem
        .vertex_incident_faces
        .iter()
        .map(|(vertex, faces)| (*vertex, faces.iter().map(|slot| remap(*slot)).collect()))
        .collect();
    out.face_vertex_neighbours = map_faces(&problem.face_vertex_neighbours);
    out.face_shared_edges = problem
        .face_shared_edges
        .iter()
        .map(|(&(left, right), &edge)| ((remap(left), remap(right)), edge))
        .collect();
    out.face_addresses = problem
        .face_addresses
        .iter()
        .map(|(slot, address)| (remap(*slot), *address))
        .collect();
    out
}
