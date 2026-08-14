//! Putting one site into a spherical Delaunay triangulation, locally.
//!
//! Bowyer and Watson: find the triangles whose circumcircles contain the new
//! point, remove them, and fan the hole they leave back to the point. What
//! comes out is Delaunay, because the removed set is exactly the set that
//! violated the criterion and the fan cannot create a new one.
//!
//! # No separate legalization pass
//!
//! The specification asks for `legalize_spherical_delaunay_edges` beside the
//! insertion. That belongs to the other construction, Lawson's, which inserts
//! into one triangle and flips outward until the criterion holds. Cavity
//! insertion reaches the same triangulation by construction, so a legalization
//! pass after it would have nothing to do. Two answers to one question, and the
//! second only ever confirming the first, is not worth the code.
//!
//! # Locality
//!
//! Everything outside the cavity keeps its id and its adjacency. The cavity's
//! own slots are reused by the new triangles, so ids stay stable across the
//! whole mesh apart from the handful this touched.

use std::collections::BTreeSet;

use crate::coordinates::magnitude;
use crate::mesh_patch::PatchError;
use crate::mesh_predicates::{in_circle_on_sphere, orientation_on_sphere, Ambiguous, Sign};
#[cfg(test)]
use crate::mesh_state::MESH_STATE_FIRST_ID;
use crate::mesh_state::{FaceId, MeshState, MeshStateError, VertexId};
use crate::mesh_voronoi::VoronoiError;
use crate::CartesianPoint;

/// Why a site could not be placed.
#[derive(Clone, Debug, PartialEq)]
pub enum InsertionError {
    /// A predicate could not decide, so the walk or the cavity has no answer.
    Ambiguous(Ambiguous),
    /// The walk crossed more triangles than the mesh has, or ran off an edge
    /// with the point still beyond it.
    LocationWalkDidNotSettle { visited: usize },
    /// A ring site's degree could not be measured while forecasting an insertion.
    DegreeUnavailable { site: usize, source: VoronoiError },
    /// The cavity bookkeeping cannot produce a valid post-insertion degree.
    InvalidForecastDegree {
        site: usize,
        degree: usize,
        lost_edges: usize,
    },
    /// The point is already a site.
    Duplicate { existing: usize },
    /// The candidate is not on the sphere the mesh lives on.
    ///
    /// Worth its own variant because of how it fails otherwise. A unit vector
    /// offered to a mesh in metres sits, as far as the predicates are
    /// concerned, at the centre of the sphere: the insertion still closes, the
    /// winding still comes out consistent, Euler still holds, and the result is
    /// silently not Delaunay.
    OffSphere {
        candidate_radius: f64,
        mesh_radius: f64,
    },
    /// The cavity reached the whole mesh, leaving no outside to fan to.
    CavitySwallowedTheMesh { triangles: usize },
    /// The cavity boundary is not one ring, so the fan would not close.
    CavityIsNotADisk {
        triangles: usize,
        boundary_edges: usize,
    },
    /// The requested split edge is not an open edge of the insertion cavity.
    BoundaryEdgeNotOpen { tail: usize, head: usize },
}

impl std::fmt::Display for InsertionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ambiguous(ambiguous) => write!(formatter, "{ambiguous}"),
            Self::LocationWalkDidNotSettle { visited } => write!(
                formatter,
                "the point location walk visited {visited} triangles without settling"
            ),
            Self::DegreeUnavailable { site, source } => {
                write!(
                    formatter,
                    "cannot forecast the degree of site {site}: {source}"
                )
            }
            Self::InvalidForecastDegree {
                site,
                degree,
                lost_edges,
            } => write!(
                formatter,
                "site {site} has degree {degree}, but the insertion forecast would remove \
                 {lost_edges} incident cavity edge(s) after adding one neighbour"
            ),
            Self::Duplicate { existing } => {
                write!(formatter, "a site is already at this position: {existing}")
            }
            Self::OffSphere {
                candidate_radius,
                mesh_radius,
            } => write!(
                formatter,
                "the candidate is at radius {candidate_radius:.3} and the mesh is at \
                 {mesh_radius:.3}; a point off the mesh's sphere inserts without complaint and \
                 leaves a triangulation that is not Delaunay"
            ),
            Self::CavitySwallowedTheMesh { triangles } => write!(
                formatter,
                "the cavity took all {triangles} triangles, leaving nothing to attach to"
            ),
            Self::CavityIsNotADisk {
                triangles,
                boundary_edges,
            } => write!(
                formatter,
                "a cavity of {triangles} triangles left {boundary_edges} boundary edges; a disk \
                 leaves two more than it holds"
            ),
            Self::BoundaryEdgeNotOpen { tail, head } => write!(
                formatter,
                "edge ({tail}, {head}) is not an open boundary edge of the insertion cavity"
            ),
        }
    }
}

