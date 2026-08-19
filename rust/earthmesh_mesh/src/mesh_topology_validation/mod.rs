use super::*;

/// Summary returned after checking an [`TriangularMesh`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MethodCTopologyValidation {
    pub checked_m_points: usize,
    pub checked_u_edges: usize,
    pub checked_w_faces: usize,
}

impl TriangularMesh {
    /// Check reciprocal `M/U/W` topology invariants for the active Method-C slots.
    ///
    /// Slot `0` is Rust's unused vector slot and slot `1` mirrors Method-C's
    /// sentinel record. Active records are `2..=nmd`, `2..=nud`, and `2..=nwd`.
    pub fn validate_topology(&self) -> io::Result<MethodCTopologyValidation> {
        require_method_c_len("m_points", self.m_points.len(), self.nmd + 1)?;
        require_method_c_len("u_edges", self.u_edges.len(), self.nud + 1)?;
        require_method_c_len("w_faces", self.w_faces.len(), self.nwd + 1)?;
        require_method_c_len("m_neighbors", self.m_neighbors.len(), self.nmd + 1)?;
        require_method_c_len("m_prognostic", self.m_prognostic.len(), self.nmd + 1)?;
        require_method_c_len("u_prognostic", self.u_prognostic.len(), self.nud + 1)?;
        require_method_c_len("w_prognostic", self.w_prognostic.len(), self.nwd + 1)?;
        validate_method_c_prognostic_map(
            "Method-C M prognostic owner",
            &self.m_prognostic,
            self.nmd,
        )?;
        validate_method_c_prognostic_map(
            "Method-C U prognostic owner",
            &self.u_prognostic,
            self.nud,
        )?;
        validate_method_c_prognostic_map(
            "Method-C W prognostic owner",
            &self.w_prognostic,
            self.nwd,
        )?;

        for iu in 2..=self.nud {
            let edge = self.u_edges[iu];
            let [im1, im2] = edge.im;
            require_method_c_id("U edge M endpoint", im1, self.nmd)?;
            require_method_c_id("U edge M endpoint", im2, self.nmd)?;
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
                require_method_c_id("U edge adjacent W face", iw, self.nwd)?;
                let w_partner = self.w_prognostic[iw];
                if w_partner > 1 && w_partner != iw {
                    require_method_c_id("U edge periodic W face partner", w_partner, self.nwd)?;
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
                require_method_c_id("periodic W face partner", w_partner, self.nwd)?;
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
                        format!("W face {iw} canonicals U edge {iu} outside its M vertices"),
                    ));
                }
            }
        }

        for im in 2..=self.nmd {
            let m_partner = self.m_prognostic[im];
            if m_partner > 1 && m_partner != im {
                require_method_c_id("periodic M point partner", m_partner, self.nmd)?;
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
                require_method_c_id("M point U edge", iu, self.nud)?;
                require_method_c_id("M point W face", iw, self.nwd)?;
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

        self.validate_global_sphere_invariants()?;

        Ok(MethodCTopologyValidation {
            checked_m_points: self.nmd.saturating_sub(1),
            checked_u_edges: self.nud.saturating_sub(1),
            checked_w_faces: self.nwd.saturating_sub(1),
        })
    }

    fn validate_global_sphere_invariants(&self) -> io::Result<()> {
        let uses_cartesian_pentagon_sentinel = self.impent.iter().all(|&point| point == 1);
        let has_global_pentagons = self.impent.iter().all(|&point| point > 1);
        if !uses_cartesian_pentagon_sentinel && !has_global_pentagons {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C impent must contain either twelve active global points or twelve Cartesian sentinel slots",
            ));
        }
        if !has_global_pentagons {
            return Ok(());
        }

        let has_periodic_copies = self
            .m_prognostic
            .iter()
            .enumerate()
            .skip(2)
            .take(self.nmd.saturating_sub(1))
            .any(|(id, &owner)| owner > 1 && owner != id)
            || self
                .u_prognostic
                .iter()
                .enumerate()
                .skip(2)
                .take(self.nud.saturating_sub(1))
                .any(|(id, &owner)| owner > 1 && owner != id)
            || self
                .w_prognostic
                .iter()
                .enumerate()
                .skip(2)
                .take(self.nwd.saturating_sub(1))
                .any(|(id, &owner)| owner > 1 && owner != id);
        if has_periodic_copies {
            return Ok(());
        }

        let mut protected = vec![false; self.nmd + 1];
        for &point in &self.impent {
            require_method_c_id("Method-C protected pentagon", point, self.nmd)?;
            if protected[point] {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Method-C impent contains duplicate protected point {point}"),
                ));
            }
            if self.m_neighbors[point].npoly != 5 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "Method-C protected pentagon {point} has degree {}, expected 5",
                        self.m_neighbors[point].npoly
                    ),
                ));
            }
            protected[point] = true;
        }

        validate_method_c_sphere_counts(self.nmd, self.nud, self.nwd)?;
        Ok(())
    }
}

