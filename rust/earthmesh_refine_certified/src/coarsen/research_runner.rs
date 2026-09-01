//! Research-only runners. They never produce a product mesh or gate decision.

use super::{
    build_face_band_problem, face_band_evidence_json, n12_interior_control_fixture,
    n12_lifted_n6_fixture, solve_exact_face_bands, solve_full_polygon_merge_from_face_bands,
    CertifiedResearchFixture, FaceBandEvidence, FaceBandLimits, FaceBandSolveOutcome,
    FullPolygonMergeLimits, FullPolygonMergeOutcome,
};
use crate::certificate::Certificate;

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
