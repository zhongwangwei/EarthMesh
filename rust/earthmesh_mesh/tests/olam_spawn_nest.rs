use earthmesh_mesh::OLAM_FORTRAN_EARTH_RADIUS_METERS;
use earthmesh_mesh::{CartesianPoint, LonLatDegrees, OlamDelaunayMesh, OlamRefinementRegion};

fn magnitude(point: earthmesh_mesh::CartesianPoint) -> f64 {
    (point.x * point.x + point.y * point.y + point.z * point.z).sqrt()
}

fn distance(a: CartesianPoint, b: CartesianPoint) -> f64 {
    magnitude(CartesianPoint::new(a.x - b.x, a.y - b.y, a.z - b.z))
}

fn lonlat_from_cartesian(point: CartesianPoint) -> LonLatDegrees {
    let lon = point.y.atan2(point.x).to_degrees();
    let lat = point
        .z
        .atan2((point.x * point.x + point.y * point.y).sqrt())
        .to_degrees();
    LonLatDegrees::new(lon, lat)
}

fn dot(a: CartesianPoint, b: CartesianPoint) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

fn assert_prognostic_copies_point_to_identity(name: &str, map: &[usize]) {
    for (id, &partner) in map.iter().enumerate().skip(2) {
        if partner > 1 && partner != id {
            assert!(
                partner < map.len(),
                "{name} prognostic copy {id} points outside map to {partner}"
            );
            assert_eq!(
                map[partner], partner,
                "{name} prognostic copy {id} should point to an identity/prognostic owner, got owner {partner} -> {}",
                map[partner]
            );
        }
    }
}

fn non_pentagon_point_away_from_pentagons(mesh: &OlamDelaunayMesh) -> usize {
    (2..=mesh.nmd)
        .find(|id| {
            if mesh.impent.contains(id) || mesh.m_neighbors[*id].npoly != 6 {
                return false;
            }
            let point = mesh.m_points[*id];
            let point_radius = magnitude(point);
            mesh.impent.iter().all(|&pentagon_id| {
                let pentagon = mesh.m_points[pentagon_id];
                dot(point, pentagon) / (point_radius * magnitude(pentagon)) < 0.9
            })
        })
        .expect("six-sided M point away from pentagons")
}

#[test]
fn spawn_nest_refines_one_circle_without_refining_the_whole_globe() {
    let mesh = OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let global_tripled = mesh.expand_global3().expect("global factor-3 expansion");
    let region = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };

    let refined = mesh.spawn_nest(&[region], 5).expect("local circle nest");
    assert!(refined.nwd > mesh.nwd, "nest should add child faces");
    assert!(
        refined.nwd < global_tripled.nwd,
        "local nest should not degrade into whole-globe expansion: base {}, local {}, global {}",
        mesh.nwd,
        refined.nwd,
        global_tripled.nwd
    );
    assert!(
        refined.w_faces.iter().skip(2).any(|face| face.mrlw >= 2),
        "selected region should contain level-2 faces"
    );
    assert!(
        !refined.boundary_rows().is_empty(),
        "local refinement should record transition rows"
    );
    assert!(
        refined.w_faces.iter().skip(2).any(|face| face.mrow < 0),
        "local refinement should mark inner transition rows with negative mrow"
    );
    assert!(
        refined.w_faces.iter().skip(2).any(|face| face.mrow > 0),
        "local refinement should mark outer transition rows with positive mrow"
    );

    let report = refined.validate_topology().expect("refined topology");
    assert_eq!(report.checked_m_points, refined.nmd - 1);
    assert_eq!(report.checked_u_edges, refined.nud - 1);
    assert_eq!(report.checked_w_faces, refined.nwd - 1);
}

#[test]
fn spawn_nest_cartesian_xy_keeps_method_c_points_unprojected_like_fortran_mdomain_ge_two() {
    let mesh = OlamDelaunayMesh::from_cart_hex(18, 1_000_000.0).expect("cart_hex OLAM mesh");
    let region = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(10_200_000.0, -310_000.0),
        radius_meters: 500_000.0,
        level: 1,
    };

    let refined = mesh
        .spawn_nest_cartesian_xy_with_max_mrows(
            &[region],
            1,
            OlamDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
        )
        .expect("Cartesian Method-C nest");

    assert!(
        refined
            .m_points
            .iter()
            .zip(refined.m_point_metadata().iter())
            .skip(2)
            .any(|(point, metadata)| {
                metadata.ngr > 1
                    && metadata.mrlm_orig >= 2
                    && (magnitude(*point) - OLAM_FORTRAN_EARTH_RADIUS_METERS).abs() > 1.0
            }),
        "Fortran spawn_nest only projects coordinates back to Earth radius when mdomain < 2"
    );
}

