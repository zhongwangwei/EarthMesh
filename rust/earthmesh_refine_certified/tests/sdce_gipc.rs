use earthmesh_refine_certified::coarsen::{
    build_face_band_problem, build_global_incidence_contract,
    build_stratified_transition_domain_v3, n12_lifted_n6_fixture, n6_legacy_mixed_fixture,
    solve_exact_face_bands, solve_global_incidence_plan, EssentialCycleKey, FaceBandLimits,
    FaceBandSolveOutcome, GlobalIncidencePlan, IncidencePlanOutcome, IncidencePlanSearchConfig,
    RingAnchorKind,
};
use std::collections::BTreeSet;

#[test]
fn frozen_n6_finds_a_global_incidence_plan_before_triangles() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let problem = build_face_band_problem(&source, &component, 2).unwrap();
    let FaceBandSolveOutcome::Closed(face_plan, _) = solve_exact_face_bands(
        &problem,
        FaceBandLimits {
            maximum_states: 16_384,
        },
    ) else {
        panic!("Frozen N6 W2 plan must close")
    };
    let domain = build_stratified_transition_domain_v3(&source, &component, &face_plan).unwrap();
    let contract = build_global_incidence_contract(&source, &component, &domain).unwrap();
    let outcome = solve_global_incidence_plan(
        &EssentialCycleKey {
            ordered_vertices: Vec::new(),
        },
        &contract,
        &domain,
        &IncidencePlanSearchConfig::default(),
        None,
    );
    let IncidencePlanOutcome::Found { plan, evidence } = outcome else {
        panic!("Frozen N6 contract must have a GIPC plan: {outcome:?}")
    };

    assert!(evidence.states <= IncidencePlanSearchConfig::default().maximum_states);
    assert!(contract.cell_ids.iter().all(|cell| {
        plan.cell_incidences[cell]
            .values()
            .map(|count| usize::from(*count))
            .sum::<usize>()
            == contract.cell_incidence_sums[cell]
    }));
    assert!(contract.vertex_domains.iter().all(|(slot, domain)| {
        !matches!(
            domain.anchor_kind,
            RingAnchorKind::IcosahedronPentagon { .. }
        ) || plan.final_degrees[slot] == 5
    }));
}

#[test]
fn selected_plan_never_assigns_degree4_to_48_or_252() {
    let plan = lifted_plan();
    assert!([48, 252]
        .into_iter()
        .all(|slot| plan.final_degrees[&slot] != 4));
}

#[test]
fn selected_plan_never_assigns_degree8_or9_to_known_defect_vertices() {
    let plan = lifted_plan();
    assert!([52, 78, 256, 343]
        .into_iter()
        .all(|slot| ![8, 9].contains(&plan.final_degrees[&slot])));
}

fn lifted_plan() -> GlobalIncidencePlan {
    let fixture = n12_lifted_n6_fixture().unwrap();
    let problem = build_face_band_problem(&fixture.source, &fixture.component, 2).unwrap();
    let FaceBandSolveOutcome::Closed(face_plan, _) = solve_exact_face_bands(
        &problem,
        FaceBandLimits {
            maximum_states: 16_384,
        },
    ) else {
        panic!("Lifted N12 W2 plan must close")
    };
    let domain =
        build_stratified_transition_domain_v3(&fixture.source, &fixture.component, &face_plan)
            .unwrap();
    let contract =
        build_global_incidence_contract(&fixture.source, &fixture.component, &domain).unwrap();
    let outcome = solve_global_incidence_plan(
        &EssentialCycleKey {
            ordered_vertices: Vec::new(),
        },
        &contract,
        &domain,
        &IncidencePlanSearchConfig {
            maximum_states: 4_096,
            priority_vertices: BTreeSet::from([48, 52, 78, 252, 256, 343]),
        },
        None,
    );
    let IncidencePlanOutcome::Found { plan, .. } = outcome else {
        panic!("Lifted N12 contract must have a GIPC plan: {outcome:?}")
    };
    plan
}
