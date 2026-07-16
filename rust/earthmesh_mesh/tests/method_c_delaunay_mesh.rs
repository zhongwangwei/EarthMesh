use earthmesh_mesh::METHOD_C_CANONICAL_EARTH_RADIUS_METERS;
use earthmesh_mesh::{
    method_c_gridinit_factorization_canonical, voronoi_grid_from_method_c_delaunay_mesh,
    CartesianPoint, MethodCDelaunayMesh,
};

fn magnitude(point: CartesianPoint) -> f64 {
    (point.x * point.x + point.y * point.y + point.z * point.z).sqrt()
}

fn distance(a: CartesianPoint, b: CartesianPoint) -> f64 {
    magnitude(CartesianPoint::new(a.x - b.x, a.y - b.y, a.z - b.z))
}

#[test]
fn method_c_delaunay_mesh_from_icosahedron_has_closed_muw_topology() {
    let mesh = MethodCDelaunayMesh::from_icosahedron(1, 0, 1.0, 0.25, 100)
        .expect("valid Method-C icosahedron mesh");

    assert_eq!(mesh.nmd, 13);
    assert_eq!(mesh.nud, 31);
    assert_eq!(mesh.nwd, 21);

    let report = mesh.validate_topology().expect("closed topology");
    assert_eq!(report.checked_m_points, mesh.nmd - 1);
    assert_eq!(report.checked_u_edges, mesh.nud - 1);
    assert_eq!(report.checked_w_faces, mesh.nwd - 1);

    for point_id in 2..=mesh.nmd {
        let radius = magnitude(mesh.m_points[point_id]);
        assert!(
            (radius - METHOD_C_CANONICAL_EARTH_RADIUS_METERS).abs() <= 1.0,
            "point {point_id} radius {radius}"
        );
    }
}

#[test]
fn method_c_topology_rejects_duplicate_or_non_pentagonal_protected_points() {
    let mesh = MethodCDelaunayMesh::from_icosahedron(2, 0, 1.0, 0.25, 100)
        .expect("valid Method-C icosahedron mesh");

    let mut duplicate = mesh.clone();
    duplicate.impent[1] = duplicate.impent[0];
    let error = duplicate.validate_topology().unwrap_err();
    assert!(
        error.to_string().contains("duplicate protected point"),
        "{error}"
    );

    let mut wrong_degree = mesh;
    let hex_point = (2..=wrong_degree.nmd)
        .find(|point| {
            !wrong_degree.impent.contains(point) && wrong_degree.m_neighbors[*point].npoly == 6
        })
        .expect("uniform icosahedron contains a six-sided point");
    wrong_degree.impent[0] = hex_point;
    let error = wrong_degree.validate_topology().unwrap_err();
    assert!(error.to_string().contains("expected 5"), "{error}");
}

#[test]
fn method_c_cart_hex_mdomain_five_uses_canonical_planar_counts_and_coordinates() {
    let mesh = MethodCDelaunayMesh::from_cart_hex(2, 1000.0).expect("valid Method-C cart_hex mesh");

    assert_eq!(mesh.nmd, 28);
    assert_eq!(mesh.nud, 64);
    assert_eq!(mesh.nwd, 53);
    for iu in 2..=mesh.nud {
        assert_eq!(mesh.u_edges[iu].mrlu, 1);
    }
    for iw in 2..=mesh.nwd {
        assert_eq!(mesh.w_faces[iw].mrlw, 1);
        assert_eq!(mesh.w_faces[iw].mrlw_orig, 1);
        assert_eq!(mesh.w_faces[iw].ngr, 1);
    }

    let unit_dist = (4.0_f64 / 3.0).sqrt().sqrt() * 1000.0;
    let xstart = -1.5 * unit_dist;
    let ystart = -(2.0 + 1.0 / 3.0) * 0.5 * 3.0_f64.sqrt() * unit_dist;

    assert_eq!(mesh.m_points[2].z, 0.0);
    assert!((mesh.m_points[2].x - xstart).abs() <= 1.0e-9);
    assert!((mesh.m_points[2].y - ystart).abs() <= 1.0e-9);

    for point_id in 2..=mesh.nmd {
        assert_eq!(mesh.m_points[point_id].z, 0.0, "point {point_id}");
        assert!(
            magnitude(mesh.m_points[point_id]) < METHOD_C_CANONICAL_EARTH_RADIUS_METERS / 100.0,
            "cart_hex point {point_id} should remain in the local Cartesian plane"
        );
    }
}