#[test]
fn spawn_nest_cart_hex_preserves_fortran_periodic_prognostic_maps() {
    let mesh = OlamDelaunayMesh::from_cart_hex(18, 1_000_000.0).expect("cart_hex OLAM mesh");
    let original_m_copies = mesh
        .m_prognostic
        .iter()
        .enumerate()
        .skip(2)
        .filter(|(id, &partner)| partner > 1 && partner != *id)
        .count();
    let original_u_copies = mesh
        .u_prognostic
        .iter()
        .enumerate()
        .skip(2)
        .filter(|(id, &partner)| partner > 1 && partner != *id)
        .count();
    let original_w_copies = mesh
        .w_prognostic
        .iter()
        .enumerate()
        .skip(2)
        .filter(|(id, &partner)| partner > 1 && partner != *id)
        .count();
    assert!(
        original_m_copies > 0 && original_u_copies > 0 && original_w_copies > 0,
        "cart_hex fixture must contain Fortran periodic M/U/W copy maps"
    );
    let region = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(10_200_000.0, -310_000.0),
        radius_meters: 500_000.0,
        level: 1,
    };

    let refined = mesh
        .spawn_nest_cartesian_xy_with_max_mrows(
            &[region],
            1,
            OlamDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
        )
        .expect("cart_hex Method-C nest");

    refined
        .validate_topology()
        .expect("cart_hex Method-C topology with periodic copies");
    assert_prognostic_copies_point_to_identity("M", &refined.m_prognostic);
    assert_prognostic_copies_point_to_identity("U", &refined.u_prognostic);
    assert_prognostic_copies_point_to_identity("W", &refined.w_prognostic);
    assert!(
        refined
            .m_prognostic
            .iter()
            .enumerate()
            .skip(2)
            .filter(|(id, &partner)| partner > 1 && partner != *id)
            .count()
            >= original_m_copies,
        "Method-C spawn must preserve Fortran cart_hex M periodic copy mapping"
    );
    assert!(
        refined
            .u_prognostic
            .iter()
            .enumerate()
            .skip(2)
            .filter(|(id, &partner)| partner > 1 && partner != *id)
            .count()
            >= original_u_copies,
        "Method-C spawn must preserve Fortran cart_hex U periodic copy mapping"
    );
    assert!(
        refined
            .w_prognostic
            .iter()
            .enumerate()
            .skip(2)
            .filter(|(id, &partner)| partner > 1 && partner != *id)
            .count()
            >= original_w_copies,
        "Method-C spawn must preserve Fortran cart_hex W periodic copy mapping"
    );
}

#[test]
fn spawn_nest_cartesian_xy_spring_keeps_movable_points_unprojected_like_fortran_mdomain_ge_two() {
    let mesh = OlamDelaunayMesh::from_cart_hex(18, 1_000_000.0).expect("cart_hex OLAM mesh");
    let region = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(10_200_000.0, -310_000.0),
        radius_meters: 500_000.0,
        level: 1,
    };

    let (refined, spring_passes) = mesh
        .spawn_nest_cartesian_xy_with_spring_and_max_mrows(
            &[region],
            1,
            OlamDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
            6,
            1,
        )
        .expect("Cartesian Method-C nest with spring");

    assert_eq!(spring_passes, 1);

    let movable_child_points: Vec<usize> = (2..=refined.nmd)
        .filter(|&point_id| {
            refined.m_point_metadata()[point_id].ngr == 2
                && refined.m_neighbors[point_id]
                    .iw
                    .iter()
                    .take(refined.m_neighbors[point_id].npoly)
                    .any(|&face_id| refined.w_faces[face_id].mrow != 0)
        })
        .collect();

    assert!(
        !movable_child_points.is_empty(),
        "test case should include transition-row M points moved by spring"
    );
    assert!(
        movable_child_points.iter().any(|&point_id| {
            (magnitude(refined.m_points[point_id]) - OLAM_FORTRAN_EARTH_RADIUS_METERS).abs() > 1.0
        }),
        "Fortran spring_dynamics_nest only projects moved M points back to Earth radius when mdomain < 2"
    );
}

