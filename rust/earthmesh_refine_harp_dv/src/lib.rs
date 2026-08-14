//! HARP-DV: threshold-driven, multi-cycle adaptation of a spherical
//! Delaunay-Voronoi mesh.
//!
//! A third refinement backend, beside Method-C and red-green rather than over
//! either. Where Method-C fits a request to a lattice it can build and
//! red-green splits marked triangles by template, HARP-DV re-reads the criteria
//! against the cells that exist now, and changes the mesh locally where they
//! are still unmet.
//!
//! # What is here
//!
//! The whole loop of specification section 8: read every Voronoi cell against
//! the active criteria, build demands from the evidence, order them, and serve
//! each with a transaction that either commits or restores the neighbourhood
//! exactly as it was. The machinery underneath -- robust predicates, local
//! insertion, patch rollback, local dual rebuild -- lives in `earthmesh_mesh`,
//! where the other backends can reach it too.
//!
//! # What is not
//!
//! The data-backed physical criteria of section 8.2; this crate owns the
//! geometry-only `TargetScale` and `MinAngle` criteria, not raster sources.
//!
//! Section 13.3's continuous improvement gate, whose three weights nobody has
//! measured. The hard gates of 13.2 are here; the discrete MVP the spec offers
//! beside the continuous objective is what belongs next.
//!
//! Boundary-curve and interface-curve r-adaptation. Unconstrained runs move
//! interior sites transactionally; protected-segment runs keep every site in
//! place because Ruppert's termination invariant depends on those positions.
//!
pub mod api;
pub mod candidate;
pub mod config;
pub mod criteria;
pub mod cycle;
pub mod error;
pub mod report;
pub mod state;
pub mod transaction;

pub use api::{refine_harp_dv, HarpDvOutcome, HarpDvRequest};
pub use config::HarpDvConfig;
pub use criteria::{CellCriterion, CellView, MinAngle, TargetRegion, TargetScale};
pub use cycle::{run_cycles, CycleLimits, CycleOutcome};
pub use error::{HarpDvError, Result};
// The criteria vocabulary is the refinement layer's, not this backend's. Three
// backends reading the same evidence is the point; a private copy per backend
// would be three vocabularies that drift.
pub use candidate::{Candidate, CandidatePolicy, CandidateSource};
pub use earthmesh_refine::{
    order_demands, CriterionSemantics, DemandEvidence, EvidenceStopReason, RefinementBackend,
    RefinementCause, RefinementDemand,
};
pub use report::{AngleWindowVerdict, HarpDvRunReport, RejectionTally, StopReason};
pub use state::{
    AdaptiveMesh, AdaptiveSite, ConservativeRemapWeight, SiteId, SiteIdAllocator, SiteMobility,
};
pub use transaction::{
    committed_site_ids, Acceptance, DemandOutcome, HardGates, Rejection, TransactionReport,
    GRIDFILE_MAX_VERTEX_DEGREE,
};

#[cfg(test)]
mod tests;
