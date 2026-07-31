use std::io;

use super::*;

impl MethodCDelaunayMesh {
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
