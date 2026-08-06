use earthmesh_mesh::{spherical_centroid_degrees, LonLatDegrees};
use earthmesh_refine_redgreen::{checked_lop_edge_flip, refine_delaunay_lop_one_based};

fn ll(lon: f64, lat: f64) -> LonLatDegrees {
    LonLatDegrees::new(lon, lat)
}

fn assert_ll(actual: LonLatDegrees, lon: f64, lat: f64) {
    assert!(
        (actual.lon_degrees - lon).abs() < 1.0e-12,
        "lon {:?} != {lon}",
        actual
    );
    assert!(
        (actual.lat_degrees - lat).abs() < 1.0e-12,
        "lat {:?} != {lat}",
        actual
    );
}

#[test]
fn delaunay_lop_flips_adjacent_triangle_diagonal_and_clears_old_triangles() {
    let num_mp = vec![0, 3, 5];
    let num_wp = vec![0, 13, 14];
    let mut triangle_points = vec![ll(0.0, 0.0); 6];
    let mut cell_points = vec![ll(0.0, 0.0); 15];
    cell_points[10] = ll(0.0, 0.0);
    cell_points[11] = ll(6.0, 0.0);
    cell_points[12] = ll(0.0, 6.0);
    cell_points[13] = ll(6.0, 6.0);
    cell_points[14] = ll(-180.0, 9.0);
    let expected_a =
        spherical_centroid_degrees(&[cell_points[10], cell_points[11], cell_points[13]])
            .expect("first spherical centroid");
    let expected_b =
        spherical_centroid_degrees(&[cell_points[10], cell_points[12], cell_points[13]])
            .expect("second spherical centroid");
    let mut cells_on_triangle = vec![[0, 0, 0]; 6];
    cells_on_triangle[2] = [10, 11, 12];
    cells_on_triangle[3] = [11, 12, 13];
    let ref_segment = vec![2, 3];

    refine_delaunay_lop_one_based(
        2,
        2,
        &num_mp,
        &num_wp,
        &mut triangle_points,
        &mut cell_points,
        &mut cells_on_triangle,
        &ref_segment,
    )
    .expect("Delaunay LOP diagonal flip");

    assert_eq!(cells_on_triangle[4], [10, 11, 13]);
    assert_eq!(cells_on_triangle[5], [10, 12, 13]);
    assert_eq!(cells_on_triangle[2], [1, 1, 1]);
    assert_eq!(cells_on_triangle[3], [1, 1, 1]);
    assert_ll(
        triangle_points[4],
        expected_a.lon_degrees,
        expected_a.lat_degrees,
    );
    assert_ll(
        triangle_points[5],
        expected_b.lon_degrees,
        expected_b.lat_degrees,
    );
    assert_ll(cell_points[14], 180.0, 9.0);
}

#[test]
fn delaunay_lop_skips_zero_pairs_without_advancing_output_child_counter() {
    let num_mp = vec![0, 3, 5];
    let num_wp = vec![0, 13, 13];
    let mut triangle_points = vec![ll(0.0, 0.0); 6];
    let mut cell_points = vec![ll(0.0, 0.0); 14];
    cell_points[10] = ll(0.0, 0.0);
    cell_points[11] = ll(6.0, 0.0);
    cell_points[12] = ll(0.0, 6.0);
    cell_points[13] = ll(6.0, 6.0);
    let mut cells_on_triangle = vec![[0, 0, 0]; 6];
    cells_on_triangle[2] = [10, 11, 12];
    cells_on_triangle[3] = [11, 12, 13];
    let ref_segment = vec![0, 0, 2, 3];

    refine_delaunay_lop_one_based(
        2,
        4,
        &num_mp,
        &num_wp,
        &mut triangle_points,
        &mut cell_points,
        &mut cells_on_triangle,
        &ref_segment,
    )
    .expect("Delaunay LOP skips zero pair");

    assert_eq!(cells_on_triangle[4], [10, 11, 13]);
    assert_eq!(cells_on_triangle[5], [10, 12, 13]);
}

#[test]
fn delaunay_lop_applies_canonical_dateline_shift_before_centroid_cleanup() {
    let num_mp = vec![0, 3, 5];
    let num_wp = vec![0, 13, 13];
    let mut triangle_points = vec![ll(0.0, 0.0); 6];
    let mut cell_points = vec![ll(0.0, 0.0); 14];
    cell_points[10] = ll(170.0, 0.0);
    cell_points[11] = ll(-170.0, 0.0);
    cell_points[12] = ll(180.0, 6.0);
    cell_points[13] = ll(-175.0, 6.0);
    let expected_a =
        spherical_centroid_degrees(&[cell_points[10], cell_points[11], cell_points[13]])
            .expect("first dateline spherical centroid");
    let expected_b =
        spherical_centroid_degrees(&[cell_points[10], cell_points[12], cell_points[13]])
            .expect("second dateline spherical centroid");
    let mut cells_on_triangle = vec![[0, 0, 0]; 6];
    cells_on_triangle[2] = [10, 11, 12];
    cells_on_triangle[3] = [11, 12, 13];
    let ref_segment = vec![2, 3];

    refine_delaunay_lop_one_based(
        2,
        2,
        &num_mp,
        &num_wp,
        &mut triangle_points,
        &mut cell_points,
        &mut cells_on_triangle,
        &ref_segment,
    )
    .expect("Delaunay LOP dateline correction");

    assert_ll(
        triangle_points[4],
        expected_a.lon_degrees,
        expected_a.lat_degrees,
    );
    assert_ll(
        triangle_points[5],
        expected_b.lon_degrees,
        expected_b.lat_degrees,
    );
}

#[test]
fn checked_lop_centroids_use_spherical_vector_average() {
    let points = vec![
        ll(0.0, 0.0),
        ll(0.0, 0.0),
        ll(-90.0, 80.0),
        ll(0.0, 70.0),
        ll(90.0, 80.0),
        ll(180.0, 70.0),
    ];
    let flip =
        checked_lop_edge_flip(2, 3, [2, 3, 4], [3, 4, 5], &points).expect("valid shared edge");
    let expected = spherical_centroid_degrees(&[points[2], points[3], points[5]])
        .expect("non-degenerate spherical centroid");

    assert!((flip.centroids[0].lon_degrees - expected.lon_degrees).abs() < 1.0e-12);
    assert!((flip.centroids[0].lat_degrees - expected.lat_degrees).abs() < 1.0e-12);
}
