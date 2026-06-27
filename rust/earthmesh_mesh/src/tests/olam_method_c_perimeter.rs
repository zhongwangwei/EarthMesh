use super::super::*;

#[test]
fn olam_method_c_suppresses_center_perimeter_segment_faces_like_fortran() {
    let mesh = OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let region = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let selected = mesh
        .selected_region_faces(&region, 1, false)
        .expect("selected Method-C faces");
    let method_c_m_neighbors = mesh
        .derive_icosahedron_m_neighbors_fortran()
        .expect("Method-C M neighbors");
    let mut nest_wd = vec![OlamMethodCNestWd::default(); mesh.nwd + 1];
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
        "Fortran Method-C suppression consumes perimeter points in triples"
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
        .expect("Fortran perim_map2 start point");
    assert_eq!(
        perimeter[0].im, expected_start,
        "Fortran perim_map2 starts from the first original M point with nwdiv == 2"
    );
    for index in 0..perimeter.len() {
        let point = perimeter[index];
        let next = perimeter[(index + 1) % perimeter.len()].im;
        let edge = mesh.u_edges[point.iu];
        let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
        if edge.im[0] == point.im {
            assert!(
                    nest_wd[iw1].flag() == 0 && nest_wd[iw2].is_subdivided(),
                    "Fortran perim_ngr advances from im(1) only when iw(1) is outside and iw(2) is inside"
                );
            assert_eq!(
                next, edge.im[1],
                "Fortran perim_ngr next M point from im(1) is im(2)"
            );
        } else {
            assert_eq!(
                edge.im[1], point.im,
                "Fortran perim_ngr perimeter U edge must contain the current M point"
            );
            assert!(
                    nest_wd[iw2].flag() == 0 && nest_wd[iw1].is_subdivided(),
                    "Fortran perim_ngr advances from im(2) only when iw(2) is outside and iw(1) is inside"
                );
            assert_eq!(
                next, edge.im[0],
                "Fortran perim_ngr next M point from im(2) is im(1)"
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
            "Fortran perim_map2 stores npolyper for each perimeter M point"
        );
        assert_eq!(
            point.nwdiv, expected_nwdiv,
            "Fortran perim_map2 stores nwdivper for each perimeter M point"
        );
        assert_eq!(
            point.near_pentagon, expected_near_pentagon,
            "Fortran perim_map2 stores nearpent from outside unsplit U edges"
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
            "Fortran suppression flag -1 must prevent full subdivision allocation"
        );
    }
}

#[test]
fn olam_method_c_repairs_non_triplet_perimeter_by_local_growth() {
    let mesh = OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let method_c_m_neighbors = mesh
        .derive_icosahedron_m_neighbors_fortran()
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
            mesh.close_olam_method_c_concavities_for_level_with_neighbors(
                &mut selected,
                &method_c_m_neighbors,
            )
            .expect("Fortran concavity closure");

            let mut nest_wd = vec![OlamMethodCNestWd::default(); mesh.nwd + 1];
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
        selected_case.expect("test requires a non-triplet Fortran perimeter case");
    let refined = mesh
            .spawn_nest_pass_with_max_mrows(
                &selected,
                2,
                OlamDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
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
fn olam_method_c_perim_ngr_matches_perimeter_next_point() {
    let mesh = OlamDelaunayMesh::from_icosahedron(66, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let method_c_m_neighbors = mesh
        .derive_icosahedron_m_neighbors_fortran()
        .expect("Method-C M neighbors");

    let mut selected_case = None;
    for im in 2..=mesh.nmd {
        let point = mesh.m_points[im];
        let region = OlamRefinementRegion::Circle {
            center: xyz_to_lonlat_degrees(point),
            radius_meters: 2_000_000.0,
            level: 1,
        };
        let selected = mesh
            .selected_region_faces(&region, 1, false)
            .expect("selected Method-C faces");
        let mut nest_wd = vec![OlamMethodCNestWd::default(); mesh.nwd + 1];
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
            .expect("fortran perim_ngr");
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
fn olam_method_c_full_subdivision_uses_grid_number_for_w_face_ngr() {
    let mesh = OlamDelaunayMesh::from_icosahedron(66, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
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
        let region = OlamRefinementRegion::Circle {
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
            "Fortran assigns full-subdivision W-face ngr from the current grid number, not mrlo + 1; mismatches: {mismatched:?}"
        );
}

#[test]
fn olam_method_c_pass_uses_fortran_table_numbering_counts() {
    let mesh = OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let region = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let mut selected = mesh
        .selected_region_faces(&region, 1, false)
        .expect("selected Method-C faces");
    let method_c_m_neighbors = mesh
        .derive_icosahedron_m_neighbors_fortran()
        .expect("Method-C M neighbors");
    mesh.close_olam_method_c_concavities_for_level_with_neighbors(
        &mut selected,
        &method_c_m_neighbors,
    )
    .expect("Method-C closure");

    let mut nest_wd = vec![OlamMethodCNestWd::default(); mesh.nwd + 1];
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
        "Fortran Method-C allocates one midpoint M only for non-suppressed split U edges"
    );
    assert_eq!(
        refined.nud,
        mesh.nud + split_u_count + 3 * selected_w_count,
        "Fortran Method-C allocates one split U plus three child-W internal U edges"
    );
    assert_eq!(
            refined.nwd,
            mesh.nwd + 3 * selected_w_count,
            "Fortran Method-C keeps the remapped parent W and adds three child W faces per subdivided W"
        );
}
