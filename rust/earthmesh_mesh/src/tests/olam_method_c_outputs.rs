use super::*;

#[test]
fn olam_method_c_emits_closed_topology_without_placeholder_neighbor_ids() {
    let mesh = OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let region = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let selected = mesh
        .selected_region_faces(&region, 1, false)
        .expect("selected Method-C faces");
    let refined = mesh
        .spawn_nest_pass_with_max_mrows(&selected, 2, 7, true)
        .expect("Method-C pass");

    refined
        .validate_topology()
        .expect("Method-C output topology should be closed");
    for iu in 2..=refined.nud {
        assert!(
            refined.u_edges[iu].im.iter().all(|&im| im > 1),
            "U edge {iu} should not contain placeholder M endpoint"
        );
        assert!(
            refined.u_edges[iu].iw.iter().take(2).all(|&iw| iw > 1),
            "U edge {iu} should not contain placeholder adjacent W face"
        );
    }
    for iw in 2..=refined.nwd {
        assert!(
            refined.w_faces[iw].im.iter().all(|&im| im > 1),
            "W face {iw} should not contain placeholder M vertex"
        );
        assert!(
            refined.w_faces[iw].iu.iter().all(|&iu| iu > 1),
            "W face {iw} should not contain placeholder U edge"
        );
    }
    for im in 2..=refined.nmd {
        let neighbors = refined.m_neighbors[im];
        for &iu in neighbors.iu.iter().take(neighbors.npoly) {
            assert!(
                iu > 1,
                "M point {im} should not contain placeholder U neighbor"
            );
        }
        for &iw in neighbors.iw.iter().take(neighbors.npoly) {
            assert!(
                iw > 1,
                "M point {im} should not contain placeholder W neighbor"
            );
        }
    }
}
#[test]
fn olam_method_c_multiple_regions_emit_projected_closed_outputs() {
    let mesh = OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let radius = active_mesh_radius(&mesh).expect("active mesh radius");
    let method_c_m_neighbors = mesh
        .derive_icosahedron_m_neighbors_fortran()
        .expect("Method-C M neighbors");
    let cases = [
        OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        },
        OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 3_500_000.0,
            level: 1,
        },
        OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(15.0, 45.0),
            radius_meters: 2_500_000.0,
            level: 1,
        },
        OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(-75.0, 10.0),
            radius_meters: 2_500_000.0,
            level: 1,
        },
        OlamRefinementRegion::Bbox {
            west_degrees: 110.0,
            east_degrees: 120.0,
            south_degrees: 20.0,
            north_degrees: 30.0,
            level: 1,
        },
        OlamRefinementRegion::Corridor {
            points: vec![
                LonLatDegrees::new(110.0, 24.0),
                LonLatDegrees::new(120.0, 26.0),
            ],
            radius_meters: vec![1_500_000.0, 1_500_000.0],
            level: 1,
        },
        OlamRefinementRegion::Polygon {
            points: vec![
                LonLatDegrees::new(110.0, 20.0),
                LonLatDegrees::new(120.0, 20.0),
                LonLatDegrees::new(120.0, 30.0),
                LonLatDegrees::new(110.0, 30.0),
            ],
            level: 1,
        },
    ];

    for region in cases {
        let selected = mesh
            .selected_region_faces(&region, 1, false)
            .expect("selected Method-C faces");
        let selected_parent_mrl = (2..=mesh.nwd)
            .find(|&iw| selected.get(iw).copied().unwrap_or(false))
            .map(|iw| mesh.w_faces[iw].mrlw);
        let mut expected_selected = selected.clone();
        mesh.close_olam_method_c_concavities_for_level_with_neighbors(
            &mut expected_selected,
            &method_c_m_neighbors,
        )
        .expect("Method-C closure");
        if let Some(parent_mrl) = selected_parent_mrl {
            for iw in 2..=mesh.nwd {
                if mesh.w_faces[iw].mrlw != parent_mrl {
                    expected_selected[iw] = false;
                }
            }
        }
        let mut nest_wd = vec![OlamMethodCNestWd::default(); mesh.nwd + 1];
        for iw in 2..=mesh.nwd {
            if expected_selected[iw] {
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
        refined
            .validate_topology()
            .expect("Method-C output topology should be closed");
        for im in 2..=refined.nmd {
            let delta = (magnitude(refined.m_points[im]) - radius).abs();
            assert!(
                    delta < 1.0e-6,
                    "Fortran spawn_nest final projection should place M point {im} on the active radius; delta={delta}"
                );
            let neighbors = refined.m_neighbors[im];
            for &iu in neighbors.iu.iter().take(neighbors.npoly) {
                assert!(
                    iu > 1,
                    "M point {im} should not contain placeholder U neighbor"
                );
            }
            for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                assert!(
                    iw > 1,
                    "M point {im} should not contain placeholder W neighbor"
                );
            }
        }
        for iu in 2..=refined.nud {
            assert!(
                refined.u_edges[iu].im.iter().all(|&im| im > 1),
                "U edge {iu} should not contain placeholder M endpoint"
            );
            assert!(
                refined.u_edges[iu].iw.iter().take(2).all(|&iw| iw > 1),
                "U edge {iu} should not contain placeholder adjacent W face"
            );
        }
        for iw in 2..=refined.nwd {
            assert!(
                refined.w_faces[iw].im.iter().all(|&im| im > 1),
                "W face {iw} should not contain placeholder M vertex"
            );
            assert!(
                refined.w_faces[iw].iu.iter().all(|&iu| iu > 1),
                "W face {iw} should not contain placeholder U edge"
            );
        }
    }
}
