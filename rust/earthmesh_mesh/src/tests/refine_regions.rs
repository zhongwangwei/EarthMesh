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
fn method_c_bbox_uses_directed_west_to_east_span() {
    let radius = earthmesh_core::EARTH_RADIUS_METERS;
    let wide = RefinementRegion::Bbox {
        west_degrees: -170.0,
        east_degrees: 170.0,
        south_degrees: -10.0,
        north_degrees: 10.0,
        level: 1,
    };
    let crossing = RefinementRegion::Bbox {
        west_degrees: 170.0,
        east_degrees: -170.0,
        south_degrees: -10.0,
        north_degrees: 10.0,
        level: 1,
    };
    let prime = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 0.0));
    let dateline = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(180.0, 0.0));

    assert!(wide.contains_cartesian(prime, radius));
    assert!(!wide.contains_cartesian(dateline, radius));
    assert!(!crossing.contains_cartesian(prime, radius));
    assert!(crossing.contains_cartesian(dateline, radius));

    let global = RefinementRegion::Bbox {
        west_degrees: -180.0,
        east_degrees: 180.0,
        south_degrees: -10.0,
        north_degrees: 10.0,
        level: 1,
    };
    assert!(global.contains_cartesian(prime, radius));
    assert!(global.contains_cartesian(dateline, radius));

    let just_south_of_wide_boundary = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, -11.0));
    assert!(wide.close_to_cartesian(just_south_of_wide_boundary, radius));
    assert!(!crossing.close_to_cartesian(prime, radius));
}

#[test]
fn method_c_circle_region_uses_canonical_polar_stereographic_distance() {
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(0.0, 0.0),
        radius_meters: 5_000_000.0,
        level: 1,
    };
    let point = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(rad_to_deg(0.75), 0.0));

    assert!(
            !region.contains_cartesian(point, earthmesh_core::EARTH_RADIUS_METERS),
            "Canonical ngr_area uses ec_ps distance, which rejects this point even though great-circle distance accepts it"
        );
}

#[test]
fn canonical_lonlat_entrypoint_makes_radius_metric_explicit() {
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(0.0, 0.0),
        radius_meters: 5_000_000.0,
        level: 1,
    };
    let point = LonLatDegrees::new(rad_to_deg(0.75), 0.0);

    assert_eq!(
        region.contains_lonlat_canonical(point),
        region.contains_cartesian(
            lonlat_degrees_to_unit_xyz(point),
            earthmesh_core::EARTH_RADIUS_METERS
        )
    );
    assert!(!region.contains_lonlat_canonical(point));
}

#[test]
fn geographic_polygon_is_spherical_at_the_pole_and_antimeridian() {
    let polygon = RefinementRegion::Polygon {
        points: vec![
            LonLatDegrees::new(170.0, 80.0),
            LonLatDegrees::new(-170.0, 80.0),
            LonLatDegrees::new(0.0, 89.0),
        ],
        level: 1,
    };

    polygon.validate().expect("valid spherical polar polygon");
    assert!(polygon.contains_lonlat_canonical(LonLatDegrees::new(180.0, 85.0)));
    assert!(!polygon.contains_lonlat_canonical(LonLatDegrees::new(0.0, 0.0)));
    assert_eq!(polygon.canonical_geometry_warning(), None);
}

#[test]
fn geographic_polygon_accepts_an_explicit_physical_closure() {
    let open = RefinementRegion::Polygon {
        points: vec![
            LonLatDegrees::new(0.0, 0.0),
            LonLatDegrees::new(4.0, 0.0),
            LonLatDegrees::new(0.0, 4.0),
        ],
        level: 1,
    };
    let closed = RefinementRegion::Polygon {
        points: vec![
            LonLatDegrees::new(0.0, 0.0),
            LonLatDegrees::new(4.0, 0.0),
            LonLatDegrees::new(0.0, 4.0),
            LonLatDegrees::new(360.0, 0.0),
        ],
        level: 1,
    };
    let inside = LonLatDegrees::new(1.0, 1.0);

    closed.validate().expect("explicit closure is valid");
    assert_eq!(
        closed.contains_lonlat_canonical(inside),
        open.contains_lonlat_canonical(inside)
    );
}

#[test]
fn geographic_polygon_rejects_a_self_intersection() {
    let bow_tie = RefinementRegion::Polygon {
        points: vec![
            LonLatDegrees::new(0.0, 0.0),
            LonLatDegrees::new(2.0, 2.0),
            LonLatDegrees::new(0.0, 2.0),
            LonLatDegrees::new(2.0, 0.0),
        ],
        level: 1,
    };

    assert!(bow_tie.validate().is_err());
}

