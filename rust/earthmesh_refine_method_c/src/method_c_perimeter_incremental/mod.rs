//! Scoring repair candidates without a whole-mesh trial each.
//!
//! The perimeter repairs try candidates one at a time: add a face (or fill
//! around a point), close the concavities, check the parent level, walk the
//! perimeters, score. Done on a copy of the whole mask, each of those steps is
//! a sweep over every face or point of the mesh, and a repair tries a candidate
//! per point of a perimeter -- quadratic in the selection. A global slope
//! selection at 12 km spent over an hour there before failing.
//!
//! Here the base selection is measured once, and a candidate is evaluated from
//! what it changes:
//!
//! - the concavity closure revisits only points whose fans the change touched,
//!   in the order the whole-mesh sweep would reach them (the fill is not
//!   order-independent, so the order is kept, not just the fixed point);
//! - the parent check reads only the added faces, the base having passed;
//! - the perimeters are the base's, less the loops through a changed point,
//!   plus those loops walked again. The score only reads loop lengths, which do
//!   not depend on where a walk starts -- except at a pinch, where the walk can
//!   take another path; a candidate whose new loops meet one, or whose loop
//!   has no start the local walk can find, has its perimeters walked over the
//!   whole mesh -- from the working flags, without the copy or the sweep.
//!
//! The whole-mesh versions remain as the oracle (`tests`).

use std::collections::{BTreeSet, HashMap};
use std::io;

use super::*;

#[cfg(test)]
thread_local! {
    /// The oracle switch: score every candidate on the whole mesh.
    pub(crate) static WHOLE_MESH_ONLY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether the repairs must score every candidate on the whole mesh.
pub(crate) fn whole_mesh_only() -> bool {
    #[cfg(test)]
    if WHOLE_MESH_ONLY.with(std::cell::Cell::get) {
        return true;
    }
    false
}

/// What a candidate does to the selection, as the repairs score it.
pub(crate) struct TrialScore {
    /// Faces the candidate and its closure turn on.
    pub(crate) added: Vec<usize>,
    /// The face the candidate turned off, if it stayed off.
    pub(crate) removed: Option<usize>,
    pub(crate) remainder: usize,
    pub(crate) length: usize,
    pub(crate) triplets: bool,
}

/// The outcome of one candidate.
pub(crate) enum Trial {
    /// Not a candidate the whole-mesh evaluation would keep: nothing added,
    /// another parent level, or a perimeter that does not walk.
    Rejected,
    Scored(TrialScore),
}

pub(crate) struct IncrementalSelection<'a> {
    mesh: &'a MethodCMesh,
    m_neighbors: &'a [IcosahedronMPointNeighbors],
    selected: Vec<bool>,
    probe: Vec<MethodCNestWd>,
    first_selected: usize,
    /// The base's second selected face: the first once the first is removed.
    second_selected: Option<usize>,
    /// The face the current trial turned off.
    removing: Option<usize>,
    parent_mrlw: usize,
    loops_through: HashMap<usize, Vec<usize>>,
    /// The base's boundary points, in order: the only points a walk can
    /// start from, besides the ones a candidate changes.
    boundary: BTreeSet<usize>,
    loop_length: Vec<usize>,
    remainder: usize,
    length: usize,
    non_triplets: usize,
    /// Whether the base's perimeters walk. A base with a pinch does not, and
    /// every candidate's perimeters are then walked whole.
    base_walks: bool,
}

