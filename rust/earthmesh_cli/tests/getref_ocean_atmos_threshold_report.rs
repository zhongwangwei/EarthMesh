use earthmesh_cli::{
    calculate_getref_atmos_threshold_report_fortran_indexed,
    calculate_getref_ocean_threshold_report_fortran_indexed, GetRefAtmosThresholdConfig,
    GetRefOceanThresholdConfig, GetRefOneLayerThresholdInput,
};

fn one_based_i32(rows: &[[i32; 3]]) -> Vec<Vec<i32>> {
    std::iter::once(vec![0, 0, 0])
        .chain(rows.iter().map(|row| vec![row[0], row[1], row[2]]))
        .collect()
}

fn one_based_pairs(rows: &[[i32; 2]]) -> Vec<Vec<i32>> {
    std::iter::once(vec![0, 0])
        .chain(rows.iter().map(|row| vec![row[0], row[1]]))
        .collect()
}

#[test]
fn ocean_threshold_report_combines_sea_ratio_and_onelayer_columns_in_fortran_order() {
    let is_in_refine_sjx = vec![0, 0, 1, 1, 0];
    let ocn_id = one_based_i32(&[[0, 0, 0], [2, 1, 4], [2, 3, 2], [2, 5, 4]]);
    let ocn_ii = one_based_pairs(&[[1, 1], [1, 2], [2, 1], [2, 2], [3, 1], [3, 2]]);
    let landtypes = vec![vec![1; 3]; 4];
    let mut sst = vec![vec![0.0; 3]; 4];
    sst[1][1] = 1.0;
    sst[1][2] = 3.0;
    sst[2][1] = 10.0;
    sst[2][2] = 14.0;
    sst[3][1] = 100.0;
    sst[3][2] = 100.0;

    let report = calculate_getref_ocean_threshold_report_fortran_indexed(
        &is_in_refine_sjx,
        &ocn_id,
        &ocn_ii,
        &landtypes,
        GetRefOceanThresholdConfig {
            num_vertex: 1,
            maxlc: 9,
            refine_sea_ratio: true,
            th_sea_ratio: [0.4, 0.8],
        },
        &[GetRefOneLayerThresholdInput {
            name: "sst",
            values: &sst,
            mean_threshold: Some(11.0),
            std_threshold: Some(1.5),
        }],
    )
    .expect("calculate ocean threshold report");

    assert_eq!(report.ref_colnum, 3);
    assert_eq!(report.column_names, ["sea_ratio", "sst_m", "sst_s"]);
    assert_eq!(report.sea_ratio.as_ref().unwrap()[2], 0.5);
    assert_eq!(report.sea_ratio.as_ref().unwrap()[3], 1.0);
    assert_eq!(&report.ref_th[2][1..=3], &[1, 0, 0]);
    assert_eq!(&report.ref_th[3][1..=3], &[0, 1, 1]);
    assert_eq!(report.ref_sjx[2], 1);
    assert_eq!(report.ref_sjx[3], 1);
    assert_eq!(report.ref_sjx[4], 0);
    assert_eq!(report.onelayer_reports[0].p_num[3], 2);
}

#[test]
fn atmos_threshold_report_contains_only_onelayer_columns() {
    let is_in_refine_sjx = vec![0, 0, 1];
    let atmos_id = one_based_i32(&[[0, 0, 0], [2, 1, 0]]);
    let atmos_ii = one_based_pairs(&[[1, 1], [1, 2]]);
    let landtypes = vec![vec![1; 3]; 2];
    let mut typhoon = vec![vec![0.0; 3]; 2];
    typhoon[1][1] = 7.0;
    typhoon[1][2] = 9.0;

    let report = calculate_getref_atmos_threshold_report_fortran_indexed(
        &is_in_refine_sjx,
        &atmos_id,
        &atmos_ii,
        &landtypes,
        GetRefAtmosThresholdConfig {
            num_vertex: 1,
            maxlc: 9,
        },
        &[GetRefOneLayerThresholdInput {
            name: "typhoon",
            values: &typhoon,
            mean_threshold: Some(8.5),
            std_threshold: None,
        }],
    )
    .expect("calculate atmos threshold report");

    assert_eq!(report.ref_colnum, 1);
    assert_eq!(report.column_names, ["typhoon_m"]);
    assert_eq!(report.onelayer_reports[0].mean[2], 8.0);
    assert_eq!(report.ref_th[2][1], 0);
    assert_eq!(report.ref_sjx[2], 0);
}
