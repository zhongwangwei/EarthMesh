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

use crate::candidate::{
    candidates_for_site, fallback_candidates_for_site, Candidate, CandidatePolicy, CandidateSource,
};
use crate::error::{HarpDvError, Result};
use crate::state::{AdaptiveMesh, SiteId};

/// The most neighbours a cell can have and still be written to a gridfile.
pub const GRIDFILE_MAX_VERTEX_DEGREE: usize = earthmesh_core::DEFAULT_HARP_DV_MAXIMUM_VERTEX_DEGREE;

/// The sites a change touches, each with a triangle that names it.
///
/// The seed is the point: a neighbourhood read that scans for one is linear in
/// the whole mesh, which makes a per-change check cost more the less of the
/// mesh it changed. `sites_touching` already knows one for every site it
/// returns, and this carries it through to the objective instead of dropping
/// it on the way.
pub(crate) type AffectedSites = std::collections::BTreeMap<usize, usize>;

/// What a transaction must satisfy to be kept.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HardGates {
    /// The largest vertex degree the run will accept.
    pub max_vertex_degree: usize,
    /// The smallest angle any triangle the change touches may have, in
    /// degrees.
    ///
    /// Zero disables this angle gate. Writer admissibility is checked
    /// independently and remains active.
    pub min_triangle_angle_deg: f64,
    /// Whether the mesh must still be closed afterwards.
    ///
    /// True for a sphere, and nothing sets it otherwise today.
    ///
    /// **Setting it false removes this gate and puts nothing in its place.** An
    /// earlier version of this comment said a regional mesh "gates on the
    /// boundary topology instead"; there is no such gate. A regional backend
    /// needs one written before it can turn this off -- `earthmesh_boundary`
    /// holds the model it would gate against, and no backend consumes that
    /// crate yet.
    pub require_closed_surface: bool,
    /// The largest patch one transaction may snapshot, in triangles.
    ///
    /// `HarpDvConfig::maximum_patch_cells` reaches the run through here.
    /// Before this it was declared, validated and passed to nothing -- a bound
    /// accepted and ignored, which the config's own note about `deterministic`
    /// says a flag must never be.
    ///
    /// The bound is on what a rollback has to be able to put back, so it is
    /// checked against the snapshot rather than against the cavity: those
    /// differ, and it is the snapshot that is held in memory.
    pub max_patch_triangles: usize,
}

impl Default for HardGates {
    fn default() -> Self {
        Self {
            max_vertex_degree: GRIDFILE_MAX_VERTEX_DEGREE,
            // Off by default. This floor is a real lever on quality and not
            // merely a guard against degeneracy -- every insertion that would
            // leave a thin triangle is refused, the ladder falls to a more
            // conservative rung, and the finished mesh's worst angle tracks the
            // number (guide 11.33, 11.34):
            //
            //      5 deg -> 17.07 worst, 7723 cells
            //     28     -> 28.12,       7371
            //     30     -> 30.00,       7132
            //     31     -> 31.01,       6162
            //     32     -> 32.01,       4835
            //     36     -> no refinement survives at all
            //
            // It is a lever in both directions, which is why it is now zero:
            // 30 rejected 7,470 production demands, 25 rejected fewer, and the
            // insertion audit of guide 11.65 measured 133 of 232 repair
            // candidates dying on the hard gates with the floor among them. A
            // run that wants the angles bought back sets
            // `NL%minimum_triangle_angle_deg`; the shared quality report still
            // warns at 25 either way, so the cost of leaving it off is
            // reported rather than hidden.
            min_triangle_angle_deg: earthmesh_core::DEFAULT_HARP_DV_MINIMUM_TRIANGLE_ANGLE_DEG,
            require_closed_surface: true,
            max_patch_triangles: earthmesh_core::DEFAULT_HARP_DV_MAXIMUM_PATCH_CELLS,
        }
    }
}

