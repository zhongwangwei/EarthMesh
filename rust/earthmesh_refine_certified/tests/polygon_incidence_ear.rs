use earthmesh_refine_certified::coarsen::{
    cut_annulus_polygon, pier_small_exact_oracle_json, solve_polygon_incidence_witness,
    OccurrenceIncidenceTarget, PolygonIncidenceWitnessOutcome,
};
use std::collections::BTreeSet;

fn known_target() -> OccurrenceIncidenceTarget {
    let lower = vec![0, 1, 2];
    let upper = vec![100, 101, 102];
    let cut = cut_annulus_polygon(&lower, &upper, 0, 0).unwrap();
    OccurrenceIncidenceTarget::new(
        lower,
        upper,
        cut,
        vec![1, 2, 3, 3, 1, 2, 3, 3],
        BTreeSet::new(),
    )
}

#[test]
fn known_occurrence_incidence_has_witness() {
    assert!(matches!(
        solve_polygon_incidence_witness(&known_target(), u64::MAX, None),
        PolygonIncidenceWitnessOutcome::Found { .. }
    ));
}

#[test]
fn malformed_relaxation_target_is_not_a_witness() {
    let mut target = known_target();
    target.incidences = vec![1; target.incidences.len()];
    assert!(matches!(
        solve_polygon_incidence_witness(&target, u64::MAX, None),
        PolygonIncidenceWitnessOutcome::InvalidInput(_)
    ));
}

#[test]
fn false_relaxation_signature_has_no_witness() {
    let base = known_target();
    let target = OccurrenceIncidenceTarget::new(
        base.lower,
        base.upper,
        base.cut,
        vec![1, 1, 1, 1, 1, 1, 6, 6],
        BTreeSet::new(),
    );
    assert!(matches!(
        solve_polygon_incidence_witness(&target, u64::MAX, None),
        PolygonIncidenceWitnessOutcome::ExactNoWitness { .. }
    ));
}

#[test]
fn frozen_pier_small_exact_oracle_passes() {
    let evidence = include_str!("fixtures/pier_small_exact_oracle.json");
    assert!(evidence.contains(
        "\"lower\":3,\"upper\":3,\"targets\":20,\"csae_topologies\":21,\"pier_topologies\":21"
    ));
    assert!(evidence.contains(
        "\"lower\":4,\"upper\":5,\"targets\":4110,\"csae_topologies\":4180,\"pier_topologies\":4180"
    ));
    assert!(evidence.contains("\"all_families_equal\":true"));
}

#[test]
#[ignore = "write the PR114 PIER small exact oracle artifact"]
fn write_pier_small_exact_oracle() {
    let json = pier_small_exact_oracle_json().unwrap();
    if let Ok(path) = std::env::var("EARTHMESH_PIER_ORACLE_JSON") {
        std::fs::write(path, &json).unwrap();
    }
    println!("{json}");
}
