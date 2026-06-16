use std::fs;
use std::io;

use earthmesh_cli::{
    MkgrdRefineLoopExecutor, MkgrdRefineLoopStepIoPlan, MkgrdRefineSource, MkgrdRefineSourceIoPlan,
};
use earthmesh_core::{EarthmeshConfig, EarthmeshRuntimeState, RefineConfig};
use earthmesh_mesh::AreaJudgeSourceBounds;

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
            "source:{}:{:?}:{}:{}:{}",
            step.step,
            source.source,
            source.area_judge_iter,
            source.get_contain_iter,
            source.getref_iter
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

    fn run_final_quality_check(
        &mut self,
        _plan: &earthmesh_cli::MkgrdFinalQualityCheckIoPlan,
    ) -> io::Result<()> {
        self.events.push("final-quality".to_string());
        Ok(())
    }

    fn runtime_state(&self) -> Option<&EarthmeshRuntimeState> {
        self.runtime_state.as_ref()
    }
}

#[derive(Default)]
struct NetcdfGridExecutor {
    runtime_state: Option<EarthmeshRuntimeState>,
    mesh: Option<earthmesh_cli::UnstructuredMesh>,
}

impl MkgrdRefineLoopExecutor for NetcdfGridExecutor {
    fn run_source_branch(
        &mut self,
        _step: &MkgrdRefineLoopStepIoPlan,
        _source: &MkgrdRefineSourceIoPlan,
    ) -> io::Result<()> {
        Ok(())
    }

    fn run_refine_loop_step(&mut self, step: &MkgrdRefineLoopStepIoPlan) -> io::Result<()> {
        let mesh = self
            .mesh
            .clone()
            .unwrap_or_else(|| earthmesh_cli::UnstructuredMesh {
                m_points: vec![earthmesh_cli::LonLatPoint { lon: 1.0, lat: 1.0 }],
                w_points: vec![
                    earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
                    earthmesh_cli::LonLatPoint { lon: 2.0, lat: 0.0 },
                    earthmesh_cli::LonLatPoint { lon: 0.0, lat: 2.0 },
                ],
                m_to_w: vec![[1, 2, 3]],
                w_to_m: vec![vec![1], vec![1], vec![1]],
                n_w_to_m: vec![1, 1, 1],
            });
        earthmesh_cli::write_unstructured_mesh_netcdf(&step.refine_loop_output_gridfile, &mesh)
            .map(|_| ())
    }

    fn run_final_quality_check(
        &mut self,
        _plan: &earthmesh_cli::MkgrdFinalQualityCheckIoPlan,
    ) -> io::Result<()> {
        Ok(())
    }

    fn runtime_state(&self) -> Option<&EarthmeshRuntimeState> {
        self.runtime_state.as_ref()
    }
}

#[derive(Default)]
struct AtmosNetcdfGridExecutor;

impl MkgrdRefineLoopExecutor for AtmosNetcdfGridExecutor {
    fn run_source_branch(
        &mut self,
        _step: &MkgrdRefineLoopStepIoPlan,
        _source: &MkgrdRefineSourceIoPlan,
    ) -> io::Result<()> {
        Ok(())
    }

    fn run_refine_loop_step(&mut self, step: &MkgrdRefineLoopStepIoPlan) -> io::Result<()> {
        if let Some(parent) = step.refine_loop_output_gridfile.parent() {
            fs::create_dir_all(parent)?;
        }
        earthmesh_cli::write_unstructured_mesh_netcdf(
            &step.refine_loop_output_gridfile,
            &sample_atmos_simple_source_mesh(),
        )
        .map(|_| ())
    }

    fn run_final_quality_check(
        &mut self,
        _plan: &earthmesh_cli::MkgrdFinalQualityCheckIoPlan,
    ) -> io::Result<()> {
        Ok(())
    }
}

fn mkgrd_config(base_dir: &str) -> EarthmeshConfig {
    EarthmeshConfig::from_mkgrd_namelist(&format!(
        "&mkgrd\n  NL%EXPNME='case_refine_exec'\n  NL%base_dir='{base_dir}'\n  NL%NXP=8\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  NL%refine=.true.\n/\n"
    ))
    .expect("parse mkgrd config")
}

fn mkgrd_atmos_config(base_dir: &str) -> EarthmeshConfig {
    EarthmeshConfig::from_mkgrd_namelist(&format!(
        "&mkgrd\n  NL%EXPNME='case_refine_exec_atmos'\n  NL%base_dir='{base_dir}'\n  NL%NXP=9\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='tri'\n  NL%output_format='MPAS-Simple'\n  NL%refine=.true.\n/\n"
    ))
    .expect("parse mkgrd atmos config")
}