impl<'a> IncrementalSelection<'a> {
    /// `None` when the base cannot be scored incrementally: empty, or not
    /// closed under the concavity fill. A base whose perimeters do not walk --
    /// one with a pinch, which is what a grower is then asked to mend -- is
    /// taken, its candidates' perimeters walked whole.
    pub(crate) fn new(
        mesh: &'a MethodCMesh,
        selected: &[bool],
        m_neighbors: &'a [IcosahedronMPointNeighbors],
    ) -> io::Result<Option<Self>> {
        let Some(first_selected) = (2..=mesh.nwd).find(|&iw| selected[iw]) else {
            return Ok(None);
        };
        let second_selected = (first_selected + 1..=mesh.nwd).find(|&iw| selected[iw]);
        let mut closed = selected.to_vec();
        mesh.close_method_c_concavities_for_level_with_neighbors(&mut closed, m_neighbors)?;
        if closed != selected {
            return Ok(None);
        }
        let (perimeters, base_walks) =
            match mesh.method_c_perimeters_from_selected_faces(selected, m_neighbors) {
                Ok(perimeters) => (perimeters, true),
                Err(_) => (Vec::new(), false),
            };
        let mut probe = vec![MethodCNestWd::default(); mesh.nwd + 1];
        for iw in 2..=mesh.nwd {
            if selected[iw] {
                probe[iw].iw[2] = 1;
            }
        }
        let mut boundary = BTreeSet::new();
        for im in 2..=mesh.nmd {
            let neighbors = m_neighbors[im];
            let around = neighbors.iw[..neighbors.npoly]
                .iter()
                .filter(|&&iw| selected[iw])
                .count();
            if around > 0 && around < neighbors.npoly {
                boundary.insert(im);
            }
        }
        let mut loops_through: HashMap<usize, Vec<usize>> = HashMap::new();
        let mut loop_length = Vec::with_capacity(perimeters.len());
        for (index, perimeter) in perimeters.iter().enumerate() {
            loop_length.push(perimeter.len());
            for point in perimeter {
                loops_through.entry(point.im).or_default().push(index);
            }
        }
        Ok(Some(Self {
            mesh,
            m_neighbors,
            selected: selected.to_vec(),
            probe,
            first_selected,
            second_selected,
            removing: None,
            parent_mrlw: mesh.w_faces[first_selected].mrlw,
            loops_through,
            boundary,
            remainder: loop_length.iter().map(|length| length % 3).sum(),
            length: loop_length.iter().sum(),
            non_triplets: loop_length
                .iter()
                .filter(|length| !length.is_multiple_of(3))
                .count(),
            loop_length,
            base_walks,
        }))
    }

    /// The base's remainder: the sum over its loops of length mod 3.
    pub(crate) fn remainder(&self) -> usize {
        self.remainder
    }

    /// The base selection with `added` turned on.
    pub(crate) fn with(&self, added: &[usize]) -> Vec<bool> {
        let mut trial = self.selected.clone();
        for &iw in added {
            trial[iw] = true;
        }
        trial
    }

    /// The base selection with `removed` off and `added` on.
    pub(crate) fn with_changes(&self, removed: Option<usize>, added: &[usize]) -> Vec<bool> {
        let mut trial = self.selected.clone();
        if let Some(iw) = removed {
            trial[iw] = false;
        }
        for &iw in added {
            trial[iw] = true;
        }
        trial
    }

    /// Score the base without `face`, closed under the concavity fill -- a
    /// shrink candidate. The closure may put the face back.
    pub(crate) fn trial_without(&mut self, face: usize) -> io::Result<Trial> {
        if !self.selected[face] {
            return Ok(Trial::Rejected);
        }
        self.selected[face] = false;
        self.probe[face].iw[2] = 0;
        self.removing = Some(face);
        let mut added = Vec::new();
        let outcome = self.score(&mut added);
        for &iw in &added {
            self.selected[iw] = false;
            self.probe[iw].iw[2] = 0;
        }
        self.selected[face] = true;
        self.probe[face].iw[2] = 1;
        self.removing = None;
        outcome
    }

    /// Score the base plus a radius-3 fill at `fill_at` (if any) plus `seeds`,
    /// closed under the concavity fill -- what a repair candidate is.
    pub(crate) fn trial(&mut self, fill_at: Option<usize>, seeds: &[usize]) -> io::Result<Trial> {
        let mut added = Vec::new();
        if let Some(im) = fill_at {
            self.fill_rad3(im, &mut added)?;
        }
        for &iw in seeds {
            if !self.selected[iw] {
                self.selected[iw] = true;
                added.push(iw);
            }
        }
        let outcome = self.score(&mut added);
        for &iw in &added {
            self.selected[iw] = false;
            self.probe[iw].iw[2] = 0;
        }
        outcome
    }

