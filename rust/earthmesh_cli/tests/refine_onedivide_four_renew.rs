use earthmesh_cli::{apply_onedivide_four_renew_fortran_indexed, LonLatPoint};

fn point(lon: f64, lat: f64) -> LonLatPoint {
    LonLatPoint { lon, lat }
}

#[test]
fn onedivide_four_renew_splits_selected_triangle_into_four_children() {
    let num_vertex = 1;
    let iter = 2;
    let ref_sjx_segment = vec![0, 0, 1];
    let num_mp = vec![0, 2, 6];
    let num_wp = vec![0, 3, 6];
    let ngrmw = vec![
        vec![0, 0, 0, 0, 0, 0, 0],
        vec![0, 1, 1, 0, 0, 0, 0],
        vec![0, 2, 2, 0, 0, 0, 0],
        vec![0, 3, 3, 0, 0, 0, 0],
    ];
    let mut ngrmw_new = ngrmw.clone();
    let mut mp_new = vec![point(0.0, 0.0); num_mp[iter] + 1];
    mp_new[1] = point(2.0, 2.0);
    mp_new[2] = point(2.0, 2.0);
    let mut wp_new = vec![point(0.0, 0.0); num_wp[iter] + 1];
    wp_new[1] = point(0.0, 0.0);
    wp_new[2] = point(6.0, 0.0);
    wp_new[3] = point(0.0, 6.0);

    let report = apply_onedivide_four_renew_fortran_indexed(
        num_vertex,
        iter,
        &ngrmw,
        &ref_sjx_segment,
        &num_mp,
        &num_wp,
        &mut mp_new,
        &mut wp_new,
        &mut ngrmw_new,
    )
    .expect("apply one-into-four renewal");

    assert_eq!(report.refined_triangles, vec![2]);
    assert_eq!(report.new_triangle_ids, vec![3, 4, 5, 6]);
    assert_eq!(report.new_vertex_ids, vec![4, 5, 6]);
    assert_eq!(report.dateline_adjusted, false);

    assert_eq!(wp_new[4], point(3.0, 3.0));
    assert_eq!(wp_new[5], point(0.0, 3.0));
    assert_eq!(wp_new[6], point(3.0, 0.0));
    assert_eq!(mp_new[3], point(1.0, 1.0));
    assert_eq!(mp_new[4], point(4.0, 1.0));
    assert_eq!(mp_new[5], point(1.0, 4.0));
    assert_eq!(mp_new[6], point(2.0, 2.0));

    assert_eq!(
        [ngrmw_new[1][2], ngrmw_new[2][2], ngrmw_new[3][2]],
        [1, 1, 1]
    );
    assert_eq!(
        [ngrmw_new[1][3], ngrmw_new[2][3], ngrmw_new[3][3]],
        [1, 6, 5]
    );
    assert_eq!(
        [ngrmw_new[1][4], ngrmw_new[2][4], ngrmw_new[3][4]],
        [2, 4, 6]
    );
    assert_eq!(
        [ngrmw_new[1][5], ngrmw_new[2][5], ngrmw_new[3][5]],
        [3, 5, 4]
    );
    assert_eq!(
        [ngrmw_new[1][6], ngrmw_new[2][6], ngrmw_new[3][6]],
        [4, 5, 6]
    );
}

#[test]
fn onedivide_four_renew_applies_fortran_dateline_shift_and_crossline_cleanup() {
    let num_vertex = 1;
    let iter = 2;
    let ref_sjx_segment = vec![0, 0, 1];
    let num_mp = vec![1, 2, 6];
    let num_wp = vec![1, 4, 7];
    let ngrmw = vec![
        vec![0, 0, 0, 0, 0, 0, 0],
        vec![0, 0, 2, 0, 0, 0, 0],
        vec![0, 0, 3, 0, 0, 0, 0],
        vec![0, 0, 4, 0, 0, 0, 0],
    ];
    let mut ngrmw_new = ngrmw.clone();
    let mut mp_new = vec![point(0.0, 0.0); num_mp[iter] + 1];
    let mut wp_new = vec![point(0.0, 0.0); num_wp[iter] + 1];
    wp_new[2] = point(170.0, 0.0);
    wp_new[3] = point(-170.0, 0.0);
    wp_new[4] = point(180.0, 6.0);

    let report = apply_onedivide_four_renew_fortran_indexed(
        num_vertex,
        iter,
        &ngrmw,
        &ref_sjx_segment,
        &num_mp,
        &num_wp,
        &mut mp_new,
        &mut wp_new,
        &mut ngrmw_new,
    )
    .expect("dateline-crossing triangle is shifted like Fortran");

    assert_eq!(report.refined_triangles, vec![2]);
    assert_eq!(report.new_triangle_ids, vec![3, 4, 5, 6]);
    assert_eq!(report.new_vertex_ids, vec![5, 6, 7]);
    assert_eq!(report.dateline_adjusted, true);

    assert_eq!(wp_new[5], point(-175.0, 3.0));
    assert_eq!(wp_new[6], point(175.0, 3.0));
    assert_eq!(wp_new[7], point(180.0, 0.0));
    assert_eq!(mp_new[3], point(175.0, 1.0));
    assert_eq!(mp_new[4], point(-175.0, 1.0));
    assert_eq!(mp_new[5], point(180.0, 4.0));
    assert_eq!(mp_new[6], point(180.0, 2.0));

    assert_eq!(
        [ngrmw_new[1][3], ngrmw_new[2][3], ngrmw_new[3][3]],
        [2, 7, 6]
    );
    assert_eq!(
        [ngrmw_new[1][4], ngrmw_new[2][4], ngrmw_new[3][4]],
        [3, 5, 7]
    );
    assert_eq!(
        [ngrmw_new[1][5], ngrmw_new[2][5], ngrmw_new[3][5]],
        [4, 6, 5]
    );
    assert_eq!(
        [ngrmw_new[1][6], ngrmw_new[2][6], ngrmw_new[3][6]],
        [5, 6, 7]
    );
}
