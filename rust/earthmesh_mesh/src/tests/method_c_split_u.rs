use super::*;

#[test]
fn method_c_split_u_second_half_ids_follow_canonical_iunew_order() {
    let mesh = MethodCMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let mut selected = mesh
        .selected_region_faces(&region, 1, false)
        .expect("selected Method-C faces");
    let method_c_m_neighbors = mesh
        .derive_icosahedron_m_neighbors_canonical()
        .expect("Method-C M neighbors");
    mesh.close_method_c_concavities_for_level_with_neighbors(&mut selected, &method_c_m_neighbors)
        .expect("Method-C closure");

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

    let mut iunew = vec![1usize; mesh.nud + 1];
    let mut expected_second_u = vec![1usize; mesh.nud + 1];
    let mut iwdiv = vec![false; mesh.nwd + 1];
    let mut iunext = 2usize;
    iunew[1] = 1;
    for iu in 2..=mesh.nud {
        iunew[iu] = iunext;
        let edge = mesh.u_edges[iu];
        let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
        if nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided() {
            if nest_wd[iw1].is_suppressed() || nest_wd[iw2].is_suppressed() {
                expected_second_u[iu] = iunew[iu];
            } else {
                iunext += 1;
                expected_second_u[iu] = iunext;
            }
        }

        for &iw in &edge.iw[0..2] {
            if !iwdiv[iw] {
                iwdiv[iw] = true;
                if nest_wd[iw].is_subdivided() {
                    iunext += 3;
                }
            }
        }
        iunext += 1;
    }

    let mut imnew = vec![1usize; mesh.nmd + 1];
    let mut expected_midpoint_m = vec![1usize; mesh.nud + 1];
    let mut iudiv = vec![false; mesh.nud + 1];
    let mut imnext = 2usize;
    imnew[1] = 1;
    for im in 2..=mesh.nmd {
        imnew[im] = imnext;
        for &iu in method_c_m_neighbors[im]
            .iu
            .iter()
            .take(method_c_m_neighbors[im].npoly)
        {
            if iudiv[iu] {
                continue;
            }
            iudiv[iu] = true;
            let edge = mesh.u_edges[iu];
            let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
            if nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided() {
                if nest_wd[iw1].is_suppressed() || nest_wd[iw2].is_suppressed() {
                    expected_midpoint_m[iu] = 1;
                } else {
                    imnext += 1;
                    expected_midpoint_m[iu] = imnext;
                }
            }
        }
        imnext += 1;
    }

    let refined = mesh
        .spawn_nest_pass_with_max_mrows(&selected, 2, 7, true)
        .expect("Method-C pass");
    let mut checked = 0usize;
    for iu in 2..=mesh.nud {
        if expected_second_u[iu] == 1 || expected_second_u[iu] == iunew[iu] {
            continue;
        }
        let old = mesh.u_edges[iu];
        let [iw1, iw2] = [old.iw[0], old.iw[1]];
        if !(nest_wd[iw1].is_subdivided() && nest_wd[iw2].is_subdivided()) {
            continue;
        }
        let midpoint = expected_midpoint_m[iu];
        let remapped_im1 = imnew[old.im[0]];
        let remapped_im2 = imnew[old.im[1]];
        let first_half = refined.u_edges[iunew[iu]].im;
        let second_half = refined.u_edges[expected_second_u[iu]].im;
        assert!(
            first_half.contains(&midpoint) || second_half.contains(&midpoint),
            "Canonical split-U {iu} should connect a half-edge to midpoint M id {midpoint}"
        );
        assert!(
            first_half.contains(&remapped_im1)
                || first_half.contains(&remapped_im2)
                || second_half.contains(&remapped_im1)
                || second_half.contains(&remapped_im2),
            "Canonical split-U {iu} half-edges should retain a remapped old endpoint"
        );
        let midpoint_count = first_half
            .into_iter()
            .chain(second_half)
            .filter(|&endpoint| endpoint == midpoint)
            .count();
        assert_eq!(
            midpoint_count, 2,
            "Canonical split-U {iu} half-edges should share midpoint M id {midpoint}"
        );
        checked += 1;
    }

    assert!(
        checked > 0,
        "test should exercise non-suppressed split-U second halves"
    );
}

