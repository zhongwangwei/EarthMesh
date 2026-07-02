use super::super::*;

#[test]
fn vector_conversion_preserves_order() {
    let points = [
        CartesianPoint::new(1.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 1.0, 0.0),
    ];
    let lonlat = xyz_points_to_lonlat_degrees(&points);
    assert_eq!(lonlat.len(), 2);
    assert_eq!(lonlat[0].lon_degrees, 0.0);
    assert_eq!(lonlat[1].lon_degrees, 90.0);
}

#[test]
fn olam_circle_region_uses_fortran_polar_stereographic_distance() {
    let region = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(0.0, 0.0),
        radius_meters: 5_000_000.0,
        level: 1,
    };
    let point = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(rad_to_deg(0.75), 0.0));

    assert!(
            !region.contains_cartesian(point, earthmesh_core::EARTH_RADIUS_METERS),
            "Fortran ngr_area uses ec_ps distance, which rejects this point even though great-circle distance accepts it"
        );
}

#[test]
fn olam_region_boundaries_use_fortran_strict_less_than_radius() {
    let radius = earthmesh_core::EARTH_RADIUS_METERS;
    let center = LonLatDegrees::new(0.0, 0.0);
    let circle_boundary = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(1.0, 0.0));
    let circle_distance = olam_ec_ps_distance_meters(circle_boundary, center, radius);
    let circle = OlamRefinementRegion::Circle {
        center,
        radius_meters: circle_distance,
        level: 1,
    };
    let circle_close = OlamRefinementRegion::Circle {
        center,
        radius_meters: circle_distance / 1.5,
        level: 1,
    };

    assert!(!circle.contains_cartesian(circle_boundary, radius));
    assert!(!circle_close.close_to_cartesian(circle_boundary, radius));

    let corridor_points = vec![LonLatDegrees::new(-1.0, 0.0), LonLatDegrees::new(1.0, 0.0)];
    let corridor_boundary = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 1.0));
    let corridor_distance = olam_corridor_segment_distance_meters(
        corridor_boundary,
        corridor_points[0],
        corridor_points[1],
        radius,
    )
    .0;
    let corridor = OlamRefinementRegion::Corridor {
        points: corridor_points.clone(),
        radius_meters: vec![corridor_distance],
        level: 1,
    };
    let zero_radius_corridor = OlamRefinementRegion::Corridor {
        points: corridor_points.clone(),
        radius_meters: vec![0.0],
        level: 1,
    };
    let on_corridor_line = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 0.0));

    assert!(!corridor.contains_cartesian(corridor_boundary, radius));
    assert!(!zero_radius_corridor.contains_cartesian(on_corridor_line, radius));
    assert!(!zero_radius_corridor.close_to_cartesian(on_corridor_line, radius));
}

#[test]
fn olam_refinement_region_rejects_radius_below_fortran_dzxmin() {
    let circle = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(0.0, 0.0),
        radius_meters: 0.0005,
        level: 1,
    };
    assert!(
        circle.validate().is_err(),
        "Fortran Method-C rejects grdrad below dzxmin=0.001"
    );

    let corridor = OlamRefinementRegion::Corridor {
        points: vec![LonLatDegrees::new(0.0, 0.0), LonLatDegrees::new(1.0, 0.0)],
        radius_meters: vec![0.001, 0.0005],
        level: 1,
    };
    assert!(
        corridor.validate().is_err(),
        "Fortran Method-C rejects any corridor grdrad below dzxmin=0.001"
    );
}

#[test]
fn olam_corridor_region_uses_fortran_segment_polar_stereographic_distance() {
    let region = OlamRefinementRegion::Corridor {
        points: vec![
            LonLatDegrees::new(-80.0, 40.0),
            LonLatDegrees::new(80.0, 40.0),
        ],
        radius_meters: vec![1_000_000.0, 1_000_000.0],
        level: 1,
    };
    let point = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 45.0));

    assert!(
        !region.contains_cartesian(point, earthmesh_core::EARTH_RADIUS_METERS),
        "Fortran ngr_area projects each segment to local PS space before linesegdist2"
    );
}

#[test]
fn olam_corridor_region_interpolates_segment_radius_like_fortran() {
    let radius = earthmesh_core::EARTH_RADIUS_METERS;
    let points = vec![LonLatDegrees::new(-1.0, 0.0), LonLatDegrees::new(1.0, 0.0)];
    let point = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 1.0));
    let (distance, t) = olam_corridor_segment_distance_meters(point, points[0], points[1], radius);
    let region = OlamRefinementRegion::Corridor {
        points,
        radius_meters: vec![distance * 0.5, distance * 3.0],
        level: 1,
    };

    assert!(distance > distance * 0.5);
    assert!(distance < olam_corridor_radius_at_segment(&[distance * 0.5, distance * 3.0], 0, t));
    assert!(
        region.contains_cartesian(point, radius),
        "Fortran ngr_area interpolates grdrad between segment endpoints using t"
    );
}

