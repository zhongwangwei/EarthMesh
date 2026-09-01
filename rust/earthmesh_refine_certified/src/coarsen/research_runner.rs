//! Research-only runners. They never produce a product mesh or gate decision.

use super::{
    audit_face_band_boundaries, audit_legacy_downstream_preflight, build_essential_cycle_problem,
    build_face_band_problem, build_plan_band_domains, face_band_evidence_json,
    n12_interior_control_fixture, n12_lifted_n6_fixture, prove_essential_cycle_family,
    solve_exact_face_bands, solve_full_polygon_merge_from_face_bands, BandBoundaryAudit,
    BandBoundaryAuditSummary, CertifiedResearchFixture, DownstreamEvaluationCache,
    DownstreamPreflightOutcome, EssentialCycleFindOneEvidence, EssentialCycleFindOneLimits,
    EssentialCycleKey, ExactFaceBandV2Outcome, FaceBandAdapterVersion, FaceBandEvidence,
    FaceBandLimits, FaceBandPlan, FaceBandPlanEvaluator, FaceBandSolveOutcome,
    FullPolygonMergeLimits, FullPolygonMergeOutcome, FullPolygonPlanEvaluator,
    PlanBandTopologyKind, PlanEvaluation, RetainedCoreCorridorFamily, TopologyBoundary,
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