#[test]
fn spawn_nest_cartesian_xy_spring_uses_fortran_deltax_target_distance_for_mdomain_ge_two() {
    let mesh = OlamDelaunayMesh::from_cart_hex(18, 1_000_000.0).expect("cart_hex OLAM mesh");
    let region = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(10_200_000.0, -310_000.0),
        radius_meters: 500_000.0,
        level: 1,
    };

    let (small_deltax, small_passes) = mesh
        .spawn_nest_cartesian_xy_with_spring_deltax_and_max_mrows(
            std::slice::from_ref(&region),
            1,
            OlamDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
            6,
            1,
            100_000.0,
        )
        .expect("Cartesian Method-C nest with small deltax spring");
    let (large_deltax, large_passes) = mesh
        .spawn_nest_cartesian_xy_with_spring_deltax_and_max_mrows(
            &[region],
            1,
            OlamDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
            6,
            1,
            2_000_000.0,
        )
        .expect("Cartesian Method-C nest with large deltax spring");

    assert_eq!(small_passes, 1);
    assert_eq!(large_passes, 1);
    assert_eq!(small_deltax.nmd, large_deltax.nmd);

    assert!(
        (2..=small_deltax.nmd).any(|point_id| {
            small_deltax.m_point_metadata()[point_id].ngr == 2
                && small_deltax.m_neighbors[point_id]
                    .iw
                    .iter()
                    .take(small_deltax.m_neighbors[point_id].npoly)
                    .any(|&face_id| small_deltax.w_faces[face_id].mrow != 0)
                && distance(
                    small_deltax.m_points[point_id],
                    large_deltax.m_points[point_id],
                ) > 1.0
        }),
        "Fortran spring_dynamics_nest uses NL%deltax, not spherical NXP spacing, for mdomain >= 2"
    );
}

#[test]
fn spawn_nest_cartesian_xy_spring_rejects_deltax_below_fortran_lower_bound() {
    let mesh = OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let region = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(0.0, 0.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };

    let err = mesh
        .spawn_nest_cartesian_xy_with_spring_deltax_and_max_mrows(
            &[region],
            1,
            OlamDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
            6,
            1,
            0.0005,
        )
        .expect_err("Fortran rejects DELTAX below dzxmin");

    assert!(
        err.to_string().contains("deltax"),
        "error should identify deltax: {err}"
    );
}

#[test]
fn spawn_nest_rejects_tiny_contained_region_that_crosses_method_c_boundary() {
    let mesh = OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let point_id = non_pentagon_point_away_from_pentagons(&mesh);
    let region = OlamRefinementRegion::Circle {
        center: lonlat_from_cartesian(mesh.m_points[point_id]),
        radius_meters: 1.0,
        level: 1,
    };

    let error = mesh.spawn_nest(&[region], 1).expect_err(
        "Fortran Method-C rejects a tiny nest that is too close to the coarse boundary",
    );

    assert!(
        error
            .to_string()
            .contains("Method-C perimeter length invalid"),
        "unexpected error: {error}"
    );
}

#[test]
fn spawn_nest_uses_one_imbeg_for_disconnected_same_level_regions_like_fortran_method_c() {
    let mesh = OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let regions = [
        OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(-120.0, 0.0),
            radius_meters: 500_000.0,
            level: 1,
        },
        OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(60.0, 0.0),
            radius_meters: 500_000.0,
            level: 1,
        },
    ];

    let refined = mesh
        .spawn_nest(&regions, 1)
        .expect("Fortran Method-C starts from one IMBEG for one spawned grid");
    let spawned_grid_numbers = refined
        .w_faces
        .iter()
        .skip(2)
        .filter_map(|face| (face.ngr > 1).then_some(face.ngr))
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(
        spawned_grid_numbers,
        std::collections::BTreeSet::from([2]),
        "one Fortran spawn_nest pass should create one spawned grid, not sibling grids"
    );
    refined
        .validate_topology()
        .expect("single-IMBEG disconnected same-level topology");
}

