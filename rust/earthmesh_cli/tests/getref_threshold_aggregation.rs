use earthmesh_cli::{
    aggregate_getref_threshold_reports_fortran_indexed, GetRefAtmosThresholdReport,
    GetRefLandThresholdReport, GetRefOceanThresholdReport,
};

#[test]
fn getref_threshold_aggregation_concatenates_land_ocean_atmos_and_marks_any_flagged_cell() {
    let land = GetRefLandThresholdReport {
        ref_colnum: 2,
        column_names: vec!["n_landtypes".into(), "lai_m".into()],
        ref_th_land: vec![
            vec![0, 0, 0],
            vec![0, 1, 1],
            vec![0, 1, 0],
            vec![0, 0, 1],
            vec![0, 0, 0],
        ],
        ref_sjx: vec![0, 0, 1, 1, 0],
        n_landtypes: Some(vec![0, 0, 2, 1, 0]),
        f_mainarea: None,
        onelayer_reports: vec![],
        twolayer_reports: vec![],
        last_p_num: None,
    };
    let ocean = GetRefOceanThresholdReport {
        ref_colnum: 1,
        column_names: vec!["sea_ratio".into()],
        ref_th: vec![vec![0, 0], vec![0, 0], vec![0, 0], vec![0, 1], vec![0, 0]],
        ref_sjx: vec![0, 0, 0, 1, 0],
        sea_ratio: Some(vec![0.0; 5]),
        onelayer_reports: vec![],
        last_p_num: None,
    };
    let atmos = GetRefAtmosThresholdReport {
        ref_colnum: 2,
        column_names: vec!["wind_m".into(), "wind_s".into()],
        ref_th: vec![
            vec![0, 0, 0],
            vec![0, 1, 0],
            vec![0, 0, 1],
            vec![0, 0, 0],
            vec![0, 0, 1],
        ],
        ref_sjx: vec![0, 0, 1, 0, 1],
        onelayer_reports: vec![],
        last_p_num: None,
    };

    let report = aggregate_getref_threshold_reports_fortran_indexed(
        1,
        Some(&land),
        Some(&ocean),
        Some(&atmos),
    )
    .expect("aggregate GetRef threshold reports");

    assert_eq!(report.ref_colnum, 5);
    assert_eq!(
        report.column_sources,
        ["land", "land", "ocean", "atmos", "atmos"]
    );
    assert_eq!(
        report.column_names,
        ["n_landtypes", "lai_m", "sea_ratio", "wind_m", "wind_s"]
    );
    assert_eq!(&report.ref_th[2][1..=5], &[1, 0, 0, 0, 1]);
    assert_eq!(&report.ref_th[3][1..=5], &[0, 1, 1, 0, 0]);
    assert_eq!(&report.ref_th[4][1..=5], &[0, 0, 0, 0, 1]);
    assert_eq!(report.ref_sjx, vec![0, 0, 1, 1, 1]);
}
