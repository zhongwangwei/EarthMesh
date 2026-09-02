//! Research-only runners. They never produce a product mesh or gate decision.

use super::{
    analyze_stratified_annular_degree_reachability, annular_reachability_storage_audit,
    audit_face_band_boundaries, audit_legacy_downstream_preflight, audit_transition_cell_pairs,
    build_essential_cycle_problem, build_face_band_problem, build_global_incidence_contract,
    build_plan_band_domains, build_stratified_transition_domain_v3,
    enumerate_balanced_annular_strips, face_band_evidence_json, find_one_essential_cycle,
    n12_interior_control_fixture, n12_lifted_n6_fixture, prove_essential_cycle_family,
    solve_exact_face_bands, solve_full_polygon_merge_from_face_bands,
    solve_transition_cell_find_one, AnchorRepairPortfolioEvidence, AnnularEnumerationError,
    AnnularReachabilityLimits, AnnularReachabilityOutcome, AnnularTransitionCellFamily,
    BandBoundaryAudit, BandBoundaryAuditSummary, CertifiedResearchFixture,
    DownstreamEvaluationCache, DownstreamPreflightOutcome, DownstreamRejectStage,
    EssentialCycleFindOneEvidence, EssentialCycleFindOneLimits, EssentialCycleFindOneOutcome,
    EssentialCycleKey, ExactFaceBandV2Outcome, FaceBandAdapterVersion, FaceBandEvidence,
    FaceBandLimits, FaceBandPlan, FaceBandPlanEvaluator, FaceBandSolveOutcome,
    FullPolygonMergeEvidence, FullPolygonMergeLimits, FullPolygonMergeOutcome,
    FullPolygonMergeTrial, FullPolygonPlanEvaluator, GlobalIncidenceContractError,
    PlanBandTopologyKind, PlanEvaluation, RetainedCoreCorridorFamily, TopologyBoundary,
    TopologyFamilyId, TopologyPairClass, TransitionCellDomain, TransitionCellFamily,
    TransitionCellMergeLimits, TransitionCellMergeOutcome, TransitionCellMergeTrial,
};
use crate::certificate::Certificate;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResearchN12OutcomeKind {
    ResearchStrictPass,
    ResearchTopologyClosed,
    ResearchSearchIncomplete,
    ResearchExactNoSolution,
    ResearchContinuousIncomplete,
}

impl ResearchN12OutcomeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ResearchStrictPass => "ResearchStrictPass",
            Self::ResearchTopologyClosed => "ResearchTopologyClosed",
            Self::ResearchSearchIncomplete => "ResearchSearchIncomplete",
            Self::ResearchExactNoSolution => "ResearchExactNoSolution",
            Self::ResearchContinuousIncomplete => "ResearchContinuousIncomplete",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResearchLegacyLimits {
    pub face_band_states: u64,
    pub downstream_topology_states: usize,
}

