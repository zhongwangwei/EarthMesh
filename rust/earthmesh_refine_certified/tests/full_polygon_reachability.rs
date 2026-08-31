use earthmesh_refine_certified::coarsen::{
    analyze_full_polygon_degree_reachability, analyze_stratified_full_polygon_degree_reachability,
    build_stratified_annulus, degree_defects_from_global_evidence, n6_legacy_mixed_fixture,
    DegreeDomainOutcome, GlobalExactMergeEvidence,
};
use std::collections::BTreeMap;

#[test]
fn n6_effective_sector_polygon_sizes_match_pr39_fixture() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let evidence = analyze_full_polygon_degree_reachability(&source, &component).unwrap();
    assert_eq!(
        evidence.sector_polygon_sizes,
        vec![5, 5, 8, 8, 5, 5, 8, 8, 6, 6, 6, 6, 8, 8]
    );
}

#[test]
fn n6_incidence_domains_are_populated_for_all_sector_vertices() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let stratified = build_stratified_annulus(&source, &component).unwrap();
    let evidence =
        analyze_stratified_full_polygon_degree_reachability(&source, &component, &stratified)
            .unwrap();

    for sector in stratified
        .probe
        .sector_components
        .iter()
        .enumerate()
        .map(|(id, _)| id as u64)
    {
        assert!(
            evidence
                .incidence_domains
                .keys()
                .any(|(sector_id, _)| *sector_id == sector),
            "sector {sector} has no incidence domain"
        );
    }
    assert!(evidence.signatures_before_ac3 >= evidence.signatures_after_ac3);
}

#[test]
fn n6_degree_reachability_hard_gate_is_not_budget_exhaustion() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let evidence = analyze_full_polygon_degree_reachability(&source, &component).unwrap();
    assert_eq!(evidence.outcome, DegreeDomainOutcome::NecessaryFeasible);
    assert_eq!(evidence.defect_vertices.len(), 0);
    assert_eq!(evidence.signatures_before_ac3, 868);
    assert_eq!(evidence.signatures_after_ac3, 448);
    assert_eq!(
        evidence.sector_topology_counts,
        vec![5, 5, 132, 132, 5, 5, 132, 132, 14, 14, 14, 14, 132, 132]
    );
    assert_eq!(
        evidence.sector_signature_counts,
        vec![5, 5, 84, 84, 5, 5, 51, 51, 14, 14, 14, 14, 51, 51]
    );
    assert!(!evidence.ear_delta_domains_exact);
    assert!(evidence
        .ear_delta_domains
        .values()
        .any(|domain| domain.contains(&-8) && domain.contains(&8)));
    assert_ne!(evidence.global_degree_domains.len(), 0);
}

#[test]
fn global_evidence_defects_use_actual_degree_and_ear_delta() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let stratified = build_stratified_annulus(&source, &component).unwrap();
    let (&vertex, contract) = stratified.link_contracts.iter().next().unwrap();
    let final_degree = usize::from(contract.target_degree_max) + 1;
    let evidence = GlobalExactMergeEvidence {
        vertex_degrees: BTreeMap::from([(vertex, final_degree)]),
        vertex_sector_contributions: BTreeMap::from([(vertex, vec![(7, 2)])]),
        vertex_ear_deltas: BTreeMap::from([(vertex, -1)]),
        ..Default::default()
    };

    let defects = degree_defects_from_global_evidence(&stratified, &evidence).unwrap();
    assert_eq!(defects.len(), 1);
    assert_eq!(defects[0].source_slot, vertex);
    assert_eq!(defects[0].final_degree, final_degree as u8);
    assert_eq!(defects[0].fixed_degree, final_degree as u8 - 1);
    assert_eq!(defects[0].selected_sector_contributions, vec![(7, 2)]);
    assert_eq!(defects[0].owner_sector_ids, vec![7]);
}
