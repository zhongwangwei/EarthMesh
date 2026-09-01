use earthmesh_refine_certified::coarsen::{
    build_face_band_problem, build_stratified_annulus_from_face_bands_v1,
    build_stratified_topology_domain_v2, n6_legacy_mixed_fixture, solve_exact_face_bands,
    solve_full_polygon_merge_from_face_bands, solve_full_polygon_merge_from_face_bands_v2,
    FaceBandLimits, FaceBandSolveOutcome, FullPolygonMergeLimits, FullPolygonMergeOutcome,
    StratifiedAnnulus,
};
use std::collections::BTreeSet;

#[test]
fn frozen_n6_v1_v2_topology_oracle_is_exact() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let problem = build_face_band_problem(&source, &component, 2).unwrap();
    let FaceBandSolveOutcome::Closed(plan, _) = solve_exact_face_bands(
        &problem,
        FaceBandLimits {
            maximum_states: 16_384,
        },
    ) else {
        panic!("Frozen N6 W2 plan must close")
    };
    let v1_domain =
        build_stratified_annulus_from_face_bands_v1(&source, &component, &plan).unwrap();
    let v2_domain = build_stratified_topology_domain_v2(&source, &component, &plan).unwrap();
    assert_domain_equal(&v1_domain, &v2_domain);

    let limits = FullPolygonMergeLimits {
        topology_states: 4_096,
    };
    let v1 = solve_full_polygon_merge_from_face_bands(&source, &component, &plan, limits);
    let v2 = solve_full_polygon_merge_from_face_bands_v2(&source, &component, &plan, limits);
    let (FullPolygonMergeOutcome::Closed(v1), FullPolygonMergeOutcome::Closed(v2)) = (v1, v2)
    else {
        panic!("both adapters must close the Frozen N6 oracle")
    };
    assert_eq!(
        v1.evidence.sector_family_counts,
        v2.evidence.sector_family_counts
    );
    assert_eq!(
        v1.evidence.retained_topology_counts,
        v2.evidence.retained_topology_counts
    );
    assert_eq!(v1.evidence.states_examined, v2.evidence.states_examined);
    assert_eq!(v1.evidence.states_by_depth, v2.evidence.states_by_depth);
    assert_eq!(
        v1.evidence.selected_topology_keys,
        v2.evidence.selected_topology_keys
    );
    assert_eq!(v1.evidence.selected_ears, v2.evidence.selected_ears);
    assert_eq!(
        v1.global_trial.evidence.anchor_degrees,
        v2.global_trial.evidence.anchor_degrees
    );
    assert_eq!(
        v1.global_trial.evidence.ordinary_degree_histogram,
        v2.global_trial.evidence.ordinary_degree_histogram
    );
    assert_eq!(
        (
            v1.global_trial.evidence.vertices,
            v1.global_trial.evidence.edges,
            v1.global_trial.evidence.faces,
            v1.global_trial.evidence.euler,
            v1.global_trial.evidence.charge,
        ),
        (
            v2.global_trial.evidence.vertices,
            v2.global_trial.evidence.edges,
            v2.global_trial.evidence.faces,
            v2.global_trial.evidence.euler,
            v2.global_trial.evidence.charge,
        )
    );
}

fn assert_domain_equal(v1: &StratifiedAnnulus, v2: &StratifiedAnnulus) {
    let ring_edges = |ring: &earthmesh_refine_certified::coarsen::RingCycle| {
        ring.vertices
            .iter()
            .map(|vertex| vertex.source_slot)
            .zip(
                ring.vertices
                    .iter()
                    .map(|vertex| vertex.source_slot)
                    .cycle()
                    .skip(1),
            )
            .take(ring.vertices.len())
            .map(|(a, b)| if a < b { (a, b) } else { (b, a) })
            .collect::<BTreeSet<_>>()
    };
    assert_eq!(
        ring_edges(&v1.coupled.coarse_interface),
        ring_edges(&v2.coupled.coarse_interface)
    );
    assert_eq!(
        ring_edges(&v1.coupled.fine_interface),
        ring_edges(&v2.coupled.fine_interface)
    );
    assert_eq!(v1.coupled.annulus_face_slots, v2.coupled.annulus_face_slots);
    assert_eq!(v1.coupled.boundary_contracts, v2.coupled.boundary_contracts);
    assert_eq!(v1.band_face_labels, v2.band_face_labels);
    assert_eq!(v1.bands, v2.bands);
    assert_eq!(v1.link_contracts, v2.link_contracts);
}