impl Default for ResearchLegacyLimits {
    fn default() -> Self {
        Self {
            face_band_states: 16_384,
            downstream_topology_states: 4_096,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResearchLegacyEvidence {
    pub fixture_name: String,
    pub limits: ResearchLegacyLimits,
    pub face_band: Option<FaceBandEvidence>,
    pub downstream_states: usize,
    pub topology_candidates_closed: usize,
    pub geometry_attempted: bool,
    pub reason: String,
    pub outcome: ResearchN12OutcomeKind,
    pub product_grid_written: bool,
    pub ready_marker_written: bool,
    pub product_gate_changed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResearchCecTopologyLimits {
    pub cycle_unique_states: u64,
    pub downstream_topology_states: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResearchV3FindOneLimits {
    pub cycle_unique_states: u64,
    pub balanced_topologies_per_cell: usize,
    pub global_topology_states: usize,
    pub ear_states_per_topology: usize,
}

impl Default for ResearchV3FindOneLimits {
    fn default() -> Self {
        Self {
            cycle_unique_states: 16_384,
            balanced_topologies_per_cell: 64,
            global_topology_states: 4_096,
            ear_states_per_topology: 256,
        }
    }
}

impl Default for ResearchCecTopologyLimits {
    fn default() -> Self {
        Self {
            cycle_unique_states: 16_384,
            downstream_topology_states: 4_096,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResearchCecTopologyOutcomeKind {
    ResearchTopologyClosed,
    ResearchExactNoSolution,
    ResearchCycleSearchIncomplete,
    ResearchDownstreamSearchIncomplete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResearchGeometryOutcome {
    StrictCertified,
    ContinuousSearchIncomplete,
    RequiresDifferentTopology,
    ScopedInfeasible,
    InvalidEmbedding,
}

impl ResearchGeometryOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::StrictCertified => "StrictCertified",
            Self::ContinuousSearchIncomplete => "ContinuousSearchIncomplete",
            Self::RequiresDifferentTopology => "RequiresDifferentTopology",
            Self::ScopedInfeasible => "ScopedInfeasible",
            Self::InvalidEmbedding => "InvalidEmbedding",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationGateGovernanceDecision {
    KeepN6ExistenceGate,
    N6StressN12Existence,
    TopologySolverBlocked,
    ContinuousGeometryBlocked,
    PentagonSpecificBlocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationGovernanceDecisionV2 {
    DownstreamAnnulusContractBlocked,
    CycleSolverBlocked,
    DownstreamTopologyBlocked,
    ContinuousGeometryBlocked,
    PentagonSpecificBlocked,
    FixtureCapacityBlocked,
    KeepN6ExistenceGate,
    N6StressN12Existence,
}

impl ValidationGovernanceDecisionV2 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DownstreamAnnulusContractBlocked => "DownstreamAnnulusContractBlocked",
            Self::CycleSolverBlocked => "CycleSolverBlocked",
            Self::DownstreamTopologyBlocked => "DownstreamTopologyBlocked",
            Self::ContinuousGeometryBlocked => "ContinuousGeometryBlocked",
            Self::PentagonSpecificBlocked => "PentagonSpecificBlocked",
            Self::FixtureCapacityBlocked => "FixtureCapacityBlocked",
            Self::KeepN6ExistenceGate => "KeepN6ExistenceGate",
            Self::N6StressN12Existence => "N6Stress_N12Existence",
        }
    }
}

impl ValidationGateGovernanceDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::KeepN6ExistenceGate => "KeepN6ExistenceGate",
            Self::N6StressN12Existence => "N6Stress_N12Existence",
            Self::TopologySolverBlocked => "TopologySolverBlocked",
            Self::ContinuousGeometryBlocked => "ContinuousGeometryBlocked",
            Self::PentagonSpecificBlocked => "PentagonSpecificBlocked",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct N12ValidationGateReport {
    pub lifted_topology: ResearchCecTopologyOutcomeKind,
    pub interior_topology: ResearchCecTopologyOutcomeKind,
    pub lifted_geometry: Option<ResearchGeometryOutcome>,
    pub interior_geometry: Option<ResearchGeometryOutcome>,
    pub decision: ValidationGateGovernanceDecision,
    pub best_mixed_angle_degrees: (f64, f64),
    pub strict_target_degrees: (f64, f64),
    pub product_gate_changed: bool,
    pub research_staircase_unlocked: bool,
    pub nxp80_unlocked: bool,
}

pub fn decide_validation_gate(
    lifted_topology: ResearchCecTopologyOutcomeKind,
    interior_topology: ResearchCecTopologyOutcomeKind,
    lifted_geometry: Option<ResearchGeometryOutcome>,
    interior_geometry: Option<ResearchGeometryOutcome>,
) -> ValidationGateGovernanceDecision {
    use ResearchCecTopologyOutcomeKind as Topology;
    use ResearchGeometryOutcome::StrictCertified;
    if matches!(
        lifted_topology,
        Topology::ResearchCycleSearchIncomplete | Topology::ResearchDownstreamSearchIncomplete
    ) || matches!(
        interior_topology,
        Topology::ResearchCycleSearchIncomplete | Topology::ResearchDownstreamSearchIncomplete
    ) {
        return ValidationGateGovernanceDecision::TopologySolverBlocked;
    }
    if lifted_topology == Topology::ResearchTopologyClosed
        && interior_topology == Topology::ResearchTopologyClosed
    {
        return match (lifted_geometry, interior_geometry) {
            (Some(StrictCertified), Some(StrictCertified)) => {
                ValidationGateGovernanceDecision::N6StressN12Existence
            }
            (lifted, Some(StrictCertified)) if lifted != Some(StrictCertified) => {
                ValidationGateGovernanceDecision::PentagonSpecificBlocked
            }
            _ => ValidationGateGovernanceDecision::ContinuousGeometryBlocked,
        };
    }
    if interior_topology == Topology::ResearchTopologyClosed
        && interior_geometry == Some(StrictCertified)
    {
        ValidationGateGovernanceDecision::PentagonSpecificBlocked
    } else {
        ValidationGateGovernanceDecision::KeepN6ExistenceGate
    }
}

pub fn current_n12_validation_gate_report() -> N12ValidationGateReport {
    let lifted_topology = ResearchCecTopologyOutcomeKind::ResearchCycleSearchIncomplete;
    let interior_topology = ResearchCecTopologyOutcomeKind::ResearchExactNoSolution;
    let lifted_geometry = None;
    let interior_geometry = None;
    N12ValidationGateReport {
        lifted_topology,
        interior_topology,
        lifted_geometry,
        interior_geometry,
        decision: decide_validation_gate(
            lifted_topology,
            interior_topology,
            lifted_geometry,
            interior_geometry,
        ),
        best_mixed_angle_degrees: (39.278_499_430_048, 80.721_500_570_507),
        strict_target_degrees: (40.2, 79.8),
        product_gate_changed: false,
        research_staircase_unlocked: false,
        nxp80_unlocked: false,
    }
}

pub fn n12_validation_gate_report_json(report: &N12ValidationGateReport) -> String {
    format!(
        "{{\"schema_version\":1,\"taskbook_sha256\":\"b327b6afdf199abfaf1a77f4e403ef296e4f5bd2483d855b360c08152a10ae53\",\"research_only\":true,\"lifted_topology\":\"{}\",\"interior_topology\":\"{}\",\"lifted_geometry\":{},\"interior_geometry\":{},\"decision\":\"{}\",\"best_mixed_angle_degrees\":[{:.12},{:.12}],\"strict_target_degrees\":[{:.1},{:.1}],\"product_gate_changed\":{},\"research_staircase_unlocked\":{},\"nxp80_unlocked\":{}}}",
        report.lifted_topology.as_str(),
        report.interior_topology.as_str(),
        report
            .lifted_geometry
            .map_or_else(|| "null".into(), |value| json_string(value.as_str())),
        report
            .interior_geometry
            .map_or_else(|| "null".into(), |value| json_string(value.as_str())),
        report.decision.as_str(),
        report.best_mixed_angle_degrees.0,
        report.best_mixed_angle_degrees.1,
        report.strict_target_degrees.0,
        report.strict_target_degrees.1,
        report.product_gate_changed,
        report.research_staircase_unlocked,
        report.nxp80_unlocked,
    )
}

impl ResearchCecTopologyOutcomeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ResearchTopologyClosed => "ResearchTopologyClosed",
            Self::ResearchExactNoSolution => "ResearchExactNoSolution",
            Self::ResearchCycleSearchIncomplete => "ResearchCycleSearchIncomplete",
            Self::ResearchDownstreamSearchIncomplete => "ResearchDownstreamSearchIncomplete",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResearchCecTopologyEvidence {
    pub fixture_name: String,
    pub limits: ResearchCecTopologyLimits,
    pub transition_faces: usize,
    pub cec: EssentialCycleFindOneEvidence,
    pub checkpoint_shards: usize,
    pub downstream_states: usize,
    pub topology_candidates_closed: usize,
    pub vertices: Option<usize>,
    pub edges: Option<usize>,
    pub faces: Option<usize>,
    pub euler: Option<isize>,
    pub charge: Option<isize>,
    pub anchor_degrees: BTreeMap<usize, usize>,
    pub ordinary_degree_histogram: BTreeMap<usize, usize>,
    pub degree_link_euler_charge_passed: bool,
    pub last_downstream_invalid_reason: Option<String>,
    pub reason: String,
    pub outcome: ResearchCecTopologyOutcomeKind,
    pub geometry_attempted: bool,
    pub product_grid_written: bool,
    pub ready_marker_written: bool,
    pub product_gate_changed: bool,
}

pub fn run_n12_cec_topology_probe(
    fixture: &CertifiedResearchFixture,
    limits: ResearchCecTopologyLimits,
) -> Result<ResearchCecTopologyEvidence, String> {
    run_n12_cec_topology_probe_with_adapter(fixture, limits, FaceBandAdapterVersion::LegacyV1)
}

pub fn run_n12_lifted_v2_replay(
    limits: ResearchCecTopologyLimits,
) -> Result<ResearchCecTopologyEvidence, String> {
    run_n12_cec_topology_probe_with_adapter(
        &n12_lifted_n6_fixture()?,
        limits,
        FaceBandAdapterVersion::TopologyDomainV2,
    )
}

fn run_n12_cec_topology_probe_with_adapter(
    fixture: &CertifiedResearchFixture,
    limits: ResearchCecTopologyLimits,
    adapter: FaceBandAdapterVersion,
) -> Result<ResearchCecTopologyEvidence, String> {
    let face_problem = build_face_band_problem(&fixture.source, &fixture.component, 2)?;
    let cycle_problem = build_essential_cycle_problem(
        &fixture.source,
        &face_problem,
        fixture.component.core_parents.iter().copied(),
        RetainedCoreCorridorFamily::F0CurrentSourceFaceCorridor,
    )?;
    let downstream_limits = FullPolygonMergeLimits {
        topology_states: limits.downstream_topology_states,
    };
    let mut evaluator = match adapter {
        FaceBandAdapterVersion::LegacyV1 => FullPolygonPlanEvaluator::uncached(
            &fixture.source,
            &fixture.component,
            downstream_limits,
        ),
        FaceBandAdapterVersion::TopologyDomainV2 => {
            FullPolygonPlanEvaluator::topology_domain_v2_uncached(
                &fixture.source,
                &fixture.component,
                downstream_limits,
            )
        }
    };
    let outcome = prove_essential_cycle_family(
        &fixture.source,
        &face_problem,
        &cycle_problem,
        EssentialCycleFindOneLimits {
            maximum_unique_states: limits.cycle_unique_states,
        },
        None,
        &mut evaluator,
        &mut DownstreamEvaluationCache::new(),
    );
    let last_downstream_invalid_reason = evaluator.last_invalid_reason().map(str::to_owned);
    let transition_faces = face_problem.transition_faces.len();
    let incomplete = |cec: EssentialCycleFindOneEvidence, checkpoint_shards, reason, outcome| {
        let downstream_states = cec.downstream_states_examined;
        ResearchCecTopologyEvidence {
            fixture_name: fixture.manifest.name.clone(),
            limits,
            transition_faces,
            cec,
            checkpoint_shards,
            downstream_states,
            topology_candidates_closed: 0,
            vertices: None,
            edges: None,
            faces: None,
            euler: None,
            charge: None,
            anchor_degrees: BTreeMap::new(),
            ordinary_degree_histogram: BTreeMap::new(),
            degree_link_euler_charge_passed: false,
            last_downstream_invalid_reason: last_downstream_invalid_reason.clone(),
            reason,
            outcome,
            geometry_attempted: false,
            product_grid_written: false,
            ready_marker_written: false,
            product_gate_changed: false,
        }
    };
    match outcome {
        ExactFaceBandV2Outcome::Closed {
            trial, evidence, ..
        } => {
            let downstream = &trial.evidence;
            let global = &trial.global_trial.evidence;
            let downstream_states = evidence.downstream_states_examined;
            Ok(ResearchCecTopologyEvidence {
                fixture_name: fixture.manifest.name.clone(),
                limits,
                transition_faces,
                cec: evidence,
                checkpoint_shards: 0,
                downstream_states,
                topology_candidates_closed: downstream.topology_candidates_closed,
                vertices: Some(global.vertices),
                edges: Some(global.edges),
                faces: Some(global.faces),
                euler: Some(global.euler),
                charge: Some(global.charge),
                anchor_degrees: global.anchor_degrees.clone(),
                ordinary_degree_histogram: global.ordinary_degree_histogram.clone(),
                degree_link_euler_charge_passed: true,
                last_downstream_invalid_reason: None,
                reason: "CEC and full-polygon degree/link/Euler/charge contracts closed".into(),
                outcome: ResearchCecTopologyOutcomeKind::ResearchTopologyClosed,
                geometry_attempted: false,
                product_grid_written: false,
                ready_marker_written: false,
                product_gate_changed: false,
            })
        }
        ExactFaceBandV2Outcome::ExactNoSolution { evidence, .. } => Ok(incomplete(
            evidence,
            0,
            "canonical essential-cycle family and exact downstream contract are exhausted".into(),
            ResearchCecTopologyOutcomeKind::ResearchExactNoSolution,
        )),
        ExactFaceBandV2Outcome::CycleSearchIncomplete {
            checkpoint,
            evidence,
        } => Ok(incomplete(
            evidence,
            checkpoint.shards.len(),
            "canonical essential-cycle unique-state budget exhausted with a resumable frontier"
                .into(),
            ResearchCecTopologyOutcomeKind::ResearchCycleSearchIncomplete,
        )),
        ExactFaceBandV2Outcome::DownstreamSearchIncomplete { evidence } => Ok(incomplete(
            evidence,
            0,
            "cycle family exhausted but at least one full-polygon evaluation remained incomplete"
                .into(),
            ResearchCecTopologyOutcomeKind::ResearchDownstreamSearchIncomplete,
        )),
        ExactFaceBandV2Outcome::InvalidInput { reason } => Err(reason),
    }
}

pub fn n12_cec_topology_probe_json(limits: ResearchCecTopologyLimits) -> Result<String, String> {
    let reports = [
        run_n12_cec_topology_probe(&n12_lifted_n6_fixture()?, limits)?,
        run_n12_cec_topology_probe(&n12_interior_control_fixture()?, limits)?,
    ];
    Ok(format!(
        "{{\"schema_version\":1,\"taskbook_sha256\":\"b327b6afdf199abfaf1a77f4e403ef296e4f5bd2483d855b360c08152a10ae53\",\"runner\":\"CanonicalEssentialCycle\",\"research_only\":true,\"reports\":[{}]}}",
        reports
            .iter()
            .map(research_cec_topology_evidence_json)
            .collect::<Vec<_>>()
            .join(",")
    ))
}

pub fn research_cec_topology_evidence_json(report: &ResearchCecTopologyEvidence) -> String {
    format!(
        "{{\"fixture\":{},\"limits\":{{\"cycle_unique_states\":{},\"downstream_topology_states\":{}}},\"transition_faces\":{},\"candidate_vertices\":{},\"candidate_edges\":{},\"unique_states\":{},\"raw_decisions\":{},\"propagation_events\":{},\"closed_cycles\":{},\"essential_cycles\":{},\"downstream_exact_rejects\":{},\"downstream_incomplete\":{},\"downstream_invalid\":{},\"checkpoint_shards\":{},\"downstream_states\":{},\"topology_candidates_closed\":{},\"vertices\":{},\"edges\":{},\"faces\":{},\"euler\":{},\"charge\":{},\"anchor_degrees\":{},\"ordinary_degree_histogram\":{},\"degree_link_euler_charge_passed\":{},\"last_downstream_invalid_reason\":{},\"geometry_attempted\":{},\"reason\":{},\"outcome\":\"{}\",\"product_grid_written\":{},\"ready_marker_written\":{},\"product_gate_changed\":{}}}",
        json_string(&report.fixture_name),
        report.limits.cycle_unique_states,
        report.limits.downstream_topology_states,
        report.transition_faces,
        report.cec.candidate_vertices,
        report.cec.candidate_edges,
        report.cec.unique_states,
        report.cec.raw_decisions,
        report.cec.propagation_events,
        report.cec.closed_cycles,
        report.cec.essential_cycles,
        report.cec.downstream_exact_rejects,
        report.cec.downstream_incomplete,
        report.cec.downstream_invalid,
        report.checkpoint_shards,
        report.downstream_states,
        report.topology_candidates_closed,
        json_option(report.vertices),
        json_option(report.edges),
        json_option(report.faces),
        json_option(report.euler),
        json_option(report.charge),
        json_usize_map(&report.anchor_degrees),
        json_usize_map(&report.ordinary_degree_histogram),
        report.degree_link_euler_charge_passed,
        report
            .last_downstream_invalid_reason
            .as_deref()
            .map_or_else(|| "null".into(), json_string),
        report.geometry_attempted,
        json_string(&report.reason),
        report.outcome.as_str(),
        report.product_grid_written,
        report.ready_marker_written,
        report.product_gate_changed,
    )
}

pub fn n12_lifted_v2_replay_json(limits: ResearchCecTopologyLimits) -> Result<String, String> {
    let report = run_n12_lifted_v2_replay(limits)?;
    let histogram = &report.cec.downstream_reject_histogram;
    let (by_stage, by_reason, first_cycles) = reject_histogram_fragments(histogram);
    let inner_guard_rejects = histogram
        .by_reason
        .iter()
        .filter(|(reason, _)| reason.contains("inner_guard"))
        .map(|(_, count)| count)
        .sum::<u64>();
    let reached_topology = report.downstream_states > 0
        || report.cec.downstream_exact_rejects > 0
        || report.cec.downstream_incomplete > 0
        || report.topology_candidates_closed > 0;
    let gate_passed = inner_guard_rejects == 0 && reached_topology;
    Ok(format!(
        "{{\"schema_version\":1,\"taskbook_sha256\":\"63215a9043f5aa87092a78b2910d0c779da3c10e2c749bb4e11d5b0e5b207c5d\",\"fixture\":\"N12-Lifted-N6\",\"adapter_version\":2,\"declared_topology_family\":\"W2CanonicalEssentialCycle+FullPolygon\",\"limits\":{{\"cycle_unique_states\":{},\"downstream_topology_states\":{}}},\"plan_independent_preflight\":\"TopologyDomainV2DefersGeometryGuard\",\"unique_states\":{},\"essential_cycles\":{},\"downstream_states\":{},\"downstream_exact_rejects\":{},\"downstream_incomplete\":{},\"downstream_invalid\":{},\"topology_candidates_closed\":{},\"reject_histogram\":{{\"by_stage\":{{{}}},\"by_reason\":{{{}}},\"first_cycle_by_reason\":{{{}}}}},\"inner_guard_rejects\":{},\"reached_meaningful_downstream\":{},\"gate_passed\":{},\"outcome\":\"{}\",\"geometry_attempted\":false,\"product_grid_written\":false,\"ready_marker_written\":false,\"product_gate_changed\":false}}",
        limits.cycle_unique_states,
        limits.downstream_topology_states,
        report.cec.unique_states,
        report.cec.essential_cycles,
        report.downstream_states,
        report.cec.downstream_exact_rejects,
        report.cec.downstream_incomplete,
        report.cec.downstream_invalid,
        report.topology_candidates_closed,
        by_stage,
        by_reason,
        first_cycles,
        inner_guard_rejects,
        reached_topology,
        gate_passed,
        report.outcome.as_str(),
    ))
}

pub fn n12_lifted_band_failure_audit_json(
    limits: ResearchCecTopologyLimits,
) -> Result<String, String> {
    let fixture = n12_lifted_n6_fixture()?;
    let face_problem = build_face_band_problem(&fixture.source, &fixture.component, 2)?;
    let cycle_problem = build_essential_cycle_problem(
        &fixture.source,
        &face_problem,
        fixture.component.core_parents.iter().copied(),
        RetainedCoreCorridorFamily::F0CurrentSourceFaceCorridor,
    )?;
    let inner = FullPolygonPlanEvaluator::topology_domain_v2_uncached(
        &fixture.source,
        &fixture.component,
        FullPolygonMergeLimits {
            topology_states: limits.downstream_topology_states,
        },
    );
    let mut evaluator = BandAuditingEvaluator {
        source: &fixture.source,
        component: &fixture.component,
        inner,
        summary: BandBoundaryAuditSummary::default(),
        first_error: None,
    };
    let outcome = prove_essential_cycle_family(
        &fixture.source,
        &face_problem,
        &cycle_problem,
        EssentialCycleFindOneLimits {
            maximum_unique_states: limits.cycle_unique_states,
        },
        None,
        &mut evaluator,
        &mut DownstreamEvaluationCache::new(),
    );
    let evidence = match outcome {
        ExactFaceBandV2Outcome::Closed { evidence, .. }
        | ExactFaceBandV2Outcome::ExactNoSolution { evidence, .. }
        | ExactFaceBandV2Outcome::CycleSearchIncomplete { evidence, .. }
        | ExactFaceBandV2Outcome::DownstreamSearchIncomplete { evidence } => evidence,
        ExactFaceBandV2Outcome::InvalidInput { reason } => return Err(reason),
    };
    if let Some(error) = evaluator.first_error {
        return Err(error);
    }
    let summary = evaluator.summary;
    let all_rejections_histogrammed = summary.cycles_audited == evidence.essential_cycles;
    let histogram = summary
        .by_band_and_failure
        .iter()
        .map(|(&(band, failure), count)| format!("\"band{band}.{}\":{count}", failure.as_str()))
        .collect::<Vec<_>>()
        .join(",");
    let first_audits = summary
        .first_audit_by_band_and_failure
        .iter()
        .map(|(&(band, failure), audit)| {
            let cycle = &summary.first_cycle_by_band_and_failure[&(band, failure)];
            format!(
                "\"band{band}.{}\":{{\"cycle\":{},\"audit\":{}}}",
                failure.as_str(),
                json_string(&format!("{cycle:?}")),
                band_boundary_audit_json(audit),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    Ok(format!(
        "{{\"schema_version\":1,\"taskbook_sha256\":\"cb911eef1de3593df10d042bf72ce3707080d2b521ceb074d36b8b05cfe4b63e\",\"fixture\":\"N12-Lifted-N6\",\"adapter_version\":2,\"declared_topology_family\":\"W2CanonicalEssentialCycle+LegacyStratifiedAuditOnly\",\"limits\":{{\"cycle_unique_states\":{},\"downstream_topology_states\":{}}},\"unique_states\":{},\"essential_cycles\":{},\"cycles_audited\":{},\"bands_audited\":{},\"topological_annuli\":{},\"topology_contract_failures\":{},\"failure_histogram\":{{{}}},\"first_evidence\":{{{}}},\"all_rejections_histogrammed\":{},\"conclusion\":\"{}\",\"solver_changed\":false,\"geometry_attempted\":false,\"product_gate_changed\":false}}",
        limits.cycle_unique_states,
        limits.downstream_topology_states,
        evidence.unique_states,
        evidence.essential_cycles,
        summary.cycles_audited,
        summary.bands_audited,
        summary.topological_annuli,
        summary.topology_contract_failures,
        histogram,
        first_audits,
        all_rejections_histogrammed,
        summary.conclusion().as_str(),
    ))
}

pub fn n12_lifted_plan_band_domain_audit_json(
    limits: ResearchCecTopologyLimits,
) -> Result<String, String> {
    let fixture = n12_lifted_n6_fixture()?;
    let face_problem = build_face_band_problem(&fixture.source, &fixture.component, 2)?;
    let cycle_problem = build_essential_cycle_problem(
        &fixture.source,
        &face_problem,
        fixture.component.core_parents.iter().copied(),
        RetainedCoreCorridorFamily::F0CurrentSourceFaceCorridor,
    )?;
    let mut evaluator = PlanBandDomainAuditingEvaluator {
        source: &fixture.source,
        component: &fixture.component,
        cycles_observed: 0,
        plans_built: 0,
        bands_built: 0,
        annular_bands: 0,
        contracted_band0: 0,
        first_boundary_sizes: None,
        errors: BTreeMap::new(),
    };
    let outcome = prove_essential_cycle_family(
        &fixture.source,
        &face_problem,
        &cycle_problem,
        EssentialCycleFindOneLimits {
            maximum_unique_states: limits.cycle_unique_states,
        },
        None,
        &mut evaluator,
        &mut DownstreamEvaluationCache::new(),
    );
    let evidence = match outcome {
        ExactFaceBandV2Outcome::Closed { evidence, .. }
        | ExactFaceBandV2Outcome::ExactNoSolution { evidence, .. }
        | ExactFaceBandV2Outcome::CycleSearchIncomplete { evidence, .. }
        | ExactFaceBandV2Outcome::DownstreamSearchIncomplete { evidence } => evidence,
        ExactFaceBandV2Outcome::InvalidInput { reason } => return Err(reason),
    };
    let expected_bands = evidence.essential_cycles.saturating_mul(2);
    let gate_passed = evaluator.cycles_observed == evidence.essential_cycles
        && evaluator.plans_built == evidence.essential_cycles
        && evaluator.bands_built == expected_bands
        && evaluator.annular_bands == expected_bands
        && evaluator.contracted_band0 == evidence.essential_cycles
        && evaluator.errors.is_empty();
    let errors = evaluator
        .errors
        .iter()
        .map(|(error, count)| format!("{}:{count}", json_string(error)))
        .collect::<Vec<_>>()
        .join(",");
    let boundary_sizes = evaluator.first_boundary_sizes.map_or_else(
        || "null".into(),
        |(band0_topology, band0_source, internal, fine)| {
            format!(
                "{{\"band0_topology\":{band0_topology},\"band0_source\":{band0_source},\"internal\":{internal},\"fine\":{fine}}}"
            )
        },
    );
    Ok(format!(
        "{{\"schema_version\":1,\"taskbook_sha256\":\"cb911eef1de3593df10d042bf72ce3707080d2b521ceb074d36b8b05cfe4b63e\",\"fixture\":\"N12-Lifted-N6\",\"declared_topology_family\":\"W2CanonicalEssentialCycle+PlanBandDomainAuditOnly\",\"limits\":{{\"cycle_unique_states\":{},\"downstream_topology_states\":{}}},\"unique_states\":{},\"essential_cycles\":{},\"cycles_observed\":{},\"plans_built\":{},\"bands_built\":{},\"annular_bands\":{},\"contracted_band0\":{},\"first_boundary_edge_counts\":{},\"errors\":{{{}}},\"gate_passed\":{},\"cec_outcome\":\"{}\",\"coupled_annulus_used\":false,\"sectors_built\":false,\"full_polygon_run\":false,\"geometry_attempted\":false,\"product_gate_changed\":false}}",
        limits.cycle_unique_states,
        limits.downstream_topology_states,
        evidence.unique_states,
        evidence.essential_cycles,
        evaluator.cycles_observed,
        evaluator.plans_built,
        evaluator.bands_built,
        evaluator.annular_bands,
        evaluator.contracted_band0,
        boundary_sizes,
        errors,
        gate_passed,
        evidence.outcome.as_str(),
    ))
}

pub fn n12_lifted_transition_cell_v3_audit_json(
    limits: ResearchCecTopologyLimits,
) -> Result<String, String> {
    let fixture = n12_lifted_n6_fixture()?;
    let face_problem = build_face_band_problem(&fixture.source, &fixture.component, 2)?;
    let cycle_problem = build_essential_cycle_problem(
        &fixture.source,
        &face_problem,
        fixture.component.core_parents.iter().copied(),
        RetainedCoreCorridorFamily::F0CurrentSourceFaceCorridor,
    )?;
    let mut evaluator = V3DomainAuditingEvaluator {
        source: &fixture.source,
        component: &fixture.component,
        cycles_observed: 0,
        domains_built: 0,
        annular_cells: 0,
        disk_cells: 0,
        first_link_contracts: None,
        errors: BTreeMap::new(),
    };
    let outcome = prove_essential_cycle_family(
        &fixture.source,
        &face_problem,
        &cycle_problem,
        EssentialCycleFindOneLimits {
            maximum_unique_states: limits.cycle_unique_states,
        },
        None,
        &mut evaluator,
        &mut DownstreamEvaluationCache::new(),
    );
    let evidence = match outcome {
        ExactFaceBandV2Outcome::Closed { evidence, .. }
        | ExactFaceBandV2Outcome::ExactNoSolution { evidence, .. }
        | ExactFaceBandV2Outcome::CycleSearchIncomplete { evidence, .. }
        | ExactFaceBandV2Outcome::DownstreamSearchIncomplete { evidence } => evidence,
        ExactFaceBandV2Outcome::InvalidInput { reason } => return Err(reason),
    };
    let expected_cells = evidence.essential_cycles.saturating_mul(2);
    let gate_passed = evaluator.cycles_observed == evidence.essential_cycles
        && evaluator.domains_built == evidence.essential_cycles
        && evaluator.annular_cells == expected_cells
        && evaluator.disk_cells == 0
        && evaluator.errors.is_empty();
    let errors = evaluator
        .errors
        .iter()
        .map(|(error, count)| format!("{}:{count}", json_string(error)))
        .collect::<Vec<_>>()
        .join(",");
    Ok(format!(
        "{{\"schema_version\":1,\"taskbook_sha256\":\"cb911eef1de3593df10d042bf72ce3707080d2b521ceb074d36b8b05cfe4b63e\",\"fixture\":\"N12-Lifted-N6\",\"declared_topology_family\":\"W2CanonicalEssentialCycle+TransitionCellV3AuditOnly\",\"limits\":{{\"cycle_unique_states\":{},\"downstream_topology_states\":{}}},\"unique_states\":{},\"essential_cycles\":{},\"cycles_observed\":{},\"domains_built\":{},\"annular_cells\":{},\"disk_cells\":{},\"first_link_contracts\":{},\"errors\":{{{}}},\"gate_passed\":{},\"coupled_annulus_used\":false,\"legacy_monotone_connectors_used\":false,\"full_polygon_run\":false,\"geometry_attempted\":false,\"product_gate_changed\":false}}",
        limits.cycle_unique_states,
        limits.downstream_topology_states,
        evidence.unique_states,
        evidence.essential_cycles,
        evaluator.cycles_observed,
        evaluator.domains_built,
        evaluator.annular_cells,
        evaluator.disk_cells,
        evaluator
            .first_link_contracts
            .map_or_else(|| "null".into(), |count| count.to_string()),
        errors,
        gate_passed,
    ))
}

pub fn n12_lifted_sdce_incidence_contract_audit_json(
    limits: ResearchCecTopologyLimits,
) -> Result<String, String> {
    let fixture = n12_lifted_n6_fixture()?;
    let face_problem = build_face_band_problem(&fixture.source, &fixture.component, 2)?;
    let cycle_problem = build_essential_cycle_problem(
        &fixture.source,
        &face_problem,
        fixture.component.core_parents.iter().copied(),
        RetainedCoreCorridorFamily::F0CurrentSourceFaceCorridor,
    )?;
    let mut evaluator = SdceContractAuditingEvaluator::new(&fixture.source, &fixture.component);
    let outcome = prove_essential_cycle_family(
        &fixture.source,
        &face_problem,
        &cycle_problem,
        EssentialCycleFindOneLimits {
            maximum_unique_states: limits.cycle_unique_states,
        },
        None,
        &mut evaluator,
        &mut DownstreamEvaluationCache::new(),
    );
    let evidence = match outcome {
        ExactFaceBandV2Outcome::Closed { evidence, .. }
        | ExactFaceBandV2Outcome::ExactNoSolution { evidence, .. }
        | ExactFaceBandV2Outcome::CycleSearchIncomplete { evidence, .. }
        | ExactFaceBandV2Outcome::DownstreamSearchIncomplete { evidence } => evidence,
        ExactFaceBandV2Outcome::InvalidInput { reason } => return Err(reason),
    };
    let gate_passed = evaluator.cycles_observed == evidence.essential_cycles
        && evaluator.domains_built == evidence.essential_cycles
        && evaluator.contracts_built == evidence.essential_cycles
        && evaluator.empty_vertex_domains == 0
        && evaluator.invalid_cell_sums == 0
        && evaluator.errors.is_empty();
    let errors = evaluator
        .errors
        .iter()
        .map(|(error, count)| format!("{}:{count}", json_string(error)))
        .collect::<Vec<_>>()
        .join(",");
    let charges = evaluator
        .transition_charges
        .iter()
        .map(|(charge, count)| format!("{}:{count}", json_string(&charge.to_string())))
        .collect::<Vec<_>>()
        .join(",");
    let defect_roles = evaluator
        .defect_roles
        .iter()
        .map(|(slot, audit)| {
            let fixed_degrees = audit
                .fixed_degrees
                .iter()
                .map(|(degree, count)| format!("{}:{count}", json_string(&degree.to_string())))
                .collect::<Vec<_>>()
                .join(",");
            let owner_counts = audit
                .owner_counts
                .iter()
                .map(|(owners, count)| format!("{}:{count}", json_string(&owners.to_string())))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{}:{{\"cycles_present\":{},\"fixed_degrees\":{{{}}},\"owner_counts\":{{{}}},\"tuple_count_min\":{},\"tuple_count_max\":{},\"anchor_kind\":\"Ordinary\"}}",
                json_string(&slot.to_string()),
                audit.cycles_present,
                fixed_degrees,
                owner_counts,
                audit.tuple_count_min.unwrap_or(0),
                audit.tuple_count_max,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let storage = annular_reachability_storage_audit();
    Ok(format!(
        "{{\"schema_version\":1,\"taskbook_sha256\":\"65f26b64c78dd7dfadaaf2a1099f52d11c6a67461afb0a9558edbbf5941ef473\",\"fixture\":\"N12-Lifted-N6\",\"declared_topology_family\":\"W2CanonicalEssentialCycle+TransitionCellV3+SdceIncidenceContractOnly\",\"limits\":{{\"cycle_unique_states\":{},\"concrete_topology_states\":0}},\"unique_states\":{},\"essential_cycles\":{},\"cycles_observed\":{},\"domains_built\":{},\"contracts_built\":{},\"contract_vertices\":{},\"contract_owner_tuples\":{},\"empty_vertex_domains\":{},\"invalid_cell_sums\":{},\"transition_charge_histogram\":{{{}}},\"pr111_defect_vertex_roles\":{{{}}},\"reachability_storage\":{{\"incidence_signatures\":{},\"link_path_signatures\":{},\"member_counts\":{},\"concrete_witnesses\":{},\"backpointers\":{},\"necessary_relaxation_only\":{}}},\"errors\":{{{}}},\"all_vertex_domains_nonempty\":{},\"adapter_mismatch\":{},\"gate_passed\":{},\"go_no_go\":\"{}\",\"concrete_topology_search_run\":false,\"geometry_attempted\":false,\"remaining_cec_shards\":49,\"cec_shards_resumed\":false,\"product_gate_changed\":false}}",
        limits.cycle_unique_states,
        evidence.unique_states,
        evidence.essential_cycles,
        evaluator.cycles_observed,
        evaluator.domains_built,
        evaluator.contracts_built,
        evaluator.contract_vertices,
        evaluator.contract_owner_tuples,
        evaluator.empty_vertex_domains,
        evaluator.invalid_cell_sums,
        charges,
        defect_roles,
        storage.stores_incidence_signatures,
        storage.stores_link_path_signatures,
        storage.stores_member_counts,
        storage.stores_concrete_witnesses,
        storage.stores_backpointers,
        storage.necessary_relaxation_only,
        errors,
        evaluator.empty_vertex_domains == 0,
        evaluator.adapter_mismatches,
        gate_passed,
        if gate_passed {
            "GoGlobalIncidencePlanCsp"
        } else {
            "StopFixIncidenceContract"
        },
    ))
}

pub fn n12_lifted_v3_prefix_replay_json(
    limits: ResearchCecTopologyLimits,
) -> Result<String, String> {
    let fixture = n12_lifted_n6_fixture()?;
    let face_problem = build_face_band_problem(&fixture.source, &fixture.component, 2)?;
    let cycle_problem = build_essential_cycle_problem(
        &fixture.source,
        &face_problem,
        fixture.component.core_parents.iter().copied(),
        RetainedCoreCorridorFamily::F0CurrentSourceFaceCorridor,
    )?;
    let mut evaluator = V3PrefixReplayEvaluator {
        source: &fixture.source,
        component: &fixture.component,
        signature_states: limits.downstream_topology_states,
        domains_built: 0,
        reached_annular_reachability: 0,
        necessary_feasible: 0,
        exact_rejects: 0,
        search_incomplete: 0,
        root_bridges_considered: 0,
        reachability_states: 0,
        degree_cap_prunes: 0,
        ac3_prunes: 0,
        concrete_enumeration_deferred: 0,
        errors: BTreeMap::new(),
    };
    let outcome = prove_essential_cycle_family(
        &fixture.source,
        &face_problem,
        &cycle_problem,
        EssentialCycleFindOneLimits {
            maximum_unique_states: limits.cycle_unique_states,
        },
        None,
        &mut evaluator,
        &mut DownstreamEvaluationCache::new(),
    );
    let topology_closed = matches!(&outcome, ExactFaceBandV2Outcome::Closed { .. });
    let evidence = match outcome {
        ExactFaceBandV2Outcome::Closed { evidence, .. }
        | ExactFaceBandV2Outcome::ExactNoSolution { evidence, .. }
        | ExactFaceBandV2Outcome::CycleSearchIncomplete { evidence, .. }
        | ExactFaceBandV2Outcome::DownstreamSearchIncomplete { evidence } => evidence,
        ExactFaceBandV2Outcome::InvalidInput { reason } => return Err(reason),
    };
    let by_stage = evidence
        .downstream_reject_histogram
        .by_stage
        .iter()
        .map(|(stage, count)| format!("{}:{count}", json_string(stage.as_str())))
        .collect::<Vec<_>>()
        .join(",");
    let errors = evaluator
        .errors
        .iter()
        .map(|(reason, count)| format!("{}:{count}", json_string(reason)))
        .collect::<Vec<_>>()
        .join(",");
    let meaningful_downstream = topology_closed
        || evidence.downstream_exact_rejects > 0
        || evidence.downstream_incomplete > 0;
    let no_legacy_sector_reject = !evidence
        .downstream_reject_histogram
        .by_stage
        .contains_key(&DownstreamRejectStage::StratifiedSectorization);
    let gate_passed = evaluator.domains_built == evidence.essential_cycles
        && evaluator.reached_annular_reachability > 0
        && meaningful_downstream
        && no_legacy_sector_reject
        && evidence.downstream_invalid == 0
        && evaluator.errors.is_empty();
    Ok(format!(
        "{{\"schema_version\":1,\"taskbook_sha256\":\"cb911eef1de3593df10d042bf72ce3707080d2b521ceb074d36b8b05cfe4b63e\",\"fixture\":\"N12-Lifted-N6\",\"adapter_version\":3,\"declared_topology_family\":\"W2CanonicalEssentialCycle+TransitionCellV3+AnnularReachability\",\"limits\":{{\"cycle_unique_states\":{},\"annular_signature_states_per_cycle\":{}}},\"unique_states\":{},\"essential_cycles\":{},\"domains_built\":{},\"reached_annular_reachability\":{},\"reachability_necessary_feasible\":{},\"reachability_exact_rejects\":{},\"reachability_search_incomplete\":{},\"root_bridges_considered\":{},\"reachability_states\":{},\"degree_cap_prunes\":{},\"ac3_prunes\":{},\"concrete_enumeration_deferred\":{},\"downstream_states\":{},\"downstream_exact_rejects\":{},\"downstream_incomplete\":{},\"downstream_invalid\":{},\"topology_closed\":{},\"rejects_by_stage\":{{{}}},\"errors\":{{{}}},\"remaining_resumable_shards\":{},\"shards_resumed\":false,\"legacy_sectorization_used\":false,\"gate_passed\":{},\"cec_outcome\":\"{}\",\"geometry_attempted\":false,\"product_gate_changed\":false}}",
        limits.cycle_unique_states,
        limits.downstream_topology_states,
        evidence.unique_states,
        evidence.essential_cycles,
        evaluator.domains_built,
        evaluator.reached_annular_reachability,
        evaluator.necessary_feasible,
        evaluator.exact_rejects,
        evaluator.search_incomplete,
        evaluator.root_bridges_considered,
        evaluator.reachability_states,
        evaluator.degree_cap_prunes,
        evaluator.ac3_prunes,
        evaluator.concrete_enumeration_deferred,
        evidence.downstream_states_examined,
        evidence.downstream_exact_rejects,
        evidence.downstream_incomplete,
        evidence.downstream_invalid,
        topology_closed,
        by_stage,
        errors,
        evidence.checkpoint_shards_created,
        gate_passed,
        evidence.outcome.as_str(),
    ))
}

pub fn n12_lifted_v3_find_one_json(limits: ResearchV3FindOneLimits) -> Result<String, String> {
    let fixture = n12_lifted_n6_fixture()?;
    let face_problem = build_face_band_problem(&fixture.source, &fixture.component, 2)?;
    let cycle_problem = build_essential_cycle_problem(
        &fixture.source,
        &face_problem,
        fixture.component.core_parents.iter().copied(),
        RetainedCoreCorridorFamily::F0CurrentSourceFaceCorridor,
    )?;
    let mut evaluator = V3BalancedFindOneEvaluator {
        source: &fixture.source,
        component: &fixture.component,
        topology_limit: limits.balanced_topologies_per_cell,
        global_limit: limits.global_topology_states,
        ear_limit: limits.ear_states_per_topology,
        current_cycle: None,
        domains_built: 0,
        candidates_examined: 0,
        topologies_generated: 0,
        exhaustive_cell_subsets: 0,
        partial_cell_subsets: 0,
        global_states: 0,
        ear_states: 0,
        closed_cycle: None,
        closed: None,
        errors: BTreeMap::new(),
    };
    let outcome = find_one_essential_cycle(
        &fixture.source,
        &face_problem,
        &cycle_problem,
        EssentialCycleFindOneLimits {
            maximum_unique_states: limits.cycle_unique_states,
        },
        &mut evaluator,
    );
    let evidence = outcome.evidence().ok_or_else(|| match &outcome {
        EssentialCycleFindOneOutcome::InvalidInput { reason } => reason.clone(),
        _ => "Lifted V3 find-one returned no evidence".into(),
    })?;
    let topology_closed = matches!(outcome, EssentialCycleFindOneOutcome::Closed { .. });
    let outcome_kind = match &outcome {
        EssentialCycleFindOneOutcome::Closed { .. } => "Closed",
        EssentialCycleFindOneOutcome::CycleSearchIncomplete { .. } => "CycleSearchIncomplete",
        EssentialCycleFindOneOutcome::DownstreamSearchIncomplete { .. } => {
            "DownstreamSearchIncomplete"
        }
        EssentialCycleFindOneOutcome::InvalidInput { .. } => unreachable!(),
    };
    let global = evaluator.closed.as_ref().map(|closed| &closed.global);
    let selected_keys = evaluator.closed.as_ref().map_or_else(
        || "[]".into(),
        |closed| {
            format!(
                "[{}]",
                closed
                    .selected_annular_keys
                    .iter()
                    .map(|key| json_string(&format!("{key:?}")))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        },
    );
    let errors = evaluator
        .errors
        .iter()
        .map(|(reason, count)| format!("{}:{count}", json_string(reason)))
        .collect::<Vec<_>>()
        .join(",");
    Ok(format!(
        "{{\"schema_version\":1,\"taskbook_sha256\":\"cb911eef1de3593df10d042bf72ce3707080d2b521ceb074d36b8b05cfe4b63e\",\"fixture\":\"N12-Lifted-N6\",\"adapter_version\":3,\"declared_topology_family\":\"W2CanonicalEssentialCycle+TransitionCellV3+BalancedAnnularStripFindOne\",\"limits\":{{\"cycle_unique_states\":{},\"balanced_topologies_per_cell\":{},\"global_topology_states\":{},\"ear_states_per_topology\":{}}},\"unique_states\":{},\"essential_cycles_examined\":{},\"domains_built\":{},\"balanced_candidates_examined\":{},\"concrete_topologies_generated\":{},\"exhaustive_cell_subsets\":{},\"partial_cell_subsets\":{},\"global_states\":{},\"ear_states\":{},\"downstream_incomplete\":{},\"topology_closed\":{},\"closed_cycle\":{},\"selected_annular_topology_keys\":{},\"vertices\":{},\"edges\":{},\"faces\":{},\"euler\":{},\"charge\":{},\"anchor_degrees\":{},\"ordinary_degree_histogram\":{},\"errors\":{{{}}},\"remaining_resumable_shards\":49,\"shards_resumed\":false,\"geometry_attempted\":false,\"outcome\":\"{}\",\"product_grid_written\":false,\"ready_marker_written\":false,\"product_gate_changed\":false}}",
        limits.cycle_unique_states,
        limits.balanced_topologies_per_cell,
        limits.global_topology_states,
        limits.ear_states_per_topology,
        evidence.unique_states,
        evidence.essential_cycles,
        evaluator.domains_built,
        evaluator.candidates_examined,
        evaluator.topologies_generated,
        evaluator.exhaustive_cell_subsets,
        evaluator.partial_cell_subsets,
        evaluator.global_states,
        evaluator.ear_states,
        evidence.downstream_incomplete,
        topology_closed,
        evaluator
            .closed_cycle
            .as_ref()
            .map_or_else(|| "null".into(), |cycle| json_string(&format!("{cycle:?}"))),
        selected_keys,
        json_option(global.map(|evidence| evidence.vertices)),
        json_option(global.map(|evidence| evidence.edges)),
        json_option(global.map(|evidence| evidence.faces)),
        json_option(global.map(|evidence| evidence.euler)),
        json_option(global.map(|evidence| evidence.charge)),
        global.map_or_else(|| "null".into(), |evidence| json_usize_map(&evidence.anchor_degrees)),
        global.map_or_else(
            || "null".into(),
            |evidence| json_usize_map(&evidence.ordinary_degree_histogram),
        ),
        errors,
        outcome_kind,
    ))
}

pub fn n12_lifted_fair_pair_audit_json(limits: ResearchV3FindOneLimits) -> Result<String, String> {
    let fixture = n12_lifted_n6_fixture()?;
    let face_problem = build_face_band_problem(&fixture.source, &fixture.component, 2)?;
    let cycle_problem = build_essential_cycle_problem(
        &fixture.source,
        &face_problem,
        fixture.component.core_parents.iter().copied(),
        RetainedCoreCorridorFamily::F0CurrentSourceFaceCorridor,
    )?;
    let mut evaluator = FairPairAuditingEvaluator::new(
        &fixture.source,
        &fixture.component,
        limits.balanced_topologies_per_cell,
    );
    let outcome = prove_essential_cycle_family(
        &fixture.source,
        &face_problem,
        &cycle_problem,
        EssentialCycleFindOneLimits {
            maximum_unique_states: limits.cycle_unique_states,
        },
        None,
        &mut evaluator,
        &mut DownstreamEvaluationCache::new(),
    );
    let evidence = match outcome {
        ExactFaceBandV2Outcome::Closed { evidence, .. }
        | ExactFaceBandV2Outcome::ExactNoSolution { evidence, .. }
        | ExactFaceBandV2Outcome::CycleSearchIncomplete { evidence, .. }
        | ExactFaceBandV2Outcome::DownstreamSearchIncomplete { evidence } => evidence,
        ExactFaceBandV2Outcome::InvalidInput { reason } => return Err(reason),
    };
    let low_ear_pairs = evaluator
        .low_ear_pairs
        .iter()
        .map(|(ears, count)| format!("\"{ears}\":{count}"))
        .collect::<Vec<_>>()
        .join(",");
    let anchor_degrees = evaluator
        .anchor_degree_histogram
        .iter()
        .map(|(degree, count)| format!("\"{degree}\":{count}"))
        .collect::<Vec<_>>()
        .join(",");
    let first_rank_mean = if evaluator.cycles_audited == 0 {
        0.0
    } else {
        evaluator.first_pair_rank_sum as f64 / evaluator.cycles_audited as f64
    };
    let pair_accounting_complete = evaluator.pair_product
        == evaluator.zero_ear_pairs + evaluator.repairable_pairs + evaluator.impossible_pairs;
    let cycle_accounting_complete = evaluator.cycles_audited == evidence.essential_cycles
        && evaluator.domains_built == evaluator.cycles_audited;
    let audit_gate_passed =
        pair_accounting_complete && cycle_accounting_complete && evaluator.errors.is_empty();
    let go_no_go = if !audit_gate_passed {
        "AuditInvalid"
    } else if evaluator.direct_zero_ear_closures > 0 {
        "GoZeroEarFastPath"
    } else if evaluator
        .low_ear_pairs
        .iter()
        .any(|(&ears, &count)| ears <= 2 && count > 0)
    {
        "GoFairLowEarScheduler"
    } else if evaluator.repairable_pairs > 0 {
        "GoAnchorRepairVariants"
    } else {
        "GoSignatureDirectedConcreteExtraction"
    };
    let telemetry = evaluator
        .first_ear_telemetry
        .as_ref()
        .map_or_else(|| "null".into(), anchor_ear_telemetry_json);
    Ok(format!(
        "{{\"schema_version\":1,\"taskbook_sha256\":\"deb33122775ef6086b4fe8a146324a2729be8f298ed2283ac25391197eef94a8\",\"fixture\":\"N12-Lifted-N6\",\"declared_topology_family\":\"W2CanonicalEssentialCycle+TransitionCellV3+BalancedAnnularPairAudit\",\"limits\":{{\"cycle_unique_states\":{},\"balanced_topologies_per_cell\":{}}},\"unique_states\":{},\"essential_cycles\":{},\"cycles_audited\":{},\"domains_built\":{},\"cell_topologies\":{},\"global_pair_product\":{},\"zero_ear_pairs\":{},\"direct_zero_ear_closures\":{},\"repairable_pairs\":{},\"impossible_pairs\":{},\"pair_accounting_complete\":{},\"cycle_accounting_complete\":{},\"low_ear_pairs\":{{{}}},\"anchor_degree_histogram\":{{{}}},\"impossible_reasons\":{{{}}},\"zero_ear_final_rejects\":{{{}}},\"first_pair_rank\":{{\"min\":{},\"max\":{},\"mean\":{:.6}}},\"first_pair_classes\":{{{}}},\"best_pair_classes\":{{{}}},\"first_pair_ordinary_defects\":{{{}}},\"first_pair_unmatched_edges\":{{{}}},\"first_pair_broken_links\":{{{}}},\"first_pair_ear_outcome\":{},\"first_pair_ear_trace\":{},\"errors\":{{{}}},\"audit_gate_passed\":{},\"go_no_go\":\"{}\",\"remaining_cec_shards\":{},\"cec_shards_resumed\":false,\"search_result_changed\":false,\"geometry_attempted\":false,\"product_gate_changed\":false}}",
        limits.cycle_unique_states,
        limits.balanced_topologies_per_cell,
        evidence.unique_states,
        evidence.essential_cycles,
        evaluator.cycles_audited,
        evaluator.domains_built,
        evaluator.cell_topologies,
        evaluator.pair_product,
        evaluator.zero_ear_pairs,
        evaluator.direct_zero_ear_closures,
        evaluator.repairable_pairs,
        evaluator.impossible_pairs,
        pair_accounting_complete,
        cycle_accounting_complete,
        low_ear_pairs,
        anchor_degrees,
        json_string_count_map(&evaluator.impossible_reasons),
        json_string_count_map(&evaluator.zero_ear_final_rejects),
        if evaluator.first_pair_rank_min == usize::MAX {
            0
        } else {
            evaluator.first_pair_rank_min
        },
        evaluator.first_pair_rank_max,
        first_rank_mean,
        json_string_count_map(&evaluator.first_pair_classes),
        json_string_count_map(&evaluator.best_pair_classes),
        json_usize_count_map(&evaluator.first_pair_ordinary_defects),
        json_usize_count_map(&evaluator.first_pair_unmatched_edges),
        json_usize_count_map(&evaluator.first_pair_broken_links),
        evaluator
            .first_ear_outcome
            .as_deref()
            .map_or_else(|| "null".into(), json_string),
        telemetry,
        json_string_count_map(&evaluator.errors),
        audit_gate_passed,
        go_no_go,
        evidence.checkpoint_shards_created,
    ))
}

pub fn n12_lifted_r2_repair_support_audit_json(
    limits: ResearchV3FindOneLimits,
) -> Result<String, String> {
    let fixture = n12_lifted_n6_fixture()?;
    let face_problem = build_face_band_problem(&fixture.source, &fixture.component, 2)?;
    let cycle_problem = build_essential_cycle_problem(
        &fixture.source,
        &face_problem,
        fixture.component.core_parents.iter().copied(),
        RetainedCoreCorridorFamily::F0CurrentSourceFaceCorridor,
    )?;
    let mut evaluator = FairPairAuditingEvaluator::new(
        &fixture.source,
        &fixture.component,
        limits.balanced_topologies_per_cell,
    );
    let outcome = prove_essential_cycle_family(
        &fixture.source,
        &face_problem,
        &cycle_problem,
        EssentialCycleFindOneLimits {
            maximum_unique_states: limits.cycle_unique_states,
        },
        None,
        &mut evaluator,
        &mut DownstreamEvaluationCache::new(),
    );
    let evidence = match outcome {
        ExactFaceBandV2Outcome::Closed { evidence, .. }
        | ExactFaceBandV2Outcome::ExactNoSolution { evidence, .. }
        | ExactFaceBandV2Outcome::CycleSearchIncomplete { evidence, .. }
        | ExactFaceBandV2Outcome::DownstreamSearchIncomplete { evidence } => evidence,
        ExactFaceBandV2Outcome::InvalidInput { reason } => return Err(reason),
    };
    let portfolio = &evaluator.anchor_repair_portfolio;
    let preflight_rejected = portfolio
        .r2_anchor_necessary_candidates
        .saturating_sub(portfolio.r2_preflight_passed);
    let pair_accounting_complete = portfolio.pair_total
        == portfolio.direct_no_ear_candidates
            + portfolio.permanent_impossible
            + portfolio.outside_r2
            + portfolio.r2_preflight_passed;
    let r2_candidate_accounting_complete = portfolio.r2_anchor_necessary_candidates
        == portfolio.r2_preflight_passed + preflight_rejected;
    let k_tier_accounting_complete =
        portfolio.k_tiers.values().sum::<usize>() == portfolio.r2_anchor_necessary_candidates;
    let cycle_accounting_complete = evaluator.cycles_audited == evidence.essential_cycles
        && evaluator.domains_built == evaluator.cycles_audited;
    let audit_gate_passed = pair_accounting_complete
        && r2_candidate_accounting_complete
        && k_tier_accounting_complete
        && cycle_accounting_complete
        && evaluator.errors.is_empty();
    let go_no_go = if !audit_gate_passed {
        "AuditInvalid"
    } else if portfolio.r2_preflight_passed > 0 {
        "GoAsveR2"
    } else if portfolio.r2_anchor_necessary_candidates > 0 {
        "GoSignatureDirectedConcreteExtraction"
    } else if portfolio.outside_r2 > 0 {
        "GoR3FeasibilityAudit"
    } else {
        "GoSignatureDirectedConcreteExtraction"
    };
    let k_tiers = portfolio
        .k_tiers
        .iter()
        .map(|(ears, count)| format!("\"{ears}\":{count}"))
        .collect::<Vec<_>>()
        .join(",");
    let preflight_passed_by_k = portfolio
        .preflight_passed_by_k
        .iter()
        .map(|(ears, count)| format!("\"{ears}\":{count}"))
        .collect::<Vec<_>>()
        .join(",");
    let preflight_rejected_by_k = portfolio
        .preflight_rejected_by_k
        .iter()
        .map(|(ears, count)| format!("\"{ears}\":{count}"))
        .collect::<Vec<_>>()
        .join(",");
    Ok(format!(
        "{{\"schema_version\":1,\"taskbook_sha256\":\"387311a0c1b2ed52f43515766c9fa785e6849ad819aa05e9c4b78efb65492c24\",\"fixture\":\"N12-Lifted-N6\",\"declared_topology_family\":\"W2CanonicalEssentialCycle+TransitionCellV3+BalancedAnnularPairR2Preflight\",\"scope_conclusion\":\"BalancedSubsetEvidenceOnly\",\"limits\":{{\"cycle_unique_states\":{},\"balanced_topologies_per_cell\":{}}},\"unique_states\":{},\"essential_cycles\":{},\"cycles_audited\":{},\"domains_built\":{},\"cell_topologies\":{},\"global_pair_product\":{},\"direct_no_ear_candidates\":{},\"permanent_impossible\":{},\"outside_repair_depth_r2\":{},\"r2_anchor_necessary_candidates\":{},\"r2_preflight_passed\":{},\"r2_preflight_rejected\":{},\"unaffected_degree_rejects\":{},\"affected_total_capacity_rejects\":{},\"fixed_link_rejects\":{},\"k_tiers\":{{{}}},\"preflight_passed_by_k\":{{{}}},\"preflight_rejected_by_k\":{{{}}},\"permanent_reasons\":{{{}}},\"pair_accounting_complete\":{},\"r2_candidate_accounting_complete\":{},\"k_tier_accounting_complete\":{},\"cycle_accounting_complete\":{},\"errors\":{{{}}},\"audit_gate_passed\":{},\"go_no_go\":\"{}\",\"remaining_cec_shards\":{},\"cec_shards_resumed\":false,\"new_repair_solver_run\":false,\"search_result_changed\":false,\"geometry_attempted\":false,\"product_gate_changed\":false}}",
        limits.cycle_unique_states,
        limits.balanced_topologies_per_cell,
        evidence.unique_states,
        evidence.essential_cycles,
        evaluator.cycles_audited,
        evaluator.domains_built,
        evaluator.cell_topologies,
        portfolio.pair_total,
        portfolio.direct_no_ear_candidates,
        portfolio.permanent_impossible,
        portfolio.outside_r2,
        portfolio.r2_anchor_necessary_candidates,
        portfolio.r2_preflight_passed,
        preflight_rejected,
        portfolio.unaffected_degree_rejects,
        portfolio.total_capacity_rejects,
        portfolio.fixed_link_rejects,
        k_tiers,
        preflight_passed_by_k,
        preflight_rejected_by_k,
        json_string_count_map_usize(&portfolio.permanent_reasons),
        pair_accounting_complete,
        r2_candidate_accounting_complete,
        k_tier_accounting_complete,
        cycle_accounting_complete,
        json_string_count_map(&evaluator.errors),
        audit_gate_passed,
        go_no_go,
        evidence.checkpoint_shards_created,
    ))
}

struct FairPairAuditingEvaluator<'a> {
    source: &'a crate::MotherGrid,
    component: &'a super::HierarchyComponent,
    topology_limit: usize,
    cycles_audited: u64,
    domains_built: u64,
    cell_topologies: u64,
    pair_product: u64,
    zero_ear_pairs: u64,
    direct_zero_ear_closures: u64,
    repairable_pairs: u64,
    impossible_pairs: u64,
    low_ear_pairs: BTreeMap<u8, u64>,
    anchor_degree_histogram: BTreeMap<usize, u64>,
    impossible_reasons: BTreeMap<String, u64>,
    zero_ear_final_rejects: BTreeMap<String, u64>,
    first_pair_rank_min: usize,
    first_pair_rank_max: usize,
    first_pair_rank_sum: u64,
    first_pair_classes: BTreeMap<String, u64>,
    best_pair_classes: BTreeMap<String, u64>,
    first_pair_ordinary_defects: BTreeMap<usize, u64>,
    first_pair_unmatched_edges: BTreeMap<usize, u64>,
    first_pair_broken_links: BTreeMap<usize, u64>,
    first_ear_outcome: Option<String>,
    first_ear_telemetry: Option<super::AnchorEarSearchTelemetry>,
    anchor_repair_portfolio: AnchorRepairPortfolioEvidence,
    errors: BTreeMap<String, u64>,
}

impl<'a> FairPairAuditingEvaluator<'a> {
    fn new(
        source: &'a crate::MotherGrid,
        component: &'a super::HierarchyComponent,
        topology_limit: usize,
    ) -> Self {
        Self {
            source,
            component,
            topology_limit,
            cycles_audited: 0,
            domains_built: 0,
            cell_topologies: 0,
            pair_product: 0,
            zero_ear_pairs: 0,
            direct_zero_ear_closures: 0,
            repairable_pairs: 0,
            impossible_pairs: 0,
            low_ear_pairs: BTreeMap::new(),
            anchor_degree_histogram: BTreeMap::new(),
            impossible_reasons: BTreeMap::new(),
            zero_ear_final_rejects: BTreeMap::new(),
            first_pair_rank_min: usize::MAX,
            first_pair_rank_max: 0,
            first_pair_rank_sum: 0,
            first_pair_classes: BTreeMap::new(),
            best_pair_classes: BTreeMap::new(),
            first_pair_ordinary_defects: BTreeMap::new(),
            first_pair_unmatched_edges: BTreeMap::new(),
            first_pair_broken_links: BTreeMap::new(),
            first_ear_outcome: None,
            first_ear_telemetry: None,
            anchor_repair_portfolio: AnchorRepairPortfolioEvidence::default(),
            errors: BTreeMap::new(),
        }
    }
}

impl FaceBandPlanEvaluator for FairPairAuditingEvaluator<'_> {
    fn observe_cycle(&mut self, _: &EssentialCycleKey, plan: &FaceBandPlan) {
        let trace_ears = self.cycles_audited == 0;
        self.cycles_audited += 1;
        let domain = match build_stratified_transition_domain_v3(self.source, self.component, plan)
        {
            Ok(domain) => domain,
            Err(error) => {
                *self.errors.entry(format!("{error:?}")).or_default() += 1;
                return;
            }
        };
        self.domains_built += 1;
        let mut families = Vec::with_capacity(domain.cells.len());
        for cell in &domain.cells {
            let TransitionCellDomain::Annulus(cell) = cell else {
                *self
                    .errors
                    .entry("PairAuditDoesNotSupportDiskCell".into())
                    .or_default() += 1;
                return;
            };
            let search = match enumerate_balanced_annular_strips(
                &cell.lower_cycle,
                &cell.upper_cycle,
                &cell.forbidden_global_edges,
                self.topology_limit,
            ) {
                Ok(search) => search,
                Err(error) => {
                    *self.errors.entry(format!("{error:?}")).or_default() += 1;
                    return;
                }
            };
            self.cell_topologies += search.family.topologies.len() as u64;
            families.push(TransitionCellFamily::Annulus(AnnularTransitionCellFamily {
                cell_id: cell.cell_id,
                family: search.family,
            }));
        }
        let audit = match audit_transition_cell_pairs(
            self.source,
            self.component,
            &domain,
            &families,
            trace_ears,
        ) {
            Ok(audit) => audit,
            Err(error) => {
                *self.errors.entry(error).or_default() += 1;
                return;
            }
        };
        self.pair_product += audit.total_pair_product as u64;
        self.zero_ear_pairs += audit.zero_ear_pairs as u64;
        self.direct_zero_ear_closures += audit.direct_zero_ear_closures as u64;
        self.repairable_pairs += audit.repairable_pairs as u64;
        self.impossible_pairs += audit.impossible_pairs as u64;
        merge_u8_counts(&mut self.low_ear_pairs, &audit.low_ear_pairs);
        merge_usize_counts(
            &mut self.anchor_degree_histogram,
            &audit.anchor_degree_histogram,
        );
        merge_string_counts(&mut self.impossible_reasons, &audit.impossible_reasons);
        merge_string_counts(
            &mut self.zero_ear_final_rejects,
            &audit.zero_ear_final_rejects,
        );
        merge_anchor_repair_portfolio(
            &mut self.anchor_repair_portfolio,
            &audit.anchor_repair_portfolio,
        );
        self.first_pair_rank_min = self
            .first_pair_rank_min
            .min(audit.first_pair_rank_by_repair_score);
        self.first_pair_rank_max = self
            .first_pair_rank_max
            .max(audit.first_pair_rank_by_repair_score);
        self.first_pair_rank_sum += audit.first_pair_rank_by_repair_score as u64;
        if let Some(first) = audit.first_pair {
            *self
                .first_pair_classes
                .entry(pair_class_name(&first.pair_class).into())
                .or_default() += 1;
            *self
                .first_pair_ordinary_defects
                .entry(first.ordinary_degree_defect_lower_bound)
                .or_default() += 1;
            *self
                .first_pair_unmatched_edges
                .entry(first.unmatched_edge_count)
                .or_default() += 1;
            *self
                .first_pair_broken_links
                .entry(first.nonrepairable_link_count)
                .or_default() += 1;
        }
        if let Some(best) = audit.best_ranked_pair {
            *self
                .best_pair_classes
                .entry(pair_class_name(&best.pair_class).into())
                .or_default() += 1;
        }
        if self.first_ear_telemetry.is_none() {
            self.first_ear_outcome = audit.first_pair_ear_outcome;
            self.first_ear_telemetry = audit.first_pair_ear_telemetry;
        }
    }

    fn evaluate(&mut self, _: &FaceBandPlan) -> PlanEvaluation {
        PlanEvaluation::AuditOnly
    }
}

fn pair_class_name(class: &TopologyPairClass) -> &'static str {
    match class {
        TopologyPairClass::DirectlyClosedWithoutEar => "DirectlyClosedWithoutEar",
        TopologyPairClass::NoEarFinalGateCandidate => "NoEarFinalGateCandidate",
        TopologyPairClass::EarRepairCandidate { .. } => "EarRepairCandidate",
        TopologyPairClass::ImpossibleBeforeEar { .. } => "ImpossibleBeforeEar",
    }
}

fn merge_u8_counts(target: &mut BTreeMap<u8, u64>, source: &BTreeMap<u8, usize>) {
    for (&key, &count) in source {
        *target.entry(key).or_default() += count as u64;
    }
}

fn merge_usize_counts(target: &mut BTreeMap<usize, u64>, source: &BTreeMap<usize, usize>) {
    for (&key, &count) in source {
        *target.entry(key).or_default() += count as u64;
    }
}

fn merge_string_counts(target: &mut BTreeMap<String, u64>, source: &BTreeMap<String, usize>) {
    for (key, &count) in source {
        *target.entry(key.clone()).or_default() += count as u64;
    }
}

fn merge_anchor_repair_portfolio(
    target: &mut AnchorRepairPortfolioEvidence,
    source: &AnchorRepairPortfolioEvidence,
) {
    target.pair_total += source.pair_total;
    target.direct_no_ear_candidates += source.direct_no_ear_candidates;
    target.permanent_impossible += source.permanent_impossible;
    target.outside_r2 += source.outside_r2;
    target.r2_anchor_necessary_candidates += source.r2_anchor_necessary_candidates;
    target.r2_preflight_passed += source.r2_preflight_passed;
    target.unaffected_degree_rejects += source.unaffected_degree_rejects;
    target.total_capacity_rejects += source.total_capacity_rejects;
    target.fixed_link_rejects += source.fixed_link_rejects;
    for (&key, &count) in &source.k_tiers {
        *target.k_tiers.entry(key).or_default() += count;
    }
    for (&key, &count) in &source.preflight_passed_by_k {
        *target.preflight_passed_by_k.entry(key).or_default() += count;
    }
    for (&key, &count) in &source.preflight_rejected_by_k {
        *target.preflight_rejected_by_k.entry(key).or_default() += count;
    }
    for (key, &count) in &source.permanent_reasons {
        *target.permanent_reasons.entry(key.clone()).or_default() += count;
    }
}

fn anchor_ear_telemetry_json(telemetry: &super::AnchorEarSearchTelemetry) -> String {
    let anchor_degrees = telemetry
        .initial_anchor_degrees
        .iter()
        .map(|(anchor, degree)| format!("\"{anchor}\":{degree}"))
        .collect::<Vec<_>>()
        .join(",");
    let candidates = telemetry
        .initial_candidates_by_anchor
        .iter()
        .map(|(anchor, count)| format!("\"{anchor}\":{count}"))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"states_examined\":{},\"maximum_depth\":{},\"duplicate_seen_states\":{},\"nodes_by_depth\":{{{}}},\"candidates_by_depth\":{{{}}},\"initial_anchor_degrees\":{{{}}},\"initial_candidates_by_anchor\":{{{}}},\"interacting_anchor_pairs\":{},\"apply_rejects\":{{{}}},\"final_gate_rejects\":{{{}}}}}",
        telemetry.states_examined,
        telemetry.maximum_depth,
        telemetry.duplicate_seen_states,
        json_usize_count_map(&telemetry.nodes_by_depth),
        json_usize_count_map(&telemetry.candidates_by_depth),
        anchor_degrees,
        candidates,
        telemetry.interacting_anchor_pairs.len(),
        json_string_count_map_usize(&telemetry.apply_rejects),
        json_string_count_map_usize(&telemetry.final_gate_rejects),
    )
}

struct V3BalancedFindOneEvaluator<'a> {
    source: &'a crate::MotherGrid,
    component: &'a super::HierarchyComponent,
    topology_limit: usize,
    global_limit: usize,
    ear_limit: usize,
    current_cycle: Option<EssentialCycleKey>,
    domains_built: u64,
    candidates_examined: usize,
    topologies_generated: usize,
    exhaustive_cell_subsets: u64,
    partial_cell_subsets: u64,
    global_states: usize,
    ear_states: usize,
    closed_cycle: Option<EssentialCycleKey>,
    closed: Option<super::TransitionCellMergeEvidence>,
    errors: BTreeMap<String, u64>,
}

impl FaceBandPlanEvaluator for V3BalancedFindOneEvaluator<'_> {
    fn observe_cycle(&mut self, cycle: &EssentialCycleKey, _: &FaceBandPlan) {
        self.current_cycle = Some(cycle.clone());
    }

    fn evaluate(&mut self, plan: &FaceBandPlan) -> PlanEvaluation {
        let domain = match build_stratified_transition_domain_v3(self.source, self.component, plan)
        {
            Ok(domain) => domain,
            Err(error) => {
                return self.invalid(DownstreamRejectStage::BandDomain, format!("{error:?}"))
            }
        };
        self.domains_built += 1;
        let mut families = Vec::with_capacity(domain.cells.len());
        for cell in &domain.cells {
            let TransitionCellDomain::Annulus(cell) = cell else {
                return PlanEvaluation::RejectedV3SearchIncomplete {
                    states_examined: 0,
                    stage: DownstreamRejectStage::AnnularConcreteEnumeration,
                    reason: "BalancedAnnularSubsetDoesNotEnumerateDiskCells".into(),
                };
            };
            let search = match enumerate_balanced_annular_strips(
                &cell.lower_cycle,
                &cell.upper_cycle,
                &cell.forbidden_global_edges,
                self.topology_limit,
            ) {
                Ok(search) => search,
                Err(AnnularEnumerationError::EmptyFamily) => {
                    return PlanEvaluation::RejectedV3SearchIncomplete {
                        states_examined: 0,
                        stage: DownstreamRejectStage::AnnularConcreteEnumeration,
                        reason: "BalancedAnnularSubsetEmpty".into(),
                    }
                }
                Err(error) => {
                    return self.invalid(
                        DownstreamRejectStage::AnnularConcreteEnumeration,
                        format!("{error:?}"),
                    )
                }
            };
            self.candidates_examined += search.candidates_examined;
            self.topologies_generated += search.family.topologies.len();
            if search.subset_exhausted {
                self.exhaustive_cell_subsets += 1;
            } else {
                self.partial_cell_subsets += 1;
            }
            families.push(TransitionCellFamily::Annulus(AnnularTransitionCellFamily {
                cell_id: cell.cell_id,
                family: search.family,
            }));
        }
        match solve_transition_cell_find_one(
            self.source,
            self.component,
            &domain,
            &families,
            TransitionCellMergeLimits {
                topology_states: self.global_limit,
                ear_states_per_topology: self.ear_limit,
            },
        ) {
            TransitionCellMergeOutcome::Closed(trial) => {
                self.global_states += trial.evidence.states_examined;
                self.ear_states += trial.evidence.ear_states_examined;
                self.closed_cycle = self.current_cycle.clone();
                self.closed = Some(trial.evidence.clone());
                PlanEvaluation::Accepted(Box::new(adapt_transition_trial(*trial)))
            }
            TransitionCellMergeOutcome::TopologyFamilyExhaustedNoSolution(evidence) => {
                self.global_states += evidence.states_examined;
                self.ear_states += evidence.ear_states_examined;
                PlanEvaluation::RejectedV3SearchIncomplete {
                    states_examined: evidence.states_examined,
                    stage: DownstreamRejectStage::GlobalLinkMerge,
                    reason: "BalancedAnnularSubsetExhaustedWithoutClosure".into(),
                }
            }
            TransitionCellMergeOutcome::SearchIncomplete(evidence) => {
                self.global_states += evidence.states_examined;
                self.ear_states += evidence.ear_states_examined;
                PlanEvaluation::RejectedV3SearchIncomplete {
                    states_examined: evidence.states_examined,
                    stage: DownstreamRejectStage::GlobalLinkMerge,
                    reason: "BalancedAnnularGlobalMergeBudgetExhausted".into(),
                }
            }
            TransitionCellMergeOutcome::InvalidInput { reason, evidence } => {
                self.global_states += evidence.states_examined;
                self.ear_states += evidence.ear_states_examined;
                self.invalid(DownstreamRejectStage::GlobalLinkMerge, reason)
            }
        }
    }

    fn topology_state_budget(&self) -> Option<usize> {
        Some(self.global_limit)
    }
}

impl V3BalancedFindOneEvaluator<'_> {
    fn invalid(&mut self, stage: DownstreamRejectStage, reason: String) -> PlanEvaluation {
        *self.errors.entry(reason.clone()).or_default() += 1;
        PlanEvaluation::RejectedV3Invalid {
            states_examined: 0,
            stage,
            reason,
        }
    }
}

fn adapt_transition_trial(trial: TransitionCellMergeTrial) -> FullPolygonMergeTrial {
    let TransitionCellMergeTrial {
        global_trial,
        evidence,
    } = trial;
    let global = global_trial.evidence.clone();
    FullPolygonMergeTrial {
        global_trial,
        evidence: FullPolygonMergeEvidence {
            family_id: TopologyFamilyId::TransitionCellAnnulus,
            sector_family_counts: evidence.cell_family_counts.clone(),
            retained_topology_counts: evidence.cell_family_counts,
            reachability: None,
            states_examined: evidence.states_examined,
            states_by_depth: Vec::new(),
            ear_states_examined: evidence.ear_states_examined,
            topology_candidates_closed: evidence.topology_candidates_closed,
            ear_degree_feasible_candidates: usize::from(evidence.topology_candidates_closed > 0),
            geometry_candidates_attempted: 0,
            last_geometry_failure: None,
            best_geometry_failure: None,
            geometry_failure_phase_counts: BTreeMap::new(),
            selected_topology_keys: Vec::new(),
            selected_ears: global.selected_ears.clone(),
            best_global_evidence: global,
        },
    }
}

struct V3PrefixReplayEvaluator<'a> {
    source: &'a crate::MotherGrid,
    component: &'a super::HierarchyComponent,
    signature_states: usize,
    domains_built: u64,
    reached_annular_reachability: u64,
    necessary_feasible: u64,
    exact_rejects: u64,
    search_incomplete: u64,
    root_bridges_considered: u64,
    reachability_states: usize,
    degree_cap_prunes: usize,
    ac3_prunes: usize,
    concrete_enumeration_deferred: u64,
    errors: BTreeMap<String, u64>,
}

impl FaceBandPlanEvaluator for V3PrefixReplayEvaluator<'_> {
    fn evaluate(&mut self, plan: &FaceBandPlan) -> PlanEvaluation {
        let domain = match build_stratified_transition_domain_v3(self.source, self.component, plan)
        {
            Ok(domain) => domain,
            Err(error) => {
                let reason = format!("{error:?}");
                *self.errors.entry(reason.clone()).or_default() += 1;
                return PlanEvaluation::RejectedV3Invalid {
                    states_examined: 0,
                    stage: DownstreamRejectStage::BandDomain,
                    reason,
                };
            }
        };
        self.domains_built += 1;
        self.reached_annular_reachability += 1;
        let reachability = match analyze_stratified_annular_degree_reachability(
            &domain,
            AnnularReachabilityLimits {
                maximum_signature_states: self.signature_states,
            },
        ) {
            Ok(evidence) => evidence,
            Err(error) => {
                let reason = format!("{error:?}");
                *self.errors.entry(reason.clone()).or_default() += 1;
                return PlanEvaluation::RejectedV3Invalid {
                    states_examined: 0,
                    stage: DownstreamRejectStage::AnnularReachability,
                    reason,
                };
            }
        };
        self.root_bridges_considered += reachability.root_bridges_considered;
        self.reachability_states += reachability.states_examined;
        self.degree_cap_prunes += reachability.degree_cap_prunes;
        self.ac3_prunes += reachability.ac3_prunes;
        match reachability.outcome {
            AnnularReachabilityOutcome::NecessaryFeasible => {
                self.necessary_feasible += 1;
                self.concrete_enumeration_deferred += 1;
                PlanEvaluation::RejectedV3SearchIncomplete {
                    states_examined: reachability.states_examined,
                    stage: DownstreamRejectStage::AnnularConcreteEnumeration,
                    reason: "AnnularConcreteFamilyNotMaterialized".into(),
                }
            }
            AnnularReachabilityOutcome::ProvenImpossibleWithinDeclaredAnnularFamily => {
                self.exact_rejects += 1;
                PlanEvaluation::RejectedV3Exact {
                    states_examined: reachability.states_examined,
                    stage: DownstreamRejectStage::AnnularReachability,
                    reason: "AnnularDegreeLinkSupportExhausted".into(),
                }
            }
            AnnularReachabilityOutcome::SearchIncomplete => {
                self.search_incomplete += 1;
                PlanEvaluation::RejectedV3SearchIncomplete {
                    states_examined: reachability.states_examined,
                    stage: DownstreamRejectStage::AnnularReachability,
                    reason: "AnnularSignatureSearchIncomplete".into(),
                }
            }
        }
    }
}

struct V3DomainAuditingEvaluator<'a> {
    source: &'a crate::MotherGrid,
    component: &'a super::HierarchyComponent,
    cycles_observed: u64,
    domains_built: u64,
    annular_cells: u64,
    disk_cells: u64,
    first_link_contracts: Option<usize>,
    errors: BTreeMap<String, u64>,
}

impl FaceBandPlanEvaluator for V3DomainAuditingEvaluator<'_> {
    fn observe_cycle(&mut self, _: &EssentialCycleKey, plan: &FaceBandPlan) {
        self.cycles_observed += 1;
        match build_stratified_transition_domain_v3(self.source, self.component, plan) {
            Ok(domain) => {
                self.domains_built += 1;
                self.first_link_contracts
                    .get_or_insert(domain.link_contracts.len());
                for cell in domain.cells {
                    match cell {
                        TransitionCellDomain::Annulus(_) => self.annular_cells += 1,
                        TransitionCellDomain::Disk(_) => self.disk_cells += 1,
                    }
                }
            }
            Err(error) => *self.errors.entry(format!("{error:?}")).or_default() += 1,
        }
    }

    fn evaluate(&mut self, _: &FaceBandPlan) -> PlanEvaluation {
        PlanEvaluation::AuditOnly
    }
}

#[derive(Debug, Default)]
struct DefectVertexRoleAudit {
    cycles_present: u64,
    fixed_degrees: BTreeMap<u8, u64>,
    owner_counts: BTreeMap<usize, u64>,
    tuple_count_min: Option<usize>,
    tuple_count_max: usize,
}

struct SdceContractAuditingEvaluator<'a> {
    source: &'a crate::MotherGrid,
    component: &'a super::HierarchyComponent,
    cycles_observed: u64,
    domains_built: u64,
    contracts_built: u64,
    contract_vertices: u64,
    contract_owner_tuples: u64,
    empty_vertex_domains: u64,
    invalid_cell_sums: u64,
    adapter_mismatches: u64,
    transition_charges: BTreeMap<i16, u64>,
    defect_roles: BTreeMap<usize, DefectVertexRoleAudit>,
    errors: BTreeMap<String, u64>,
}

impl<'a> SdceContractAuditingEvaluator<'a> {
    fn new(source: &'a crate::MotherGrid, component: &'a super::HierarchyComponent) -> Self {
        Self {
            source,
            component,
            cycles_observed: 0,
            domains_built: 0,
            contracts_built: 0,
            contract_vertices: 0,
            contract_owner_tuples: 0,
            empty_vertex_domains: 0,
            invalid_cell_sums: 0,
            adapter_mismatches: 0,
            transition_charges: BTreeMap::new(),
            defect_roles: [48, 52, 78, 252, 256, 343]
                .into_iter()
                .map(|slot| (slot, DefectVertexRoleAudit::default()))
                .collect(),
            errors: BTreeMap::new(),
        }
    }
}

impl FaceBandPlanEvaluator for SdceContractAuditingEvaluator<'_> {
    fn observe_cycle(&mut self, _: &EssentialCycleKey, plan: &FaceBandPlan) {
        self.cycles_observed += 1;
        let domain = match build_stratified_transition_domain_v3(self.source, self.component, plan)
        {
            Ok(domain) => {
                self.domains_built += 1;
                domain
            }
            Err(error) => {
                *self.errors.entry(format!("{error:?}")).or_default() += 1;
                return;
            }
        };
        let contract = match build_global_incidence_contract(self.source, self.component, &domain) {
            Ok(contract) => contract,
            Err(error) => {
                if matches!(error, GlobalIncidenceContractError::AdapterMismatch { .. }) {
                    self.adapter_mismatches += 1;
                }
                *self.errors.entry(format!("{error:?}")).or_default() += 1;
                return;
            }
        };
        self.contracts_built += 1;
        self.contract_vertices += contract.vertex_domains.len() as u64;
        self.contract_owner_tuples += contract
            .vertex_domains
            .values()
            .map(|domain| domain.allowed_owner_tuples.len() as u64)
            .sum::<u64>();
        self.empty_vertex_domains += contract
            .vertex_domains
            .values()
            .filter(|domain| domain.allowed_owner_tuples.is_empty())
            .count() as u64;
        self.invalid_cell_sums += contract
            .cell_ids
            .iter()
            .filter(|cell| {
                contract.cell_incidence_sums[*cell] != 3 * contract.cell_triangle_counts[*cell]
            })
            .count() as u64;
        *self
            .transition_charges
            .entry(contract.target_transition_charge)
            .or_default() += 1;
        for (&slot, audit) in &mut self.defect_roles {
            let Some(vertex) = contract.vertex_domains.get(&slot) else {
                continue;
            };
            audit.cycles_present += 1;
            *audit.fixed_degrees.entry(vertex.fixed_degree).or_default() += 1;
            *audit.owner_counts.entry(vertex.owners.len()).or_default() += 1;
            let tuples = vertex.allowed_owner_tuples.len();
            audit.tuple_count_min =
                Some(audit.tuple_count_min.map_or(tuples, |old| old.min(tuples)));
            audit.tuple_count_max = audit.tuple_count_max.max(tuples);
        }
    }

    fn evaluate(&mut self, _: &FaceBandPlan) -> PlanEvaluation {
        PlanEvaluation::AuditOnly
    }
}

struct PlanBandDomainAuditingEvaluator<'a> {
    source: &'a crate::MotherGrid,
    component: &'a super::HierarchyComponent,
    cycles_observed: u64,
    plans_built: u64,
    bands_built: u64,
    annular_bands: u64,
    contracted_band0: u64,
    first_boundary_sizes: Option<(usize, usize, usize, usize)>,
    errors: BTreeMap<String, u64>,
}

impl FaceBandPlanEvaluator for PlanBandDomainAuditingEvaluator<'_> {
    fn observe_cycle(&mut self, _: &EssentialCycleKey, plan: &FaceBandPlan) {
        self.cycles_observed += 1;
        match build_plan_band_domains(self.source, self.component, plan) {
            Ok(bands) => {
                self.plans_built += 1;
                self.bands_built += bands.len() as u64;
                self.annular_bands += bands
                    .iter()
                    .filter(|band| band.topology_kind == PlanBandTopologyKind::Annulus)
                    .count() as u64;
                if let Some(band0) = bands.first() {
                    if matches!(
                        band0.lower_boundary,
                        TopologyBoundary::ContractedCoarseCycle { .. }
                    ) {
                        self.contracted_band0 += 1;
                    }
                }
                if self.first_boundary_sizes.is_none() && bands.len() == 2 {
                    self.first_boundary_sizes = Some((
                        bands[0].lower_boundary.topology_vertices().len(),
                        bands[0].lower_boundary.source_edges().len(),
                        bands[0].upper_boundary.source_edges().len(),
                        bands[1].upper_boundary.source_edges().len(),
                    ));
                }
            }
            Err(error) => *self.errors.entry(format!("{error:?}")).or_default() += 1,
        }
    }