#[test]
fn spawn_nest_uses_one_imbeg_for_disconnected_mixed_level_regions_like_fortran_method_c() {
    let mesh = OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let high_center = LonLatDegrees::new(-120.0, 0.0);
    let regions = [
        OlamRefinementRegion::Circle {
            center: high_center,
            radius_meters: 1_500_000.0,
            level: 1,
        },
        OlamRefinementRegion::Circle {
            center: high_center,
            radius_meters: 350_000.0,
            level: 2,
        },
        OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(60.0, 0.0),
            radius_meters: 500_000.0,
            level: 1,
        },
    ];

    let refined = mesh
        .spawn_nest(&regions, 5)
        .expect("Fortran Method-C starts each pass from one IMBEG");
    let spawned_grid_numbers = refined
        .w_faces
        .iter()
        .skip(2)
        .filter_map(|face| (face.ngr > 1).then_some(face.ngr))
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(
        spawned_grid_numbers,
        std::collections::BTreeSet::from([2, 3]),
        "two Fortran spawn_nest passes should create one spawned grid per pass"
    );
    refined
        .validate_topology()
        .expect("single-IMBEG disconnected mixed-level topology");
}

#[test]
fn spawn_nest_uses_current_levels_only_when_max_level_exceeds_input() {
    let mesh = OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let region = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(-120.0, 0.0),
        radius_meters: 500_000.0,
        level: 2,
    };

    let refined = mesh
        .spawn_nest(&[region], 5)
        .expect("Fortran Method-C should not spawn empty passes");
    let spawned_grid_numbers = refined
        .w_faces
        .iter()
        .skip(2)
        .filter_map(|face| (face.ngr > 1).then_some(face.ngr))
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(
        spawned_grid_numbers,
        std::collections::BTreeSet::from([2]),
        "max_level larger than input levels should still spawn exactly one nested grid"
    );
}

#[test]
fn spawn_nest_continues_grid_numbers_on_already_nested_mesh() {
    let mesh = OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let first_region = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(-120.0, 0.0),
        radius_meters: 500_000.0,
        level: 1,
    };
    let second_region = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(60.0, 0.0),
        radius_meters: 500_000.0,
        level: 1,
    };

    let first = mesh
        .spawn_nest(&[first_region], 1)
        .expect("first Method-C nest");
    let max_existing_ngr = first
        .w_faces
        .iter()
        .skip(2)
        .map(|face| face.ngr)
        .max()
        .expect("existing W faces");
    let second = first
        .spawn_nest(&[second_region], 1)
        .expect("second Method-C nest on already nested mesh");
    let new_grid_numbers = second
        .w_faces
        .iter()
        .skip(2)
        .filter_map(|face| (face.ngr > max_existing_ngr).then_some(face.ngr))
        .collect::<std::collections::BTreeSet<_>>();

    assert!(
        !new_grid_numbers.is_empty(),
        "Fortran grid numbers are globally unique across spawned grids; no ngr greater than {max_existing_ngr} was created"
    );
}

#[test]
fn spawn_nest_rejects_mixed_regions_when_child_crosses_parent_mrow_like_fortran() {
    let mesh = OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let low = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(-70.0, -20.0),
        radius_meters: 2_000_000.0,
        level: 1,
    };
    let high = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 800_000.0,
        level: 2,
    };
    let high_parent = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 3_000_000.0,
        level: 1,
    };

    let error = mesh
        .spawn_nest(&[low, high_parent, high], 5)
        .expect_err("Fortran Method-C rejects child grids that cross parent mrow boundaries");
    assert!(
        error.to_string().contains("mrow") || error.to_string().contains("parent boundary"),
        "unexpected error: {error}"
    );
}

#[test]
fn spawn_nest_two_level_china_regions_keep_olam_valence_limit() {
    let mesh = OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let regions = [
        OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 1_500_000.0,
            level: 1,
        },
        OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(90.0, 25.0),
            radius_meters: 1_500_000.0,
            level: 1,
        },
        OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 500_000.0,
            level: 2,
        },
        OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(90.0, 25.0),
            radius_meters: 500_000.0,
            level: 2,
        },
    ];

    let refined = mesh
        .spawn_nest(&regions, 5)
        .expect("Fortran Method-C uses one spawned grid per pass");

    assert!(refined.nwd > mesh.nwd);
    refined
        .validate_topology()
        .expect("two-level China single-IMBEG topology");
    for im in 2..=mesh.nmd {
        assert!(
            refined.m_neighbors[im].npoly <= 7,
            "old M point {im} exceeds OLAM-supported valence after Method-C nesting"
        );
    }
}

