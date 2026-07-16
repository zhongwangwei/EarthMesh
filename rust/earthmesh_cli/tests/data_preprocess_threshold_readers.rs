use std::fs;

use earthmesh_cli::{
    area_judge_threshold_inputs::data_read_onelayer_one_based,
    area_judge_threshold_inputs::data_read_twolayer_one_based,
    area_judge_threshold_inputs::threshold_read_lnd_one_based,
    area_judge_threshold_inputs::threshold_read_ocn_one_based,
    area_judge_types::ThresholdReadLndConfig, area_judge_types::ThresholdReadOcnConfig,
};
use earthmesh_mesh::AreaJudgeSourceBounds;

#[test]
fn data_preprocess_onelayer_and_twolayer_read_canonical_windows() {
    let root = temp_root("earthmesh_cli_data_preprocess_threshold_windows");
    write_2d_file(&root.join("lai.nc"), "lai", &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    write_2layer_file(
        &root.join("k_s.nc"),
        "k_s",
        &[10.0, 20.0, 30.0, 40.0, 50.0, 60.0],
        &[11.0, 21.0, 31.0, 41.0, 51.0, 61.0],
    );
    let bounds = AreaJudgeSourceBounds {
        minlon_source: 2,
        maxlon_source: 3,
        maxlat_source: 1,
        minlat_source: 2,
    };

    let one = data_read_onelayer_one_based(root.join("lai.nc"), "lai", bounds)
        .expect("read onelayer window");
    assert_eq!(one.name, "lai");
    assert_eq!(one.values[1][1], 3.0);
    assert_eq!(one.values[2][2], 6.0);

    let two = data_read_twolayer_one_based(root.join("k_s.nc"), "k_s", bounds)
        .expect("read twolayer window");
    assert_eq!(two.name, "k_s");
    assert_eq!(two.layers[0][1][1], 30.0);
    assert_eq!(two.layers[1][2][2], 61.0);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn data_preprocess_onelayer_reads_lat_lon_source_data_order() {
    let root = temp_root("earthmesh_cli_data_preprocess_threshold_lat_lon");
    write_2d_lat_lon_file(&root.join("lai.nc"), "lai", &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    let bounds = AreaJudgeSourceBounds {
        minlon_source: 2,
        maxlon_source: 3,
        maxlat_source: 1,
        minlat_source: 2,
    };

    let one = data_read_onelayer_one_based(root.join("lai.nc"), "lai", bounds)
        .expect("read lat-lon onelayer window");
    assert_eq!(one.values[1][1], 2.0);
    assert_eq!(one.values[2][2], 6.0);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn data_preprocess_onelayer_normalizes_ascending_latitude_coordinates() {
    let root = temp_root("earthmesh_cli_data_preprocess_threshold_ascending_lat");
    let path = root.join("lai.nc");
    let mut file = earthmesh_cli::create_netcdf_quiet(&path).expect("create threshold file");
    file.add_dimension("longitude", 2).unwrap();
    file.add_dimension("latitude", 3).unwrap();
    file.add_variable::<f64>("latitude", &["latitude"])
        .unwrap()
        .put_values(&[-90.0, 0.0, 90.0], ..)
        .unwrap();
    file.add_variable::<f64>("lai", &["longitude", "latitude"])
        .unwrap()
        .put_values(&[10.0, 11.0, 12.0, 20.0, 21.0, 22.0], (.., ..))
        .unwrap();
    drop(file);

    let values = data_read_onelayer_one_based(
        &path,
        "lai",
        AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: 2,
            maxlat_source: 1,
            minlat_source: 3,
        },
    )
    .expect("ascending latitude is normalized to Canonical north-to-south order");

    assert_eq!(&values.values[1][1..=3], &[12.0, 11.0, 10.0]);
    assert_eq!(&values.values[2][1..=3], &[22.0, 21.0, 20.0]);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn data_preprocess_onelayer_rejects_fill_values() {
    let root = temp_root("earthmesh_cli_data_preprocess_threshold_fill");
    let path = root.join("lai.nc");
    let mut file = earthmesh_cli::create_netcdf_quiet(&path).expect("create threshold file");
    file.add_dimension("lon", 2).unwrap();
    file.add_dimension("lat", 2).unwrap();
    let mut variable = file.add_variable::<f64>("lai", &["lon", "lat"]).unwrap();
    variable.put_attribute("_FillValue", -9999.0).unwrap();
    variable
        .put_values(&[1.0, -9999.0, 3.0, 4.0], (.., ..))
        .unwrap();
    drop(file);

    let error = data_read_onelayer_one_based(
        &path,
        "lai",
        AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: 2,
            maxlat_source: 1,
            minlat_source: 2,
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("missing/non-finite"), "{error}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn data_preprocess_onelayer_rejects_reversed_bounds_before_arithmetic() {
    let error = data_read_onelayer_one_based(
        "/file/is/not/opened.nc",
        "lai",
        AreaJudgeSourceBounds {
            minlon_source: 3,
            maxlon_source: 2,
            maxlat_source: 1,
            minlat_source: 2,
        },
    )
    .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert!(error
        .to_string()
        .contains("invalid one-based threshold bounds"));
}

#[test]
fn threshold_read_lnd_and_ocn_follow_enabled_flag_pairs() {
    let root = temp_root("earthmesh_cli_data_preprocess_threshold_groups");
    write_2d_file(&root.join("lai.nc"), "lai", &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    write_2d_file(
        &root.join("slope_avg.nc"),
        "slope_avg",
        &[7.0, 8.0, 9.0, 10.0, 11.0, 12.0],
    );
    write_2d_file(
        &root.join("dem.nc"),
        "topo",
        &[40.0, 41.0, 42.0, 43.0, 44.0, 45.0],
    );
    write_2d_file(
        &root.join("sst.nc"),
        "sst",
        &[13.0, 14.0, 15.0, 16.0, 17.0, 18.0],
    );
    write_2layer_file(
        &root.join("k_s.nc"),
        "k_s",
        &[20.0, 21.0, 22.0, 23.0, 24.0, 25.0],
        &[30.0, 31.0, 32.0, 33.0, 34.0, 35.0],
    );
    let bounds = AreaJudgeSourceBounds {
        minlon_source: 1,
        maxlon_source: 2,
        maxlat_source: 1,
        minlat_source: 2,
    };

    let lnd = threshold_read_lnd_one_based(ThresholdReadLndConfig {
        threshold_dir: &root,
        refine_onelayer_lnd: &[true, false, false, false, true, false, false, false],
        refine_twolayer_lnd: &[
            false, true, false, false, false, false, false, false, false, false,
        ],
        bounds,
    })
    .expect("read land thresholds");
    assert!(lnd.onelayer[0].is_some());
    assert!(lnd.onelayer[1].is_none());
    assert_eq!(lnd.onelayer[2].as_ref().unwrap().name, "dem");
    assert_eq!(lnd.onelayer[2].as_ref().unwrap().values[1][1], 40.0);
    assert!(lnd.twolayer[0].is_some());
    assert!(lnd.twolayer[1].is_none());

    let ocn = threshold_read_ocn_one_based(ThresholdReadOcnConfig {
        threshold_dir: &root,
        refine_onelayer_ocn: &[true, false, false, false, false, false, false, false],
        bounds,
    })
    .expect("read ocean thresholds");
    assert!(ocn.onelayer[0].is_some());
    assert_eq!(ocn.onelayer[0].as_ref().unwrap().values[2][2], 16.0);

    let _ = fs::remove_dir_all(&root);
}

fn write_2d_file(path: &std::path::Path, var: &str, values: &[f64]) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create 2d threshold file");
    file.add_dimension("lon", 3).expect("lon dim");
    file.add_dimension("lat", 2).expect("lat dim");
    let mut variable = file
        .add_variable::<f64>(var, &["lon", "lat"])
        .expect("add 2d var");
    variable
        .put_values(values, (.., ..))
        .expect("write 2d values");
}

fn write_2d_lat_lon_file(path: &std::path::Path, var: &str, values: &[f64]) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create lat-lon threshold file");
    file.add_dimension("latitude", 2).expect("latitude dim");
    file.add_dimension("longitude", 3).expect("longitude dim");
    let mut variable = file
        .add_variable::<f64>(var, &["latitude", "longitude"])
        .expect("add lat-lon var");
    variable
        .put_values(values, (.., ..))
        .expect("write lat-lon values");
}

fn write_2layer_file(path: &std::path::Path, stem: &str, layer1: &[f64], layer2: &[f64]) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create 2layer threshold file");
    file.add_dimension("lon", 3).expect("lon dim");
    file.add_dimension("lat", 2).expect("lat dim");
    for (suffix, values) in [("l1", layer1), ("l2", layer2)] {
        let name = format!("{stem}_{suffix}");
        let mut variable = file
            .add_variable::<f64>(&name, &["lon", "lat"])
            .expect("add layer var");
        variable
            .put_values(values, (.., ..))
            .expect("write layer values");
    }
}

fn temp_root(label: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("{label}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    root
}
