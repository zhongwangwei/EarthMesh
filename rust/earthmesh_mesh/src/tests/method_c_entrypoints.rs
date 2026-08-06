use super::*;

#[test]
fn method_c_public_spawn_entrypoints_use_same_table_path() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let selected = mesh
        .selected_region_faces(&region, 1, false)
        .expect("selected Method-C faces");
    let expected = mesh
        .spawn_nest_pass_with_max_mrows(&selected, 2, 7, true)
        .expect("direct Method-C pass");
    let expected_counts = (expected.nmd, expected.nud, expected.nwd);

    let surface = mesh
        .spawn_nest(std::slice::from_ref(&region), 1)
        .expect("public surface Method-C spawn");
    assert_eq!(
        (surface.nmd, surface.nud, surface.nwd),
        expected_counts,
        "spawn_nest should use the same Method-C table path as the direct pass"
    );
    surface
        .validate_topology()
        .expect("surface Method-C topology");

    let surface_alias = mesh
        .spawn_nest_as_surface(std::slice::from_ref(&region), 1)
        .expect("public surface alias Method-C spawn");
    assert_eq!(
        (surface_alias.nmd, surface_alias.nud, surface_alias.nwd),
        expected_counts,
        "spawn_nest_as_surface should use the same Method-C table path as spawn_nest"
    );
    surface_alias
        .validate_topology()
        .expect("surface alias Method-C topology");

    let explicit = mesh
        .spawn_nest_with_max_mrows(std::slice::from_ref(&region), 1, 7)
        .expect("explicit-width Method-C spawn");
    assert_eq!(
        (explicit.nmd, explicit.nud, explicit.nwd),
        expected_counts,
        "spawn_nest_with_max_mrows should use the same Method-C table path"
    );
    explicit
        .validate_topology()
        .expect("explicit-width Method-C topology");

    let atmosphere = mesh
        .spawn_nest_as_atmosmesh(std::slice::from_ref(&region), 1)
        .expect("public atmosphere Method-C spawn");
    assert_eq!(
        (atmosphere.nmd, atmosphere.nud, atmosphere.nwd),
        expected_counts,
        "spawn_nest_as_atmosmesh should change mrow width without leaving the Method-C table path"
    );
    atmosphere
        .validate_topology()
        .expect("atmosphere Method-C topology");

    let (spring, spring_passes) = mesh
        .spawn_nest_with_spring(std::slice::from_ref(&region), 1, 16, 0)
        .expect("public spring Method-C spawn");
    assert_eq!(spring_passes, 0);
    assert_eq!(
        (spring.nmd, spring.nud, spring.nwd),
        expected_counts,
        "spawn_nest_with_spring should use the same Method-C table path before optional springing"
    );
    spring
        .validate_topology()
        .expect("spring Method-C topology");

    let cart_mesh =
        MethodCDelaunayMesh::from_cart_hex(18, 1_000_000.0).expect("cart_hex Method-C mesh");
    let cart_region = RefinementRegion::Circle {
        center: LonLatDegrees::new(10_200_000.0, -310_000.0),
        radius_meters: 500_000.0,
        level: 1,
    };
    let cart_selected = cart_mesh
        .selected_region_faces(&cart_region, 1, true)
        .expect("selected Cartesian Method-C faces");
    let cart_expected = cart_mesh
        .spawn_nest_pass_with_max_mrows(
            &cart_selected,
            2,
            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
            false,
        )
        .expect("direct Cartesian Method-C pass");
    let cart_expected_counts = (cart_expected.nmd, cart_expected.nud, cart_expected.nwd);
    let cart_public = cart_mesh
        .spawn_nest_cartesian_xy_with_max_mrows(
            std::slice::from_ref(&cart_region),
            1,
            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
        )
        .expect("public Cartesian Method-C spawn");
    assert_eq!(
            (cart_public.nmd, cart_public.nud, cart_public.nwd),
            cart_expected_counts,
            "spawn_nest_cartesian_xy_with_max_mrows should use the same Method-C table path as the direct Cartesian pass"
        );
    cart_public
        .validate_topology()
        .expect("Cartesian Method-C topology");

    let (cart_spring, cart_spring_passes) = cart_mesh
        .spawn_nest_cartesian_xy_with_spring_and_max_mrows(
            std::slice::from_ref(&cart_region),
            1,
            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
            18,
            0,
        )
        .expect("public Cartesian spring Method-C spawn");
    assert_eq!(cart_spring_passes, 0);
    assert_eq!(
            (cart_spring.nmd, cart_spring.nud, cart_spring.nwd),
            cart_expected_counts,
            "spawn_nest_cartesian_xy_with_spring_and_max_mrows should use the same Method-C table path before optional springing"
        );
    cart_spring
        .validate_topology()
        .expect("Cartesian spring Method-C topology");
}

