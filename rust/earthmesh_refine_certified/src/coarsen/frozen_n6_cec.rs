//! Bounded, research-only CEC reclassification of the Alpha5 Frozen N6 unknowns.

use super::{
    build_essential_cycle_problem, n6_legacy_mixed_fixture, plan_retained_core_subsets,
    prove_essential_cycle_family, remaining_connected_retained_core_candidates,
    retained_core::{
        component_for_retained_core, retained_core_family_problem, FamilyProblemError,
    },
    solve_exact_face_bands, CycleSearchCheckpoint, DownstreamEvaluationCache,
    EssentialCycleFindOneEvidence, EssentialCycleFindOneLimits, EssentialCycleProblemKey,
    ExactFaceBandV2Outcome, FaceBandLimits, FaceBandSolveOutcome, FullPolygonMergeLimits,
    FullPolygonPlanEvaluator, RetainedCoreCorridorFamily,
};
use std::collections::{BTreeMap, BTreeSet};

pub type FrozenN6CecResumeMap = BTreeMap<EssentialCycleProblemKey, CycleSearchCheckpoint>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrozenN6CecClosureLimits {
    pub legacy_face_band_states: u64,
    pub cycle_unique_states: u64,
    pub topology_states: usize,
}

impl Default for FrozenN6CecClosureLimits {
    fn default() -> Self {
        Self {
            legacy_face_band_states: 16_384,
            cycle_unique_states: 16_384,
            topology_states: 4_096,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FrozenN6CecStatus {
    Closed,
    ExactNoSolution,
    CycleSearchIncomplete,
    DownstreamSearchIncomplete,
}

impl FrozenN6CecStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Closed => "Closed",
            Self::ExactNoSolution => "ExactNoSolution",
            Self::CycleSearchIncomplete => "CycleSearchIncomplete",
            Self::DownstreamSearchIncomplete => "DownstreamSearchIncomplete",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FrozenN6CecAttemptRecord {
    pub candidate_index: usize,
    pub retained_parents: usize,
    pub corridor_family: RetainedCoreCorridorFamily,
    pub transition_faces: usize,
    pub candidate_edges: usize,
    pub legacy_states: u64,
    pub legacy_budget_exhausted: bool,
    pub exact_duplicate_reused: bool,
    pub status: FrozenN6CecStatus,
    pub evidence: EssentialCycleFindOneEvidence,
    pub checkpoint: Option<CycleSearchCheckpoint>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FrozenN6CecClosureReport {
    pub limits: FrozenN6CecClosureLimits,
    pub retained_core_candidates: usize,
    pub family_attempts: usize,
    pub legacy_exact_no_solution: usize,
    pub legacy_closed_unknowns: usize,
    pub legacy_budget_unknowns: usize,
    pub targeted_unknowns: usize,
    pub exact_duplicate_reuses: usize,
    pub status_counts: BTreeMap<FrozenN6CecStatus, usize>,
    pub total_unique_states: u64,
    pub total_raw_decisions: u64,
    pub total_closed_cycles: u64,
    pub total_cache_hits: u64,
    pub total_cache_misses: u64,
    pub records: Vec<FrozenN6CecAttemptRecord>,
}

pub fn run_frozen_n6_cec_closure(
    limits: FrozenN6CecClosureLimits,
    resume: &FrozenN6CecResumeMap,
) -> Result<FrozenN6CecClosureReport, String> {
    let (source, original) = n6_legacy_mixed_fixture()?;
    let initial_core = original
        .core_parents
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let plan = plan_retained_core_subsets(&source, &initial_core, &initial_core)?;
    let candidates = remaining_connected_retained_core_candidates(&plan);
    let retained_core_candidates = candidates.len();
    let family_attempts = retained_core_candidates * RetainedCoreCorridorFamily::ALL.len();
    let mut report = FrozenN6CecClosureReport {
        limits,
        retained_core_candidates,
        family_attempts,
        legacy_exact_no_solution: 0,
        legacy_closed_unknowns: 0,
        legacy_budget_unknowns: 0,
        targeted_unknowns: 0,
        exact_duplicate_reuses: 0,
        status_counts: BTreeMap::new(),
        total_unique_states: 0,
        total_raw_decisions: 0,
        total_closed_cycles: 0,
        total_cache_hits: 0,
        total_cache_misses: 0,
        records: Vec::new(),
    };
    let mut reusable = BTreeMap::<EssentialCycleProblemKey, ReusableClassification>::new();

    for (candidate_index, candidate) in candidates.iter().enumerate() {
        let component = component_for_retained_core(&original, candidate)?;
        for family in RetainedCoreCorridorFamily::ALL {
            let problem = match retained_core_family_problem(&source, &component, family) {
                Ok(problem) => problem,
                Err(FamilyProblemError::ExactNoSolution) => {
                    report.legacy_exact_no_solution += 1;
                    continue;
                }
                Err(FamilyProblemError::Invalid(reason)) => return Err(reason),
            };
            let (legacy_states, legacy_budget_exhausted) = match solve_exact_face_bands(
                &problem,
                FaceBandLimits {
                    maximum_states: limits.legacy_face_band_states,
                },
            ) {
                FaceBandSolveOutcome::Closed(_, evidence) => {
                    report.legacy_closed_unknowns += 1;
                    (evidence.states_examined, false)
                }
                FaceBandSolveOutcome::SearchBudgetExhausted {
                    states_examined, ..
                } => {
                    report.legacy_budget_unknowns += 1;
                    (states_examined, true)
                }
                FaceBandSolveOutcome::FamilyExhaustedNoSolution { .. } => {
                    report.legacy_exact_no_solution += 1;
                    continue;
                }
                FaceBandSolveOutcome::InvalidInput { reason } => return Err(reason),
            };
            report.targeted_unknowns += 1;
            let cycle_problem = build_essential_cycle_problem(
                &source,
                &problem,
                candidate.retained_parents.iter().copied(),
                family,
            )?;
            let key = cycle_problem.problem_key.clone();
            let (classification, exact_duplicate_reused) = if let Some(reused) = reusable.get(&key)
            {
                report.exact_duplicate_reuses += 1;
                (reused.clone(), true)
            } else {
                let mut evaluator = FullPolygonPlanEvaluator::uncached(
                    &source,
                    &component,
                    FullPolygonMergeLimits {
                        topology_states: limits.topology_states,
                    },
                );
                let outcome = prove_essential_cycle_family(
                    &source,
                    &problem,
                    &cycle_problem,
                    EssentialCycleFindOneLimits {
                        maximum_unique_states: limits.cycle_unique_states,
                    },
                    resume.get(&key),
                    &mut evaluator,
                    &mut DownstreamEvaluationCache::new(),
                );
                let classified = ReusableClassification::from_outcome(outcome)?;
                reusable.insert(key, classified.clone());
                (classified, false)
            };
            *report
                .status_counts
                .entry(classification.status)
                .or_default() += 1;
            report.total_unique_states += classification.evidence.unique_states;
            report.total_raw_decisions += classification.evidence.raw_decisions;
            report.total_closed_cycles += classification.evidence.closed_cycles;
            report.total_cache_hits += classification.evidence.cache_hits;
            report.total_cache_misses += classification.evidence.cache_misses;
            report.records.push(FrozenN6CecAttemptRecord {
                candidate_index,
                retained_parents: candidate.retained_parents.len(),
                corridor_family: family,
                transition_faces: problem.transition_faces.len(),
                candidate_edges: cycle_problem.candidate_edges.len(),
                legacy_states,
                legacy_budget_exhausted,
                exact_duplicate_reused,
                status: classification.status,
                evidence: classification.evidence,
                checkpoint: classification.checkpoint,
            });
        }
    }
    if report.targeted_unknowns != 659 {
        return Err(format!(
            "Frozen N6 target drifted from 659 to {}",
            report.targeted_unknowns
        ));
    }
    Ok(report)
}

pub fn frozen_n6_cec_closure_report_json(report: &FrozenN6CecClosureReport) -> String {
    let statuses = FrozenN6CecStatus::ALL
        .iter()
        .map(|status| {
            format!(
                "\"{}\":{}",
                status.as_str(),
                report.status_counts.get(status).copied().unwrap_or(0)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let records = report
        .records
        .iter()
        .map(|record| {
            format!(
                "{{\"candidate_index\":{},\"retained_parents\":{},\"corridor_family\":\"{}\",\"transition_faces\":{},\"candidate_edges\":{},\"legacy_states\":{},\"legacy_budget_exhausted\":{},\"exact_duplicate_reused\":{},\"unique_states\":{},\"raw_decisions\":{},\"closed_cycles\":{},\"checkpoint_shards\":{},\"status\":\"{}\"}}",
                record.candidate_index,
                record.retained_parents,
                record.corridor_family.as_str(),
                record.transition_faces,
                record.candidate_edges,
                record.legacy_states,
                record.legacy_budget_exhausted,
                record.exact_duplicate_reused,
                record.evidence.unique_states,
                record.evidence.raw_decisions,
                record.evidence.closed_cycles,
                record.checkpoint.as_ref().map_or(0, |value| value.shards.len()),
                record.status.as_str(),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema_version\":1,\"taskbook_sha256\":\"b327b6afdf199abfaf1a77f4e403ef296e4f5bd2483d855b360c08152a10ae53\",\"limits\":{{\"legacy_face_band_states\":{},\"cycle_unique_states\":{},\"topology_states\":{}}},\"retained_core_candidates\":{},\"family_attempts\":{},\"legacy_exact_no_solution\":{},\"legacy_closed_unknowns\":{},\"legacy_budget_unknowns\":{},\"targeted_unknowns\":{},\"exact_duplicate_reuses\":{},\"status_counts\":{{{}}},\"total_unique_states\":{},\"total_raw_decisions\":{},\"total_closed_cycles\":{},\"total_cache_hits\":{},\"total_cache_misses\":{},\"records\":[{}]}}",
        report.limits.legacy_face_band_states,
        report.limits.cycle_unique_states,
        report.limits.topology_states,
        report.retained_core_candidates,
        report.family_attempts,
        report.legacy_exact_no_solution,
        report.legacy_closed_unknowns,
        report.legacy_budget_unknowns,
        report.targeted_unknowns,
        report.exact_duplicate_reuses,
        statuses,
        report.total_unique_states,
        report.total_raw_decisions,
        report.total_closed_cycles,
        report.total_cache_hits,
        report.total_cache_misses,
        records,
    )
}

impl FrozenN6CecStatus {
    const ALL: [Self; 4] = [
        Self::Closed,
        Self::ExactNoSolution,
        Self::CycleSearchIncomplete,
        Self::DownstreamSearchIncomplete,
    ];
}

#[derive(Clone)]
struct ReusableClassification {
    status: FrozenN6CecStatus,
    evidence: EssentialCycleFindOneEvidence,
    checkpoint: Option<CycleSearchCheckpoint>,
}

impl ReusableClassification {
    fn from_outcome(outcome: ExactFaceBandV2Outcome) -> Result<Self, String> {
        match outcome {
            ExactFaceBandV2Outcome::Closed { evidence, .. } => Ok(Self {
                status: FrozenN6CecStatus::Closed,
                evidence,
                checkpoint: None,
            }),
            ExactFaceBandV2Outcome::ExactNoSolution { evidence, .. } => Ok(Self {
                status: FrozenN6CecStatus::ExactNoSolution,
                evidence,
                checkpoint: None,
            }),
            ExactFaceBandV2Outcome::CycleSearchIncomplete {
                checkpoint,
                evidence,
            } => Ok(Self {
                status: FrozenN6CecStatus::CycleSearchIncomplete,
                evidence,
                checkpoint: Some(checkpoint),
            }),
            ExactFaceBandV2Outcome::DownstreamSearchIncomplete { evidence } => Ok(Self {
                status: FrozenN6CecStatus::DownstreamSearchIncomplete,
                evidence,
                checkpoint: None,
            }),
            ExactFaceBandV2Outcome::InvalidInput { reason } => Err(reason),
        }
    }
}
