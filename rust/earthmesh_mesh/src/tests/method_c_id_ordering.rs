use super::*;

#[test]
fn method_c_child_w_ids_follow_canonical_parent_then_three_children_order() {
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

    let mut iwnew = vec![1usize; mesh.nwd + 1];
    let mut expected_child_w = vec![[1usize; 3]; mesh.nwd + 1];
    let mut iwnext = 2usize;
    iwnew[1] = 1;
    for iw in 2..=mesh.nwd {
        iwnew[iw] = iwnext;
        if nest_wd[iw].is_subdivided() {
            iwnext += 1;
            expected_child_w[iw][0] = iwnext;
            iwnext += 1;
            expected_child_w[iw][1] = iwnext;
            iwnext += 1;
            expected_child_w[iw][2] = iwnext;
        }
        iwnext += 1;
    }

    let refined = mesh
        .spawn_nest_pass_with_max_mrows(&selected, 2, 7, true)
        .expect("Method-C pass");
    let mut checked = 0usize;
    for iw in 2..=mesh.nwd {
        if !nest_wd[iw].is_subdivided() {
            continue;
        }
        let parent_id = iwnew[iw];
        assert_eq!(
            expected_child_w[iw],
            [parent_id + 1, parent_id + 2, parent_id + 3],
            "Canonical iwnew places three child W ids immediately after parent W {iw}"
        );
        assert_eq!(refined.w_faces[parent_id].mrlw, mesh.w_faces[iw].mrlw + 1);
        assert_eq!(
            refined.w_faces[parent_id].mrlw_orig, mesh.w_faces[iw].mrlw_orig,
            "Canonical promotes remapped full-subdivision parent W mrlw but preserves mrlw_orig"
        );
        assert_eq!(refined.w_faces[parent_id].ngr, 2);
        for child_id in expected_child_w[iw] {
            assert_eq!(refined.w_faces[child_id].mrlw, mesh.w_faces[iw].mrlw + 1);
            assert_eq!(
                refined.w_faces[child_id].mrlw_orig,
                mesh.w_faces[iw].mrlw + 1
            );
            assert_eq!(refined.w_faces[child_id].ngr, 2);
            assert!(
                    refined.w_faces[child_id].im.iter().all(|&im| im > 1),
                    "Canonical tri_neighbors should rebuild child W {child_id} M vertices from Method-C U endpoints"
                );
            for &iu in &refined.w_faces[child_id].iu {
                assert!(
                    refined.u_edges[iu]
                        .im
                        .iter()
                        .all(|endpoint| refined.w_faces[child_id].im.contains(endpoint)),
                    "child W {child_id} U edge {iu} should use only that W face's M vertices"
                );
            }
            checked += 1;
        }
    }

    assert!(
        checked > 0,
        "test should exercise subdivided Method-C W faces"
    );
}

#[test]
fn method_c_midpoint_m_ids_follow_canonical_first_seen_edge_order() {
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
        .spawn_nest_pass_with_max_mrows(&selected, 2, 7, false)
        .expect("Method-C pass without final projection");
    let checked = (2..=mesh.nud)
            .filter(|&iu| expected_midpoint_m[iu] > 1)
            .filter(|&iu| {
                let edge = mesh.u_edges[iu];
                let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
                nest_wd[iw1].is_subdivided() && nest_wd[iw2].is_subdivided()
            })
            .map(|iu| {
                let old_edge = mesh.u_edges[iu];
                let midpoint = expected_midpoint_m[iu];
                let remapped_im1 = imnew[old_edge.im[0]];
                let remapped_im2 = imnew[old_edge.im[1]];
                let has_half_edge = |endpoint: usize| {
                    refined.u_edges.iter().skip(2).any(|edge| {
                        edge.im.contains(&midpoint) && edge.im.contains(&endpoint)
                    })
                };
                assert!(
                    has_half_edge(remapped_im1) && has_half_edge(remapped_im2),
                    "Canonical assigns split-U {iu} midpoint to first-seen M id {midpoint} and connects both remapped endpoints"
                );
                1usize
            })
            .sum::<usize>();

    assert!(
        checked > 0,
        "test should exercise Method-C split-U midpoint ids"
    );
}
