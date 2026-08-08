//! Turning boundary edges into ordered closed rings.
//!
//! Three places in this repository walk a set of boundary edges into rings: the
//! mask post-process, red-green's refined/unrefined frontier, and Method-C's
//! block perimeter. What each of them means by "boundary" is its own business
//! -- `mrl_new == 4` against `mrl_new == 1`, or `nest_wd.is_subdivided()`, or a
//! coastline read off a file. What they then *do* with those edges is the same
//! walk, under the same invariant, and it belongs in one place.
//!
//! # The invariant is the point
//!
//! A boundary vertex has exactly two boundary neighbours: the ring arrives and
//! leaves. One means the curve stops in mid-air, three means a junction the
//! ring cannot pass through without choosing, and either way the result is not
//! a closed curve. A walker that carries on regardless returns something
//! ring-shaped that is not the boundary, which is the failure this crate exists
//! to prevent -- so the degree is checked rather than assumed, and a violation
//! names the vertex.

use std::collections::BTreeMap;

/// Why a set of boundary edges is not a set of closed rings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RingError {
    /// A vertex the ring can enter but not leave, or leave but not enter.
    NotTwoNeighbours { vertex: usize, neighbours: usize },
    /// Fewer than three vertices, which encloses nothing.
    DegenerateRing { vertices: usize },
    /// The walk left a vertex and could not get back, so the edges do not close.
    RingDoesNotClose { start: usize },
}

impl std::fmt::Display for RingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotTwoNeighbours { vertex, neighbours } => write!(
                formatter,
                "boundary vertex {vertex} has {neighbours} boundary neighbours; a closed ring \
                 arrives once and leaves once"
            ),
            Self::DegenerateRing { vertices } => write!(
                formatter,
                "a ring of {vertices} vertices encloses nothing; three is the fewest that can"
            ),
            Self::RingDoesNotClose { start } => write!(
                formatter,
                "the walk from boundary vertex {start} ran out of edges before returning to it"
            ),
        }
    }
}

impl std::error::Error for RingError {}

/// Walk unordered boundary edges into ordered closed rings.
///
/// `edges` are undirected pairs; each is added to both endpoints. The result is
/// one `Vec` per ring, in traversal order, with the first vertex not repeated
/// at the end -- the same shape [`crate::BoundaryLoop::vertices`] holds.
///
/// # Direction is stable, and means nothing
///
/// A set of undirected edges says which vertices the ring visits and in what
/// cycle. It does not say which way round -- and on a sphere that is not a
/// detail, because [`SphericalBoundaryModel::contains`] reads the direction to
/// decide which side is inside. Assembling a ring therefore cannot produce an
/// oriented loop, and pretending otherwise would give a boundary whose inside
/// depends on the order the caller happened to collect its edges. Measured:
/// the same four edges shuffled walked the opposite way round.
///
/// So each ring starts at its lowest vertex and steps first to the smaller of
/// that vertex's two neighbours. The result is reproducible -- the same edges
/// in any order give the same `Vec` -- and carries no claim about inside.
/// A caller that needs one orients the ring itself, against a point it knows
/// to be interior.
///
/// Rings come back sorted by their lowest vertex id for the same reason.
pub fn closed_rings(edges: &[(usize, usize)]) -> Result<Vec<Vec<usize>>, RingError> {
    let mut neighbours: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for &(from, to) in edges {
        if from == to {
            return Err(RingError::NotTwoNeighbours {
                vertex: from,
                neighbours: 1,
            });
        }
        neighbours.entry(from).or_default().push(to);
        neighbours.entry(to).or_default().push(from);
    }
    // Checked before walking, so a malformed set is reported as what is wrong
    // with it rather than as wherever the walk happened to give up.
    for (&vertex, ends) in &neighbours {
        ends.len()
            .eq(&2)
            .then_some(())
            .ok_or(RingError::NotTwoNeighbours {
                vertex,
                neighbours: ends.len(),
            })?;
    }

    let mut unvisited: BTreeMap<usize, ()> = neighbours.keys().map(|&v| (v, ())).collect();
    let mut rings = Vec::new();
    while let Some((&start, _)) = unvisited.iter().next() {
        let mut ring = vec![start];
        unvisited.remove(&start);
        let mut previous = start;
        // The smaller neighbour, not the first one inserted: see the note on
        // direction above. `unvisited` is a `BTreeMap`, so `start` is already
        // the lowest id left, which makes the pair (start, first step) a
        // property of the edge set rather than of how it was collected.
        let ends = &neighbours[&start];
        let mut current = ends[0].min(ends[1]);
        while current != start {
            if unvisited.remove(&current).is_none() {
                // Reached a vertex another ring already claimed, which the
                // degree check should have made impossible.
                return Err(RingError::RingDoesNotClose { start });
            }
            ring.push(current);
            let ends = &neighbours[&current];
            let next = if ends[0] == previous {
                ends[1]
            } else {
                ends[0]
            };
            previous = current;
            current = next;
        }
        if ring.len() < 3 {
            return Err(RingError::DegenerateRing {
                vertices: ring.len(),
            });
        }
        rings.push(ring);
    }
    rings.sort_by_key(|ring| ring.iter().copied().min().unwrap_or(0));
    Ok(rings)
}

#[cfg(test)]
mod tests;
