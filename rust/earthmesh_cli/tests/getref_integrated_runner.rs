use std::fs;
use std::path::{Path, PathBuf};

use earthmesh_cli::{
    run_getref_integrated_threshold_files_fortran_indexed, write_contain_netcdf, ContainMesh,
    GetRefAtmosThresholdConfig, GetRefIntegratedFileRunConfig, GetRefLandBasicConfig,
    GetRefOceanThresholdConfig,
};
use earthmesh_mesh::AreaJudgeSourceBounds;

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp root");
    path
}

fn write_lai_threshold(path: &Path) {
    write_2d_threshold(path, "lai", &[10.0, 1.0, 10.0, 1.0]);
}

fn write_2d_threshold(path: &Path, var_name: &str, values: &[f64]) {
    let mut file = netcdf::create(path).expect("create lai threshold file");
    let lon = values.len() / 2;
    file.add_dimension("lon", lon).expect("lon dim");
    file.add_dimension("lat", 2).expect("lat dim");
    let mut var = file
        .add_variable::<f64>(var_name, &["lon", "lat"])
        .expect("threshold variable");
    var.put_values(values, (.., ..))
        .expect("write threshold values");
}

fn one_based_landtypes() -> Vec<Vec<i32>> {
    vec![vec![0, 0, 0], vec![0, 1, 1], vec![0, 1, 1]]
}

