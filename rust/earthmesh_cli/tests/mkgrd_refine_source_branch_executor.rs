use std::fs;

use earthmesh_cli::{
    plan_mkgrd_refine_loop_io, write_bbox_mask_netcdf, write_unstructured_mesh_netcdf, BBoxMask,
    BBoxPoint, GetRefAtmosThresholdConfig, GetRefLandBasicConfig, GetRefOceanThresholdConfig,
    LonLatPoint, MkgrdCalculatedRefineSourceExecutorOptions, MkgrdRefineSource,
    MkgrdRefineSourceBranchExecutor, MkgrdRefineSourceBranchExecutorOptions,
    MkgrdRefineSourceBranchReport, MkgrdSpecifiedRefineSourceExecutorOptions, UnstructuredMesh,
};
use earthmesh_core::{EarthmeshConfig, RefineConfig};
use earthmesh_mesh::AreaJudgeSourceBounds;

fn mkgrd_config(base_dir: &str) -> EarthmeshConfig {
    EarthmeshConfig::from_mkgrd_namelist(&format!(
        "&mkgrd\n  NL%EXPNME='case_source_dispatch'\n  NL%base_dir='{base_dir}'\n  NL%NXP=4\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%refine=.true.\n/\n"
    ))
    .expect("parse mkgrd config")
}

fn refine_config() -> RefineConfig {
    RefineConfig::from_mkrefine_namelist(
        "&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_cal_type='bbox'\n  RL%refine_num_landtypes=.true.\n/\n",
        "landmesh",
        "tri",
    )
    .expect("parse refine config")
}

#[test]
fn source_branch_executor_dispatches_calculated_then_specified_sources() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mkgrd_refine_source_branch_executor_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let base_dir = format!("{}/", root.display());
    let mkgrd = mkgrd_config(&base_dir);
    let refine = refine_config();
    let plan = plan_mkgrd_refine_loop_io(&mkgrd, &refine).expect("plan refine io");
    let step = &plan.steps[0];
    assert_eq!(step.sources.len(), 2);
    assert_eq!(
        step.sources[0].source,
        MkgrdRefineSource::CalculatedIterZero
    );
    assert_eq!(step.sources[1].source, MkgrdRefineSource::SpecifiedStep);

    write_bbox_mask_netcdf(
        plan.file_dir.join("tmpfile/mask_refine_bbox_1_01.nc4"),
        &BBoxMask {
            refine_degree: 1,
            points: vec![BBoxPoint {
                west: -179.5,
                east: -176.0,
                north: 89.5,
                south: 86.0,
            }],
        },
    )
    .expect("write specified bbox");
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
    let is_in_domain = vec![vec![0; lat_i.len()]; lon_i.len()]
        .into_iter()
        .enumerate()
        .map(|(lon, mut row)| {
            if lon > 0 {
                for value in row.iter_mut().skip(1) {
                    *value = 1;
                }
            }
            row
        })
        .collect::<Vec<_>>();
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
    let dispatcher = MkgrdRefineSourceBranchExecutor::new(MkgrdRefineSourceBranchExecutorOptions {
        calculated: Some(MkgrdCalculatedRefineSourceExecutorOptions {
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
        }),
        specified: Some(MkgrdSpecifiedRefineSourceExecutorOptions {
            file_dir: &plan.file_dir,
            mesh_type: &plan.mesh_type,
            mask_refine_spc_type: "bbox",
            mask_refine_ndm: 1,
            is_in_domain: &is_in_domain,
            seaorland: &seaorland,
            lon_vertex: &lon_vertex,
            lat_vertex: &lat_vertex,
            lon_i: &lon_i,
            lat_i: &lat_i,
            gridnum_perdegree: 1,
            nlons_source: 6,
            nlats_source: 6,
            num_vertex: 0,
        }),
    });

    let calculated = dispatcher
        .run_source_branch_report(step, &step.sources[0])
        .expect("dispatch calculated source branch");
    let specified = dispatcher
        .run_source_branch_report(step, &step.sources[1])
        .expect("dispatch specified source branch");

    match calculated {
        MkgrdRefineSourceBranchReport::Calculated(report) => {
            assert_eq!(
                report.area.refine_write.output,
                step.sources[0].area_judge_output
            );
            assert_eq!(report.contain.output, step.sources[0].contain_output);
            assert_eq!(
                report
                    .getref
                    .single_mesh
                    .as_ref()
                    .unwrap()
                    .writes
                    .land
                    .as_ref()
                    .unwrap()
                    .output,
                step.sources[0].threshold_outputs[0]
            );
        }
        other => panic!("expected calculated report, got {other:?}"),
    }
    match specified {
        MkgrdRefineSourceBranchReport::Specified(report) => {
            assert_eq!(
                report.area.refine_write.output,
                step.sources[1].area_judge_output
            );
            assert_eq!(report.contain.output, step.sources[1].contain_output);
            assert_eq!(
                report.specified_threshold.output,
                step.sources[1]
                    .specified_threshold_output
                    .as_ref()
                    .unwrap()
                    .clone()
            );
        }
        other => panic!("expected specified report, got {other:?}"),
    }
    assert!(step.sources[0].threshold_outputs[0].exists());
    assert!(step.sources[1]
        .specified_threshold_output
        .as_ref()
        .unwrap()
        .exists());

    let _ = fs::remove_dir_all(&root);
}
