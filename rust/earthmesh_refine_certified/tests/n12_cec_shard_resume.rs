use earthmesh_refine_certified::{
    coarsen::{
        build_essential_cycle_problem, build_face_band_problem, merge_cycle_search_checkpoints,
        n12_lifted_n6_fixture, prove_essential_cycle_family, solve_sdce_plan_find_one,
        CycleSearchCheckpoint, DownstreamEvaluationCache, DownstreamRejectStage,
        EssentialCycleFindOneLimits, EssentialCycleKey, ExactFaceBandV2Outcome, FaceBandPlan,
        FaceBandPlanEvaluator, HierarchyComponent, PlanEvaluation, RetainedCoreCorridorFamily,
        SdcePlanFindOneOutcome, SdcePlanQuantum,
    },
    MotherGrid,
};
use std::fs;

const INITIAL_STATES: u64 = 16_384;
const STATES_PER_SHARD: u64 = 256;
const SCREENING: SdcePlanQuantum = SdcePlanQuantum {
    balanced_topologies_per_cell: 8,
    beam_width: 1,
    maximum_flip_depth: 0,
    maximum_joint_pairs: 0,
};

#[test]
fn frozen_pr121_resume_evidence_is_safe() {
    let evidence = include_str!("fixtures/n12_cec_shard_resume.json");
    for fact in [
        "\"input_checkpoint_shards\":49",
        "\"shards_resumed\":49",
        "\"invalid\":0",
        "\"geometry_attempted\":false",
        "\"strict_geometry_gate_passed\":false",
        "\"n24_n40_nxp80_unlocked\":false",
        "\"product_gate_changed\":false",
    ] {
        assert!(evidence.contains(fact), "missing frozen fact: {fact}");
    }
    assert!(evidence.contains(
        "\"classification\":{\"closed\":0,\"exact_no_solution\":1,\"cycle_search_incomplete\":45,\"downstream_search_incomplete\":3,\"invalid\":0}"
    ));
    assert!(evidence.contains("\"remaining_checkpoint_shards\":1233"));
    assert!(evidence.contains("\"best_incidence_distance\":16"));
}

#[test]
#[ignore = "PR121 fair resume of the 49 frozen N12 CEC shards"]
fn write_n12_cec_shard_resume() {
    let json = run_resume().unwrap();
    if let Ok(path) = std::env::var("EARTHMESH_N12_CEC_SHARD_RESUME_JSON") {
        fs::write(path, &json).unwrap();
    }
    println!("{json}");
}

fn run_resume() -> Result<String, String> {
    let fixture = n12_lifted_n6_fixture()?;
    let face = build_face_band_problem(&fixture.source, &fixture.component, 2)?;
    let problem = build_essential_cycle_problem(
        &fixture.source,
        &face,
        fixture.component.core_parents.iter().copied(),
        RetainedCoreCorridorFamily::F0CurrentSourceFaceCorridor,
    )?;
    let initial = prove_essential_cycle_family(
        &fixture.source,
        &face,
        &problem,
        EssentialCycleFindOneLimits {
            maximum_unique_states: INITIAL_STATES,
        },
        None,
        &mut CheckpointOnly,
        &mut DownstreamEvaluationCache::new(),
    );
    let ExactFaceBandV2Outcome::CycleSearchIncomplete { checkpoint, .. } = initial else {
        return Err(format!("expected frozen N12 checkpoint, got {initial:?}"));
    };
    if checkpoint.shards.len() != 49 {
        return Err(format!(
            "expected 49 CEC shards, got {}",
            checkpoint.shards.len()
        ));
    }

    let mut closed = 0;
    let mut exact = 0;
    let mut cycle_incomplete = 0;
    let mut downstream_incomplete = 0;
    let mut unique_states = 0;
    let mut essential_cycles = 0;
    let mut screening = Screening::new(&fixture.source, &fixture.component);
    let mut remaining = Vec::new();
    for shard in checkpoint.shards.iter().cloned() {
        let single = CycleSearchCheckpoint {
            problem_key: checkpoint.problem_key.clone(),
            shards: vec![shard],
        };
        let outcome = prove_essential_cycle_family(
            &fixture.source,
            &face,
            &problem,
            EssentialCycleFindOneLimits {
                maximum_unique_states: STATES_PER_SHARD,
            },
            Some(&single),
            &mut screening,
            &mut DownstreamEvaluationCache::new(),
        );
        let evidence = match outcome {
            ExactFaceBandV2Outcome::Closed { evidence, .. } => {
                closed += 1;
                evidence
            }
            ExactFaceBandV2Outcome::ExactNoSolution { evidence, .. } => {
                exact += 1;
                evidence
            }
            ExactFaceBandV2Outcome::CycleSearchIncomplete {
                checkpoint,
                evidence,
            } => {
                cycle_incomplete += 1;
                remaining.push(checkpoint);
                evidence
            }
            ExactFaceBandV2Outcome::DownstreamSearchIncomplete { evidence } => {
                downstream_incomplete += 1;
                evidence
            }
            ExactFaceBandV2Outcome::InvalidInput { reason } => return Err(reason),
        };
        unique_states += evidence.unique_states;
        essential_cycles += evidence.essential_cycles;
    }
    let remaining = if remaining.is_empty() {
        0
    } else {
        merge_cycle_search_checkpoints(remaining)?.shards.len()
    };
    Ok(format!(
        "{{\"schema_version\":1,\"taskbook_sha256\":\"65f26b64c78dd7dfadaaf2a1099f52d11c6a67461afb0a9558edbbf5941ef473\",\"runner\":\"N12Pr121CecShardResume\",\"fixture\":\"N12-Lifted-N6\",\"research_only\":true,\"limits\":{{\"initial_cycle_unique_states\":{INITIAL_STATES},\"unique_states_per_shard\":{STATES_PER_SHARD},\"screening_topologies_per_cell\":{},\"screening_beam_width\":{},\"screening_flip_depth\":{},\"screening_joint_pairs\":{}}},\"input_checkpoint_shards\":49,\"shards_resumed\":49,\"classification\":{{\"closed\":{closed},\"exact_no_solution\":{exact},\"cycle_search_incomplete\":{cycle_incomplete},\"downstream_search_incomplete\":{downstream_incomplete},\"invalid\":{}}},\"remaining_checkpoint_shards\":{remaining},\"resumed_unique_states\":{unique_states},\"resumed_essential_cycles\":{essential_cycles},\"sdce_cycles_screened\":{},\"sdce_pairs_scored\":{},\"best_incidence_distance\":{},\"topology_closed\":{},\"resume_complete\":{},\"geometry_attempted\":false,\"strict_geometry_gate_passed\":false,\"n24_n40_nxp80_unlocked\":false,\"product_grid_written\":false,\"ready_marker_written\":false,\"product_gate_changed\":false}}",
        SCREENING.balanced_topologies_per_cell,
        SCREENING.beam_width,
        SCREENING.maximum_flip_depth,
        SCREENING.maximum_joint_pairs,
        screening.invalid,
        screening.cycles,
        screening.pairs,
        screening.best.map_or_else(|| "null".into(), |value| value.to_string()),
        screening.closed > 0,
        remaining == 0,
    ))
}

