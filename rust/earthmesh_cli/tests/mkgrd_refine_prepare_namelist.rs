use earthmesh_cli::{
    mkgrd_refine_source_branch_options_from_prepare,
    mkgrd_specified_refine_source_options_from_prepare, prepare_mkgrd_refine_loop_namelist,
    run_mkgrd_refine_loop_namelist_with_executor, write_bbox_mask_netcdf, BBoxMask, BBoxPoint,
    MkgrdFinalQualityCheckIoPlan, MkgrdRefineLoopExecutor, MkgrdRefineLoopStepIoPlan,
    MkgrdRefinePrepareSourceGridOptions, MkgrdRefineSourceIoPlan,
};
use earthmesh_core::{EarthmeshConfig, EarthmeshRuntimeState};
use std::fs;
use std::io;
use std::path::PathBuf;

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp root");
    path
}

fn small_axes() -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
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
    let lon_i = std::iter::once(f64::NAN)
        .chain((0..6).map(|idx| -179.5 + idx as f64))
        .collect::<Vec<_>>();
    let lat_i = std::iter::once(f64::NAN)
        .chain((0..6).map(|idx| 89.5 - idx as f64))
        .collect::<Vec<_>>();
    (lon_vertex, lat_vertex, lon_i, lat_i)
}

#[test]
fn library_builds_data_preprocess_calculated_refine_from_prepare() {
    let root = temp_root("data_preprocess_calculated_refine_prepare");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_bbox_mask_netcdf(
        sources.join("refine_cal_01.nc4"),
        &BBoxMask {
            refine_degree: 0,
            points: vec![BBoxPoint {
                west: -179.5,
                east: -176.0,
                north: 89.5,
                south: 86.0,
            }],
        },
    )
    .expect("write calculated refine source");
    let namelist = root.join("mkgrd_data_preprocess_calculated_refine.nml");
    let base_dir = format!("{}/", root.display());
    let refine_cal_prefix = sources.join("refine_cal_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_data_preprocess_calculated_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=8\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  NL%refine=.true.\n  NL%mask_domain_global=.true.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_cal_prefix}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=0\n/\n"
        ),
    )
    .expect("write namelist");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();
    let state = earthmesh_cli::MkgrdDataPreprocessSourceState {
        lon_vertex,
        lat_vertex,
        lon_i,
        lat_i,
        gridnum_perdegree: 1,
        nlons_source: 6,
        nlats_source: 6,
        first_triangle_id: 1,
        num_vertex: 2,
        sources: Vec::new(),
        is_in_domain: vec![vec![1; 7]; 7],
        seaorland: vec![vec![1; 7]; 7],
        landtypes_global: vec![vec![1; 7]; 7],
        maxlc: 1,
    };
    let prepare = earthmesh_cli::prepare_mkgrd_refine_loop_namelist_with_source_grid(
        &namelist,
        &root,
        state.refine_prepare_source_grid(),
    )
    .expect("prepare refine loop with source grid");

    assert_eq!(prepare.runtime_state.source_grid.nlons_source, 6);
    assert_eq!(prepare.runtime_state.source_grid.nlats_source, 6);

    let report = earthmesh_cli::data_preprocess_source_state_calculated_refine_from_prepare(
        &prepare, &state,
    )
    .expect("build calculated refine from prepare")
    .expect("calculated refine report");

    assert!(report.numpatch > 0);
    let selected_cells = report
        .is_in_area
        .iter()
        .flat_map(|row| row.iter())
        .filter(|&&value| value != 0)
        .count();
    assert!(selected_cells > 0);
    assert_eq!(report.is_in_area[5][5], 0);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn library_provides_data_preprocess_source_branch_options_from_prepare() {
    let root = temp_root("data_preprocess_source_branch_options_prepare");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_bbox_mask_netcdf(
        sources.join("refine_cal_01.nc4"),
        &BBoxMask {
            refine_degree: 0,
            points: vec![BBoxPoint {
                west: -179.5,
                east: -176.0,
                north: 89.5,
                south: 86.0,
            }],
        },
    )
    .expect("write calculated refine source");
    let namelist = root.join("mkgrd_data_preprocess_source_branch_options.nml");
    let base_dir = format!("{}/", root.display());
    let refine_cal_prefix = sources.join("refine_cal_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_data_preprocess_source_branch_options'\n  NL%base_dir='{base_dir}'\n  NL%NXP=8\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  NL%refine=.true.\n  NL%mask_domain_global=.true.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_cal_prefix}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=0\n/\n"
        ),
    )
    .expect("write namelist");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();
    let state = earthmesh_cli::MkgrdDataPreprocessSourceState {
        lon_vertex,
        lat_vertex,
        lon_i,
        lat_i,
        gridnum_perdegree: 1,
        nlons_source: 6,
        nlats_source: 6,
        first_triangle_id: 1,
        num_vertex: 2,
        sources: Vec::new(),
        is_in_domain: vec![vec![1; 7]; 7],
        seaorland: vec![vec![1; 7]; 7],
        landtypes_global: vec![vec![1; 7]; 7],
        maxlc: 1,
    };
    let prepare = earthmesh_cli::prepare_mkgrd_refine_loop_namelist_with_source_grid(
        &namelist,
        &root,
        state.refine_prepare_source_grid(),
    )
    .expect("prepare refine loop with source grid");

    let (has_calculated, has_specified, selected_cells) =
        earthmesh_cli::with_data_preprocess_source_state_refine_source_branch_options_from_prepare(
            &prepare,
            &state,
            |options| {
                let calculated = options
                    .calculated
                    .expect("calculated source branch should be active");
                let selected_cells = calculated
                    .calculated_refine
                    .0
                    .iter()
                    .flat_map(|row| row.iter())
                    .filter(|&&value| value != 0)
                    .count();
                Ok((true, options.specified.is_some(), selected_cells))
            },
        )
        .expect("derive data_preprocess source branch options");

    assert!(has_calculated);
    assert!(!has_specified);
    assert!(selected_cells > 0);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn library_runs_data_preprocess_refine_execution_from_prepare() {
    let root = temp_root("data_preprocess_refine_execution_prepare");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_bbox_mask_netcdf(
        sources.join("refine_cal_01.nc4"),
        &BBoxMask {
            refine_degree: 0,
            points: vec![BBoxPoint {
                west: -179.5,
                east: -176.0,
                north: 89.5,
                south: 86.0,
            }],
        },
    )
    .expect("write calculated refine source");
    let namelist = root.join("mkgrd_data_preprocess_refine_execution.nml");
    let base_dir = format!("{}/", root.display());
    let refine_cal_prefix = sources.join("refine_cal_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_data_preprocess_refine_execution'\n  NL%base_dir='{base_dir}'\n  NL%NXP=8\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  NL%refine=.true.\n  NL%mask_domain_global=.true.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_cal_prefix}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=0\n/\n"
        ),
    )
    .expect("write namelist");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();
    let state = earthmesh_cli::MkgrdDataPreprocessSourceState {
        lon_vertex,
        lat_vertex,
        lon_i,
        lat_i,
        gridnum_perdegree: 1,
        nlons_source: 6,
        nlats_source: 6,
        first_triangle_id: 1,
        num_vertex: 2,
        sources: Vec::new(),
        is_in_domain: vec![vec![1; 7]; 7],
        seaorland: vec![vec![1; 7]; 7],
        landtypes_global: vec![vec![1; 7]; 7],
        maxlc: 1,
    };
    let prepare = earthmesh_cli::prepare_mkgrd_refine_loop_namelist_with_source_grid(
        &namelist,
        &root,
        state.refine_prepare_source_grid(),
    )
    .expect("prepare refine loop with source grid");
    let initial_mesh = earthmesh_cli::UnstructuredMesh {
        m_points: vec![earthmesh_cli::LonLatPoint {
            lon: -178.5,
            lat: 88.5,
        }],
        w_points: vec![
            earthmesh_cli::LonLatPoint {
                lon: -179.5,
                lat: 87.5,
            },
            earthmesh_cli::LonLatPoint {
                lon: -177.5,
                lat: 87.5,
            },
            earthmesh_cli::LonLatPoint {
                lon: -179.5,
                lat: 89.5,
            },
        ],
        m_to_w: vec![[1, 2, 3]],
        w_to_m: vec![vec![1], vec![1], vec![1]],
        n_w_to_m: vec![1, 1, 1],
    };

    let execution =
        earthmesh_cli::run_mkgrd_refine_loop_execution_with_data_preprocess_source_state(
            &prepare,
            &initial_mesh,
            &state,
            "LOCmesh",
            0.0,
            NetcdfFinalExecutor,
        )
        .expect("run data_preprocess refine execution from prepare");

    assert_eq!(execution.executed_sources, 1);
    assert_eq!(execution.executed_refine_steps, 1);
    assert!(prepare.plan.steps[0].refine_loop_input_gridfile.exists());
    assert!(prepare.plan.final_result_gridfile.exists());
    assert!(execution.final_handoff.generated_contain.is_some());

    let _ = fs::remove_dir_all(root);
}

