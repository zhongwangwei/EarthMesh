use earthmesh_cli::{apply_onedivide_two_fortran_indexed, LonLatPoint};

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
fn onedivide_two_adapter_splits_transition_triangle_and_updates_fortran_rows() {
    let iter = 2;
    let num_vertex = 1;
    let num_mp = vec![0, 3, 5];
    let num_wp = vec![0, 6, 7];
    let triangle_neighbors = vec![
        vec![0, 0, 0],
        vec![0, 0, 0],
        vec![3, 3, 3],
        vec![2, 2, 2],
        vec![2, 3, 3],
        vec![2, 3, 3],
    ];
    let ngrmw = vec![
        vec![0, 0, 0, 0, 0, 0],
        vec![0, 0, 2, 3, 2, 2],
        vec![0, 0, 3, 4, 5, 6],
        vec![0, 0, 4, 5, 6, 7],
    ];
    let mut ngrmw_new = ngrmw.clone();
    let ref_sjx = vec![0, 0, 1, 0, 0, 0];
    let mrl_new = vec![0, 1, 1, 4, 1, 1];
    let mut mp_new = vec![point(0.0, 0.0); num_mp[iter] + 1];
    let mut wp_new = vec![point(0.0, 0.0); num_wp[iter] + 1];
    wp_new[2] = point(0.0, 0.0);
    wp_new[3] = point(6.0, 0.0);
    wp_new[4] = point(0.0, 6.0);
    let mut sjx_child = vec![[0, 0]; num_mp[iter] + 1];

    let report = apply_onedivide_two_fortran_indexed(
        iter,
        false,
        num_vertex,
        &num_mp,
        &num_wp,
        &triangle_neighbors,
        &ngrmw,
        &ref_sjx,
        &mrl_new,
        &mut mp_new,
        &mut wp_new,
        &mut ngrmw_new,
        &mut sjx_child,
    )
    .expect("apply one-into-two transition split through CLI adapter");

    assert_eq!(report.split_triangles, vec![2]);
    assert_eq!(report.new_triangle_ids, vec![4, 5]);
    assert_eq!(report.new_vertex_ids, vec![7]);
    assert!(!report.dateline_adjusted);

    assert_point(wp_new[7], 3.0, 3.0);
    assert_point(mp_new[4], 3.0, 1.0);
    assert_point(mp_new[5], 1.0, 3.0);
    assert_eq!(sjx_child[2], [4, 5]);
    assert_eq!(
        [ngrmw_new[1][2], ngrmw_new[2][2], ngrmw_new[3][2]],
        [1, 1, 1]
    );
    assert_eq!(
        [ngrmw_new[1][4], ngrmw_new[2][4], ngrmw_new[3][4]],
        [2, 3, 7]
    );
    assert_eq!(
        [ngrmw_new[1][5], ngrmw_new[2][5], ngrmw_new[3][5]],
        [2, 4, 7]
    );
}

#[test]
fn onedivide_two_adapter_preserves_dateline_cleanup() {
    let iter = 2;
    let num_vertex = 1;
    let num_mp = vec![0, 3, 5];
    let num_wp = vec![0, 6, 7];
    let triangle_neighbors = vec![
        vec![0, 0, 0],
        vec![0, 0, 0],
        vec![3, 3, 3],
        vec![2, 2, 2],
        vec![2, 3, 3],
        vec![2, 3, 3],
    ];
    let ngrmw = vec![
        vec![0, 0, 0, 0, 0, 0],
        vec![0, 0, 2, 3, 2, 2],
        vec![0, 0, 3, 4, 5, 6],
        vec![0, 0, 4, 5, 6, 7],
    ];
    let mut ngrmw_new = ngrmw.clone();
    let ref_sjx = vec![0, 0, 1, 0, 0, 0];
    let mrl_new = vec![0, 1, 1, 1, 4, 4];
    let mut mp_new = vec![point(0.0, 0.0); num_mp[iter] + 1];
    let mut wp_new = vec![point(0.0, 0.0); num_wp[iter] + 1];
    wp_new[2] = point(170.0, 0.0);
    wp_new[3] = point(-170.0, 0.0);
    wp_new[4] = point(180.0, 6.0);
    let mut sjx_child = vec![[0, 0]; num_mp[iter] + 1];

    let report = apply_onedivide_two_fortran_indexed(
        iter,
        true,
        num_vertex,
        &num_mp,
        &num_wp,
        &triangle_neighbors,
        &ngrmw,
        &ref_sjx,
        &mrl_new,
        &mut mp_new,
        &mut wp_new,
        &mut ngrmw_new,
        &mut sjx_child,
    )
    .expect("apply reverse one-into-two transition split through CLI adapter");

    assert_eq!(report.split_triangles, vec![2]);
    assert!(report.dateline_adjusted);
    assert_point(wp_new[7], -175.0, 3.0);
    assert_point(mp_new[4], -178.33333333333334, 1.0);
    assert_point(mp_new[5], 178.33333333333334, 3.0);
    assert_eq!(sjx_child[2], [4, 5]);
}