#[test]
fn olam_corridor_region_requires_radius_per_fortran_endpoint() {
    let region = OlamRefinementRegion::Corridor {
        points: vec![LonLatDegrees::new(-1.0, 0.0), LonLatDegrees::new(1.0, 0.0)],
        radius_meters: vec![1_000_000.0],
        level: 1,
    };

    assert!(
            region.validate().is_err(),
            "Fortran ngr_area interpolates grdrad(ipt) and grdrad(jpt), so each corridor endpoint must provide a radius"
        );
}

#[test]
fn olam_native_cartesian_circle_uses_fortran_mdomain_ge_two_distance() {
    let region = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(10.0, 20.0),
        radius_meters: 5.0,
        level: 1,
    };

    assert!(region.contains_cartesian_xy(CartesianPoint::new(12.0, 23.0, 999.0)));
    assert!(!region.contains_cartesian_xy(CartesianPoint::new(13.0, 24.0, 999.0)));
    assert!(region.close_to_cartesian_xy(CartesianPoint::new(10.0, 27.0, 999.0)));
}

#[test]
fn olam_native_cartesian_region_validation_allows_fortran_mdomain_ge_two_coordinates() {
    let circle = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(250.0, 200.0),
        radius_meters: 5.0,
        level: 1,
    };
    let corridor = OlamRefinementRegion::Corridor {
        points: vec![
            LonLatDegrees::new(250.0, 200.0),
            LonLatDegrees::new(260.0, 210.0),
        ],
        radius_meters: vec![2.0, 6.0],
        level: 1,
    };

    assert!(circle.validate().is_err());
    assert!(corridor.validate().is_err());
    circle
        .validate_cartesian_xy()
        .expect("Fortran mdomain >= 2 accepts Cartesian native circle coordinates");
    corridor
        .validate_cartesian_xy()
        .expect("Fortran mdomain >= 2 accepts Cartesian native corridor coordinates");
}

#[test]
fn olam_cartesian_region_validation_rejects_bbox_and_polygon() {
    // `contains_cartesian_xy` has no bbox/polygon implementation, so Cartesian
    // validation must reject these variants instead of letting a spawn pass
    // silently refine a wrong single-anchor patch.
    let bbox = OlamRefinementRegion::Bbox {
        west_degrees: 10.0,
        east_degrees: 20.0,
        south_degrees: 30.0,
        north_degrees: 40.0,
        level: 1,
    };
    let polygon = OlamRefinementRegion::Polygon {
        points: vec![
            LonLatDegrees::new(10.0, 30.0),
            LonLatDegrees::new(20.0, 30.0),
            LonLatDegrees::new(20.0, 40.0),
        ],
        level: 1,
    };
    assert!(bbox.validate().is_ok(), "bbox stays valid in lon/lat mode");
    assert!(bbox.validate_cartesian_xy().is_err());
    assert!(polygon.validate_cartesian_xy().is_err());
}

#[test]
fn olam_native_cartesian_start_uses_imcent_not_global_pentagon_like_fortran() {
    let mesh = OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let method_c_m_neighbors = mesh.method_c_m_neighbors().expect("Method-C M neighbors");
    let pentagon = mesh.impent[0];
    let non_pentagon = (2..=mesh.nmd)
        .find(|im| !mesh.impent.contains(im))
        .expect("non-pentagon M point");
    let pentagon_xy = mesh.m_points[pentagon];
    let anchor_xy = mesh.m_points[non_pentagon];
    let radius_meters = (anchor_xy.x - pentagon_xy.x).hypot(anchor_xy.y - pentagon_xy.y) * 1.01;
    let region = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(anchor_xy.x, anchor_xy.y),
        radius_meters,
        level: 1,
    };

    assert!(region.contains_cartesian_xy(pentagon_xy));
    let start = mesh
        .olam_refinement_start_point_with_neighbors(
            &region,
            active_mesh_radius(&mesh).expect("mesh radius"),
            &method_c_m_neighbors,
            true,
        )
        .expect("cartesian Method-C start");

    assert_eq!(
        start, non_pentagon,
        "Fortran mdomain >= 2 skips impent logic and starts from imcent"
    );
}

#[test]
fn olam_selected_faces_do_not_pre_expand_for_future_levels_like_fortran() {
    let mesh = OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
    let region_level_one = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(105.0, 35.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let region_level_two = OlamRefinementRegion::Circle {
        center: LonLatDegrees::new(105.0, 35.0),
        radius_meters: 2_500_000.0,
        level: 2,
    };

    let selected_level_one = mesh
        .selected_region_faces(&region_level_one, 1, false)
        .expect("level-one selected faces");
    let selected_level_two = mesh
        .selected_region_faces(&region_level_two, 1, false)
        .expect("level-two pass-one selected faces");

    assert_eq!(
            selected_level_one, selected_level_two,
            "Fortran spawn_nest selects each NN independently and does not pre-expand pass 1 for future nested grids"
        );
}

#[test]
fn olam_native_cartesian_corridor_uses_fortran_linesegdist2_radius_interpolation() {
    let region = OlamRefinementRegion::Corridor {
        points: vec![LonLatDegrees::new(0.0, 0.0), LonLatDegrees::new(10.0, 0.0)],
        radius_meters: vec![2.0, 6.0],
        level: 1,
    };

    assert!(region.contains_cartesian_xy(CartesianPoint::new(5.0, 3.0, 999.0)));
    assert!(!region.contains_cartesian_xy(CartesianPoint::new(5.0, 4.0, 999.0)));
    assert!(region.close_to_cartesian_xy(CartesianPoint::new(5.0, 4.7, 999.0)));
}

#[test]
fn olam_polygon_near_edge_uses_fortran_segment_polar_stereographic_distance() {
    let region = OlamRefinementRegion::Polygon {
        points: vec![
            LonLatDegrees::new(-80.0, 40.0),
            LonLatDegrees::new(80.0, 40.0),
            LonLatDegrees::new(80.0, -40.0),
            LonLatDegrees::new(-80.0, -40.0),
        ],
        level: 1,
    };
    let point = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 45.0));

    assert!(
        !region.close_to_cartesian(point, earthmesh_core::EARTH_RADIUS_METERS),
        "polygon near-edge halo should use the same Fortran PS segment distance as ngr_area"
    );
}

