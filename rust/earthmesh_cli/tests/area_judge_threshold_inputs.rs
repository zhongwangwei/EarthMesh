use earthmesh_cli::{
    area_judge_threshold_inputs::read_area_judge_threshold_inputs_one_based,
    area_judge_types::AreaJudgeThresholdReadConfig,
};
use earthmesh_mesh::AreaJudgeSourceBounds;
use std::path::{Path, PathBuf};

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("create temp root");
    path
}

fn one_based_landtypes(nx: usize, ny: usize) -> Vec<Vec<i32>> {
    let mut values = vec![vec![0; ny + 1]; nx + 1];
    for lon in 1..=nx {
        for lat in 1..=ny {
            values[lon][lat] = (lon as i32) * 10 + lat as i32;
        }
    }
    values
}

fn write_2d_threshold(path: &Path, var_name: &str, nx: usize, ny: usize, base: f64) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create threshold file");
    file.add_dimension("lon", nx).expect("lon dim");
    file.add_dimension("lat", ny).expect("lat dim");
    let mut var = file
        .add_variable::<f64>(var_name, &["lon", "lat"])
        .expect("threshold variable");
    let values = (0..nx)
        .flat_map(|lon| {
            (0..ny).map(move |lat| base + (lon as f64 + 1.0) * 100.0 + lat as f64 + 1.0)
        })
        .collect::<Vec<_>>();
    var.put_values(&values, (.., ..))
        .expect("write threshold variable");
}

fn write_2layer_threshold(path: &Path, var_name: &str, nx: usize, ny: usize, base: f64) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create threshold file");
    file.add_dimension("lon", nx).expect("lon dim");
    file.add_dimension("lat", ny).expect("lat dim");
    for (layer_index, suffix) in ["l1", "l2"].iter().enumerate() {
        let mut var = file
            .add_variable::<f64>(&format!("{var_name}_{suffix}"), &["lon", "lat"])
            .expect("threshold layer variable");
        let values = (0..nx)
            .flat_map(|lon| {
                (0..ny).map(move |lat| {
                    base + (layer_index as f64 + 1.0) * 1000.0
                        + (lon as f64 + 1.0) * 100.0
                        + lat as f64
                        + 1.0
                })
            })
            .collect::<Vec<_>>();
        var.put_values(&values, (.., ..))
            .expect("write threshold variable");
    }
}

#[test]
fn threshold_inputs_follow_area_judge_bounds_and_mesh_type_dispatch() {
    let root = temp_root("area_judge_threshold_inputs");
    write_2d_threshold(&root.join("lai.nc"), "lai", 5, 5, 0.0);
    write_2d_threshold(&root.join("dem.nc"), "topo", 5, 5, 40_000.0);
    write_2d_threshold(&root.join("slope_max.nc"), "slope_max", 5, 5, 50_000.0);
    write_2layer_threshold(&root.join("k_s.nc"), "k_s", 5, 5, 10_000.0);
    write_2d_threshold(&root.join("sst.nc"), "sst", 5, 5, 20_000.0);
    write_2d_threshold(&root.join("typhoon.nc"), "typhoon", 5, 5, 30_000.0);

    let report = read_area_judge_threshold_inputs_one_based(
        AreaJudgeThresholdReadConfig {
            threshold_dir: &root,
            mesh_type: "LOCmesh",
            refine_onelayer_lnd: &[true, false, false, false, false, true, true, false],
            refine_twolayer_lnd: &[
                false, true, false, false, false, false, false, false, false, false,
            ],
            refine_onelayer_ocn: &[true, false, false, false, false, false, false, false],
            refine_onelayer_atmos: &[true, false],
        },
        &one_based_landtypes(5, 5),
        AreaJudgeSourceBounds {
            minlon_source: 2,
            maxlon_source: 3,
            maxlat_source: 2,
            minlat_source: 4,
        },
    )
    .expect("read threshold inputs");

    assert_eq!(report.nlons_select, 2);
    assert_eq!(report.nlats_select, 3);
    assert_eq!(report.landtypes[1][1], 22);
    assert_eq!(report.landtypes[2][3], 34);

    assert_eq!(report.land_onelayer.len(), 4);
    assert_eq!(
        report.land_onelayer[0].as_ref().expect("lai loaded").name,
        "lai"
    );
    assert_eq!(
        report.land_onelayer[0].as_ref().unwrap().values[1][1],
        202.0
    );
    assert!(report.land_onelayer[1].is_none());
    assert_eq!(
        report.land_onelayer[2].as_ref().expect("dem loaded").name,
        "dem"
    );
    assert_eq!(
        report.land_onelayer[2].as_ref().unwrap().values[1][1],
        40_202.0
    );
    assert_eq!(
        report.land_onelayer[3]
            .as_ref()
            .expect("slope_max loaded")
            .name,
        "slope_max"
    );

    assert_eq!(report.land_twolayer.len(), 5);
    let k_s = report.land_twolayer[0]
        .as_ref()
        .expect("k_s loaded from second flag");
    assert_eq!(k_s.name, "k_s");
    assert_eq!(k_s.layers[0][1][1], 11_202.0);
    assert_eq!(k_s.layers[1][2][3], 12_304.0);

    assert_eq!(report.ocean_onelayer.len(), 4);
    assert_eq!(report.ocean_onelayer[0].as_ref().unwrap().name, "sst");
    assert_eq!(
        report.ocean_onelayer[0].as_ref().unwrap().values[2][3],
        20_304.0
    );

    assert_eq!(report.atmos_onelayer.len(), 1);
    assert_eq!(report.atmos_onelayer[0].as_ref().unwrap().name, "typhoon");
    assert_eq!(
        report.atmos_onelayer[0].as_ref().unwrap().values[1][2],
        30_203.0
    );
}

#[test]
fn threshold_inputs_skip_irrelevant_mesh_type_readers() {
    let root = temp_root("area_judge_threshold_inputs_land_only");
    write_2d_threshold(&root.join("lai.nc"), "lai", 5, 5, 0.0);

    let report = read_area_judge_threshold_inputs_one_based(
        AreaJudgeThresholdReadConfig {
            threshold_dir: &root,
            mesh_type: "landmesh",
            refine_onelayer_lnd: &[true, false, false, false, false, false, false, false],
            refine_twolayer_lnd: &[false; 10],
            refine_onelayer_ocn: &[true, false, false, false, false, false, false, false],
            refine_onelayer_atmos: &[true, false],
        },
        &one_based_landtypes(5, 5),
        AreaJudgeSourceBounds {
            minlon_source: 2,
            maxlon_source: 2,
            maxlat_source: 2,
            minlat_source: 2,
        },
    )
    .expect("read land-only threshold inputs");

    assert!(report.land_onelayer[0].is_some());
    assert!(report.ocean_onelayer.is_empty());
    assert!(report.atmos_onelayer.is_empty());
}