#[test]
fn method_c_circle_region_resolves_sub_f32_boundary_offsets() {
    let radius = earthmesh_core::EARTH_RADIUS_METERS;
    let center = LonLatDegrees::new(0.0, 0.0);
    let inner = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(1.0, 0.0));
    let outer = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(1.000_000_01, 0.0));
    let inner_distance = method_c_ec_ps_distance_meters(inner, center, radius);
    let outer_distance = method_c_ec_ps_distance_meters(outer, center, radius);
    let region = RefinementRegion::Circle {
        center,
        radius_meters: 0.5 * (inner_distance + outer_distance),
        level: 1,
    };

    assert!(outer_distance > inner_distance);
    assert!(region.contains_cartesian(inner, radius));
    assert!(!region.contains_cartesian(outer, radius));
}

#[test]
fn method_c_stereographic_regions_reject_antipodal_singularity() {
    let radius = earthmesh_core::EARTH_RADIUS_METERS;
    let antipode = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(180.0, 0.0));
    let near_antipode = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(179.999_999, 0.0));
    let center = LonLatDegrees::new(0.0, 0.0);
    let circle = RefinementRegion::Circle {
        center,
        radius_meters: 2_000_000.0,
        level: 1,
    };
    let corridor = RefinementRegion::Corridor {
        points: vec![LonLatDegrees::new(-1.0, 0.0), LonLatDegrees::new(1.0, 0.0)],
        radius_meters: vec![2_000_000.0, 2_000_000.0],
        level: 1,
    };

    assert!(method_c_ec_ps_distance_meters(antipode, center, radius).is_infinite());
    assert!(method_c_ec_ps_distance_meters(near_antipode, center, radius).is_infinite());
    assert!(!circle.contains_cartesian(antipode, radius));
    assert!(!circle.contains_cartesian(near_antipode, radius));
    assert!(!corridor.contains_cartesian(antipode, radius));
    assert!(!corridor.contains_cartesian(near_antipode, radius));
}

#[test]
fn method_c_region_boundaries_use_canonical_strict_less_than_radius() {
    let radius = earthmesh_core::EARTH_RADIUS_METERS;
    let center = LonLatDegrees::new(0.0, 0.0);
    let circle_boundary = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(1.0, 0.0));
    let circle_distance = method_c_ec_ps_distance_meters(circle_boundary, center, radius);
    let circle = RefinementRegion::Circle {
        center,
        radius_meters: circle_distance,
        level: 1,
    };
    let circle_close = RefinementRegion::Circle {
        center,
        radius_meters: circle_distance / 1.5,
        level: 1,
    };

    assert!(!circle.contains_cartesian(circle_boundary, radius));
    assert!(!circle_close.close_to_cartesian(circle_boundary, radius));

    let corridor_points = vec![LonLatDegrees::new(-1.0, 0.0), LonLatDegrees::new(1.0, 0.0)];
    let corridor_boundary = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 1.0));
    let corridor_distance = method_c_corridor_segment_distance_meters(
        corridor_boundary,
        corridor_points[0],
        corridor_points[1],
        radius,
    )
    .0;
    let corridor = RefinementRegion::Corridor {
        points: corridor_points.clone(),
        radius_meters: vec![corridor_distance],
        level: 1,
    };
    let zero_radius_corridor = RefinementRegion::Corridor {
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
fn method_c_refinement_region_rejects_radius_below_canonical_dzxmin() {
    let circle = RefinementRegion::Circle {
        center: LonLatDegrees::new(0.0, 0.0),
        radius_meters: 0.0005,
        level: 1,
    };
    assert!(
        circle.validate().is_err(),
        "Canonical Method-C rejects grdrad below dzxmin=0.001"
    );

    let corridor = RefinementRegion::Corridor {
        points: vec![LonLatDegrees::new(0.0, 0.0), LonLatDegrees::new(1.0, 0.0)],
        radius_meters: vec![0.001, 0.0005],
        level: 1,
    };
    assert!(
        corridor.validate().is_err(),
        "Canonical Method-C rejects any corridor grdrad below dzxmin=0.001"
    );
}

#[test]
fn method_c_corridor_region_uses_canonical_segment_polar_stereographic_distance() {
    let region = RefinementRegion::Corridor {
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
        "Canonical ngr_area projects each segment to local PS space before linesegdist2"
    );
}

#[test]
fn method_c_corridor_region_interpolates_segment_radius_like_canonical() {
    let radius = earthmesh_core::EARTH_RADIUS_METERS;
    let points = vec![LonLatDegrees::new(-1.0, 0.0), LonLatDegrees::new(1.0, 0.0)];
    let point = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 1.0));
    let (distance, t) =
        method_c_corridor_segment_distance_meters(point, points[0], points[1], radius);
    let region = RefinementRegion::Corridor {
        points,
        radius_meters: vec![distance * 0.5, distance * 3.0],
        level: 1,
    };

    assert!(distance > distance * 0.5);
    assert!(
        distance < method_c_corridor_radius_at_segment(&[distance * 0.5, distance * 3.0], 0, t)
    );
    assert!(
        region.contains_cartesian(point, radius),
        "Canonical ngr_area interpolates grdrad between segment endpoints using t"
    );
}

