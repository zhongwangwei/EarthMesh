use earthmesh_refine_certified::coarsen::{
    build_essential_cycle_problem, build_face_band_problem, essential_cycle_find_one_evidence_json,
    find_one_essential_cycle, n6_legacy_mixed_fixture, solve_exact_face_bands,
    EssentialCycleFindOneLimits, EssentialCycleFindOneOutcome, FaceBandLimits,
    FaceBandSolveOutcome, FullPolygonMergeLimits, FullPolygonPlanEvaluator,
    RetainedCoreCorridorFamily,
};

#[test]
fn known_n6_w2_cycle_closes_in_fewer_unique_states_than_legacy_raw_states() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let face_problem = build_face_band_problem(&source, &component, 2).unwrap();
    let FaceBandSolveOutcome::Closed(_, legacy) = solve_exact_face_bands(
        &face_problem,
        FaceBandLimits {
            maximum_states: 1_000_000,
        },
    ) else {
        panic!("known legacy W2 problem must close")
    };
    let cycle_problem = build_essential_cycle_problem(
        &source,
        &face_problem,
        component.core_parents.iter().copied(),
        RetainedCoreCorridorFamily::F0CurrentSourceFaceCorridor,
    )
    .unwrap();
    let mut evaluator = FullPolygonPlanEvaluator::new(
        &source,
        &component,
        FullPolygonMergeLimits {
            topology_states: 1_000,
        },
    );
    let outcome = find_one_essential_cycle(
        &source,
        &face_problem,
        &cycle_problem,
        EssentialCycleFindOneLimits {
            maximum_unique_states: 5_000,
        },
        &mut evaluator,
    );
    let EssentialCycleFindOneOutcome::Closed {
        plan,
        trial,
        evidence,
        ..
    } = outcome
    else {
        panic!("CEC find-one must close the known N6 W2 problem: {outcome:?}")
    };
    assert!(evidence.unique_states < legacy.states_examined);
    assert_eq!(evidence.outcome.as_str(), "Closed");
    assert!(evidence.propagation_events > 0);
    assert!(evidence.peak_trail_records > 0);
    assert_eq!(plan.band_count, 2);
    assert_eq!(plan.interface_edges[0].len(), 20);
    assert_eq!(trial.evidence.states_examined, 31);
    assert!(essential_cycle_find_one_evidence_json(&evidence).contains("\"outcome\":\"Closed\""));
    eprintln!("{}", essential_cycle_find_one_evidence_json(&evidence));
}

#[test]
fn zero_unique_state_budget_is_incomplete_not_no_solution() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let face_problem = build_face_band_problem(&source, &component, 2).unwrap();
    let cycle_problem = build_essential_cycle_problem(
        &source,
        &face_problem,
        component.core_parents.iter().copied(),
        RetainedCoreCorridorFamily::F0CurrentSourceFaceCorridor,
    )
    .unwrap();
    let mut evaluator = FullPolygonPlanEvaluator::new(
        &source,
        &component,
        FullPolygonMergeLimits { topology_states: 0 },
    );
    let outcome = find_one_essential_cycle(
        &source,
        &face_problem,
        &cycle_problem,
        EssentialCycleFindOneLimits {
            maximum_unique_states: 0,
        },
        &mut evaluator,
    );
    let EssentialCycleFindOneOutcome::CycleSearchIncomplete { evidence } = outcome else {
        panic!("zero-budget find-one must remain cycle-search incomplete")
    };
    assert_eq!(evidence.unique_states, 0);
}