#[test]
fn method_c_spring_niter_keeps_table_path_and_closed_topology() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let radius = active_mesh_radius(&mesh).expect("active mesh radius");
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let selected = mesh
        .selected_region_faces(&region, 1, false)
        .expect("selected Method-C faces");
    let expected = mesh
        .spawn_nest_pass_with_max_mrows(&selected, 2, 7, true)
        .expect("direct Method-C pass");

    let (spring, spring_passes) = mesh
        .spawn_nest_with_spring(std::slice::from_ref(&region), 1, 16, 1)
        .expect("public spring Method-C spawn with iterations");

    assert_eq!(
        spring_passes, 1,
        "niter > 0 should run one Method-C nest spring pass after the active Method-C refinement pass"
    );
    assert_eq!(
            (spring.nmd, spring.nud, spring.nwd),
            (expected.nmd, expected.nud, expected.nwd),
            "spring_nest should relax the Method-C table output without changing its Canonical allocation counts"
        );
    spring
        .validate_topology()
        .expect("spring-relaxed Method-C topology");
    for im in 2..=spring.nmd {
        let delta = (magnitude(spring.m_points[im]) - radius).abs();
        assert!(
                delta < 0.5,
                "spring-relaxed Method-C M point {im} should stay projected on the Canonical real-valued active radius; delta={delta}"
            );
    }
}

#[test]
fn method_c_cartesian_spring_niter_keeps_table_path_and_closed_topology() {
    let mesh = MethodCDelaunayMesh::from_cart_hex(18, 1_000_000.0).expect("cart_hex Method-C mesh");
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(10_200_000.0, -310_000.0),
        radius_meters: 500_000.0,
        level: 1,
    };
    let selected = mesh
        .selected_region_faces(&region, 1, true)
        .expect("selected Cartesian Method-C faces");
    let expected = mesh
        .spawn_nest_pass_with_max_mrows(
            &selected,
            2,
            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
            false,
        )
        .expect("direct Cartesian Method-C pass");

    let (spring, spring_passes) = mesh
        .spawn_nest_cartesian_xy_with_spring_and_max_mrows(
            std::slice::from_ref(&region),
            1,
            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
            18,
            1,
        )
        .expect("public Cartesian spring Method-C spawn with iterations");

    assert_eq!(
            spring_passes, 1,
            "Cartesian niter > 0 should run one Method-C nest spring pass after the active Method-C refinement pass"
        );
    assert_eq!(
            (spring.nmd, spring.nud, spring.nwd),
            (expected.nmd, expected.nud, expected.nwd),
            "Cartesian spring_nest should relax the Method-C table output without changing Canonical allocation counts"
        );
    spring
        .validate_topology()
        .expect("Cartesian spring-relaxed Method-C topology");
    for im in 2..=spring.nmd {
        let point = spring.m_points[im];
        assert!(
            point.x.is_finite() && point.y.is_finite() && point.z.is_finite(),
            "Cartesian spring-relaxed Method-C M point {im} should remain finite"
        );
    }
}
#[test]
fn method_c_cartesian_deltax_spring_niter_keeps_table_path_and_closed_topology() {
    let deltax = 1_000_000.0;
    let mesh = MethodCDelaunayMesh::from_cart_hex(18, deltax).expect("cart_hex Method-C mesh");
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(10_200_000.0, -310_000.0),
        radius_meters: 500_000.0,
        level: 1,
    };
    let selected = mesh
        .selected_region_faces(&region, 1, true)
        .expect("selected Cartesian Method-C faces");
    let expected = mesh
        .spawn_nest_pass_with_max_mrows(
            &selected,
            2,
            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
            false,
        )
        .expect("direct Cartesian Method-C pass");

    let (spring, spring_passes) = mesh
        .spawn_nest_cartesian_xy_with_spring_deltax_and_max_mrows(
            std::slice::from_ref(&region),
            1,
            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
            18,
            1,
            deltax,
        )
        .expect("public Cartesian deltax spring Method-C spawn with iterations");

    assert_eq!(
            spring_passes, 1,
            "Cartesian deltax niter > 0 should run one Method-C nest spring pass after the active Method-C refinement pass"
        );
    assert_eq!(
            (spring.nmd, spring.nud, spring.nwd),
            (expected.nmd, expected.nud, expected.nwd),
            "Cartesian deltax spring_nest should relax the Method-C table output without changing Canonical allocation counts"
        );
    spring
        .validate_topology()
        .expect("Cartesian deltax spring-relaxed Method-C topology");
    for im in 2..=spring.nmd {
        let point = spring.m_points[im];
        assert!(
            point.x.is_finite() && point.y.is_finite() && point.z.is_finite(),
            "Cartesian deltax spring-relaxed Method-C M point {im} should remain finite"
        );
    }
}
