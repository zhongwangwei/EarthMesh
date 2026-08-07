use super::*;

#[test]
fn method_c_split_u_midpoint_coordinates_match_canonical_edge_average_projection() {
    let mesh = MethodCMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let radius = active_mesh_radius(&mesh).expect("active mesh radius");
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
        if ![iw1, iw2].into_iter().all(|iw| {
            mesh.w_faces[iw]
                .iw
                .iter()
                .take(3)
                .all(|&neighbor| neighbor > 1 && nest_wd[neighbor].is_subdivided())
        }) {
            continue;
        }
        let linear_midpoint =
            weighted_point(mesh.m_points[old.im[0]], 1.0, mesh.m_points[old.im[1]], 1.0)
                .expect("Canonical midpoint average");
        let expected = normalize_cartesian_to_radius(linear_midpoint, radius)
            .expect("Canonical final radius projection");
        let actual = refined.m_points[midpoint];
        let delta = magnitude(CartesianPoint::new(
            actual.x - expected.x,
            actual.y - expected.y,
            actual.z - expected.z,
        ));
        assert!(
                delta < 1.0e-6,
                "Canonical Method-C split-U {iu} midpoint M {midpoint} should be edge-average projected to radius; delta={delta}"
            );
        checked += 1;
    }

    assert!(
        checked > 0,
        "test should exercise interior non-suppressed split-U midpoint coordinates"
    );
}

#[test]
fn method_c_cartesian_split_u_midpoint_coordinates_match_native_edge_average() {
    let mesh = MethodCMesh::from_cart_hex(18, 1_000_000.0).expect("cart_hex Method-C mesh");
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(10_200_000.0, -310_000.0),
        radius_meters: 500_000.0,
        level: 1,
    };
    let mut selected = mesh
        .selected_region_faces(&region, 1, true)
        .expect("selected Cartesian Method-C faces");
    let method_c_m_neighbors = mesh.method_c_m_neighbors().expect("Method-C M neighbors");
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
        .spawn_nest_pass_with_max_mrows(&selected, 2, MethodCMesh::MAX_MROWS_ATMOS, false)
        .expect("Cartesian Method-C pass");
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
        if ![iw1, iw2].into_iter().all(|iw| {
            mesh.w_faces[iw]
                .iw
                .iter()
                .take(3)
                .all(|&neighbor| neighbor > 1 && nest_wd[neighbor].is_subdivided())
        }) {
            continue;
        }
        let expected = weighted_point(mesh.m_points[old.im[0]], 1.0, mesh.m_points[old.im[1]], 1.0)
            .expect("Canonical Cartesian midpoint average");
        let actual = refined.m_points[midpoint];
        let delta = magnitude(CartesianPoint::new(
            actual.x - expected.x,
            actual.y - expected.y,
            actual.z - expected.z,
        ));
        assert!(
                delta < 1.0e-9,
                "Canonical Cartesian Method-C split-U {iu} midpoint M {midpoint} should be native edge-average without radius projection; delta={delta}"
            );
        checked += 1;
    }

    assert!(
        checked > 0,
        "test should exercise full-interior Cartesian split-U midpoint coordinates"
    );
}
