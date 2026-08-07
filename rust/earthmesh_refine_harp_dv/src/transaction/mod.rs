//! One proposed change, checked before it is kept.
//!
//! Specification section 13. A transaction proposes a site, carries out the
//! local insertion, puts the result through the hard gates, and either commits
//! it or restores the neighbourhood exactly as it was. Nothing between those
//! two outcomes is left in the mesh.
//!
//! # Why the degree gate is first, and not optional
//!
//! The gridfile carries seven neighbours per cell and no more: `ItabW`'s
//! `im`/`iv`/`iw` rows are `[i32; 7]`, and the ring walk in
//! `icosahedron_m_neighbors` refuses a valence above seven. Method-C holds the
//! bound with its transition rows; red-green holds it with its judge chain.
//!
//! Delaunay insertion holds nothing. Measured on an NXP 6 sphere, ten
//! insertions are enough to produce a site of degree eight -- a mesh that is
//! valid, closed, Delaunay, and cannot be written. So the bound is a gate on
//! every transaction rather than an audit at the end: by the time a run has
//! finished there is no local change left to undo.
//!
//! # What is not here yet
//!
//! The improvement gate of section 13.3 -- the objective \Phi with its three
//! weights. The specification offers a discrete version for the MVP and that is
//! what belongs here first: every term of the continuous one needs a constant
//! nobody has measured, and a gate tuned by guesswork rejects and accepts for
//! reasons no one can trace.

use std::collections::BTreeSet;

use earthmesh_mesh::{
    CartesianPoint, InsertionError, MeshState, VoronoiError, MESH_STATE_FIRST_ID,
};

use crate::candidate::{candidates_for_site, CandidatePolicy, CandidateSource};
use crate::error::{HarpDvError, Result};
use crate::state::{AdaptiveMesh, SiteId};

/// The most neighbours a cell can have and still be written to a gridfile.
pub const GRIDFILE_MAX_VERTEX_DEGREE: usize = 7;

/// What a transaction must satisfy to be kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HardGates {
    /// The largest vertex degree the run will accept.
    pub max_vertex_degree: usize,
    /// Whether the mesh must still be closed afterwards.
    ///
    /// True for a sphere. A regional mesh with a real boundary sets it false
    /// and gates on the boundary topology instead.
    pub require_closed_surface: bool,
}

impl Default for HardGates {
    fn default() -> Self {
        Self {
            max_vertex_degree: GRIDFILE_MAX_VERTEX_DEGREE,
            require_closed_surface: true,
        }
    }
}

/// Why a proposal was not kept.
#[derive(Clone, Debug, PartialEq)]
pub enum Rejection {
    /// The point could not be inserted at all.
    NotInsertable(InsertionError),
    /// A site would have ended with more neighbours than the run allows.
    DegreeOverBudget {
        site: usize,
        degree: usize,
        budget: usize,
    },
    /// The change opened the surface.
    SurfaceOpened { open_edges: usize },
    /// The change left an adjacency that is not a triangulation.
    TopologyInvalid { faults: Vec<String> },
    /// A fan could not be walked, so the neighbourhood could not be checked.
    Unmeasurable(VoronoiError),
    /// The mesh could not be walked back to Delaunay after the move.
    CouldNotLegalize(String),
    /// One of the twelve pentagons stopped being a pentagon.
    ProtectedPentagonDisturbed { site: usize, degree: usize },
    /// Legal, and no better than what it replaced.
    ///
    /// Section 13.3. A move that passes every hard gate and improves nothing
    /// is the one that makes a loop churn: it is accepted, so the driver reads
    /// progress, and the next cycle finds the same violation.
    NoImprovement { site: usize },
}

