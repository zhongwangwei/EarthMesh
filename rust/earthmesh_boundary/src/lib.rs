//! A boundary as topology, not as a list of points.
//!
//! A coastline read afresh each pass is a sequence of coordinates, and nothing
//! in it says which side is water, which loop is a lake inside an island, or
//! that this segment may be split but never crossed. Those are the facts a
//! refinement has to preserve, so they live here as a model the run holds onto
//! rather than as something rediscovered from geometry every time.
//!
//! Backend neutral by construction: no criterion, no data source, no refinement
//! policy. What a backend *does* about a boundary is the backend's business;
//! what the boundary *is* belongs here.
//!
//! # Scope
//!
//! The types and their invariants. The adaptation策略 -- encroachment, segment
//! splitting, sliding, narrow-feature policy -- belong to whichever backend is
//! doing the adapting.

pub mod rings;
pub mod segments;
pub use rings::{closed_rings, RingError};
pub use segments::SegmentList;

use std::collections::BTreeMap;

/// What a boundary means for the mesh that meets it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BoundaryRole {
    /// A coastline, a basin outline. No cell may cross it.
    HardDomain,
    /// Land against sea, one material against another. Cells live on both
    /// sides; the curve is where they meet.
    MaterialInterface,
    /// A river, a levee, a fault. Has to appear in the mesh as edges.
    EmbeddedFeature,
    /// A regional ocean's open edge. Marker and ordering are part of the
    /// output, so both have to survive refinement.
    OpenBoundary,
    /// A storm track, a named corridor. Creates demand and constrains nothing.
    RefinementGuide,
    /// The two sides of a periodic domain, which must stay in correspondence.
    PeriodicSeam,
}

impl BoundaryRole {
    /// Whether the mesh is forbidden to cross this curve.
    pub fn is_impassable(self) -> bool {
        matches!(self, Self::HardDomain | Self::PeriodicSeam)
    }

    /// Whether an edge on this curve may be removed by an ordinary flip.
    ///
    /// Only a guide may: it constrains nothing, so nothing is lost by
    /// reshaping across it.
    pub fn permits_edge_flip(self) -> bool {
        matches!(self, Self::RefinementGuide)
    }
}

/// Which side of the domain a loop encloses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LoopType {
    /// The outside of a region.
    Outer,
    /// A lake, or the sea inside an atoll. Held as its own loop rather than
    /// joined to the outer one by a cut, because a cut is a lie about the
    /// topology that later passes cannot tell from a real edge.
    Hole,
}

/// A point on the boundary, and how free it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoundaryVertex {
    pub lon_degrees: f64,
    pub lat_degrees: f64,
    /// A corner or a junction of curves. Refinement may not move it.
    pub pinned: bool,
}

/// One closed ring of boundary vertices.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundaryLoop {
    pub loop_type: LoopType,
    pub role: BoundaryRole,
    /// Vertex indices in order. The ring closes implicitly; the first index is
    /// not repeated at the end.
    pub vertices: Vec<usize>,
    /// Which outer loop this hole sits in. `None` for an outer loop.
    pub parent: Option<usize>,
}

/// Every boundary the run has to respect, as one model.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SphericalBoundaryModel {
    pub vertices: Vec<BoundaryVertex>,
    pub loops: Vec<BoundaryLoop>,
}

/// What is wrong with a boundary model, said precisely enough to fix.
#[derive(Clone, Debug, PartialEq)]
pub enum BoundaryError {
    /// A loop names a vertex that is not in the model.
    UnknownVertex { loop_index: usize, vertex: usize },
    /// A ring of fewer than three vertices does not enclose anything.
    DegenerateLoop { loop_index: usize, vertices: usize },
    /// A hole with no outer loop to be inside of.
    OrphanHole { loop_index: usize },
    /// A hole whose parent is not an outer loop.
    HoleInsideHole { loop_index: usize, parent: usize },
    /// An outer loop that names a parent.
    OuterLoopWithParent { loop_index: usize },
    /// The same vertex twice in one ring, which makes it pinch rather than
    /// close.
    RepeatedVertex { loop_index: usize, vertex: usize },
}

