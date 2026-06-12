use earthmesh_cli::{
    calculate_getref_land_threshold_report_fortran_indexed, GetRefLandBasicConfig,
    GetRefOneLayerThresholdInput, GetRefTwoLayerThresholdInput,
};

fn one_based_i32(rows: &[[i32; 2]]) -> Vec<Vec<i32>> {
    std::iter::once(vec![0, 0])
        .chain(rows.iter().map(|row| vec![row[0], row[1]]))
        .collect()
}

fn blank_layers(nlon: usize, nlat: usize) -> Vec<Vec<Vec<f64>>> {
    vec![vec![vec![0.0; nlat + 1]; nlon + 1]; 2]
}

#[test]
fn land_threshold_report_combines_basic_onelayer_and_twolayer_columns_in_fortran_order() {
    let is_in_refine_sjx = vec![0, 0, 1, 1];
    let lnd_id = one_based_i32(&[[0, 0], [2, 1], [2, 3]]);
    let lnd_ii = one_based_i32(&[[1, 1], [1, 2], [2, 1], [2, 2]]);
    let mut landtypes = vec![vec![0; 3]; 3];
    landtypes[1][1] = 1;
    landtypes[1][2] = 2;
    landtypes[2][1] = 1;
    landtypes[2][2] = 1;

    let mut lai = vec![vec![0.0; 3]; 3];
    lai[1][1] = 2.0;
    lai[1][2] = 4.0;
    lai[2][1] = 10.0;
    lai[2][2] = 12.0;

    let mut k_s = blank_layers(2, 2);
    k_s[0][1][1] = 30.0;
    k_s[0][1][2] = 32.0;
    k_s[1][1][1] = 3.0;
    k_s[1][1][2] = 5.0;
    k_s[0][2][1] = 1.0;
    k_s[0][2][2] = 1.0;
    k_s[1][2][1] = 2.0;
    k_s[1][2][2] = 2.0;

    let report = calculate_getref_land_threshold_report_fortran_indexed(
        &is_in_refine_sjx,
        &lnd_id,
        &lnd_ii,
        &landtypes,
        GetRefLandBasicConfig {
            num_vertex: 1,
            maxlc: 9,
            refine_num_landtypes: true,
            th_num_landtypes: 1,
            refine_area_mainland: false,
            th_area_mainland: 0.0,
        },
        &[GetRefOneLayerThresholdInput {
            name: "lai",
            values: &lai,
            mean_threshold: Some(3.5),
            std_threshold: Some(0.5),
        }],
        &[GetRefTwoLayerThresholdInput {
            name: "k_s",
            layers: &k_s,
            mean_thresholds: Some([20.0, 20.0]),
            std_thresholds: None,
        }],
    )
    .expect("calculate combined land threshold report");

    assert_eq!(report.ref_colnum, 4);
    assert_eq!(
        report.column_names,
        ["n_landtypes", "lai_m", "lai_s", "k_s_m"]
    );
    assert_eq!(&report.ref_th_land[2][1..=4], &[1, 0, 1, 1]);
    assert_eq!(&report.ref_th_land[3][1..=4], &[0, 1, 1, 0]);
    assert_eq!(report.ref_sjx[2], 1);
    assert_eq!(report.ref_sjx[3], 1);
    assert_eq!(report.last_p_num.as_ref().unwrap()[2], 2);
    assert_eq!(report.last_p_num.as_ref().unwrap()[3], 2);
    assert_eq!(report.onelayer_reports[0].mean[2], 3.0);
    assert_eq!(report.twolayer_reports[0].mean[2], [31.0, 4.0]);
}
