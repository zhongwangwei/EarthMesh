use earthmesh_refine_certified::coarsen::{
    annular_reachability_storage_audit, build_face_band_problem, build_global_incidence_contract,
    build_stratified_transition_domain_v3, n6_legacy_mixed_fixture, solve_exact_face_bands,
    FaceBandLimits, FaceBandSolveOutcome,
};

#[test]
fn frozen_n6_fixed_topology_contract_is_consistent() {
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
    let domain = build_stratified_transition_domain_v3(&source, &component, &plan).unwrap();
    let contract = build_global_incidence_contract(&source, &component, &domain).unwrap();

    assert_eq!(contract.cell_ids.len(), 2);
    assert!(contract
        .vertex_domains
        .values()
        .all(|domain| !domain.allowed_owner_tuples.is_empty()));
    assert!(contract.cell_ids.iter().all(|cell| {
        contract.cell_incidence_sums[cell] == 3 * contract.cell_triangle_counts[cell]
    }));
}

#[test]
fn reachability_audit_reports_partial_signatures_without_witnesses() {
    let audit = annular_reachability_storage_audit();
    assert!(audit.stores_incidence_signatures);
    assert!(audit.stores_link_path_signatures);
    assert!(audit.stores_member_counts);
    assert!(!audit.stores_concrete_witnesses);
    assert!(!audit.stores_backpointers);
    assert!(audit.necessary_relaxation_only);
}

#[test]
fn production_has_no_slot_special_case() {
    let source = include_str!("../src/coarsen/sdce_incidence.rs");
    let numeric_tokens = source
        .split(|character: char| !character.is_ascii_digit())
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    for slot in [48, 52, 78, 252, 256, 343] {
        assert!(!numeric_tokens.contains(&slot.to_string().as_str()));
    }
}
