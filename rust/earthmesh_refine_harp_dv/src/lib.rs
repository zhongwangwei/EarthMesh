//! HARP-DV: threshold-driven, multi-cycle adaptation of a spherical
//! Delaunay-Voronoi mesh.
//!
//! A third refinement backend, beside Method-C and red-green rather than over
//! either. Where Method-C fits a request to a lattice it can build and
//! red-green splits marked triangles by template, HARP-DV re-reads the criteria
//! against the cells that exist now, and changes the mesh locally where they
//! are still unmet.
//!
//! # What this crate is at Phase 1
//!
//! The skeleton: stable site identity, a validated config, an error model, the
//! evidence a criterion returns, and a run that correctly reports having done
//! nothing. No criteria, no transactions, no insertion.
//!
//! That is not modesty about scope. The machinery HARP-DV needs from
//! `earthmesh_mesh` -- patch extraction, spherical point location, a Delaunay
//! cavity, local insertion, edge legalization, local Voronoi rebuild, patch
//! validation and replacement -- **does not exist**, and neither does the
//! incremental spherical Delaunay kernel underneath it or the robust
//! orientation predicates underneath that. `docs/HARP_DV_REUSE_MAP.md` records
//! the audit, function by function. Building on top of that gap before closing
//! it would produce something that compiles and cannot work.

pub mod api;
pub mod config;
pub mod criteria;
pub mod error;
pub mod report;
pub mod state;

pub use api::{refine_harp_dv, HarpDvOutcome, HarpDvRequest};
pub use config::HarpDvConfig;
pub use criteria::{CriterionSemantics, DemandEvidence, EvidenceStopReason};
pub use error::{HarpDvError, Result};
pub use report::{HarpDvRunReport, StopReason};
pub use state::{AdaptiveMesh, AdaptiveSite, SiteId, SiteIdAllocator, SiteMobility};

#[cfg(test)]
mod tests;
