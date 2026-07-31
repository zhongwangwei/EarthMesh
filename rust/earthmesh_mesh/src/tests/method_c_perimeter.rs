use super::super::*;

#[test]
fn method_c_suppresses_center_perimeter_segment_faces_like_canonical() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let region = MethodCRefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let selected = mesh
        .selected_region_faces(&region, 1, false)
        .expect("selected Method-C faces");
    let method_c_m_neighbors = mesh
        .derive_icosahedron_m_neighbors_canonical()
        .expect("Method-C M neighbors");
    let mut nest_wd = vec![MethodCNestWd::default(); mesh.nwd + 1];
    for iw in 2..=mesh.nwd {
        if selected[iw] {
            nest_wd[iw].iw[2] = 1;
        }
    }
    let perimeter = mesh
        .perim_map2_method_c(&nest_wd, &method_c_m_neighbors)
        .expect("Method-C perimeter");
    assert_eq!(
        perimeter.len() % 3,
        0,
        "Canonical Method-C suppression consumes perimeter points in triples"
    );
    let expected_start = (2..=mesh.nmd)
        .find(|&im| {
            let neighbors = method_c_m_neighbors[im];
            neighbors
                .iw
                .iter()
                .take(neighbors.npoly)
                .filter(|&&iw| nest_wd[iw].is_subdivided())
                .count()
                == 2
        })
        .expect("Canonical perim_map2 start point");
    assert_eq!(
        perimeter[0].im, expected_start,
        "Canonical perim_map2 starts from the first original M point with nwdiv == 2"
    );
    for index in 0..perimeter.len() {
        let point = perimeter[index];
        let next = perimeter[(index + 1) % perimeter.len()].im;
        let edge = mesh.u_edges[point.iu];
        let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
        if edge.im[0] == point.im {
            assert!(
                    nest_wd[iw1].flag() == 0 && nest_wd[iw2].is_subdivided(),
                    "Canonical perim_ngr advances from im(1) only when iw(1) is outside and iw(2) is inside"
                );
            assert_eq!(
                next, edge.im[1],
                "Canonical perim_ngr next M point from im(1) is im(2)"
            );
        } else {
            assert_eq!(
                edge.im[1], point.im,
                "Canonical perim_ngr perimeter U edge must contain the current M point"
            );
            assert!(
                    nest_wd[iw2].flag() == 0 && nest_wd[iw1].is_subdivided(),
                    "Canonical perim_ngr advances from im(2) only when iw(2) is outside and iw(1) is inside"
                );
            assert_eq!(
                next, edge.im[0],
                "Canonical perim_ngr next M point from im(2) is im(1)"
            );
        }
    }
    for point in &perimeter {
        let neighbors = method_c_m_neighbors[point.im];
        let mut expected_nwdiv = 0usize;
        let mut expected_near_pentagon = false;
        for j in 0..neighbors.npoly {
            let iw = neighbors.iw[j];
            if nest_wd[iw].is_subdivided() {
                expected_nwdiv += 1;
            }

            let iu = neighbors.iu[j];
            let edge = mesh.u_edges[iu];
            let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
            if nest_wd[iw1].flag() == 0 && nest_wd[iw2].flag() == 0 {
                if point.im == edge.im[0] && method_c_m_neighbors[edge.im[1]].npoly == 5 {
                    expected_near_pentagon = true;
                }
                if point.im == edge.im[1] && method_c_m_neighbors[edge.im[0]].npoly == 5 {
                    expected_near_pentagon = true;
                }
            }
        }

        assert_eq!(
            point.npoly, neighbors.npoly,
            "Canonical perim_map2 stores npolyper for each perimeter M point"
        );
        assert_eq!(
            point.nwdiv, expected_nwdiv,
            "Canonical perim_map2 stores nwdivper for each perimeter M point"
        );
        assert_eq!(
            point.near_pentagon, expected_near_pentagon,
            "Canonical perim_map2 stores nearpent from outside unsplit U edges"
        );
    }

    for triple in perimeter.chunks_exact(3) {
        let center = triple[1];
        let edge = mesh.u_edges[center.iu];
        let suppressed_w = if center.im == edge.im[0] {
            edge.iw[1]
        } else {
            edge.iw[0]
        };
        assert!(
            selected[suppressed_w],
            "suppressed W face {suppressed_w} should be an originally selected center-segment face"
        );
        nest_wd[suppressed_w].iw[2] = -1;
    }

    for face in nest_wd.iter().skip(2).filter(|face| face.is_suppressed()) {
        assert!(
            !face.is_subdivided(),
            "Canonical suppression flag -1 must prevent full subdivision allocation"
        );
    }
}