#[test]
fn spawn_nest_rejects_two_level_polygon_without_explicit_parent_halo_like_fortran() {
    let mesh = OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let region = OlamRefinementRegion::Polygon {
        points: vec![
            LonLatDegrees::new(100.0, 15.0),
            LonLatDegrees::new(130.0, 15.0),
            LonLatDegrees::new(130.0, 35.0),
            LonLatDegrees::new(100.0, 35.0),
        ],
        level: 2,
    };

    let error = mesh
        .spawn_nest(&[region], 5)
        .expect_err("Fortran Method-C does not synthesize a parent halo for future grids");
    assert!(
        error.to_string().contains("perimeter length")
            || error.to_string().contains("parent boundary"),
        "unexpected error: {error}"
    );
}

#[test]
fn debug_single_west_china_method_c_region() {
    let mesh = OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let region = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(90.0, 25.0),
        radius_meters: 500_000.0,
        level: 1,
    };
    let refined = mesh.spawn_nest(&[region], 1).expect("west region");
    refined.validate_topology().expect("west region topology");
}

#[test]
fn debug_single_east_china_method_c_region() {
    let mesh = OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let region = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 500_000.0,
        level: 1,
    };
    let refined = mesh.spawn_nest(&[region], 1).expect("east region");
    refined.validate_topology().expect("east region topology");
}

#[test]
fn debug_single_south_america_method_c_region() {
    let mesh = OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let region = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(-70.0, -20.0),
        radius_meters: 2_000_000.0,
        level: 1,
    };
    let error = mesh
        .spawn_nest(&[region], 1)
        .expect_err("Fortran Method-C rejects this coarse South America region");

    assert!(
        error
            .to_string()
            .contains("Method-C perimeter length invalid"),
        "unexpected error: {error}"
    );
}

#[test]
fn spawn_nest_rejects_overlapping_china_regions_with_method_c_perimeter_error() {
    let mesh = OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let regions = [
        OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 500_000.0,
            level: 3,
        },
        OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(112.0, 24.0),
            radius_meters: 500_000.0,
            level: 3,
        },
    ];

    let error = mesh
        .spawn_nest(&regions, 5)
        .expect_err("Fortran Method-C rejects overlapping local-rebuild-style nests");

    assert!(
        error.to_string().contains("perimeter length")
            || error.to_string().contains("thirdm opposite U edge"),
        "unexpected error: {error}"
    );
}

#[test]
fn spawn_nest_with_spring_uses_one_imbeg_for_disconnected_regions() {
    let mesh = OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let regions = [
        OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 500_000.0,
            level: 1,
        },
        OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(90.0, 25.0),
            radius_meters: 500_000.0,
            level: 1,
        },
    ];

    let (refined, _) = mesh
        .spawn_nest_with_spring(&regions, 5, 16, 1)
        .expect("Fortran Method-C uses one spawned grid before spring relaxation");

    assert!(refined.nwd > mesh.nwd);
    refined
        .validate_topology()
        .expect("single-IMBEG spring topology");
}

#[test]
fn spawn_nest_with_spring_runs_per_pass_for_mixed_level_regions_like_fortran() {
    let mesh = OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let regions = [
        OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 1_500_000.0,
            level: 1,
        },
        OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 500_000.0,
            level: 2,
        },
    ];

    let (refined, spring_passes) = mesh
        .spawn_nest_with_spring(&regions, 5, 12, 1)
        .expect("mixed-level spring nest");
    let spawned_grid_numbers = refined
        .w_faces
        .iter()
        .skip(2)
        .filter_map(|face| (face.ngr > 1).then_some(face.ngr))
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(
        spring_passes, 2,
        "one spring pass should run for each spawned nested pass"
    );
    assert_eq!(
        spawned_grid_numbers,
        std::collections::BTreeSet::from([2, 3]),
        "mixed-level spring should spawn level-1 and level-2 grids only"
    );
}

#[test]
fn spawn_nest_accepts_bbox_regions() {
    let mesh = OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let region = OlamRefinementRegion::Bbox {
        west_degrees: 100.0,
        east_degrees: 125.0,
        south_degrees: 15.0,
        north_degrees: 35.0,
        level: 1,
    };

    let refined = mesh.spawn_nest(&[region], 5).expect("bbox nest");

    assert!(refined.nwd > mesh.nwd);
    assert!(!refined.boundary_rows().is_empty());
    refined.validate_topology().expect("bbox topology");
}

