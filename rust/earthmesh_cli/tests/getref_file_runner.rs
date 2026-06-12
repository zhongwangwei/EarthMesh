use std::fs;
use std::path::PathBuf;

use earthmesh_cli::{
    run_getref_single_mesh_threshold_files_fortran_indexed, write_contain_netcdf, ContainMesh,
    GetRefAtmosThresholdConfig, GetRefLandBasicConfig, GetRefOceanThresholdConfig,
    GetRefSingleMeshFileRunConfig,
};

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp root");
    path
}

#[test]
fn getref_file_runner_reads_contain_and_writes_land_threshold_output() {
    let root = temp_root("getref_file_runner_land");
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
    let mut landtypes = vec![vec![1; 3]; 3];
    landtypes[1][1] = 1;
    landtypes[2][1] = 2;
    landtypes[1][2] = 1;
    landtypes[2][2] = 1;

    let report =
        run_getref_single_mesh_threshold_files_fortran_indexed(GetRefSingleMeshFileRunConfig {
            mesh_type: "landmesh",
            contain_file: &contain_file,
            land_threshold_output: Some(&threshold_file),
            ocean_threshold_output: None,
            atmos_threshold_output: None,
            is_in_refine_sjx: &[0, 0, 1, 1],
            landtypes: &landtypes,
            land_basic_config: GetRefLandBasicConfig {
                num_vertex: 1,
                maxlc: 9,
                refine_num_landtypes: true,
                th_num_landtypes: 1,
                refine_area_mainland: false,
                th_area_mainland: 0.0,
            },
            land_onelayer_inputs: &[],
            land_twolayer_inputs: &[],
            ocean_config: GetRefOceanThresholdConfig {
                num_vertex: 1,
                maxlc: 9,
                refine_sea_ratio: false,
                th_sea_ratio: [0.0, 0.0],
            },
            ocean_onelayer_inputs: &[],
            atmos_config: GetRefAtmosThresholdConfig {
                num_vertex: 1,
                maxlc: 9,
            },
            atmos_onelayer_inputs: &[],
        })
        .expect("run GetRef landmesh file runner");

    assert_eq!(report.threshold.land.as_ref().unwrap().ref_colnum, 1);
    assert!(report.threshold.ocean.is_none());
    assert!(report.threshold.atmos.is_none());
    assert_eq!(report.threshold.aggregate.ref_sjx, vec![0, 0, 1, 0]);
    assert_eq!(report.writes.land.as_ref().unwrap().output, threshold_file);
    assert!(report.writes.ocean.is_none());
    assert!(report.writes.atmos.is_none());

    let file = netcdf::open(&threshold_file).expect("open written land threshold");
    assert_eq!(read_i32(&file, "n_landtypes"), vec![0, 2, 1]);
    assert_eq!(read_i32_2d(&file, "ref_th_Lnd"), vec![0, 1, 0]);
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

#[test]
fn getref_file_runner_reads_loc_contain_and_writes_component_threshold_outputs() {
    let root = temp_root("getref_file_runner_loc");
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
    let mut landtypes = vec![vec![1; 3]; 4];
    landtypes[1][1] = 1;
    landtypes[2][1] = 2;
    let mut air = vec![vec![0.0; 3]; 4];
    air[1][1] = 1.0;
    air[1][2] = 8.0;
    air[2][1] = 1.0;
    air[2][2] = 10.0;
    air[3][1] = 10.0;

    let report = earthmesh_cli::run_getref_loc_mesh_threshold_files_fortran_indexed(
        earthmesh_cli::GetRefLocMeshFileRunConfig {
            contain_file: &contain_file,
            land_threshold_output: Some(&land_output),
            ocean_threshold_output: Some(&ocean_output),
            atmos_threshold_output: Some(&atmos_output),
            is_in_refine_sjx: &[0, 0, 1, 1],
            landtypes: &landtypes,
            land_basic_config: GetRefLandBasicConfig {
                num_vertex: 1,
                maxlc: 9,
                refine_num_landtypes: true,
                th_num_landtypes: 1,
                refine_area_mainland: false,
                th_area_mainland: 0.0,
            },
            land_onelayer_inputs: &[],
            land_twolayer_inputs: &[],
            ocean_config: GetRefOceanThresholdConfig {
                num_vertex: 1,
                maxlc: 9,
                refine_sea_ratio: true,
                th_sea_ratio: [0.2, 0.5],
            },
            ocean_onelayer_inputs: &[],
            atmos_config: GetRefAtmosThresholdConfig {
                num_vertex: 1,
                maxlc: 9,
            },
            atmos_onelayer_inputs: &[earthmesh_cli::GetRefOneLayerThresholdInput {
                name: "air",
                values: &air,
                mean_threshold: Some(9.0),
                std_threshold: None,
            }],
        },
    )
    .expect("run GetRef LOCmesh file runner");

    assert_eq!(report.threshold.aggregate.ref_colnum, 3);
    assert_eq!(report.threshold.aggregate.ref_sjx, vec![0, 0, 1, 1]);
    assert_eq!(report.writes.land.as_ref().unwrap().output, land_output);
    assert_eq!(report.writes.ocean.as_ref().unwrap().output, ocean_output);
    assert_eq!(report.writes.atmos.as_ref().unwrap().output, atmos_output);
    assert!(netcdf::open(&land_output).is_ok());
    assert!(netcdf::open(&ocean_output).is_ok());
    assert!(netcdf::open(&atmos_output).is_ok());
}
