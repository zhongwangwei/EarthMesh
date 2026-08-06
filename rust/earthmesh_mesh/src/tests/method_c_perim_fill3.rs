use super::super::*;

#[test]
fn method_c_perim_fill3_writes_canonical_weighted_transition_coordinates() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let region = MethodCRefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let mut selected = mesh
        .selected_region_faces(&region, 1, false)
        .expect("selected Method-C faces");
    let selected_parent_mrl = (2..=mesh.nwd)
        .find(|&iw| selected.get(iw).copied().unwrap_or(false))
        .map(|iw| mesh.w_faces[iw].mrlw);
    let method_c_m_neighbors = mesh
        .derive_icosahedron_m_neighbors_canonical()
        .expect("Method-C M-neighbor table should derive");
    mesh.close_method_c_concavities_for_level_with_neighbors(&mut selected, &method_c_m_neighbors)
        .expect("Method-C concavity closure");
    if let Some(parent_mrl) = selected_parent_mrl {
        for iw in 2..=mesh.nwd {
            if mesh.w_faces[iw].mrlw != parent_mrl {
                selected[iw] = false;
            }
        }
    }

    let mut nest_wd = vec![MethodCNestWd::default(); mesh.nwd + 1];
    for iw in 2..=mesh.nwd {
        if selected[iw] {
            nest_wd[iw].iw[2] = 1;
        }
    }
    let perimeter = mesh
        .perim_map2_method_c(&nest_wd, &method_c_m_neighbors)
        .expect("Method-C perimeter");
    for triple in perimeter.chunks_exact(3) {
        let center = triple[1];
        let edge = mesh.u_edges[center.iu];
        let suppressed_w = if center.im == edge.im[0] {
            edge.iw[1]
        } else {
            edge.iw[0]
        };
        nest_wd[suppressed_w].iw[2] = -1;
    }

    let mut iwnew = vec![1usize; mesh.nwd + 1];
    let mut iwnext = 2usize;
    iwnew[1] = 1;
    for iw in 2..=mesh.nwd {
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

    let mut nest_ud = vec![MethodCNestUd::default(); mesh.nud + 1];
    let mut iunew = vec![1usize; mesh.nud + 1];
    let mut iwdiv = vec![false; mesh.nwd + 1];
    let mut iunext = 2usize;
    iunew[1] = 1;
    for iu in 2..=mesh.nud {
        iunew[iu] = iunext;
        let edge = mesh.u_edges[iu];
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

    let mut imnew = vec![1usize; mesh.nmd + 1];
    let mut iudiv = vec![false; mesh.nud + 1];
    let mut imnext = 2usize;
    imnew[1] = 1;
    for im in 2..=mesh.nmd {
        imnew[im] = imnext;
        let neighbors = method_c_m_neighbors[im];
        for &iu in neighbors.iu.iter().take(neighbors.npoly) {
            if !iudiv[iu] {
                iudiv[iu] = true;
                let edge = mesh.u_edges[iu];
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

    let mut m_points = vec![CartesianPoint::new(0.0, 0.0, 0.0); nmd0 + 1];
    let mut m_metadata = default_method_c_m_metadata(nmd0);
    let mut u_edges = vec![IcosahedronUEdge::default(); nud0 + 1];
    let mut w_faces = vec![IcosahedronWFace::default(); nwd0 + 1];

    for im in 2..=mesh.nmd {
        let imn = imnew[im];
        m_points[imn] = mesh.m_points[im];
        m_metadata[imn] = mesh.m_metadata[im];
    }
    for iu in 2..=mesh.nud {
        let iun = iunew[iu];
        let old = mesh.u_edges[iu];
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
            m_points[im_mid] = weighted_point(m_points[im1], 1.0, m_points[im2], 1.0).unwrap();
        }
    }
    for iw in 2..=mesh.nwd {
        let iwn = iwnew[iw];
        let old = mesh.w_faces[iw];
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
            mesh.fill_method_c_full_subdivision(
                iw,
                &iwnew,
                &iunew,
                &imnew,
                2,
                &nest_wd,
                &nest_ud,
                &mut u_edges,
                &mut w_faces,
            )
            .expect("full Method-C face subdivision");
        }
    }

    let [p1, p2, p3] = [perimeter[0], perimeter[1], perimeter[2]];
    let [jm1, jm2, jm3] = [p1.im, p2.im, p3.im];
    let [ju1, ju2, ju3] = [p1.iu, p2.iu, p3.iu];
    let im16 = imnew[jm1];
    let im17 = nest_ud[ju1].im;
    let im18 = imnew[jm2];
    let im19 = imnew[jm3];
    let im20 = nest_ud[ju3].im;
    let iu43 = iunew[ju2];
    assert!(
        im17 > 1 && im20 > 1,
        "perim_fill3 test triple should have split endpoint M ids"
    );

    let (iu41, iu42, iu46, iw26, iw27) = if jm1 == mesh.u_edges[ju1].im[0] {
        (
            iunew[ju1],
            nest_ud[ju1].iu,
            iunew[mesh.u_edges[ju1].iu[4]],
            iwnew[mesh.u_edges[ju1].iw[2]],
            iwnew[mesh.u_edges[ju1].iw[0]],
        )
    } else {
        (
            nest_ud[ju1].iu,
            iunew[ju1],
            iunew[mesh.u_edges[ju1].iu[11]],
            iwnew[mesh.u_edges[ju1].iw[5]],
            iwnew[mesh.u_edges[ju1].iw[1]],
        )
    };
    let (iu49, iu50, iu34, iu35, iu48, iu51, iw6o, iw9o, iw6, iw9, iw29, iw20, iw28, iw30) =
        if jm2 == mesh.u_edges[ju2].im[0] {
            (
                iunew[mesh.u_edges[ju2].iu[0]],
                iunew[mesh.u_edges[ju2].iu[1]],
                iunew[mesh.u_edges[ju2].iu[2]],
                iunew[mesh.u_edges[ju2].iu[3]],
                iunew[mesh.u_edges[ju2].iu[4]],
                iunew[mesh.u_edges[ju2].iu[7]],
                mesh.u_edges[ju2].iw[4],
                mesh.u_edges[ju2].iw[5],
                iwnew[mesh.u_edges[ju2].iw[4]],
                iwnew[mesh.u_edges[ju2].iw[5]],
                iwnew[mesh.u_edges[ju2].iw[0]],
                iwnew[mesh.u_edges[ju2].iw[1]],
                iwnew[mesh.u_edges[ju2].iw[2]],
                iwnew[mesh.u_edges[ju2].iw[3]],
            )
        } else {
            (
                iunew[mesh.u_edges[ju2].iu[3]],
                iunew[mesh.u_edges[ju2].iu[2]],
                iunew[mesh.u_edges[ju2].iu[1]],
                iunew[mesh.u_edges[ju2].iu[0]],
                iunew[mesh.u_edges[ju2].iu[11]],
                iunew[mesh.u_edges[ju2].iu[8]],
                mesh.u_edges[ju2].iw[3],
                mesh.u_edges[ju2].iw[2],
                iwnew[mesh.u_edges[ju2].iw[3]],
                iwnew[mesh.u_edges[ju2].iw[2]],
                iwnew[mesh.u_edges[ju2].iw[1]],
                iwnew[mesh.u_edges[ju2].iw[0]],
                iwnew[mesh.u_edges[ju2].iw[5]],
                iwnew[mesh.u_edges[ju2].iw[4]],
            )
        };
    let (im21, iu44, iu45, iu53, iw31, iw32) = if jm3 == mesh.u_edges[ju3].im[0] {
        (
            imnew[mesh.u_edges[ju3].im[1]],
            iunew[ju3],
            nest_ud[ju3].iu,
            iunew[mesh.u_edges[ju3].iu[7]],
            iwnew[mesh.u_edges[ju3].iw[0]],
            iwnew[mesh.u_edges[ju3].iw[3]],
        )
    } else {
        (
            imnew[mesh.u_edges[ju3].im[0]],
            nest_ud[ju3].iu,
            iunew[ju3],
            iunew[mesh.u_edges[ju3].iu[8]],
            iwnew[mesh.u_edges[ju3].iw[1]],
            iwnew[mesh.u_edges[ju3].iw[4]],
        )
    };
    let im22 = canonical_other_endpoint_by_first(u_edges[iu46], im16);
    let im23 = canonical_other_endpoint_by_first(u_edges[iu48], im18);
    let im24 = canonical_other_endpoint_by_first(u_edges[iu49], im18);
    let im25 = canonical_other_endpoint_by_first(u_edges[iu51], im19);
    let im26 = canonical_other_endpoint_by_first(u_edges[iu53], im21);
    let im5 = if u_edges[iu34].im[0] == im18 {
        u_edges[iu34].im[1]
    } else {
        u_edges[iu34].im[0]
    };

    let [iu25, iu15] = method_c_split_outer_edges(nest_wd[iw6o], &u_edges, "iw6", jm2)
        .expect("split outer edges for iw6");
    let iw19 = if u_edges[iu25].iw[0] == iw6 {
        u_edges[iu25].iw[1]
    } else {
        u_edges[iu25].iw[0]
    };
    let iw7 = if u_edges[iu15].iw[0] == iw6 {
        u_edges[iu15].iw[1]
    } else {
        u_edges[iu15].iw[0]
    };
    let iu33 = if w_faces[iw19].iu[0] == iu25 {
        w_faces[iw19].iu[1]
    } else if w_faces[iw19].iu[1] == iu25 {
        w_faces[iw19].iu[2]
    } else {
        w_faces[iw19].iu[0]
    };
    let im12 = if u_edges[iu25].iw[0] == iw6 {
        u_edges[iu25].im[1]
    } else {
        u_edges[iu25].im[0]
    };
    let [iu16, iu26] = method_c_split_outer_edges(nest_wd[iw9o], &u_edges, "iw9", jm2)
        .expect("split outer edges for iw9");
    let iw8 = if u_edges[iu16].iw[0] == iw9 {
        u_edges[iu16].iw[1]
    } else {
        u_edges[iu16].iw[0]
    };
    let iw21 = if u_edges[iu26].iw[0] == iw9 {
        u_edges[iu26].iw[1]
    } else {
        u_edges[iu26].iw[0]
    };
    let im13 = if u_edges[iu26].iw[0] == iw9 {
        u_edges[iu26].im[0]
    } else {
        u_edges[iu26].im[1]
    };

    let pre_points = m_points.clone();
    let expected_im19 = weighted_point(pre_points[im24], 1.0, pre_points[im5], 1.0).unwrap();
    let expected_im18 = weighted_point(expected_im19, 1.0, pre_points[im5], 1.0).unwrap();
    let expected_im17 = weighted_point(pre_points[im17], 0.75, expected_im19, 0.25).unwrap();
    let expected_im20 = weighted_point(pre_points[im20], 0.75, expected_im19, 0.25).unwrap();
    let expected_im12 = weighted_point(pre_points[im12], 0.833, expected_im18, 0.167).unwrap();
    let expected_im13 = weighted_point(pre_points[im13], 0.833, expected_im18, 0.167).unwrap();
    let parent_level = selected_parent_mrl.unwrap_or(1);
    let expected_im17_mrlm_orig = m_metadata[im18].mrlm_orig;
    let expected_im20_mrlm_orig = m_metadata[im19].mrlm_orig;
    let expected_neighbor_ownership = [im22, im23, im24, im25, im26]
        .map(|im| (im, m_metadata[im].mrlm, m_metadata[im].mrlm_orig));
    let mut expected_iw8_iu = w_faces[iw8].iu;
    if expected_iw8_iu[0] == iu16 {
        expected_iw8_iu[2] = iu34;
    } else if expected_iw8_iu[1] == iu16 {
        expected_iw8_iu[0] = iu34;
    } else {
        expected_iw8_iu[1] = iu34;
    }
    let mut expected_iw19_iu = w_faces[iw19].iu;
    if expected_iw19_iu[0] == iu25 {
        expected_iw19_iu[2] = iu35;
    } else if expected_iw19_iu[1] == iu25 {
        expected_iw19_iu[0] = iu35;
    } else {
        expected_iw19_iu[1] = iu35;
    }
    let mut expected_iw20_iu = w_faces[iw20].iu;
    if expected_iw20_iu[0] == iu43 {
        expected_iw20_iu[1] = iu42;
        expected_iw20_iu[2] = iu49;
    } else if expected_iw20_iu[1] == iu43 {
        expected_iw20_iu[2] = iu42;
        expected_iw20_iu[0] = iu49;
    } else {
        expected_iw20_iu[0] = iu42;
        expected_iw20_iu[1] = iu49;
    }
    let mut expected_iw27_iu = w_faces[iw27].iu;
    if expected_iw27_iu[0] == iu48 {
        expected_iw27_iu[1] = iu41;
    } else if expected_iw27_iu[1] == iu48 {
        expected_iw27_iu[2] = iu41;
    } else {
        expected_iw27_iu[0] = iu41;
    }
    let mut expected_iw29_iu = w_faces[iw29].iu;
    if expected_iw29_iu[0] == iu50 {
        expected_iw29_iu[1] = iu44;
        expected_iw29_iu[2] = iu43;
    } else if expected_iw29_iu[1] == iu50 {
        expected_iw29_iu[2] = iu44;
        expected_iw29_iu[0] = iu43;
    } else {
        expected_iw29_iu[0] = iu44;
        expected_iw29_iu[1] = iu43;
    }
    let mut expected_iw31_iu = w_faces[iw31].iu;
    if expected_iw31_iu[0] == iu51 {
        expected_iw31_iu[2] = iu45;
    } else if expected_iw31_iu[1] == iu51 {
        expected_iw31_iu[0] = iu45;
    } else {
        expected_iw31_iu[1] = iu45;
    }
    let mut expected_iu34 = u_edges[iu34];
    if expected_iu34.im[0] == im18 {
        expected_iu34.iw = set_first_two(expected_iu34.iw, iw8, iw7);
    } else {
        expected_iu34.iw = set_first_two(expected_iu34.iw, iw7, iw8);
    }
    let mut expected_iu35 = u_edges[iu35];
    if expected_iu35.im[0] == im19 {
        expected_iu35.iw[1] = iw19;
        expected_iu35.iw[0] = iw21;
        expected_iu35.im[1] = im18;
    } else {
        expected_iu35.iw[0] = iw19;
        expected_iu35.iw[1] = iw21;
        expected_iu35.im[0] = im18;
    }
    let mut expected_iu41 = u_edges[iu41];
    if expected_iu41.im[1] == im17 {
        expected_iu41.iw[0] = iw27;
    } else {
        expected_iu41.iw[1] = iw27;
    }
    let mut expected_iu42 = u_edges[iu42];
    if expected_iu42.im[0] == im17 {
        expected_iu42.im[1] = im19;
        expected_iu42.iw[0] = iw20;
    } else {
        expected_iu42.im[0] = im19;
        expected_iu42.iw[1] = iw20;
    }
    let mut expected_iu43 = u_edges[iu43];
    if expected_iu43.im[1] == im19 {
        expected_iu43.im[0] = im24;
    } else {
        expected_iu43.im[1] = im24;
    }
    let mut expected_iu44 = u_edges[iu44];
    if expected_iu44.im[0] == im19 {
        expected_iu44.iw[0] = iw29;
    } else {
        expected_iu44.iw[1] = iw29;
    }
    let mut expected_iu45 = u_edges[iu45];
    if expected_iu45.im[0] == im20 {
        expected_iu45.iw[0] = iw31;
    } else {
        expected_iu45.iw[1] = iw31;
    }
    let mut expected_iu48 = u_edges[iu48];
    if expected_iu48.iw[1] == iw27 {
        expected_iu48.im[1] = im17;
    } else {
        expected_iu48.im[0] = im17;
    }
    let mut expected_iu49 = u_edges[iu49];
    if expected_iu49.im[1] == im24 {
        expected_iu49.im[0] = im17;
        expected_iu49.iw[1] = iw20;
    } else {
        expected_iu49.im[1] = im17;
        expected_iu49.iw[0] = iw20;
    }
    let mut expected_iu50 = u_edges[iu50];
    if expected_iu50.im[0] == im24 {
        expected_iu50.im[1] = im20;
    } else {
        expected_iu50.im[0] = im20;
    }
    let mut expected_iu51 = u_edges[iu51];
    if expected_iu51.iw[1] == iw31 {
        expected_iu51.im[0] = im20;
    } else {
        expected_iu51.im[1] = im20;
    }
    let mut expected_iu33 = u_edges[iu33];
    if expected_iu33.iw[1] == iw19 {
        expected_iu33.im[1] = im19;
    } else {
        expected_iu33.im[0] = im19;
    }

    let radius = active_mesh_radius(&mesh).expect("active mesh radius");
    mesh.perim_fill3_method_c(
        &perimeter[0..3],
        parent_level,
        &iwnew,
        &iunew,
        &imnew,
        &nest_wd,
        &mut nest_ud,
        &mut u_edges,
        &mut w_faces,
        &mut m_points,
        &mut m_metadata,
        radius,
        2,
    )
    .expect("perim_fill3 first transition triple");

    let assert_point = |label: &str, actual: CartesianPoint, expected: CartesianPoint| {
        let delta = magnitude(CartesianPoint::new(
            actual.x - expected.x,
            actual.y - expected.y,
            actual.z - expected.z,
        ));
        assert!(
            delta < 1.0e-9,
            "{label} should match Canonical perim_fill3 weighted coordinate formula; delta={delta}"
        );
    };
    assert_point("im19", m_points[im19], expected_im19);
    assert_point("im18", m_points[im18], expected_im18);
    assert_point("im17", m_points[im17], expected_im17);
    assert_point("im20", m_points[im20], expected_im20);
    assert_point("im12", m_points[im12], expected_im12);
    assert_point("im13", m_points[im13], expected_im13);
    assert_eq!(m_metadata[im17].mrlm_orig, expected_im17_mrlm_orig);
    assert_eq!(m_metadata[im20].mrlm_orig, expected_im20_mrlm_orig);
    assert_eq!(m_metadata[im18].mrlm_orig, parent_level + 1);
    assert_eq!(m_metadata[im19].mrlm_orig, parent_level + 1);
    for (im, expected_mrlm, expected_mrlm_orig) in expected_neighbor_ownership {
        assert_eq!(m_metadata[im].ngr, 2);
        assert_eq!(
                m_metadata[im].mrlm, expected_mrlm,
                "Canonical perim_fill3 sets ngr for transition neighbor M {im} without changing mrlm ownership"
            );
        assert_eq!(
                m_metadata[im].mrlm_orig, expected_mrlm_orig,
                "Canonical perim_fill3 sets ngr for transition neighbor M {im} without changing mrlm_orig ownership"
            );
    }
    for iw in [iw20, iw26, iw27, iw28, iw29, iw30, iw31, iw32] {
        assert_eq!(w_faces[iw].ngr, 2);
    }
    let has_edge = |iw: usize, iu: usize| w_faces[iw].iu.iter().take(3).any(|&edge| edge == iu);
    assert!(has_edge(iw8, iu34));
    assert!(has_edge(iw19, iu35));
    assert!(has_edge(iw20, iu42) && has_edge(iw20, iu49));
    assert!(has_edge(iw27, iu41));
    assert!(has_edge(iw29, iu44) && has_edge(iw29, iunew[ju2]));
    assert!(has_edge(iw31, iu45));
    assert_eq!(w_faces[iw8].iu, expected_iw8_iu);
    assert_eq!(w_faces[iw19].iu, expected_iw19_iu);
    assert_eq!(w_faces[iw20].iu, expected_iw20_iu);
    assert_eq!(w_faces[iw27].iu, expected_iw27_iu);
    assert_eq!(w_faces[iw29].iu, expected_iw29_iu);
    assert_eq!(w_faces[iw31].iu, expected_iw31_iu);
    for (iu, expected) in [
        (iu33, expected_iu33),
        (iu34, expected_iu34),
        (iu35, expected_iu35),
        (iu41, expected_iu41),
        (iu42, expected_iu42),
        (iu43, expected_iu43),
        (iu44, expected_iu44),
        (iu45, expected_iu45),
        (iu48, expected_iu48),
        (iu49, expected_iu49),
        (iu50, expected_iu50),
        (iu51, expected_iu51),
    ] {
        assert_eq!(
            u_edges[iu].im, expected.im,
            "Canonical perim_fill3 should preserve exact endpoint slot order for U edge {iu}"
        );
        assert_eq!(
            [u_edges[iu].iw[0], u_edges[iu].iw[1]],
            [expected.iw[0], expected.iw[1]],
            "Canonical perim_fill3 should preserve exact adjacent-W slot order for U edge {iu}"
        );
    }
    let has_m_endpoint = |iu: usize, im: usize| u_edges[iu].im.contains(&im);
    assert!(has_m_endpoint(iu15, im18));
    assert!(has_m_endpoint(iu16, im18));
    assert!(has_m_endpoint(iu25, im18));
    assert!(has_m_endpoint(iu26, im18));
    assert!(has_m_endpoint(iu33, im19));
    assert!(has_m_endpoint(iu35, im18) && has_m_endpoint(iu35, im19));
    assert!(has_m_endpoint(iu42, im17) && has_m_endpoint(iu42, im19));
    assert!(has_m_endpoint(iunew[ju2], im19) && has_m_endpoint(iunew[ju2], im24));
    assert!(has_m_endpoint(iu48, im17));
    assert!(has_m_endpoint(iu49, im17) && has_m_endpoint(iu49, im24));
    assert!(has_m_endpoint(iu50, im24) && has_m_endpoint(iu50, im20));
    assert!(has_m_endpoint(iu51, im20));
    let has_w_face = |iu: usize, iw: usize| u_edges[iu].iw.iter().take(2).any(|&face| face == iw);
    assert!(has_w_face(iu41, iw27));
    assert!(has_w_face(iu42, iw20));
    assert!(has_w_face(iu44, iw29));
    assert!(has_w_face(iu45, iw31));
    assert!(has_w_face(iu49, iw20));
}
