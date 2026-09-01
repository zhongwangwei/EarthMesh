use earthmesh_refine_certified::coarsen::{
    build_essential_cycle_problem, build_face_band_problem, essential_cycle_find_one_evidence_json,
    merge_cycle_search_checkpoints, n12_interior_control_fixture, n6_legacy_mixed_fixture,
    prove_essential_cycle_family, DownstreamEvaluationCache, EssentialCycleFindOneLimits,
    ExactFaceBandV2Outcome, FaceBandPlan, FaceBandPlanEvaluator, FullPolygonMergeLimits,
    FullPolygonPlanEvaluator, PlanEvaluation, RetainedCoreCorridorFamily,
};

#[test]
fn small_fixture_can_close_as_exact_no_solution() {
    let fixture = n12_interior_control_fixture().unwrap();
    let face_problem = build_face_band_problem(&fixture.source, &fixture.component, 2).unwrap();
    let cycle_problem = build_essential_cycle_problem(
        &fixture.source,
        &face_problem,
        fixture.component.core_parents.iter().copied(),
        RetainedCoreCorridorFamily::F0CurrentSourceFaceCorridor,
    )
    .unwrap();
    let mut evaluator = PanicEvaluator;
    let outcome = prove_essential_cycle_family(
        &fixture.source,
        &face_problem,
        &cycle_problem,
        EssentialCycleFindOneLimits {
            maximum_unique_states: 10_000,
        },
        None,
        &mut evaluator,
        &mut DownstreamEvaluationCache::new(),
    );
    let ExactFaceBandV2Outcome::ExactNoSolution {
        cycle_family_closed,
        all_downstream_exact_no_solution,
        evidence,
    } = outcome
    else {
        panic!("small Interior-Control CEC family must exhaust: {outcome:?}")
    };
    assert!(cycle_family_closed);
    assert!(all_downstream_exact_no_solution);
    assert_eq!(evidence.outcome.as_str(), "ExactNoSolution");
    eprintln!("{}", essential_cycle_find_one_evidence_json(&evidence));
}

#[test]
fn checkpoint_resume_matches_one_shot_and_cache_is_exact_keyed() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let face_problem = build_face_band_problem(&source, &component, 2).unwrap();
    let cycle_problem = build_essential_cycle_problem(
        &source,
        &face_problem,
        component.core_parents.iter().copied(),
        RetainedCoreCorridorFamily::F0CurrentSourceFaceCorridor,
    )
    .unwrap();
    let mut cache = DownstreamEvaluationCache::new();

    let one_shot = solve(
        &source,
        &component,
        &face_problem,
        &cycle_problem,
        5_000,
        None,
        &mut cache,
    );
    let ExactFaceBandV2Outcome::Closed {
        cycle: one_cycle,
        plan: one_plan,
        evidence: one_evidence,
        ..
    } = one_shot
    else {
        panic!("one-shot proof must find the known closed family")
    };
    assert_eq!(one_evidence.cache_misses, 1);

    let first_chunk = solve(
        &source,
        &component,
        &face_problem,
        &cycle_problem,
        1,
        None,
        &mut cache,
    );
    let ExactFaceBandV2Outcome::CycleSearchIncomplete { checkpoint, .. } = first_chunk else {
        panic!("one-state proof chunk must checkpoint")
    };
    assert!(!checkpoint.shards.is_empty());
    assert_eq!(
        merge_cycle_search_checkpoints([checkpoint.clone(), checkpoint.clone()]).unwrap(),
        checkpoint
    );

    let resumed = solve(
        &source,
        &component,
        &face_problem,
        &cycle_problem,
        5_000,
        Some(&checkpoint),
        &mut cache,
    );
    let ExactFaceBandV2Outcome::Closed {
        cycle,
        plan,
        evidence,
        ..
    } = resumed
    else {
        panic!("resumed proof must find the known closed family: {resumed:?}")
    };
    assert_eq!(cycle, one_cycle);
    assert_eq!(plan, one_plan);
    assert_eq!(evidence.cache_hits, 1);
}

fn solve(
    source: &earthmesh_refine_certified::MotherGrid,
    component: &earthmesh_refine_certified::coarsen::HierarchyComponent,
    face_problem: &earthmesh_refine_certified::coarsen::FaceBandProblem,
    cycle_problem: &earthmesh_refine_certified::coarsen::EssentialCycleProblem,
    maximum_unique_states: u64,
    checkpoint: Option<&earthmesh_refine_certified::coarsen::CycleSearchCheckpoint>,
    cache: &mut DownstreamEvaluationCache,
) -> ExactFaceBandV2Outcome {
    let mut evaluator = FullPolygonPlanEvaluator::new(
        source,
        component,
        FullPolygonMergeLimits {
            topology_states: 1_000,
        },
    );
    prove_essential_cycle_family(
        source,
        face_problem,
        cycle_problem,
        EssentialCycleFindOneLimits {
            maximum_unique_states,
        },
        checkpoint,
        &mut evaluator,
        cache,
    )
}

struct PanicEvaluator;

impl FaceBandPlanEvaluator for PanicEvaluator {
    fn evaluate(&mut self, _: &FaceBandPlan) -> PlanEvaluation {
        panic!("Interior-Control exact cycle exhaustion must not reach downstream")
    }
}