/// Why a proposal was not kept.
#[derive(Clone, Debug, PartialEq)]
pub enum Rejection {
    /// The change would have snapshotted more than the run allows to be held
    /// for rollback.
    PatchTooLarge { triangles: usize, allowed: usize },
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
    /// The change left a triangle too thin to write.
    SliverTriangle { triangle: usize, angle_deg: f64 },
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
            Self::PatchTooLarge { triangles, allowed } => write!(
                formatter,
                "the change would snapshot {triangles} triangles for rollback and the run allows \
                 {allowed}"
            ),
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
            Self::SliverTriangle {
                triangle,
                angle_deg,
            } => write!(
                formatter,
                "triangle {triangle} would have an angle of {angle_deg:.2} degrees, below the \
                 requested transaction minimum"
            ),
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

impl AdaptiveMesh {
    /// Where a candidate should go instead, if it encroaches a segment.
    fn encroachment_of(&self, candidate: &Candidate) -> Option<earthmesh_mesh::Encroachment> {
        if self.segments_are_empty() {
            return None;
        }
        self.state()
            .encroached_segment_edges(candidate.point, self.segments.iter())
    }
}

/// The gates, run against the neighbourhood a change touched.
///
/// Only the neighbourhood: everything else was equal before the change and the
/// change is local, so a global sweep per transaction would re-measure an
/// unchanged mesh once per site inserted.
pub(crate) fn check(
    state: &MeshState,
    touched: &BTreeSet<usize>,
    gates: HardGates,
    pentagons: &[usize; 12],
) -> std::result::Result<usize, Rejection> {
    // The region, not the mesh: the changed triangles plus the ring around
    // them. Everything else was closed and valid before and was not touched.
    let mut region = touched.clone();
    for &triangle in touched {
        if !state.is_triangle_live(triangle) {
            continue;
        }
        region.extend(
            state.neighbours()[triangle]
                .iter()
                .copied()
                .filter(|&neighbour| state.is_triangle_live(neighbour)),
        );
    }
    if gates.require_closed_surface {
        let open = state.open_edges_in(&region);
        if open != 0 {
            return Err(Rejection::SurfaceOpened { open_edges: open });
        }
    }
    // Slivers, checked over the triangles the change actually made or moved.
    //
    // The angle floor is optional and is zero by default; the writer's
    // admissibility test below is not, and used to be switched off with it.
    for &triangle in touched {
        if !state.is_triangle_live(triangle) {
            continue;
        }
        let corners = state.triangles()[triangle];
        let points = [
            state.vertices()[corners[0]],
            state.vertices()[corners[1]],
            state.vertices()[corners[2]],
        ];
        let angle = crate::criteria::smallest_triangle_angle_deg(points);
        if gates.min_triangle_angle_deg > 0.0 && angle < gates.min_triangle_angle_deg {
            return Err(Rejection::SliverTriangle {
                triangle,
                angle_deg: angle,
            });
        }
        // The writer's own test, which an angle floor does not imply: a large
        // obtuse triangle can clear five degrees and still have a circumcentre
        // nowhere near it, which is what `circumcenter_spherical` refuses.
        // Gating on the writer's predicate rather than on a proxy for it is the
        // difference between a refused transaction and a run that dies at
        // output -- so it runs whatever the floor is set to. Nesting it inside
        // the floor's guard, as it was, meant turning the floor off also turned
        // this off, silently, and only at the gridfile would anyone find out.
        let barycentre = CartesianPoint::new(
            (points[0].x + points[1].x + points[2].x) / 3.0,
            (points[0].y + points[1].y + points[2].y) / 3.0,
            (points[0].z + points[1].z + points[2].z) / 3.0,
        );
        let centre = state
            .circumcentre(triangle)
            .map_err(|error| Rejection::TopologyInvalid {
                faults: vec![format!(
                    "triangle {triangle} has no usable spherical circumcentre: {error}"
                )],
            })?;
        if !earthmesh_mesh::circumcenter_is_local_enough(barycentre, centre, points) {
            return Err(Rejection::TopologyInvalid {
                faults: vec![format!(
                    "triangle {triangle} has a non-local spherical circumcentre"
                )],
            });
        }
        // Measurability, on the same argument as the circumcentre test above and
        // with the same scope: gate on the predicate the reader actually uses,
        // not on a proxy for it. `triangle_eta` is what the quality optimiser
        // measures with, and `None` from it aborts the whole optimisation pass
        // with "triangle area-length quality is not measurable" -- a transaction
        // that commits such a triangle fails no gate here and kills the run one
        // phase later, which is the worst place to find out.
        //
        // Not implied by the winding check below. That one asks
        // `orientation_on_sphere` for a sign and refuses `Zero`; this one asks
        // whether Heron's formula still resolves a positive area from three arc
        // lengths. They are different predicates over different quantities, and
        // the second fails first -- Heron loses the area of a needle to
        // cancellation while the determinant still has a decidable sign.
        //
        // Angle-independent by construction, so it holds with the floor at zero.
        // Until the floor was turned off, 25 degrees was quietly doing this job
        // too, and nothing recorded that it was.
        if crate::criteria::triangle_eta(points).is_none() {
            return Err(Rejection::TopologyInvalid {
                faults: vec![format!(
                    "triangle {triangle} has no measurable area-length quality; its smallest \
                     angle is {angle:.6} degrees"
                )],
            });
        }
    }

    // Every triangle here has to wind the same way, and none of them may be
    // degenerate.
    //
    // Nothing else in this function looks. Open edges, degree, pentagons and
    // `validate_region` are all satisfied by a mesh that has turned part of
    // itself inside out: the adjacency still pairs up and the surface is still
    // closed, the triangles simply overlap. Measured, a committed move is what
    // folds the first one -- and once two triangles cover the same patch of
    // sphere, `locate_triangle` answers differently depending on where its walk
    // started, which is what makes the next rollback unsound.
    //
    // Read over `region` rather than `touched`, so the untouched ring is in the
    // comparison: it says which way this neighbourhood was already wound, and
    // no global convention has to be assumed. A closed spherical triangulation
    // has one winding throughout, so a disagreement inside one patch is the
    // fold itself.
    //
    // After the angle floor, not before it, so a triangle that is degenerate
    // *and* thin keeps being reported as the sliver it is. It never depended on
    // the floor being set -- a run that asks for no angle floor is still not
    // asking for a mesh that is inside out -- and now neither does the
    // circumcentre test above it.
    let mut winding = None;
    for &triangle in &region {
        if !state.is_triangle_live(triangle) {
            continue;
        }
        let corners = state.triangles()[triangle];
        let sign = earthmesh_mesh::orientation_on_sphere(
            state.vertices()[corners[0]],
            state.vertices()[corners[1]],
            state.vertices()[corners[2]],
        )
        .map_err(|_| Rejection::TopologyInvalid {
            faults: vec![format!("triangle {triangle} has an undecidable winding")],
        })?;
        if sign == earthmesh_mesh::Sign::Zero {
            return Err(Rejection::TopologyInvalid {
                faults: vec![format!(
                    "triangle {triangle} is degenerate: its three corners lie on one great circle"
                )],
            });
        }
        match winding {
            None => winding = Some(sign),
            Some(first) if first == sign => {}
            Some(_) => {
                return Err(Rejection::TopologyInvalid {
                    faults: vec![format!(
                        "triangle {triangle} winds against the rest of its neighbourhood; the \
                         change folded the surface"
                    )],
                })
            }
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

    /// The same, recording the new site one generation deeper than `parent`.
    pub fn propose_site_for(
        &mut self,
        point: CartesianPoint,
        hint: Option<usize>,
        gates: HardGates,
        parent: usize,
    ) -> Result<Acceptance> {
        self.refining = Some(parent);
        let outcome = self.propose_site_near(point, hint, gates);
        self.refining = None;
        outcome
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
        let patch_size = patch.triangles().count();
        if gates.max_patch_triangles > 0 && patch_size > gates.max_patch_triangles {
            return Ok(Acceptance::RolledBack(Rejection::PatchTooLarge {
                triangles: patch_size,
                allowed: gates.max_patch_triangles,
            }));
        }
        // The cavity this snapshot was taken around, not one the insertion
        // carves for itself. `insert_site` re-locates from no hint, and on a
        // mesh with overlapping triangles it lands somewhere else -- then it
        // rewrites triangles the patch does not hold, and the rollback below
        // truncates the new vertex away while leaving triangles that still name
        // it. The next cavity walk indexes one of those and panics.
        let report = match self
            .state_mut()
            .insert_site_with_cavity(point, containing, &cavity)
        {
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
                let parent = self.refining_site();
                let site_id = self.adopt_inserted_site(report.site, parent);
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
    fn move_neighbourhood(
        &self,
        site: usize,
    ) -> std::result::Result<(BTreeSet<usize>, BTreeSet<usize>), VoronoiError> {
        let fan: BTreeSet<usize> = self.state().triangle_fan(site)?.into_iter().collect();
        let reach: BTreeSet<usize> = self.state().snapshot_around(&fan).triangles().collect();
        Ok((fan, reach))
    }

    pub(crate) fn score_before_move<Score>(
        &self,
        site: usize,
        objective: &dyn Fn(&MeshState, &AffectedSites) -> Option<Score>,
    ) -> Option<Score> {
        let (_, reach) = self.move_neighbourhood(site).ok()?;
        let affected_sites = self.state().sites_touching(&reach);
        objective(self.state(), &affected_sites)
    }

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
    /// `improves` is section 13.3's public improvement gate: the hard gates say
    /// the mesh is legal, and this says it is better than it was. Both are
    /// needed, and the
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
        let objective = |state: &MeshState, _: &AffectedSites| Some(!improves(state));
        self.propose_move_cached(site, destination, gates, &objective, Some(&true), false)
    }

    pub(crate) fn propose_move_cached<Score: PartialOrd>(
        &mut self,
        site: usize,
        destination: CartesianPoint,
        gates: HardGates,
        objective: &dyn Fn(&MeshState, &AffectedSites) -> Option<Score>,
        cached_before: Option<&Score>,
        objective_reads_affected_sites: bool,
    ) -> Result<Acceptance> {
        let (fan, reach) = match self.move_neighbourhood(site) {
            Ok(neighbourhood) => neighbourhood,
            Err(error) => return Ok(Acceptance::RolledBack(Rejection::Unmeasurable(error))),
        };
        // Two rings of snapshot for one ring of flips. A flip rewrites the
        // adjacency of both triangles *and* of everything across their edges,
        // so a patch that covered only what may be flipped would leave those
        // outside rewrites unrestorable -- and a rollback that puts most of a
        // change back leaves a mesh that is neither the old one nor the new
        // one, and that validates. Measured: without the extra ring, a balance
        // run ends with four asymmetric neighbour pairs.
        let patch = self.state().snapshot_around(&reach);
        // The same bound the insertion path checks. It covered only insertion
        // when it went in, so a move could snapshot any amount and the config
        // was half-honoured -- which is the shape it was meant to end.
        let patch_size = patch.triangles().count();
        if gates.max_patch_triangles > 0 && patch_size > gates.max_patch_triangles {
            return Ok(Acceptance::RolledBack(Rejection::PatchTooLarge {
                triangles: patch_size,
                allowed: gates.max_patch_triangles,
            }));
        }
        let affected_sites = if objective_reads_affected_sites {
            self.state().sites_touching(&reach)
        } else {
            AffectedSites::new()
        };
        let computed_before = if cached_before.is_none() {
            objective(self.state(), &affected_sites)
        } else {
            None
        };
        let before = cached_before.or(computed_before.as_ref());
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

        // Seeds taken before the move do not survive it: legalising rewrites the
        // corner arrays of the triangles it flips, so a slot that named this
        // site may no longer. Take them again from the same reach, and require
        // the same sites -- the objective compares two scores over one set of
        // cells, and a set that changed underneath it is not a comparison.
        let affected_sites = if objective_reads_affected_sites {
            let after = self.state().sites_touching(&reach);
            if let Some(&missing) = affected_sites.keys().find(|site| !after.contains_key(site)) {
                self.state_mut().move_vertex(site, origin);
                self.state_mut()
                    .restore_patch(patch)
                    .map_err(|error| HarpDvError::TopologyViolation(error.to_string()))?;
                return Ok(Acceptance::RolledBack(Rejection::Unmeasurable(
                    VoronoiError::SiteIsInNoTriangle { site: missing },
                )));
            }
            after
        } else {
            affected_sites
        };
        let pentagons = self.pentagon_ids();
        let verdict = match check(self.state(), &touched, gates, &pentagons) {
            Ok(max_degree_touched) => {
                let improved = match (
                    before.as_ref(),
                    objective(self.state(), &affected_sites).as_ref(),
                ) {
                    (Some(before), Some(after)) => {
                        after.partial_cmp(before) == Some(std::cmp::Ordering::Less)
                    }
                    _ => false,
                };
                if improved {
                    Ok(max_degree_touched)
                } else {
                    Err(Rejection::NoImprovement { site })
                }
            }
            Err(rejection) => Err(rejection),
        };
        match verdict {
            Ok(max_degree_touched) => {
                let Some(site_id) = self.record_moved_sites(&[site]) else {
                    self.state_mut().move_vertex(site, origin);
                    self.state_mut()
                        .restore_patch(patch)
                        .map_err(|error| HarpDvError::TopologyViolation(error.to_string()))?;
                    return Ok(Acceptance::RolledBack(Rejection::TopologyInvalid {
                        faults: vec![format!("moved vertex {site} has no active site id")],
                    }));
                };
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

    /// Move two neighbouring sites as one transaction.
    ///
    /// Some Delaunay edge changes need both ends to move before the degree
    /// objective improves. Two ordinary move transactions cannot cross that
    /// saddle because the first one is correctly rejected as unimproving.
    pub(crate) fn propose_pair_move_cached<Score: PartialOrd>(
        &mut self,
        first: (usize, CartesianPoint),
        second: (usize, CartesianPoint),
        gates: HardGates,
        objective: &dyn Fn(&MeshState, &AffectedSites) -> Option<Score>,
        cached_before: Option<&Score>,
    ) -> Result<Acceptance> {
        if first.0 == second.0 {
            return Ok(Acceptance::RolledBack(Rejection::NoImprovement {
                site: first.0,
            }));
        }
        let (first_fan, first_reach) = match self.move_neighbourhood(first.0) {
            Ok(neighbourhood) => neighbourhood,
            Err(error) => return Ok(Acceptance::RolledBack(Rejection::Unmeasurable(error))),
        };
        let (second_fan, second_reach) = match self.move_neighbourhood(second.0) {
            Ok(neighbourhood) => neighbourhood,
            Err(error) => return Ok(Acceptance::RolledBack(Rejection::Unmeasurable(error))),
        };
        let fan: BTreeSet<usize> = first_fan.union(&second_fan).copied().collect();
        let reach: BTreeSet<usize> = first_reach.union(&second_reach).copied().collect();
        let patch = self.state().snapshot_around(&reach);
        let patch_size = patch.triangles().count();
        if gates.max_patch_triangles > 0 && patch_size > gates.max_patch_triangles {
            return Ok(Acceptance::RolledBack(Rejection::PatchTooLarge {
                triangles: patch_size,
                allowed: gates.max_patch_triangles,
            }));
        }
        let affected_sites = self.state().sites_touching(&reach);
        let computed_before = cached_before
            .is_none()
            .then(|| objective(self.state(), &affected_sites))
            .flatten();
        let before = cached_before.or(computed_before.as_ref());
        let first_origin = self.state().vertices()[first.0];
        let second_origin = self.state().vertices()[second.0];

        self.state_mut().move_vertex(first.0, first.1);
        self.state_mut().move_vertex(second.0, second.1);
        if let Err(error) = self.state_mut().legalize_within(&fan, Some(&reach)) {
            self.state_mut().move_vertex(first.0, first_origin);
            self.state_mut().move_vertex(second.0, second_origin);
            self.state_mut()
                .restore_patch(patch)
                .map_err(|error| HarpDvError::TopologyViolation(error.to_string()))?;
            return Ok(Acceptance::RolledBack(Rejection::CouldNotLegalize(
                error.to_string(),
            )));
        }

        // Same reason as the single-site path: legalising can rewrite the slot
        // that named a site, so the seeds have to be taken again from the
        // moved mesh and the site set has to be the one the before-score used.
        let affected_sites = {
            let after = self.state().sites_touching(&reach);
            if let Some(&missing) = affected_sites.keys().find(|site| !after.contains_key(site)) {
                self.state_mut().move_vertex(first.0, first_origin);
                self.state_mut().move_vertex(second.0, second_origin);
                self.state_mut()
                    .restore_patch(patch)
                    .map_err(|error| HarpDvError::TopologyViolation(error.to_string()))?;
                return Ok(Acceptance::RolledBack(Rejection::Unmeasurable(
                    VoronoiError::SiteIsInNoTriangle { site: missing },
                )));
            }
            after
        };
        let pentagons = self.pentagon_ids();
        let verdict = match check(self.state(), &reach, gates, &pentagons) {
            Ok(max_degree_touched) => {
                let improved = match (
                    before.as_ref(),
                    objective(self.state(), &affected_sites).as_ref(),
                ) {
                    (Some(before), Some(after)) => {
                        after.partial_cmp(before) == Some(std::cmp::Ordering::Less)
                    }
                    _ => false,
                };
                improved
                    .then_some(max_degree_touched)
                    .ok_or(Rejection::NoImprovement { site: first.0 })
            }
            Err(rejection) => Err(rejection),
        };
        match verdict {
            Ok(max_degree_touched) => {
                let Some(site_id) = self.record_moved_sites(&[first.0, second.0]) else {
                    self.state_mut().move_vertex(first.0, first_origin);
                    self.state_mut().move_vertex(second.0, second_origin);
                    self.state_mut()
                        .restore_patch(patch)
                        .map_err(|error| HarpDvError::TopologyViolation(error.to_string()))?;
                    return Ok(Acceptance::RolledBack(Rejection::TopologyInvalid {
                        faults: vec![format!(
                            "one of moved vertices [{}, {}] has no active site id",
                            first.0, second.0
                        )],
                    }));
                };
                Ok(Acceptance::Committed(TransactionReport {
                    site_id,
                    vertex: first.0,
                    triangles_removed: 0,
                    triangles_created: 0,
                    max_degree_touched,
                }))
            }
            Err(rejection) => {
                self.state_mut().move_vertex(first.0, first_origin);
                self.state_mut().move_vertex(second.0, second_origin);
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
        self.refine_cell_with_ladder(site, ladder, gates)
    }

    /// Broaden the search for a demand after the ordinary ladder failed for a
    /// whole cycle. Productive cycles never call this path.
    pub(crate) fn refine_cell_fallback(
        &mut self,
        site: usize,
        policy: CandidatePolicy,
        gates: HardGates,
    ) -> Result<DemandOutcome> {
        let ladder = match fallback_candidates_for_site(self.state(), site, policy) {
            Ok(ladder) => ladder,
            Err(error) => return Ok(DemandOutcome::NotAttempted(error)),
        };
        self.refine_cell_with_ladder(site, ladder, gates)
    }

    fn refine_cell_with_ladder(
        &mut self,
        site: usize,
        mut ladder: Vec<Candidate>,
        gates: HardGates,
    ) -> Result<DemandOutcome> {
        // Order the ladder by what each candidate would do to the degrees
        // around it, before anything is written. The forecast accounts for the
        // new neighbour and for cavity-internal edges that disappear (notably
        // an on-edge split), so a candidate that would push one past the budget
        // is knowable in advance -- and the degree bound is 96% of everything
        // this backend cannot do (guide 11.13).
        //
        // Sorting rather than filtering: the ladder's own order still decides
        // among candidates that are equally safe, so a witness still leads
        // when nothing separates them on degree. A stable sort is what keeps
        // that true.
        let mut forecasts: Vec<usize> = Vec::with_capacity(ladder.len());
        for candidate in &ladder {
            let worst = self
                .state()
                .forecast_degrees(candidate.point, Some(candidate.hint))
                .map(|forecast| forecast.worst_neighbour.max(forecast.new_site))
                .unwrap_or(usize::MAX);
            forecasts.push(worst);
        }
        let mut order: Vec<usize> = (0..ladder.len()).collect();
        order.sort_by_key(|&index| {
            // Anything within budget is equally acceptable; only the ones that
            // would break it are pushed back, worst last.
            forecasts[index].max(gates.max_vertex_degree)
        });
        ladder = order.into_iter().map(|index| ladder[index]).collect();

        let mut refusals = Vec::with_capacity(ladder.len());
        for candidate in ladder {
            // Ruppert's rule: split an encroached protected segment instead of
            // inserting inside its diametral circle.
            let encroached = self.encroachment_of(&candidate);
            let candidate = match &encroached {
                Some(split) => Candidate {
                    point: split.split_at,
                    ..candidate
                },
                None => candidate,
            };
            match self.propose_site_for(candidate.point, Some(candidate.hint), gates, site)? {
                Acceptance::Committed(report) => {
                    // Keep both halves protected, or the rule stops after one split.
                    if let Some(split) = encroached {
                        self.split_segment(split.tail, split.head, report.vertex);
                    }
                    return Ok(DemandOutcome::Resolved {
                        source: candidate.source,
                        report,
                    });
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