#[test]
fn method_c_cart_hex_rejects_deltax_below_canonical_lower_bound() {
    let err = MethodCDelaunayMesh::from_cart_hex(2, 0.0009)
        .expect_err("Canonical cart_hex rejects DELTAX below 0.001");

    assert!(
        err.to_string().contains("DELTAX"),
        "unexpected error: {err}"
    );
}

#[test]
fn method_c_cart_hex_fills_first_canonical_u_and_w_neighbors() {
    let mesh = MethodCDelaunayMesh::from_cart_hex(2, 1000.0).expect("valid Method-C cart_hex mesh");

    assert_eq!(mesh.u_edges[2].im, [2, 3]);
    assert_eq!(&mesh.u_edges[2].iw[..2], &[16, 2]);
    assert_eq!(mesh.u_edges[3].im, [2, 6]);
    assert_eq!(&mesh.u_edges[3].iw[..2], &[2, 3]);
    assert_eq!(mesh.u_edges[4].im, [5, 2]);
    assert_eq!(&mesh.u_edges[4].iw[..2], &[8, 3]);

    assert_eq!(mesh.w_faces[2].npoly, 3);
    assert_eq!(mesh.w_faces[2].iu, [2, 7, 3]);
    assert_eq!(mesh.w_faces[2].im, [2, 3, 6]);
    assert_eq!(mesh.w_faces[3].npoly, 3);
    assert_eq!(mesh.w_faces[3].iu, [3, 11, 4]);
    assert_eq!(mesh.w_faces[3].im, [2, 6, 5]);
}

#[test]
fn method_c_cart_hex_derives_interior_m_neighbors_from_canonical_incidence() {
    let mesh = MethodCDelaunayMesh::from_cart_hex(2, 1000.0).expect("valid Method-C cart_hex mesh");

    let neighbors = mesh.m_neighbors[6];
    assert_eq!(neighbors.npoly, 6);

    let mut u_edges = neighbors.iu[..neighbors.npoly].to_vec();
    u_edges.sort_unstable();
    assert_eq!(u_edges, vec![3, 7, 11, 14, 15, 16]);

    let mut w_faces = neighbors.iw[..neighbors.npoly].to_vec();
    w_faces.sort_unstable();
    assert_eq!(w_faces, vec![2, 3, 5, 9, 11, 12]);
}

#[test]
fn method_c_cart_hex_preserves_canonical_boundary_periodic_maps() {
    let mesh = MethodCDelaunayMesh::from_cart_hex(2, 1000.0).expect("valid Method-C cart_hex mesh");

    assert_eq!(mesh.m_prognostic[2], 15);
    assert_eq!(mesh.u_prognostic[2], 24);
    assert_eq!(mesh.u_prognostic[4], 35);
    assert_eq!(mesh.w_prognostic[16], 39);
    assert_eq!(mesh.w_prognostic[8], 28);
}

#[test]
fn method_c_cart_hex_ghost_w_faces_copy_canonical_periodic_partner_topology() {
    let mesh = MethodCDelaunayMesh::from_cart_hex(2, 1000.0).expect("valid Method-C cart_hex mesh");

    let ghost = 16;
    let partner = mesh.w_prognostic[ghost];
    assert_eq!(partner, 39);
    assert_eq!(mesh.w_faces[ghost].npoly, 3);
    assert_eq!(mesh.w_faces[ghost].im, mesh.w_faces[partner].im);
    assert_eq!(mesh.w_faces[ghost].iu[0], 2);
}