impl std::fmt::Display for Rejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotInsertable(error) => write!(formatter, "the site does not insert: {error}"),
            Self::DegreeOverBudget {
                site,
                degree,
                budget,
            } => write!(
                formatter,
                "site {site} would have {degree} neighbours and the run allows {budget}; the \
                 gridfile carries {GRIDFILE_MAX_VERTEX_DEGREE}"
            ),
            Self::SurfaceOpened { open_edges } => write!(
                formatter,
                "the change left {open_edges} edges with nothing across them"
            ),
            Self::TopologyInvalid { faults } => write!(
                formatter,
                "the change left a mesh that is not a triangulation: {}",
                faults.join("; ")
            ),
            Self::Unmeasurable(error) => {
                write!(formatter, "the neighbourhood could not be checked: {error}")
            }
            Self::ProtectedPentagonDisturbed { site, degree } => write!(
                formatter,
                "site {site} is one of the twelve pentagons and would have degree {degree}; the \
                 gridfile rebuild refuses a protected pentagon that is not degree five"
            ),
            Self::NoImprovement { site } => write!(
                formatter,
                "the change at site {site} was legal and no better than what it replaced"
            ),
            Self::CouldNotLegalize(message) => write!(
                formatter,
                "the mesh could not be restored to Delaunay after the move: {message}"
            ),
        }
    }
}

/// What one committed transaction did.
#[derive(Clone, Debug, PartialEq)]
pub struct TransactionReport {
    pub site_id: SiteId,
    /// The site's row in the triangulation.
    pub vertex: usize,
    pub triangles_removed: usize,
    pub triangles_created: usize,
    /// The largest degree among the sites this change touched.
    pub max_degree_touched: usize,
}

/// A proposal's outcome. Rejected means the mesh is untouched.
#[derive(Clone, Debug, PartialEq)]
pub enum Acceptance {
    Committed(TransactionReport),
    RolledBack(Rejection),
}

impl Acceptance {
    pub fn committed(&self) -> Option<&TransactionReport> {
        match self {
            Self::Committed(report) => Some(report),
            Self::RolledBack(_) => None,
        }
    }

    pub fn rejection(&self) -> Option<&Rejection> {
        match self {
            Self::RolledBack(rejection) => Some(rejection),
            Self::Committed(_) => None,
        }
    }
}

/// The gates, run against the neighbourhood a change touched.
///
/// Only the neighbourhood: everything else was equal before the change and the
/// change is local, so a global sweep per transaction would re-measure an
/// unchanged mesh once per site inserted.
fn check(
    state: &MeshState,
    touched: &BTreeSet<usize>,
    gates: HardGates,
    pentagons: &[usize; 12],
) -> std::result::Result<usize, Rejection> {
    // The region, not the mesh: the changed triangles plus the ring around
    // them. Everything else was closed and valid before and was not touched.
    let mut region = touched.clone();
    for &triangle in touched {
        region.extend(
            state.neighbours()[triangle]
                .iter()
                .copied()
                .filter(|&neighbour| neighbour >= MESH_STATE_FIRST_ID),
        );
    }
    if gates.require_closed_surface {
        let open = state.open_edges_in(&region);
        if open != 0 {
            return Err(Rejection::SurfaceOpened { open_edges: open });
        }
    }
    let mut worst = 0;
    // Seeded, not scanned. `sites_touching` already knows a triangle at each
    // site, and looking for one instead is linear in the whole mesh -- which
    // makes a per-change check cost more the less of the mesh it changed.
    for (&site, &seed) in state.sites_touching(touched).iter() {
        if site < MESH_STATE_FIRST_ID {
            continue;
        }
        let degree = state
            .vertex_degree_from(site, seed)
            .map_err(Rejection::Unmeasurable)?;
        // The twelve pentagons have to stay pentagons. Found by integration
        // rather than from the design docs: the rebuild that produces the
        // three tables refuses a protected pentagon whose degree has moved,
        // and one insertion beside one is enough to move it to seven. Cheaper
        // to refuse the transaction than to discover at the writer that a
        // whole run cannot be written.
        if pentagons.contains(&site) && degree != 5 {
            return Err(Rejection::ProtectedPentagonDisturbed { site, degree });
        }
        if degree > gates.max_vertex_degree {
            return Err(Rejection::DegreeOverBudget {
                site,
                degree,
                budget: gates.max_vertex_degree,
            });
        }
        worst = worst.max(degree);
    }
    if let Err(errors) = state.validate_region(&region) {
        return Err(Rejection::TopologyInvalid {
            faults: errors.iter().take(4).map(ToString::to_string).collect(),
        });
    }
    Ok(worst)
}

impl AdaptiveMesh {
    /// Propose a site: insert it, check it, keep it or put everything back.
    ///
    /// On rejection the triangulation compares equal to what it was, and the
    /// site table is unchanged -- no id is spent on a site that was not kept,
    /// so an id in a report always names a site that existed.
    pub fn propose_site(&mut self, point: CartesianPoint, gates: HardGates) -> Result<Acceptance> {
        self.propose_site_near(point, None, gates)
    }

