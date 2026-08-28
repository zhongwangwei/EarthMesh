//! Typed, serialization-free live trace records for HARP-DV.

use crate::certifier::{AngleViolation, MeshCertification};
use crate::state::SiteId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum HarpTraceStage {
    Input,
    PostRefinement,
    PostInitialLowDegree,
    PostEta,
    PostWindow,
    PostFinalLowDegree,
    Final,
}

impl HarpTraceStage {
    pub const ALL: [Self; 7] = [
        Self::Input,
        Self::PostRefinement,
        Self::PostInitialLowDegree,
        Self::PostEta,
        Self::PostWindow,
        Self::PostFinalLowDegree,
        Self::Final,
    ];

    pub const fn index(self) -> u8 {
        match self {
            Self::Input => 0,
            Self::PostRefinement => 1,
            Self::PostInitialLowDegree => 2,
            Self::PostEta => 3,
            Self::PostWindow => 4,
            Self::PostFinalLowDegree => 5,
            Self::Final => 6,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::PostRefinement => "post_refinement",
            Self::PostInitialLowDegree => "post_initial_low_degree",
            Self::PostEta => "post_eta",
            Self::PostWindow => "post_window",
            Self::PostFinalLowDegree => "post_final_low_degree",
            Self::Final => "final",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum HarpTraceEvent {
    StageSummary {
        stage: HarpTraceStage,
        certification: MeshCertification,
    },
    AngleViolation {
        stage: HarpTraceStage,
        violation: AngleViolation,
    },
    PhaseSkipped {
        stage: HarpTraceStage,
        reason: &'static str,
    },
    DegreeFourRetirementSummary(DegreeFourRetirementSummary),
    DegreeFourRetirementSite(DegreeFourRetirementSite),
    DegreeFourRetirementTrial(DegreeFourRetirementTrial),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DegreeFourCheckStatus {
    Pass,
    Fail,
    NotEvaluated,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DegreeFourRetirementSummary {
    pub evaluated: bool,
    pub sites_total: usize,
    pub sites_not_leaf: usize,
    pub sites_eligible: usize,
    pub sites_without_window_violation: usize,
    pub sites_audited: usize,
    pub sites_ranked_beyond_64: usize,
    pub sites_with_any_valid_trial: usize,
    pub sites_with_any_fully_acceptable_trial: usize,
    pub sites_committed: usize,
    pub trials_total: usize,
    pub trials_geometry_pass: usize,
    pub trials_hard_gate_pass: usize,
    pub trials_physical_pass: usize,
    pub trials_scale_balance_pass: usize,
    pub trials_no_new_low_degree_pass: usize,
    pub trials_angle_count_pass: usize,
    pub trials_worst_deviation_pass: usize,
    pub trials_penalty_pass: usize,
    pub trials_eta_pass: usize,
    pub trials_margin_pass: usize,
    pub trials_conservative_remap_pass: usize,
    pub trials_fully_acceptable: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DegreeFourRetirementSite {
    pub site_id: SiteId,
    pub vertex: usize,
    pub ranked_beyond_64: bool,
    pub trial_count: usize,
    pub any_valid_trial: bool,
    pub any_fully_acceptable_trial: bool,
    pub committed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DegreeFourRetirementTrial {
    pub site_id: SiteId,
    pub vertex: usize,
    pub trial_index: u8,
    pub ring_site_ids: [SiteId; 4],
    pub diagonal_site_ids: [SiteId; 2],
    pub geometry: DegreeFourCheckStatus,
    pub hard_gate: DegreeFourCheckStatus,
    pub physical_demand: DegreeFourCheckStatus,
    pub scale_balance: DegreeFourCheckStatus,
    pub no_new_low_degree: DegreeFourCheckStatus,
    pub angle_count: DegreeFourCheckStatus,
    pub worst_deviation: DegreeFourCheckStatus,
    pub penalty: DegreeFourCheckStatus,
    pub eta: DegreeFourCheckStatus,
    pub margin: DegreeFourCheckStatus,
    pub conservative_remap: DegreeFourCheckStatus,
    pub fully_acceptable: bool,
}

// Back-compatible short aliases for downstream serializers.
pub type AuditCheckStatus = DegreeFourCheckStatus;
pub type DegreeFourRetirementAuditSummary = DegreeFourRetirementSummary;
pub type DegreeFourRetirementSiteAudit = DegreeFourRetirementSite;
pub type DegreeFourRetirementTrialAudit = DegreeFourRetirementTrial;
