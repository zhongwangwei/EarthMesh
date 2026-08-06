use std::io;

use super::*;

impl TriangularMesh {
    pub(crate) fn emit_method_c_tables(
        &self,
        perimeter: &[MethodCPerimeterPoint],
        m_neighbors: &[IcosahedronMPointNeighbors],
        nest_wd: &mut [MethodCNestWd],
        child_level: usize,
        max_mrows: usize,
        project_to_radius: bool,
    ) -> io::Result<Self> {
        let radius = active_mesh_radius(self)?;
        let parent_level = child_level - 1;
        require_method_c_len(
            "Method-C perim M-neighbors",
            m_neighbors.len(),
            self.nmd + 1,
        )?;

        let mut iwnew = vec![1usize; self.nwd + 1];
        let mut iwnext = 2usize;
        iwnew[1] = 1;
        for iw in 2..=self.nwd {
            iwnew[iw] = iwnext;
            if nest_wd[iw].is_subdivided() {
                iwnext += 1;
                nest_wd[iw].iw[0] = iwnext as isize;
                iwnext += 1;
                nest_wd[iw].iw[1] = iwnext as isize;
                iwnext += 1;
                nest_wd[iw].iw[2] = iwnext as isize;
            }
            iwnext += 1;
        }
        let nwd0 = iwnext - 1;

        let mut nest_ud = vec![MethodCNestUd::default(); self.nud + 1];
        let mut iunew = vec![1usize; self.nud + 1];
        let mut iwdiv = vec![false; self.nwd + 1];
        let mut iunext = 2usize;
        iunew[1] = 1;
        for iu in 2..=self.nud {
            iunew[iu] = iunext;
            let edge = self.u_edges[iu];
            let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
            if nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided() {
                if nest_wd[iw1].is_suppressed() || nest_wd[iw2].is_suppressed() {
                    nest_ud[iu].iu = iunew[iu];
                } else {
                    iunext += 1;
                    nest_ud[iu].iu = iunext;
                }
            }

            for &iw in &edge.iw[0..2] {
                if !iwdiv[iw] {
                    iwdiv[iw] = true;
                    if nest_wd[iw].is_subdivided() {
                        iunext += 1;
                        nest_wd[iw].iu[0] = iunext;
                        iunext += 1;
                        nest_wd[iw].iu[1] = iunext;
                        iunext += 1;
                        nest_wd[iw].iu[2] = iunext;
                    }
                }
            }
            iunext += 1;
        }
        let nud0 = iunext - 1;

        let mut imnew = vec![1usize; self.nmd + 1];
        let mut iudiv = vec![false; self.nud + 1];
        let mut imnext = 2usize;
        imnew[1] = 1;
        for im in 2..=self.nmd {
            imnew[im] = imnext;
            let neighbors = m_neighbors[im];
            for &iu in neighbors.iu.iter().take(neighbors.npoly) {
                if !iudiv[iu] {
                    iudiv[iu] = true;
                    let edge = self.u_edges[iu];
                    let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
                    if nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided() {
                        if nest_wd[iw1].is_suppressed() || nest_wd[iw2].is_suppressed() {
                            nest_ud[iu].im = 1;
                        } else {
                            imnext += 1;
                            nest_ud[iu].im = imnext;
                        }
                    }
                }
            }
            imnext += 1;
        }
        let nmd0 = imnext - 1;

        let mut impent = [1usize; 12];
        for (slot, &old_im) in self.impent.iter().enumerate() {
            if old_im <= 1 {
                continue;
            }
            require_method_c_id("Method-C impent", old_im, self.nmd)?;
            impent[slot] = imnew[old_im];
        }

        let mut m_points = vec![CartesianPoint::new(0.0, 0.0, 0.0); nmd0 + 1];
        let mut m_metadata = default_method_c_m_metadata(nmd0);
        let mut u_edges = vec![IcosahedronUEdge::default(); nud0 + 1];
        let mut w_faces = vec![IcosahedronWFace::default(); nwd0 + 1];

        for im in 2..=self.nmd {
            let imn = imnew[im];
            m_points[imn] = self.m_points[im];
            m_metadata[imn] = self.m_metadata[im];
        }

        // Lineage travels with the row. A subdivided parent hands its own
        // lineage to all four children, so "where did this cell come from"
        // survives both the split and the renumbering that follows it.
        let mut w_lineage = vec![0usize; nwd0 + 1];
        let mut m_lineage = vec![0usize; nmd0 + 1];
        // The placeholder row maps to itself, as `iwnew[1]`/`imnew[1]` do.
        w_lineage[1] = 1;
        m_lineage[1] = 1;
        for im in 2..=self.nmd {
            let imn = imnew[im];
            if imn < m_lineage.len() {
                m_lineage[imn] = self.m_lineage.get(im).copied().unwrap_or(im);
            }
        }

        let mut parent_mrlm = 0usize;
        for iu in 2..=self.nud {
            let iun = iunew[iu];
            let old = self.u_edges[iu];
            u_edges[iun] = IcosahedronUEdge {
                im: old.im.map(|im| imnew[im]),
                iw: old.iw.map(|iw| iwnew[iw]),
                iu: old.iu.map(|iu2| iunew[iu2]),
                mrlu: old.mrlu,
            };

            if nest_ud[iu].im > 1 {
                let im_mid = nest_ud[iu].im;
                let im1 = u_edges[iun].im[0];
                let im2 = u_edges[iun].im[1];
                if parent_mrlm == 0 {
                    parent_mrlm = m_metadata[im1].mrlm;
                }
                let refined_mrlm = parent_mrlm + 1;
                m_points[im_mid] = weighted_point(m_points[im1], 1.0, m_points[im2], 1.0)?;
                m_metadata[im1].mrlm = refined_mrlm;
                m_metadata[im2].mrlm = refined_mrlm;
                m_metadata[im_mid].mrlm = refined_mrlm;
                m_metadata[im_mid].mrlm_orig = refined_mrlm;
                m_metadata[im1].ngr = child_level;
                m_metadata[im2].ngr = child_level;
                m_metadata[im_mid].ngr = child_level;
                // A midpoint is new: it descends from the edge it split, so it
                // takes the ancestry of that edge's endpoints. The smaller of
                // the two keeps the choice deterministic. Leaving it unset would
                // write ancestor 0 -- a row that does not exist -- which is the
                // same defect the placeholder row had.
                if im_mid < m_lineage.len() {
                    let ends = [
                        m_lineage.get(im1).copied().unwrap_or(0),
                        m_lineage.get(im2).copied().unwrap_or(0),
                    ];
                    m_lineage[im_mid] = ends
                        .into_iter()
                        .filter(|id| *id > 0)
                        .min()
                        .unwrap_or(im_mid);
                }
            }
        }

        let mut parent_mrlw = 0usize;
        for iw in 2..=self.nwd {
            let iwn = iwnew[iw];
            let old = self.w_faces[iw];
            let parent_lineage = self.w_lineage.get(iw).copied().unwrap_or(iw);
            if iwn < w_lineage.len() {
                w_lineage[iwn] = parent_lineage;
            }
            if nest_wd[iw].is_subdivided() {
                for slot in 0..3 {
                    let child = nest_wd[iw].iw[slot] as usize;
                    if child < w_lineage.len() {
                        w_lineage[child] = parent_lineage;
                    }
                }
            }
            w_faces[iwn] = IcosahedronWFace {
                npoly: old.npoly,
                im: old.im.map(|im| imnew[im]),
                iu: old.iu.map(|iu| iunew[iu]),
                iw: old.iw.map(|iw2| iwnew[iw2]),
                mrlw: old.mrlw,
                mrlw_orig: old.mrlw_orig,
                ngr: old.ngr,
                mrow: old.mrow,
            };

            if nest_wd[iw].is_subdivided() {
                if parent_mrlw == 0 {
                    parent_mrlw = old.mrlw;
                }
                if old.mrlw != parent_mrlw {
                    let center = normalized_face_center(
                        m_points[old.im[0]],
                        m_points[old.im[1]],
                        m_points[old.im[2]],
                        radius,
                    )?;
                    let ll = xyz_to_lonlat_degrees(center);
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "Current nested grid {child_level} crosses the parent boundary / crosses (or is too close to) the next coarser grid boundary at W face {iw} (mrlw={}, lon={:.3}, lat={:.3})",
                            old.mrlw, ll.lon_degrees, ll.lat_degrees
                        ),
                    ));
                }
                self.fill_method_c_full_subdivision(
                    iw,
                    &iwnew,
                    &iunew,
                    &imnew,
                    child_level,
                    nest_wd,
                    &nest_ud,
                    &mut u_edges,
                    &mut w_faces,
                )?;
            }
        }

        let transition_parent_mrlw = if parent_mrlw == 0 {
            parent_level
        } else {
            parent_mrlw
        };
        self.perim_fill3_method_c(
            perimeter,
            transition_parent_mrlw,
            &iwnew,
            &iunew,
            &imnew,
            nest_wd,
            &mut nest_ud,
            &mut u_edges,
            &mut w_faces,
            &mut m_points,
            &mut m_metadata,
            radius,
            child_level,
        )?;

        if project_to_radius {
            for point in m_points.iter_mut().take(nmd0 + 1).skip(2) {
                *point = normalize_cartesian_to_radius(*point, radius)?;
            }
        }

        let mut connectivity = IcosahedronDiamondConnectivity { u_edges, w_faces };
        derive_icosahedron_w_neighbors_canonical(&mut connectivity).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "failed to derive Method-C W-face neighbors",
            )
        })?;
        derive_icosahedron_u_neighbors_canonical(&mut connectivity).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "failed to derive Method-C U-edge neighbors",
            )
        })?;
        require_method_c_len(
            "Method-C M prognostic map",
            self.m_prognostic.len(),
            self.nmd + 1,
        )?;
        require_method_c_len(
            "Method-C U prognostic map",
            self.u_prognostic.len(),
            self.nud + 1,
        )?;
        require_method_c_len(
            "Method-C W prognostic map",
            self.w_prognostic.len(),
            self.nwd + 1,
        )?;
        let mut m_prognostic = method_c_identity_prognostic_map(nmd0);
        for old_im in 2..=self.nmd {
            let partner = self.m_prognostic[old_im];
            if partner > 1 {
                require_method_c_id("Method-C M prognostic partner", partner, self.nmd)?;
                m_prognostic[imnew[old_im]] = imnew[partner];
            }
        }
        let mut u_prognostic = method_c_identity_prognostic_map(nud0);
        for old_iu in 2..=self.nud {
            let partner = self.u_prognostic[old_iu];
            if partner > 1 {
                require_method_c_id("Method-C U prognostic partner", partner, self.nud)?;
                u_prognostic[iunew[old_iu]] = iunew[partner];
            }
        }
        let mut w_prognostic = method_c_identity_prognostic_map(nwd0);
        for old_iw in 2..=self.nwd {
            let partner = self.w_prognostic[old_iw];
            if partner > 1 {
                require_method_c_id("Method-C W prognostic partner", partner, self.nwd)?;
                w_prognostic[iwnew[old_iw]] = iwnew[partner];
            }
        }
        let has_prognostic_w_faces = w_prognostic
            .iter()
            .enumerate()
            .skip(2)
            .any(|(iw, &partner)| partner > 1 && partner != iw);
        let m_neighbors = if has_prognostic_w_faces {
            derive_cart_hex_m_neighbors_from_active_faces(
                nmd0,
                &connectivity.u_edges,
                &connectivity.w_faces,
                &w_prognostic,
            )?
        } else {
            derive_icosahedron_m_neighbors_canonical_checked_with_prognostic(
                nmd0,
                &connectivity.u_edges,
                &connectivity.w_faces,
                None,
            )?
        };

        let mut mesh = TriangularMesh {
            nmd: nmd0,
            nud: nud0,
            nwd: nwd0,
            impent,
            m_points,
            m_metadata,
            u_edges: connectivity.u_edges,
            w_faces: connectivity.w_faces,
            m_neighbors,
            m_prognostic,
            u_prognostic,
            w_prognostic,
            boundary_rows: Vec::new(),
            w_lineage,
            m_lineage,
        };
        mesh.apply_method_c_perimeter_mrows(child_level, max_mrows)?;
        // Defense in depth: this function performs the densest parent->child
        // index remapping in the crate; validate the emitted topology like
        // `method_c_spring`/`method_c_nest_spring` validate theirs, so an in-range but
        // wrong id cannot silently escape to callers.
        mesh.validate_topology()?;
        Ok(mesh)
    }
}
