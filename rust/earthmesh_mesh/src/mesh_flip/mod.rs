//! Turning an edge, and turning enough of them to be Delaunay again.
//!
//! Insertion reaches a Delaunay triangulation by construction and needs no
//! flips. Moving a site does not: the sites around it keep their triangles and
//! those triangles may now violate the criterion, so the mesh has to be walked
//! back to Delaunay. That is Lawson's, and it is why this module exists beside
//! the cavity insertion rather than instead of it.
//!
//! # The quadrilateral
//!
//! An edge is shared by two triangles, which together have four corners: the
//! two on the edge and one apiece off it. Flipping replaces the shared edge
//! with the other diagonal. Nothing else about the mesh changes -- same
//! vertices, same triangle count, same two slots reused -- so ids stay stable,
//! which is what lets a caller hold onto them across a repair.
//!
//! # Termination
//!
//! Lawson's flip decreases a well-defined quantity, so it ends; but a
//! degenerate configuration or a predicate that cannot decide would leave that
//! argument without its premise. So the loop is bounded and reports how many
//! flips it made, and a run that hits the bound is an error rather than a
//! quietly truncated repair.

use std::collections::BTreeSet;

use crate::mesh_predicates::{in_circle_on_sphere, orientation_on_sphere, Ambiguous, Sign};
use crate::mesh_state::MeshState;

/// Why an edge could not be turned.
#[derive(Clone, Debug, PartialEq)]
pub enum FlipError {
    /// A predicate could not decide.
    Ambiguous(Ambiguous),
    /// The edge names no triangle across it, so there is no quadrilateral.
    EdgeIsOnTheBoundary { triangle: usize, corner: usize },
    /// The two triangles do not agree about which edge they share.
    AdjacencyDisagrees { triangle: usize, neighbour: usize },
    /// The quadrilateral is not convex, so the other diagonal lies outside it
    /// and the flip would fold the surface.
    QuadrilateralIsNotConvex { triangle: usize, neighbour: usize },
    /// The repair needed to turn an edge the caller did not snapshot.
    ReachedPastTheRegion { triangle: usize, neighbour: usize },
    /// The repair ran longer than any terminating one could.
    DidNotSettle { flips: usize },
}

impl std::fmt::Display for FlipError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ambiguous(ambiguous) => write!(formatter, "{ambiguous}"),
            Self::EdgeIsOnTheBoundary { triangle, corner } => write!(
                formatter,
                "the edge opposite corner {corner} of triangle {triangle} has nothing across it"
            ),
            Self::AdjacencyDisagrees {
                triangle,
                neighbour,
            } => write!(
                formatter,
                "triangles {triangle} and {neighbour} name each other and share no edge"
            ),
            Self::QuadrilateralIsNotConvex {
                triangle,
                neighbour,
            } => write!(
                formatter,
                "the quadrilateral of triangles {triangle} and {neighbour} is not convex; the \
                 other diagonal lies outside it"
            ),
            Self::ReachedPastTheRegion {
                triangle,
                neighbour,
            } => write!(
                formatter,
                "the repair needed to turn the edge between triangles {triangle} and {neighbour}, \
                 which is outside the region the caller can put back"
            ),
            Self::DidNotSettle { flips } => write!(
                formatter,
                "legalization made {flips} flips without settling; Lawson's argument says it \
                 should have"
            ),
        }
    }
}

impl std::error::Error for FlipError {}

impl From<Ambiguous> for FlipError {
    fn from(ambiguous: Ambiguous) -> Self {
        Self::Ambiguous(ambiguous)
    }
}

impl MeshState {
    /// Replace the edge opposite `corner` of `triangle` with the other
    /// diagonal of its quadrilateral.
    ///
    /// Both triangles keep their ids. A caller holding either across the flip
    /// still holds a triangle -- a different one, in the same place.
    pub fn flip_edge(&mut self, triangle: usize, corner: usize) -> Result<(), FlipError> {
        if !self.is_triangle_live(triangle) {
            return Err(FlipError::EdgeIsOnTheBoundary { triangle, corner });
        }
        let neighbour = self.neighbours()[triangle][corner];
        if !self.is_triangle_live(neighbour) {
            return Err(FlipError::EdgeIsOnTheBoundary { triangle, corner });
        }
        let here = self.triangles()[triangle];
        let there = self.triangles()[neighbour];

        // `apex` is this triangle's corner off the shared edge; `opposite` is
        // the other triangle's.
        let apex = here[corner];
        let tail = here[(corner + 1) % 3];
        let head = here[(corner + 2) % 3];
        let Some(opposite) = there.iter().copied().find(|c| *c != tail && *c != head) else {
            return Err(FlipError::AdjacencyDisagrees {
                triangle,
                neighbour,
            });
        };
        if !there.contains(&tail) || !there.contains(&head) {
            return Err(FlipError::AdjacencyDisagrees {
                triangle,
                neighbour,
            });
        }

        // The new diagonal runs apex-opposite, and only lies inside the
        // quadrilateral if it is convex. Checked by winding: apex and opposite
        // must be on opposite sides of the shared edge, and the two new
        // triangles must wind the same way as the old ones.
        let points = |a: usize, b: usize, c: usize| {
            orientation_on_sphere(self.vertices()[a], self.vertices()[b], self.vertices()[c])
        };
        let before = points(apex, tail, head)?;
        let first = points(apex, tail, opposite)?;
        let second = points(apex, opposite, head)?;
        if first != before || second != before {
            return Err(FlipError::QuadrilateralIsNotConvex {
                triangle,
                neighbour,
            });
        }

        self.set_triangle(triangle, [apex, tail, opposite]);
        self.set_triangle(neighbour, [apex, opposite, head]);

        let changed: BTreeSet<usize> = [triangle, neighbour].into_iter().collect();
        let mut region = changed.clone();
        for &face in &changed {
            region.extend(
                self.neighbours()[face]
                    .iter()
                    .copied()
                    .filter(|&other| self.is_triangle_live(other)),
            );
        }
        self.repair_adjacency_across(&region, &changed);
        Ok(())
    }

