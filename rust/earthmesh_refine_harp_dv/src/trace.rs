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
    WindowBudgetPassSummary(WindowBudgetPassSummary),
    WindowBudgetArmSummary(WindowBudgetArmSummary),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WindowBudgetArm {
    W32,
    W64,
    W96,
}

impl WindowBudgetArm {
    pub const ALL: [Self; 3] = [Self::W32, Self::W64, Self::W96];

    pub const fn pass_limit(self) -> usize {
        match self {
            Self::W32 => 32,
            Self::W64 => 64,
            Self::W96 => 96,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::W32 => "W32",
            Self::W64 => "W64",
            Self::W96 => "W96",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowBudgetStopReason {
    PassLimit,
    NoRetainedMoves,
    CompletedNoImprovementSweep,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WindowBudgetPassSummary {
    pub arm: WindowBudgetArm,
    pub pass_index: usize,
    pub window_pass_limit: usize,
    pub per_pass_site_budget: usize,
    pub processed_sites: usize,
    pub eligible_sites: usize,
    pub found_sites: usize,
    pub unique_sites_seen: usize,
    pub candidate_count: usize,
    pub line_search_attempt_count: usize,
    pub retained_move_count: usize,
    pub completed_breadth_sweep: bool,
    pub below_40_count: usize,
    pub above_80_count: usize,
    pub total_violation_count: usize,
    pub resolved_s3_cohort_key_count: usize,
    pub persisted_s3_cohort_key_count: usize,
    pub kind_changed_s3_cohort_key_count: usize,
    pub new_global_angle_key_count: usize,
    pub worst_window_deviation_deg: f64,
    pub window_penalty: f64,
    pub eta_min: f64,
    pub eta_p1: f64,
    pub physical_demands_remaining: usize,
    pub balance_demands_remaining: usize,
    pub unbalanced_pairs_remaining: usize,
    pub wall_time_ms: u64,
    pub stop_reason_if_terminal: Option<WindowBudgetStopReason>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WindowBudgetArmSummary {
    pub arm: WindowBudgetArm,
    pub window_pass_limit: usize,
    pub pass_count: usize,
    pub s3_violation_key_count: usize,
    pub s4_below_40_count: usize,
    pub s4_above_80_count: usize,
    pub s4_total_violation_count: usize,
    pub s4_worst_window_deviation_deg: f64,
    pub s4_window_penalty: f64,
    pub s4_eta_min: f64,
    pub s4_eta_p1: f64,
    pub s4_physical_demands_remaining: usize,
    pub s4_balance_demands_remaining: usize,
    pub s4_unbalanced_pairs_remaining: usize,
    pub s4_resolved_s3_cohort_key_count: usize,
    pub s4_persisted_s3_cohort_key_count: usize,
    pub s4_kind_changed_s3_cohort_key_count: usize,
    pub s4_new_global_angle_key_count: usize,
    pub s6_below_40_count: usize,
    pub s6_above_80_count: usize,
    pub s6_total_violation_count: usize,
    pub s6_worst_window_deviation_deg: f64,
    pub s6_window_penalty: f64,
    pub s6_eta_min: f64,
    pub s6_eta_p1: f64,
    pub s6_physical_demands_remaining: usize,
    pub s6_balance_demands_remaining: usize,
    pub s6_unbalanced_pairs_remaining: usize,
    pub s6_resolved_s3_cohort_key_count: usize,
    pub s6_persisted_s3_cohort_key_count: usize,
    pub s6_kind_changed_s3_cohort_key_count: usize,
    pub s6_new_global_angle_key_count: usize,
    pub final_low_degree_moves: usize,
    pub default_leaf_retirements: usize,
    pub wall_time_ms: u64,
    pub stop_reason: WindowBudgetStopReason,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DegreeFourCheckStatus {
    Pass,
    Fail,
    NotEvaluated,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DegreeFourCheckCounts {
    pub pass: usize,
    pub fail: usize,
    pub not_evaluated: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DegreeFourRetirementCheckCounts {
    pub geometry: DegreeFourCheckCounts,
    pub hard_gate: DegreeFourCheckCounts,
    pub physical_demand: DegreeFourCheckCounts,
    pub scale_balance: DegreeFourCheckCounts,
    pub no_new_low_degree: DegreeFourCheckCounts,
    pub angle_count: DegreeFourCheckCounts,
    pub worst_deviation: DegreeFourCheckCounts,
    pub penalty: DegreeFourCheckCounts,
    pub eta: DegreeFourCheckCounts,
    pub margin: DegreeFourCheckCounts,
    pub conservative_remap: DegreeFourCheckCounts,
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
    pub checks: DegreeFourRetirementCheckCounts,
    pub trials_quality_improving: usize,
    pub trials_fully_acceptable: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DegreeFourRetirementSite {
    pub site_id: SiteId,
    pub vertex: usize,
    pub interior_leaf: bool,
    pub window_violation: bool,
    pub candidate_rank: Option<usize>,
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
    pub ring_site_ids: Option<[SiteId; 4]>,
    pub diagonal_site_ids: Option<[SiteId; 2]>,
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
