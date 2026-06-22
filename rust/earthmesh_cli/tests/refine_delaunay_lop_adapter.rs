use earthmesh_cli::{apply_delaunay_lop_fortran_indexed, LonLatPoint};

fn point(lon: f64, lat: f64) -> LonLatPoint {
    LonLatPoint { lon, lat }
}

fn assert_point(actual: LonLatPoint, lon: f64, lat: f64) {
    assert!(
        (actual.lon - lon).abs() < 1.0e-12,
        "lon {:?} != {lon}",
        actual
    );
    assert!(
        (actual.lat - lat).abs() < 1.0e-12,
        "lat {:?} != {lat}",
        actual
    );
}

#[test]
fn delaunay_lop_adapter_flips_diagonal_and_updates_fortran_rows() {
    let iter = 2;
    let num_ref = 2;
    let num_mp = vec![0, 3, 5];
    let num_wp = vec![0, 13, 14];
    let mut mp_new = vec![point(0.0, 0.0); num_mp[iter] + 1];
    let mut wp_new = vec![point(0.0, 0.0); num_wp[iter] + 1];
    wp_new[10] = point(0.0, 0.0);
    wp_new[11] = point(6.0, 0.0);
    wp_new[12] = point(0.0, 6.0);
    wp_new[13] = point(6.0, 6.0);
    wp_new[14] = point(-180.0, 9.0);
    let mut ngrmw_new = vec![vec![0; num_mp[iter] + 1]; 4];
    ngrmw_new[1][2] = 10;
    ngrmw_new[2][2] = 11;
    ngrmw_new[3][2] = 12;
    ngrmw_new[1][3] = 11;
    ngrmw_new[2][3] = 12;
    ngrmw_new[3][3] = 13;
    let ref_sjx_segment = vec![0, 2, 3];

    let report = apply_delaunay_lop_fortran_indexed(
        iter,
        num_ref,
        &num_mp,
        &num_wp,
        &mut mp_new,
        &mut wp_new,
        &mut ngrmw_new,
        &ref_sjx_segment,
    )
    .expect("apply Delaunay LOP through CLI adapter");

    assert_eq!(report.flipped_pairs, vec![(2, 3)]);
    assert_eq!(report.new_triangle_ids, vec![4, 5]);
    assert!(!report.dateline_adjusted);
    assert_eq!(
        [ngrmw_new[1][4], ngrmw_new[2][4], ngrmw_new[3][4]],
        [10, 11, 13]
    );
    assert_eq!(
        [ngrmw_new[1][5], ngrmw_new[2][5], ngrmw_new[3][5]],
        [10, 12, 13]
    );
    assert_eq!(
        [ngrmw_new[1][2], ngrmw_new[2][2], ngrmw_new[3][2]],
        [1, 1, 1]
    );
    assert_eq!(
        [ngrmw_new[1][3], ngrmw_new[2][3], ngrmw_new[3][3]],
        [1, 1, 1]
    );
    assert_point(mp_new[4], 4.0, 2.0);
    assert_point(mp_new[5], 2.0, 4.0);
    assert_point(wp_new[14], 180.0, 9.0);
}

#[test]
fn delaunay_lop_adapter_skips_zero_pairs_and_preserves_dateline_cleanup() {
    let iter = 2;
    let num_ref = 4;
    let num_mp = vec![0, 3, 5];
    let num_wp = vec![0, 13, 13];
    let mut mp_new = vec![point(0.0, 0.0); num_mp[iter] + 1];
    let mut wp_new = vec![point(0.0, 0.0); num_wp[iter] + 1];
    wp_new[10] = point(170.0, 0.0);
    wp_new[11] = point(-170.0, 0.0);
    wp_new[12] = point(180.0, 6.0);
    wp_new[13] = point(-175.0, 6.0);
    let mut ngrmw_new = vec![vec![0; num_mp[iter] + 1]; 4];
    ngrmw_new[1][2] = 10;
    ngrmw_new[2][2] = 11;
    ngrmw_new[3][2] = 12;
    ngrmw_new[1][3] = 11;
    ngrmw_new[2][3] = 12;
    ngrmw_new[3][3] = 13;
    let ref_sjx_segment = vec![0, 0, 0, 2, 3];

    let report = apply_delaunay_lop_fortran_indexed(
        iter,
        num_ref,
        &num_mp,
        &num_wp,
        &mut mp_new,
        &mut wp_new,
        &mut ngrmw_new,
        &ref_sjx_segment,
    )
    .expect("apply Delaunay LOP through CLI adapter");

    assert_eq!(report.flipped_pairs, vec![(2, 3)]);
    assert_eq!(report.new_triangle_ids, vec![4, 5]);
    assert!(report.dateline_adjusted);
    assert_point(mp_new[4], -178.33333333333334, 2.0);
    assert_point(mp_new[5], 178.33333333333334, 4.0);
}
