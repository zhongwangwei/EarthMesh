//! What evidence becomes once a run has to act on it.

use earthmesh_mesh::LonLatDegrees;

use crate::criteria::DemandEvidence;

/// Why a cell is being refined, kept apart from how much.
///
/// Balance and quality demands are counted separately from physical ones,
/// because a run that added a million cells to satisfy its own scale ratio
/// should not read as a run that found a million cells of coastline.
#[derive(Clone, Debug, PartialEq)]
pub enum RefinementCause {
    /// A criterion over the data asked.
    PhysicalCriterion { criterion_id: String },
    /// The project named this region outright.
    UserSpecified,
    /// A boundary has to be resolved.
    BoundaryResolution,
    /// A neighbour got fine enough that this cell is now too coarse beside it.
    ScaleBalance { ratio_before: f64 },
    /// The mesh here is not usable and has to be mended.
    QualityRepair,
}

impl RefinementCause {
    /// Whether this cause came from the data rather than from the mesh's own
    /// consistency rules.
    pub fn is_physical(&self) -> bool {
        matches!(
            self,
            Self::PhysicalCriterion { .. } | Self::UserSpecified | Self::BoundaryResolution
        )
    }
}

/// Everything asked of one cell, from every criterion that spoke.
#[derive(Clone, Debug, PartialEq)]
pub struct RefinementDemand {
    /// Whatever the backend uses to name a cell. Opaque here on purpose: this
    /// layer does not know what a cell is in any particular backend.
    pub cell: u64,
    pub evidences: Vec<DemandEvidence>,
    pub priority: f64,
    pub requested_scale_m: Option<f64>,
    pub preferred_witness: Option<LonLatDegrees>,
    pub hard: bool,
    pub cause: RefinementCause,
}

impl RefinementDemand {
    /// Gather one cell's evidence into a demand.
    ///
    /// The requested scale is the finest any criterion asked for, because a
    /// cell that satisfies the loosest of several requests still fails the
    /// others. The witness comes from the strongest violation, since that is
    /// the evidence with most to say about where the trouble is.
    pub fn from_evidence(
        cell: u64,
        evidences: Vec<DemandEvidence>,
        cause: RefinementCause,
    ) -> Self {
        let requested_scale_m = evidences
            .iter()
            .filter_map(|evidence| evidence.requested_scale_m)
            .fold(None::<f64>, |finest, scale| {
                Some(finest.map_or(scale, |current: f64| current.min(scale)))
            });
        let hard = evidences.iter().any(|evidence| evidence.hard_requirement);
        let strongest = evidences
            .iter()
            .filter(|evidence| evidence.demands_work())
            .max_by(|left, right| {
                left.normalized_violation
                    .partial_cmp(&right.normalized_violation)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        let priority = strongest.map_or(0.0, |evidence| {
            evidence.normalized_violation * evidence.confidence
        });
        Self {
            cell,
            preferred_witness: strongest.and_then(|evidence| evidence.witness),
            evidences,
            priority,
            requested_scale_m,
            hard,
            cause,
        }
    }

    /// Whether anything here still asks for work.
    pub fn demands_work(&self) -> bool {
        self.evidences.iter().any(DemandEvidence::demands_work)
    }
}

/// Order demands so a run processes them the same way twice.
///
/// Hard before soft, then by priority, then by cell id. The last term is what
/// makes it total: two demands with equal priority still have an order, and it
/// is the same order on every machine.
pub fn order_demands(demands: &mut [RefinementDemand]) {
    demands.sort_by(|left, right| {
        right
            .hard
            .cmp(&left.hard)
            .then_with(|| {
                right
                    .priority
                    .partial_cmp(&left.priority)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| left.cell.cmp(&right.cell))
    });
}
