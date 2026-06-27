use super::*;

/// Summary returned after checking an [`OlamDelaunayMesh`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OlamTopologyValidation {
    pub checked_m_points: usize,
    pub checked_u_edges: usize,
    pub checked_w_faces: usize,
}

impl OlamDelaunayMesh {
    /// Check reciprocal `M/U/W` topology invariants for the active OLAM slots.
    ///
    /// Slot `0` is Rust's unused vector slot and slot `1` mirrors OLAM's
    /// sentinel record. Active records are `2..=nmd`, `2..=nud`, and `2..=nwd`.
    pub fn validate_topology(&self) -> io::Result<OlamTopologyValidation> {
        require_olam_len("m_points", self.m_points.len(), self.nmd + 1)?;
        require_olam_len("u_edges", self.u_edges.len(), self.nud + 1)?;
        require_olam_len("w_faces", self.w_faces.len(), self.nwd + 1)?;
        require_olam_len("m_neighbors", self.m_neighbors.len(), self.nmd + 1)?;
        require_olam_len("m_prognostic", self.m_prognostic.len(), self.nmd + 1)?;
        require_olam_len("u_prognostic", self.u_prognostic.len(), self.nud + 1)?;
        require_olam_len("w_prognostic", self.w_prognostic.len(), self.nwd + 1)?;

        for iu in 2..=self.nud {
            let edge = self.u_edges[iu];
            let [im1, im2] = edge.im;
            require_olam_id("U edge M endpoint", im1, self.nmd)?;
            require_olam_id("U edge M endpoint", im2, self.nmd)?;
            if im1 == im2 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("U edge {iu} has duplicate M endpoints {im1}"),
                ));
            }

            let adjacent_faces = [edge.iw[0], edge.iw[1]];
            if adjacent_faces[0] == adjacent_faces[1] {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "U edge {iu} has duplicate adjacent W face {}",
                        adjacent_faces[0]
                    ),
                ));
            }
            for &iw in &adjacent_faces {
                require_olam_id("U edge adjacent W face", iw, self.nwd)?;
                let w_partner = self.w_prognostic[iw];
                if w_partner > 1 && w_partner != iw {
                    require_olam_id("U edge periodic W face partner", w_partner, self.nwd)?;
                    continue;
                }
                let face = self.w_faces[iw];
                if !face.iu.contains(&iu) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "U edge {iu} points to W face {iw}, but the face does not point back"
                        ),
                    ));
                }
                if !face.im.contains(&im1) || !face.im.contains(&im2) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("U edge {iu} endpoints [{im1}, {im2}] are not both on W face {iw}"),
                    ));
                }
            }
        }

        for iw in 2..=self.nwd {
            let w_partner = self.w_prognostic[iw];
            if w_partner > 1 && w_partner != iw {
                require_olam_id("periodic W face partner", w_partner, self.nwd)?;
                continue;
            }
            let face = self.w_faces[iw];
            if face.npoly != 3 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("W face {iw} must be triangular, got npoly {}", face.npoly),
                ));
            }
            require_unique_active_triplet("W face M vertices", iw, face.im, self.nmd)?;
            require_unique_active_triplet("W face U edges", iw, face.iu, self.nud)?;

            for &iu in &face.iu {
                let edge = self.u_edges[iu];
                if edge.iw[0] != iw && edge.iw[1] != iw {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "W face {iw} points to U edge {iu}, but the edge does not point back"
                        ),
                    ));
                }
                if !face.im.contains(&edge.im[0]) || !face.im.contains(&edge.im[1]) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("W face {iw} references U edge {iu} outside its M vertices"),
                    ));
                }
            }
        }

        for im in 2..=self.nmd {
            let m_partner = self.m_prognostic[im];
            if m_partner > 1 && m_partner != im {
                require_olam_id("periodic M point partner", m_partner, self.nmd)?;
                continue;
            }
            let neighbors = self.m_neighbors[im];
            if !(3..=7).contains(&neighbors.npoly) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("M point {im} has invalid npoly {}", neighbors.npoly),
                ));
            }
            for j in 0..neighbors.npoly {
                let iu = neighbors.iu[j];
                let iw = neighbors.iw[j];
                require_olam_id("M point U edge", iu, self.nud)?;
                require_olam_id("M point W face", iw, self.nwd)?;
                if !self.u_edges[iu].im.contains(&im) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "M point {im} points to U edge {iu}, but the edge does not point back"
                        ),
                    ));
                }
                if !self.w_faces[iw].im.contains(&im) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "M point {im} points to W face {iw}, but the face does not point back"
                        ),
                    ));
                }
            }
        }

        Ok(OlamTopologyValidation {
            checked_m_points: self.nmd.saturating_sub(1),
            checked_u_edges: self.nud.saturating_sub(1),
            checked_w_faces: self.nwd.saturating_sub(1),
        })
    }
}