#[test]
fn method_c_corridor_region_requires_radius_per_canonical_endpoint() {
    let region = RefinementRegion::Corridor {
        points: vec![LonLatDegrees::new(-1.0, 0.0), LonLatDegrees::new(1.0, 0.0)],
        radius_meters: vec![1_000_000.0],
        level: 1,
    };

    assert!(
            region.validate().is_err(),
            "Canonical ngr_area interpolates grdrad(ipt) and grdrad(jpt), so each corridor endpoint must provide a radius"
        );
}

#[test]
fn method_c_native_cartesian_circle_uses_canonical_mdomain_ge_two_distance() {
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(10.0, 20.0),
        radius_meters: 5.0,
        level: 1,
    };

    assert!(region.contains_cartesian_xy(CartesianPoint::new(12.0, 23.0, 999.0)));
    assert!(!region.contains_cartesian_xy(CartesianPoint::new(13.0, 24.0, 999.0)));
    assert!(region.close_to_cartesian_xy(CartesianPoint::new(10.0, 27.0, 999.0)));
}

#[test]
fn method_c_native_cartesian_region_validation_allows_canonical_mdomain_ge_two_coordinates() {
    let circle = RefinementRegion::Circle {
        center: LonLatDegrees::new(250.0, 200.0),
        radius_meters: 5.0,
        level: 1,
    };
    let corridor = RefinementRegion::Corridor {
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
        .expect("Canonical mdomain >= 2 accepts Cartesian native circle coordinates");
    corridor
        .validate_cartesian_xy()
        .expect("Canonical mdomain >= 2 accepts Cartesian native corridor coordinates");
}

#[test]
fn method_c_cartesian_bbox_and_polygon_use_native_xy_geometry() {
    let bbox = RefinementRegion::Bbox {
        west_degrees: 10.0,
        east_degrees: 20.0,
        south_degrees: 30.0,
        north_degrees: 40.0,
        level: 1,
    };
    let polygon = RefinementRegion::Polygon {
        points: vec![
            LonLatDegrees::new(10.0, 30.0),
            LonLatDegrees::new(20.0, 30.0),
            LonLatDegrees::new(20.0, 40.0),
        ],
        level: 1,
    };
    assert!(bbox.validate().is_ok(), "bbox stays valid in lon/lat mode");
    bbox.validate_cartesian_xy()
        .expect("Cartesian bbox coordinates are finite and ordered");
    polygon
        .validate_cartesian_xy()
        .expect("Cartesian polygon coordinates are finite");

    let inside = CartesianPoint::new(15.0, 35.0, 0.0);
    let outside = CartesianPoint::new(25.0, 35.0, 0.0);
    assert!(bbox.contains_cartesian_xy(inside));
    assert!(!bbox.contains_cartesian_xy(outside));
    assert_eq!(bbox.cartesian_xy_outside_distance_meters(inside), 0.0);
    assert_eq!(bbox.cartesian_xy_outside_distance_meters(outside), 5.0);

    assert!(polygon.contains_cartesian_xy(CartesianPoint::new(18.0, 32.0, 0.0)));
    assert!(!polygon.contains_cartesian_xy(outside));
    assert_eq!(
        polygon.cartesian_xy_outside_distance_meters(CartesianPoint::new(15.0, 25.0, 0.0)),
        5.0
    );
}

#[test]
fn method_c_native_cartesian_corridor_uses_canonical_linesegdist2_radius_interpolation() {
    let region = RefinementRegion::Corridor {
        points: vec![LonLatDegrees::new(0.0, 0.0), LonLatDegrees::new(10.0, 0.0)],
        radius_meters: vec![2.0, 6.0],
        level: 1,
    };

    assert!(region.contains_cartesian_xy(CartesianPoint::new(5.0, 3.0, 999.0)));
    assert!(!region.contains_cartesian_xy(CartesianPoint::new(5.0, 4.0, 999.0)));
    assert!(region.close_to_cartesian_xy(CartesianPoint::new(5.0, 4.7, 999.0)));
}

#[test]
fn method_c_polygon_uses_minor_great_circle_edges() {
    let region = RefinementRegion::Polygon {
        points: vec![
            LonLatDegrees::new(-80.0, 40.0),
            LonLatDegrees::new(80.0, 40.0),
            LonLatDegrees::new(80.0, -40.0),
            LonLatDegrees::new(-80.0, -40.0),
        ],
        level: 1,
    };
    let point = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 45.0));

    assert!(region.contains_cartesian(point, earthmesh_core::EARTH_RADIUS_METERS));
    assert!(region.close_to_cartesian(point, earthmesh_core::EARTH_RADIUS_METERS));
}