    /// The same, starting the search at a triangle already known to be near.
    ///
    /// Worth threading through. Locating a point from a fixed start walks
    /// across the mesh, and that walk is what a proposal costs once everything
    /// else is local: measured over meshes from 11k to 737k triangles, one
    /// proposal goes from 17 to 275 microseconds, and all of the growth is the
    /// walk. A caller proposing near a cell it has just evaluated knows a
    /// triangle there and can pay none of it.
    pub fn propose_site_near(
        &mut self,
        point: CartesianPoint,
        hint: Option<usize>,
        gates: HardGates,
    ) -> Result<Acceptance> {
        let containing = match self.state().locate_triangle(point, hint) {
            Ok(triangle) => triangle,
            Err(error) => return Ok(Acceptance::RolledBack(Rejection::NotInsertable(error))),
        };
        let cavity = match self.state().delaunay_cavity(point, containing) {
            Ok(cavity) => cavity,
            Err(error) => return Ok(Acceptance::RolledBack(Rejection::NotInsertable(error))),
        };
        let patch = self.state().snapshot_around(&cavity);

        let report = match self.state_mut().insert_site(point) {
            Ok(report) => report,
            Err(error) => {
                // Nothing was written, so there is nothing to put back; the
                // patch is dropped rather than applied.
                return Ok(Acceptance::RolledBack(Rejection::NotInsertable(error)));
            }
        };
        let touched: BTreeSet<usize> = report.created.iter().copied().collect();

        let pentagons = self.pentagon_ids();
        match check(self.state(), &touched, gates, &pentagons) {
            Ok(max_degree_touched) => {
                let site_id = self.adopt_inserted_site(report.site);
                Ok(Acceptance::Committed(TransactionReport {
                    site_id,
                    vertex: report.site,
                    triangles_removed: report.removed.len(),
                    triangles_created: report.created.len(),
                    max_degree_touched,
                }))
            }
            Err(rejection) => {
                self.state_mut()
                    .restore_patch(patch)
                    .map_err(|error| HarpDvError::TopologyViolation(error.to_string()))?;
                Ok(Acceptance::RolledBack(rejection))
            }
        }
    }
}

impl AdaptiveMesh {
    /// Move a site, restore Delaunay around it, and keep it only if the gates
    /// pass.
    ///
    /// Section 8.1 puts this ahead of insertion, and the measured reason is in
    /// guide section 11.8: closing the last neighbour-scale ratios needs cells
    /// the degree gate refuses, and moving a site changes scale without
    /// changing anyone's degree.
    ///
    /// Unlike an insertion this is not local by construction. Moving a site
    /// leaves its triangles in place and they may no longer be Delaunay, so a
    /// legalization pass follows -- and a flip can make a neighbouring edge
    /// illegal, so the repair reaches past the fan it started in. The patch is
    /// taken over the whole fan and its ring for that reason.
    /// `improves` is section 13.3's improvement gate, in the discrete form the
    /// specification offers for the MVP: the hard gates say the mesh is legal,
    /// and this says it is better than it was. Both are needed, and the
    /// measurement that says so is in guide section 11.9 -- without it a
    /// balance run committed 389 moves over 40 cycles and left more violations
    /// than it started with, because every one of them was legal and the loop
    /// read "something was accepted" as progress.
    pub fn propose_move(
        &mut self,
        site: usize,
        destination: CartesianPoint,
        gates: HardGates,
        improves: &dyn Fn(&MeshState) -> bool,
    ) -> Result<Acceptance> {
        let fan: BTreeSet<usize> = match self.state().triangle_fan(site) {
            Ok(fan) => fan.into_iter().collect(),
            Err(error) => return Ok(Acceptance::RolledBack(Rejection::Unmeasurable(error))),
        };
        // Two rings of snapshot for one ring of flips. A flip rewrites the
        // adjacency of both triangles *and* of everything across their edges,
        // so a patch that covered only what may be flipped would leave those
        // outside rewrites unrestorable -- and a rollback that puts most of a
        // change back leaves a mesh that is neither the old one nor the new
        // one, and that validates. Measured: without the extra ring, a balance
        // run ends with four asymmetric neighbour pairs.
        let reach: BTreeSet<usize> = self.state().snapshot_around(&fan).triangles().collect();
        let patch = self.state().snapshot_around(&reach);
        let origin = self.state().vertices()[site];

        self.state_mut().move_vertex(site, destination);
        let touched = match self.state_mut().legalize_within(&fan, Some(&reach)) {
            // Everything a flip could have touched is inside `reach`, and so
            // is everything the gates have to re-check.
            Ok(_) => reach.clone(),
            Err(error) => {
                self.state_mut().move_vertex(site, origin);
                self.state_mut()
                    .restore_patch(patch)
                    .map_err(|error| HarpDvError::TopologyViolation(error.to_string()))?;
                return Ok(Acceptance::RolledBack(Rejection::CouldNotLegalize(
                    error.to_string(),
                )));
            }
        };

        let pentagons = self.pentagon_ids();
        let verdict = match check(self.state(), &touched, gates, &pentagons) {
            Ok(max_degree_touched) if improves(self.state()) => Ok(max_degree_touched),
            Ok(_) => Err(Rejection::NoImprovement { site }),
            Err(rejection) => Err(rejection),
        };
        match verdict {
            Ok(max_degree_touched) => {
                let site_id = self.record_moved_site(site);
                Ok(Acceptance::Committed(TransactionReport {
                    site_id,
                    vertex: site,
                    triangles_removed: 0,
                    triangles_created: 0,
                    max_degree_touched,
                }))
            }
            Err(rejection) => {
                self.state_mut().move_vertex(site, origin);
                self.state_mut()
                    .restore_patch(patch)
                    .map_err(|error| HarpDvError::TopologyViolation(error.to_string()))?;
                Ok(Acceptance::RolledBack(rejection))
            }
        }
    }
}