fn read_i32_2d(file: &netcdf::File, name: &str) -> Vec<i32> {
    file.variable(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
        .get_values::<i32, _>((.., ..))
        .expect("read i32 matrix")
}

#[test]
fn integrated_runner_reads_area_judge_threshold_files_and_writes_land_getref_output() {
    let root = temp_root("getref_integrated_runner_land");
    let threshold_dir = root.join("threshold_inputs");
    fs::create_dir_all(&threshold_dir).expect("threshold input dir");
    write_lai_threshold(&threshold_dir.join("lai.nc"));

    let contain_file = root.join("contain_landmesh_refine_cal_NXP0009_03_tri.nc4");
    let threshold_file = root.join("threshold/threshold_calculate_land_NXP0009_03.nc4");
    write_contain_netcdf(
        &contain_file,
        &ContainMesh {
            ustr_id: vec![vec![0, 0], vec![2, 1], vec![2, 3]],
            ustr_ii: vec![vec![1, 1], vec![2, 1], vec![1, 2], vec![2, 2]],
            is_in_area_ustr: vec![0, 1, 1],
        },
    )
    .expect("write contain fixture");

    let report =
        run_getref_integrated_threshold_files_fortran_indexed(GetRefIntegratedFileRunConfig {
            mesh_type: "landmesh",
            threshold_dir: &threshold_dir,
            contain_file: &contain_file,
            land_threshold_output: Some(&threshold_file),
            ocean_threshold_output: None,
            atmos_threshold_output: None,
            landtypes_global: &one_based_landtypes(),
            threshold_bounds: AreaJudgeSourceBounds {
                minlon_source: 1,
                maxlon_source: 2,
                maxlat_source: 1,
                minlat_source: 2,
            },
            is_in_refine_sjx: &[0, 0, 1, 1],
            refine_onelayer_lnd: &[true, false, false, false],
            th_onelayer_lnd: &[5.0, 0.0, 0.0, 0.0],
            refine_twolayer_lnd: &[false; 10],
            th_twolayer_lnd: &[[0.0, 0.0]; 10],
            refine_onelayer_ocn: &[false; 8],
            th_onelayer_ocn: &[0.0; 8],
            refine_onelayer_atmos: &[false; 2],
            th_onelayer_atmos: &[0.0; 2],
            land_basic_config: GetRefLandBasicConfig {
                num_vertex: 1,
                maxlc: 9,
                refine_num_landtypes: false,
                th_num_landtypes: 0,
                refine_area_mainland: false,
                th_area_mainland: 0.0,
            },
            ocean_config: GetRefOceanThresholdConfig {
                num_vertex: 1,
                maxlc: 9,
                refine_sea_ratio: false,
                th_sea_ratio: [0.0, 0.0],
            },
            atmos_config: GetRefAtmosThresholdConfig {
                num_vertex: 1,
                maxlc: 9,
            },
        })
        .expect("run integrated GetRef landmesh runner");

    assert_eq!(
        report.threshold_inputs.land_onelayer[0]
            .as_ref()
            .unwrap()
            .name,
        "lai"
    );
    let single = report.single_mesh.as_ref().expect("single-mesh report");
    assert!(report.loc_mesh.is_none());
    assert_eq!(single.threshold.aggregate.ref_sjx, vec![0, 0, 1, 0]);
    assert_eq!(single.writes.land.as_ref().unwrap().output, threshold_file);

    let file = netcdf::open(&threshold_file).expect("open written land threshold");
    assert_eq!(read_i32_2d(&file, "ref_th_Lnd"), vec![0, 1, 0]);
}

#[test]
fn integrated_runner_dispatches_locmesh_threshold_files_to_all_component_outputs() {
    let root = temp_root("getref_integrated_runner_loc");
    let threshold_dir = root.join("threshold_inputs");
    fs::create_dir_all(&threshold_dir).expect("threshold input dir");
    write_2d_threshold(
        &threshold_dir.join("lai.nc"),
        "lai",
        &[10.0, 1.0, 10.0, 1.0, 10.0, 1.0],
    );
    write_2d_threshold(
        &threshold_dir.join("sst.nc"),
        "sst",
        &[1.0, 20.0, 1.0, 20.0, 20.0, 1.0],
    );
    write_2d_threshold(
        &threshold_dir.join("typhoon.nc"),
        "typhoon",
        &[1.0, 8.0, 1.0, 10.0, 10.0, 1.0],
    );

    let contain_file = root.join("contain_LOCmesh_refine_cal_NXP0009_03_tri.nc4");
    let land_output = root.join("threshold/threshold_calculate_land_NXP0009_03.nc4");
    let ocean_output = root.join("threshold/threshold_calculate_ocean_NXP0009_03.nc4");
    let atmos_output = root.join("threshold/threshold_calculate_atmos_NXP0009_03.nc4");
    write_contain_netcdf(
        &contain_file,
        &ContainMesh {
            ustr_id: vec![vec![0, 0, 0], vec![3, 1, 0], vec![2, 4, 0]],
            ustr_ii: vec![
                vec![1, 1, 1],
                vec![1, 2, 0],
                vec![2, 1, 1],
                vec![2, 2, 0],
                vec![3, 1, 0],
            ],
            is_in_area_ustr: vec![0, 1, 1],
        },
    )
    .expect("write LOC contain fixture");

    let report =
        run_getref_integrated_threshold_files_fortran_indexed(GetRefIntegratedFileRunConfig {
            mesh_type: "LOCmesh",
            threshold_dir: &threshold_dir,
            contain_file: &contain_file,
            land_threshold_output: Some(&land_output),
            ocean_threshold_output: Some(&ocean_output),
            atmos_threshold_output: Some(&atmos_output),
            landtypes_global: &[vec![0, 0, 0], vec![0, 1, 1], vec![0, 2, 1], vec![0, 1, 1]],
            threshold_bounds: AreaJudgeSourceBounds {
                minlon_source: 1,
                maxlon_source: 3,
                maxlat_source: 1,
                minlat_source: 2,
            },
            is_in_refine_sjx: &[0, 0, 1, 1],
            refine_onelayer_lnd: &[true, false, false, false],
            th_onelayer_lnd: &[5.0, 0.0, 0.0, 0.0],
            refine_twolayer_lnd: &[false; 10],
            th_twolayer_lnd: &[[0.0, 0.0]; 10],
            refine_onelayer_ocn: &[true, false, false, false, false, false, false, false],
            th_onelayer_ocn: &[5.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            refine_onelayer_atmos: &[true, false],
            th_onelayer_atmos: &[9.0, 0.0],
            land_basic_config: GetRefLandBasicConfig {
                num_vertex: 1,
                maxlc: 9,
                refine_num_landtypes: true,
                th_num_landtypes: 1,
                refine_area_mainland: false,
                th_area_mainland: 0.0,
            },
            ocean_config: GetRefOceanThresholdConfig {
                num_vertex: 1,
                maxlc: 9,
                refine_sea_ratio: true,
                th_sea_ratio: [0.2, 0.5],
            },
            atmos_config: GetRefAtmosThresholdConfig {
                num_vertex: 1,
                maxlc: 9,
            },
        })
        .expect("run integrated GetRef LOCmesh runner");

    assert!(report.single_mesh.is_none());
    let loc = report.loc_mesh.as_ref().expect("LOC report");
    assert!(report.threshold_inputs.land_onelayer[0].is_some());
    assert!(report.threshold_inputs.ocean_onelayer[0].is_some());
    assert!(report.threshold_inputs.atmos_onelayer[0].is_some());
    assert_eq!(loc.threshold.aggregate.ref_colnum, 5);
    assert_eq!(loc.writes.land.as_ref().unwrap().output, land_output);
    assert_eq!(loc.writes.ocean.as_ref().unwrap().output, ocean_output);
    assert_eq!(loc.writes.atmos.as_ref().unwrap().output, atmos_output);
    assert!(netcdf::open(&land_output).is_ok());
    assert!(netcdf::open(&ocean_output).is_ok());
    assert!(netcdf::open(&atmos_output).is_ok());
}
