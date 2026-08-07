//! The layer between what a project asks for and the backend that builds it.
//!
//! Three backends refine meshes, and they disagree about almost everything:
//! Method-C nests regions on a lattice, red-green splits marked triangles by
//! template, HARP-DV inserts sites where the criteria are still unmet. What
//! they share is upstream of all of it -- how a request is expressed, what a
//! criterion says about a cell, and what that becomes as demand. That is this
//! crate.
//!
//! # Layering
//!
//! ```text
//! earthmesh_mesh, earthmesh_boundary      geometry and topology
//!         ↑
//! earthmesh_refine                        request, criteria, demand, h-field
//!         ↑
//! method_c   redgreen   harp_dv           the backends, as siblings
//! ```
//!
//! This crate must never depend on a backend. That edge is what would turn
//! three siblings into a chain.

pub mod api;
pub mod criteria;
pub mod demand;
pub mod hfield;

pub use api::RefinementBackend;
pub use criteria::{CriterionSemantics, DemandEvidence, EvidenceStopReason};
pub use demand::{order_demands, RefinementCause, RefinementDemand};

#[cfg(test)]
mod tests;
