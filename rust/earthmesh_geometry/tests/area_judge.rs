use earthmesh_geometry::{
    cross_product_2d, haversine_km, is_point_in_circle_km, is_point_in_convex_polygon, Point,
};

fn approx_eq(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual {actual} expected {expected} tolerance {tolerance}"
    );
}

#[test]
fn haversine_matches_mod_area_judge_zero_and_one_degree_equator() {
    approx_eq(haversine_km(Point::new(0.0, 0.0), Point::new(0.0, 0.0)), 0.0, 1.0e-12);
    approx_eq(
        haversine_km(Point::new(0.0, 0.0), Point::new(1.0, 0.0)),
        111.1989234485458,
        1.0e-9,
    );
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
