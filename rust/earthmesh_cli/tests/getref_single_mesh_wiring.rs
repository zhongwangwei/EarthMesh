use earthmesh_cli::{
    calculate_getref_single_mesh_threshold_reports_fortran_indexed, GetRefAtmosThresholdConfig,
    GetRefLandBasicConfig, GetRefOceanThresholdConfig,
};

fn one_based_i32(rows: &[[i32; 2]]) -> Vec<Vec<i32>> {
    std::iter::once(vec![0, 0])
        .chain(rows.iter().map(|row| vec![row[0], row[1]]))
        .collect()
}

#[test]
fn single_mesh_wiring_calculates_land_report_and_top_level_aggregate() {
    let is_in_refine_sjx = vec![0, 0, 1, 1];
    let lnd_id = one_based_i32(&[[0, 0], [2, 1], [2, 3]]);
    let lnd_ii = one_based_i32(&[[1, 1], [2, 1], [1, 2], [2, 2]]);
    let mut landtypes = vec![vec![1; 3]; 3];
    landtypes[1][1] = 1;
    landtypes[2][1] = 2;
    landtypes[1][2] = 1;
    landtypes[2][2] = 1;

    let report = calculate_getref_single_mesh_threshold_reports_fortran_indexed(
        "landmesh",
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
        &[],
        &[],
        GetRefOceanThresholdConfig {
            num_vertex: 1,
            maxlc: 9,
            refine_sea_ratio: false,
            th_sea_ratio: [0.0, 0.0],
        },
        &[],
        GetRefAtmosThresholdConfig {
            num_vertex: 1,
            maxlc: 9,
        },
        &[],
    )
    .expect("calculate landmesh GetRef report");

    assert_eq!(report.mesh_type, "landmesh");
    assert!(report.land.is_some());
    assert!(report.ocean.is_none());
    assert!(report.atmos.is_none());
    assert_eq!(report.aggregate.ref_colnum, 1);
    assert_eq!(report.aggregate.column_sources, ["land"]);
    assert_eq!(report.aggregate.column_names, ["n_landtypes"]);
    assert_eq!(report.aggregate.ref_sjx, vec![0, 0, 1, 0]);
}
