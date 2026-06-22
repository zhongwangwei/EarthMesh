use earthmesh_mesh::OLAM_FORTRAN_EARTH_RADIUS_METERS;
use earthmesh_mesh::{
    olam_gridinit_factorization_fortran, voronoi_grid_from_olam_delaunay_mesh, CartesianPoint,
    OlamDelaunayMesh,
};

fn magnitude(point: CartesianPoint) -> f64 {
    (point.x * point.x + point.y * point.y + point.z * point.z).sqrt()
}

fn distance(a: CartesianPoint, b: CartesianPoint) -> f64 {
    magnitude(CartesianPoint::new(a.x - b.x, a.y - b.y, a.z - b.z))
}

#[test]
fn olam_delaunay_mesh_from_icosahedron_has_closed_muw_topology() {
    let mesh = OlamDelaunayMesh::from_icosahedron(1, 0, 1.0, 0.25, 100)
        .expect("valid OLAM icosahedron mesh");

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
            (radius - OLAM_FORTRAN_EARTH_RADIUS_METERS).abs() <= 1.0,
            "point {point_id} radius {radius}"
        );
    }
}

#[test]
fn olam_cart_hex_mdomain_five_uses_fortran_planar_counts_and_coordinates() {
    let mesh = OlamDelaunayMesh::from_cart_hex(2, 1000.0).expect("valid OLAM cart_hex mesh");

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
            magnitude(mesh.m_points[point_id]) < OLAM_FORTRAN_EARTH_RADIUS_METERS / 100.0,
            "cart_hex point {point_id} should remain in the local Cartesian plane"
        );
    }
}

#[test]
fn olam_cart_hex_rejects_deltax_below_fortran_lower_bound() {
    let err = OlamDelaunayMesh::from_cart_hex(2, 0.0009)
        .expect_err("Fortran cart_hex rejects DELTAX below 0.001");

    assert!(
        err.to_string().contains("DELTAX"),
        "unexpected error: {err}"
    );
}