fn refine_config() -> RefineConfig {
    RefineConfig::from_mkrefine_namelist(
        "&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=2\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_cal_type='bbox'\n  RL%refine_num_landtypes=.true.\n/\n",
        "landmesh",
        "hex",
    )
    .expect("parse refine config")
}

fn refine_atmos_config() -> RefineConfig {
    RefineConfig::from_mkrefine_namelist(
        "&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n/\n",
        "atmosmesh",
        "tri",
    )
    .expect("parse refine atmos config")
}

fn sample_atmos_simple_source_mesh() -> earthmesh_cli::UnstructuredMesh {
    earthmesh_cli::UnstructuredMesh {
        m_points: vec![
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint {
                lon: 90.0,
                lat: 0.0,
            },
        ],
        w_points: vec![
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint {
                lon: 180.0,
                lat: 0.0,
            },
        ],
        m_to_w: vec![[1, 1, 1], [1, 2, 1], [2, 1, 2]],
        w_to_m: vec![vec![1], vec![1, 2], vec![2, 1]],
        n_w_to_m: vec![1, 2, 2],
    }
}

#[test]
fn refine_loop_execution_dispatches_sources_steps_and_final_handoff_in_fortran_order() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mkgrd_refine_loop_execution_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let base_dir = format!("{}/", root.display());
    let mkgrd = mkgrd_config(&base_dir);
    let refine = refine_config();
    let plan =
        earthmesh_cli::plan_mkgrd_refine_loop_io(&mkgrd, &refine).expect("plan refine loop io");

    fs::create_dir_all(plan.steps[0].refine_loop_input_gridfile.parent().unwrap())
        .expect("create gridfile dir");
    fs::write(
        &plan.steps[0].refine_loop_input_gridfile,
        "initial gridfile",
    )
    .expect("write initial gridfile");

    let mut expected_runtime_state = EarthmeshRuntimeState::new(mkgrd.clone());
    expected_runtime_state
        .record_mesh_counts_for_step(2, 11, 21)
        .expect("record expected runtime counts");
    let mut executor = RecordingExecutor {
        runtime_state: Some(expected_runtime_state),
        ..RecordingExecutor::default()
    };
    let report = earthmesh_cli::run_mkgrd_refine_loop_execution(&plan, &mut executor, None)
        .expect("run refine loop execution");

    assert_eq!(
        executor.events,
        vec![
            format!("source:1:{:?}:0:0:0", MkgrdRefineSource::CalculatedIterZero),
            format!("source:1:{:?}:1:1:1", MkgrdRefineSource::SpecifiedStep),
            "refine:1".to_string(),
            format!("source:2:{:?}:0:0:0", MkgrdRefineSource::CalculatedIterZero),
            "refine:2".to_string(),
        ]
    );
    assert_eq!(report.executed_sources, 3);
    let runtime_state = report
        .runtime_state
        .as_ref()
        .expect("execution report should carry executor runtime state");
    assert_eq!(runtime_state.step, 2);
    assert_eq!(runtime_state.num_mp_step[1], 11);
    assert_eq!(runtime_state.num_wp_step[1], 21);
    assert_eq!(report.executed_refine_steps, 2);
    assert!(!report.ran_final_quality_check);
    assert_eq!(
        report.final_handoff.copied_result_gridfile,
        plan.final_result_gridfile
    );
    assert_eq!(
        fs::read_to_string(&plan.final_result_gridfile).expect("read final result gridfile"),
        "gridfile after step 2"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn refine_loop_execution_runs_final_quality_before_final_handoff_when_enabled() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mkgrd_refine_loop_final_quality_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let base_dir = format!("{}/", root.display());
    let mkgrd = mkgrd_config(&base_dir);
    let mut refine = refine_config();
    refine.spring_global_type = 1;
    let plan =
        earthmesh_cli::plan_mkgrd_refine_loop_io(&mkgrd, &refine).expect("plan refine loop io");

    fs::create_dir_all(plan.steps[0].refine_loop_input_gridfile.parent().unwrap())
        .expect("create gridfile dir");
    fs::write(
        &plan.steps[0].refine_loop_input_gridfile,
        "initial gridfile",
    )
    .expect("write initial gridfile");

    let mut executor = RecordingExecutor::default();
    let report = earthmesh_cli::run_mkgrd_refine_loop_execution(&plan, &mut executor, None)
        .expect("run refine loop execution");

    assert!(report.ran_final_quality_check);
    assert_eq!(executor.events.last(), Some(&"final-quality".to_string()));
    assert_eq!(
        fs::read_to_string(&plan.final_result_gridfile).expect("read final result gridfile"),
        "gridfile after step 2"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn refine_loop_execution_can_generate_final_domain_contain_during_handoff() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mkgrd_refine_loop_execution_final_contain_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let base_dir = format!("{}/", root.display());
    let mkgrd = mkgrd_config(&base_dir);
    let refine = refine_config();
    let plan =
        earthmesh_cli::plan_mkgrd_refine_loop_io(&mkgrd, &refine).expect("plan refine loop io");

    let lon_i = vec![f64::NAN, 0.5, 1.5, 2.5];
    let lat_i = vec![f64::NAN, 1.5, 0.5];
    let lon_vertex = vec![f64::NAN, 0.0, 2.0, 3.0];
    let lat_vertex = vec![f64::NAN, 2.0, 0.0];
    let mut domain_grid = vec![vec![0; lat_i.len()]; lon_i.len()];
    domain_grid[1][1] = 1;
    domain_grid[1][2] = 1;
    domain_grid[2][1] = 1;
    domain_grid[2][2] = 1;
    let payload = earthmesh_cli::select_area_judge_grid_fortran_indexed(
        &domain_grid,
        None,
        &lon_i,
        &lat_i,
        AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: 2,
            maxlat_source: 1,
            minlat_source: 2,
        },
    )
    .expect("select final domain area grid");
    let domain_area = plan.file_dir.join("result/IsInDmArea_grid.nc4");
    earthmesh_cli::write_area_judge_grid_netcdf(&domain_area, &payload)
        .expect("write final domain area grid");

    let mut seaorland = vec![vec![0; lat_i.len()]; lon_i.len()];
    seaorland[1][1] = 1;
    seaorland[1][2] = 1;
    seaorland[2][1] = 0;
    seaorland[2][2] = 1;

    let mut executor = NetcdfGridExecutor {
        runtime_state: Some(EarthmeshRuntimeState::new(mkgrd.clone())),
        mesh: None,
    };
    let report = earthmesh_cli::run_mkgrd_refine_loop_execution_with_final_domain_contain(
        &plan,
        &mut executor,
        Some(earthmesh_cli::MkgrdFinalDomainContainOptions {
            area_grid_file: &domain_area,
            mesh_kind: earthmesh_cli::GetContainMeshKind::Land,
            seaorland: &seaorland,
            lon_vertex: &lon_vertex,
            lat_vertex: &lat_vertex,
            lon_i: &lon_i,
            lat_i: &lat_i,
            num_vertex: 0,
        }),
        None,
    )
    .expect("run refine loop execution with final domain contain");

    assert!(report.final_handoff.generated_contain.is_some());
    assert_eq!(
        report.final_handoff.contain_domain,
        plan.final_domain_contain_output
    );
    let contain = earthmesh_cli::read_contain_netcdf(&plan.final_domain_contain_output)
        .expect("read generated final domain contain");
    assert_eq!(contain.is_in_area_ustr, vec![0, 1]);
    assert_eq!(contain.ustr_id, vec![vec![0, 0], vec![3, 1]]);
    assert_eq!(contain.ustr_ii, vec![vec![1, 1], vec![1, 2], vec![2, 2]]);
    let runtime_state = report
        .runtime_state
        .as_ref()
        .expect("final contain handoff should preserve runtime state");
    assert_eq!(runtime_state.step, plan.final_mask_postproc_step);
    assert_eq!(
        runtime_state.num_mp_step[plan.final_mask_postproc_step - 1],
        1
    );
    assert_eq!(
        runtime_state.num_wp_step[plan.final_mask_postproc_step - 1],
        3
    );
    assert_eq!(runtime_state.num_vertex, 0);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn refine_loop_final_domain_contain_records_previous_num_vertex_in_runtime_state() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mkgrd_refine_loop_final_contain_num_vertex_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let base_dir = format!("{}/", root.display());
    let mkgrd = mkgrd_config(&base_dir);
    let refine = refine_config();
    let plan =
        earthmesh_cli::plan_mkgrd_refine_loop_io(&mkgrd, &refine).expect("plan refine loop io");

    let lon_i = vec![f64::NAN, 0.5, 1.5, 2.5];
    let lat_i = vec![f64::NAN, 1.5, 0.5];
    let lon_vertex = vec![f64::NAN, 0.0, 2.0, 3.0];
    let lat_vertex = vec![f64::NAN, 2.0, 0.0];
    let mut domain_grid = vec![vec![0; lat_i.len()]; lon_i.len()];
    domain_grid[1][1] = 1;
    domain_grid[1][2] = 1;
    domain_grid[2][1] = 1;
    domain_grid[2][2] = 1;
    let payload = earthmesh_cli::select_area_judge_grid_fortran_indexed(
        &domain_grid,
        None,
        &lon_i,
        &lat_i,
        AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: 2,
            maxlat_source: 1,
            minlat_source: 2,
        },
    )
    .expect("select final domain area grid");
    let domain_area = plan.file_dir.join("result/IsInDmArea_grid.nc4");
    earthmesh_cli::write_area_judge_grid_netcdf(&domain_area, &payload)
        .expect("write final domain area grid");

    let mut seaorland = vec![vec![0; lat_i.len()]; lon_i.len()];
    seaorland[1][1] = 1;
    seaorland[1][2] = 1;
    seaorland[2][1] = 0;
    seaorland[2][2] = 1;

    let mut w_points = Vec::new();
    let mut m_to_w = Vec::new();
    let mut w_to_m = Vec::new();
    let mut n_w_to_m = Vec::new();
    for cell_id in 1..=4 {
        let base = (cell_id - 1) * 3;
        m_to_w.push([base + 1, base + 2, base + 3]);
        for _ in 0..3 {
            w_points.push(earthmesh_cli::LonLatPoint { lon: 1.0, lat: 1.0 });
            w_to_m.push(vec![cell_id]);
            n_w_to_m.push(1);
        }
    }
    let final_mesh = earthmesh_cli::UnstructuredMesh {
        m_points: vec![
            earthmesh_cli::LonLatPoint { lon: 1.0, lat: 1.0 },
            earthmesh_cli::LonLatPoint { lon: 1.1, lat: 1.0 },
            earthmesh_cli::LonLatPoint { lon: 1.2, lat: 1.0 },
            earthmesh_cli::LonLatPoint { lon: 1.3, lat: 1.0 },
        ],
        w_points,
        m_to_w,
        w_to_m,
        n_w_to_m,
    };
    let mut executor = NetcdfGridExecutor {
        runtime_state: Some(EarthmeshRuntimeState::new(mkgrd.clone())),
        mesh: Some(final_mesh),
    };
    let report = earthmesh_cli::run_mkgrd_refine_loop_execution_with_final_domain_contain(
        &plan,
        &mut executor,
        Some(earthmesh_cli::MkgrdFinalDomainContainOptions {
            area_grid_file: &domain_area,
            mesh_kind: earthmesh_cli::GetContainMeshKind::Land,
            seaorland: &seaorland,
            lon_vertex: &lon_vertex,
            lat_vertex: &lat_vertex,
            lon_i: &lon_i,
            lat_i: &lat_i,
            num_vertex: 3,
        }),
        None,
    )
    .expect("run refine loop execution with final domain contain");

    let generated = report
        .final_handoff
        .generated_contain
        .as_ref()
        .expect("final contain should run");
    assert_eq!(generated.runtime_counts.previous_num_vertex, 3);
    let runtime_state = report
        .runtime_state
        .as_ref()
        .expect("final contain handoff should preserve runtime state");
    assert_eq!(runtime_state.step, plan.final_mask_postproc_step);
    assert_eq!(
        runtime_state.num_mp_step[plan.final_mask_postproc_step - 1],
        4
    );
    assert_eq!(
        runtime_state.num_wp_step[plan.final_mask_postproc_step - 1],
        12
    );
    assert_eq!(runtime_state.num_vertex, 3);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn refine_loop_execution_can_run_data_preprocess_final_domain_handoff() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mkgrd_refine_loop_data_preprocess_final_handoff_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let base_dir = format!("{}/", root.display());
    let mkgrd = mkgrd_config(&base_dir);
    let refine = refine_config();
    let plan =
        earthmesh_cli::plan_mkgrd_refine_loop_io(&mkgrd, &refine).expect("plan refine loop io");
    let final_domain_area_grid = plan
        .file_dir
        .join("tmpfile/final_domain_area_grid_from_landtype.nc4");
    let state = earthmesh_cli::MkgrdDataPreprocessSourceState {
        lon_vertex: vec![f64::NAN, 0.0, 2.0, 3.0],
        lat_vertex: vec![f64::NAN, 2.0, 0.0],
        lon_i: vec![f64::NAN, 0.5, 1.5, 2.5],
        lat_i: vec![f64::NAN, 1.5, 0.5],
        gridnum_perdegree: 1,
        nlons_source: 3,
        nlats_source: 2,
        first_triangle_id: 1,
        num_vertex: 3,
        sources: Vec::new(),
        is_in_domain: vec![vec![0, 0, 0], vec![0, 1, 1], vec![0, 1, 1], vec![0, 0, 0]],
        seaorland: vec![vec![0, 0, 0], vec![0, 1, 1], vec![0, 0, 1], vec![0, 0, 0]],
        landtypes_global: vec![vec![0, 0, 0], vec![0, 5, 5], vec![0, 0, 5], vec![0, 0, 0]],
        maxlc: 5,
    };

    let mut executor = NetcdfGridExecutor::default();
    let report =
        earthmesh_cli::run_mkgrd_refine_loop_execution_with_data_preprocess_final_domain_handoff(
            &plan,
            &mut executor,
            &state,
            "LOCmesh",
            &final_domain_area_grid,
            0.0,
            "CoLM",
        )
        .expect("run data_preprocess final-domain handoff");

    assert!(final_domain_area_grid.exists());
    assert!(report.final_handoff.generated_contain.is_some());
    assert_eq!(report.final_handoff.postproc, None);
    let area_payload = earthmesh_cli::read_area_judge_grid_netcdf(&final_domain_area_grid)
        .expect("read data_preprocess final domain area grid");
    assert_eq!(
        area_payload.is_in_area_select,
        vec![vec![1, 1], vec![1, 1], vec![0, 0]]
    );
    let _contain = earthmesh_cli::read_contain_netcdf(&plan.final_domain_contain_output)
        .expect("read generated final domain contain");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn refine_loop_execution_can_run_data_preprocess_atmos_final_postproc() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mkgrd_refine_loop_data_preprocess_atmos_final_handoff_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let base_dir = format!("{}/", root.display());
    let mkgrd = mkgrd_atmos_config(&base_dir);
    let refine = refine_atmos_config();
    let plan =
        earthmesh_cli::plan_mkgrd_refine_loop_io(&mkgrd, &refine).expect("plan refine loop io");
    let final_domain_area_grid = plan
        .file_dir
        .join("tmpfile/final_domain_area_grid_from_landtype.nc4");
    let state = earthmesh_cli::MkgrdDataPreprocessSourceState {
        lon_vertex: vec![f64::NAN, 0.0, 2.0, 3.0],
        lat_vertex: vec![f64::NAN, 2.0, 0.0],
        lon_i: vec![f64::NAN, 0.5, 1.5, 2.5],
        lat_i: vec![f64::NAN, 1.5, 0.5],
        gridnum_perdegree: 1,
        nlons_source: 3,
        nlats_source: 2,
        first_triangle_id: 1,
        num_vertex: 3,
        sources: Vec::new(),
        is_in_domain: vec![vec![0, 0, 0], vec![0, 1, 1], vec![0, 1, 1], vec![0, 0, 0]],
        seaorland: vec![vec![0, 0, 0], vec![0, 1, 1], vec![0, 0, 1], vec![0, 0, 0]],
        landtypes_global: vec![vec![0, 0, 0], vec![0, 5, 5], vec![0, 0, 5], vec![0, 0, 0]],
        maxlc: 5,
    };
    let atmos_mesh = sample_atmos_simple_source_mesh();
    fs::create_dir_all(root.join("case_refine_exec_atmos/result")).expect("create result dir");
    earthmesh_cli::write_cellwidth_netcdf(
        root.join("case_refine_exec_atmos/result/cellwidth_NXP0009_global.nc4"),
        &earthmesh_cli::CellwidthMesh {
            cell_points: atmos_mesh.w_points,
            cellwidth: vec![12.0, 24.0, 48.0],
        },
    )
    .expect("write cellwidth");

    let mut executor = AtmosNetcdfGridExecutor;
    let report =
        earthmesh_cli::run_mkgrd_refine_loop_execution_with_data_preprocess_final_domain_handoff(
            &plan,
            &mut executor,
            &state,
            "atmosmesh",
            &final_domain_area_grid,
            0.0,
            "MPAS-Simple",
        )
        .expect("run data_preprocess atmos final-domain handoff");

    assert!(!final_domain_area_grid.exists());
    assert!(report.final_handoff.generated_contain.is_none());
    match report
        .final_handoff
        .postproc
        .expect("atmos postproc report")
    {
        earthmesh_cli::MkgrdFinalDomainPostprocReport::Atmos(postproc) => {
            assert_eq!(
                postproc.output,
                root.join("case_refine_exec_atmos/result/MPASOUT_NXP0009_global_Simple.nc4")
            );
            assert!(postproc.output.exists());
        }
        other => panic!("expected atmos MPAS-Simple report, got {other:?}"),
    }

    let _ = fs::remove_dir_all(&root);
}