#[test]
fn method_c_bbox_halo_follows_the_segmented_directed_boundary() {
    let region = RefinementRegion::Bbox {
        west_degrees: -80.0,
        east_degrees: 80.0,
        south_degrees: -40.0,
        north_degrees: 40.0,
        level: 1,
    };
    let near = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 45.0));
    let far = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 70.0));

    assert!(
        region.close_to_cartesian(near, earthmesh_core::EARTH_RADIUS_METERS),
        "a point five degrees beyond the north edge must remain in the bbox halo"
    );
    assert!(
        !region.close_to_cartesian(far, earthmesh_core::EARTH_RADIUS_METERS),
        "a point farther than the configured bbox halo must remain outside"
    );
}

#[test]
fn method_c_bbox_and_polygon_regions_fill_lonlat_interiors() {
    let radius = earthmesh_core::EARTH_RADIUS_METERS;
    let point = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 0.0));
    let polygon = RefinementRegion::Polygon {
        points: vec![
            LonLatDegrees::new(-40.0, -40.0),
            LonLatDegrees::new(40.0, -40.0),
            LonLatDegrees::new(40.0, 40.0),
            LonLatDegrees::new(-40.0, 40.0),
        ],
        level: 1,
    };
    let bbox = RefinementRegion::Bbox {
        west_degrees: -40.0,
        east_degrees: 40.0,
        south_degrees: -40.0,
        north_degrees: 40.0,
        level: 1,
    };

    assert!(
        polygon.contains_cartesian(point, radius),
        "bbox/polygon refinement masks fill interiors instead of leaving wide-area holes"
    );
    assert!(
        bbox.contains_cartesian(point, radius),
        "bbox refinement masks fill interiors instead of selecting only a fixed-width corridor"
    );
}

#[test]
fn method_c_bbox_and_polygon_close_paths_include_filled_interiors() {
    let radius = earthmesh_core::EARTH_RADIUS_METERS;
    let point = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 0.0));
    let bbox = RefinementRegion::Bbox {
        west_degrees: -80.0,
        east_degrees: 80.0,
        south_degrees: -40.0,
        north_degrees: 40.0,
        level: 1,
    };
    let polygon = RefinementRegion::Polygon {
        points: vec![
            LonLatDegrees::new(-80.0, -40.0),
            LonLatDegrees::new(80.0, -40.0),
            LonLatDegrees::new(80.0, 40.0),
            LonLatDegrees::new(-80.0, 40.0),
        ],
        level: 1,
    };

    assert!(bbox.close_to_cartesian(point, radius));
    assert!(polygon.close_to_cartesian(point, radius));
}

#[test]
fn method_c_polygon_region_closes_last_point_to_first_for_interior_fill() {
    let radius = earthmesh_core::EARTH_RADIUS_METERS;
    let polygon = RefinementRegion::Polygon {
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
        polygon.contains_cartesian(point_on_implicit_closing_segment, radius),
        "polygon containment closes the final edge when filling interiors"
    );
}

#[test]
fn method_c_multipoint_region_anchor_uses_first_specified_point_like_canonical() {
    let first = LonLatDegrees::new(-40.0, -30.0);
    let corridor = RefinementRegion::Corridor {
        points: vec![first, LonLatDegrees::new(20.0, 30.0)],
        radius_meters: vec![500_000.0, 500_000.0],
        level: 1,
    };
    let polygon = RefinementRegion::Polygon {
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
        "Canonical chooses imcent from grdlat/grdlon index 1 for multi-point NGR regions"
    );
    assert_eq!(
        polygon.anchor_lonlat(),
        first,
        "Canonical chooses imcent from grdlat/grdlon index 1 for multi-point NGR regions"
    );
}

#[test]
fn method_c_bbox_region_anchor_uses_first_closed_corridor_corner_like_canonical() {
    let bbox = RefinementRegion::Bbox {
        west_degrees: -40.0,
        east_degrees: 40.0,
        south_degrees: -30.0,
        north_degrees: 30.0,
        level: 1,
    };

    assert_eq!(
            bbox.anchor_lonlat(),
            LonLatDegrees::new(-40.0, -30.0),
            "bbox regions are reduced to closed Canonical corridor segments, so anchor must be the first generated corner"
        );
}
