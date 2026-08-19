use super::*;

#[test]
fn method_c_internal_u_ids_follow_canonical_first_seen_w_order() {
    let mesh = MethodCMesh::from_icosahedron(16, 0, 1.0, 0.25).expect("base Method-C mesh");
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

    let mut expected_internal_u = vec![[1usize; 3]; mesh.nwd + 1];
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
            if !nest_wd[iw1].is_suppressed() && !nest_wd[iw2].is_suppressed() {
                iunext += 1;
                expected_second_u[iu] = iunext;
            } else {
                expected_second_u[iu] = iunew[iu];
            }
        }
        for &iw in &edge.iw[0..2] {
            if !iwdiv[iw] {
                iwdiv[iw] = true;
                if nest_wd[iw].is_subdivided() {
                    iunext += 1;
                    expected_internal_u[iw][0] = iunext;
                    iunext += 1;
                    expected_internal_u[iw][1] = iunext;
                    iunext += 1;
                    expected_internal_u[iw][2] = iunext;
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
    for iw in 2..=mesh.nwd {
        if !nest_wd[iw].is_subdivided() {
            continue;
        }
        if !mesh.w_faces[iw]
            .iw
            .iter()
            .take(3)
            .all(|&neighbor| neighbor > 1 && nest_wd[neighbor].is_subdivided())
        {
            continue;
        }
        let parent_id = iwnew[iw];
        assert_eq!(
            refined.w_faces[parent_id].iu, expected_internal_u[iw],
            "Canonical writes nest_wd(iw)%iu(1:3) to the remapped parent W face {iw}"
        );
        for (slot, child_id) in expected_child_w[iw].into_iter().enumerate() {
            assert_eq!(
                refined.w_faces[child_id].iu[0], expected_internal_u[iw][slot],
                "Canonical writes internal U edge {} as child W {child_id}'s first U",
                expected_internal_u[iw][slot]
            );
            checked += 1;
        }
        let midpoint_ids = mesh.w_faces[iw].iu.map(|iu| expected_midpoint_m[iu]);
        let mut actual_pairs = expected_internal_u[iw]
            .into_iter()
            .map(|iu| {
                let mut endpoints = refined.u_edges[iu].im;
                endpoints.sort_unstable();
                endpoints
            })
            .collect::<Vec<_>>();
        actual_pairs.sort_unstable();
        let mut expected_pairs = [
            [midpoint_ids[0], midpoint_ids[1]],
            [midpoint_ids[0], midpoint_ids[2]],
            [midpoint_ids[1], midpoint_ids[2]],
        ];
        for pair in &mut expected_pairs {
            pair.sort_unstable();
        }
        expected_pairs.sort_unstable();
        assert_eq!(
                actual_pairs,
                expected_pairs,
                "Canonical full-subdivision internal U edges connect the three split-edge midpoint M ids for W face {iw}"
            );
        let mut actual_parent_vertices = refined.w_faces[parent_id].im;
        actual_parent_vertices.sort_unstable();
        let mut expected_parent_vertices = midpoint_ids;
        expected_parent_vertices.sort_unstable();
        assert_eq!(
                actual_parent_vertices,
                expected_parent_vertices,
                "Canonical full-subdivision remapped parent W face {parent_id} should be the central midpoint triangle for old W face {iw}"
            );
        for &iu in &refined.w_faces[parent_id].iu {
            assert!(
                refined.u_edges[iu]
                    .im
                    .iter()
                    .all(|endpoint| refined.w_faces[parent_id].im.contains(endpoint)),
                "central remapped W face {parent_id} U edge {iu} should use only midpoint vertices"
            );
        }
        let w_family = [
            parent_id,
            expected_child_w[iw][0],
            expected_child_w[iw][1],
            expected_child_w[iw][2],
        ];
        for &iu in &mesh.w_faces[iw].iu {
            assert!(
                    refined.u_edges[iunew[iu]]
                        .iw
                        .iter()
                        .take(2)
                        .any(|face| w_family.contains(face)),
                    "Canonical remapped first half of split-U {iu} should touch W face family for subdivided W {iw}"
                );
            assert!(
                    refined.u_edges[expected_second_u[iu]]
                        .iw
                        .iter()
                        .take(2)
                        .any(|face| w_family.contains(face)),
                    "Canonical second half of split-U {iu} should touch W face family for subdivided W {iw}"
                );
        }
        let expected_split_child_faces = [
            if iw == mesh.u_edges[mesh.w_faces[iw].iu[0]].iw[0] {
                (expected_child_w[iw][2], expected_child_w[iw][1])
            } else {
                (expected_child_w[iw][1], expected_child_w[iw][2])
            },
            if iw == mesh.u_edges[mesh.w_faces[iw].iu[1]].iw[0] {
                (expected_child_w[iw][0], expected_child_w[iw][2])
            } else {
                (expected_child_w[iw][2], expected_child_w[iw][0])
            },
            if iw == mesh.u_edges[mesh.w_faces[iw].iu[2]].iw[0] {
                (expected_child_w[iw][1], expected_child_w[iw][0])
            } else {
                (expected_child_w[iw][0], expected_child_w[iw][1])
            },
        ];
        for (slot, &iu) in mesh.w_faces[iw].iu.iter().enumerate() {
            let (first_half_child, second_half_child) = expected_split_child_faces[slot];
            assert!(
                    refined.u_edges[iunew[iu]]
                        .iw
                        .iter()
                        .take(2)
                        .any(|&face| face == first_half_child),
                    "Canonical full-subdivision split-U {iu} first half should touch child W {first_half_child} for old W {iw} edge slot {slot}"
                );
            assert!(
                    refined.u_edges[expected_second_u[iu]]
                        .iw
                        .iter()
                        .take(2)
                        .any(|&face| face == second_half_child),
                    "Canonical full-subdivision split-U {iu} second half should touch child W {second_half_child} for old W {iw} edge slot {slot}"
                );
        }
        let [iu1o, iu2o, iu3o] = mesh.w_faces[iw].iu;
        let expected_child_iu = [
            [
                expected_internal_u[iw][0],
                if iw == mesh.u_edges[iu2o].iw[0] {
                    iunew[iu2o]
                } else {
                    expected_second_u[iu2o]
                },
                if iw == mesh.u_edges[iu3o].iw[0] {
                    expected_second_u[iu3o]
                } else {
                    iunew[iu3o]
                },
            ],
            [
                expected_internal_u[iw][1],
                if iw == mesh.u_edges[iu3o].iw[0] {
                    iunew[iu3o]
                } else {
                    expected_second_u[iu3o]
                },
                if iw == mesh.u_edges[iu1o].iw[0] {
                    expected_second_u[iu1o]
                } else {
                    iunew[iu1o]
                },
            ],
            [
                expected_internal_u[iw][2],
                if iw == mesh.u_edges[iu1o].iw[0] {
                    iunew[iu1o]
                } else {
                    expected_second_u[iu1o]
                },
                if iw == mesh.u_edges[iu2o].iw[0] {
                    expected_second_u[iu2o]
                } else {
                    iunew[iu2o]
                },
            ],
        ];
        for (slot, child_id) in expected_child_w[iw].into_iter().enumerate() {
            assert_eq!(
                    refined.w_faces[child_id].iu,
                    expected_child_iu[slot],
                    "Canonical ltab_wd child W {child_id} should preserve exact Method-C U-edge slot order for old W {iw}"
                );
        }
    }

    assert!(
        checked > 0,
        "test should exercise interior full-subdivision W faces"
    );
}