#[test]
fn olam_cart_hex_fills_first_fortran_u_and_w_neighbors() {
    let mesh = OlamDelaunayMesh::from_cart_hex(2, 1000.0).expect("valid OLAM cart_hex mesh");

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
fn olam_cart_hex_derives_interior_m_neighbors_from_fortran_incidence() {
    let mesh = OlamDelaunayMesh::from_cart_hex(2, 1000.0).expect("valid OLAM cart_hex mesh");

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
fn olam_cart_hex_preserves_fortran_boundary_periodic_maps() {
    let mesh = OlamDelaunayMesh::from_cart_hex(2, 1000.0).expect("valid OLAM cart_hex mesh");

    assert_eq!(mesh.m_prognostic[2], 15);
    assert_eq!(mesh.u_prognostic[2], 24);
    assert_eq!(mesh.u_prognostic[4], 35);
    assert_eq!(mesh.w_prognostic[16], 39);
    assert_eq!(mesh.w_prognostic[8], 28);
}

#[test]
fn olam_cart_hex_ghost_w_faces_copy_fortran_periodic_partner_topology() {
    let mesh = OlamDelaunayMesh::from_cart_hex(2, 1000.0).expect("valid OLAM cart_hex mesh");

    let ghost = 16;
    let partner = mesh.w_prognostic[ghost];
    assert_eq!(partner, 39);
    assert_eq!(mesh.w_faces[ghost].npoly, 3);
    assert_eq!(mesh.w_faces[ghost].im, mesh.w_faces[partner].im);
    assert_eq!(mesh.w_faces[ghost].iu[0], 2);
}

#[test]
fn olam_cart_hex_topology_validation_allows_fortran_periodic_ghost_faces() {
    let mesh = OlamDelaunayMesh::from_cart_hex(5, 1000.0).expect("valid OLAM cart_hex mesh");

    mesh.validate_topology()
        .expect("Fortran cart_hex ghost W faces are validated through w_prognostic");
}

#[test]
fn olam_cart_hex_orders_outer_w_faces_for_fill_rad3_sectors() {
    let mesh = OlamDelaunayMesh::from_cart_hex(5, 1000.0).expect("valid OLAM cart_hex mesh");

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
fn olam_delaunay_mesh_from_icosahedron_uses_olam_global_spring_pentagon_rule() {
    let unsprung = OlamDelaunayMesh::from_icosahedron(2, 0, 1.0, 0.25, 100)
        .expect("unsprung OLAM icosahedron mesh");
    let sprung = OlamDelaunayMesh::from_icosahedron(2, 2, 1.0, 0.25, 100)
        .expect("sprung OLAM icosahedron mesh");

    for &pentagon_id in &unsprung.impent {
        assert_eq!(
            sprung.m_points[pentagon_id], unsprung.m_points[pentagon_id],
            "OLAM global spring must not move impent pentagon point {pentagon_id}"
        );
    }
}

#[test]
fn olam_delaunay_mesh_can_drive_voronoi_grid_generation() {
    let mesh = OlamDelaunayMesh::from_icosahedron(1, 0, 1.0, 0.25, 100)
        .expect("valid OLAM icosahedron mesh");

    let state = voronoi_grid_from_olam_delaunay_mesh(&mesh, OLAM_FORTRAN_EARTH_RADIUS_METERS)
        .expect("OLAM Voronoi state");

    assert_eq!(state.grid.nma, mesh.nwd);
    assert_eq!(state.grid.nua, mesh.nud);
    assert_eq!(state.grid.nva, mesh.nud);
    assert_eq!(state.grid.nwa, mesh.nmd);
    assert_eq!(
        state.tabs.m[2].iw,
        mesh.w_faces[2].im.map(|value| value as i32)
    );
    assert_eq!(state.tabs.w[2].npoly, mesh.m_neighbors[2].npoly as i32);
    assert_eq!(state.grid.xew[2], mesh.m_points[2].x as f32);
    assert_eq!(state.grid.yew[2], mesh.m_points[2].y as f32);
    assert_eq!(state.grid.zew[2], mesh.m_points[2].z as f32);
}

#[test]
fn olam_gridinit_factorization_matches_get_factors_selection_rules() {
    let nxp16 = olam_gridinit_factorization_fortran(16).expect("NXP 16 is valid");
    assert_eq!(nxp16.base_nxp, 16);
    assert_eq!(nxp16.expansion_factor, 1);

    let nxp72 = olam_gridinit_factorization_fortran(72).expect("NXP 72 is valid");
    assert_eq!(nxp72.base_nxp, 36);
    assert_eq!(nxp72.expansion_factor, 2);

    let nxp96 = olam_gridinit_factorization_fortran(96).expect("NXP 96 is valid");
    assert_eq!(nxp96.base_nxp, 32);
    assert_eq!(nxp96.expansion_factor, 3);
}

#[test]
fn olam_expand_global2_subdivides_each_triangle_and_rebuilds_topology() {
    let mesh =
        OlamDelaunayMesh::from_icosahedron(1, 0, 1.0, 0.25, 100).expect("valid base OLAM mesh");

    let expanded = mesh.expand_global2().expect("factor-2 OLAM expansion");

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
            (radius - OLAM_FORTRAN_EARTH_RADIUS_METERS).abs() <= 1.0,
            "point {point_id} radius {radius}"
        );
    }
}

#[test]
fn olam_expand_global3_trisects_each_triangle_and_rebuilds_topology() {
    let mesh =
        OlamDelaunayMesh::from_icosahedron(1, 0, 1.0, 0.25, 100).expect("valid base OLAM mesh");

    let expanded = mesh.expand_global3().expect("factor-3 OLAM expansion");

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
            (radius - OLAM_FORTRAN_EARTH_RADIUS_METERS).abs() <= 1.0,
            "point {point_id} radius {radius}"
        );
    }
}

