use earthmesh_cli::{calculate_getref_mean_std_2d_fortran_indexed, GetRefMeanStd2DConfig};

fn one_based_i32(rows: &[[i32; 2]]) -> Vec<Vec<i32>> {
    std::iter::once(vec![0, 0])
        .chain(rows.iter().map(|row| vec![row[0], row[1]]))
        .collect()
}

#[test]
fn getref_mean_std_2d_matches_fortran_mean_std_and_threshold_columns() {
    let is_in_refine_sjx = vec![0, 0, 0, 1, 1, 0];
    let mp_id = one_based_i32(&[[0, 0], [0, 0], [3, 1], [2, 4], [2, 6]]);
    let mp_ii = one_based_i32(&[[1, 1], [1, 2], [1, 3], [2, 1], [2, 2], [3, 1], [3, 2]]);
    let mut landtypes = vec![vec![1; 4]; 4];
    landtypes[3][1] = 9;
    landtypes[3][2] = 1;
    let mut var2d = vec![vec![0.0; 4]; 4];
    var2d[1][1] = 1.0;
    var2d[1][2] = 3.0;
    var2d[1][3] = 5.0;
    var2d[2][1] = 10.0;
    var2d[2][2] = 14.0;
    var2d[3][1] = 100.0;
    var2d[3][2] = 7.0;

    let report = calculate_getref_mean_std_2d_fortran_indexed(
        &is_in_refine_sjx,
        &mp_id,
        &mp_ii,
        &landtypes,
        &var2d,
        GetRefMeanStd2DConfig {
            num_vertex: 2,
            maxlc: 9,
            mean_threshold: Some(11.0),
            std_threshold: Some(1.5),
        },
    )
    .expect("calculate one-layer mean/std");

    assert_eq!(report.ref_colnum, 2);
    assert_eq!(report.p_num[3], 3);
    assert_eq!(report.p_num[4], 2);
    assert!((report.mean[3] - 3.0).abs() < 1.0e-12);
    assert!((report.mean[4] - 12.0).abs() < 1.0e-12);
    assert!((report.stddev.as_ref().unwrap()[3] - (8.0_f64 / 3.0).sqrt()).abs() < 1.0e-12);
    assert!((report.stddev.as_ref().unwrap()[4] - 2.0).abs() < 1.0e-12);

    assert_eq!(report.ref_th[3][1], 0);
    assert_eq!(report.ref_th[4][1], 1);
    assert_eq!(report.ref_th[3][2], 1);
    assert_eq!(report.ref_th[4][2], 1);
    assert_eq!(report.ref_sjx[3], 1);
    assert_eq!(report.ref_sjx[4], 1);
    assert_eq!(report.ref_sjx[5], 0);
}

#[test]
fn getref_mean_std_2d_can_emit_mean_only_and_skip_maxlc_cells() {
    let is_in_refine_sjx = vec![0, 0, 1];
    let mp_id = one_based_i32(&[[0, 0], [2, 1]]);
    let mp_ii = one_based_i32(&[[1, 1], [1, 2]]);
    let mut landtypes = vec![vec![1; 3]; 2];
    landtypes[1][1] = 9;
    landtypes[1][2] = 2;
    let mut var2d = vec![vec![0.0; 3]; 2];
    var2d[1][1] = 100.0;
    var2d[1][2] = 4.0;

    let report = calculate_getref_mean_std_2d_fortran_indexed(
        &is_in_refine_sjx,
        &mp_id,
        &mp_ii,
        &landtypes,
        &var2d,
        GetRefMeanStd2DConfig {
            num_vertex: 1,
            maxlc: 9,
            mean_threshold: Some(3.0),
            std_threshold: None,
        },
    )
    .expect("calculate mean-only threshold");

    assert_eq!(report.ref_colnum, 1);
    assert_eq!(report.p_num[2], 1);
    assert_eq!(report.mean[2], 4.0);
    assert!(report.stddev.is_none());
    assert_eq!(report.ref_th[2][1], 1);
}
