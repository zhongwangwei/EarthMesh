use earthmesh_mesh::{
    area_judge_closed_curve_fill_one_based, area_judge_minmax_range_make_one_based,
    area_judge_source_find_one_based, AreaJudgeAxis, AreaJudgeSourceBounds, LonLatDegrees,
};

fn one_degree_lon_vertices_one_based() -> Vec<f64> {
    let mut vertices = Vec::with_capacity(362);
    vertices.push(f64::NAN);
    for lon in -180..=180 {
        vertices.push(lon as f64);
    }
    vertices
}

fn one_degree_lat_vertices_one_based() -> Vec<f64> {
    let mut vertices = Vec::with_capacity(182);
    vertices.push(f64::NAN);
    for lat in (-90..=90).rev() {
        vertices.push(lat as f64);
    }
    vertices
}

#[test]
fn source_find_uses_canonical_one_based_window_for_lon_and_lat_vertices() {
    let lon_vertices = one_degree_lon_vertices_one_based();
    let lat_vertices = one_degree_lat_vertices_one_based();

    assert_eq!(
        area_judge_source_find_one_based(-180.0, &lon_vertices, AreaJudgeAxis::Longitude, 1, 360,),
        Some(1)
    );
    assert_eq!(
        area_judge_source_find_one_based(-179.2, &lon_vertices, AreaJudgeAxis::Longitude, 1, 360,),
        Some(2)
    );
    assert_eq!(
        area_judge_source_find_one_based(180.0, &lon_vertices, AreaJudgeAxis::Longitude, 1, 360,),
        Some(361)
    );

    assert_eq!(
        area_judge_source_find_one_based(90.0, &lat_vertices, AreaJudgeAxis::Latitude, 1, 180,),
        Some(1)
    );
    assert_eq!(
        area_judge_source_find_one_based(89.2, &lat_vertices, AreaJudgeAxis::Latitude, 1, 180,),
        Some(2)
    );
    assert_eq!(
        area_judge_source_find_one_based(-90.0, &lat_vertices, AreaJudgeAxis::Latitude, 1, 180,),
        Some(181)
    );
}

#[test]
fn minmax_range_make_returns_canonical_adjusted_cell_bounds() {
    let lon_vertices = one_degree_lon_vertices_one_based();
    let lat_vertices = one_degree_lat_vertices_one_based();

    let bounds = area_judge_minmax_range_make_one_based(
        -179.2,
        -177.2,
        89.2,
        87.2,
        &lon_vertices,
        &lat_vertices,
        1,
        360,
        180,
    );

    assert_eq!(
        bounds,
        Some(AreaJudgeSourceBounds {
            minlon_source: 2,
            maxlon_source: 2,
            maxlat_source: 2,
            minlat_source: 2,
        })
    );
}

#[test]
fn minmax_range_make_preserves_canonical_eastern_and_southern_edge_adjustments() {
    let lon_vertices = one_degree_lon_vertices_one_based();
    let lat_vertices = one_degree_lat_vertices_one_based();

    let bounds = area_judge_minmax_range_make_one_based(
        178.2,
        179.2,
        -88.2,
        -89.2,
        &lon_vertices,
        &lat_vertices,
        1,
        360,
        180,
    );

    assert_eq!(
        bounds,
        Some(AreaJudgeSourceBounds {
            minlon_source: 360,
            maxlon_source: 360,
            maxlat_source: 180,
            minlat_source: 180,
        })
    );
}

#[test]
fn minmax_range_make_rejects_subcell_bbox_instead_of_returning_inverted_bounds() {
    let lon_vertices = one_degree_lon_vertices_one_based();
    let lat_vertices = one_degree_lat_vertices_one_based();

    let bounds = area_judge_minmax_range_make_one_based(
        0.1,
        0.2,
        1.2,
        1.1,
        &lon_vertices,
        &lat_vertices,
        1,
        360,
        180,
    );

    assert_eq!(
        bounds, None,
        "subcell bbox must not produce reversed ranges"
    );
}

#[test]
fn closed_curve_fill_marks_cells_between_sorted_ray_intersections() {
    let lon_vertices = one_degree_lon_vertices_one_based();
    let lat_vertices = one_degree_lat_vertices_one_based();
    let square = [
        LonLatDegrees::new(0.0, 2.0),
        LonLatDegrees::new(2.0, 2.0),
        LonLatDegrees::new(2.0, 0.0),
        LonLatDegrees::new(0.0, 0.0),
    ];

    let filled = area_judge_closed_curve_fill_one_based(
        &square,
        &lon_vertices,
        &lat_vertices,
        1,
        360,
        180,
        false,
    )
    .expect("square fill should be valid");

    assert_eq!(filled.patch_count, 4);
    assert_eq!(
        filled.cells,
        vec![(181, 89), (182, 89), (181, 90), (182, 90)]
    );
}

#[test]
fn closed_curve_fill_restores_shifted_dateline_longitudes() {
    let lon_vertices = one_degree_lon_vertices_one_based();
    let lat_vertices = one_degree_lat_vertices_one_based();
    let shifted_square = [
        LonLatDegrees::new(-2.0, 2.0),
        LonLatDegrees::new(2.0, 2.0),
        LonLatDegrees::new(2.0, 0.0),
        LonLatDegrees::new(-2.0, 0.0),
    ];

    let filled = area_judge_closed_curve_fill_one_based(
        &shifted_square,
        &lon_vertices,
        &lat_vertices,
        1,
        360,
        180,
        true,
    )
    .expect("dateline-shifted fill should be valid");

    assert_eq!(filled.patch_count, 8);
    assert_eq!(
        filled.cells,
        vec![
            (359, 89),
            (360, 89),
            (1, 89),
            (2, 89),
            (359, 90),
            (360, 90),
            (1, 90),
            (2, 90)
        ]
    );
}