#[test]
fn olam_expand_by_factor_applies_factor2_and_rejects_unsupported_products() {
    let mesh =
        OlamDelaunayMesh::from_icosahedron(1, 0, 1.0, 0.25, 100).expect("valid base OLAM mesh");

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
        .expect_err("factor 5 is not an OLAM expansion product");
    assert!(
        err.to_string().contains("2 and 3"),
        "unexpected error: {err}"
    );
}

#[test]
fn olam_global_spring_preserves_topology_radius_and_pentagons() {
    let mut mesh =
        OlamDelaunayMesh::from_icosahedron(2, 0, 1.0, 0.25, 100).expect("valid OLAM mesh");
    let regular_point_id = (2..=mesh.nmd)
        .find(|point_id| !mesh.impent.contains(point_id))
        .expect("non-pentagon M point");
    let regular_point = mesh.m_points[regular_point_id];
    let perturbed = CartesianPoint::new(
        regular_point.x + 50_000.0,
        regular_point.y - 25_000.0,
        regular_point.z,
    );
    let scale = OLAM_FORTRAN_EARTH_RADIUS_METERS / magnitude(perturbed);
    mesh.m_points[regular_point_id] = CartesianPoint::new(
        perturbed.x * scale,
        perturbed.y * scale,
        perturbed.z * scale,
    );
    let original_points = mesh.m_points.clone();

    let adjusted = mesh
        .spring_global(2, 2)
        .expect("OLAM global spring adjustment");

    assert_eq!(adjusted.nmd, mesh.nmd);
    assert_eq!(adjusted.nud, mesh.nud);
    assert_eq!(adjusted.nwd, mesh.nwd);
    adjusted.validate_topology().expect("topology stays closed");

    for point_id in 2..=adjusted.nmd {
        let radius = magnitude(adjusted.m_points[point_id]);
        assert!(
            (radius - OLAM_FORTRAN_EARTH_RADIUS_METERS).abs() <= 1.0,
            "point {point_id} radius {radius}"
        );
    }

    for &pentagon_id in &mesh.impent {
        assert_eq!(
            adjusted.m_points[pentagon_id], original_points[pentagon_id],
            "OLAM global spring must keep impent pentagon point {pentagon_id} fixed"
        );
    }

    assert!(
        magnitude(CartesianPoint::new(
            adjusted.m_points[regular_point_id].x - original_points[regular_point_id].x,
            adjusted.m_points[regular_point_id].y - original_points[regular_point_id].y,
            adjusted.m_points[regular_point_id].z - original_points[regular_point_id].z,
        )) > 1.0e-3,
        "OLAM global spring should move the perturbed non-pentagon M point"
    );
}

#[test]
fn olam_global_cartesian_spring_keeps_points_unprojected_like_fortran_mdomain_ge_two() {
    let mut mesh =
        OlamDelaunayMesh::from_icosahedron(2, 0, 1.0, 0.25, 100).expect("valid OLAM mesh");
    let regular_point_id = (2..=mesh.nmd)
        .find(|point_id| !mesh.impent.contains(point_id))
        .expect("non-pentagon M point");
    let point = mesh.m_points[regular_point_id];
    mesh.m_points[regular_point_id] = CartesianPoint::new(point.x * 0.75, point.y * 0.75, point.z);

    let adjusted = mesh
        .spring_global_cartesian_with_controls(2, 1, 1_000_000.0, 0.035)
        .expect("Cartesian OLAM global spring adjustment");

    adjusted.validate_topology().expect("topology stays closed");
    assert!(
        magnitude(adjusted.m_points[regular_point_id]) < OLAM_FORTRAN_EARTH_RADIUS_METERS - 1.0,
        "Fortran spring_dynamics_globe only projects M points back to Earth radius when mdomain < 2"
    );
}

#[test]
fn olam_global_cartesian_spring_uses_fortran_deltax_target_distance() {
    let mesh =
        OlamDelaunayMesh::from_icosahedron(2, 0, 1.0, 0.25, 100).expect("valid OLAM mesh");
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
        "Fortran spring_dynamics_globe uses DELTAX, not spherical NXP spacing, for mdomain >= 2"
    );
}
