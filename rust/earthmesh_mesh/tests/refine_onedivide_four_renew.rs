use earthmesh_mesh::{refine_onedivide_four_renew_fortran_indexed, LonLatDegrees};

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
fn onedivide_four_renew_generates_midpoint_cells_child_triangles_and_connectivity() {
    let cells_on_triangle = vec![[0, 0, 0], [0, 0, 0], [2, 3, 4], [4, 5, 6]];
    let ref_sjx_segment = vec![0, 0, 1, 0];
    let num_mp = vec![0, 3, 7];
    let num_wp = vec![0, 6, 9];
    let mut triangle_points = vec![ll(0.0, 0.0); 8];
    let mut cell_points = vec![ll(0.0, 0.0); 10];
    cell_points[2] = ll(0.0, 0.0);
    cell_points[3] = ll(6.0, 0.0);
    cell_points[4] = ll(0.0, 6.0);
    let mut cells_on_triangle_new = vec![[0, 0, 0]; 8];
    cells_on_triangle_new[2] = [2, 3, 4];
    cells_on_triangle_new[3] = [4, 5, 6];

    refine_onedivide_four_renew_fortran_indexed(
        2,
        1,
        &num_mp,
        &num_wp,
        &cells_on_triangle,
        &ref_sjx_segment,
        &mut triangle_points,
        &mut cell_points,
        &mut cells_on_triangle_new,
    )
    .expect("renew one refined triangle into four child triangles");

    assert_ll(cell_points[7], 3.0, 3.0);
    assert_ll(cell_points[8], 0.0, 3.0);
    assert_ll(cell_points[9], 3.0, 0.0);
    assert_ll(triangle_points[4], 1.0, 1.0);
    assert_ll(triangle_points[5], 4.0, 1.0);
    assert_ll(triangle_points[6], 1.0, 4.0);
    assert_ll(triangle_points[7], 2.0, 2.0);
    assert_eq!(cells_on_triangle_new[2], [1, 1, 1]);
    assert_eq!(cells_on_triangle_new[4], [2, 9, 8]);
    assert_eq!(cells_on_triangle_new[5], [3, 7, 9]);
    assert_eq!(cells_on_triangle_new[6], [4, 8, 7]);
    assert_eq!(cells_on_triangle_new[7], [7, 8, 9]);
}

#[test]
fn onedivide_four_renew_applies_fortran_dateline_shift_and_crossline_cleanup() {
    let cells_on_triangle = vec![[0, 0, 0], [0, 0, 0], [2, 3, 4]];
    let ref_sjx_segment = vec![0, 0, 1];
    let num_mp = vec![1, 2, 6];
    let num_wp = vec![1, 4, 7];
    let mut triangle_points = vec![ll(0.0, 0.0); 7];
    let mut cell_points = vec![ll(0.0, 0.0); 8];
    cell_points[2] = ll(170.0, 0.0);
    cell_points[3] = ll(-170.0, 0.0);
    cell_points[4] = ll(180.0, 6.0);
    let mut cells_on_triangle_new = vec![[0, 0, 0]; 7];
    cells_on_triangle_new[2] = [2, 3, 4];

    refine_onedivide_four_renew_fortran_indexed(
        2,
        1,
        &num_mp,
        &num_wp,
        &cells_on_triangle,
        &ref_sjx_segment,
        &mut triangle_points,
        &mut cell_points,
        &mut cells_on_triangle_new,
    )
    .expect("dateline-crossing triangle is shifted like Fortran");

    // Fortran CheckCrossing maps [170, -170, 180] to [-10, 10, 0], builds
    // children there, then shifts generated longitudes once more.  crossline_check
    // finally converts any generated -180 longitude to +180.
    assert_ll(cell_points[5], -175.0, 3.0);
    assert_ll(cell_points[6], 175.0, 3.0);
    assert_ll(cell_points[7], 180.0, 0.0);
    assert_ll(triangle_points[3], 175.0, 1.0);
    assert_ll(triangle_points[4], -175.0, 1.0);
    assert_ll(triangle_points[5], 180.0, 4.0);
    assert_ll(triangle_points[6], 180.0, 2.0);
}

#[test]
fn onedivide_four_renew_rejects_too_short_output_storage() {
    let cells_on_triangle = vec![[0, 0, 0], [0, 0, 0], [2, 3, 4]];
    let ref_sjx_segment = vec![0, 0, 1];
    let num_mp = vec![0, 2, 6];
    let num_wp = vec![0, 4, 7];
    let mut triangle_points = vec![ll(0.0, 0.0); 6];
    let mut cell_points = vec![ll(0.0, 0.0); 8];
    let mut cells_on_triangle_new = vec![[0, 0, 0]; 7];

    let err = refine_onedivide_four_renew_fortran_indexed(
        2,
        1,
        &num_mp,
        &num_wp,
        &cells_on_triangle,
        &ref_sjx_segment,
        &mut triangle_points,
        &mut cell_points,
        &mut cells_on_triangle_new,
    )
    .expect_err("missing child triangle storage should be rejected");

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
}
