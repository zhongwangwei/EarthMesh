use std::io;

use super::*;

impl MethodCMesh {
    pub(crate) fn method_c_perimeters_from_selected_faces(
        &self,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<Vec<Vec<MethodCPerimeterPoint>>> {
        let mut probe_nest_wd = vec![MethodCNestWd::default(); self.nwd + 1];
        for iw in 2..=self.nwd {
            if selected[iw] {
                probe_nest_wd[iw].iw[2] = 1;
            }
        }
        self.perim_maps2_method_c(&probe_nest_wd, m_neighbors)
    }

    pub(crate) fn method_c_perimeters_are_triplets(
        perimeters: &[Vec<MethodCPerimeterPoint>],
    ) -> bool {
        perimeters.iter().all(|perimeter| perimeter.len() % 3 == 0)
    }

    /// Perimeter points the transition patch is measured to refuse.
    ///
    /// `perim_fill3` works on consecutive triples of each block's perimeter and
    /// only carries a straight run or a corner that opens a triple. Over 420
    /// contiguous footprint unions at NXP 21 and 40 (`method_c_shape_probe`),
    /// every normalised mask with one of these failed to emit and every mask
    /// without one built:
    ///
    /// - a defect on the perimeter: a point whose ring is not six faces (a
    ///   pentagon, or a transition-band point of an earlier generation);
    /// - a spike: a point held by a single selected face;
    /// - a corner (selected-face count other than the straight-edge three) at a
    ///   triple position other than the first, i.e. a straight run between two
    ///   corners whose length is not a multiple of three.
    // Wired into the triplet normaliser next; measured by the shape probe now.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn method_c_perimeter_shape_violations(
        selected: &[bool],
        perimeters: &[Vec<MethodCPerimeterPoint>],
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> Vec<(usize, MethodCShapeViolation)> {
        let mut violations = Vec::new();
        for perimeter in perimeters {
            for (position, point) in perimeter.iter().enumerate() {
                let neighbors = m_neighbors[point.im];
                if neighbors.npoly != 6 {
                    violations.push((point.im, MethodCShapeViolation::DefectOnPerimeter));
                    continue;
                }
                let held = neighbors
                    .iw
                    .iter()
                    .take(neighbors.npoly)
                    .filter(|&&iw| selected.get(iw).copied().unwrap_or(false))
                    .count();
                if held == 1 {
                    violations.push((point.im, MethodCShapeViolation::Spike));
                } else if held != 3 && position % 3 != 0 {
                    violations.push((point.im, MethodCShapeViolation::MisalignedCorner));
                }
            }
        }
        violations
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

        for triple in perimeter.as_chunks::<3>().0 {
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
                // Canonical behavior: fill when the selected incidence is at least
                // (npoly - 1), including pentagons when exactly one face is
                // missing and when all faces are selected.
                if selected_count < neighbors.npoly.saturating_sub(1) {
                    continue;
                }
                changed |=
                    self.mark_fill_rad3_faces_with_neighbors(im, selected_faces, m_neighbors)?;
            }
            if !changed {
                return Ok(());
            }
        }
    }
}

/// Why [`MethodCMesh::method_c_perimeter_shape_violations`] flagged a point.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MethodCShapeViolation {
    DefectOnPerimeter,
    Spike,
    MisalignedCorner,
}