#[derive(Default)]
struct RecordingExecutor {
    events: Vec<String>,
    runtime_state: Option<EarthmeshRuntimeState>,
}

impl MkgrdRefineLoopExecutor for RecordingExecutor {
    fn run_source_branch(
        &mut self,
        step: &MkgrdRefineLoopStepIoPlan,
        source: &MkgrdRefineSourceIoPlan,
    ) -> io::Result<()> {
        self.events.push(format!(
            "source:{}:{}:{}:{}",
            step.step, source.area_judge_iter, source.get_contain_iter, source.getref_iter
        ));
        Ok(())
    }

    fn run_refine_loop_step(&mut self, step: &MkgrdRefineLoopStepIoPlan) -> io::Result<()> {
        self.events.push(format!("refine:{}", step.step));
        if let Some(parent) = step.refine_loop_output_gridfile.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(
            &step.refine_loop_output_gridfile,
            format!("gridfile after step {}", step.step),
        )?;
        Ok(())
    }

    fn run_final_quality_check(&mut self, plan: &MkgrdFinalQualityCheckIoPlan) -> io::Result<()> {
        assert!(
            plan.regional_source_mask.is_some(),
            "final regional quality should be enriched before execution"
        );
        self.events.push("final-quality".to_string());
        Ok(())
    }

    fn runtime_state(&self) -> Option<&EarthmeshRuntimeState> {
        self.runtime_state.as_ref()
    }
}

#[derive(Default)]
struct NetcdfFinalExecutor;

impl MkgrdRefineLoopExecutor for NetcdfFinalExecutor {
    fn run_source_branch(
        &mut self,
        _step: &MkgrdRefineLoopStepIoPlan,
        _source: &MkgrdRefineSourceIoPlan,
    ) -> io::Result<()> {
        Ok(())
    }

