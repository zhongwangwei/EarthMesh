use earthmesh_geometry::{
    clip_convex_polygon, intersection_area, overlay_cell, overlay_cells, polygon_area,
    OverlayCellInput, OverlayMask, Point,
};

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

#[test]
fn intersection_area_handles_concave_mask_against_convex_cell() {
    let cell = vec![
        Point::new(0.5, 0.5),
        Point::new(2.5, 0.5),
        Point::new(2.5, 2.5),
        Point::new(0.5, 2.5),
    ];
    let concave_l_mask = vec![
        Point::new(0.0, 0.0),
        Point::new(3.0, 0.0),
        Point::new(3.0, 1.0),
        Point::new(1.0, 1.0),
        Point::new(1.0, 3.0),
        Point::new(0.0, 3.0),
    ];

    let area = intersection_area(&cell, &concave_l_mask);

    assert!(
        (area - 1.75).abs() < 1.0e-9,
        "convex cells clipped by concave hydro/coast masks should preserve the L-shaped overlap area, got {area}"
    );
}

#[test]
fn overlay_cell_returns_class_fractions_and_priority_from_rust() {
    let cell = vec![
        Point::new(0.5, 0.5),
        Point::new(2.5, 0.5),
        Point::new(2.5, 2.5),
        Point::new(0.5, 2.5),
    ];
    let masks = vec![
        OverlayMask {
            feature_id: "coast".to_string(),
            mask_class: "COAST_LAND".to_string(),
            priority: 10,
            polygon: cell.clone(),
        },
        OverlayMask {
            feature_id: "river".to_string(),
            mask_class: "R2".to_string(),
            priority: 30,
            polygon: vec![
                Point::new(0.0, 0.0),
                Point::new(3.0, 0.0),
                Point::new(3.0, 1.0),
                Point::new(1.0, 1.0),
                Point::new(1.0, 3.0),
                Point::new(0.0, 3.0),
            ],
        },
    ];

    let result = overlay_cell(&cell, &masks);

    assert_eq!(result.winning_class, "R2");
    assert_eq!(result.winning_priority, 30);
    assert_eq!(result.source_feature_ids, vec!["coast", "river"]);
    assert!(result.quality_flags.is_empty());
    assert_eq!(result.class_fractions[0], ("COAST_LAND".to_string(), 1.0));
    assert!((result.class_fractions[1].1 - 0.4375).abs() < 1.0e-9);
}

#[test]
fn overlay_cells_batches_cell_ids_and_missing_mask_flags_in_rust() {
    let cells = vec![
        OverlayCellInput {
            cell_id: "wet-cell".to_string(),
            vertices: vec![
                Point::new(0.5, 0.5),
                Point::new(2.5, 0.5),
                Point::new(2.5, 2.5),
                Point::new(0.5, 2.5),
            ],
        },
        OverlayCellInput {
            cell_id: "dry-cell".to_string(),
            vertices: vec![
                Point::new(5.0, 5.0),
                Point::new(6.0, 5.0),
                Point::new(6.0, 6.0),
                Point::new(5.0, 6.0),
            ],
        },
    ];
    let masks = vec![OverlayMask {
        feature_id: "river".to_string(),
        mask_class: "R2".to_string(),
        priority: 30,
        polygon: vec![
            Point::new(0.0, 0.0),
            Point::new(3.0, 0.0),
            Point::new(3.0, 1.0),
            Point::new(1.0, 1.0),
            Point::new(1.0, 3.0),
            Point::new(0.0, 3.0),
        ],
    }];

    let results = overlay_cells(&cells, &masks);

    assert_eq!(results[0].cell_id, "wet-cell");
    assert_eq!(results[0].winning_class, "R2");
    assert!((results[0].class_fractions[0].1 - 0.4375).abs() < 1.0e-9);
    assert_eq!(results[1].cell_id, "dry-cell");
    assert_eq!(results[1].winning_class, "UNKNOWN");
    assert_eq!(
        results[1].class_fractions,
        vec![("UNKNOWN".to_string(), 1.0)]
    );
    assert_eq!(results[1].quality_flags, vec!["missing_mask"]);
}