impl std::error::Error for InsertionError {}

/// Why a transactionized insertion did not commit.
#[derive(Clone, Debug, PartialEq)]
pub enum InsertionTransactionError {
    Insert(InsertionError),
    Topology(Vec<MeshStateError>),
    Rejected,
    Rollback(PatchError),
}

impl std::fmt::Display for InsertionTransactionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Insert(error) => write!(formatter, "{error}"),
            Self::Topology(errors) => {
                write!(formatter, "insertion left invalid topology: {errors:?}")
            }
            Self::Rejected => write!(formatter, "insertion rejected by postcondition"),
            Self::Rollback(error) => write!(
                formatter,
                "rollback failed after insertion rejection: {error}"
            ),
        }
    }
}

impl std::error::Error for InsertionTransactionError {}

impl From<Ambiguous> for InsertionError {
    fn from(ambiguous: Ambiguous) -> Self {
        Self::Ambiguous(ambiguous)
    }
}

/// What one insertion changed.
#[derive(Clone, Debug, PartialEq)]
pub struct InsertionReport {
    pub site: usize,
    /// Stable id of the new site.
    pub site_id: VertexId,
    /// Triangles the cavity removed. Their slots were reused.
    pub removed: Vec<usize>,
    /// Stable ids of the removed faces. They no longer resolve after commit.
    pub removed_ids: Vec<FaceId>,
    /// Triangles the fan created.
    pub created: Vec<usize>,
    /// Stable ids of the faces occupying the created slots after commit.
    pub created_ids: Vec<FaceId>,
}

impl MeshState {
    /// The triangle containing `point`, by walking from `hint`.
    ///
    /// Bounded by the triangle count, so an adjacency that is not a surface
    /// ends as an error rather than a loop.
    pub fn locate_triangle(
        &self,
        point: CartesianPoint,
        hint: Option<usize>,
    ) -> Result<usize, InsertionError> {
        let triangles = self.triangles();
        let mut current = hint
            .filter(|&triangle| self.is_triangle_live(triangle))
            .or_else(|| self.active_triangle_slots().next())
            .ok_or(InsertionError::LocationWalkDidNotSettle { visited: 0 })?;
        let limit = self.triangle_count() + 1;
        for visited in 0..limit {
            let corners = triangles[current];
            let winding = orientation_on_sphere(
                self.vertices()[corners[0]],
                self.vertices()[corners[1]],
                self.vertices()[corners[2]],
            )?;
            let mut stepped = false;
            for corner in 0..3 {
                let tail = corners[(corner + 1) % 3];
                let head = corners[(corner + 2) % 3];
                let side =
                    orientation_on_sphere(self.vertices()[tail], self.vertices()[head], point)?;
                let outside = matches!(
                    (winding, side),
                    (Sign::Positive, Sign::Negative) | (Sign::Negative, Sign::Positive)
                );
                if outside {
                    let neighbour = self.neighbours()[current][corner];
                    if neighbour == 0 || !self.is_triangle_live(neighbour) {
                        return Err(InsertionError::LocationWalkDidNotSettle { visited });
                    }
                    current = neighbour;
                    stepped = true;
                    break;
                }
            }
            if !stepped {
                return Ok(current);
            }
        }
        Err(InsertionError::LocationWalkDidNotSettle { visited: limit })
    }