#[test]
fn method_c_cart_hex_topology_validation_allows_canonical_periodic_ghost_faces() {
    let mesh = MethodCDelaunayMesh::from_cart_hex(5, 1000.0).expect("valid Method-C cart_hex mesh");

    mesh.validate_topology()
        .expect("Canonical cart_hex ghost W faces are validated through w_prognostic");
}

#[test]
fn method_c_cart_hex_orders_outer_w_faces_for_fill_rad3_sectors() {
    let mesh = MethodCDelaunayMesh::from_cart_hex(5, 1000.0).expect("valid Method-C cart_hex mesh");

    let face = mesh.w_faces[135];
    assert_eq!(face.im, [67, 108, 109]);

    let imx = face.im[0];
    let iwx = face.iw[7];
    let iwy = face.iw[8];
    assert!(
        mesh.w_faces[iwx].im.contains(&imx),
        "fill_rad3 iwx face {iwx} should contain imx {imx}"
    );

    let im2 = if mesh.w_faces[iwx].im[0] == imx {
        mesh.w_faces[iwx].im[2]
    } else if mesh.w_faces[iwx].im[1] == imx {
        mesh.w_faces[iwx].im[0]
    } else {
        mesh.w_faces[iwx].im[1]
    };
    assert!(
        mesh.w_faces[iwy].im.contains(&im2),
        "fill_rad3 iwy face {iwy} should contain im2 {im2}"
    );
}

#[test]
fn method_c_global_spring_preserves_nxp2_equilibrium_within_canonical_storage_precision() {
    let unsprung = MethodCDelaunayMesh::from_icosahedron(2, 0, 1.0, 0.25, 100)
        .expect("unsprung Method-C icosahedron mesh");
    let sprung = MethodCDelaunayMesh::from_icosahedron(2, 2, 1.0, 0.25, 100)
        .expect("sprung Method-C icosahedron mesh");

    let max_displacement = (2..=unsprung.nmd)
        .map(|point_id| distance(sprung.m_points[point_id], unsprung.m_points[point_id]))
        .fold(0.0_f64, f64::max);
    assert!(
        max_displacement <= 2.0,
        "NXP2 equilibrium moved by {max_displacement} meters"
    );
}

#[test]
fn method_c_delaunay_mesh_can_drive_voronoi_grid_generation() {
    let mesh = MethodCDelaunayMesh::from_icosahedron(1, 0, 1.0, 0.25, 100)
        .expect("valid Method-C icosahedron mesh");

    let state =
        voronoi_grid_from_method_c_delaunay_mesh(&mesh, METHOD_C_CANONICAL_EARTH_RADIUS_METERS)
            .expect("Method-C Voronoi state");

    assert_eq!(state.grid.nma, mesh.nwd);
    assert_eq!(state.grid.nua, mesh.nud);
    assert_eq!(state.grid.nva, mesh.nud);
    assert_eq!(state.grid.nwa, mesh.nmd);
    assert_eq!(
        state.tabs.m[2].iw,
        mesh.w_faces[2].im.map(|value| value as i32)
    );
    assert_eq!(state.tabs.w[2].npoly, mesh.m_neighbors[2].npoly as i32);
    assert_eq!(state.grid.xew[2], mesh.m_points[2].x);
    assert_eq!(state.grid.yew[2], mesh.m_points[2].y);
    assert_eq!(state.grid.zew[2], mesh.m_points[2].z);
}

#[test]
fn method_c_gridinit_factorization_matches_get_factors_selection_rules() {
    let nxp16 = method_c_gridinit_factorization_canonical(16).expect("NXP 16 is valid");
    assert_eq!(nxp16.base_nxp, 16);
    assert_eq!(nxp16.expansion_factor, 1);

    let nxp72 = method_c_gridinit_factorization_canonical(72).expect("NXP 72 is valid");
    assert_eq!(nxp72.base_nxp, 36);
    assert_eq!(nxp72.expansion_factor, 2);

    let nxp96 = method_c_gridinit_factorization_canonical(96).expect("NXP 96 is valid");
    assert_eq!(nxp96.base_nxp, 32);
    assert_eq!(nxp96.expansion_factor, 3);
}