#[test]
fn olam_bbox_near_edge_uses_fortran_segment_polar_stereographic_distance() {
    let region = OlamRefinementRegion::Bbox {
        west_degrees: -80.0,
        east_degrees: 80.0,
        south_degrees: -40.0,
        north_degrees: 40.0,
        level: 1,
    };
    let point = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 45.0));

    assert!(
        !region.close_to_cartesian(point, earthmesh_core::EARTH_RADIUS_METERS),
        "bbox near-edge halo should use the same Fortran PS segment distance as ngr_area"
    );
}

#[test]
fn olam_bbox_and_polygon_regions_use_closed_corridor_not_lonlat_interior() {
    let radius = earthmesh_core::EARTH_RADIUS_METERS;
    let point = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 0.0));
    let polygon = OlamRefinementRegion::Polygon {
        points: vec![
            LonLatDegrees::new(-40.0, -40.0),
            LonLatDegrees::new(40.0, -40.0),
            LonLatDegrees::new(40.0, 40.0),
            LonLatDegrees::new(-40.0, 40.0),
        ],
        level: 1,
    };
    let bbox = OlamRefinementRegion::Bbox {
        west_degrees: -40.0,
        east_degrees: 40.0,
        south_degrees: -40.0,
        north_degrees: 40.0,
        level: 1,
    };

    assert!(
            !polygon.contains_cartesian(point, radius),
            "Fortran ngr_area has no point-in-polygon interior fill; closed masks are treated as corridor segments"
        );
    assert!(
            !bbox.contains_cartesian(point, radius),
            "Fortran ngr_area has no lon/lat bbox interior fill; bbox input is reduced to closed corridor segments"
        );
}

#[test]
fn olam_polygon_region_does_not_close_last_point_to_first_unless_explicit() {
    let radius = earthmesh_core::EARTH_RADIUS_METERS;
    let polygon = OlamRefinementRegion::Polygon {
        points: vec![
            LonLatDegrees::new(0.0, 0.0),
            LonLatDegrees::new(60.0, 0.0),
            LonLatDegrees::new(60.0, 60.0),
        ],
        level: 1,
    };
    let point_on_implicit_closing_segment =
        lonlat_degrees_to_unit_xyz(LonLatDegrees::new(30.0, 30.0));

    assert!(
            !polygon.contains_cartesian(point_on_implicit_closing_segment, radius),
            "Fortran ngr_area only checks connected input segments 1..ngrdll-1; it does not add an implicit last-to-first closing segment"
        );
}

#[test]
fn olam_multipoint_region_anchor_uses_first_specified_point_like_fortran() {
    let first = LonLatDegrees::new(-40.0, -30.0);
    let corridor = OlamRefinementRegion::Corridor {
        points: vec![first, LonLatDegrees::new(20.0, 30.0)],
        radius_meters: vec![500_000.0, 500_000.0],
        level: 1,
    };
    let polygon = OlamRefinementRegion::Polygon {
        points: vec![
            first,
            LonLatDegrees::new(40.0, -30.0),
            LonLatDegrees::new(40.0, 30.0),
            LonLatDegrees::new(-40.0, 30.0),
        ],
        level: 1,
    };

    assert_eq!(
        corridor.anchor_lonlat(),
        first,
        "Fortran chooses imcent from grdlat/grdlon index 1 for multi-point NGR regions"
    );
    assert_eq!(
        polygon.anchor_lonlat(),
        first,
        "Fortran chooses imcent from grdlat/grdlon index 1 for multi-point NGR regions"
    );
}

#[test]
fn olam_bbox_region_anchor_uses_first_closed_corridor_corner_like_fortran() {
    let bbox = OlamRefinementRegion::Bbox {
        west_degrees: -40.0,
        east_degrees: 40.0,
        south_degrees: -30.0,
        north_degrees: 30.0,
        level: 1,
    };

    assert_eq!(
            bbox.anchor_lonlat(),
            LonLatDegrees::new(-40.0, -30.0),
            "bbox regions are reduced to closed Fortran corridor segments, so anchor must be the first generated corner"
        );
}