#[test]
fn method_c_repairs_non_triplet_perimeter_by_local_growth() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let method_c_m_neighbors = mesh
        .derive_icosahedron_m_neighbors_canonical()
        .expect("Method-C M neighbors");

    let mut selected_case = None;
    'faces: for iw in 2..=mesh.nwd {
        for &adjacent in mesh.w_faces[iw].iw.iter().take(3) {
            if adjacent <= 1
                || adjacent > mesh.nwd
                || mesh.w_faces[adjacent].mrlw != mesh.w_faces[iw].mrlw
            {
                continue;
            }
            let mut selected = vec![false; mesh.nwd + 1];
            selected[iw] = true;
            selected[adjacent] = true;
            mesh.close_method_c_concavities_for_level_with_neighbors(
                &mut selected,
                &method_c_m_neighbors,
            )
            .expect("Canonical concavity closure");

            let mut nest_wd = vec![MethodCNestWd::default(); mesh.nwd + 1];
            for test_iw in 2..=mesh.nwd {
                if selected[test_iw] {
                    nest_wd[test_iw].iw[2] = 1;
                }
            }
            let Ok(perimeter) = mesh.perim_map2_method_c(&nest_wd, &method_c_m_neighbors) else {
                continue;
            };
            if !perimeter.is_empty() && perimeter.len() % 3 != 0 {
                selected_case = Some((selected, perimeter.len()));
                break 'faces;
            }
        }
    }

    let (selected, perimeter_len) =
        selected_case.expect("test requires a non-triplet Canonical perimeter case");
    let refined = mesh
            .spawn_nest_pass_with_max_mrows(
                &selected,
                2,
                MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
                true,
            )
            .unwrap_or_else(|error| {
                panic!(
                    "non-triplet perimeter length {perimeter_len} should be locally repairable when same-MRL boundary faces are available: {error}"
                )
            });
    refined
        .validate_topology()
        .expect("locally repaired Method-C mesh topology");
}

#[test]
fn method_c_concavity_closure_does_not_cross_parent_level() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(18, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let region = MethodCRefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let mesh = mesh
        .spawn_nest_with_max_mrows(
            &[region],
            1,
            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
        )
        .expect("mixed-level Method-C mesh");
    let neighbors = mesh
        .derive_icosahedron_m_neighbors_canonical()
        .expect("Method-C M neighbors");

    let (im, parent_mrlw) = (2..=mesh.nmd)
        .find_map(|im| {
            let incident = neighbors[im]
                .iw
                .iter()
                .take(neighbors[im].npoly)
                .copied()
                .collect::<Vec<_>>();
            let parent_mrlw = mesh.w_faces[*incident.first()?].mrlw;
            if !incident
                .iter()
                .all(|&iw| mesh.w_faces[iw].mrlw == parent_mrlw)
            {
                return None;
            }
            let footprint = mesh
                .method_c_rad3_faces_with_neighbors(im, &neighbors)
                .ok()?;
            footprint
                .iter()
                .any(|&iw| iw >= 2 && mesh.w_faces[iw].mrlw != parent_mrlw)
                .then_some((im, parent_mrlw))
        })
        .expect("mixed-level mesh must expose a boundary-adjacent concavity");

    let mut selected = vec![false; mesh.nwd + 1];
    for &iw in neighbors[im].iw.iter().take(neighbors[im].npoly - 1) {
        selected[iw] = true;
    }
    mesh.close_method_c_concavities_for_level_with_neighbors(&mut selected, &neighbors)
        .expect("level-preserving concavity closure");

    assert!(
        (2..=mesh.nwd).all(|iw| !selected[iw] || mesh.w_faces[iw].mrlw == parent_mrlw),
        "concavity closure must not add faces from another parent level"
    );
}

