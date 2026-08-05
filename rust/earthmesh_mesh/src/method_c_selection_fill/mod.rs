use super::*;

impl MethodCDelaunayMesh {
    pub(crate) fn method_c_rad3_faces_with_neighbors(
        &self,
        im: usize,
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<Vec<usize>> {
        require_method_c_id("Method-C fill_rad3 M point", im, self.nmd)?;
        require_method_c_len(
            "Method-C perim M-neighbors",
            m_neighbors.len(),
            self.nmd + 1,
        )?;

        let neighbors = m_neighbors[im];
        let mut faces = Vec::new();
        for &iw in neighbors.iw.iter().take(neighbors.npoly) {
            require_method_c_id("Method-C fill_rad3 sector W face", iw, self.nwd)?;
            faces.push(iw);

            let face = self.w_faces[iw];
            let (imx, iwx, iwy) = if im == face.im[0] {
                (face.im[1], face.iw[3], face.iw[4])
            } else if im == face.im[1] {
                (face.im[2], face.iw[5], face.iw[6])
            } else if im == face.im[2] {
                (face.im[0], face.iw[7], face.iw[8])
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Method-C fill_rad3 M point {im} is not on W face {iw}"),
                ));
            };
            require_method_c_id("Method-C fill_rad3 sector M point", imx, self.nmd)?;
            require_method_c_id("Method-C fill_rad3 outer W face", iwx, self.nwd)?;
            require_method_c_id("Method-C fill_rad3 outer W face", iwy, self.nwd)?;

            let (im1, im2) =
                face_following_two_vertices(self.w_faces[iwx], imx, iwx).map_err(|error| {
                    io::Error::new(
                        error.kind(),
                        format!(
                            "fill_rad3 im={im} iw={iw} imx={imx} iwx={iwx} face={:?}/{:?}: {error}",
                            face.im, face.iw
                        ),
                    )
                })?;
            require_method_c_id("Method-C fill_rad3 distant M point", im1, self.nmd)?;
            require_method_c_id("Method-C fill_rad3 distant M point", im2, self.nmd)?;
            let im3 = face_following_vertex(self.w_faces[iwy], im2, iwy).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "fill_rad3 im={im} iw={iw} imx={imx} im2={im2} iwy={iwy} face={:?}/{:?}: {error}",
                        face.im, face.iw
                    ),
                )
            })?;
            require_method_c_id("Method-C fill_rad3 distant M point", im3, self.nmd)?;

            for far_im in [im1, im2, im3] {
                let far_neighbors = m_neighbors[far_im];
                for &far_iw in far_neighbors.iw.iter().take(6) {
                    if far_iw > self.nwd {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("Method-C fill_rad3 distant W face {far_iw} is out of range"),
                        ));
                    }
                    faces.push(far_iw);
                }
            }
        }
        Ok(faces)
    }

    pub(crate) fn mark_fill_rad3_faces_with_neighbors(
        &self,
        im: usize,
        selected_faces: &mut [bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<bool> {
        require_method_c_id("Method-C fill_rad3 M point", im, self.nmd)?;
        require_method_c_len("selected_faces", selected_faces.len(), self.nwd + 1)?;
        require_method_c_len(
            "Method-C perim M-neighbors",
            m_neighbors.len(),
            self.nmd + 1,
        )?;

        // The mask holds one generation by contract, and the seed footprint
        // that built it filters to that generation -- this fill has to as well.
        // Unfiltered, a concavity next to a block refined earlier in the same
        // pass pulled that block's finer faces into the mask, and the
        // shared-parent validation then refused the whole group: measured on a
        // global run as "crosses (or is too close to) the next coarser grid
        // boundary", refusing the continental groups that held 98% of the
        // circles.
        let mask_mrlw = (2..=self.nwd)
            .find(|&iw| selected_faces[iw])
            .map(|iw| self.w_faces[iw].mrlw);
        let mut changed = false;
        for iw in self.method_c_rad3_faces_with_neighbors(im, m_neighbors)? {
            if mask_mrlw.is_some_and(|mrlw| self.w_faces[iw].mrlw != mrlw) {
                continue;
            }
            changed |= !selected_faces[iw];
            selected_faces[iw] = true;
        }

        Ok(changed)
    }

    #[cfg(test)]
    pub(crate) fn method_c_w_face_is_active(&self, iw: usize) -> bool {
        if iw > self.nwd || self.w_prognostic.get(iw).copied().unwrap_or(iw) != iw {
            return false;
        }
        self.w_faces[iw]
            .im
            .iter()
            .all(|&im| im > 1 && self.m_prognostic.get(im).copied().unwrap_or(im) == im)
    }
}
