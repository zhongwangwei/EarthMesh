use earthmesh_cli::{
    aggregate_getref_threshold_reports_fortran_indexed, read_getref_calculated_ref_sjx_netcdf,
    GetRefAtmosThresholdReport, GetRefLandThresholdReport, GetRefOceanThresholdReport,
};
use std::{fs, path::Path};

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

#[test]
fn getref_calculated_threshold_reader_or_aggregates_component_files_like_fortran() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_getref_threshold_aggregation_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let land = root.join("threshold_calculate_land_NXP0009_03.nc4");
    let ocean = root.join("threshold_calculate_ocean_NXP0009_03.nc4");
    let atmos = root.join("threshold_calculate_atmos_NXP0009_03.nc4");

    write_ref_th_matrix(&land, "ref_th_Lnd", 4, 2, &[1, 0, 0, 0, 0, 0, 0, 1]);
    write_ref_th_matrix(&ocean, "ref_th_Ocn", 4, 1, &[0, 1, 0, 0]);
    write_ref_th_matrix(&atmos, "ref_th_Atmos", 4, 2, &[0, 0, 0, 0, 0, 1, 0, 0]);

    let ref_sjx = read_getref_calculated_ref_sjx_netcdf(&[land, ocean, atmos], 1)
        .expect("read calculated threshold markers");

    assert_eq!(
        ref_sjx,
        vec![0, 0, 1, 1, 1],
        "Fortran calculated GetRef handoff ORs land/ocean/atmos ref_th flags and ignores num_vertex rows"
    );
}

fn write_ref_th_matrix(
    path: &Path,
    name: &str,
    sjx_points: usize,
    ref_colnum: usize,
    values: &[i32],
) {
    let mut file = netcdf::create(path).expect("create threshold fixture");
    file.add_dimension("sjx_points", sjx_points)
        .expect("add sjx_points");
    file.add_dimension("ref_colnum", ref_colnum)
        .expect("add ref_colnum");
    let mut variable = file
        .add_variable::<i32>(name, &["sjx_points", "ref_colnum"])
        .expect("add ref_th variable");
    variable
        .put_values(values, (.., ..))
        .expect("write ref_th values");
}
