//! Ruppert's segments: the boundary as a list a refinement may split.
//!
//! Guide 11.28 and 11.29 record why this is a list and not a predicate. An
//! approximation that asked "are both endpoints boundary sites?" looked like a
//! segment list and was not one: two sites that merely happen to be adjacent
//! qualify, every split creates more such pairs, and the refinement spends
//! itself splitting segments that were never there. Measured, the unsound
//! version made the mesh *look* better -- 21.09 degrees against 12.29 -- because
//! it stopped the quality refinement from running at all.
//!
//! # The split is the point
//!
//! Ruppert's termination proof runs an induction: a segment that gets split is
//! two segments, so the rule that made the refinement terminate still applies
//! where it was just applied. A list without [`SegmentList::split`] is a list
//! that stops being true the first time the mesh changes.
//!
//! With a real list, the measured behaviour is exactly what the theory says:
//! 20 degrees converges and crosses the bound, 25 degrees diverges, and 30
//! needs Chew's variant (guide 11.29).
//!
//! # Backend neutral
//!
//! Vertices are opaque ids. What they index -- a `MeshState`'s sites, a
//! gridfile's rows -- is the caller's business, and so is what "inside" means.

use std::collections::BTreeMap;

/// The boundary edges a refinement must respect, and may only split.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SegmentList {
    segments: BTreeMap<(usize, usize), usize>,
}

impl SegmentList {
    /// The edges that straddle the domain edge: one end inside, one outside.
    ///
    /// That is what the boundary curve looks like on a mesh that was not built
    /// to follow it -- the discretisation 11.28 asks for. `edges` may repeat a
    /// pair or give it either way round; both are normalised away.
    ///
    /// `inside` decides the side. Passing a predicate that is expensive per
    /// vertex is fine: it is called at most twice per edge, not once per test.
    pub fn from_straddling_edges(
        edges: impl IntoIterator<Item = (usize, usize)>,
        mut inside: impl FnMut(usize) -> bool,
    ) -> Self {
        let mut known: std::collections::BTreeMap<usize, bool> = std::collections::BTreeMap::new();
        let mut segments = BTreeMap::new();
        for (a, b) in edges {
            if a == b {
                continue;
            }
            let a_in = *known.entry(a).or_insert_with(|| inside(a));
            let b_in = *known.entry(b).or_insert_with(|| inside(b));
            if a_in != b_in {
                segments.insert((a.min(b), a.max(b)), 0);
            }
        }
        Self { segments }
    }

    /// Take an explicit list, however the caller found it.
    pub fn from_pairs(pairs: impl IntoIterator<Item = (usize, usize)>) -> Self {
        Self {
            segments: pairs
                .into_iter()
                .filter(|(a, b)| a != b)
                .map(|(a, b)| ((a.min(b), a.max(b)), 0))
                .collect(),
        }
    }

    /// Take explicit marked segments. Splits inherit this marker.
    pub fn from_marked_pairs(pairs: impl IntoIterator<Item = (usize, usize, usize)>) -> Self {
        Self {
            segments: pairs
                .into_iter()
                .filter(|(a, b, _)| a != b)
                .map(|(a, b, marker)| ((a.min(b), a.max(b)), marker))
                .collect(),
        }
    }

    /// Whether this edge is a segment, either way round.
    pub fn contains(&self, a: usize, b: usize) -> bool {
        self.segments.contains_key(&(a.min(b), a.max(b)))
    }

    /// Marker carried by this segment. Unmarked constructors use marker zero.
    pub fn marker(&self, a: usize, b: usize) -> Option<usize> {
        self.segments.get(&(a.min(b), a.max(b))).copied()
    }

    /// Replace a segment with its two halves, once the midpoint exists.
    ///
    /// Returns whether there was a segment to split. A caller that inserts a
    /// midpoint on an edge that is *not* a segment has not changed the
    /// boundary, and saying so lets it tell the two cases apart -- splitting an
    /// edge that was never a segment is how the unsound version multiplied its
    /// own work.
    pub fn split(&mut self, a: usize, b: usize, midpoint: usize) -> bool {
        let key = (a.min(b), a.max(b));
        let Some(marker) = self.segments.remove(&key) else {
            return false;
        };
        if midpoint == a || midpoint == b {
            self.segments.insert(key, marker);
            return false;
        }
        self.segments
            .insert((a.min(midpoint), a.max(midpoint)), marker);
        self.segments
            .insert((b.min(midpoint), b.max(midpoint)), marker);
        true
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    pub fn len(&self) -> usize {
        self.segments.len()
    }

    /// Every segment, in a fixed order.
    pub fn iter(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.segments.keys().copied()
    }
}

impl FromIterator<(usize, usize)> for SegmentList {
    fn from_iter<T: IntoIterator<Item = (usize, usize)>>(pairs: T) -> Self {
        Self::from_pairs(pairs)
    }
}

#[cfg(test)]
mod tests;