    /// Triangles whose circumcircle contains `point`, grown from `seed`.
    ///
    /// Depth first over adjacency rather than a scan: the set is connected,
    /// so a neighbour that passes the test is a wall.
    pub fn delaunay_cavity(
        &self,
        point: CartesianPoint,
        seed: usize,
    ) -> Result<BTreeSet<usize>, InsertionError> {
        let mut cavity = BTreeSet::new();
        let mut queue = vec![seed];
        cavity.insert(seed);
        while let Some(triangle) = queue.pop() {
            for corner in 0..3 {
                let neighbour = self.neighbours()[triangle][corner];
                if neighbour == 0
                    || !self.is_triangle_live(neighbour)
                    || cavity.contains(&neighbour)
                {
                    continue;
                }
                let corners = self.triangles()[neighbour];
                let inside = in_circle_on_sphere(
                    self.vertices()[corners[0]],
                    self.vertices()[corners[1]],
                    self.vertices()[corners[2]],
                    point,
                )?;
                if inside == Sign::Positive {
                    cavity.insert(neighbour);
                    queue.push(neighbour);
                }
            }
        }
        Ok(cavity)
    }
}

/// A protected edge a candidate point encroaches on, and where to split it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Encroachment {
    pub tail: usize,
    pub head: usize,
    /// The midpoint, on the sphere: what Ruppert inserts instead of the point
    /// that encroached.
    pub split_at: CartesianPoint,
}

/// What inserting a point would do to the degrees around it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DegreeForecast {
    /// The degree the new site would have: the size of the cavity ring.
    pub new_site: usize,
    /// The largest degree any existing site would end with.
    pub worst_neighbour: usize,
}

impl MeshState {
    /// A protected edge near `point` whose diametral circle contains it.
    ///
    /// Ruppert's encroachment test, and the precondition his termination proof
    /// needs: a circumcentre that falls inside a protected segment's diametral
    /// circle must not be inserted -- the segment is split at its midpoint
    /// instead. Without this the refinement subdivides without end near a
    /// protected boundary, which is measured in guide 11.25: a 25-degree angle
    /// target ran to the cycle limit and ended at a degenerate circumcentre.
    ///
    /// `protected` says which edges are segments. Only edges of triangles in
    /// `region` are examined, so the cost is the neighbourhood rather than the
    /// mesh.
    pub fn encroached_segment(
        &self,
        point: CartesianPoint,
        region: &BTreeSet<usize>,
        protected: &dyn Fn(usize, usize) -> bool,
    ) -> Option<Encroachment> {
        let edges = region
            .iter()
            .filter(|&&triangle| self.is_triangle_live(triangle))
            .map(|&triangle| self.triangles()[triangle])
            .flat_map(|corners| {
                (0..3).map(move |corner| (corners[(corner + 1) % 3], corners[(corner + 2) % 3]))
            });
        self.encroached_segment_edges(point, edges.filter(|&(a, b)| protected(a, b)))
    }