/// What became of one demand after the whole ladder was tried.
#[derive(Clone, Debug, PartialEq)]
pub enum DemandOutcome {
    /// A candidate passed, and which rung produced it.
    Resolved {
        source: CandidateSource,
        report: TransactionReport,
    },
    /// Every candidate was refused, and why each was.
    ///
    /// Specification section 13.4: the last candidate is not committed
    /// unconditionally. A mesh that kept a point nothing accepted is worse than
    /// a mesh that did not refine, because the first cannot be told from a mesh
    /// that was refined correctly.
    Unresolved {
        refusals: Vec<(CandidateSource, Rejection)>,
    },
    /// The cell could not be read, so no candidate was generated.
    NotAttempted(VoronoiError),
}

impl DemandOutcome {
    pub fn resolved(&self) -> Option<&TransactionReport> {
        match self {
            Self::Resolved { report, .. } => Some(report),
            _ => None,
        }
    }
}

impl AdaptiveMesh {
    /// Refine one cell: try the ladder, keep the first candidate that passes.
    ///
    /// Every attempt that fails is rolled back before the next is tried, so at
    /// most one of them is ever in the mesh, and none is if they all fail.
    pub fn refine_cell(
        &mut self,
        site: usize,
        witness: Option<CartesianPoint>,
        policy: CandidatePolicy,
        gates: HardGates,
    ) -> Result<DemandOutcome> {
        let ladder = match candidates_for_site(self.state(), site, witness, policy) {
            Ok(ladder) => ladder,
            Err(error) => return Ok(DemandOutcome::NotAttempted(error)),
        };
        let mut refusals = Vec::with_capacity(ladder.len());
        for candidate in ladder {
            match self.propose_site_near(candidate.point, Some(candidate.hint), gates)? {
                Acceptance::Committed(report) => {
                    return Ok(DemandOutcome::Resolved {
                        source: candidate.source,
                        report,
                    })
                }
                Acceptance::RolledBack(rejection) => refusals.push((candidate.source, rejection)),
            }
        }
        Ok(DemandOutcome::Unresolved { refusals })
    }
}

/// The sites a run added, in the order it added them.
pub fn committed_site_ids(outcomes: &[Acceptance]) -> Vec<SiteId> {
    outcomes
        .iter()
        .filter_map(|outcome| outcome.committed().map(|report| report.site_id))
        .collect()
}

#[cfg(test)]
mod tests;
