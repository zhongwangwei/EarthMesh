use earthmesh_mesh::{spherical_centroid_degrees, LonLatDegrees};
use earthmesh_refine_redgreen::refine_onedivide_two_one_based;

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
fn onedivide_two_forward_splits_triangle_next_to_refined_neighbor() {
    let triangle_neighbors = vec![
        vec![0, 0, 0],
        vec![0, 0, 0],
        vec![3, 3, 3],
        vec![2, 2, 2],
        vec![2, 3, 3],
        vec![2, 3, 3],
    ];
    let cells_on_triangle = vec![
        [0, 0, 0],
        [0, 0, 0],
        [2, 3, 4],
        [3, 4, 5],
        [2, 5, 6],
        [2, 6, 7],
    ];
    let ref_sjx = vec![0, 0, 1, 0, 0, 0];
    let mrl_new = vec![0, 1, 1, 4, 1, 1];
    let num_mp = vec![0, 3, 5];
    let num_wp = vec![0, 6, 7];
    let mut triangle_points = vec![ll(0.0, 0.0); 6];
    let mut cell_points = vec![ll(0.0, 0.0); 8];
    cell_points[2] = ll(0.0, 0.0);
    cell_points[3] = ll(6.0, 0.0);
    cell_points[4] = ll(0.0, 6.0);
    let expected_midpoint = spherical_centroid_degrees(&[cell_points[3], cell_points[4]])
        .expect("spherical edge midpoint");
    let expected_a =
        spherical_centroid_degrees(&[cell_points[2], expected_midpoint, cell_points[3]])
            .expect("first child spherical centroid");
    let expected_b =
        spherical_centroid_degrees(&[cell_points[2], expected_midpoint, cell_points[4]])
            .expect("second child spherical centroid");
    let mut cells_on_triangle_new = cells_on_triangle.clone();
    let mut sjx_child = vec![[0, 0]; 6];

    refine_onedivide_two_one_based(
        2,
        false,
        1,
        &num_mp,
        &num_wp,
        &triangle_neighbors,
        &cells_on_triangle,
        &ref_sjx,
        &mrl_new,
        &mut triangle_points,
        &mut cell_points,
        &mut cells_on_triangle_new,
        &mut sjx_child,
    )
    .expect("split transition triangle into two children");

    assert_ll(
        cell_points[7],
        expected_midpoint.lon_degrees,
        expected_midpoint.lat_degrees,
    );
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
    assert_eq!(cells_on_triangle_new[2], [1, 1, 1]);
    assert_eq!(cells_on_triangle_new[4], [2, 3, 7]);
    assert_eq!(cells_on_triangle_new[5], [2, 4, 7]);
    assert_eq!(sjx_child[2], [4, 5]);
}

#[test]
fn onedivide_two_ignores_markers_after_previous_triangle_count() {
    let triangle_neighbors = vec![
        vec![0, 0, 0],
        vec![0, 0, 0],
        vec![3, 3, 3],
        vec![2, 2, 2],
        vec![2, 3, 3],
        vec![2, 3, 3],
    ];
    let cells_on_triangle = vec![
        [0, 0, 0],
        [0, 0, 0],
        [2, 3, 4],
        [3, 4, 5],
        [2, 5, 6],
        [2, 6, 7],
    ];
    let ref_sjx = vec![0, 0, 1, 0, 1, 0];
    let mrl_new = vec![0, 1, 1, 4, 1, 1];
    let num_mp = vec![0, 3, 5];
    let num_wp = vec![0, 6, 7];
    let mut triangle_points = vec![ll(0.0, 0.0); 6];
    let mut cell_points = vec![ll(0.0, 0.0); 8];
    cell_points[2] = ll(0.0, 0.0);
    cell_points[3] = ll(6.0, 0.0);
    cell_points[4] = ll(0.0, 6.0);
    let mut cells_on_triangle_new = cells_on_triangle.clone();
    let mut sjx_child = vec![[0, 0]; 6];

    refine_onedivide_two_one_based(
        2,
        false,
        1,
        &num_mp,
        &num_wp,
        &triangle_neighbors,
        &cells_on_triangle,
        &ref_sjx,
        &mrl_new,
        &mut triangle_points,
        &mut cell_points,
        &mut cells_on_triangle_new,
        &mut sjx_child,
    )
    .expect("one-into-two must only scan previous parent triangles");

    assert_eq!(sjx_child[2], [4, 5]);
    assert_eq!(sjx_child[4], [0, 0]);
    assert_eq!(cells_on_triangle_new[4], [2, 3, 7]);
    assert_eq!(cells_on_triangle_new[5], [2, 4, 7]);
}