    fn run_refine_loop_step(&mut self, step: &MkgrdRefineLoopStepIoPlan) -> io::Result<()> {
        earthmesh_cli::write_unstructured_mesh_netcdf(
            &step.refine_loop_output_gridfile,
            &earthmesh_cli::UnstructuredMesh {
                m_points: vec![earthmesh_cli::LonLatPoint {
                    lon: -178.5,
                    lat: 88.5,
                }],
                w_points: vec![
                    earthmesh_cli::LonLatPoint {
                        lon: -179.5,
                        lat: 87.5,
                    },
                    earthmesh_cli::LonLatPoint {
                        lon: -177.5,
                        lat: 87.5,
                    },
                    earthmesh_cli::LonLatPoint {
                        lon: -179.5,
                        lat: 89.5,
                    },
                ],
                m_to_w: vec![[1, 2, 3]],
                w_to_m: vec![vec![1], vec![1], vec![1]],
                n_w_to_m: vec![1, 1, 1],
            },
        )
        .map(|_| ())
    }

    fn run_final_quality_check(&mut self, _plan: &MkgrdFinalQualityCheckIoPlan) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn refine_prepare_namelist_applies_masks_and_enriches_final_regional_source_mask() {
    let root = temp_root("refine_prepare_namelist");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_bbox_mask_netcdf(
        sources.join("patch_01.nc4"),
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
    .expect("write mask patch source");

    let namelist = root.join("mkgrd_refine.nml");
    let base_dir = format!("{}/", root.display());
    let patch_prefix = sources.join("patch_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_refine_prepare'\n  NL%base_dir='{base_dir}'\n  NL%NXP=8\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  NL%refine=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.true.\n  NL%mask_patch_type='bbox'\n  NL%mask_patch_fprefix='{patch_prefix}'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=2\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{patch_prefix}'\n/\n"
        ),
    )
    .expect("write namelist");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();

    let report = prepare_mkgrd_refine_loop_namelist(
        &namelist,
        &root,
        &lon_vertex,
        &lat_vertex,
        &lon_i,
        &lat_i,
        1,
        6,
        6,
        2,
    )
    .expect("prepare refine loop from namelist");

    assert_eq!(report.workspace_mask.mask_counts.mask_patch_ndm[1], 1);
    assert_eq!(report.runtime_state.mask_counts.mask_patch_ndm[1], 1);
    assert_eq!(report.runtime_state.mask_counts.mask_refine_ndm[1], 1);
    assert_eq!(report.runtime_state.mask_counts.mask_domain_ndm, 0);
    assert_eq!(
        report.workspace_mask.mask_reports[0].outputs,
        vec![root.join("case_refine_prepare/tmpfile/mask_patch_bbox_1_01.nc4")]
    );
    assert!(report.final_source_mask_injected);
    assert_eq!(
        report.runtime_state.config.experiment_name,
        "case_refine_prepare"
    );
    assert_eq!(
        report.runtime_state.refine.as_ref().unwrap().refine_setting,
        "specified"
    );
    assert_eq!(
        report.runtime_state.step,
        report.plan.final_mask_postproc_step
    );
    assert_eq!(report.runtime_state.try_nxp().expect("positive NXP"), 8);
    assert_eq!(report.runtime_state.num_mp_step, [1; 10]);
    assert_eq!(report.runtime_state.num_wp_step, [1; 10]);

    let mut shadowed_report = report.clone();
    shadowed_report.refine.refine_spc = false;
    let is_in_domain = vec![vec![1; 7]; 7];
    let seaorland = vec![vec![1; 7]; 7];
    let landtypes_global = vec![vec![1; 7]; 7];
    let options = mkgrd_specified_refine_source_options_from_prepare(
        &shadowed_report,
        MkgrdRefinePrepareSourceGridOptions {
            lon_vertex: &lon_vertex,
            lat_vertex: &lat_vertex,
            lon_i: &lon_i,
            lat_i: &lat_i,
            gridnum_perdegree: 1,
            nlons_source: 6,
            nlats_source: 6,
            first_triangle_id: 2,
        },
        &is_in_domain,
        &seaorland,
        2,
    )
    .expect("specified source options should use runtime_state refine config");
    assert_eq!(options.mask_refine_spc_type, "bbox");
    assert_eq!(options.mask_refine_ndm, 1);

    let branch_options = mkgrd_refine_source_branch_options_from_prepare(
        &shadowed_report,
        MkgrdRefinePrepareSourceGridOptions {
            lon_vertex: &lon_vertex,
            lat_vertex: &lat_vertex,
            lon_i: &lon_i,
            lat_i: &lat_i,
            gridnum_perdegree: 1,
            nlons_source: 6,
            nlats_source: 6,
            first_triangle_id: 2,
        },
        None,
        Some(&is_in_domain),
        &seaorland,
        &landtypes_global,
        2,
        9,
    )
    .expect("branch options should use runtime_state refine config");
    assert!(branch_options.specified.is_some());
    assert!(branch_options.calculated.is_none());

    let source_mask = report
        .plan
        .final_quality_check
        .regional_source_mask
        .as_ref()
        .expect("source mask injected");
    assert_eq!(source_mask.first_triangle_id, 2);
    assert!(source_mask.mask_patch[2][2]);
    assert!(!source_mask.mask_patch[1][1]);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn refine_run_namelist_prepares_enriches_and_executes_with_existing_executor() {
    let root = temp_root("refine_run_namelist");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_bbox_mask_netcdf(
        sources.join("patch_01.nc4"),
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
    .expect("write mask patch source");

    let namelist = root.join("mkgrd_refine_run.nml");
    let base_dir = format!("{}/", root.display());
    let patch_prefix = sources.join("patch_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_refine_run'\n  NL%base_dir='{base_dir}'\n  NL%NXP=8\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  NL%refine=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.true.\n  NL%mask_patch_type='bbox'\n  NL%mask_patch_fprefix='{patch_prefix}'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=2\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{patch_prefix}'\n/\n"
        ),
    )
    .expect("write namelist");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();
    let mut expected_runtime_state = EarthmeshRuntimeState::new(EarthmeshConfig::default());
    expected_runtime_state
        .record_mesh_counts_for_step(1, 7, 9)
        .expect("record expected runtime counts");
    let mut executor = RecordingExecutor {
        runtime_state: Some(expected_runtime_state),
        ..RecordingExecutor::default()
    };

    let report = run_mkgrd_refine_loop_namelist_with_executor(
        &namelist,
        &root,
        &lon_vertex,
        &lat_vertex,
        &lon_i,
        &lat_i,
        1,
        6,
        6,
        2,
        &mut executor,
        None,
    )
    .expect("run refine loop from namelist");

    assert!(report.prepare.final_source_mask_injected);
    assert!(report.source_branch_reports().is_empty());
    let runtime_state = report.runtime_state();
    assert_eq!(runtime_state.num_mp_step[0], 7);
    assert_eq!(runtime_state.num_wp_step[0], 9);
    assert!(report.execution.ran_final_quality_check);
    assert_eq!(
        executor.events,
        vec![
            "source:1:1:1:1".to_string(),
            "refine:1".to_string(),
            "final-quality".to_string()
        ]
    );
    assert_eq!(
        fs::read_to_string(&report.prepare.plan.final_result_gridfile)
            .expect("read final result gridfile"),
        "gridfile after step 1"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn refine_run_namelist_can_generate_final_domain_contain_after_prepare() {
    let root = temp_root("refine_run_namelist_final_domain_contain");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_bbox_mask_netcdf(
        sources.join("refine_01.nc4"),
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
    .expect("write specified refine source");

    let namelist = root.join("mkgrd_refine_final_domain_contain.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_refine_final_domain_contain'\n  NL%base_dir='{base_dir}'\n  NL%NXP=8\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  NL%refine=.true.\n  NL%mask_domain_global=.true.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n"
        ),
    )
    .expect("write namelist");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();
    let mut domain = vec![vec![0; lat_i.len()]; lon_i.len()];
    domain[1][1] = 1;
    domain[1][2] = 1;
    domain[2][1] = 1;
    domain[2][2] = 1;
    let area_payload = earthmesh_cli::select_area_judge_grid_fortran_indexed(
        &domain,
        None,
        &lon_i,
        &lat_i,
        earthmesh_mesh::AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: 2,
            maxlat_source: 1,
            minlat_source: 2,
        },
    )
    .expect("select domain area grid");
    let area_grid = root
        .join("case_refine_final_domain_contain")
        .join("result/IsInDmArea_grid.nc4");
    let mut seaorland = vec![vec![0; lat_i.len()]; lon_i.len()];
    seaorland[1][1] = 1;
    seaorland[1][2] = 1;
    seaorland[2][2] = 1;
    let mut executor = NetcdfFinalExecutor;

    let report =
        earthmesh_cli::run_mkgrd_refine_loop_namelist_with_executor_and_source_grid_and_final_domain_contain(
            &namelist,
            &root,
            earthmesh_cli::MkgrdRefinePrepareSourceGridOptions {
                lon_vertex: &lon_vertex,
                lat_vertex: &lat_vertex,
                lon_i: &lon_i,
                lat_i: &lat_i,
                gridnum_perdegree: 1,
                nlons_source: 6,
                nlats_source: 6,
                first_triangle_id: 1,
            },
            &mut executor,
            Some(earthmesh_cli::MkgrdFinalDomainContainOptions {
                area_grid_file: &area_grid,
                mesh_kind: earthmesh_cli::GetContainMeshKind::Land,
                seaorland: &seaorland,
                lon_vertex: &lon_vertex,
                lat_vertex: &lat_vertex,
                lon_i: &lon_i,
                lat_i: &lat_i,
                num_vertex: 0,
            }),
            None,
            |prepare| {
                earthmesh_cli::write_area_judge_grid_netcdf(&area_grid, &area_payload)?;
                earthmesh_cli::write_unstructured_mesh_netcdf(
                    &prepare.plan.steps[0].refine_loop_input_gridfile,
                    &earthmesh_cli::UnstructuredMesh {
                        m_points: vec![earthmesh_cli::LonLatPoint {
                            lon: -178.5,
                            lat: 88.5,
                        }],
                        w_points: vec![
                            earthmesh_cli::LonLatPoint {
                                lon: -179.5,
                                lat: 87.5,
                            },
                            earthmesh_cli::LonLatPoint {
                                lon: -177.5,
                                lat: 87.5,
                            },
                            earthmesh_cli::LonLatPoint {
                                lon: -179.5,
                                lat: 89.5,
                            },
                        ],
                        m_to_w: vec![[1, 2, 3]],
                        w_to_m: vec![vec![1], vec![1], vec![1]],
                        n_w_to_m: vec![1, 1, 1],
                    },
                )
                .map(|_| ())
            },
        )
        .expect("run namelist refine with final domain contain");

    assert!(report.execution.final_handoff.generated_contain.is_some());
    let contain =
        earthmesh_cli::read_contain_netcdf(&report.prepare.plan.final_domain_contain_output)
            .expect("read generated final domain contain");
    assert_eq!(contain.is_in_area_ustr, vec![0, 1]);
    assert_eq!(contain.ustr_id, vec![vec![0, 0], vec![3, 1]]);
    assert_eq!(contain.ustr_ii, vec![vec![1, 1], vec![1, 2], vec![2, 2]]);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn refine_run_namelist_with_migrated_executor_uses_standard_source_and_working_state_stack() {
    let root = temp_root("refine_run_migrated_executor");
    let namelist = root.join("mkgrd_refine_migrated.nml");
    let base_dir = format!("{}/", root.display());
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_bbox_mask_netcdf(
        sources.join("refine_01.nc4"),
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
    .expect("write specified refine source");
    let refine_prefix = sources.join("refine_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_refine_migrated'\n  NL%base_dir='{base_dir}'\n  NL%NXP=8\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  NL%refine=.true.\n  NL%mask_domain_global=.true.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n"
        ),
    )
    .expect("write namelist");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();

    let report =
        earthmesh_cli::run_mkgrd_refine_loop_namelist_with_migrated_executor_and_prepare_hook(
            &namelist,
            &root,
            earthmesh_cli::MkgrdRefinePrepareSourceGridOptions {
                lon_vertex: &lon_vertex,
                lat_vertex: &lat_vertex,
                lon_i: &lon_i,
                lat_i: &lat_i,
                gridnum_perdegree: 1,
                nlons_source: 6,
                nlats_source: 6,
                first_triangle_id: 1,
            },
            earthmesh_cli::MkgrdRefineSourceBranchExecutorOptions {
                calculated: None,
                specified: Some(earthmesh_cli::MkgrdSpecifiedRefineSourceExecutorOptions {
                    file_dir: &root.join("case_refine_migrated"),
                    mesh_type: "landmesh",
                    mask_refine_spc_type: "bbox",
                    mask_refine_ndm: 1,
                    mask_refine_ndm_by_iter: &[0, 1, 0, 0, 0, 0, 0, 0, 0, 0],
                    is_in_domain: &vec![vec![1; lat_i.len()]; lon_i.len()],
                    seaorland: &vec![vec![1; lat_i.len()]; lon_i.len()],
                    lon_vertex: &lon_vertex,
                    lat_vertex: &lat_vertex,
                    lon_i: &lon_i,
                    lat_i: &lat_i,
                    gridnum_perdegree: 1,
                    nlons_source: 6,
                    nlats_source: 6,
                    num_vertex: 0,
                }),
            },
            earthmesh_cli::MkgrdRefineLoopWorkingStateExecutor::default(),
            None,
            |prepare| {
                earthmesh_cli::write_unstructured_mesh_netcdf(
                    &prepare.plan.steps[0].refine_loop_input_gridfile,
                    &earthmesh_cli::UnstructuredMesh {
                        m_points: vec![earthmesh_cli::LonLatPoint {
                            lon: -178.5,
                            lat: 88.5,
                        }],
                        w_points: vec![
                            earthmesh_cli::LonLatPoint {
                                lon: -179.5,
                                lat: 87.5,
                            },
                            earthmesh_cli::LonLatPoint {
                                lon: -177.5,
                                lat: 87.5,
                            },
                            earthmesh_cli::LonLatPoint {
                                lon: -179.5,
                                lat: 89.5,
                            },
                        ],
                        m_to_w: vec![[1, 2, 3]],
                        w_to_m: vec![vec![1], vec![1], vec![1]],
                        n_w_to_m: vec![1, 1, 1],
                    },
                )
                .map(|_| ())
            },
        )
        .expect("run refine loop through standard migrated executor");

    assert_eq!(report.execution.executed_sources, 1);
    assert_eq!(report.execution.executed_refine_steps, 1);
    assert!(!report.execution.ran_final_quality_check);
    let runtime_state = report.runtime_state();
    assert_eq!(runtime_state.num_mp_step[0], 1);
    assert_eq!(runtime_state.num_wp_step[0], 3);
    let final_mesh =
        earthmesh_cli::read_unstructured_mesh_netcdf(&report.prepare.plan.final_result_gridfile)
            .expect("read final passthrough gridfile");
    assert_eq!(final_mesh.m_points.len(), 1);
    assert_eq!(final_mesh.w_points.len(), 3);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn refine_run_namelist_with_specified_migrated_executor_derives_source_options_after_prepare() {
    let root = temp_root("refine_run_specified_migrated_executor");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_bbox_mask_netcdf(
        sources.join("refine_01.nc4"),
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
    .expect("write specified refine source");

    let namelist = root.join("mkgrd_refine_specified_migrated.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_refine_specified_migrated'\n  NL%base_dir='{base_dir}'\n  NL%NXP=8\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  NL%refine=.true.\n  NL%mask_domain_global=.true.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n"
        ),
    )
    .expect("write namelist");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();
    let is_in_domain = vec![vec![1; lat_i.len()]; lon_i.len()];
    let seaorland = vec![vec![1; lat_i.len()]; lon_i.len()];

    let report = earthmesh_cli::run_mkgrd_refine_loop_namelist_with_specified_migrated_executor_and_prepare_hook(
        &namelist,
        &root,
        earthmesh_cli::MkgrdRefinePrepareSourceGridOptions {
            lon_vertex: &lon_vertex,
            lat_vertex: &lat_vertex,
            lon_i: &lon_i,
            lat_i: &lat_i,
            gridnum_perdegree: 1,
            nlons_source: 6,
            nlats_source: 6,
            first_triangle_id: 1,
        },
        &is_in_domain,
        &seaorland,
        0,
        earthmesh_cli::MkgrdRefineLoopWorkingStateExecutor::default(),
        None,
        |prepare| {
            earthmesh_cli::write_unstructured_mesh_netcdf(
                &prepare.plan.steps[0].refine_loop_input_gridfile,
                &earthmesh_cli::UnstructuredMesh {
                    m_points: vec![earthmesh_cli::LonLatPoint { lon: -178.5, lat: 88.5 }],
                    w_points: vec![
                        earthmesh_cli::LonLatPoint { lon: -179.5, lat: 87.5 },
                        earthmesh_cli::LonLatPoint { lon: -177.5, lat: 87.5 },
                        earthmesh_cli::LonLatPoint { lon: -179.5, lat: 89.5 },
                    ],
                    m_to_w: vec![[1, 2, 3]],
                    w_to_m: vec![vec![1], vec![1], vec![1]],
                    n_w_to_m: vec![1, 1, 1],
                },
            )
            .map(|_| ())
        },
    )
    .expect("run specified migrated refine loop with derived options");

    assert_eq!(report.execution.executed_sources, 1);
    assert_eq!(report.execution.executed_refine_steps, 1);
    assert_eq!(
        report.prepare.workspace_mask.mask_counts.mask_refine_ndm[1],
        1
    );
    assert!(report.prepare.plan.final_result_gridfile.exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn refine_run_namelist_with_calculated_migrated_executor_derives_source_options_after_prepare() {
    let root = temp_root("refine_run_calculated_migrated_executor");
    let namelist = root.join("mkgrd_refine_calculated_migrated.nml");
    let threshold_dir = root.join("threshold_inputs");
    fs::create_dir_all(&threshold_dir).expect("create threshold dir");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_bbox_mask_netcdf(
        sources.join("refine_cal_01.nc4"),
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
    .expect("write calculated refine source");
    let base_dir = format!("{}/", root.display());
    let threshold_dir_text = threshold_dir.display().to_string();
    let refine_cal_prefix = sources.join("refine_cal_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_refine_calculated_migrated'\n  NL%base_dir='{base_dir}'\n  NL%NXP=8\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  NL%refine=.true.\n  NL%mask_domain_global=.true.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_cal_prefix}'\n  RL%threshold_dir='{threshold_dir_text}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=0\n/\n"
        ),
    )
    .expect("write namelist");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();
    let mut calculated_refine = vec![vec![0; lat_i.len()]; lon_i.len()];
    calculated_refine[1][1] = 1;
    calculated_refine[1][2] = 1;
    calculated_refine[2][1] = 1;
    calculated_refine[2][2] = 1;
    let bounds = earthmesh_mesh::AreaJudgeSourceBounds {
        minlon_source: 1,
        maxlon_source: 2,
        maxlat_source: 1,
        minlat_source: 2,
    };
    let mut seaorland = vec![vec![0; lat_i.len()]; lon_i.len()];
    seaorland[1][1] = 1;
    seaorland[1][2] = 1;
    seaorland[2][2] = 1;
    let mut landtypes = vec![vec![0; lat_i.len()]; lon_i.len()];
    landtypes[1][1] = 1;
    landtypes[1][2] = 2;
    landtypes[2][2] = 3;

    let report = earthmesh_cli::run_mkgrd_refine_loop_namelist_with_calculated_migrated_executor_and_prepare_hook(
        &namelist,
        &root,
        earthmesh_cli::MkgrdRefinePrepareSourceGridOptions {
            lon_vertex: &lon_vertex,
            lat_vertex: &lat_vertex,
            lon_i: &lon_i,
            lat_i: &lat_i,
            gridnum_perdegree: 1,
            nlons_source: 6,
            nlats_source: 6,
            first_triangle_id: 1,
        },
        (&calculated_refine, bounds),
        &seaorland,
        &landtypes,
        0,
        99,
        earthmesh_cli::MkgrdRefineLoopWorkingStateExecutor::default(),
        None,
        |prepare| {
            earthmesh_cli::write_unstructured_mesh_netcdf(
                &prepare.plan.steps[0].refine_loop_input_gridfile,
                &earthmesh_cli::UnstructuredMesh {
                    m_points: vec![earthmesh_cli::LonLatPoint { lon: -178.5, lat: 88.5 }],
                    w_points: vec![
                        earthmesh_cli::LonLatPoint { lon: -179.5, lat: 87.5 },
                        earthmesh_cli::LonLatPoint { lon: -177.5, lat: 87.5 },
                        earthmesh_cli::LonLatPoint { lon: -179.5, lat: 89.5 },
                    ],
                    m_to_w: vec![[1, 2, 3]],
                    w_to_m: vec![vec![1], vec![1], vec![1]],
                    n_w_to_m: vec![1, 1, 1],
                },
            )
            .map(|_| ())
        },
    )
    .expect("run calculated migrated refine loop with derived options");

    assert_eq!(report.execution.executed_sources, 1);
    assert_eq!(report.execution.executed_refine_steps, 1);
    assert_eq!(
        report.prepare.refine.threshold_dir,
        threshold_dir.display().to_string()
    );
    assert!(report.prepare.plan.steps[0].sources[0].threshold_outputs[0].exists());
    assert!(report.prepare.plan.final_result_gridfile.exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn refine_run_namelist_with_derived_migrated_executor_handles_mixed_sources_after_prepare() {
    let root = temp_root("refine_run_derived_mixed_migrated_executor");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    for stem in ["refine_cal_01.nc4", "refine_spc_01.nc4"] {
        write_bbox_mask_netcdf(
            sources.join(stem),
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
        .expect("write mixed refine source");
    }

    let namelist = root.join("mkgrd_refine_derived_mixed.nml");
    let threshold_dir = root.join("threshold_inputs");
    fs::create_dir_all(&threshold_dir).expect("create threshold dir");
    let base_dir = format!("{}/", root.display());
    let threshold_dir_text = threshold_dir.display().to_string();
    let refine_cal_prefix = sources.join("refine_cal_").display().to_string();
    let refine_spc_prefix = sources.join("refine_spc_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_refine_derived_mixed'\n  NL%base_dir='{base_dir}'\n  NL%NXP=8\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  NL%refine=.true.\n  NL%mask_domain_global=.true.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_cal_prefix}'\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_spc_prefix}'\n  RL%threshold_dir='{threshold_dir_text}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=0\n/\n"
        ),
    )
    .expect("write mixed namelist");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();
    let source_grid = earthmesh_cli::MkgrdRefinePrepareSourceGridOptions {
        lon_vertex: &lon_vertex,
        lat_vertex: &lat_vertex,
        lon_i: &lon_i,
        lat_i: &lat_i,
        gridnum_perdegree: 1,
        nlons_source: 6,
        nlats_source: 6,
        first_triangle_id: 1,
    };
    let bounds = earthmesh_mesh::AreaJudgeSourceBounds {
        minlon_source: 1,
        maxlon_source: 2,
        maxlat_source: 1,
        minlat_source: 2,
    };
    let mut calculated_refine = vec![vec![0; lat_i.len()]; lon_i.len()];
    for lon in 1..=2 {
        for lat in 1..=2 {
            calculated_refine[lon][lat] = 1;
        }
    }
    let is_in_domain = vec![vec![1; lat_i.len()]; lon_i.len()];
    let mut seaorland = vec![vec![0; lat_i.len()]; lon_i.len()];
    seaorland[1][1] = 1;
    seaorland[1][2] = 1;
    seaorland[2][2] = 1;
    let mut landtypes = vec![vec![0; lat_i.len()]; lon_i.len()];
    landtypes[1][1] = 1;
    landtypes[1][2] = 2;
    landtypes[2][2] = 3;

    let report = earthmesh_cli::run_mkgrd_refine_loop_namelist_with_derived_migrated_executor_and_prepare_hook(
        &namelist,
        &root,
        source_grid,
        Some((&calculated_refine, bounds)),
        Some(&is_in_domain),
        &seaorland,
        &landtypes,
        0,
        99,
        earthmesh_cli::MkgrdRefineLoopWorkingStateExecutor::default(),
        None,
        |prepare| {
            earthmesh_cli::write_unstructured_mesh_netcdf(
                &prepare.plan.steps[0].refine_loop_input_gridfile,
                &earthmesh_cli::UnstructuredMesh {
                    m_points: vec![earthmesh_cli::LonLatPoint { lon: -178.5, lat: 88.5 }],
                    w_points: vec![
                        earthmesh_cli::LonLatPoint { lon: -179.5, lat: 87.5 },
                        earthmesh_cli::LonLatPoint { lon: -177.5, lat: 87.5 },
                        earthmesh_cli::LonLatPoint { lon: -179.5, lat: 89.5 },
                    ],
                    m_to_w: vec![[1, 2, 3]],
                    w_to_m: vec![vec![1], vec![1], vec![1]],
                    n_w_to_m: vec![1, 1, 1],
                },
            )
            .map(|_| ())
        },
    )
    .expect("run mixed migrated refine loop with derived options");

    assert_eq!(report.execution.executed_sources, 2);
    assert_eq!(report.execution.executed_refine_steps, 1);
    assert!(report.prepare.plan.steps[0].sources[0].threshold_outputs[0].exists());
    assert!(report.prepare.plan.steps[0].sources[1]
        .specified_threshold_output
        .as_ref()
        .expect("specified threshold output")
        .exists());
    assert!(report.prepare.plan.final_result_gridfile.exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn refine_run_namelist_with_derived_migrated_executor_can_generate_final_domain_contain() {
    let root = temp_root("refine_run_derived_migrated_final_domain_contain");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_bbox_mask_netcdf(
        sources.join("refine_spc_01.nc4"),
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
    .expect("write specified refine source");

    let namelist = root.join("mkgrd_refine_derived_final_domain_contain.nml");
    let base_dir = format!("{}/", root.display());
    let refine_spc_prefix = sources.join("refine_spc_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_refine_derived_final_domain_contain'\n  NL%base_dir='{base_dir}'\n  NL%NXP=8\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  NL%refine=.true.\n  NL%mask_domain_global=.true.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_spc_prefix}'\n/\n"
        ),
    )
    .expect("write namelist");

    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();
    let source_grid = earthmesh_cli::MkgrdRefinePrepareSourceGridOptions {
        lon_vertex: &lon_vertex,
        lat_vertex: &lat_vertex,
        lon_i: &lon_i,
        lat_i: &lat_i,
        gridnum_perdegree: 1,
        nlons_source: 6,
        nlats_source: 6,
        first_triangle_id: 1,
    };
    let is_in_domain = vec![vec![1; lat_i.len()]; lon_i.len()];
    let mut domain = vec![vec![0; lat_i.len()]; lon_i.len()];
    for lon in 1..=2 {
        for lat in 1..=2 {
            domain[lon][lat] = 1;
        }
    }
    let area_payload = earthmesh_cli::select_area_judge_grid_fortran_indexed(
        &domain,
        None,
        &lon_i,
        &lat_i,
        earthmesh_mesh::AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: 2,
            maxlat_source: 1,
            minlat_source: 2,
        },
    )
    .expect("select final domain area grid");
    let area_grid = root
        .join("case_refine_derived_final_domain_contain")
        .join("result/IsInDmArea_grid.nc4");
    let mut seaorland = vec![vec![0; lat_i.len()]; lon_i.len()];
    seaorland[1][1] = 1;
    seaorland[1][2] = 1;
    seaorland[2][2] = 1;
    let landtypes = vec![vec![0; lat_i.len()]; lon_i.len()];

    let report = earthmesh_cli::run_mkgrd_refine_loop_namelist_with_derived_migrated_executor_and_final_domain_contain_and_prepare_hook(
        &namelist,
        &root,
        source_grid,
        None,
        Some(&is_in_domain),
        &seaorland,
        &landtypes,
        0,
        99,
        earthmesh_cli::MkgrdRefineLoopWorkingStateExecutor::default(),
        Some(earthmesh_cli::MkgrdFinalDomainContainOptions {
            area_grid_file: &area_grid,
            mesh_kind: earthmesh_cli::GetContainMeshKind::Land,
            seaorland: &seaorland,
            lon_vertex: &lon_vertex,
            lat_vertex: &lat_vertex,
            lon_i: &lon_i,
            lat_i: &lat_i,
            num_vertex: 0,
        }),
        None,
        |prepare| {
            earthmesh_cli::write_area_judge_grid_netcdf(&area_grid, &area_payload)?;
            earthmesh_cli::write_unstructured_mesh_netcdf(
                &prepare.plan.steps[0].refine_loop_input_gridfile,
                &earthmesh_cli::UnstructuredMesh {
                    m_points: vec![earthmesh_cli::LonLatPoint { lon: -178.5, lat: 88.5 }],
                    w_points: vec![
                        earthmesh_cli::LonLatPoint { lon: -179.5, lat: 87.5 },
                        earthmesh_cli::LonLatPoint { lon: -177.5, lat: 87.5 },
                        earthmesh_cli::LonLatPoint { lon: -179.5, lat: 89.5 },
                    ],
                    m_to_w: vec![[1, 2, 3]],
                    w_to_m: vec![vec![1], vec![1], vec![1]],
                    n_w_to_m: vec![1, 1, 1],
                },
            )
            .map(|_| ())
        },
    )
    .expect("run derived migrated refine loop with final domain contain");

    assert!(report.execution.final_handoff.generated_contain.is_some());
    let contain =
        earthmesh_cli::read_contain_netcdf(&report.prepare.plan.final_domain_contain_output)
            .expect("read generated final domain contain");
    assert_eq!(contain.is_in_area_ustr, vec![0, 1]);
    assert_eq!(contain.ustr_id, vec![vec![0, 0], vec![3, 1]]);
    assert_eq!(contain.ustr_ii, vec![vec![1, 1], vec![1, 2], vec![2, 2]]);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn specified_refine_options_keep_per_iter_mask_counts() {
    let root = temp_root("specified_refine_per_iter_counts");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_bbox_mask_netcdf(
        sources.join("refine_01.nc4"),
        &BBoxMask {
            refine_degree: 1,
            points: vec![BBoxPoint {
                west: -179.5,
                east: -178.0,
                north: 89.5,
                south: 88.0,
            }],
        },
    )
    .expect("write iter1 source A");
    write_bbox_mask_netcdf(
        sources.join("refine_02.nc4"),
        &BBoxMask {
            refine_degree: 1,
            points: vec![BBoxPoint {
                west: -178.5,
                east: -177.0,
                north: 88.5,
                south: 87.0,
            }],
        },
    )
    .expect("write iter1 source B");
    write_bbox_mask_netcdf(
        sources.join("refine_03.nc4"),
        &BBoxMask {
            refine_degree: 2,
            points: vec![BBoxPoint {
                west: -177.5,
                east: -176.0,
                north: 87.5,
                south: 86.0,
            }],
        },
    )
    .expect("write iter2 source");

    let namelist = root.join("mkgrd_specified_per_iter_counts.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_specified_per_iter_counts'\n  NL%base_dir='{base_dir}'\n  NL%NXP=8\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  NL%refine=.true.\n  NL%mask_domain_global=.true.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=2\n  RL%max_iter_cal=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n"
        ),
    )
    .expect("write namelist");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();
    let report = prepare_mkgrd_refine_loop_namelist(
        &namelist,
        &root,
        &lon_vertex,
        &lat_vertex,
        &lon_i,
        &lat_i,
        1,
        6,
        6,
        2,
    )
    .expect("prepare specified refine");
    let is_in_domain = vec![vec![1; 7]; 7];
    let seaorland = vec![vec![1; 7]; 7];
    let options = mkgrd_specified_refine_source_options_from_prepare(
        &report,
        MkgrdRefinePrepareSourceGridOptions {
            lon_vertex: &lon_vertex,
            lat_vertex: &lat_vertex,
            lon_i: &lon_i,
            lat_i: &lat_i,
            gridnum_perdegree: 1,
            nlons_source: 6,
            nlats_source: 6,
            first_triangle_id: 2,
        },
        &is_in_domain,
        &seaorland,
        2,
    )
    .expect("derive specified options");

    assert_eq!(options.mask_refine_ndm_by_iter[1], 2);
    assert_eq!(options.mask_refine_ndm_by_iter[2], 1);

    let _ = fs::remove_dir_all(root);
}