#[test]
fn method_c_concavity_fill_is_not_monotone_over_selected_face_sets() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let neighbors = mesh
        .derive_icosahedron_m_neighbors_canonical()
        .expect("Method-C M neighbors");

    let witness = (2..=mesh.nmd).find_map(|im| {
        let ring = &neighbors[im].iw[..neighbors[im].npoly];
        let mut smaller = vec![false; mesh.nwd + 1];
        for &iw in &ring[..ring.len() - 1] {
            smaller[iw] = true;
        }
        let mut larger = smaller.clone();
        larger[*ring.last()?] = true;

        mesh.close_method_c_concavities_for_level_with_neighbors(&mut smaller, &neighbors)
            .ok()?;
        mesh.close_method_c_concavities_for_level_with_neighbors(&mut larger, &neighbors)
            .ok()?;

        smaller
            .iter()
            .zip(&larger)
            .enumerate()
            .find_map(|(iw, (&in_smaller, &in_larger))| {
                (in_smaller && !in_larger).then_some((im, iw))
            })
    });

    assert!(
        witness.is_some(),
        "Canonical exactly-one-missing concavity fill is intentionally not a monotone set closure"
    );
}

#[test]
fn preserving_demand_spawn_repairs_a_vertex_only_perimeter_contact() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let neighbors = mesh
        .derive_icosahedron_m_neighbors_canonical()
        .expect("Method-C M neighbors");
    let (contact, selected) = (2..=mesh.nmd)
        .filter(|&im| neighbors[im].npoly == 6)
        .find_map(|im| {
            let mut selected = vec![false; mesh.nwd + 1];
            for slot in [0, 1, 3, 4] {
                selected[neighbors[im].iw[slot]] = true;
            }
            mesh.method_c_perimeters_from_selected_faces(&selected, &neighbors)
                .is_err_and(|error| error.to_string().contains("revisited M point"))
                .then_some((im, selected))
        })
        .expect("degree-four perimeter contact fixture");
    let anchors = neighbors[contact]
        .iw
        .iter()
        .copied()
        .filter(|&iw| selected[iw])
        .map(|iw| (contact, vec![iw]))
        .collect();
    let coverage = crate::method_c_spawn_hfield::MethodCHfieldDemandCoverage::from_anchors(anchors);

    let refined = mesh
        .spawn_nest_pass_method_c_preserving_demands(
            &selected,
            2,
            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
            true,
            &coverage,
        )
        .expect("growth-only repair must make the preserving-demand perimeter walkable");

    refined
        .validate_topology()
        .expect("repaired preserving-demand topology");
}

#[test]
fn method_c_perimeter_repair_fills_multiple_vertex_only_contacts() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let neighbors = mesh
        .derive_icosahedron_m_neighbors_canonical()
        .expect("Method-C M neighbors");
    let contacts = (2..=mesh.nmd)
        .filter(|&im| neighbors[im].npoly == 6)
        .collect::<Vec<_>>();
    let (contacts, selected) = contacts
        .iter()
        .enumerate()
        .find_map(|(left_index, &left)| {
            let left_footprint = mesh
                .method_c_rad3_faces_with_neighbors(left, &neighbors)
                .ok()?
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>();
            contacts.iter().skip(left_index + 1).find_map(|&right| {
                let right_footprint = mesh
                    .method_c_rad3_faces_with_neighbors(right, &neighbors)
                    .ok()?
                    .into_iter()
                    .collect::<std::collections::BTreeSet<_>>();
                if !left_footprint.is_disjoint(&right_footprint) {
                    return None;
                }
                let mut selected = vec![false; mesh.nwd + 1];
                for im in [left, right] {
                    for slot in [0, 1, 3, 4] {
                        selected[neighbors[im].iw[slot]] = true;
                    }
                }
                mesh.method_c_perimeters_from_selected_faces(&selected, &neighbors)
                    .is_err_and(|error| error.to_string().contains("revisited M point"))
                    .then_some(([left, right], selected))
            })
        })
        .expect("two independent degree-four perimeter contacts");
    let demanded_faces = contacts
        .into_iter()
        .flat_map(|im| neighbors[im].iw.iter().copied())
        .filter(|&iw| selected[iw])
        .collect::<Vec<_>>();
    assert!(
        mesh.method_c_vertex_only_perimeter_contacts(&selected, &neighbors)
            .expect("classified perimeter contacts")
            .len()
            >= 2
    );
    let mut repaired = selected;
    let perimeter = mesh
        .repair_method_c_non_triplet_perimeter(&mut repaired, &neighbors, 2)
        .expect("batch repair must make all perimeter contacts walkable");

    assert_eq!(perimeter.len() % 3, 0);
    assert!(
        demanded_faces.into_iter().all(|iw| repaired[iw]),
        "growth-only batch repair must preserve every demanded face"
    );
}