    /// A protected edge near `point`, scanning an explicit deterministic segment list.
    pub fn encroached_segment_edges(
        &self,
        point: CartesianPoint,
        edges: impl IntoIterator<Item = (usize, usize)>,
    ) -> Option<Encroachment> {
        let mut best: Option<(f64, Encroachment)> = None;
        for (tail, head) in edges {
            let (Some(&a), Some(&b)) = (self.vertices().get(tail), self.vertices().get(head))
            else {
                continue;
            };
            let to_a = CartesianPoint::new(a.x - point.x, a.y - point.y, a.z - point.z);
            let to_b = CartesianPoint::new(b.x - point.x, b.y - point.y, b.z - point.z);
            let dot = to_a.x * to_b.x + to_a.y * to_b.y + to_a.z * to_b.z;
            if dot >= 0.0 {
                continue;
            }
            let midpoint =
                CartesianPoint::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0, (a.z + b.z) / 2.0);
            let length = magnitude(midpoint);
            if length <= 0.0 {
                continue;
            }
            let radius = (magnitude(a) + magnitude(b)) / 2.0;
            let split_at = CartesianPoint::new(
                midpoint.x / length * radius,
                midpoint.y / length * radius,
                midpoint.z / length * radius,
            );
            let severity = -dot;
            if best.as_ref().is_none_or(|(worst, _)| severity > *worst) {
                best = Some((
                    severity,
                    Encroachment {
                        tail,
                        head,
                        split_at,
                    },
                ));
            }
        }
        best.map(|(_, encroachment)| encroachment)
    }

    /// Predict the degrees an insertion would leave, without performing it.
    ///
    /// Cheap and exact rather than a heuristic. Bowyer-Watson fans the cavity
    /// ring to the new point. Every ring site gains the new neighbour and loses
    /// each incident edge internal to the cavity; for an on-edge split this is
    /// what keeps the two edge endpoints at the same degree. The new site's
    /// own degree is the ring size. All are known before anything is written.
    ///
    /// This is what lets a caller *choose* where to insert on degree grounds.
    /// Four attempts at fixing degree after the fact are recorded in guide
    /// sections 11.9 and 11.15, all negative, and they share a cause: degree
    /// distribution belongs to the Delaunay triangulation of the point set, so
    /// the only lever is which points there are.
    pub fn forecast_degrees(
        &self,
        point: CartesianPoint,
        hint: Option<usize>,
    ) -> Result<DegreeForecast, InsertionError> {
        let containing = self.locate_triangle(point, hint)?;
        let cavity = self.delaunay_cavity(point, containing)?;
        let mut ring: BTreeSet<usize> = BTreeSet::new();
        let mut lost_edges: BTreeSet<[usize; 2]> = BTreeSet::new();
        let mut ring_edges = 0usize;
        for &triangle in &cavity {
            let corners = self.triangles()[triangle];
            for corner in 0..3 {
                let neighbour = self.neighbours()[triangle][corner];
                if neighbour != 0 && self.is_triangle_live(neighbour) && cavity.contains(&neighbour)
                {
                    let a = corners[(corner + 1) % 3];
                    let b = corners[(corner + 2) % 3];
                    lost_edges.insert([a.min(b), a.max(b)]);
                    continue;
                }
                ring_edges += 1;
                ring.insert(corners[(corner + 1) % 3]);
                ring.insert(corners[(corner + 2) % 3]);
            }
        }
        let mut worst = 0usize;
        for &site in &ring {
            let degree = self
                .vertex_degree(site)
                .map_err(|source| InsertionError::DegreeUnavailable { site, source })?;
            let lost = lost_edges
                .iter()
                .filter(|edge| edge.contains(&site))
                .count();
            let after = degree
                .checked_add(1)
                .and_then(|degree| degree.checked_sub(lost))
                .ok_or(InsertionError::InvalidForecastDegree {
                    site,
                    degree,
                    lost_edges: lost,
                })?;
            worst = worst.max(after);
        }
        Ok(DegreeForecast {
            new_site: ring_edges,
            worst_neighbour: worst,
        })
    }

    /// Insert a site transactionally: snapshot, insert, validate, then either
    /// commit by keeping the edit or roll back exactly to the snapshot.
    pub fn insert_site_transactionally(
        &mut self,
        point: CartesianPoint,
        postcondition: impl FnOnce(&Self, &InsertionReport) -> bool,
    ) -> Result<InsertionReport, InsertionTransactionError> {
        let containing = self
            .locate_triangle(point, None)
            .map_err(InsertionTransactionError::Insert)?;
        let cavity = self
            .delaunay_cavity(point, containing)
            .map_err(InsertionTransactionError::Insert)?;
        let patch = self.snapshot_around(&cavity);
        let report = match self.insert_site_with_cavity(point, containing, &cavity) {
            Ok(report) => report,
            Err(error) => {
                self.restore_patch(patch)
                    .map_err(InsertionTransactionError::Rollback)?;
                return Err(InsertionTransactionError::Insert(error));
            }
        };
        let mut affected: BTreeSet<_> = patch.triangles().collect();
        affected.extend(report.created.iter().copied());
        if let Err(errors) = self.validate_region(&affected) {
            self.restore_patch(patch)
                .map_err(InsertionTransactionError::Rollback)?;
            return Err(InsertionTransactionError::Topology(errors));
        }
        if !postcondition(self, &report) {
            self.restore_patch(patch)
                .map_err(InsertionTransactionError::Rollback)?;
            return Err(InsertionTransactionError::Rejected);
        }
        Ok(report)
    }

    /// Insert a site on an existing open boundary edge.
    ///
    /// Ordinary cavity insertion would fan the old boundary edge back to the
    /// new midpoint and create a zero-area triangle. Boundary insertion drops
    /// that one ring edge, so the old segment becomes the two new open edges.
    pub fn insert_site_on_boundary_edge_transactionally(
        &mut self,
        point: CartesianPoint,
        tail: usize,
        head: usize,
        postcondition: impl FnOnce(&Self, &InsertionReport) -> bool,
    ) -> Result<InsertionReport, InsertionTransactionError> {
        let containing = self
            .open_edge_triangle(tail, head)
            .ok_or(InsertionError::BoundaryEdgeNotOpen { tail, head })
            .map_err(InsertionTransactionError::Insert)?;
        self.insert_site_on_boundary_edge_from_transactionally(
            point,
            containing,
            tail,
            head,
            postcondition,
        )
    }

    /// Insert on an open boundary edge when its incident triangle is already known.
    ///
    /// This is the same transaction as
    /// [`insert_site_on_boundary_edge_transactionally`](Self::insert_site_on_boundary_edge_transactionally),
    /// without repeating a whole-mesh lookup performed by the caller.
    pub fn insert_site_on_boundary_edge_from_transactionally(
        &mut self,
        point: CartesianPoint,
        containing: usize,
        tail: usize,
        head: usize,
        postcondition: impl FnOnce(&Self, &InsertionReport) -> bool,
    ) -> Result<InsertionReport, InsertionTransactionError> {
        let key = (tail.min(head), tail.max(head));
        let edge_is_open = self
            .triangles()
            .get(containing)
            .zip(self.neighbours().get(containing))
            .is_some_and(|(corners, neighbours)| {
                (0..3).any(|corner| {
                    let a = corners[(corner + 1) % 3];
                    let b = corners[(corner + 2) % 3];
                    (a.min(b), a.max(b)) == key && neighbours[corner] == 0
                })
            });
        if !edge_is_open {
            return Err(InsertionTransactionError::Insert(
                InsertionError::BoundaryEdgeNotOpen { tail, head },
            ));
        }
        let cavity = self
            .delaunay_cavity(point, containing)
            .map_err(InsertionTransactionError::Insert)?;
        let patch = self.snapshot_around(&cavity);
        let report = match self
            .insert_site_on_boundary_edge_with_cavity(point, containing, &cavity, tail, head)
        {
            Ok(report) => report,
            Err(error) => {
                self.restore_patch(patch)
                    .map_err(InsertionTransactionError::Rollback)?;
                return Err(InsertionTransactionError::Insert(error));
            }
        };
        let mut affected: BTreeSet<_> = patch.triangles().collect();
        affected.extend(report.created.iter().copied());
        if let Err(errors) = self.validate_region(&affected) {
            self.restore_patch(patch)
                .map_err(InsertionTransactionError::Rollback)?;
            return Err(InsertionTransactionError::Topology(errors));
        }
        if !postcondition(self, &report) {
            self.restore_patch(patch)
                .map_err(InsertionTransactionError::Rollback)?;
            return Err(InsertionTransactionError::Rejected);
        }
        Ok(report)
    }

    fn open_edge_triangle(&self, tail: usize, head: usize) -> Option<usize> {
        let key = (tail.min(head), tail.max(head));
        for triangle in self.active_triangle_slots() {
            let corners = self.triangles()[triangle];
            for corner in 0..3 {
                let a = corners[(corner + 1) % 3];
                let b = corners[(corner + 2) % 3];
                if (a.min(b), a.max(b)) == key && self.neighbours()[triangle][corner] == 0 {
                    return Some(triangle);
                }
            }
        }
        None
    }

    /// Insert a site, leaving everything outside the cavity as it was.
    pub fn insert_site(
        &mut self,
        point: CartesianPoint,
    ) -> Result<InsertionReport, InsertionError> {
        let containing = self.locate_triangle(point, None)?;
        let cavity = self.delaunay_cavity(point, containing)?;
        self.insert_site_with_cavity(point, containing, &cavity)
    }

    /// Insert into a cavity the caller already carved.
    ///
    /// Public because a caller that has to be able to *undo* the insertion
    /// cannot use [`insert_site`](Self::insert_site): that one locates the point
    /// again from no hint and carves its own cavity, so a rollback patch taken
    /// around the caller's cavity need not cover what the insertion overwrote.
    /// The two locations agree on a mesh whose triangles do not overlap, and
    /// differ on one whose triangles do -- and nothing here refuses an inverted
    /// triangle, so that is reachable rather than hypothetical. Threading the
    /// cavity through is what keeps the snapshot and the edit talking about the
    /// same triangles.
    ///
    /// `containing` and `cavity` must come from one
    /// [`locate_triangle`](Self::locate_triangle) and
    /// [`delaunay_cavity`](Self::delaunay_cavity) pair over the mesh as it is
    /// now; passing a stale pair is what this exists to prevent.
    pub fn insert_site_with_cavity(
        &mut self,
        point: CartesianPoint,
        containing: usize,
        cavity: &BTreeSet<usize>,
    ) -> Result<InsertionReport, InsertionError> {
        // A point off the sphere is the one bad input that does not announce
        // itself: it locates, it carves a cavity, it closes, and the mesh it
        // leaves is not Delaunay. Loose tolerance, because this is catching a
        // wrong unit rather than policing a projection.
        //
        // Measured against a corner of the triangle the point landed in, not
        // against the mesh's mean radius. Local for the same reason everything
        // else here is -- `sphere_radius` walks every site, which makes one
        // insertion cost the whole mesh -- and nearer the question anyway on a
        // relaxed mesh, whose sites are not all at one radius.
        let mesh_radius = magnitude(self.vertices()[self.triangles()[containing][0]]);
        let candidate_radius = magnitude(point);
        if mesh_radius > 0.0 && ((candidate_radius - mesh_radius).abs() / mesh_radius) > 1.0e-3 {
            return Err(InsertionError::OffSphere {
                candidate_radius,
                mesh_radius,
            });
        }

        for corner in self.triangles()[containing] {
            if self.vertices()[corner] == point {
                return Err(InsertionError::Duplicate { existing: corner });
            }
        }
        if cavity.len() >= self.triangle_count() {
            return Err(InsertionError::CavitySwallowedTheMesh {
                triangles: cavity.len(),
            });
        }

        // The boundary, each edge in the winding of the triangle it came from
        // so the fan inherits the orientation.
        let mut ring = Vec::new();
        for &triangle in cavity {
            let corners = self.triangles()[triangle];
            for corner in 0..3 {
                let neighbour = self.neighbours()[triangle][corner];
                if neighbour != 0 && self.is_triangle_live(neighbour) && cavity.contains(&neighbour)
                {
                    continue;
                }
                ring.push((
                    corners[(corner + 1) % 3],
                    corners[(corner + 2) % 3],
                    neighbour,
                ));
            }
        }
        // A triangulated disk of k triangles has k + 2 boundary edges. Anything
        // else means the cavity wrapped around something and the fan will not
        // close, which is worth saying now rather than leaving as a hole.
        if ring.len() != cavity.len() + 2 {
            return Err(InsertionError::CavityIsNotADisk {
                triangles: cavity.len(),
                boundary_edges: ring.len(),
            });
        }

        let site = self.push_vertex(point);
        let removed: Vec<usize> = cavity.iter().copied().collect();
        let removed_ids = removed
            .iter()
            .map(|&slot| self.face_id(slot).expect("cavity face has a stable id"))
            .collect();
        let mut created = Vec::with_capacity(ring.len());
        let mut spare = removed.iter().copied();
        for &(tail, head, _) in &ring {
            let slot = match spare.next() {
                Some(slot) => {
                    self.set_triangle(slot, [tail, head, site]);
                    slot
                }
                None => self.push_triangle([tail, head, site]),
            };
            created.push(slot);
        }
        // The changed triangles and the ring just outside them. Every edge
        // that moved is incident to one of the new triangles, so its other
        // claimant is either another new one or a triangle on that ring.
        let authoritative: BTreeSet<usize> = created.iter().copied().collect();
        let mut region = authoritative.clone();
        region.extend(
            ring.iter()
                .filter_map(|&(_, _, outside)| (outside != 0).then_some(outside)),
        );
        self.repair_adjacency_across(&region, &authoritative);

        let site_id = self
            .vertex_id(site)
            .expect("the site just inserted has a stable id");
        let created_ids = created
            .iter()
            .map(|&slot| self.face_id(slot).expect("the fan face has a stable id"))
            .collect();

        Ok(InsertionReport {
            site,
            site_id,
            removed,
            removed_ids,
            created,
            created_ids,
        })
    }

    fn insert_site_on_boundary_edge_with_cavity(
        &mut self,
        point: CartesianPoint,
        containing: usize,
        cavity: &BTreeSet<usize>,
        boundary_tail: usize,
        boundary_head: usize,
    ) -> Result<InsertionReport, InsertionError> {
        let mesh_radius = magnitude(self.vertices()[self.triangles()[containing][0]]);
        let candidate_radius = magnitude(point);
        if mesh_radius > 0.0 && ((candidate_radius - mesh_radius).abs() / mesh_radius) > 1.0e-3 {
            return Err(InsertionError::OffSphere {
                candidate_radius,
                mesh_radius,
            });
        }
        for corner in self.triangles()[containing] {
            if self.vertices()[corner] == point {
                return Err(InsertionError::Duplicate { existing: corner });
            }
        }

        let key = (
            boundary_tail.min(boundary_head),
            boundary_tail.max(boundary_head),
        );
        let mut saw_boundary = false;
        let mut ring = Vec::new();
        for &triangle in cavity {
            let corners = self.triangles()[triangle];
            for corner in 0..3 {
                let neighbour = self.neighbours()[triangle][corner];
                if neighbour != 0 && self.is_triangle_live(neighbour) && cavity.contains(&neighbour)
                {
                    continue;
                }
                let tail = corners[(corner + 1) % 3];
                let head = corners[(corner + 2) % 3];
                if (tail.min(head), tail.max(head)) == key {
                    if neighbour != 0 {
                        return Err(InsertionError::BoundaryEdgeNotOpen {
                            tail: boundary_tail,
                            head: boundary_head,
                        });
                    }
                    saw_boundary = true;
                    continue;
                }
                ring.push((tail, head, neighbour));
            }
        }
        if !saw_boundary || ring.len() != cavity.len() + 1 {
            return Err(InsertionError::BoundaryEdgeNotOpen {
                tail: boundary_tail,
                head: boundary_head,
            });
        }

        let site = self.push_vertex(point);
        let removed: Vec<usize> = cavity.iter().copied().collect();
        let removed_ids = removed
            .iter()
            .map(|&slot| self.face_id(slot).expect("cavity face has a stable id"))
            .collect();
        let mut created = Vec::with_capacity(ring.len());
        let mut spare = removed.iter().copied();
        for &(tail, head, _) in &ring {
            let slot = match spare.next() {
                Some(slot) => {
                    self.set_triangle(slot, [tail, head, site]);
                    slot
                }
                None => self.push_triangle([tail, head, site]),
            };
            created.push(slot);
        }
        let authoritative: BTreeSet<usize> = created.iter().copied().collect();
        let mut region = authoritative.clone();
        region.extend(
            ring.iter()
                .filter_map(|&(_, _, outside)| (outside != 0).then_some(outside)),
        );
        self.repair_adjacency_across(&region, &authoritative);

        let site_id = self
            .vertex_id(site)
            .expect("the site just inserted has a stable id");
        let created_ids = created
            .iter()
            .map(|&slot| self.face_id(slot).expect("the fan face has a stable id"))
            .collect();
        Ok(InsertionReport {
            site,
            site_id,
            removed,
            removed_ids,
            created,
            created_ids,
        })
    }
}

#[cfg(test)]
mod tests;