    fn evaluate(&mut self, _: &FaceBandPlan) -> PlanEvaluation {
        PlanEvaluation::AuditOnly
    }
}

struct BandAuditingEvaluator<'a> {
    source: &'a crate::MotherGrid,
    component: &'a super::HierarchyComponent,
    inner: FullPolygonPlanEvaluator<'a>,
    summary: BandBoundaryAuditSummary,
    first_error: Option<String>,
}

impl FaceBandPlanEvaluator for BandAuditingEvaluator<'_> {
    fn observe_cycle(&mut self, cycle: &EssentialCycleKey, plan: &FaceBandPlan) {
        match audit_face_band_boundaries(self.source, self.component, plan) {
            Ok(audits) => self.summary.record(cycle, audits),
            Err(error) => {
                self.first_error.get_or_insert(error);
            }
        };
    }

    fn evaluate(&mut self, plan: &FaceBandPlan) -> PlanEvaluation {
        self.inner.evaluate(plan)
    }

    fn topology_state_budget(&self) -> Option<usize> {
        self.inner.topology_state_budget()
    }
}

fn band_boundary_audit_json(audit: &BandBoundaryAudit) -> String {
    let degree_histogram = audit
        .undirected_boundary_degree_histogram
        .iter()
        .map(|(degree, count)| format!("\"{degree}\":{count}"))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"band_id\":{},\"face_count\":{},\"vertices\":{},\"edges\":{},\"euler\":{},\"undirected_boundary_edges\":{},\"undirected_boundary_cycle_count\":{},\"undirected_boundary_degree_histogram\":{{{}}},\"directed_outdegree_violations\":{},\"directed_indegree_violations\":{},\"lower_trace_edges\":{},\"upper_trace_edges\":{},\"lower_boundary_match\":{},\"upper_boundary_match\":{},\"lower_vertices_with_direct_upper_connector\":{},\"lower_vertices_without_direct_upper_connector\":{},\"failure\":{}}}",
        audit.band_id,
        audit.face_count,
        audit.vertices,
        audit.edges,
        audit.euler,
        audit.undirected_boundary_edges,
        audit.undirected_boundary_cycle_count,
        degree_histogram,
        json_usize_array(&audit.directed_outdegree_violations),
        json_usize_array(&audit.directed_indegree_violations),
        audit.lower_trace_edges,
        audit.upper_trace_edges,
        audit.lower_boundary_match,
        audit.upper_boundary_match,
        audit.lower_vertices_with_direct_upper_connector,
        json_usize_array(&audit.lower_vertices_without_direct_upper_connector),
        audit
            .failure
            .as_ref()
            .map_or_else(|| "null".into(), |failure| json_string(&format!("{failure:?}"))),
    )
}

