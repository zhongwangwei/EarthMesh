//! What a caller hands in, and what comes back.

use crate::config::HarpDvConfig;
use crate::error::Result;
use crate::report::{HarpDvRunReport, StopReason};
use crate::state::AdaptiveMesh;

/// One run's worth of instruction.
///
/// Criteria are absent from this struct in Phase 1 because the trait they would
/// implement needs a decision the audit raised and nobody has taken:
/// `earthmesh_refine_planner` already carries a `RefinementCriterion` and a
/// `CriterionContext`, with different semantics from the ones the spec asks
/// for. Declaring a second trait of the same name before that is settled would
/// be the confusion, not the fix. See `docs/HARP_DV_REUSE_MAP.md`.
#[derive(Clone, Debug)]
pub struct HarpDvRequest {
    pub config: HarpDvConfig,
}

/// What the run produced, and the account of how.
#[derive(Debug)]
pub struct HarpDvOutcome {
    pub mesh: AdaptiveMesh,
    pub report: HarpDvRunReport,
}

/// Adapt a mesh until every criterion is satisfied or the run has to stop.
///
/// Phase 1 has no criteria to evaluate and no transaction machinery to run, so
/// a request always describes a run with nothing to do: the mesh comes back as
/// it went in, with a report saying so. That is the identity this phase is
/// meant to establish, and the thing later phases must not break.
///
/// It is deliberately not a `todo!()`. A production path that panics is worse
/// than one that says, truthfully, that it did nothing.
pub fn refine_harp_dv(mesh: AdaptiveMesh, request: &HarpDvRequest) -> Result<HarpDvOutcome> {
    request.config.validate()?;
    let sites = mesh.active_site_count();
    Ok(HarpDvOutcome {
        mesh,
        report: HarpDvRunReport::empty(sites, StopReason::AllSatisfied),
    })
}
