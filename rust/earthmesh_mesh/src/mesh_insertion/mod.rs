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
use crate::mesh_predicates::{in_circle_on_sphere, orientation_on_sphere, Ambiguous, Sign};
use crate::mesh_state::{MeshState, MESH_STATE_FIRST_ID};
use crate::CartesianPoint;

/// Why a site could not be placed.
#[derive(Clone, Debug, PartialEq)]
pub enum InsertionError {
    /// A predicate could not decide, so the walk or the cavity has no answer.
    Ambiguous(Ambiguous),
    /// The walk crossed more triangles than the mesh has, or ran off an edge
    /// with the point still beyond it.
    LocationWalkDidNotSettle { visited: usize },
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
}

impl std::fmt::Display for InsertionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ambiguous(ambiguous) => write!(formatter, "{ambiguous}"),
            Self::LocationWalkDidNotSettle { visited } => write!(
                formatter,
                "the point location walk visited {visited} triangles without settling"
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
        }
    }
}

impl std::error::Error for InsertionError {}

impl From<Ambiguous> for InsertionError {
    fn from(ambiguous: Ambiguous) -> Self {
        Self::Ambiguous(ambiguous)
    }
}

/// What one insertion changed.
#[derive(Clone, Debug, PartialEq)]
pub struct InsertionReport {
    pub site: usize,
    /// Triangles the cavity removed. Their slots were reused.
    pub removed: Vec<usize>,
    /// Triangles the fan created.
    pub created: Vec<usize>,
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
            .filter(|&triangle| triangle >= MESH_STATE_FIRST_ID && triangle < triangles.len())
            .unwrap_or(MESH_STATE_FIRST_ID);
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
                    if neighbour == 0 {
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
    /// Breadth first over adjacency rather than a scan: the set is connected,
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
                if neighbour == 0 || cavity.contains(&neighbour) {
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

    /// Insert a site, leaving everything outside the cavity as it was.
    pub fn insert_site(
        &mut self,
        point: CartesianPoint,
    ) -> Result<InsertionReport, InsertionError> {
        let containing = self.locate_triangle(point, None)?;

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
        let cavity = self.delaunay_cavity(point, containing)?;
        if cavity.len() >= self.triangle_count() {
            return Err(InsertionError::CavitySwallowedTheMesh {
                triangles: cavity.len(),
            });
        }

        // The boundary, each edge in the winding of the triangle it came from
        // so the fan inherits the orientation.
        let mut ring = Vec::new();
        for &triangle in &cavity {
            let corners = self.triangles()[triangle];
            for corner in 0..3 {
                let neighbour = self.neighbours()[triangle][corner];
                if neighbour != 0 && cavity.contains(&neighbour) {
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

        Ok(InsertionReport {
            site,
            removed,
            created,
        })
    }
}

#[cfg(test)]
mod tests;