#[test]
fn method_c_boundary_repairs_are_deterministic_across_thread_counts() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let neighbors = mesh
        .derive_icosahedron_m_neighbors_canonical()
        .expect("Method-C M neighbors");
    let region = MethodCRefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let selected = mesh
        .selected_region_faces(&region, 1, false)
        .expect("selected Method-C faces");
    let perimeter = mesh
        .method_c_perimeters_from_selected_faces(&selected, &neighbors)
        .expect("Method-C perimeter")
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let run = |threads| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("local Rayon pool")
            .install(|| {
                Ok::<_, std::io::Error>((
                    mesh.try_fill_method_c_perimeter_boundary(
                        &selected,
                        &neighbors,
                        2,
                        Some(&perimeter),
                        None,
                        MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
                        true,
                    )?,
                    mesh.try_shrink_method_c_perimeter_once(
                        &selected,
                        &neighbors,
                        2,
                        Some(&perimeter),
                        None,
                    )?,
                ))
            })
            .expect("boundary repairs")
    };

    assert_eq!(run(1), run(4));
}

#[test]
fn method_c_repair_witness_dependency_faces_cover_parent_u_stencil() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let neighbors = mesh
        .derive_icosahedron_m_neighbors_canonical()
        .expect("Method-C M neighbors");
    let parent_u = 2;
    let error = crate::method_c_table_helpers::method_c_repairable_error_with_parent_origin(
        method_c_repairable_error(MethodCRepairableKind::Valence, None, "test witness"),
        None,
        Some(parent_u),
    );
    let faces = mesh.method_c_repair_witness_dependency_faces(&error, &neighbors);

    assert!(!faces.is_empty());
    assert!(mesh.u_edges[parent_u]
        .iw
        .into_iter()
        .filter(|&iw| iw > 1)
        .all(|iw| faces.binary_search(&iw).is_ok()));
}

#[test]
fn method_c_shrink_uses_the_best_coverage_preserving_candidate() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let neighbors = mesh
        .derive_icosahedron_m_neighbors_canonical()
        .expect("Method-C M neighbors");
    let region = MethodCRefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let base_selected = mesh
        .selected_region_faces(&region, 1, false)
        .expect("selected Method-C faces");
    let mut selected = base_selected;
    selected[120] = true;
    selected[121] = true;
    mesh.close_method_c_concavities_with_neighbors(&mut selected, &neighbors)
        .expect("closed two-protrusion fixture");
    let (unconstrained, _) = mesh
        .try_shrink_method_c_perimeter_once(&selected, &neighbors, 2, None, None)
        .expect("unconstrained shrink")
        .expect("best shrink candidate");
    assert!(
        !unconstrained[121],
        "the unconstrained best candidate removes face 121"
    );
    let coverage = crate::method_c_spawn_hfield::MethodCHfieldDemandCoverage::from_anchors(vec![(
        2,
        vec![121],
    )]);
    let (preserving, _) = mesh
        .try_shrink_method_c_perimeter_once(&selected, &neighbors, 2, None, Some(&coverage))
        .expect("coverage-aware shrink")
        .expect("a lower-ranked coverage-preserving shrink candidate");

    assert!(preserving[121]);
    assert_ne!(preserving, unconstrained);
}

#[test]
fn preserving_demand_spawn_rejects_a_shrink_that_uncovers_an_anchor() {
    let coverage = crate::method_c_spawn_hfield::MethodCHfieldDemandCoverage::from_anchors(vec![(
        7,
        vec![2, 3],
    )]);
    let mut candidate = vec![false; 4];
    assert!(
        !MethodCDelaunayMesh::method_c_repair_candidate_preserves_coverage(
            Some(&coverage),
            &candidate,
        )
    );
    candidate[3] = true;
    assert!(
        MethodCDelaunayMesh::method_c_repair_candidate_preserves_coverage(
            Some(&coverage),
            &candidate,
        )
    );
    assert!(MethodCDelaunayMesh::method_c_repair_candidate_preserves_coverage(None, &candidate,));
}

