use std::fs;

use earthmesh_cli::{
    write_getref_land_threshold_netcdf, GetRefLandThresholdReport, GetRefMeanStd2DReport,
    GetRefMeanStd3DReport,
};

#[test]
fn land_threshold_writer_preserves_fortran_dimensions_column_order_and_values() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_getref_land_threshold_writer_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let output = root.join("threshold_calculate_land_NXP0009_03.nc4");

    let report = GetRefLandThresholdReport {
        ref_colnum: 5,
        column_names: vec![
            "n_landtypes".to_string(),
            "f_mainarea".to_string(),
            "lai_m".to_string(),
            "lai_s".to_string(),
            "k_s_m".to_string(),
        ],
        ref_th_land: vec![
            vec![0, 0, 0, 0, 0, 0],
            vec![0, 0, 0, 0, 0, 0],
            vec![0, 1, 1, 0, 1, 1],
            vec![0, 0, 0, 1, 1, 0],
        ],
        ref_sjx: vec![0, 0, 1, 1],
        n_landtypes: Some(vec![0, 0, 2, 1]),
        f_mainarea: Some(vec![0.0, 0.0, 0.5, 1.0]),
        onelayer_reports: vec![GetRefMeanStd2DReport {
            ref_colnum: 2,
            ref_th: vec![vec![0; 3]; 4],
            ref_sjx: vec![0; 4],
            p_num: vec![0, 0, 2, 3],
            mean: vec![0.0, 0.0, 3.0, 8.0],
            stddev: Some(vec![0.0, 0.0, 1.0, 2.0]),
        }],
        twolayer_reports: vec![GetRefMeanStd3DReport {
            ref_colnum: 1,
            ref_th: vec![vec![0; 2]; 4],
            ref_sjx: vec![0; 4],
            p_num: vec![0, 0, 2, 3],
            mean: vec![[0.0, 0.0], [0.0, 0.0], [30.0, 4.0], [10.0, 2.0]],
            stddev: None,
        }],
        last_p_num: Some(vec![0, 0, 2, 3]),
    };

    let written =
        write_getref_land_threshold_netcdf(&output, &report).expect("write land threshold netcdf");

    assert_eq!(written.output, output);
    assert_eq!(written.sjx_points, 3);
    assert_eq!(written.dima, 2);
    assert_eq!(written.ref_colnum, 5);

    let file = netcdf::open(&written.output).expect("open land threshold file");
    assert_eq!(file.dimension("sjx_points").unwrap().len(), 3);
    assert_eq!(file.dimension("dima").unwrap().len(), 2);
    assert_eq!(file.dimension("ref_colnum").unwrap().len(), 5);

    assert_eq!(read_i32(&file, "n_landtypes"), vec![0, 2, 1]);
    assert_eq!(read_f64(&file, "f_mainarea"), vec![0.0, 0.5, 1.0]);
    assert_eq!(read_f64(&file, "lai_m"), vec![0.0, 3.0, 8.0]);
    assert_eq!(read_f64(&file, "lai_s"), vec![0.0, 1.0, 2.0]);
    assert_eq!(
        read_f64_2d(&file, "k_s_m"),
        vec![0.0, 0.0, 30.0, 4.0, 10.0, 2.0]
    );
    assert_eq!(read_i32(&file, "p_num"), vec![0, 2, 3]);
    assert_eq!(
        read_i32_2d(&file, "ref_th_Lnd"),
        vec![0, 0, 0, 0, 0, 1, 1, 0, 1, 1, 0, 0, 1, 1, 0]
    );
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

fn read_f64_2d(file: &netcdf::File, name: &str) -> Vec<f64> {
    file.variable(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
        .get_values::<f64, _>((.., ..))
        .expect("read f64 matrix")
}