impl std::fmt::Display for BoundaryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownVertex { loop_index, vertex } => write!(
                formatter,
                "loop {loop_index} names vertex {vertex}, which the model does not carry"
            ),
            Self::DegenerateLoop {
                loop_index,
                vertices,
            } => write!(
                formatter,
                "loop {loop_index} has {vertices} vertices; a ring needs at least three"
            ),
            Self::OrphanHole { loop_index } => write!(
                formatter,
                "loop {loop_index} is a hole with no outer loop to sit in"
            ),
            Self::HoleInsideHole { loop_index, parent } => write!(
                formatter,
                "loop {loop_index} is a hole whose parent {parent} is itself a hole"
            ),
            Self::OuterLoopWithParent { loop_index } => {
                write!(formatter, "loop {loop_index} is outer and names a parent")
            }
            Self::RepeatedVertex { loop_index, vertex } => write!(
                formatter,
                "loop {loop_index} visits vertex {vertex} twice, so it pinches rather than closes"
            ),
        }
    }
}

impl std::error::Error for BoundaryError {}

impl SphericalBoundaryModel {
    /// Check every invariant the rest of the system is entitled to assume.
    ///
    /// Run once when the model is built. A boundary that is wrong here is wrong
    /// in a way that surfaces much later as a mesh with a hole nobody asked for.
    pub fn validate(&self) -> Result<(), Vec<BoundaryError>> {
        let mut errors = Vec::new();
        for (loop_index, ring) in self.loops.iter().enumerate() {
            if ring.vertices.len() < 3 {
                errors.push(BoundaryError::DegenerateLoop {
                    loop_index,
                    vertices: ring.vertices.len(),
                });
            }
            let mut seen = BTreeMap::new();
            for &vertex in &ring.vertices {
                if vertex >= self.vertices.len() {
                    errors.push(BoundaryError::UnknownVertex { loop_index, vertex });
                    continue;
                }
                if seen.insert(vertex, ()).is_some() {
                    errors.push(BoundaryError::RepeatedVertex { loop_index, vertex });
                }
            }
            match (ring.loop_type, ring.parent) {
                (LoopType::Hole, None) => errors.push(BoundaryError::OrphanHole { loop_index }),
                (LoopType::Hole, Some(parent)) => {
                    match self.loops.get(parent).map(|outer| outer.loop_type) {
                        Some(LoopType::Outer) => {}
                        Some(LoopType::Hole) => {
                            errors.push(BoundaryError::HoleInsideHole { loop_index, parent })
                        }
                        None => errors.push(BoundaryError::UnknownVertex {
                            loop_index,
                            vertex: parent,
                        }),
                    }
                }
                (LoopType::Outer, Some(_)) => {
                    errors.push(BoundaryError::OuterLoopWithParent { loop_index })
                }
                (LoopType::Outer, None) => {}
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Whether a point is inside the domain this model describes.
    ///
    /// Inside an outer loop and not inside any of its holes. A lake in an
    /// island is outside; the sea around the island is outside; the land
    /// between them is inside. Getting that right is the whole reason holes are
    /// their own loops rather than joined to the outer ring by a cut.
    ///
    /// # Why this is here and not in a backend
    ///
    /// It answers "what is this boundary", which is this crate's subject. What
    /// a backend *does* with the answer -- refine inside it, refuse to cross
    /// it, split a segment that encroaches -- stays with the backend.
    ///
    /// # On the sphere, "inside" is a choice, and the ring's direction makes it
    ///
    /// A closed curve on a plane has an inside and an outside. On a sphere it
    /// has two sides and neither is smaller by nature -- the winding sum is
    /// `+2*pi` on one side and `-2*pi` on the other, so its *magnitude* calls
    /// both of them enclosed. Testing `abs(turn) > pi` therefore reports the
    /// far side of the globe as inside, which is what the dateline test caught.
    ///
    /// So the sign is what decides, and the convention is:
    ///
    /// **every ring runs counter-clockwise seen from outside the sphere, and
    /// the region it encloses is the one on its left.** An outer ring's left is
    /// the domain; a hole's left is the void inside it -- the lake, not the
    /// island around it. A ring given the other way round describes the
    /// complementary region, and does so deliberately rather than by accident.
    ///
    /// Winding is summed on the sphere rather than cast as a ray in longitude,
    /// so the dateline and the poles need no special case: a ring spanning 170
    /// east to 170 west is a twenty-degree strip, not almost the whole globe.
    pub fn contains(&self, lon_degrees: f64, lat_degrees: f64) -> bool {
        let mut inside = false;
        for (index, ring) in self.loops.iter().enumerate() {
            if ring.loop_type != LoopType::Outer
                || !self.loop_winds_around(ring, lon_degrees, lat_degrees)
            {
                continue;
            }
            let in_a_hole = self.loops.iter().any(|hole| {
                hole.loop_type == LoopType::Hole
                    && hole.parent == Some(index)
                    && self.loop_winds_around(hole, lon_degrees, lat_degrees)
            });
            if !in_a_hole {
                inside = true;
                break;
            }
        }
        inside
    }

    /// Whether this ring encloses the point, by spherical winding.
    fn loop_winds_around(&self, ring: &BoundaryLoop, lon_degrees: f64, lat_degrees: f64) -> bool {
        let to_unit = |lon: f64, lat: f64| {
            let (lon, lat) = (lon.to_radians(), lat.to_radians());
            [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
        };
        let here = to_unit(lon_degrees, lat_degrees);
        // Each edge is projected into the plane tangent at the test point, and
        // the angles it turns through there are summed. A ring that encloses
        // the point turns a full circle; one that does not returns to zero.
        let tangent = |point: [f64; 3]| -> Option<(f64, f64)> {
            let dot = here[0] * point[0] + here[1] * point[1] + here[2] * point[2];
            let flat = [
                point[0] - here[0] * dot,
                point[1] - here[1] * dot,
                point[2] - here[2] * dot,
            ];
            let length = (flat[0] * flat[0] + flat[1] * flat[1] + flat[2] * flat[2]).sqrt();
            if length <= 1.0e-12 {
                // The test point sits on this vertex. Counting it as enclosed
                // is the choice that keeps a boundary vertex inside its own
                // domain rather than in neither.
                return None;
            }
            // Any fixed basis in the tangent plane does; the sum of turns is
            // independent of which.
            let east = [-here[1], here[0], 0.0];
            let east_length = (east[0] * east[0] + east[1] * east[1]).sqrt();
            let east = if east_length > 1.0e-12 {
                [east[0] / east_length, east[1] / east_length, 0.0]
            } else {
                // At a pole, east is undefined; any perpendicular will do.
                [1.0, 0.0, 0.0]
            };
            let north = [
                here[1] * east[2] - here[2] * east[1],
                here[2] * east[0] - here[0] * east[2],
                here[0] * east[1] - here[1] * east[0],
            ];
            Some((
                (flat[0] * east[0] + flat[1] * east[1] + flat[2] * east[2]) / length,
                (flat[0] * north[0] + flat[1] * north[1] + flat[2] * north[2]) / length,
            ))
        };

        let mut turned = 0.0_f64;
        let count = ring.vertices.len();
        for step in 0..count {
            let Some(&from) = ring.vertices.get(step) else {
                return false;
            };
            let Some(&to) = ring.vertices.get((step + 1) % count) else {
                return false;
            };
            let (Some(a), Some(b)) = (self.vertices.get(from), self.vertices.get(to)) else {
                return false;
            };
            let (Some(a), Some(b)) = (
                tangent(to_unit(a.lon_degrees, a.lat_degrees)),
                tangent(to_unit(b.lon_degrees, b.lat_degrees)),
            ) else {
                return true;
            };
            let cross = a.0 * b.1 - a.1 * b.0;
            let dot = a.0 * b.0 + a.1 * b.1;
            turned += cross.atan2(dot);
        }
        // Signed, not absolute: see the convention on `contains`. The far side
        // of the ring sums to the negative of this and must not count.
        turned > std::f64::consts::PI
    }

    /// Every boundary edge, as ordered pairs of vertex indices.
    ///
    /// Rings close implicitly, so the last pair joins the final vertex to the
    /// first. A backend that has to place these on a mesh -- as Ruppert's
    /// segments, as edges no flip may remove -- starts here.
    pub fn segments(&self) -> Vec<(usize, usize)> {
        let mut segments = Vec::new();
        for ring in &self.loops {
            let count = ring.vertices.len();
            for step in 0..count {
                let (Some(&from), Some(&to)) = (
                    ring.vertices.get(step),
                    ring.vertices.get((step + 1) % count),
                ) else {
                    continue;
                };
                segments.push((from, to));
            }
        }
        segments
    }

    /// Outer loops and holes, counted.
    ///
    /// The pair a refinement has to leave unchanged. A run that ends with a
    /// different count has removed an island or closed a channel, whatever else
    /// it reports.
    pub fn topology_counts(&self) -> (usize, usize) {
        let holes = self
            .loops
            .iter()
            .filter(|ring| ring.loop_type == LoopType::Hole)
            .count();
        (self.loops.len() - holes, holes)
    }
}

#[cfg(test)]
mod tests;
