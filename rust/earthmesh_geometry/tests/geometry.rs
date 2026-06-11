use earthmesh_geometry::{clip_convex_polygon, intersection_area, polygon_area, Point};

#[test]
fn polygon_area_matches_python_reference_fixtures() {
    let triangle = vec![
        Point::new(0.0, 0.0),
        Point::new(2.0, 0.0),
        Point::new(0.0, 2.0),
    ];
    let rectangle = vec![
        Point::new(0.0, 0.0),
        Point::new(2.0, 0.0),
        Point::new(2.0, 1.0),
        Point::new(0.0, 1.0),
    ];

    assert_eq!(polygon_area(&triangle), 2.0);
    assert_eq!(polygon_area(&rectangle), 2.0);
}

#[test]
fn clip_convex_polygon_returns_rectangle_overlap() {
    let subject = vec![
        Point::new(0.0, 0.0),
        Point::new(2.0, 0.0),
        Point::new(2.0, 2.0),
        Point::new(0.0, 2.0),
    ];
    let clip = vec![
        Point::new(1.0, 0.0),
        Point::new(3.0, 0.0),
        Point::new(3.0, 2.0),
        Point::new(1.0, 2.0),
    ];

    let intersection = clip_convex_polygon(&subject, &clip);

    assert_eq!(intersection.len(), 4);
    assert!((polygon_area(&intersection) - 2.0).abs() < 1.0e-9);
}

#[test]
fn intersection_area_is_zero_for_disjoint_polygons() {
    let a = vec![
        Point::new(0.0, 0.0),
        Point::new(1.0, 0.0),
        Point::new(1.0, 1.0),
        Point::new(0.0, 1.0),
    ];
    let b = vec![
        Point::new(2.0, 0.0),
        Point::new(3.0, 0.0),
        Point::new(3.0, 1.0),
        Point::new(2.0, 1.0),
    ];

    assert_eq!(intersection_area(&a, &b), 0.0);
}