pub fn n12_lifted_downstream_reject_audit_json(
    limits: ResearchCecTopologyLimits,
) -> Result<String, String> {
    let fixture = n12_lifted_n6_fixture()?;
    let preflight = audit_legacy_downstream_preflight(&fixture.source, &fixture.component);
    let report = run_n12_cec_topology_probe(&fixture, limits)?;
    let preflight_json = match preflight {
        DownstreamPreflightOutcome::Ready(evidence) => format!(
            "{{\"outcome\":\"Ready\",\"stage\":null,\"plan_independent\":{},\"geometry_guard_deferred\":{},\"reason\":null}}",
            evidence.plan_independent, evidence.geometry_guard_deferred,
        ),
        DownstreamPreflightOutcome::ContractBlocked { stage, evidence } => format!(
            "{{\"outcome\":\"ContractBlocked\",\"stage\":\"{}\",\"plan_independent\":{},\"geometry_guard_deferred\":{},\"reason\":{}}}",
            stage.as_str(),
            evidence.plan_independent,
            evidence.geometry_guard_deferred,
            evidence.reason.as_deref().map_or_else(|| "null".into(), json_string),
        ),
    };
    let histogram = &report.cec.downstream_reject_histogram;
    let (by_stage, by_reason, first_cycles) = reject_histogram_fragments(histogram);
    Ok(format!(
        "{{\"schema_version\":1,\"taskbook_sha256\":\"63215a9043f5aa87092a78b2910d0c779da3c10e2c749bb4e11d5b0e5b207c5d\",\"fixture\":\"N12-Lifted-N6\",\"adapter_version\":1,\"declared_topology_family\":\"W2CanonicalEssentialCycle+FullPolygon\",\"limits\":{{\"cycle_unique_states\":{},\"downstream_topology_states\":{}}},\"preflight\":{},\"essential_cycles\":{},\"downstream_states\":{},\"downstream_invalid\":{},\"reject_histogram\":{{\"by_stage\":{{{}}},\"by_reason\":{{{}}},\"first_cycle_by_reason\":{{{}}}}},\"decision\":\"{}\",\"geometry_attempted\":false,\"product_grid_written\":false,\"ready_marker_written\":false,\"product_gate_changed\":false}}",
        limits.cycle_unique_states,
        limits.downstream_topology_states,
        preflight_json,
        report.cec.essential_cycles,
        report.downstream_states,
        report.cec.downstream_invalid,
        by_stage,
        by_reason,
        first_cycles,
        ValidationGovernanceDecisionV2::DownstreamAnnulusContractBlocked.as_str(),
    ))
}