#[test]
fn method_c_perim_ngr_matches_perimeter_next_point() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(66, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let method_c_m_neighbors = mesh
        .derive_icosahedron_m_neighbors_canonical()
        .expect("Method-C M neighbors");

    let mut selected_case = None;
    for im in 2..=mesh.nmd {
        let point = mesh.m_points[im];
        let region = MethodCRefinementRegion::Circle {
            center: xyz_to_lonlat_degrees(point),
            radius_meters: 2_000_000.0,
            level: 1,
        };
        let selected = mesh
            .selected_region_faces(&region, 1, false)
            .expect("selected Method-C faces");
        let mut nest_wd = vec![MethodCNestWd::default(); mesh.nwd + 1];
        for iw in 2..=mesh.nwd {
            if selected[iw] {
                nest_wd[iw].iw[2] = 1;
            }
        }
        let perimeter = match mesh.perim_map2_method_c(&nest_wd, &method_c_m_neighbors) {
            Ok(perimeter) => perimeter,
            Err(_) => continue,
        };
        if perimeter.is_empty() {
            continue;
        }
        selected_case = Some((perimeter, nest_wd));
        break;
    }
    let (perimeter, nest_wd) = selected_case.expect("perimeter case in selected Method-C region");

    for point in perimeter {
        let edge = mesh.u_edges[point.iu];
        let next_expected = if point.im == edge.im[0] {
            edge.im[1]
        } else {
            edge.im[0]
        };
        let (next, next_edge) = mesh
            .perim_ngr_method_c(point.im, &nest_wd, &method_c_m_neighbors)
            .expect("canonical perim_ngr");
        assert_eq!(
            next_edge, point.iu,
            "perim_map2 and perim_ngr must agree on boundary edge"
        );
        assert_eq!(
            next, next_expected,
            "perim_ngr should return the immediate perimeter neighbor without prognostic folding"
        );
    }
}

#[test]
fn method_c_full_subdivision_uses_grid_number_for_w_face_ngr() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(66, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let child_grid_number = 4;
    let mut refined_case = None;

    for radius_meters in [
        2_500_000.0,
        3_000_000.0,
        3_500_000.0,
        4_000_000.0,
        4_500_000.0,
        5_000_000.0,
        5_500_000.0,
        6_000_000.0,
    ] {
        let region = MethodCRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters,
            level: 1,
        };
        let selected = mesh
            .selected_region_faces(&region, 1, false)
            .expect("selected Method-C faces");
        let Ok(refined) =
            mesh.spawn_nest_pass_with_max_mrows(&selected, child_grid_number, 7, true)
        else {
            continue;
        };
        if refined
            .w_faces
            .iter()
            .skip(2)
            .any(|face| face.mrlw == 2 && face.mrow == 0)
        {
            refined_case = Some(refined);
            break;
        }
    }

    let refined = refined_case.expect("test case with an interior full-subdivision W face");
    let mismatched = refined
        .w_faces
        .iter()
        .enumerate()
        .skip(2)
        .filter_map(|(iw, face)| {
            if face.mrlw == 2 && face.ngr != child_grid_number {
                Some((iw, face.ngr))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    assert!(
            mismatched.is_empty(),
            "Canonical assigns full-subdivision W-face ngr from the current grid number, not mrlo + 1; mismatches: {mismatched:?}"
        );
}

#[test]
fn method_c_pass_uses_canonical_table_numbering_counts() {
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

    let selected_w_count = (2..=mesh.nwd)
        .filter(|&iw| nest_wd[iw].is_subdivided())
        .count();
    let split_u_count = (2..=mesh.nud)
        .filter(|&iu| {
            let edge = mesh.u_edges[iu];
            let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
            (nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided())
                && !nest_wd[iw1].is_suppressed()
                && !nest_wd[iw2].is_suppressed()
        })
        .count();

    let refined = mesh
        .spawn_nest_pass_with_max_mrows(&selected, 2, 7, true)
        .expect("Method-C pass");

    assert_eq!(
        refined.nmd,
        mesh.nmd + split_u_count,
        "Canonical Method-C allocates one midpoint M only for non-suppressed split U edges"
    );
    assert_eq!(
        refined.nud,
        mesh.nud + split_u_count + 3 * selected_w_count,
        "Canonical Method-C allocates one split U plus three child-W internal U edges"
    );
    assert_eq!(
            refined.nwd,
            mesh.nwd + 3 * selected_w_count,
            "Canonical Method-C keeps the remapped parent W and adds three child W faces per subdivided W"
        );
}
