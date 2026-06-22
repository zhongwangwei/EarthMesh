use std::fs;

use earthmesh_cli::{
    data_read_onelayer_fortran_indexed, data_read_twolayer_fortran_indexed,
    threshold_read_lnd_fortran_indexed, threshold_read_ocn_fortran_indexed, ThresholdReadLndConfig,
    ThresholdReadOcnConfig,
};
use earthmesh_mesh::AreaJudgeSourceBounds;

#[test]
fn data_preprocess_onelayer_and_twolayer_read_fortran_windows() {
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

    let one = data_read_onelayer_fortran_indexed(root.join("lai.nc"), "lai", bounds)
        .expect("read onelayer window");
    assert_eq!(one.name, "lai");
    assert_eq!(one.values[1][1], 3.0);
    assert_eq!(one.values[2][2], 6.0);

    let two = data_read_twolayer_fortran_indexed(root.join("k_s.nc"), "k_s", bounds)
        .expect("read twolayer window");
    assert_eq!(two.name, "k_s");
    assert_eq!(two.layers[0][1][1], 30.0);
    assert_eq!(two.layers[1][2][2], 61.0);

    let _ = fs::remove_dir_all(&root);
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

    let lnd = threshold_read_lnd_fortran_indexed(ThresholdReadLndConfig {
        threshold_dir: &root,
        refine_onelayer_lnd: &[true, false, false, false],
        refine_twolayer_lnd: &[
            false, true, false, false, false, false, false, false, false, false,
        ],
        bounds,
    })
    .expect("read land thresholds");
    assert!(lnd.onelayer[0].is_some());
    assert!(lnd.onelayer[1].is_none());
    assert!(lnd.twolayer[0].is_some());
    assert!(lnd.twolayer[1].is_none());

    let ocn = threshold_read_ocn_fortran_indexed(ThresholdReadOcnConfig {
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
    let mut file = netcdf::create(path).expect("create 2d threshold file");
    file.add_dimension("lon", 3).expect("lon dim");
    file.add_dimension("lat", 2).expect("lat dim");
    let mut variable = file
        .add_variable::<f64>(var, &["lon", "lat"])
        .expect("add 2d var");
    variable
        .put_values(values, (.., ..))
        .expect("write 2d values");
}

fn write_2layer_file(path: &std::path::Path, stem: &str, layer1: &[f64], layer2: &[f64]) {
    let mut file = netcdf::create(path).expect("create 2layer threshold file");
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
