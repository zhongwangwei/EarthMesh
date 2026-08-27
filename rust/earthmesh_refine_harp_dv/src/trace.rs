//! Typed, serialization-free live trace records for HARP-DV.

use crate::certifier::{AngleViolation, MeshCertification};

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
}