fn reject_histogram_fragments(
    histogram: &super::DownstreamRejectHistogram,
) -> (String, String, String) {
    let by_stage = histogram
        .by_stage
        .iter()
        .map(|(stage, count)| format!("\"{}\":{count}", stage.as_str()))
        .collect::<Vec<_>>()
        .join(",");
    let by_reason = histogram
        .by_reason
        .iter()
        .map(|(reason, count)| format!("{}:{count}", json_string(reason)))
        .collect::<Vec<_>>()
        .join(",");
    let first_cycles = histogram
        .first_cycle_by_reason
        .iter()
        .map(|(reason, cycle)| {
            format!(
                "{}:{{\"ordered_vertices\":{},\"key\":{}}}",
                json_string(reason),
                cycle.ordered_vertices.len(),
                json_string(&format!("{cycle:?}")),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    (by_stage, by_reason, first_cycles)
}

pub fn run_n12_legacy_baseline(
    fixture: &CertifiedResearchFixture,
    limits: ResearchLegacyLimits,
) -> ResearchLegacyEvidence {
    let problem = match build_face_band_problem(&fixture.source, &fixture.component, 2) {
        Ok(problem) => problem,
        Err(reason) => return incomplete(fixture, limits, reason, None),
    };
    match solve_exact_face_bands(
        &problem,
        FaceBandLimits {
            maximum_states: limits.face_band_states,
        },
    ) {
        FaceBandSolveOutcome::Closed(plan, face_band) => {
            match solve_full_polygon_merge_from_face_bands(
                &fixture.source,
                &fixture.component,
                &plan,
                FullPolygonMergeLimits {
                    topology_states: limits.downstream_topology_states,
                },
            ) {
                FullPolygonMergeOutcome::Closed(trial) => {
                    let states = trial.evidence.states_examined;
                    let closed = trial.evidence.topology_candidates_closed;
                    match Certificate::internal().verify_geometry(&trial.global_trial.mesh.mesh) {
                        Ok(_) => evidence(
                            fixture,
                            limits,
                            Some(face_band),
                            states,
                            closed,
                            true,
                            "current closed topology satisfies the internal geometry certificate",
                            ResearchN12OutcomeKind::ResearchStrictPass,
                        ),
                        Err(error) => evidence(
                            fixture,
                            limits,
                            Some(face_band),
                            states,
                            closed,
                            true,
                            format!("closed topology has no strict current-coordinate witness: {error}"),
                            ResearchN12OutcomeKind::ResearchContinuousIncomplete,
                        ),
                    }
                }
                FullPolygonMergeOutcome::TopologyFamilyExhaustedNoSolution(downstream) => evidence(
                    fixture,
                    limits,
                    Some(face_band),
                    downstream.states_examined,
                    downstream.topology_candidates_closed,
                    false,
                    "legacy face band closed but the declared downstream topology family is exhausted",
                    ResearchN12OutcomeKind::ResearchExactNoSolution,
                ),
                FullPolygonMergeOutcome::SearchBudgetExhausted(downstream) => evidence(
                    fixture,
                    limits,
                    Some(face_band),
                    downstream.states_examined,
                    downstream.topology_candidates_closed,
                    false,
                    "legacy downstream topology budget exhausted",
                    ResearchN12OutcomeKind::ResearchSearchIncomplete,
                ),
                FullPolygonMergeOutcome::InvalidInput { reason, evidence: downstream } => evidence(
                    fixture,
                    limits,
                    Some(face_band),
                    downstream.states_examined,
                    downstream.topology_candidates_closed,
                    false,
                    format!("legacy downstream evaluator rejected the fixture: {reason}"),
                    ResearchN12OutcomeKind::ResearchSearchIncomplete,
                ),
            }
        }
        FaceBandSolveOutcome::FamilyExhaustedNoSolution { evidence: face_band, .. } => evidence(
            fixture,
            limits,
            Some(face_band),
            0,
            0,
            false,
            "legacy W2 face-label family exhausted",
            ResearchN12OutcomeKind::ResearchExactNoSolution,
        ),
        FaceBandSolveOutcome::SearchBudgetExhausted { evidence: face_band, .. } => evidence(
            fixture,
            limits,
            Some(face_band),
            0,
            0,
            false,
            "legacy W2 face-label budget exhausted",
            ResearchN12OutcomeKind::ResearchSearchIncomplete,
        ),
        FaceBandSolveOutcome::InvalidInput { reason } => incomplete(fixture, limits, reason, None),
    }
}

pub fn n12_legacy_baseline_json(limits: ResearchLegacyLimits) -> Result<String, String> {
    let lifted = n12_lifted_n6_fixture()?;
    let interior = n12_interior_control_fixture()?;
    let reports = [
        run_n12_legacy_baseline(&lifted, limits),
        run_n12_legacy_baseline(&interior, limits),
    ];
    Ok(format!(
        "{{\"schema_version\":1,\"runner\":\"Alpha5LegacyW2\",\"research_only\":true,\"reports\":[{}]}}",
        reports
            .iter()
            .map(research_legacy_evidence_json)
            .collect::<Vec<_>>()
            .join(",")
    ))
}

pub fn research_legacy_evidence_json(report: &ResearchLegacyEvidence) -> String {
    format!(
        "{{\"fixture\":{},\"limits\":{{\"face_band_states\":{},\"downstream_topology_states\":{}}},\"face_band\":{},\"downstream_states\":{},\"topology_candidates_closed\":{},\"geometry_attempted\":{},\"reason\":{},\"outcome\":\"{}\",\"product_grid_written\":{},\"ready_marker_written\":{},\"product_gate_changed\":{}}}",
        json_string(&report.fixture_name),
        report.limits.face_band_states,
        report.limits.downstream_topology_states,
        report.face_band.as_ref().map_or_else(|| "null".into(), face_band_evidence_json),
        report.downstream_states,
        report.topology_candidates_closed,
        report.geometry_attempted,
        json_string(&report.reason),
        report.outcome.as_str(),
        report.product_grid_written,
        report.ready_marker_written,
        report.product_gate_changed,
    )
}

fn incomplete(
    fixture: &CertifiedResearchFixture,
    limits: ResearchLegacyLimits,
    reason: String,
    face_band: Option<FaceBandEvidence>,
) -> ResearchLegacyEvidence {
    evidence(
        fixture,
        limits,
        face_band,
        0,
        0,
        false,
        reason,
        ResearchN12OutcomeKind::ResearchSearchIncomplete,
    )
}

#[allow(clippy::too_many_arguments)]
fn evidence(
    fixture: &CertifiedResearchFixture,
    limits: ResearchLegacyLimits,
    face_band: Option<FaceBandEvidence>,
    downstream_states: usize,
    topology_candidates_closed: usize,
    geometry_attempted: bool,
    reason: impl Into<String>,
    outcome: ResearchN12OutcomeKind,
) -> ResearchLegacyEvidence {
    ResearchLegacyEvidence {
        fixture_name: fixture.manifest.name.clone(),
        limits,
        face_band,
        downstream_states,
        topology_candidates_closed,
        geometry_attempted,
        reason: reason.into(),
        outcome,
        product_grid_written: false,
        ready_marker_written: false,
        product_gate_changed: false,
    }
}

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            character if character.is_control() => {
                use std::fmt::Write as _;
                write!(out, "\\u{:04x}", character as u32).expect("writing to String cannot fail");
            }
            character => out.push(character),
        }
    }
    out.push('"');
    out
}

fn json_option(value: Option<impl ToString>) -> String {
    value.map_or_else(|| "null".into(), |value| value.to_string())
}

fn json_usize_array(values: &[usize]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn json_usize_map(values: &BTreeMap<usize, usize>) -> String {
    format!(
        "{{{}}}",
        values
            .iter()
            .map(|(key, value)| format!("\"{key}\":{value}"))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn json_usize_count_map(values: &BTreeMap<usize, impl ToString>) -> String {
    values
        .iter()
        .map(|(key, value)| format!("\"{key}\":{}", value.to_string()))
        .collect::<Vec<_>>()
        .join(",")
}

fn json_string_count_map(values: &BTreeMap<String, u64>) -> String {
    values
        .iter()
        .map(|(key, value)| format!("{}:{value}", json_string(key)))
        .collect::<Vec<_>>()
        .join(",")
}

fn json_string_count_map_usize(values: &BTreeMap<String, usize>) -> String {
    values
        .iter()
        .map(|(key, value)| format!("{}:{value}", json_string(key)))
        .collect::<Vec<_>>()
        .join(",")
}
