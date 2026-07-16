use earthmesh_geometry::{
    area_judge_first_self_intersection_one_based, cross_product_2d, haversine_km,
    is_point_in_circle_km, is_point_in_convex_polygon, AreaJudgeSelfIntersection, Point,
};

fn approx_eq(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual {actual} expected {expected} tolerance {tolerance}"
    );
}

#[test]
fn haversine_matches_mod_area_judge_zero_and_one_degree_equator() {
    approx_eq(
        haversine_km(Point::new(0.0, 0.0), Point::new(0.0, 0.0)),
        0.0,
        1.0e-12,
    );
    approx_eq(
        haversine_km(Point::new(0.0, 0.0), Point::new(1.0, 0.0)),
        111.1989234485458,
        1.0e-9,
    );
}

#[test]
fn haversine_stays_finite_for_near_antipodal_points() {
    let distance = haversine_km(
        Point::new(49.862495338956535, 19.36923549968097),
        Point::new(-130.13750466109158, -19.36923549968843),
    );

    assert!(
        distance.is_finite(),
        "near-antipodal distance was {distance}"
    );
    assert!(distance > 20_000.0);
}

#[test]
fn circle_test_uses_haversine_radius_in_kilometers() {
    let center = Point::new(113.5, 22.5);
    let inside = Point::new(113.55, 22.5);
    let outside = Point::new(114.5, 22.5);

    assert!(is_point_in_circle_km(inside, center, 10.0));
    assert!(!is_point_in_circle_km(outside, center, 10.0));
}

#[test]
fn cross_product_matches_mod_area_judge_orientation_formula() {
    let p1 = Point::new(0.0, 0.0);
    let p2 = Point::new(2.0, 0.0);

    approx_eq(cross_product_2d(p1, p2, Point::new(1.0, 1.0)), 2.0, 0.0);
    approx_eq(cross_product_2d(p1, p2, Point::new(1.0, -1.0)), -2.0, 0.0);
    approx_eq(cross_product_2d(p1, p2, Point::new(1.0, 0.0)), 0.0, 0.0);
}

#[test]
fn convex_polygon_test_accepts_inside_boundary_and_rejects_outside() {
    let square = [
        Point::new(0.0, 0.0),
        Point::new(2.0, 0.0),
        Point::new(2.0, 2.0),
        Point::new(0.0, 2.0),
    ];

    assert!(is_point_in_convex_polygon(&square, Point::new(1.0, 1.0)));
    assert!(is_point_in_convex_polygon(&square, Point::new(2.0, 1.0)));
    assert!(!is_point_in_convex_polygon(&square, Point::new(3.0, 1.0)));
}

#[test]
fn ray_segment_intersection_matches_canonical_sentinel_cases() {
    assert_eq!(
        earthmesh_geometry::ray_segment_intersection_lon(
            Point::new(-200.0, 1.0),
            0.0,
            10.0,
            2.0,
            20.0
        ),
        Some(15.0)
    );

    // Horizontal segments are treated as no intersection by MOD_Area_judge.
    assert_eq!(
        earthmesh_geometry::ray_segment_intersection_lon(
            Point::new(-200.0, 1.0),
            1.0,
            10.0,
            1.0,
            20.0
        ),
        None
    );

    // Ray latitude outside the segment latitude range is also no intersection.
    assert_eq!(
        earthmesh_geometry::ray_segment_intersection_lon(
            Point::new(-200.0, 3.0),
            0.0,
            10.0,
            2.0,
            20.0
        ),
        None
    );
}

#[test]
fn ray_segment_intersection_counts_shared_vertices_once() {
    let ray = Point::new(-200.0, 1.0);
    assert_eq!(
        earthmesh_geometry::ray_segment_intersection_lon(ray, 0.0, 0.0, 1.0, 1.0),
        None,
        "edge ending on the ray is excluded by the half-open rule"
    );
    assert_eq!(
        earthmesh_geometry::ray_segment_intersection_lon(ray, 1.0, 1.0, 2.0, 2.0),
        Some(1.0),
        "the adjacent edge starting on the ray owns the shared vertex"
    );
}

#[test]
fn strict_segment_intersection_matches_canonical_cross_product_rule() {
    assert!(earthmesh_geometry::segments_intersect_strict(
        Point::new(0.0, 0.0),
        Point::new(2.0, 2.0),
        Point::new(0.0, 2.0),
        Point::new(2.0, 0.0),
    ));

    // Endpoint touches are false because Canonical requires cp1*cp2 < 0 and cp3*cp4 < 0.
    assert!(!earthmesh_geometry::segments_intersect_strict(
        Point::new(0.0, 0.0),
        Point::new(1.0, 1.0),
        Point::new(1.0, 1.0),
        Point::new(2.0, 0.0),
    ));
}

#[test]
fn dateline_crossing_shift_matches_canonical_checkcrossing() {
    let shifted = earthmesh_geometry::shift_longitudes_for_dateline_crossing(&[
        Point::new(-170.0, 10.0),
        Point::new(175.0, 11.0),
        Point::new(0.0, 12.0),
    ]);

    assert_eq!(shifted[0], Point::new(10.0, 10.0));
    assert_eq!(shifted[1], Point::new(-5.0, 11.0));
    assert_eq!(shifted[2], Point::new(-180.0, 12.0));
}

#[test]
fn self_intersection_wrapper_reports_canonical_one_based_segments_and_points() {
    let bow_tie = [
        Point::new(0.0, 0.0),
        Point::new(2.0, 2.0),
        Point::new(0.0, 2.0),
        Point::new(2.0, 0.0),
    ];

    assert_eq!(
        area_judge_first_self_intersection_one_based(&bow_tie),
        Some(AreaJudgeSelfIntersection {
            first_segment_id: 1,
            second_segment_id: 3,
            first_segment: [Point::new(0.0, 0.0), Point::new(2.0, 2.0)],
            second_segment: [Point::new(0.0, 2.0), Point::new(2.0, 0.0)],
        })
    );
}

#[test]
fn self_intersection_wrapper_accepts_simple_polygons_and_endpoint_touches() {
    let square = [
        Point::new(0.0, 0.0),
        Point::new(2.0, 0.0),
        Point::new(2.0, 2.0),
        Point::new(0.0, 2.0),
    ];
    let triangle = [
        Point::new(0.0, 0.0),
        Point::new(1.0, 1.0),
        Point::new(2.0, 0.0),
    ];

    assert_eq!(area_judge_first_self_intersection_one_based(&square), None);
    assert_eq!(
        area_judge_first_self_intersection_one_based(&triangle),
        None
    );
}
