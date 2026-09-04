use earthmesh_refine_certified::coarsen::{
    analyze_legacy_transition_family, n6_legacy_mixed_fixture, TransitionFeasibilityOutcomeKind,
};

#[test]
fn n6_hidden_fixture_freezes_exact_parent_component_shape() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();

    assert_eq!(source.subdivision, 6);
    assert_eq!(component.id, 0);
    assert_eq!(component.parents.len(), 32);
    assert_eq!(component.core_parents.len(), 10);
    assert_eq!(component.transition_parents.len(), 22);
}

#[test]
fn n6_legacy_transition_family_emits_honest_machine_readable_evidence() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let proof = analyze_legacy_transition_family(&source, &component, 2_000, 1);
    let json = proof.to_machine_readable_json();

    assert_eq!(proof.source_subdivision, 6);
    assert_eq!(proof.core_parent_count, 10);
    assert_eq!(proof.transition_parent_count, 22);
    assert!(proof.family_topology_count > 0);
    assert!(proof.best_numerical_margin_degrees.is_some());
    // The family count is after hard topology gates prune invalid candidates.
    assert_eq!(proof.family_topology_count, 188);
    assert!(proof.topology_family_closed);
    assert_eq!(proof.interval_boxes, 188);
    assert_eq!(proof.interval_upper_margin_degrees, Some(19.8));
    assert!(proof
        .best_numerical_margin_degrees
        .is_some_and(|margin| (margin + 41.917_474_411_461).abs() < 1.0e-9));
    assert_eq!(
        proof.outcome,
        TransitionFeasibilityOutcomeKind::UnknownBudgetExhausted
    );
    assert!(proof.topologies.iter().all(|topology| {
        topology.fixed_vertices > 0
            && topology.movable_vertices > 0
            && topology.boxes == 1
            && topology.outcome == TransitionFeasibilityOutcomeKind::UnknownBudgetExhausted
    }));

    assert!(json.starts_with('{') && json.ends_with('}'));
    assert!(json.contains("\"fixture\":\"n6_hidden_mixed_exact_32_parent_component\""));
    assert!(json.contains("\"source_subdivision\":6"));
    assert!(json.contains("\"core_parent_count\":10"));
    assert!(json.contains("\"transition_parent_count\":22"));
    assert!(json.contains("\"family_topology_count\":"));
    assert!(json.contains("\"topology_family_closed\":true"));
    assert!(json.contains("\"interval_box_budget_per_topology\":1"));
    assert!(json.contains("\"interval_upper_margin_degrees\":"));
    assert!(json.contains("\"outcome\":"));
    assert!(json.contains("\"topologies\":["));
    assert!(!json.contains("ProvenInfeasible"));
}