#[test]
fn method_c_expand_global2_subdivides_each_triangle_and_rebuilds_topology() {
    let mesh = MethodCDelaunayMesh::from_icosahedron(1, 0, 1.0, 0.25, 100)
        .expect("valid base Method-C mesh");

    let expanded = mesh.expand_global2().expect("factor-2 Method-C expansion");

    assert_eq!(expanded.nmd, 43);
    assert_eq!(expanded.nud, 121);
    assert_eq!(expanded.nwd, 81);

    let report = expanded.validate_topology().expect("expanded topology");
    assert_eq!(report.checked_m_points, expanded.nmd - 1);
    assert_eq!(report.checked_u_edges, expanded.nud - 1);
    assert_eq!(report.checked_w_faces, expanded.nwd - 1);

    for point_id in 2..=expanded.nmd {
        let radius = magnitude(expanded.m_points[point_id]);
        assert!(
            (radius - METHOD_C_CANONICAL_EARTH_RADIUS_METERS).abs() <= 1.0,
            "point {point_id} radius {radius}"
        );
    }
}

#[test]
fn method_c_expand_global3_trisects_each_triangle_and_rebuilds_topology() {
    let mesh = MethodCDelaunayMesh::from_icosahedron(1, 0, 1.0, 0.25, 100)
        .expect("valid base Method-C mesh");

    let expanded = mesh.expand_global3().expect("factor-3 Method-C expansion");

    assert_eq!(expanded.nmd, 93);
    assert_eq!(expanded.nud, 271);
    assert_eq!(expanded.nwd, 181);

    let report = expanded.validate_topology().expect("expanded topology");
    assert_eq!(report.checked_m_points, expanded.nmd - 1);
    assert_eq!(report.checked_u_edges, expanded.nud - 1);
    assert_eq!(report.checked_w_faces, expanded.nwd - 1);

    for point_id in 2..=expanded.nmd {
        let radius = magnitude(expanded.m_points[point_id]);
        assert!(
            (radius - METHOD_C_CANONICAL_EARTH_RADIUS_METERS).abs() <= 1.0,
            "point {point_id} radius {radius}"
        );
    }
}

#[test]
fn method_c_expand_by_factor_applies_factor2_and_rejects_unsupported_products() {
    let mesh = MethodCDelaunayMesh::from_icosahedron(1, 0, 1.0, 0.25, 100)
        .expect("valid base Method-C mesh");

    let doubled = mesh.expand_by_factor(2).expect("factor 2 expansion");
    assert_eq!(doubled.nmd, 43);
    assert_eq!(doubled.nwd, 81);

    let tripled = mesh.expand_by_factor(3).expect("factor 3 expansion");
    assert_eq!(tripled.nmd, 93);
    assert_eq!(tripled.nwd, 181);

    let expanded_by_six = mesh.expand_by_factor(6).expect("factor 6 expansion");
    assert_eq!(expanded_by_six.nmd, 363);
    assert_eq!(expanded_by_six.nwd, 721);

    let err = mesh
        .expand_by_factor(5)
        .expect_err("factor 5 is not an Method-C expansion product");
    assert!(
        err.to_string().contains("2 and 3"),
        "unexpected error: {err}"
    );
}