#[test]
fn spawn_nest_accepts_coarse_calculated_bbox_region() {
    let mesh = OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let region = OlamRefinementRegion::Bbox {
        west_degrees: 105.0,
        east_degrees: 125.0,
        south_degrees: 15.0,
        north_degrees: 35.0,
        level: 1,
    };

    let refined = mesh.spawn_nest(&[region], 5).expect("coarse bbox nest");

    assert!(refined.nwd > mesh.nwd);
    assert!(!refined.boundary_rows().is_empty());
    refined.validate_topology().expect("coarse bbox topology");
}

#[test]
fn spawn_nest_uses_olam_surface_transition_row_width() {
    let mesh = OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let region = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 500_000.0,
        level: 1,
    };

    let refined = mesh.spawn_nest(&[region], 5).expect("local circle nest");
    let max_abs_mrow = refined
        .w_faces
        .iter()
        .skip(2)
        .map(|face| face.mrow.unsigned_abs())
        .max()
        .unwrap_or_default();

    assert!(
        max_abs_mrow >= 4,
        "OLAM surface spawn_nest should keep a multi-row transition band; got max |mrow|={max_abs_mrow}"
    );
}

#[test]
fn spawn_nest_explicit_max_mrows_controls_transition_width() {
    let mesh = OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let region = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 500_000.0,
        level: 1,
    };

    let narrow = mesh
        .spawn_nest_with_max_mrows(
            std::slice::from_ref(&region),
            5,
            OlamDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
        )
        .expect("surface-width nest");
    let wide = mesh
        .spawn_nest_with_max_mrows(
            std::slice::from_ref(&region),
            5,
            OlamDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
        )
        .expect("atmos-width nest");

    let narrow_max_abs_mrow = narrow
        .w_faces
        .iter()
        .skip(2)
        .map(|face| face.mrow.unsigned_abs())
        .max()
        .unwrap_or_default();
    let wide_max_abs_mrow = wide
        .w_faces
        .iter()
        .skip(2)
        .map(|face| face.mrow.unsigned_abs())
        .max()
        .unwrap_or_default();

    assert!(
        wide_max_abs_mrow >= narrow_max_abs_mrow,
        "larger max_mrows should not shrink transition-band width: narrow={narrow_max_abs_mrow}, wide={wide_max_abs_mrow}"
    );
    assert!(wide_max_abs_mrow <= OlamDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS);
    assert!(narrow_max_abs_mrow <= OlamDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE);
}

#[test]
fn spawn_nest_explicit_widths_change_transition_band_for_same_input() {
    let mesh = OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let region = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 500_000.0,
        level: 1,
    };

    let narrow = mesh
        .spawn_nest_with_max_mrows(
            std::slice::from_ref(&region),
            5,
            OlamDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
        )
        .expect("surface-width nest");
    let wide = mesh
        .spawn_nest_with_max_mrows(
            std::slice::from_ref(&region),
            5,
            OlamDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
        )
        .expect("atmos-width nest");

    assert!(
        wide.nwd >= narrow.nwd,
        "larger max_mrows should not reduce refined faces: wide={} narrow={}",
        wide.nwd,
        narrow.nwd
    );
    assert!(
        wide.boundary_rows().len() > narrow.boundary_rows().len(),
        "larger max_mrows should produce a wider transition band: wide={} narrow={}",
        wide.boundary_rows().len(),
        narrow.boundary_rows().len()
    );
}

#[test]
fn spawn_nest_olam_method_c_constant_widths_match_fortran_defaults() {
    assert_eq!(OlamDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS, 13);
    assert_eq!(OlamDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE, 7);
    // (the exact-value assertions above already pin ATMOS > SURFACE)
}