    /// Whether the edge opposite `corner` of `triangle` violates the Delaunay
    /// criterion: the triangle across it has a corner inside this one's
    /// circumcircle.
    pub fn edge_is_illegal(&self, triangle: usize, corner: usize) -> Result<bool, FlipError> {
        if !self.is_triangle_live(triangle) {
            return Ok(false);
        }
        let neighbour = self.neighbours()[triangle][corner];
        if !self.is_triangle_live(neighbour) {
            return Ok(false);
        }
        let here = self.triangles()[triangle];
        let there = self.triangles()[neighbour];
        let Some(opposite) = there.iter().copied().find(|c| !here.contains(c)) else {
            return Ok(false);
        };
        let inside = in_circle_on_sphere(
            self.vertices()[here[0]],
            self.vertices()[here[1]],
            self.vertices()[here[2]],
            self.vertices()[opposite],
        )?;
        Ok(inside == Sign::Positive)
    }

    /// Flip until every edge around these triangles is legal.
    ///
    /// Returns how many flips it took. The set grows as flips expose new
    /// edges, which is Lawson's: a flip can make a neighbouring edge illegal,
    /// and the repair is not local to where it started.
    ///
    /// Which is why a caller holding a rollback patch wants
    /// [`Self::legalize_within`] instead. This one will reach as far as it
    /// needs to, including past whatever was snapshotted.
    pub fn legalize_around(&mut self, seed: &BTreeSet<usize>) -> Result<usize, FlipError> {
        self.legalize_within(seed, None)
    }

    /// The same, refusing to turn an edge outside `allowed`.
    ///
    /// A repair that reaches past what a caller snapshotted cannot be rolled
    /// back, and a rollback that silently leaves some of the change behind is
    /// worse than a refusal -- the mesh it leaves is neither the old one nor
    /// the new one, and it validates. So the reach is bounded to what the
    /// patch covers and a repair that would exceed it is reported as
    /// `ReachedPastTheRegion` rather than half-applied.
    pub fn legalize_within(
        &mut self,
        seed: &BTreeSet<usize>,
        allowed: Option<&BTreeSet<usize>>,
    ) -> Result<usize, FlipError> {
        // Generous, and a bound rather than a guess: Lawson's argument says
        // this terminates, so hitting the cap means the premise failed and the
        // caller should hear about it rather than get a partial repair.
        let limit = 16 * self.triangle_count() + 64;
        let mut pending: Vec<usize> = seed
            .iter()
            .copied()
            .filter(|&triangle| self.is_triangle_live(triangle))
            .collect();
        let mut flips = 0usize;
        while let Some(triangle) = pending.pop() {
            if !self.is_triangle_live(triangle) {
                continue;
            }
            for corner in 0..3 {
                if !self.edge_is_illegal(triangle, corner)? {
                    continue;
                }
                let neighbour = self.neighbours()[triangle][corner];
                if let Some(allowed) = allowed {
                    if !allowed.contains(&triangle) || !allowed.contains(&neighbour) {
                        return Err(FlipError::ReachedPastTheRegion {
                            triangle,
                            neighbour,
                        });
                    }
                }
                match self.flip_edge(triangle, corner) {
                    Ok(()) => {}
                    // A non-convex quadrilateral cannot be turned, and leaving
                    // it is correct: the criterion it violates is one no flip
                    // available here can fix.
                    Err(FlipError::QuadrilateralIsNotConvex { .. }) => continue,
                    Err(error) => return Err(error),
                }
                flips += 1;
                if flips > limit {
                    return Err(FlipError::DidNotSettle { flips });
                }
                // Both triangles changed shape, so both need re-checking, and
                // so does whatever now lies across their edges.
                for face in [triangle, neighbour] {
                    pending.push(face);
                    pending.extend(
                        self.neighbours()[face]
                            .iter()
                            .copied()
                            .filter(|&other| self.is_triangle_live(other)),
                    );
                }
                break;
            }
        }
        Ok(flips)
    }
}

#[cfg(test)]
mod tests;
