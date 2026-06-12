use std::fs;

use earthmesh_cli::{
    write_getref_atmos_threshold_netcdf, write_getref_ocean_threshold_netcdf,
    GetRefAtmosThresholdReport, GetRefMeanStd2DReport, GetRefOceanThresholdReport,
};

#[test]
fn ocean_threshold_writer_preserves_sea_ratio_onelayer_pnum_and_ref_flags() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_getref_ocean_threshold_writer_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let output = root.join("threshold_calculate_ocean_NXP0009_03.nc4");

    let report = GetRefOceanThresholdReport {
        ref_colnum: 3,
        column_names: vec!["sea_ratio".into(), "sst_m".into(), "sst_s".into()],
        ref_th: vec![vec![0, 0, 0, 0], vec![0, 0, 0, 0], vec![0, 1, 0, 1]],
        ref_sjx: vec![0, 0, 1],
        sea_ratio: Some(vec![0.0, 0.0, 0.5]),
        onelayer_reports: vec![GetRefMeanStd2DReport {
            ref_colnum: 2,
            ref_th: vec![vec![0; 3]; 3],
            ref_sjx: vec![0; 3],
            p_num: vec![0, 0, 2],
            mean: vec![0.0, 0.0, 12.0],
            stddev: Some(vec![0.0, 0.0, 2.0]),
        }],
        last_p_num: Some(vec![0, 0, 2]),
    };

    let written = write_getref_ocean_threshold_netcdf(&output, &report)
        .expect("write ocean threshold netcdf");

    assert_eq!(written.output, output);
    assert_eq!(written.sjx_points, 2);
    assert_eq!(written.ref_colnum, 3);

    let file = netcdf::open(&written.output).expect("open ocean threshold file");
    assert_eq!(file.dimension("sjx_points").unwrap().len(), 2);
    assert_eq!(file.dimension("ref_colnum").unwrap().len(), 3);
    assert_eq!(read_f64(&file, "sea_ratio"), vec![0.0, 0.5]);
    assert_eq!(read_f64(&file, "sst_m"), vec![0.0, 12.0]);
    assert_eq!(read_f64(&file, "sst_s"), vec![0.0, 2.0]);
    assert_eq!(read_i32(&file, "p_num"), vec![0, 2]);
    assert_eq!(read_i32_2d(&file, "ref_th_Ocn"), vec![0, 0, 0, 1, 0, 1]);
}

#[test]
fn atmos_threshold_writer_preserves_onelayer_pnum_and_ref_flags() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_getref_atmos_threshold_writer_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let output = root.join("threshold_calculate_atmos_NXP0009_03.nc4");

    let report = GetRefAtmosThresholdReport {
        ref_colnum: 1,
        column_names: vec!["typhoon_m".into()],
        ref_th: vec![vec![0, 0], vec![0, 0], vec![0, 1]],
        ref_sjx: vec![0, 0, 1],
        onelayer_reports: vec![GetRefMeanStd2DReport {
            ref_colnum: 1,
            ref_th: vec![vec![0; 2]; 3],
            ref_sjx: vec![0; 3],
            p_num: vec![0, 0, 2],
            mean: vec![0.0, 0.0, 9.0],
            stddev: None,
        }],
        last_p_num: Some(vec![0, 0, 2]),
    };

    let written = write_getref_atmos_threshold_netcdf(&output, &report)
        .expect("write atmos threshold netcdf");

    assert_eq!(written.output, output);
    assert_eq!(written.sjx_points, 2);
    assert_eq!(written.ref_colnum, 1);

    let file = netcdf::open(&written.output).expect("open atmos threshold file");
    assert_eq!(file.dimension("sjx_points").unwrap().len(), 2);
    assert_eq!(file.dimension("ref_colnum").unwrap().len(), 1);
    assert_eq!(read_f64(&file, "typhoon_m"), vec![0.0, 9.0]);
    assert_eq!(read_i32(&file, "p_num"), vec![0, 2]);
    assert_eq!(read_i32_2d(&file, "ref_th_Atmos"), vec![0, 1]);
}

fn read_i32(file: &netcdf::File, name: &str) -> Vec<i32> {
    file.variable(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
        .get_values::<i32, _>(..)
        .expect("read i32 values")
}

fn read_i32_2d(file: &netcdf::File, name: &str) -> Vec<i32> {
    file.variable(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
        .get_values::<i32, _>((.., ..))
        .expect("read i32 matrix")
}

fn read_f64(file: &netcdf::File, name: &str) -> Vec<f64> {
    file.variable(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
        .get_values::<f64, _>(..)
        .expect("read f64 values")
}
