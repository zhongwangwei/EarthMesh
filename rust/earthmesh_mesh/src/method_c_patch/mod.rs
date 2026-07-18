use std::io;

use super::*;

impl MethodCDelaunayMesh {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn perim_fill3_method_c(
        &self,
        perimeter: &[MethodCPerimeterPoint],
        parent_level: usize,
        iwnew: &[usize],
        iunew: &[usize],
        imnew: &[usize],
        nest_wd: &[MethodCNestWd],
        nest_ud: &mut [MethodCNestUd],
        u_edges: &mut [IcosahedronUEdge],
        w_faces: &mut [IcosahedronWFace],
        m_points: &mut [CartesianPoint],
        m_metadata: &mut [IcosahedronMPointMetadata],
        radius: f64,
        child_level: usize,
    ) -> io::Result<()> {
        for triple in perimeter.chunks_exact(3) {
            let [p1, p2, p3] = [triple[0], triple[1], triple[2]];
            let [jm1, jm2, jm3] = [p1.im, p2.im, p3.im];
            let [ju1, ju2, ju3] = [p1.iu, p2.iu, p3.iu];

            let (iu41, iu42, iu46, iw26, iw27) = if jm1 == self.u_edges[ju1].im[0] {
                (
                    iunew[ju1],
                    nest_ud[ju1].iu,
                    iunew[self.u_edges[ju1].iu[4]],
                    iwnew[self.u_edges[ju1].iw[2]],
                    iwnew[self.u_edges[ju1].iw[0]],
                )
            } else {
                (
                    nest_ud[ju1].iu,
                    iunew[ju1],
                    iunew[self.u_edges[ju1].iu[11]],
                    iwnew[self.u_edges[ju1].iw[5]],
                    iwnew[self.u_edges[ju1].iw[1]],
                )
            };

            let (iu49, iu50, iu34, iu35, iu48, iu51, iw6o, iw9o, iw6, iw9, iw29, iw20, iw28, iw30) =
                if jm2 == self.u_edges[ju2].im[0] {
                    (
                        iunew[self.u_edges[ju2].iu[0]],
                        iunew[self.u_edges[ju2].iu[1]],
                        iunew[self.u_edges[ju2].iu[2]],
                        iunew[self.u_edges[ju2].iu[3]],
                        iunew[self.u_edges[ju2].iu[4]],
                        iunew[self.u_edges[ju2].iu[7]],
                        self.u_edges[ju2].iw[4],
                        self.u_edges[ju2].iw[5],
                        iwnew[self.u_edges[ju2].iw[4]],
                        iwnew[self.u_edges[ju2].iw[5]],
                        iwnew[self.u_edges[ju2].iw[0]],
                        iwnew[self.u_edges[ju2].iw[1]],
                        iwnew[self.u_edges[ju2].iw[2]],
                        iwnew[self.u_edges[ju2].iw[3]],
                    )
                } else {
                    (
                        iunew[self.u_edges[ju2].iu[3]],
                        iunew[self.u_edges[ju2].iu[2]],
                        iunew[self.u_edges[ju2].iu[1]],
                        iunew[self.u_edges[ju2].iu[0]],
                        iunew[self.u_edges[ju2].iu[11]],
                        iunew[self.u_edges[ju2].iu[8]],
                        self.u_edges[ju2].iw[3],
                        self.u_edges[ju2].iw[2],
                        iwnew[self.u_edges[ju2].iw[3]],
                        iwnew[self.u_edges[ju2].iw[2]],
                        iwnew[self.u_edges[ju2].iw[1]],
                        iwnew[self.u_edges[ju2].iw[0]],
                        iwnew[self.u_edges[ju2].iw[5]],
                        iwnew[self.u_edges[ju2].iw[4]],
                    )
                };

            let (im21, iu44, iu45, iu53, iw31, iw32) = if jm3 == self.u_edges[ju3].im[0] {
                (
                    imnew[self.u_edges[ju3].im[1]],
                    iunew[ju3],
                    nest_ud[ju3].iu,
                    iunew[self.u_edges[ju3].iu[7]],
                    iwnew[self.u_edges[ju3].iw[0]],
                    iwnew[self.u_edges[ju3].iw[3]],
                )
            } else {
                (
                    imnew[self.u_edges[ju3].im[0]],
                    nest_ud[ju3].iu,
                    iunew[ju3],
                    iunew[self.u_edges[ju3].iu[8]],
                    iwnew[self.u_edges[ju3].iw[1]],
                    iwnew[self.u_edges[ju3].iw[4]],
                )
            };

            let im16 = imnew[jm1];
            let im17 = nest_ud[ju1].im;
            let im18 = imnew[jm2];
            let im19 = imnew[jm3];
            let im20 = nest_ud[ju3].im;
            let iu43 = iunew[ju2];

            let [iu25, iu15] = method_c_split_outer_edges(nest_wd[iw6o].iu, u_edges, "iw6", jm2)?;
            let iw7 = other_edge_face(u_edges[iu15], iw6)?;
            let (iw19, im12) = if u_edges[iu25].iw[0] == iw6 {
                (u_edges[iu25].iw[1], u_edges[iu25].im[1])
            } else {
                (u_edges[iu25].iw[0], u_edges[iu25].im[0])
            };

            let [iu16, iu26] = method_c_split_outer_edges(nest_wd[iw9o].iu, u_edges, "iw9", jm2)?;
            let iw8 = other_edge_face(u_edges[iu16], iw9)?;
            let (iw21, im13) = if u_edges[iu26].iw[0] == iw9 {
                (u_edges[iu26].iw[1], u_edges[iu26].im[0])
            } else {
                (u_edges[iu26].iw[0], u_edges[iu26].im[1])
            };

            let im22 = canonical_other_endpoint_by_first(u_edges[iu46], im16);
            let im23 = canonical_other_endpoint_by_first(u_edges[iu48], im18);
            let im24 = canonical_other_endpoint_by_first(u_edges[iu49], im18);
            let im25 = canonical_other_endpoint_by_first(u_edges[iu51], im19);
            let im26 = canonical_other_endpoint_by_first(u_edges[iu53], im21);

            fill_missing_endpoint(&mut u_edges[iu15], im18);
            fill_missing_endpoint(&mut u_edges[iu16], im18);
            fill_missing_endpoint(&mut u_edges[iu25], im18);
            fill_missing_endpoint(&mut u_edges[iu26], im18);

            let im5 = if u_edges[iu34].im[0] == im18 {
                u_edges[iu34].iw = set_first_two(u_edges[iu34].iw, iw8, iw7);
                u_edges[iu34].im[1]
            } else {
                u_edges[iu34].iw = set_first_two(u_edges[iu34].iw, iw7, iw8);
                u_edges[iu34].im[0]
            };

            if u_edges[iu35].im[0] == im19 {
                u_edges[iu35].iw[1] = iw19;
                u_edges[iu35].iw[0] = iw21;
                u_edges[iu35].im[1] = im18;
            } else {
                u_edges[iu35].iw[0] = iw19;
                u_edges[iu35].iw[1] = iw21;
                u_edges[iu35].im[0] = im18;
            }

            if u_edges[iu41].im[1] == im17 {
                u_edges[iu41].iw[0] = iw27;
            } else {
                u_edges[iu41].iw[1] = iw27;
            }
            if u_edges[iu42].im[0] == im17 {
                u_edges[iu42].im[1] = im19;
                u_edges[iu42].iw[0] = iw20;
            } else {
                u_edges[iu42].im[0] = im19;
                u_edges[iu42].iw[1] = iw20;
            }
            if u_edges[iu43].im[1] == im19 {
                u_edges[iu43].im[0] = im24;
            } else {
                u_edges[iu43].im[1] = im24;
            }
            if u_edges[iu44].im[0] == im19 {
                u_edges[iu44].iw[0] = iw29;
            } else {
                u_edges[iu44].iw[1] = iw29;
            }
            if u_edges[iu45].im[0] == im20 {
                u_edges[iu45].iw[0] = iw31;
            } else {
                u_edges[iu45].iw[1] = iw31;
            }
            if u_edges[iu48].iw[1] == iw27 {
                u_edges[iu48].im[1] = im17;
            } else {
                u_edges[iu48].im[0] = im17;
            }
            if u_edges[iu49].im[1] == im24 {
                u_edges[iu49].im[0] = im17;
                u_edges[iu49].iw[1] = iw20;
            } else {
                u_edges[iu49].im[1] = im17;
                u_edges[iu49].iw[0] = iw20;
            }
            if u_edges[iu50].im[0] == im24 {
                u_edges[iu50].im[1] = im20;
            } else {
                u_edges[iu50].im[0] = im20;
            }
            if u_edges[iu51].iw[1] == iw31 {
                u_edges[iu51].im[0] = im20;
            } else {
                u_edges[iu51].im[1] = im20;
            }

            replace_w_face_edge_after(w_faces, iw8, iu16, iu34, "iw8/iu16->iu34")?;
            let iu33 =
                replace_w_face_edge_with_side_return(w_faces, iw19, iu25, iu35, "iw19/iu25->iu35")?;
            if u_edges[iu33].iw[1] == iw19 {
                u_edges[iu33].im[1] = im19;
            } else {
                u_edges[iu33].im[0] = im19;
            }

            replace_w_face_edges_at(w_faces, iw20, iu43, [iu42, iu49], "iw20/iu43")?;
            replace_w_face_edge_before(w_faces, iw27, iu48, iu41, "iw27/iu48->iu41")?;
            replace_w_face_edges_at(w_faces, iw29, iu50, [iu44, iu43], "iw29/iu50")?;
            replace_w_face_edge_after(w_faces, iw31, iu51, iu45, "iw31/iu51->iu45")?;

            for im in [im22, im23, im24, im25, im26] {
                m_metadata[im].ngr = child_level;
            }
            let transition_w_faces = [iw20, iw26, iw27, iw28, iw29, iw30, iw31, iw32];
            for iw in transition_w_faces.iter().copied() {
                w_faces[iw].ngr = child_level;
            }
            for iw in transition_w_faces {
                if w_faces[iw].mrlw != parent_level {
                    let center = normalized_face_center(
                        m_points[w_faces[iw].im[0]],
                        m_points[w_faces[iw].im[1]],
                        m_points[w_faces[iw].im[2]],
                        radius,
                    )?;
                    let ll = xyz_to_lonlat_degrees(center);
                    return Err(method_c_repairable_error(
                        MethodCRepairableKind::TransitionPatch,
                        Some(jm2),
                        format!(
                            "Method-C perimeter length invalid: Current nested grid {child_level} crosses the parent boundary in Method-C transition at W face {iw} (mrlw={}, lon={:.3}, lat={:.3})",
                            w_faces[iw].mrlw,
                            ll.lon_degrees,
                            ll.lat_degrees
                        ),
                    ));
                }
            }

            m_metadata[im17].mrlm_orig = m_metadata[im18].mrlm_orig;
            m_metadata[im20].mrlm_orig = m_metadata[im19].mrlm_orig;
            m_metadata[im18].mrlm_orig = parent_level + 1;
            m_metadata[im19].mrlm_orig = parent_level + 1;

            m_points[im19] = weighted_point(m_points[im24], 1.0, m_points[im5], 1.0)?;
            m_points[im18] = weighted_point(m_points[im19], 1.0, m_points[im5], 1.0)?;
            m_points[im17] = weighted_point(m_points[im17], 0.75, m_points[im19], 0.25)?;
            m_points[im20] = weighted_point(m_points[im20], 0.75, m_points[im19], 0.25)?;
            m_points[im12] = weighted_point(m_points[im12], 0.833, m_points[im18], 0.167)?;
            m_points[im13] = weighted_point(m_points[im13], 0.833, m_points[im18], 0.167)?;
        }

        Ok(())
    }
}