#[test]
fn onedivide_two_reverse_uses_unrefined_neighbor_and_restores_dateline_shift() {
    let triangle_neighbors = vec![
        vec![0, 0, 0],
        vec![0, 0, 0],
        vec![3, 3, 3],
        vec![2, 2, 2],
        vec![2, 3, 3],
        vec![2, 3, 3],
    ];
    let cells_on_triangle = vec![
        [0, 0, 0],
        [0, 0, 0],
        [2, 3, 4],
        [3, 4, 5],
        [2, 5, 6],
        [2, 6, 7],
    ];
    let ref_sjx = vec![0, 0, 1, 0, 0, 0];
    let mrl_new = vec![0, 1, 1, 1, 4, 4];
    let num_mp = vec![0, 3, 5];
    let num_wp = vec![0, 6, 7];
    let mut triangle_points = vec![ll(0.0, 0.0); 6];
    let mut cell_points = vec![ll(0.0, 0.0); 8];
    cell_points[2] = ll(170.0, 0.0);
    cell_points[3] = ll(-170.0, 0.0);
    cell_points[4] = ll(180.0, 6.0);
    let expected_midpoint = spherical_centroid_degrees(&[cell_points[3], cell_points[4]])
        .expect("dateline spherical edge midpoint");
    let expected_a =
        spherical_centroid_degrees(&[cell_points[2], expected_midpoint, cell_points[3]])
            .expect("first dateline child centroid");
    let expected_b =
        spherical_centroid_degrees(&[cell_points[2], expected_midpoint, cell_points[4]])
            .expect("second dateline child centroid");
    let mut cells_on_triangle_new = cells_on_triangle.clone();
    let mut sjx_child = vec![[0, 0]; 6];

    refine_onedivide_two_one_based(
        2,
        true,
        1,
        &num_mp,
        &num_wp,
        &triangle_neighbors,
        &cells_on_triangle,
        &ref_sjx,
        &mrl_new,
        &mut triangle_points,
        &mut cell_points,
        &mut cells_on_triangle_new,
        &mut sjx_child,
    )
    .expect("reverse split uses the single unrefined neighbor");

    assert_ll(
        cell_points[7],
        expected_midpoint.lon_degrees,
        expected_midpoint.lat_degrees,
    );
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
    assert_eq!(sjx_child[2], [4, 5]);
}

#[test]
fn onedivide_two_rejects_marked_triangle_without_required_neighbor_state() {
    let triangle_neighbors = vec![
        vec![0, 0, 0],
        vec![0, 0, 0],
        vec![3, 3, 3],
        vec![2, 2, 2],
        vec![2, 3, 3],
        vec![2, 3, 3],
    ];
    let cells_on_triangle = vec![[0, 0, 0]; 6];
    let ref_sjx = vec![0, 0, 1, 0, 0, 0];
    let mrl_new = vec![0, 1, 1, 1, 1, 1];
    let num_mp = vec![0, 3, 5];
    let num_wp = vec![0, 6, 7];
    let mut triangle_points = vec![ll(0.0, 0.0); 6];
    let mut cell_points = vec![ll(0.0, 0.0); 8];
    let mut cells_on_triangle_new = cells_on_triangle.clone();
    let mut sjx_child = vec![[0, 0]; 6];

    let err = refine_onedivide_two_one_based(
        2,
        false,
        1,
        &num_mp,
        &num_wp,
        &triangle_neighbors,
        &cells_on_triangle,
        &ref_sjx,
        &mrl_new,
        &mut triangle_points,
        &mut cell_points,
        &mut cells_on_triangle_new,
        &mut sjx_child,
    )
    .expect_err("forward split requires a refined neighbor");

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
}