    fn score(&mut self, added: &mut Vec<usize>) -> io::Result<Trial> {
        self.close_concavities(added)?;
        // A face the closure put back is no change at all.
        let removed = self.removing.filter(|&iw| !self.selected[iw]);
        if added.is_empty() && self.removing.is_none() {
            return Ok(Trial::Rejected);
        }
        if added
            .iter()
            .any(|&iw| self.mesh.w_faces[iw].mrlw != self.parent_mrlw)
        {
            return Ok(Trial::Rejected);
        }
        for &iw in added.iter() {
            self.probe[iw].iw[2] = 1;
        }
        let removed_face = self.removing.filter(|&iw| !self.selected[iw]);
        if !self.base_walks {
            return self.score_all_perimeters(added, removed_face);
        }

        let mut touched = BTreeSet::new();
        for &iw in added.iter().chain(self.removing.iter()) {
            touched.extend(self.mesh.w_faces[iw].im);
        }
        let mut gone = BTreeSet::new();
        for im in &touched {
            if let Some(loops) = self.loops_through.get(im) {
                gone.extend(loops.iter().copied());
            }
        }
        let (mut remainder, mut length, mut non_triplets) =
            (self.remainder, self.length, self.non_triplets);
        let mut loops = self.loop_length.len() - gone.len();
        for &index in &gone {
            let n = self.loop_length[index];
            remainder -= n % 3;
            length -= n;
            non_triplets -= usize::from(!n.is_multiple_of(3));
        }

        // Walked, as the whole-mesh walk does, only from a point with two
        // selected faces: from another boundary point -- a spike -- the walk
        // enters the loop past its start and revisits a point.
        let mut walked = BTreeSet::new();
        for &point in &touched {
            if walked.contains(&point) || !self.on_boundary(point)? {
                continue;
            }
            // Followed along its loop to a start the whole-mesh walk could use.
            let Some(start) = self.start_along(point)? else {
                return self.score_all_perimeters(added, removed);
            };
            if walked.contains(&start) {
                continue;
            }
            let perimeter =
                match self
                    .mesh
                    .perim_map2_method_c_from(start, &self.probe, self.m_neighbors)
                {
                    Ok(perimeter) => perimeter,
                    Err(_) => return Ok(Trial::Rejected),
                };
            let mut has_start = false;
            for point in &perimeter {
                walked.insert(point.im);
                if self.fans_around(point.im)? >= 2 {
                    return self.score_all_perimeters(added, removed);
                }
                has_start |= point.nwdiv == 2;
            }
            // The whole-mesh walk starts only from a point with two
            // subdivided faces; a loop without one is not among its perimeters.
            if has_start {
                let n = perimeter.len();
                remainder += n % 3;
                length += n;
                non_triplets += usize::from(!n.is_multiple_of(3));
                loops += 1;
            }
        }
        // A changed boundary point no walk reached belongs to a loop whose
        // starts all lie elsewhere; that candidate is the whole mesh's.
        for &im in &touched {
            if !walked.contains(&im) && self.on_boundary(im)? {
                return self.score_all_perimeters(added, removed);
            }
        }
        // The whole-mesh walk refuses a selection with no perimeter at all.
        if loops == 0 {
            return Ok(Trial::Rejected);
        }
        Ok(Trial::Scored(TrialScore {
            added: added.clone(),
            removed,
            remainder,
            length,
            triplets: non_triplets == 0,
        }))
    }

    /// The candidate's perimeters walked over the whole mesh, as the
    /// whole-mesh evaluation walks them, from the working flags: for a loop
    /// whose local walk is not certain to match (a pinch on it, or no start
    /// found along it). The closure and the parent check stay local.
    fn score_all_perimeters(&self, added: &[usize], removed: Option<usize>) -> io::Result<Trial> {
        // `perim_maps2_method_c`, scanning only the points that can start a
        // walk -- the base's boundary and the changed points -- in its order.
        let mut points = self.boundary.clone();
        for &iw in added.iter().chain(self.removing.iter()) {
            points.extend(self.mesh.w_faces[iw].im);
        }
        let mut perimeters: Vec<Vec<MethodCPerimeterPoint>> = Vec::new();
        let mut seen = BTreeSet::new();
        for im in points {
            if im < 2 || seen.contains(&im) || self.selected_around(im)? != 2 {
                continue;
            }
            let Ok(perimeter) =
                self.mesh
                    .perim_map2_method_c_from(im, &self.probe, self.m_neighbors)
            else {
                return Ok(Trial::Rejected);
            };
            seen.extend(perimeter.iter().map(|point| point.im));
            perimeters.push(perimeter);
        }
        if perimeters.is_empty() {
            return Ok(Trial::Rejected);
        }
        Ok(Trial::Scored(TrialScore {
            added: added.to_vec(),
            removed,
            remainder: perimeters.iter().map(|p| p.len() % 3).sum(),
            length: perimeters.iter().map(Vec::len).sum(),
            triplets: perimeters.iter().all(|p| p.len().is_multiple_of(3)),
        }))
    }

    /// The first point with two selected faces reached by stepping along the
    /// perimeter from `point`, `point` itself if it is one.
    fn start_along(&self, point: usize) -> io::Result<Option<usize>> {
        let mut current = point;
        let mut seen = BTreeSet::new();
        loop {
            if self.selected_around(current)? == 2 {
                return Ok(Some(current));
            }
            if !seen.insert(current) {
                return Ok(None);
            }
            let Ok((next, _)) =
                self.mesh
                    .perim_ngr_method_c(current, &self.probe, self.m_neighbors)
            else {
                return Ok(None);
            };
            current = next;
        }
    }

