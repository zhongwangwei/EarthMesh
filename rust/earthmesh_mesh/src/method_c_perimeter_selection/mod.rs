use std::io;

use super::*;

/// Reusable scratch for the Method-C perimeter walk.
///
/// Holds the `nest_wd` vector the walk reads plus the indices last marked, so
/// resetting costs one pass over the selection rather than over the mesh.
#[derive(Default)]
pub(crate) struct MethodCPerimeterProbe {
    nest_wd: Vec<MethodCNestWd>,
    marked: Vec<usize>,
    m_candidates: Vec<usize>,
}

impl MethodCPerimeterProbe {
    fn reset(&mut self, len: usize) {
        if self.nest_wd.len() == len {
            for &iw in &self.marked {
                self.nest_wd[iw].iw[2] = 0;
            }
        } else {
            self.nest_wd.clear();
            self.nest_wd.resize(len, MethodCNestWd::default());
        }
        self.marked.clear();
        self.m_candidates.clear();
    }

    fn mark(&mut self, iw: usize, corners: [usize; 3]) {
        self.nest_wd[iw].iw[2] = 1;
        self.marked.push(iw);
        self.m_candidates.extend_from_slice(&corners);
    }

}

impl MethodCDelaunayMesh {
    pub(crate) fn method_c_perimeters_from_selected_faces(
        &self,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<Vec<Vec<MethodCPerimeterPoint>>> {
        let mut probe = MethodCPerimeterProbe::default();
        self.method_c_perimeters_from_selected_faces_with_probe(selected, m_neighbors, &mut probe)
    }

    /// Perimeter walk over a caller-owned probe buffer.
    ///
    /// The plain entry point allocates and zeroes a `nwd`-sized
    /// `MethodCNestWd` vector every call — 57 MB at NXP=243 — to describe a
    /// selection of a few hundred faces. The repair search calls this once per
    /// candidate face, so it keeps one buffer alive and clears only the entries
    /// it set last time.
    pub(crate) fn method_c_perimeters_from_selected_faces_with_probe(
        &self,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        probe: &mut MethodCPerimeterProbe,
    ) -> io::Result<Vec<Vec<MethodCPerimeterPoint>>> {
        probe.reset(self.nwd + 1);
        for iw in 2..=self.nwd {
            if selected[iw] {
                probe.mark(iw, self.w_faces[iw].im);
            }
        }
        // Borrow the candidate list before the walk so the sort happens once.
        let candidates = std::mem::take(&mut probe.m_candidates);
        let mut candidates = candidates;
        candidates.sort_unstable();
        candidates.dedup();
        let perimeters =
            self.perim_maps2_method_c_over(&probe.nest_wd, m_neighbors, Some(&candidates));
        probe.m_candidates = candidates;
        perimeters
    }

    /// Perimeter length of the selection one canonical seed would produce.
    ///
    /// Answers, without materializing anything, whether a single demand point
    /// can be served at all: the seed's rad3 footprint plus concavity closure
    /// is exactly what selection would hand to `perim_fill3`, and a length that
    /// is not a multiple of three cannot be decomposed into transition triples
    /// no matter what the repair loop does afterwards.
    pub fn seed_footprint_perimeter_length(&self, im: usize) -> io::Result<Option<usize>> {
        if im < 2 || im > self.nmd {
            return Ok(None);
        }
        let m_neighbors = self.method_c_m_neighbors()?;
        let Ok(footprint) = self.method_c_rad3_faces_with_neighbors(im, &m_neighbors) else {
            return Ok(None);
        };
        let mut selected = vec![false; self.nwd + 1];
        for iw in footprint {
            if iw >= 2 && iw <= self.nwd {
                selected[iw] = true;
            }
        }
        if self
            .close_method_c_concavities_for_level_with_neighbors(&mut selected, &m_neighbors)
            .is_err()
        {
            return Ok(None);
        }
        Ok(self
            .method_c_perimeters_from_selected_faces(&selected, &m_neighbors)
            .ok()
            .map(|perimeters| perimeters.iter().map(Vec::len).sum()))
    }

    pub(crate) fn method_c_perimeters_are_triplets(
        perimeters: &[Vec<MethodCPerimeterPoint>],
    ) -> bool {
        perimeters.iter().all(|perimeter| perimeter.len() % 3 == 0)
    }

    pub(crate) fn method_c_perimeter_remainder_score(
        perimeters: &[Vec<MethodCPerimeterPoint>],
    ) -> usize {
        perimeters.iter().map(|perimeter| perimeter.len() % 3).sum()
    }

    pub(crate) fn method_c_nest_wd_from_selected_and_perimeter(
        &self,
        selected: &[bool],
        perimeter: &[MethodCPerimeterPoint],
    ) -> io::Result<Vec<MethodCNestWd>> {
        let mut nest_wd = vec![MethodCNestWd::default(); self.nwd + 1];
        for iw in 2..=self.nwd {
            if selected[iw] {
                nest_wd[iw].iw[2] = 1;
            }
        }

        for triple in perimeter.chunks_exact(3) {
            let center = triple[1];
            let edge = self.u_edges[center.iu];
            let suppressed_w = if center.im == edge.im[0] {
                edge.iw[1]
            } else {
                edge.iw[0]
            };
            require_method_c_id("Method-C suppressed W face", suppressed_w, self.nwd)?;
            nest_wd[suppressed_w].iw[2] = -1;
        }
        Ok(nest_wd)
    }

    #[cfg(test)]
    pub(crate) fn close_method_c_concavities(&self, selected_faces: &mut [bool]) -> io::Result<()> {
        let method_c_m_neighbors = self.method_c_m_neighbors()?;
        self.close_method_c_concavities_with_neighbors(selected_faces, &method_c_m_neighbors)
    }

    #[cfg(test)]
    pub(crate) fn close_method_c_concavities_with_neighbors(
        &self,
        selected_faces: &mut [bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<()> {
        self.close_method_c_concavities_for_level_with_neighbors(selected_faces, m_neighbors)
    }

    pub(crate) fn close_method_c_concavities_for_level_with_neighbors(
        &self,
        selected_faces: &mut [bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<()> {
        require_method_c_len("selected_faces", selected_faces.len(), self.nwd + 1)?;
        require_method_c_len(
            "Method-C perim M-neighbors",
            m_neighbors.len(),
            self.nmd + 1,
        )?;
        let Some(parent_mrlw) = selected_faces
            .iter()
            .enumerate()
            .skip(2)
            .find_map(|(iw, &selected)| selected.then_some(self.w_faces[iw].mrlw))
        else {
            return Ok(());
        };
        loop {
            let mut changed = false;
            for im in 2..=self.nmd {
                let neighbors = m_neighbors[im];
                let mut selected_count = 0usize;
                for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                    require_method_c_id("Method-C concavity W face", iw, self.nwd)?;
                    selected_count += usize::from(selected_faces[iw]);
                }
                if selected_count == 0 || selected_count == neighbors.npoly {
                    continue;
                }
                // Canonical behavior: expand the rad3 footprint when exactly one
                // incident face is missing. A fully selected ring was skipped
                // above because it has no local concavity to close.
                if selected_count < neighbors.npoly.saturating_sub(1) {
                    continue;
                }
                let footprint = self.method_c_rad3_faces_with_neighbors(im, m_neighbors)?;
                if footprint
                    .iter()
                    .any(|&iw| iw >= 2 && self.w_faces[iw].mrlw != parent_mrlw)
                {
                    continue;
                }
                for iw in footprint {
                    changed |= !selected_faces[iw];
                    selected_faces[iw] = true;
                }
            }
            if !changed {
                return Ok(());
            }
        }
    }
}