#[test]
fn method_c_global_spring_preserves_topology_radius_and_moves_fortran_pentagons() {
    let mut mesh =
        MethodCDelaunayMesh::from_icosahedron(2, 0, 1.0, 0.25, 100).expect("valid Method-C mesh");
    let pentagon_id = mesh.impent[0];
    let adjacent_edge = mesh.m_neighbors[pentagon_id].iu[0];
    let [edge_start, edge_end] = mesh.u_edges[adjacent_edge].im;
    let regular_point_id = if edge_start == pentagon_id {
        edge_end
    } else {
        edge_start
    };
    let regular_point = mesh.m_points[regular_point_id];
    let perturbed = CartesianPoint::new(
        regular_point.x + 50_000.0,
        regular_point.y - 25_000.0,
        regular_point.z,
    );
    let scale = METHOD_C_CANONICAL_EARTH_RADIUS_METERS / magnitude(perturbed);
    mesh.m_points[regular_point_id] = CartesianPoint::new(
        perturbed.x * scale,
        perturbed.y * scale,
        perturbed.z * scale,
    );
    let original_points = mesh.m_points.clone();

    let adjusted = mesh
        .spring_global(2, 2)
        .expect("Method-C global spring adjustment");

    assert_eq!(adjusted.nmd, mesh.nmd);
    assert_eq!(adjusted.nud, mesh.nud);
    assert_eq!(adjusted.nwd, mesh.nwd);
    adjusted.validate_topology().expect("topology stays closed");

    for point_id in 2..=adjusted.nmd {
        let radius = magnitude(adjusted.m_points[point_id]);
        assert!(
            (radius - METHOD_C_CANONICAL_EARTH_RADIUS_METERS).abs() <= 1.0,
            "point {point_id} radius {radius}"
        );
    }

    assert!(
        distance(adjusted.m_points[pentagon_id], original_points[pentagon_id]) > 1.0e-3,
        "Fortran spring_dynamics1 updates pentagons; its freeze statement is commented out"
    );

    assert!(
        magnitude(CartesianPoint::new(
            adjusted.m_points[regular_point_id].x - original_points[regular_point_id].x,
            adjusted.m_points[regular_point_id].y - original_points[regular_point_id].y,
            adjusted.m_points[regular_point_id].z - original_points[regular_point_id].z,
        )) > 1.0e-3,
        "Method-C global spring should move the perturbed non-pentagon M point"
    );
    for point in adjusted.m_points.iter().skip(2) {
        assert_eq!(point.x, point.x as f32 as f64);
        assert_eq!(point.y, point.y as f32 as f64);
        assert_eq!(point.z, point.z as f32 as f64);
    }
}

#[test]
fn method_c_global_cartesian_spring_keeps_points_unprojected_like_canonical_mdomain_ge_two() {
    let mut mesh =
        MethodCDelaunayMesh::from_icosahedron(2, 0, 1.0, 0.25, 100).expect("valid Method-C mesh");
    let regular_point_id = (2..=mesh.nmd)
        .find(|point_id| !mesh.impent.contains(point_id))
        .expect("non-pentagon M point");
    let point = mesh.m_points[regular_point_id];
    mesh.m_points[regular_point_id] = CartesianPoint::new(point.x * 0.75, point.y * 0.75, point.z);

    let adjusted = mesh
        .spring_global_cartesian_with_controls(2, 1, 1_000_000.0, 0.035)
        .expect("Cartesian Method-C global spring adjustment");

    adjusted.validate_topology().expect("topology stays closed");
    assert!(
        magnitude(adjusted.m_points[regular_point_id]) < METHOD_C_CANONICAL_EARTH_RADIUS_METERS - 1.0,
        "Canonical spring_dynamics_globe only projects M points back to Earth radius when mdomain < 2"
    );
}

#[test]
fn method_c_global_cartesian_spring_uses_canonical_deltax_target_distance() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(2, 0, 1.0, 0.25, 100).expect("valid Method-C mesh");
    let regular_point_id = (2..=mesh.nmd)
        .find(|point_id| !mesh.impent.contains(point_id))
        .expect("non-pentagon M point");

    let small_deltax = mesh
        .spring_global_cartesian_with_controls(2, 1, 100_000.0, 0.035)
        .expect("Cartesian global spring with small deltax");
    let large_deltax = mesh
        .spring_global_cartesian_with_controls(2, 1, 2_000_000.0, 0.035)
        .expect("Cartesian global spring with large deltax");

    assert!(
        distance(
            small_deltax.m_points[regular_point_id],
            large_deltax.m_points[regular_point_id],
        ) > 1.0,
        "Canonical spring_dynamics_globe uses DELTAX, not spherical NXP spacing, for mdomain >= 2"
    );
}
