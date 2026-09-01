use earthmesh_refine_certified::coarsen::{
    audit_legacy_downstream_preflight, build_geometry_guard_region,
    build_stratified_annulus_from_face_bands_v1, build_stratified_topology_domain_v2,
    build_transition_topology_domain_from_face_bands, n12_lifted_n6_fixture,
    n6_legacy_mixed_fixture, solve_exact_face_bands, DownstreamPreflightOutcome, FaceBandLimits,
    FaceBandSolveOutcome,
};
use std::collections::BTreeSet;

fn lifted_plan() -> (
    earthmesh_refine_certified::coarsen::CertifiedResearchFixture,
    earthmesh_refine_certified::coarsen::FaceBandPlan,
) {
    let fixture = n12_lifted_n6_fixture().unwrap();
    let problem = earthmesh_refine_certified::coarsen::build_face_band_problem(
        &fixture.source,
        &fixture.component,
        2,
    )
    .unwrap();
    let FaceBandSolveOutcome::Closed(plan, _) = solve_exact_face_bands(
        &problem,
        FaceBandLimits {
            maximum_states: 16_384,
        },
    ) else {
        panic!("frozen Lifted face band must close")
    };
    (fixture, *plan)
}

#[test]
fn n6_v1_v2_boundary_inputs_match() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let problem =
        earthmesh_refine_certified::coarsen::build_face_band_problem(&source, &component, 2)
            .unwrap();
    let FaceBandSolveOutcome::Closed(plan, _) = solve_exact_face_bands(
        &problem,
        FaceBandLimits {
            maximum_states: 16_384,
        },
    ) else {
        panic!("N6 plan")
    };
    let v1 = build_stratified_annulus_from_face_bands_v1(&source, &component, &plan).unwrap();
    let v2 = build_stratified_topology_domain_v2(&source, &component, &plan).unwrap();
    let slots = |ring: &earthmesh_refine_certified::coarsen::RingCycle| {
        ring.vertices
            .iter()
            .map(|vertex| vertex.source_slot)
            .collect::<std::collections::BTreeSet<_>>()
    };
    assert_eq!(
        slots(&v1.coupled.coarse_interface),
        slots(&v2.coupled.coarse_interface)
    );
    assert_eq!(
        slots(&v1.coupled.fine_interface),
        slots(&v2.coupled.fine_interface)
    );
    assert_eq!(v1.coupled.boundary_contracts, v2.coupled.boundary_contracts);
}

#[test]
fn lifted_topology_domain_does_not_require_inner_guard() {
    let (fixture, plan) = lifted_plan();
    assert!(matches!(
        audit_legacy_downstream_preflight(&fixture.source, &fixture.component),
        DownstreamPreflightOutcome::ContractBlocked { .. }
    ));
    let domain = build_transition_topology_domain_from_face_bands(
        &fixture.source,
        &fixture.component,
        &plan,
    )
    .unwrap();
    assert_eq!(domain.internal_interfaces.len(), 1);
    assert_eq!(domain.annulus_face_slots.len(), plan.labels.len());
    let result = build_stratified_topology_domain_v2(&fixture.source, &fixture.component, &plan);
    assert!(!format!("{result:?}").contains("inner_guard"));
}

#[test]
fn geometry_guard_is_explicitly_deferred_until_after_topology_domain() {
    let (fixture, plan) = lifted_plan();
    let domain = build_transition_topology_domain_from_face_bands(
        &fixture.source,
        &fixture.component,
        &plan,
    )
    .unwrap();
    let guard = build_geometry_guard_region(&fixture.source, &domain, &BTreeSet::new()).unwrap();
    assert!(!guard.movable_source_vertices.is_empty());
    assert!(guard
        .movable_source_vertices
        .is_disjoint(&guard.fixed_source_vertices));
    assert!(!guard.guard_face_slots.is_empty());
}
