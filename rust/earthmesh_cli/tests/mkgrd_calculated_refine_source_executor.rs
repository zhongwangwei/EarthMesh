use std::fs;
use std::path::Path;

use earthmesh_cli::{
    plan_mkgrd_refine_loop_io, read_contain_netcdf, write_unstructured_mesh_netcdf,
    GetRefAtmosThresholdConfig, GetRefLandBasicConfig, GetRefOceanThresholdConfig, LonLatPoint,
    MkgrdCalculatedRefineSourceExecutor, MkgrdCalculatedRefineSourceExecutorOptions,
    UnstructuredMesh,
};
use earthmesh_core::{EarthmeshConfig, RefineConfig};
use earthmesh_mesh::AreaJudgeSourceBounds;

fn mkgrd_config(base_dir: &str) -> EarthmeshConfig {
    mkgrd_config_for_mesh(base_dir, "landmesh")
}

fn mkgrd_config_for_mesh(base_dir: &str, mesh_type: &str) -> EarthmeshConfig {
    EarthmeshConfig::from_mkgrd_namelist(&format!(
        "&mkgrd\n  NL%EXPNME='case_cal_source'\n  NL%base_dir='{base_dir}'\n  NL%NXP=4\n  NL%mesh_type='{mesh_type}'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%refine=.true.\n/\n"
    ))
    .expect("parse mkgrd config")
}

fn refine_config() -> RefineConfig {
    RefineConfig::from_mkrefine_namelist(
        "&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=4,4,3\n  RL%max_transition_row=4,4,3\n  RL%mask_refine_cal_type='bbox'\n  RL%refine_num_landtypes=.true.\n/\n",
        "landmesh",
        "tri",
    )
    .expect("parse refine config")
}

fn loc_refine_config() -> RefineConfig {
    RefineConfig::from_mkrefine_namelist(
        "&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=4,4,3\n  RL%max_transition_row=4,4,3\n  RL%mask_refine_cal_type='bbox'\n  RL%refine_lai_m=.true.\n  RL%th_lai_m=5.0\n  RL%refine_sst_m=.true.\n  RL%th_sst_m=5.0\n  RL%refine_typhoon_m=.true.\n  RL%th_typhoon_m=9.0\n  RL%refine_sea_ratio=.true.\n  RL%th_sea_ratio=0.2,0.5\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n",
        "LOCmesh",
        "tri",
    )
    .expect("parse LOC refine config")
}

fn write_2d_threshold(path: &Path, var_name: &str, values: &[f64]) {
    let mut file = netcdf::create(path).expect("create threshold file");
    let lon = values.len() / 2;
    file.add_dimension("lon", lon).expect("lon dim");
    file.add_dimension("lat", 2).expect("lat dim");
    let mut var = file
        .add_variable::<f64>(var_name, &["lon", "lat"])
        .expect("threshold variable");
    var.put_values(values, (.., ..))
        .expect("write threshold values");
}