struct CheckpointOnly;

impl FaceBandPlanEvaluator for CheckpointOnly {
    fn evaluate(&mut self, _: &FaceBandPlan) -> PlanEvaluation {
        PlanEvaluation::AuditOnly
    }
}

struct Screening<'a> {
    source: &'a MotherGrid,
    component: &'a HierarchyComponent,
    cycle: Option<EssentialCycleKey>,
    cycles: usize,
    pairs: usize,
    best: Option<usize>,
    closed: usize,
    invalid: usize,
}

impl<'a> Screening<'a> {
    fn new(source: &'a MotherGrid, component: &'a HierarchyComponent) -> Self {
        Self {
            source,
            component,
            cycle: None,
            cycles: 0,
            pairs: 0,
            best: None,
            closed: 0,
            invalid: 0,
        }
    }

    fn record(&mut self, pairs: usize, distance: usize) {
        self.pairs += pairs;
        self.best = Some(self.best.map_or(distance, |best| best.min(distance)));
    }
}

impl FaceBandPlanEvaluator for Screening<'_> {
    fn observe_cycle(&mut self, cycle: &EssentialCycleKey, _: &FaceBandPlan) {
        self.cycle = Some(cycle.clone());
    }

    fn evaluate(&mut self, plan: &FaceBandPlan) -> PlanEvaluation {
        self.cycles += 1;
        match solve_sdce_plan_find_one(
            self.source,
            self.component,
            self.cycle.as_ref().unwrap(),
            plan,
            SCREENING,
        ) {
            SdcePlanFindOneOutcome::Closed { evidence, .. } => {
                self.closed += 1;
                self.record(evidence.pairs_scored, evidence.best_incidence_distance);
                PlanEvaluation::RejectedV3SearchIncomplete {
                    states_examined: evidence.pairs_scored,
                    stage: DownstreamRejectStage::SearchIncomplete,
                    reason: "Pr121ScreeningClosedNeedsFinalistValidation".into(),
                }
            }
            SdcePlanFindOneOutcome::SearchIncomplete(evidence) => {
                self.record(evidence.pairs_scored, evidence.best_incidence_distance);
                PlanEvaluation::RejectedV3SearchIncomplete {
                    states_examined: evidence.pairs_scored,
                    stage: DownstreamRejectStage::SearchIncomplete,
                    reason: "Pr121SdceScreeningIncomplete".into(),
                }
            }
            SdcePlanFindOneOutcome::InvalidInput(reason) => {
                self.invalid += 1;
                PlanEvaluation::RejectedV3Invalid {
                    states_examined: 0,
                    stage: DownstreamRejectStage::SearchIncomplete,
                    reason,
                }
            }
        }
    }
}
