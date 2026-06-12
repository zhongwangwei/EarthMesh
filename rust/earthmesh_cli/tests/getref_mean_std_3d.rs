use earthmesh_cli::{calculate_getref_mean_std_3d_fortran_indexed, GetRefMeanStd3DConfig};

fn one_based_i32(rows: &[[i32; 2]]) -> Vec<Vec<i32>> {
    std::iter::once(vec![0, 0])
        .chain(rows.iter().map(|row| vec![row[0], row[1]]))
        .collect()
}

fn blank_layers(nlon: usize, nlat: usize) -> Vec<Vec<Vec<f64>>> {
    vec![vec![vec![0.0; nlat + 1]; nlon + 1]; 2]
}

#[test]
fn getref_mean_std_3d_matches_fortran_two_layer_mean_std_and_or_thresholds() {
    let is_in_refine_sjx = vec![0, 0, 0, 1, 1, 0];
    let mp_id = one_based_i32(&[[0, 0], [0, 0], [3, 1], [2, 4], [2, 6]]);
    let mp_ii = one_based_i32(&[[1, 1], [1, 2], [1, 3], [2, 1], [2, 2], [3, 1], [3, 2]]);
    let mut landtypes = vec![vec![1; 4]; 4];
    landtypes[3][1] = 9;
    landtypes[3][2] = 1;
    let mut var3d = blank_layers(3, 3);
    var3d[0][1][1] = 1.0;
    var3d[0][1][2] = 3.0;
    var3d[0][1][3] = 5.0;
    var3d[1][1][1] = 10.0;
    var3d[1][1][2] = 14.0;
    var3d[1][1][3] = 18.0;
    var3d[0][2][1] = 8.0;
    var3d[0][2][2] = 10.0;
    var3d[1][2][1] = 2.0;
    var3d[1][2][2] = 4.0;
    var3d[0][3][1] = 100.0;
    var3d[1][3][1] = 100.0;
    var3d[0][3][2] = 7.0;
    var3d[1][3][2] = 9.0;

    let report = calculate_getref_mean_std_3d_fortran_indexed(
        &is_in_refine_sjx,
        &mp_id,
        &mp_ii,
        &landtypes,
        &var3d,
        GetRefMeanStd3DConfig {
            num_vertex: 2,
            maxlc: 9,
            mean_thresholds: Some([11.0, 15.0]),
            std_thresholds: Some([2.0, 3.0]),
        },
    )
    .expect("calculate two-layer mean/std");

    assert_eq!(report.ref_colnum, 2);
    assert_eq!(report.p_num[3], 3);
    assert_eq!(report.p_num[4], 2);
    assert_eq!(report.mean[3], [3.0, 14.0]);
    assert_eq!(report.mean[4], [9.0, 3.0]);
    let stddev = report.stddev.as_ref().unwrap();
    assert!((stddev[3][0] - (8.0_f64 / 3.0).sqrt()).abs() < 1.0e-12);
    assert!((stddev[3][1] - (32.0_f64 / 3.0).sqrt()).abs() < 1.0e-12);
    assert_eq!(stddev[4], [1.0, 1.0]);

    assert_eq!(report.ref_th[3][1], 0);
    assert_eq!(report.ref_th[4][1], 0);
    assert_eq!(report.ref_th[3][2], 1);
    assert_eq!(report.ref_th[4][2], 0);
    assert_eq!(report.ref_sjx[3], 1);
    assert_eq!(report.ref_sjx[4], 0);
    assert_eq!(report.ref_sjx[5], 0);
}

#[test]
fn getref_mean_std_3d_can_emit_mean_only_and_skip_maxlc_cells() {
    let is_in_refine_sjx = vec![0, 0, 1];
    let mp_id = one_based_i32(&[[0, 0], [2, 1]]);
    let mp_ii = one_based_i32(&[[1, 1], [1, 2]]);
    let mut landtypes = vec![vec![1; 3]; 2];
    landtypes[1][1] = 9;
    landtypes[1][2] = 2;
    let mut var3d = blank_layers(1, 2);
    var3d[0][1][1] = 100.0;
    var3d[1][1][1] = 100.0;
    var3d[0][1][2] = 4.0;
    var3d[1][1][2] = 8.0;

    let report = calculate_getref_mean_std_3d_fortran_indexed(
        &is_in_refine_sjx,
        &mp_id,
        &mp_ii,
        &landtypes,
        &var3d,
        GetRefMeanStd3DConfig {
            num_vertex: 1,
            maxlc: 9,
            mean_thresholds: Some([5.0, 7.0]),
            std_thresholds: None,
        },
    )
    .expect("calculate mean-only two-layer threshold");

    assert_eq!(report.ref_colnum, 1);
    assert_eq!(report.p_num[2], 1);
    assert_eq!(report.mean[2], [4.0, 8.0]);
    assert!(report.stddev.is_none());
    assert_eq!(report.ref_th[2][1], 1);
}