#[test]
fn calculated_refine_source_executor_runs_area_contain_and_integrated_getref_files() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mkgrd_calculated_refine_source_executor_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let base_dir = format!("{}/", root.display());
    let mkgrd = mkgrd_config(&base_dir);
    let refine = refine_config();
    let plan = plan_mkgrd_refine_loop_io(&mkgrd, &refine).expect("plan refine io");
    let step = &plan.steps[0];
    let source = &step.sources[0];

    write_unstructured_mesh_netcdf(
        &step.refine_loop_input_gridfile,
        &UnstructuredMesh {
            m_points: vec![LonLatPoint {
                lon: -178.5,
                lat: 88.5,
            }],
            w_points: vec![
                LonLatPoint {
                    lon: -179.5,
                    lat: 87.5,
                },
                LonLatPoint {
                    lon: -177.5,
                    lat: 87.5,
                },
                LonLatPoint {
                    lon: -179.5,
                    lat: 89.5,
                },
            ],
            m_to_w: vec![[1, 2, 3]],
            w_to_m: vec![vec![1], vec![1], vec![1]],
            n_w_to_m: vec![1, 1, 1],
        },
    )
    .expect("write current tri gridfile");

    let lon_i = vec![f64::NAN, -179.5, -178.5, -177.5, -176.5, -175.5, -174.5];
    let lat_i = vec![f64::NAN, 89.5, 88.5, 87.5, 86.5, 85.5, 84.5];
    let lon_vertex = vec![
        f64::NAN,
        -180.0,
        -179.0,
        -178.0,
        -177.0,
        -176.0,
        -175.0,
        -174.0,
    ];
    let lat_vertex = vec![f64::NAN, 90.0, 89.0, 88.0, 87.0, 86.0, 85.0, 84.0];
    let bounds = AreaJudgeSourceBounds {
        minlon_source: 1,
        maxlon_source: 2,
        maxlat_source: 1,
        minlat_source: 2,
    };
    let mut calculated_refine = vec![vec![0; lat_i.len()]; lon_i.len()];
    calculated_refine[1][1] = 1;
    calculated_refine[1][2] = 1;
    calculated_refine[2][1] = 1;
    calculated_refine[2][2] = 1;
    let mut seaorland = vec![vec![0; lat_i.len()]; lon_i.len()];
    seaorland[1][1] = 1;
    seaorland[1][2] = 1;
    seaorland[2][1] = 0;
    seaorland[2][2] = 1;
    let mut landtypes = vec![vec![0; lat_i.len()]; lon_i.len()];
    landtypes[1][1] = 1;
    landtypes[1][2] = 2;
    landtypes[2][2] = 3;

    let threshold_dir = plan.file_dir.join("threshold_inputs");
    let runner =
        MkgrdCalculatedRefineSourceExecutor::new(MkgrdCalculatedRefineSourceExecutorOptions {
            file_dir: &plan.file_dir,
            mesh_type: &plan.mesh_type,
            threshold_dir: &threshold_dir,
            calculated_refine: (&calculated_refine, bounds),
            seaorland: &seaorland,
            lon_vertex: &lon_vertex,
            lat_vertex: &lat_vertex,
            lon_i: &lon_i,
            lat_i: &lat_i,
            num_vertex: 0,
            landtypes_global: &landtypes,
            refine_onelayer_lnd: &[false, false, false, false],
            th_onelayer_lnd: &[0.0, 0.0, 0.0, 0.0],
            refine_twolayer_lnd: &[false; 10],
            th_twolayer_lnd: &[[0.0, 0.0]; 10],
            refine_onelayer_ocn: &[false, false, false, false],
            th_onelayer_ocn: &[0.0, 0.0, 0.0, 0.0],
            refine_onelayer_atmos: &[false, false, false, false],
            th_onelayer_atmos: &[0.0, 0.0, 0.0, 0.0],
            land_basic_config: GetRefLandBasicConfig {
                num_vertex: 1,
                maxlc: 99,
                refine_num_landtypes: true,
                th_num_landtypes: 0,
                refine_area_mainland: false,
                th_area_mainland: 0.0,
            },
            ocean_config: GetRefOceanThresholdConfig {
                num_vertex: 1,
                maxlc: 99,
                refine_sea_ratio: false,
                th_sea_ratio: [0.0, 1.0],
            },
            atmos_config: GetRefAtmosThresholdConfig {
                num_vertex: 1,
                maxlc: 99,
            },
        });

    let report = runner
        .run_source_branch_report(step, source)
        .expect("run calculated source branch");

    assert_eq!(report.area.refine_write.output, source.area_judge_output);
    assert_eq!(report.contain.output, source.contain_output);
    let land_write = report
        .getref
        .single_mesh
        .as_ref()
        .expect("single mesh getref")
        .writes
        .land
        .as_ref()
        .expect("land threshold write");
    assert_eq!(land_write.output, source.threshold_outputs[0]);
    assert!(source.area_judge_output.exists());
    assert!(source.contain_output.exists());
    assert!(source.threshold_outputs[0].exists());

    let file = netcdf::open(&source.threshold_outputs[0]).expect("open land threshold");
    assert_eq!(file.dimension("sjx_points").unwrap().len(), 2);
    assert_eq!(file.dimension("ref_colnum").unwrap().len(), 1);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn calculated_refine_source_executor_runs_locmesh_area_contain_and_integrated_getref_files() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mkgrd_calculated_refine_source_executor_loc_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let base_dir = format!("{}/", root.display());
    let mkgrd = mkgrd_config_for_mesh(&base_dir, "LOCmesh");
    let refine = loc_refine_config();
    let plan = plan_mkgrd_refine_loop_io(&mkgrd, &refine).expect("plan LOC refine io");
    let step = &plan.steps[0];
    let source = &step.sources[0];

    write_unstructured_mesh_netcdf(
        &step.refine_loop_input_gridfile,
        &UnstructuredMesh {
            m_points: vec![LonLatPoint {
                lon: -178.5,
                lat: 88.5,
            }],
            w_points: vec![
                LonLatPoint {
                    lon: -179.5,
                    lat: 87.5,
                },
                LonLatPoint {
                    lon: -177.5,
                    lat: 87.5,
                },
                LonLatPoint {
                    lon: -179.5,
                    lat: 89.5,
                },
            ],
            m_to_w: vec![[1, 2, 3]],
            w_to_m: vec![vec![1], vec![1], vec![1]],
            n_w_to_m: vec![1, 1, 1],
        },
    )
    .expect("write current LOC tri gridfile");

    let threshold_dir = plan.file_dir.join("threshold_inputs");
    fs::create_dir_all(&threshold_dir).expect("threshold input dir");
    write_2d_threshold(
        &threshold_dir.join("lai.nc"),
        "lai",
        &[10.0, 1.0, 10.0, 1.0],
    );
    write_2d_threshold(
        &threshold_dir.join("sst.nc"),
        "sst",
        &[1.0, 20.0, 1.0, 20.0],
    );
    write_2d_threshold(
        &threshold_dir.join("typhoon.nc"),
        "typhoon",
        &[1.0, 10.0, 10.0, 1.0],
    );

    let lon_i = vec![f64::NAN, -179.5, -178.5, -177.5, -176.5, -175.5, -174.5];
    let lat_i = vec![f64::NAN, 89.5, 88.5, 87.5, 86.5, 85.5, 84.5];
    let lon_vertex = vec![
        f64::NAN,
        -180.0,
        -179.0,
        -178.0,
        -177.0,
        -176.0,
        -175.0,
        -174.0,
    ];
    let lat_vertex = vec![f64::NAN, 90.0, 89.0, 88.0, 87.0, 86.0, 85.0, 84.0];
    let bounds = AreaJudgeSourceBounds {
        minlon_source: 1,
        maxlon_source: 2,
        maxlat_source: 1,
        minlat_source: 2,
    };
    let mut calculated_refine = vec![vec![0; lat_i.len()]; lon_i.len()];
    calculated_refine[1][1] = 1;
    calculated_refine[1][2] = 1;
    calculated_refine[2][1] = 1;
    calculated_refine[2][2] = 1;
    let mut seaorland = vec![vec![0; lat_i.len()]; lon_i.len()];
    seaorland[1][1] = 1;
    seaorland[1][2] = 0;
    seaorland[2][1] = 1;
    seaorland[2][2] = 0;
    let mut landtypes = vec![vec![0; lat_i.len()]; lon_i.len()];
    landtypes[1][1] = 1;
    landtypes[2][1] = 2;

    let runner =
        MkgrdCalculatedRefineSourceExecutor::new(MkgrdCalculatedRefineSourceExecutorOptions {
            file_dir: &plan.file_dir,
            mesh_type: &plan.mesh_type,
            threshold_dir: &threshold_dir,
            calculated_refine: (&calculated_refine, bounds),
            seaorland: &seaorland,
            lon_vertex: &lon_vertex,
            lat_vertex: &lat_vertex,
            lon_i: &lon_i,
            lat_i: &lat_i,
            num_vertex: 0,
            landtypes_global: &landtypes,
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
                maxlc: 99,
                refine_num_landtypes: true,
                th_num_landtypes: 1,
                refine_area_mainland: false,
                th_area_mainland: 0.0,
            },
            ocean_config: GetRefOceanThresholdConfig {
                num_vertex: 1,
                maxlc: 99,
                refine_sea_ratio: true,
                th_sea_ratio: [0.2, 0.5],
            },
            atmos_config: GetRefAtmosThresholdConfig {
                num_vertex: 1,
                maxlc: 99,
            },
        });

    let report = runner
        .run_source_branch_report(step, source)
        .expect("run LOC calculated source branch");

    assert_eq!(report.area.refine_write.output, source.area_judge_output);
    assert_eq!(report.contain.output, source.contain_output);
    assert!(report.contain.contained_source_pixels > 0);
    let loc = report.getref.loc_mesh.as_ref().expect("LOC getref report");
    assert_eq!(
        loc.writes.land.as_ref().unwrap().output,
        source.threshold_outputs[0]
    );
    assert_eq!(
        loc.writes.ocean.as_ref().unwrap().output,
        source.threshold_outputs[1]
    );
    assert_eq!(
        loc.writes.atmos.as_ref().unwrap().output,
        source.threshold_outputs[2]
    );
    let contain_payload = read_contain_netcdf(&source.contain_output).expect("read LOC contain");
    assert_eq!(contain_payload.ustr_id[1].len(), 3);
    assert_eq!(contain_payload.ustr_ii[0].len(), 3);
    assert!(source.threshold_outputs[0].exists());
    assert!(source.threshold_outputs[1].exists());
    assert!(source.threshold_outputs[2].exists());

    let _ = fs::remove_dir_all(&root);
}
