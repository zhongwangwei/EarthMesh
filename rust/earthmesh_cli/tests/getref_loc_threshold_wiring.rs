use earthmesh_cli::{
    calculate_getref_loc_threshold_reports_fortran_indexed, GetRefAtmosThresholdConfig,
    GetRefLandBasicConfig, GetRefOceanThresholdConfig, GetRefOneLayerThresholdInput,
};

fn one_based_i32(rows: &[[i32; 3]]) -> Vec<Vec<i32>> {
    std::iter::once(vec![0, 0, 0])
        .chain(rows.iter().map(|row| vec![row[0], row[1], row[2]]))
        .collect()
}

#[test]
fn loc_threshold_wiring_splits_components_calculates_reports_and_aggregates_columns() {
    let is_in_refine_sjx = vec![0, 0, 1, 1];
    let loc_id = one_based_i32(&[[0, 0, 0], [3, 1, 0], [2, 4, 0]]);
    let loc_ii = one_based_i32(&[[1, 1, 1], [1, 2, 0], [2, 1, 1], [2, 2, 0], [3, 1, 0]]);
    let mut landtypes = vec![vec![1; 3]; 4];
    landtypes[1][1] = 1;
    landtypes[2][1] = 2;

    let mut air = vec![vec![0.0; 3]; 4];
    air[1][1] = 1.0;
    air[1][2] = 8.0;
    air[2][1] = 1.0;
    air[2][2] = 10.0;
    air[3][1] = 10.0;

    let report = calculate_getref_loc_threshold_reports_fortran_indexed(
        &is_in_refine_sjx,
        &loc_id,
        &loc_ii,
        &landtypes,
        GetRefLandBasicConfig {
            num_vertex: 1,
            maxlc: 9,
            refine_num_landtypes: true,
            th_num_landtypes: 1,
            refine_area_mainland: false,
            th_area_mainland: 0.0,
        },
        &[],
        &[],
        GetRefOceanThresholdConfig {
            num_vertex: 1,
            maxlc: 9,
            refine_sea_ratio: true,
            th_sea_ratio: [0.2, 0.5],
        },
        &[],
        GetRefAtmosThresholdConfig {
            num_vertex: 1,
            maxlc: 9,
        },
        &[GetRefOneLayerThresholdInput {
            name: "air",
            values: &air,
            mean_threshold: Some(9.0),
            std_threshold: None,
        }],
    )
    .expect("calculate LOC threshold reports");

    assert_eq!(report.split.land.mp_id[2], vec![2, 1]);
    assert_eq!(report.split.ocean.mp_id[2], vec![1, 1, 3]);
    assert!(report.land.is_some());
    assert!(report.ocean.is_some());
    assert!(report.atmos.is_some());
    assert_eq!(report.aggregate.ref_colnum, 3);
    assert_eq!(report.aggregate.column_sources, ["land", "ocean", "atmos"]);
    assert_eq!(
        report.aggregate.column_names,
        ["n_landtypes", "sea_ratio", "air_m"]
    );
    assert_eq!(&report.aggregate.ref_th[2][1..=3], &[1, 1, 0]);
    assert_eq!(&report.aggregate.ref_th[3][1..=3], &[0, 0, 1]);
    assert_eq!(report.aggregate.ref_sjx, vec![0, 0, 1, 1]);
}