fn validate_method_c_prognostic_map(
    label: &str,
    owners: &[usize],
    max_id: usize,
) -> io::Result<()> {
    for id in 2..=max_id {
        let owner = owners[id];
        require_method_c_id(label, owner, max_id)?;
        if owner != id && owners[owner] != owner {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{label} for periodic copy {id} points to non-owner {owner}, whose owner is {}",
                    owners[owner]
                ),
            ));
        }
    }
    Ok(())
}

fn validate_method_c_sphere_counts(nmd: usize, nud: usize, nwd: usize) -> io::Result<()> {
    let vertices = nmd.saturating_sub(1) as i128;
    let edges = nud.saturating_sub(1) as i128;
    let faces = nwd.saturating_sub(1) as i128;
    let euler_characteristic = vertices - edges + faces;
    if euler_characteristic != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Method-C global sphere violates Euler invariant: V-E+F = {vertices}-{edges}+{faces} = {euler_characteristic}, expected 2"
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{validate_method_c_sphere_counts, TriangularMesh};

    #[test]
    fn global_sphere_count_check_rejects_extra_components() {
        validate_method_c_sphere_counts(13, 31, 21).expect("icosahedron has Euler two");
        let error = validate_method_c_sphere_counts(17, 37, 25).unwrap_err();
        assert!(error.to_string().contains("expected 2"), "{error}");
    }

    #[test]
    fn topology_rejects_out_of_range_prognostic_owners_before_periodic_skip() {
        let base = TriangularMesh::from_icosahedron(2, 0, 1.0, 0.25).expect("valid Method-C mesh");

        let mut invalid_m = base.clone();
        invalid_m.m_prognostic[2] = invalid_m.nmd + 1;
        let error = invalid_m.validate_topology().unwrap_err();
        assert!(error.to_string().contains("M prognostic owner"), "{error}");

        let mut invalid_u = base.clone();
        invalid_u.u_prognostic[2] = invalid_u.nud + 1;
        let error = invalid_u.validate_topology().unwrap_err();
        assert!(error.to_string().contains("U prognostic owner"), "{error}");

        let mut invalid_w = base;
        invalid_w.w_prognostic[2] = invalid_w.nwd + 1;
        let error = invalid_w.validate_topology().unwrap_err();
        assert!(error.to_string().contains("W prognostic owner"), "{error}");
    }

    #[test]
    fn topology_rejects_periodic_owner_chains() {
        let mut mesh =
            TriangularMesh::from_icosahedron(2, 0, 1.0, 0.25).expect("valid Method-C mesh");
        mesh.u_prognostic[2] = 3;
        mesh.u_prognostic[3] = 4;

        let error = mesh.validate_topology().unwrap_err();
        assert!(error.to_string().contains("non-owner"), "{error}");
    }
}
