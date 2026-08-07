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