#[test]
fn spawn_nest_default_width_matches_surface_max_mrows() {
    let mesh = OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let region = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 500_000.0,
        level: 1,
    };

    let default = mesh
        .spawn_nest(std::slice::from_ref(&region), 5)
        .expect("default-surface-width nest");
    let explicit_surface = mesh
        .spawn_nest_with_max_mrows(
            std::slice::from_ref(&region),
            5,
            OlamDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
        )
        .expect("explicit-surface-width nest");

    let default_max_abs_mrow = default
        .w_faces
        .iter()
        .skip(2)
        .map(|face| face.mrow.unsigned_abs())
        .max()
        .unwrap_or_default();
    let explicit_surface_max_abs_mrow = explicit_surface
        .w_faces
        .iter()
        .skip(2)
        .map(|face| face.mrow.unsigned_abs())
        .max()
        .unwrap_or_default();

    assert_eq!(
        default_max_abs_mrow, explicit_surface_max_abs_mrow,
        "spawn_nest default width should match METHOD_C_MAX_MROWS_SURFACE"
    );
}

#[test]
fn spawn_nest_as_atmosmesh_uses_atmos_transition_width() {
    let mesh = OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let region = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 500_000.0,
        level: 1,
    };

    let surface = mesh
        .spawn_nest(std::slice::from_ref(&region), 5)
        .expect("default-surface-width nest");
    let atmos = mesh
        .spawn_nest_as_atmosmesh(std::slice::from_ref(&region), 5)
        .expect("atmos-width nest");

    let surface_max_abs_mrow = surface
        .w_faces
        .iter()
        .skip(2)
        .map(|face| face.mrow.unsigned_abs())
        .max()
        .unwrap_or_default();
    let atmos_max_abs_mrow = atmos
        .w_faces
        .iter()
        .skip(2)
        .map(|face| face.mrow.unsigned_abs())
        .max()
        .unwrap_or_default();

    assert!(
        atmos_max_abs_mrow >= surface_max_abs_mrow,
        "atmos wrapper should not reduce transition width: surface={surface_max_abs_mrow}, atmos={atmos_max_abs_mrow}"
    );
    let atmos_nwd = atmos.nwd;
    let surface_nwd = surface.nwd;
    assert!(
        atmos_nwd >= surface_nwd,
        "atmos wrapper should refine no fewer faces than surface: atmosphere={atmos_nwd}, surface={surface_nwd}"
    );
}

#[test]
fn spawn_nest_with_spring_as_atmosmesh_uses_atmos_transition_width_and_runs_spring() {
    let mesh = OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let region = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 500_000.0,
        level: 1,
    };

    let (atmos, spring_passes) = mesh
        .spawn_nest_with_spring_as_atmosmesh(std::slice::from_ref(&region), 5, 16, 2)
        .expect("atmos spring-width nest");
    let atmos_baseline = mesh
        .spawn_nest_as_atmosmesh(std::slice::from_ref(&region), 5)
        .expect("atmos baseline nest");

    let atmos_spring_max_abs_mrow = atmos
        .w_faces
        .iter()
        .skip(2)
        .map(|face| face.mrow.unsigned_abs())
        .max()
        .unwrap_or_default();
    let atmos_baseline_max_abs_mrow = atmos_baseline
        .w_faces
        .iter()
        .skip(2)
        .map(|face| face.mrow.unsigned_abs())
        .max()
        .unwrap_or_default();

    assert_eq!(
        spring_passes, 1,
        "spawn_nest_with_spring_as_atmosmesh should run one spring pass when niter_refine > 0"
    );
    assert_eq!(
        atmos_spring_max_abs_mrow, atmos_baseline_max_abs_mrow,
        "spring pass should not alter the atmosphere transition width"
    );
}

#[test]
fn spawn_nest_tracks_fortran_m_point_grid_metadata() {
    let mesh = OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let region = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 500_000.0,
        level: 1,
    };

    let refined = mesh.spawn_nest(&[region], 5).expect("local circle nest");
    let metadata = refined.m_point_metadata();

    assert_eq!(
        metadata.len(),
        refined.nmd + 1,
        "M metadata should keep OLAM's one-based table layout"
    );
    assert!(
        metadata
            .iter()
            .skip(2)
            .any(|meta| meta.mrlm >= 2 && meta.mrlm_orig >= 2 && meta.ngr == 2),
        "spawn_nest should mark refined M points with Fortran itab_md mrlm/mrlm_orig/ngr"
    );
    assert!(
        metadata
            .iter()
            .skip(2)
            .any(|meta| meta.mrlm == 1 && meta.ngr == 1),
        "spawn_nest should preserve parent-grid M metadata outside the nest"
    );
    let max_mrlm_orig = metadata
        .iter()
        .skip(2)
        .map(|meta| meta.mrlm_orig)
        .max()
        .expect("M metadata");
    assert_eq!(
        max_mrlm_orig, 2,
        "Fortran perim_fill3 sets transition M mrlm_orig to parent mrlo + 1, not max_level or grid number"
    );
    assert!(
        refined
            .w_faces
            .iter()
            .skip(2)
            .any(|face| face.ngr == 2 && face.mrlw == 2),
        "spawn_nest should include fully subdivided child W faces at parent mrlo + 1"
    );
    assert!(
        refined
            .w_faces
            .iter()
            .skip(2)
            .any(|face| face.ngr == 2 && face.mrlw == 1),
        "Fortran perim_fill3 marks transition W faces with the child grid number while preserving parent mrlw"
    );
}

