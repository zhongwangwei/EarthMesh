use crate::{auto_refine_level_cap, effective_auto_refine_pass, next_auto_refine_pass};

/// Observation produced by one engine/quality attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AutoRefineEvent {
    QualityPassed,
    QualityViolation,
    EngineFailed(String),
}

/// Pure orchestration decision; callers own all engine and filesystem effects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AutoRefineAction {
    Complete { pass: u8 },
    Retry { next_pass: u8 },
    CapReached { pass: u8, cap: u8 },
    AbortEngine { pass: u8, message: String },
}

/// Bounded AutoRefine progression independent of the CLI/engine implementation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutoRefineState {
    current_pass: u8,
    target_nxp: i32,
}

impl AutoRefineState {
    pub fn new(requested_pass: u8, target_nxp: i32) -> Self {
        Self {
            current_pass: effective_auto_refine_pass(requested_pass, target_nxp),
            target_nxp,
        }
    }

    pub fn current_pass(&self) -> u8 {
        self.current_pass
    }

    pub fn cap(&self) -> u8 {
        auto_refine_level_cap(self.target_nxp)
    }

    pub fn transition(&mut self, event: AutoRefineEvent) -> AutoRefineAction {
        match event {
            AutoRefineEvent::QualityPassed => AutoRefineAction::Complete {
                pass: self.current_pass,
            },
            AutoRefineEvent::QualityViolation => {
                if let Some(next_pass) = next_auto_refine_pass(self.current_pass, self.target_nxp) {
                    self.current_pass = next_pass;
                    AutoRefineAction::Retry { next_pass }
                } else {
                    AutoRefineAction::CapReached {
                        pass: self.current_pass,
                        cap: self.cap(),
                    }
                }
            }
            AutoRefineEvent::EngineFailed(message) => AutoRefineAction::AbortEngine {
                pass: self.current_pass,
                message,
            },
        }
    }
}