    fn selected_around(&self, im: usize) -> io::Result<usize> {
        let neighbors = self.m_neighbors[im];
        let mut count = 0usize;
        for &iw in &neighbors.iw[..neighbors.npoly] {
            require_method_c_id("Method-C incremental W face", iw, self.mesh.nwd)?;
            count += usize::from(self.selected[iw]);
        }
        Ok(count)
    }

    fn on_boundary(&self, im: usize) -> io::Result<bool> {
        let count = self.selected_around(im)?;
        Ok(count > 0 && count < self.m_neighbors[im].npoly)
    }

    /// Separate fans of selected faces around `im`.
    fn fans_around(&self, im: usize) -> io::Result<usize> {
        let neighbors = self.m_neighbors[im];
        let faces = &neighbors.iw[..neighbors.npoly];
        let mut fan = [usize::MAX; 7];
        let find = |fan: &[usize; 7], mut k: usize| {
            while fan[k] != k {
                k = fan[k];
            }
            k
        };
        for (k, &iw) in faces.iter().enumerate() {
            if self.selected[iw] {
                fan[k] = k;
            }
        }
        for &iu in &neighbors.iu[..neighbors.npoly] {
            require_method_c_id("Method-C incremental U edge", iu, self.mesh.nud)?;
            let edge = self.mesh.u_edges[iu];
            let (iw1, iw2) = (edge.iw[0], edge.iw[1]);
            if !(self.selected[iw1] && self.selected[iw2]) {
                continue;
            }
            let (Some(a), Some(b)) = (
                faces.iter().position(|&iw| iw == iw1),
                faces.iter().position(|&iw| iw == iw2),
            ) else {
                continue;
            };
            let (ra, rb) = (find(&fan, a), find(&fan, b));
            if ra != rb {
                fan[ra] = rb;
            }
        }
        Ok((0..faces.len())
            .filter(|&k| fan[k] != usize::MAX && find(&fan, k) == k)
            .count())
    }

    /// `mark_fill_rad3_faces_with_neighbors` on the working mask, recording
    /// what it turns on.
    fn fill_rad3(&mut self, im: usize, added: &mut Vec<usize>) -> io::Result<bool> {
        // The base's first selected face, or its second when the trial took
        // the first away; a face added before either comes first.
        let base_first = if self.selected[self.first_selected] {
            Some(self.first_selected)
        } else {
            self.second_selected
        };
        let first = added.iter().copied().chain(base_first).min();
        let mask_mrlw = first.map_or(self.parent_mrlw, |iw| self.mesh.w_faces[iw].mrlw);
        let mut changed = false;
        for iw in self
            .mesh
            .method_c_rad3_faces_with_neighbors(im, self.m_neighbors)?
        {
            if self.mesh.w_faces[iw].mrlw != mask_mrlw {
                continue;
            }
            if !self.selected[iw] {
                self.selected[iw] = true;
                added.push(iw);
                changed = true;
            }
        }
        Ok(changed)
    }

    /// `close_method_c_concavities_for_level_with_neighbors`, revisiting only
    /// the points the changes touched, in the whole-mesh sweep's order: a
    /// point after the one being filled is reached in this sweep, one at or
    /// before it in the next.
    fn close_concavities(&mut self, added: &mut Vec<usize>) -> io::Result<()> {
        let mut current = BTreeSet::new();
        for &iw in added.iter().chain(self.removing.iter()) {
            current.extend(self.mesh.w_faces[iw].im);
        }
        let mut next = BTreeSet::new();
        loop {
            while let Some(im) = current.pop_first() {
                if im < 2 || im > self.mesh.nmd {
                    continue;
                }
                let neighbors = self.m_neighbors[im];
                let mut count = 0usize;
                for &iw in &neighbors.iw[..neighbors.npoly] {
                    require_method_c_id("Method-C concavity W face", iw, self.mesh.nwd)?;
                    count += usize::from(self.selected[iw]);
                }
                if count == 0
                    || count == neighbors.npoly
                    || count < neighbors.npoly.saturating_sub(1)
                {
                    continue;
                }
                let before = added.len();
                if self.fill_rad3(im, added)? {
                    for &iw in &added[before..] {
                        for m in self.mesh.w_faces[iw].im {
                            if m > im {
                                current.insert(m);
                            } else {
                                next.insert(m);
                            }
                        }
                    }
                }
            }
            if next.is_empty() {
                return Ok(());
            }
            current = std::mem::take(&mut next);
        }
    }
}

#[cfg(test)]
mod tests;