#[test]
fn method_c_split_u_m_metadata_marks_child_ownership() {
    let mesh = MethodCMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let mut selected = mesh
        .selected_region_faces(&region, 1, false)
        .expect("selected Method-C faces");
    let method_c_m_neighbors = mesh
        .derive_icosahedron_m_neighbors_canonical()
        .expect("Method-C M neighbors");
    mesh.close_method_c_concavities_for_level_with_neighbors(&mut selected, &method_c_m_neighbors)
        .expect("Method-C closure");

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

    let mut imnew = vec![1usize; mesh.nmd + 1];
    let mut expected_midpoint_m = vec![1usize; mesh.nud + 1];
    let mut iudiv = vec![false; mesh.nud + 1];
    let mut imnext = 2usize;
    imnew[1] = 1;
    for im in 2..=mesh.nmd {
        imnew[im] = imnext;
        for &iu in method_c_m_neighbors[im]
            .iu
            .iter()
            .take(method_c_m_neighbors[im].npoly)
        {
            if iudiv[iu] {
                continue;
            }
            iudiv[iu] = true;
            let edge = mesh.u_edges[iu];
            let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
            if (nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided())
                && !(nest_wd[iw1].is_suppressed() || nest_wd[iw2].is_suppressed())
            {
                imnext += 1;
                expected_midpoint_m[iu] = imnext;
            }
        }
        imnext += 1;
    }

    let refined = mesh
        .spawn_nest_pass_with_max_mrows(&selected, 2, 7, true)
        .expect("Method-C pass");
    let mut checked = 0usize;
    for iu in 2..=mesh.nud {
        let midpoint = expected_midpoint_m[iu];
        if midpoint <= 1 {
            continue;
        }
        let old = mesh.u_edges[iu];
        let [iw1, iw2] = [old.iw[0], old.iw[1]];
        if !(nest_wd[iw1].is_subdivided() && nest_wd[iw2].is_subdivided()) {
            continue;
        }
        for &old_im in &old.im {
            let remapped = imnew[old_im];
            assert_eq!(
                refined.m_metadata[remapped].mrlm, 2,
                "Canonical Method-C split-U {iu} raises old endpoint M {old_im} to child mrlm"
            );
            assert_eq!(
                    refined.m_metadata[remapped].ngr, 2,
                    "Canonical Method-C split-U {iu} marks old endpoint M {old_im} with child grid ownership"
                );
        }
        assert_eq!(
            refined.m_metadata[midpoint].mrlm, 2,
            "Canonical Method-C split-U {iu} gives new midpoint M {midpoint} child mrlm"
        );
        assert_eq!(
                refined.m_metadata[midpoint].mrlm_orig, 2,
                "Canonical Method-C split-U {iu} gives new midpoint M {midpoint} child original ownership"
            );
        assert_eq!(
                refined.m_metadata[midpoint].ngr, 2,
                "Canonical Method-C split-U {iu} marks new midpoint M {midpoint} with child grid ownership"
            );
        checked += 1;
    }

    assert!(
        checked > 0,
        "test should exercise non-suppressed split-U M metadata"
    );
}

#[test]
fn method_c_suppressed_split_u_reuses_original_u_and_skips_midpoint_like_canonical() {
    let mesh = MethodCMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let mut selected = mesh
        .selected_region_faces(&region, 1, false)
        .expect("selected Method-C faces");
    let method_c_m_neighbors = mesh
        .derive_icosahedron_m_neighbors_canonical()
        .expect("Method-C M neighbors");
    mesh.close_method_c_concavities_for_level_with_neighbors(&mut selected, &method_c_m_neighbors)
        .expect("Method-C closure");

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

    let mut iunew = vec![1usize; mesh.nud + 1];
    let mut expected_second_u = vec![1usize; mesh.nud + 1];
    let mut iwdiv = vec![false; mesh.nwd + 1];
    let mut iunext = 2usize;
    iunew[1] = 1;
    for iu in 2..=mesh.nud {
        iunew[iu] = iunext;
        let edge = mesh.u_edges[iu];
        let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
        if nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided() {
            if nest_wd[iw1].is_suppressed() || nest_wd[iw2].is_suppressed() {
                expected_second_u[iu] = iunew[iu];
            } else {
                iunext += 1;
                expected_second_u[iu] = iunext;
            }
        }

        for &iw in &edge.iw[0..2] {
            if !iwdiv[iw] {
                iwdiv[iw] = true;
                if nest_wd[iw].is_subdivided() {
                    iunext += 3;
                }
            }
        }
        iunext += 1;
    }

    let mut expected_midpoint_m = vec![1usize; mesh.nud + 1];
    let mut iudiv = vec![false; mesh.nud + 1];
    let mut imnext = 2usize;
    for im in 2..=mesh.nmd {
        for &iu in method_c_m_neighbors[im]
            .iu
            .iter()
            .take(method_c_m_neighbors[im].npoly)
        {
            if iudiv[iu] {
                continue;
            }
            iudiv[iu] = true;
            let edge = mesh.u_edges[iu];
            let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
            if nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided() {
                if nest_wd[iw1].is_suppressed() || nest_wd[iw2].is_suppressed() {
                    expected_midpoint_m[iu] = 1;
                } else {
                    imnext += 1;
                    expected_midpoint_m[iu] = imnext;
                }
            }
        }
        imnext += 1;
    }

    let refined = mesh
        .spawn_nest_pass_with_max_mrows(&selected, 2, 7, true)
        .expect("Method-C pass");
    let mut checked = 0usize;
    for iu in 2..=mesh.nud {
        let old = mesh.u_edges[iu];
        let [iw1, iw2] = [old.iw[0], old.iw[1]];
        if !(nest_wd[iw1].is_suppressed() || nest_wd[iw2].is_suppressed()) {
            continue;
        }
        if !(nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided()) {
            continue;
        }
        assert_eq!(
            expected_second_u[iu], iunew[iu],
            "Canonical suppressed split-U {iu} reuses iunew(iu) instead of allocating a second half"
        );
        assert_eq!(
            expected_midpoint_m[iu], 1,
            "Canonical suppressed split-U {iu} sets nest_ud(iu)%im = 1"
        );
        assert!(
            !refined.u_edges[iunew[iu]].im.contains(&1),
            "suppressed split-U {iu} should not canonical a new midpoint M id"
        );
        checked += 1;
    }

    assert!(
        checked > 0,
        "test should exercise suppressed Method-C split-U edges"
    );
}
