//! What a caller hands in, and what comes back.

use crate::candidate::CandidatePolicy;
use crate::config::HarpDvConfig;
use crate::criteria::CellCriterion;
use crate::cycle::{run_cycles, CycleLimits};
use crate::error::Result;
use crate::report::{HarpDvRunReport, StopReason};
use crate::state::AdaptiveMesh;
use crate::transaction::HardGates;

/// One run's worth of instruction.
pub struct HarpDvRequest<'a> {
    pub config: HarpDvConfig,
    /// What the run is trying to satisfy. An empty list is a run with nothing
    /// to do, which is a legitimate request and reports itself as satisfied.
    pub criteria: &'a [Box<dyn CellCriterion>],
    /// Where a candidate may go.
    pub candidate_policy: CandidatePolicy,
    /// What a transaction must satisfy to be kept.
    pub gates: HardGates,
}

/// What the run produced, and the account of how.
#[derive(Debug)]
pub struct HarpDvOutcome {
    pub mesh: AdaptiveMesh,
    pub report: HarpDvRunReport,
    /// Cells whose demand no candidate could serve.
    ///
    /// Returned rather than only counted: a caller deciding whether to accept
    /// the mesh needs to know *where* it fell short, and a number cannot say.
    pub unresolved_cells: Vec<usize>,
}

/// Adapt a mesh until every criterion is satisfied or the run has to stop.
///
/// The config is validated before the mesh is touched, so a request that could
/// never be honoured fails without having half-refined anything.
pub fn refine_harp_dv(
    mut mesh: AdaptiveMesh,
    request: &HarpDvRequest<'_>,
) -> Result<HarpDvOutcome> {
    request.config.validate()?;
    if request.criteria.is_empty() {
        let sites = mesh.active_site_count();
        return Ok(HarpDvOutcome {
            mesh,
            report: HarpDvRunReport::empty(sites, StopReason::AllSatisfied),
            unresolved_cells: Vec::new(),
        });
    }

    let limits = CycleLimits {
        max_cycles: request.config.max_cycles,
        max_sites: request.config.maximum_cells,
        minimum_cell_width_m: request.config.minimum_cell_width_m,
        max_neighbour_scale_ratio: request.config.maximum_neighbor_scale_ratio,
    };
    let outcome = run_cycles(
        &mut mesh,
        request.criteria,
        request.candidate_policy,
        request.gates,
        limits,
    )?;
    Ok(HarpDvOutcome {
        mesh,
        report: outcome.report,
        unresolved_cells: outcome.unresolved_cells,
    })
}
