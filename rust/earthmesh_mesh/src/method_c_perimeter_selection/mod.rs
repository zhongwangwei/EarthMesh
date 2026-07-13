use std::io;

use super::*;

impl MethodCDelaunayMesh {
    pub(crate) fn method_c_perimeter_from_selected_faces(
        &self,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<Vec<MethodCPerimeterPoint>> {
        let mut probe_nest_wd = vec![MethodCNestWd::default(); self.nwd + 1];
        for iw in 2..=self.nwd {
            if selected[iw] {
                probe_nest_wd[iw].iw[2] = 1;
            }
        }

        let perimeter = self.perim_map2_method_c(&probe_nest_wd, m_neighbors)?;
        if perimeter.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Method-C perimeter is empty",
            ));
        }
        Ok(perimeter)
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