#[test]
fn spawn_nest_marks_perimeter_m_points_with_current_grid_number() {
    let mesh = OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let region = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 500_000.0,
        level: 1,
    };

    let refined = mesh.spawn_nest(&[region], 5).expect("local circle nest");
    let current_grid = 2;
    let boundary_m_points = (2..=refined.nmd)
        .filter(|&im| {
            let neighbors = refined.m_neighbors[im];
            neighbors
                .iw
                .iter()
                .take(neighbors.npoly)
                .any(|&iw| refined.w_faces[iw].ngr == current_grid)
        })
        .collect::<Vec<_>>();
    let mismatched = boundary_m_points
        .iter()
        .copied()
        .filter(|&im| refined.m_point_metadata()[im].ngr != current_grid)
        .collect::<Vec<_>>();

    assert!(
        !boundary_m_points.is_empty(),
        "test case should include M points adjacent to the current nested grid"
    );
    assert!(
        mismatched.is_empty(),
        "Fortran perim_mrow marks every M point adjacent to W faces on current ngr; mismatched M ids: {mismatched:?}"
    );
}

#[test]
fn olam_nest_spring_moves_transition_points_without_changing_topology() {
    let mesh = OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let region = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let mut refined = mesh.spawn_nest(&[region], 1).expect("local circle nest");
    let boundary_face_id = *refined
        .boundary_rows()
        .first()
        .expect("transition row face should be recorded");
    let transition_point_id = refined.w_faces[boundary_face_id].im[0];
    let transition_point = refined.m_points[transition_point_id];
    let perturbed = CartesianPoint::new(
        transition_point.x + 50_000.0,
        transition_point.y - 20_000.0,
        transition_point.z,
    );
    let scale = OLAM_FORTRAN_EARTH_RADIUS_METERS / magnitude(perturbed);
    refined.m_points[transition_point_id] = CartesianPoint::new(
        perturbed.x * scale,
        perturbed.y * scale,
        perturbed.z * scale,
    );
    let original_points = refined.m_points.clone();

    let smoothed = refined
        .spring_nest(6, 2, 2, false)
        .expect("OLAM nest spring adjustment");

    assert_eq!(smoothed.nmd, refined.nmd);
    assert_eq!(smoothed.nud, refined.nud);
    assert_eq!(smoothed.nwd, refined.nwd);
    smoothed.validate_topology().expect("topology stays closed");
    for point_id in 2..=smoothed.nmd {
        let radius = magnitude(smoothed.m_points[point_id]);
        assert!(
            (radius - OLAM_FORTRAN_EARTH_RADIUS_METERS).abs() <= 1.0,
            "point {point_id} radius {radius}"
        );
    }
    assert!(
        magnitude(CartesianPoint::new(
            smoothed.m_points[transition_point_id].x - original_points[transition_point_id].x,
            smoothed.m_points[transition_point_id].y - original_points[transition_point_id].y,
            smoothed.m_points[transition_point_id].z - original_points[transition_point_id].z,
        )) > 1.0e-3,
        "OLAM nest spring should move perturbed transition M point"
    );
}

#[test]
fn spawn_nest_rejects_invalid_refinement_levels() {
    let mesh = OlamDelaunayMesh::from_icosahedron(3, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let region = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(0.0, 0.0),
        radius_meters: 1_000_000.0,
        level: 6,
    };

    let err = mesh
        .spawn_nest(&[region], 5)
        .expect_err("levels above 5 are unsupported");
    assert!(err.to_string().contains("1..=5"), "unexpected error: {err}");
}
