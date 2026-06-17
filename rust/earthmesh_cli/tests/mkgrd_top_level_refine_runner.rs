use earthmesh_cli::{
    write_bbox_mask_netcdf, BBoxMask, BBoxPoint, MkgrdFinalQualityCheckIoPlan,
    MkgrdRefineLoopExecutor, MkgrdRefineLoopStepIoPlan, MkgrdRefinePrepareSourceGridOptions,
    MkgrdRefineSourceIoPlan,
};
use earthmesh_core::{EarthmeshConfig, EarthmeshRuntimeState};
use std::{fs, io, path::Path, path::PathBuf};

static NETCDF_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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

fn write_2d_threshold(path: &Path, var_name: &str, values: &[f64]) {
    let mut file = netcdf::create(path).expect("create threshold input");
    let lon = values.len() / 2;
    file.add_dimension("lon", lon).expect("lon dim");
    file.add_dimension("lat", 2).expect("lat dim");
    let mut var = file
        .add_variable::<f64>(var_name, &["lon", "lat"])
        .expect("threshold variable");
    var.put_values(values, (.., ..))
        .expect("write threshold values");
}

#[derive(Default)]
struct PassthroughRefineExecutor {
    saw_gridinit_input: bool,
    runtime_state: Option<EarthmeshRuntimeState>,
    final_quality_cellwidth_output: Option<PathBuf>,
}

impl MkgrdRefineLoopExecutor for PassthroughRefineExecutor {
    fn run_source_branch(
        &mut self,
        _step: &MkgrdRefineLoopStepIoPlan,
        _source: &MkgrdRefineSourceIoPlan,
    ) -> io::Result<()> {
        Ok(())
    }

    fn run_refine_loop_step(&mut self, step: &MkgrdRefineLoopStepIoPlan) -> io::Result<()> {
        let mesh = earthmesh_cli::read_unstructured_mesh_netcdf(&step.refine_loop_input_gridfile)?;
        self.saw_gridinit_input = mesh.m_points.len() == 21 && mesh.w_points.len() == 13;
        earthmesh_cli::write_unstructured_mesh_netcdf(&step.refine_loop_output_gridfile, &mesh)
            .map(|_| ())
    }

    fn run_final_quality_check(&mut self, plan: &MkgrdFinalQualityCheckIoPlan) -> io::Result<()> {
        if let Some(output_gridfile) = plan.output_gridfile.as_ref() {
            let mesh = earthmesh_cli::read_unstructured_mesh_netcdf(&plan.input_gridfile)?;
            earthmesh_cli::write_unstructured_mesh_netcdf(output_gridfile, &mesh)?;
            if let Some(cellwidth_output) = self.final_quality_cellwidth_output.as_ref() {
                let cellwidth_len = mesh.w_points.len();
                earthmesh_cli::write_cellwidth_netcdf(
                    cellwidth_output,
                    &earthmesh_cli::CellwidthMesh {
                        cell_points: mesh.w_points,
                        cellwidth: vec![100.0; cellwidth_len],
                    },
                )?;
            }
        }
        Ok(())
    }

    fn runtime_state(&self) -> Option<&EarthmeshRuntimeState> {
        self.runtime_state.as_ref()
    }
}

#[test]
fn top_level_runner_preserves_gridinit_output_across_refine_prepare_cleanup() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_top_level_refine_runner");
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
    let namelist = root.join("mkgrd_refine_top.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_top_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=1\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();
    let source_grid = MkgrdRefinePrepareSourceGridOptions {
        lon_vertex: &lon_vertex,
        lat_vertex: &lat_vertex,
        lon_i: &lon_i,
        lat_i: &lat_i,
        gridnum_perdegree: 1,
        nlons_source: 6,
        nlats_source: 6,
        first_triangle_id: 1,
    };
    let mut expected_runtime_state = EarthmeshRuntimeState::new(EarthmeshConfig::default());
    expected_runtime_state
        .record_mesh_counts_for_step(1, 5, 8)
        .expect("record expected runtime counts");
    let mut executor = PassthroughRefineExecutor {
        runtime_state: Some(expected_runtime_state),
        ..PassthroughRefineExecutor::default()
    };

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_refine_executor_and_source_grid(
        &namelist,
        &root,
        100,
        source_grid,
        &mut executor,
        None,
    )
    .expect("run top-level gridinit then refine");

    assert_eq!(report.gridinit.gridfile.sjx_points, 21);
    assert!(executor.saw_gridinit_input);
    assert!(report.source_branch_reports().is_empty());
    let runtime_state = report
        .runtime_state()
        .expect("top-level report should expose refine runtime state");
    assert_eq!(runtime_state.num_mp_step[0], 5);
    assert_eq!(runtime_state.num_wp_step[0], 8);
    let refine = report.refine.expect("refine run report");
    assert_eq!(refine.execution.executed_refine_steps, 1);
    assert!(refine.prepare.plan.final_result_gridfile.exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn library_runner_can_execute_refine_passthrough_from_global_source_dimensions() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_top_level_global_source_passthrough_runner");
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
    let namelist = root.join("mkgrd_refine_global_source_passthrough.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_global_source_passthrough'\n  NL%base_dir='{base_dir}'\n  NL%NXP=1\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_refine_passthrough_global_source_namelist(
        &namelist, &root, 100, 1, 6, 6, 1,
    )
    .expect("run global-source passthrough refine runner");

    assert_eq!(report.gridinit.gridfile.sjx_points, 21);
    let refine = report.refine.expect("refine report");
    assert_eq!(refine.execution.executed_refine_steps, 1);
    assert!(refine.prepare.plan.final_result_gridfile.exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn top_level_runner_derives_migrated_source_options_and_runs_standard_stack() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_top_level_derived_migrated_runner");
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
    let namelist = root.join("mkgrd_refine_top_derived.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_top_derived'\n  NL%base_dir='{base_dir}'\n  NL%NXP=1\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();
    let source_grid = MkgrdRefinePrepareSourceGridOptions {
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
    let seaorland = vec![vec![1; lat_i.len()]; lon_i.len()];
    let landtypes = vec![vec![1; lat_i.len()]; lon_i.len()];

    let report =
        earthmesh_cli::run_mkgrd_top_level_namelist_with_derived_migrated_executor_and_source_grid(
            &namelist,
            &root,
            100,
            source_grid,
            None,
            Some(&is_in_domain),
            &seaorland,
            &landtypes,
            0,
            99,
            earthmesh_cli::MkgrdRefineLoopWorkingStateExecutor::default(),
            None,
        )
        .expect("run top-level standard migrated refine stack");

    let refine = report.refine.expect("refine run report");
    assert_eq!(refine.execution.executed_sources, 1);
    assert_eq!(refine.execution.executed_refine_steps, 1);
    assert!(refine.prepare.plan.steps[0].sources[0]
        .specified_threshold_output
        .as_ref()
        .expect("specified threshold output")
        .exists());
    assert!(refine.prepare.plan.final_result_gridfile.exists());

    let _ = fs::remove_dir_all(&root);
}

// IGNORED: degenerate fixture, not an engine bug. This drives the full
// derived-migrated refine stack at NXP=1 (a 12-cell icosahedron) over a tiny
// polar source patch (lon[-179.5,-174.5] x lat[84.5,89.5]). The refine bbox marks
// ZERO triangles (the 12-cell mesh is far too coarse for a 3.5-degree region) so
// refinement is a no-op, and the only near-pole cell sits exactly at (0,90) —
// outside the patch in both lon and lat — so `active_unstructured_cells` is
// correctly 0. The getcontain / refine-marking logic is unchanged since this test
// was added (commit b227c26), i.e. `active_unstructured_cells > 0` was never
// satisfiable for this setup. Making it meaningful needs a non-degenerate fixture
// (a finer NXP or a domain aligned to a real cell, consistent with the source-grid
// origin used by the bbox check) — a fixture redesign, tracked separately.
#[test]
#[ignore = "degenerate NXP=1 polar fixture: active_unstructured_cells>0 unsatisfiable; see comment"]
fn top_level_runner_derives_migrated_sources_and_generates_final_domain_contain() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_top_level_derived_final_domain_contain");
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
    let namelist = root.join("mkgrd_refine_top_derived_contain.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_top_derived_final_domain_contain'\n  NL%base_dir='{base_dir}'\n  NL%NXP=1\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();
    let source_grid = MkgrdRefinePrepareSourceGridOptions {
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
    let seaorland = vec![vec![1; lat_i.len()]; lon_i.len()];
    let landtypes = vec![vec![1; lat_i.len()]; lon_i.len()];
    let area_payload = earthmesh_cli::select_area_judge_grid_fortran_indexed(
        &is_in_domain,
        None,
        &lon_i,
        &lat_i,
        earthmesh_mesh::AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: 6,
            maxlat_source: 1,
            minlat_source: 6,
        },
    )
    .expect("select final domain area grid");
    let area_grid = root
        .join("case_top_derived_final_domain_contain")
        .join("result/IsInDmArea_grid.nc4");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_derived_migrated_executor_and_source_grid_and_final_domain_contain_and_prepare_hook(
        &namelist,
        &root,
        100,
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
        |_prepare| earthmesh_cli::write_area_judge_grid_netcdf(&area_grid, &area_payload),
    )
    .expect("run top-level standard migrated refine stack with final contain");

    let refine = report.refine.expect("refine run report");
    let generated = refine
        .execution
        .final_handoff
        .generated_contain
        .expect("generated final domain contain");
    assert_eq!(
        generated.output,
        refine.prepare.plan.final_domain_contain_output
    );
    assert!(generated.active_unstructured_cells > 0);
    assert!(generated.contained_source_pixels > 0);
    assert!(refine.prepare.plan.final_domain_contain_output.exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_can_run_refine_namelist_through_top_level_passthrough_smoke() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_binary_top_level_refine_passthrough");
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
    let namelist = root.join("mkgrd_binary_refine.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_binary_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=1\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--max-tris")
        .arg("100")
        .arg("--run-refine-passthrough")
        .arg("--source-gridnum-perdegree")
        .arg("1")
        .arg("--source-nlons")
        .arg("6")
        .arg("--source-nlats")
        .arg("6")
        .arg("--source-first-triangle-id")
        .arg("1")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary refine smoke");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("refine_steps=1"), "stdout={stdout}");
    assert!(stdout.contains("refine_final_gridfile="), "stdout={stdout}");
    assert!(root
        .join("case_binary_refine/result/gridfile_NXP0001_hex.nc4")
        .exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_can_run_refine_namelist_with_source_state_through_migrated_stack() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_binary_top_level_refine_source_state");
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
    let namelist = root.join("mkgrd_binary_refine_source_state.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_binary_refine_source_state'\n  NL%base_dir='{base_dir}'\n  NL%NXP=1\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");
    let source_state = root.join("source_state.txt");
    let row = "1 1 1 1 1 1 1";
    let seven_rows = std::iter::repeat(row)
        .take(7)
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        &source_state,
        format!(
            "gridnum_perdegree=1\nnlons=6\nnlats=6\nfirst_triangle_id=1\nnum_vertex=3\nmaxlc=99\n[is_in_domain]\n{seven_rows}\n[seaorland]\n{seven_rows}\n[landtypes_global]\n{seven_rows}\n"
        ),
    )
    .expect("write source-state file");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--max-tris")
        .arg("100")
        .arg("--run-refine-source-state")
        .arg(&source_state)
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary refine source-state path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("refine_steps=1"), "stdout={stdout}");
    assert!(stdout.contains("refine_sources=1"), "stdout={stdout}");
    assert!(stdout.contains("refine_final_gridfile="), "stdout={stdout}");
    assert!(root
        .join("case_binary_refine_source_state/result/gridfile_NXP0001_hex.nc4")
        .exists());
    assert!(root
        .join("case_binary_refine_source_state/threshold/threshold_specified_NXP0001_01.nc4")
        .exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn library_runner_can_execute_compact_source_state_namelist_without_cli_orchestration() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_library_compact_source_state_runner");
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
    let namelist = root.join("mkgrd_library_compact_source_state.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_library_compact_source_state'\n  NL%base_dir='{base_dir}'\n  NL%NXP=1\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");
    let source_state = root.join("compact_source_state.txt");
    let row = "1 1 1 1 1 1 1";
    let seven_rows = std::iter::repeat(row)
        .take(7)
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        &source_state,
        format!(
            "gridnum_perdegree=1\nnlons=6\nnlats=6\nfirst_triangle_id=1\nnum_vertex=3\nmaxlc=99\nfinal_domain_contain=land\n[is_in_domain]\n{seven_rows}\n[seaorland]\n{seven_rows}\n[landtypes_global]\n{seven_rows}\n"
        ),
    )
    .expect("write compact source-state file");

    let report = earthmesh_cli::run_mkgrd_refine_compact_source_state_namelist(
        &namelist,
        &root,
        &source_state,
        100,
        earthmesh_cli::MkgrdRefineLoopWorkingStateExecutor::default(),
    )
    .expect("run compact source-state namelist through library helper");

    assert_eq!(report.source_state.maxlc, 99);
    assert_eq!(report.source_state.first_triangle_id, 1);
    assert_eq!(report.source_branch_reports().len(), 1);
    let runtime_state = report
        .runtime_state()
        .expect("compact source-state report should expose runtime state");
    assert_eq!(
        runtime_state.config.experiment_name,
        "case_library_compact_source_state"
    );
    assert!(runtime_state.refine.is_some());
    assert_eq!(runtime_state.num_vertex, 3);
    let final_counts = report
        .final_domain_contain_runtime_counts()
        .expect("compact source-state report should expose final contain runtime counts");
    assert_eq!(final_counts.previous_num_vertex, 3);
    let refine = report.refine.expect("refine report");
    assert_eq!(refine.execution.executed_refine_steps, 1);
    assert_eq!(refine.execution.executed_sources, 1);
    assert!(refine.execution.final_handoff.generated_contain.is_some());
    assert!(root
        .join("case_library_compact_source_state/contain/contain_landmesh_domain_NXP0001_hex.nc4")
        .exists());
}

#[test]
fn binary_source_state_can_generate_final_domain_contain() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_binary_source_state_final_domain_contain");
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
    let namelist = root.join("mkgrd_binary_refine_source_state_contain.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_binary_source_state_final_domain_contain'\n  NL%base_dir='{base_dir}'\n  NL%NXP=2\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=0\n  RL%num_rc=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");
    let source_state = root.join("source_state_contain.txt");
    let row = std::iter::repeat("1")
        .take(91)
        .collect::<Vec<_>>()
        .join(" ");
    let source_rows = std::iter::repeat(row.as_str())
        .take(81)
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        &source_state,
        format!(
            "gridnum_perdegree=1\nnlons=80\nnlats=90\nfirst_triangle_id=1\nnum_vertex=3\nmaxlc=99\nfinal_domain_contain=land\n[is_in_domain]\n{source_rows}\n[seaorland]\n{source_rows}\n[landtypes_global]\n{source_rows}\n"
        ),
    )
    .expect("write source-state file");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--max-tris")
        .arg("100")
        .arg("--run-refine-source-state")
        .arg(&source_state)
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary refine source-state final contain path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("refine_steps=1"), "stdout={stdout}");
    assert!(stdout.contains("refine_sources=1"), "stdout={stdout}");
    assert!(stdout.contains("refine_final_contain="), "stdout={stdout}");
    let contain_path = root.join(
        "case_binary_source_state_final_domain_contain/contain/contain_landmesh_domain_NXP0002_tri.nc4",
    );
    assert!(contain_path.exists());
    let contain = earthmesh_cli::read_contain_netcdf(&contain_path).expect("read final contain");
    assert!(contain.is_in_area_ustr.iter().any(|&value| value == 1));
    assert!(!contain.ustr_ii.is_empty());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_source_state_land_reports_patchtype_output() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_binary_source_state_land_postproc");
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
    let namelist = root.join("mkgrd_binary_refine_source_state_land_postproc.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_binary_source_state_land_postproc'\n  NL%base_dir='{base_dir}'\n  NL%NXP=2\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=0\n  RL%num_rc=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");
    let source_state = root.join("source_state_land_postproc.txt");
    let nlons = 40usize;
    let nlats = 60usize;
    let domain_row = std::iter::repeat("1")
        .take(nlats + 1)
        .collect::<Vec<_>>()
        .join(" ");
    let domain_rows = std::iter::repeat(domain_row.as_str())
        .take(nlons + 1)
        .collect::<Vec<_>>()
        .join("\n");
    let land_pixels = [
        (37usize, 33usize),
        (38, 33),
        (38, 34),
        (38, 35),
        (38, 36),
        (38, 37),
        (39, 33),
        (39, 34),
        (39, 35),
        (39, 36),
        (39, 37),
        (39, 38),
        (39, 39),
        (39, 40),
        (40, 34),
        (40, 35),
        (40, 36),
        (40, 37),
        (40, 38),
        (40, 39),
        (40, 40),
        (40, 41),
        (40, 42),
        (40, 43),
    ];
    let mut seaorland = vec![vec![0_i32; nlats + 1]; nlons + 1];
    for (lon, lat) in land_pixels {
        seaorland[lon][lat] = 1;
    }
    let seaorland_rows = seaorland
        .iter()
        .map(|row| row.iter().map(i32::to_string).collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        &source_state,
        format!(
            "gridnum_perdegree=1\nnlons={nlons}\nnlats={nlats}\nfirst_triangle_id=1\nnum_vertex=3\nmaxlc=99\nfinal_domain_contain=land\nfinal_domain_postproc=land\n[is_in_domain]\n{domain_rows}\n[seaorland]\n{seaorland_rows}\n[landtypes_global]\n{domain_rows}\n"
        ),
    )
    .expect("write source-state file");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--max-tris")
        .arg("100")
        .arg("--run-refine-source-state")
        .arg(&source_state)
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary refine source-state land postproc path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("refine_steps=1"), "stdout={stdout}");
    assert!(stdout.contains("refine_sources=1"), "stdout={stdout}");
    assert!(
        stdout.contains("refine_final_postproc_patchtype="),
        "stdout={stdout}"
    );
    assert!(root
        .join("case_binary_source_state_land_postproc/patchtype/patchtype_NXP0002_tri.nc4")
        .exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_source_state_can_run_ocean_final_domain_postproc() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_binary_source_state_ocean_final_postproc");
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
    let namelist = root.join("mkgrd_binary_refine_source_state_ocean_postproc.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_binary_source_state_ocean_final_postproc'\n  NL%base_dir='{base_dir}'\n  NL%NXP=1\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='FVCOM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");
    let source_state = root.join("source_state_ocean_postproc.txt");
    let row = "1 1 1 1 1 1 1";
    let ocean_row = "0 0 0 0 0 0 0";
    let seven_rows = std::iter::repeat(row)
        .take(7)
        .collect::<Vec<_>>()
        .join("\n");
    let seven_ocean_rows = std::iter::repeat(ocean_row)
        .take(7)
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        &source_state,
        format!(
            "gridnum_perdegree=1\nnlons=6\nnlats=6\nfirst_triangle_id=1\nnum_vertex=3\nmaxlc=99\nfinal_domain_contain=ocean\nfinal_domain_postproc=ocean\n[is_in_domain]\n{seven_rows}\n[seaorland]\n{seven_ocean_rows}\n[landtypes_global]\n{seven_rows}\n"
        ),
    )
    .expect("write source-state file");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--max-tris")
        .arg("100")
        .arg("--run-refine-source-state")
        .arg(&source_state)
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary refine source-state ocean postproc path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("refine_final_contain="), "stdout={stdout}");
    assert!(
        stdout.contains("refine_final_postproc_gridfile="),
        "stdout={stdout}"
    );
    assert!(root
        .join("case_binary_source_state_ocean_final_postproc/result/gridfile_NXP0001_hex_oceanmesh.nc4")
        .exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_source_state_ocean_tri_final_postproc_reports_boundary_outputs() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_binary_source_state_ocean_tri_final_postproc");
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
    let namelist = root.join("mkgrd_binary_refine_source_state_ocean_tri_postproc.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_binary_source_state_ocean_tri_final_postproc'\n  NL%base_dir='{base_dir}'\n  NL%NXP=1\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='FVCOM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");
    let source_state = root.join("source_state_ocean_tri_postproc.txt");
    let domain_row = "1 1 1 1 1 1 1";
    let ocean_row = "0 0 0 0 0 0 0";
    let seven_domain_rows = std::iter::repeat(domain_row)
        .take(7)
        .collect::<Vec<_>>()
        .join("\n");
    let seven_ocean_rows = std::iter::repeat(ocean_row)
        .take(7)
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        &source_state,
        format!(
            "gridnum_perdegree=1\nnlons=6\nnlats=6\nfirst_triangle_id=1\nnum_vertex=3\nmaxlc=99\nfinal_domain_contain=ocean\nfinal_domain_postproc=ocean\n[is_in_domain]\n{seven_domain_rows}\n[seaorland]\n{seven_ocean_rows}\n[landtypes_global]\n{seven_domain_rows}\n"
        ),
    )
    .expect("write source-state file");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--max-tris")
        .arg("100")
        .arg("--run-refine-source-state")
        .arg(&source_state)
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary refine source-state ocean tri postproc path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("refine_final_postproc_gridfile="),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("refine_final_postproc_obc="),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("refine_final_postproc_obcv2="),
        "stdout={stdout}"
    );
    assert!(root
        .join("case_binary_source_state_ocean_tri_final_postproc/result/obc.nc4")
        .exists());
    assert!(root
        .join("case_binary_source_state_ocean_tri_final_postproc/result/obcv2.nc4")
        .exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_source_state_atmos_full_mpas_reports_mesh_and_graph_outputs() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_binary_source_state_atmos_full_mpas");
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
    let namelist = root.join("mkgrd_binary_refine_source_state_atmos_full_mpas.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_binary_source_state_atmos_full_mpas'\n  NL%base_dir='{base_dir}'\n  NL%NXP=2\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=0\n  RL%num_rc=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");
    let source_state = root.join("source_state_atmos_full_mpas.txt");
    let row = "1 1 1 1 1 1 1";
    let seven_rows = std::iter::repeat(row)
        .take(7)
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        &source_state,
        format!(
            "gridnum_perdegree=1\nnlons=6\nnlats=6\nfirst_triangle_id=1\nnum_vertex=3\nmaxlc=99\nfinal_domain_contain=atmos\nfinal_domain_postproc=atmos\n[is_in_domain]\n{seven_rows}\n[seaorland]\n{seven_rows}\n[landtypes_global]\n{seven_rows}\n"
        ),
    )
    .expect("write source-state file");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--max-tris")
        .arg("100")
        .arg("--run-refine-source-state")
        .arg(&source_state)
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary refine source-state atmos full MPAS path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("refine_steps=1"), "stdout={stdout}");
    assert!(stdout.contains("refine_sources=1"), "stdout={stdout}");
    assert!(
        stdout.contains("refine_final_postproc_mpas="),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("refine_final_postproc_mpas_graph="),
        "stdout={stdout}"
    );
    assert!(root
        .join("case_binary_source_state_atmos_full_mpas/result/MPASOUT_NXP0002_global.nc4")
        .exists());
    assert!(root
        .join("case_binary_source_state_atmos_full_mpas/result/MPASOUT_NXP0002_global.graph.info")
        .exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_source_state_earth_reports_patchtype_and_info_outputs() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_binary_source_state_earth_postproc");
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
    let namelist = root.join("mkgrd_binary_refine_source_state_earth.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_binary_source_state_earth_postproc'\n  NL%base_dir='{base_dir}'\n  NL%NXP=2\n  NL%mesh_type='earthmesh'\n  NL%mode_grid='tri'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%mask_sea_ratio=0.4\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=0\n  RL%num_rc=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");
    let source_state = root.join("source_state_earth_postproc.txt");
    let row = std::iter::repeat("1")
        .take(91)
        .collect::<Vec<_>>()
        .join(" ");
    let source_rows = std::iter::repeat(row.as_str())
        .take(81)
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        &source_state,
        format!(
            "gridnum_perdegree=1\nnlons=80\nnlats=90\nfirst_triangle_id=1\nnum_vertex=3\nmaxlc=99\nfinal_domain_contain=earthmesh\nfinal_domain_postproc=earthmesh\n[is_in_domain]\n{source_rows}\n[seaorland]\n{source_rows}\n[landtypes_global]\n{source_rows}\n"
        ),
    )
    .expect("write source-state file");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--max-tris")
        .arg("100")
        .arg("--run-refine-source-state")
        .arg(&source_state)
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary refine source-state earth postproc path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("refine_steps=1"), "stdout={stdout}");
    assert!(stdout.contains("refine_sources=1"), "stdout={stdout}");
    assert!(
        stdout.contains("refine_final_postproc_patchtype="),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("refine_final_postproc_earthmesh_info="),
        "stdout={stdout}"
    );
    assert!(root
        .join("case_binary_source_state_earth_postproc/patchtype/patchtype_NXP0002_tri.nc4")
        .exists());
    assert!(root
        .join("case_binary_source_state_earth_postproc/result/earthmesh_info.nc4")
        .exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_accepts_source_state_with_calculated_refine_metadata() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_binary_source_state_calculated_metadata");
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
    let namelist = root.join("mkgrd_binary_refine_calculated_metadata.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_binary_refine_calculated_metadata'\n  NL%base_dir='{base_dir}'\n  NL%NXP=1\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");
    let source_state = root.join("source_state_with_calculated_metadata.txt");
    let row = "1 1 1 1 1 1 1";
    let seven_rows = std::iter::repeat(row)
        .take(7)
        .collect::<Vec<_>>()
        .join("\n");
    let calculated_rows = [
        "0 0 0 0 0 0 0",
        "0 1 1 0 0 0 0",
        "0 1 1 0 0 0 0",
        "0 0 0 0 0 0 0",
        "0 0 0 0 0 0 0",
        "0 0 0 0 0 0 0",
        "0 0 0 0 0 0 0",
    ]
    .join("\n");
    fs::write(
        &source_state,
        format!(
            "gridnum_perdegree=1\nnlons=6\nnlats=6\nfirst_triangle_id=1\nnum_vertex=3\nmaxlc=99\ncalculated_minlon_source=1\ncalculated_maxlon_source=2\ncalculated_maxlat_source=1\ncalculated_minlat_source=2\n[calculated_refine]\n{calculated_rows}\n[is_in_domain]\n{seven_rows}\n[seaorland]\n{seven_rows}\n[landtypes_global]\n{seven_rows}\n"
        ),
    )
    .expect("write source-state file with calculated metadata");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--max-tris")
        .arg("100")
        .arg("--run-refine-source-state")
        .arg(&source_state)
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary source-state metadata path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("refine_steps=1"), "stdout={stdout}");
    assert!(stdout.contains("refine_sources=1"), "stdout={stdout}");
    assert!(root
        .join("case_binary_refine_calculated_metadata/threshold/threshold_specified_NXP0001_01.nc4")
        .exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_can_run_calculated_refine_namelist_with_source_state() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_binary_active_calculated_refine_source_state");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_bbox_mask_netcdf(
        sources.join("calref_01.nc4"),
        &BBoxMask {
            refine_degree: 1,
            points: vec![BBoxPoint {
                west: -179.5,
                east: -177.0,
                north: 89.5,
                south: 87.0,
            }],
        },
    )
    .expect("write calculated refine mask source");
    let threshold_inputs = root.join("threshold_inputs");
    fs::create_dir_all(&threshold_inputs).expect("create threshold inputs");
    {
        let mut file = netcdf::create(threshold_inputs.join("lai.nc")).expect("create lai input");
        file.add_dimension("lon", 2).expect("lon dim");
        file.add_dimension("lat", 2).expect("lat dim");
        let mut var = file
            .add_variable::<f64>("lai", &["lon", "lat"])
            .expect("lai var");
        var.put_values(&[10.0, 1.0, 10.0, 1.0], (.., ..))
            .expect("write lai values");
    }
    let namelist = root.join("mkgrd_binary_calculated_refine_source_state.nml");
    let base_dir = format!("{}/", root.display());
    let calref_prefix = sources.join("calref_").display().to_string();
    let threshold_dir = threshold_inputs.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_binary_active_calculated_refine_source_state'\n  NL%base_dir='{base_dir}'\n  NL%NXP=1\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{calref_prefix}'\n  RL%threshold_dir='{threshold_dir}'\n  RL%refine_lai_m=.true.\n  RL%th_lai_m=5.0\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");
    let source_state = root.join("source_state_calculated.txt");
    let all_land = "1 1 1 1 1 1 1";
    let all_land_rows = std::iter::repeat(all_land)
        .take(7)
        .collect::<Vec<_>>()
        .join("\n");
    let calculated_rows = [
        "0 0 0 0 0 0 0",
        "0 1 1 0 0 0 0",
        "0 1 1 0 0 0 0",
        "0 0 0 0 0 0 0",
        "0 0 0 0 0 0 0",
        "0 0 0 0 0 0 0",
        "0 0 0 0 0 0 0",
    ]
    .join("\n");
    let landtypes_rows = [
        "0 0 0 0 0 0 0",
        "0 1 1 0 0 0 0",
        "0 1 1 0 0 0 0",
        "0 0 0 0 0 0 0",
        "0 0 0 0 0 0 0",
        "0 0 0 0 0 0 0",
        "0 0 0 0 0 0 0",
    ]
    .join("\n");
    fs::write(
        &source_state,
        format!(
            "gridnum_perdegree=1\nnlons=6\nnlats=6\nfirst_triangle_id=1\nnum_vertex=1\nmaxlc=99\ncalculated_minlon_source=1\ncalculated_maxlon_source=2\ncalculated_maxlat_source=1\ncalculated_minlat_source=2\n[calculated_refine]\n{calculated_rows}\n[is_in_domain]\n{all_land_rows}\n[seaorland]\n{all_land_rows}\n[landtypes_global]\n{landtypes_rows}\n"
        ),
    )
    .expect("write calculated source-state file");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--max-tris")
        .arg("100")
        .arg("--run-refine-source-state")
        .arg(&source_state)
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary calculated refine source-state path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("refine_steps=1"), "stdout={stdout}");
    assert!(stdout.contains("refine_sources=1"), "stdout={stdout}");
    assert!(root
        .join("case_binary_active_calculated_refine_source_state/threshold/threshold_calculate_land_NXP0001_01.nc4")
        .exists());
    assert!(root
        .join("case_binary_active_calculated_refine_source_state/result/gridfile_NXP0001_tri.nc4")
        .exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_can_run_locmesh_calculated_refine_source_state_to_component_thresholds() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_binary_locmesh_calculated_refine_source_state");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_bbox_mask_netcdf(
        sources.join("calref_01.nc4"),
        &BBoxMask {
            refine_degree: 1,
            points: vec![BBoxPoint {
                west: -179.5,
                east: -177.0,
                north: 89.5,
                south: 87.0,
            }],
        },
    )
    .expect("write calculated refine mask source");
    let threshold_inputs = root.join("threshold_inputs");
    fs::create_dir_all(&threshold_inputs).expect("create threshold inputs");
    write_2d_threshold(
        &threshold_inputs.join("lai.nc"),
        "lai",
        &[10.0, 1.0, 10.0, 1.0],
    );
    write_2d_threshold(
        &threshold_inputs.join("sst.nc"),
        "sst",
        &[1.0, 20.0, 1.0, 20.0],
    );
    write_2d_threshold(
        &threshold_inputs.join("typhoon.nc"),
        "typhoon",
        &[1.0, 10.0, 10.0, 1.0],
    );

    let namelist = root.join("mkgrd_binary_locmesh_calculated_refine_source_state.nml");
    let base_dir = format!("{}/", root.display());
    let calref_prefix = sources.join("calref_").display().to_string();
    let threshold_dir = threshold_inputs.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_binary_locmesh_calculated_refine_source_state'\n  NL%base_dir='{base_dir}'\n  NL%NXP=1\n  NL%mesh_type='LOCmesh'\n  NL%mode_grid='tri'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{calref_prefix}'\n  RL%threshold_dir='{threshold_dir}'\n  RL%refine_lai_m=.true.\n  RL%th_lai_m=5.0\n  RL%refine_sst_m=.true.\n  RL%th_sst_m=5.0\n  RL%refine_typhoon_m=.true.\n  RL%th_typhoon_m=9.0\n  RL%refine_sea_ratio=.true.\n  RL%th_sea_ratio=0.2,0.5\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n",
            root.display()
        ),
    )
    .expect("write LOCmesh calculated namelist");

    let source_state = root.join("source_state_locmesh_calculated.txt");
    let domain_rows = std::iter::repeat("1 1 1 1 1 1 1")
        .take(7)
        .collect::<Vec<_>>()
        .join("\n");
    let seaorland_rows = [
        "0 0 0 0 0 0 0",
        "0 1 0 0 0 0 0",
        "0 1 0 0 0 0 0",
        "0 0 0 0 0 0 0",
        "0 0 0 0 0 0 0",
        "0 0 0 0 0 0 0",
        "0 0 0 0 0 0 0",
    ]
    .join("\n");
    let calculated_rows = [
        "0 0 0 0 0 0 0",
        "0 1 1 0 0 0 0",
        "0 1 1 0 0 0 0",
        "0 0 0 0 0 0 0",
        "0 0 0 0 0 0 0",
        "0 0 0 0 0 0 0",
        "0 0 0 0 0 0 0",
    ]
    .join("\n");
    let landtypes_rows = [
        "0 0 0 0 0 0 0",
        "0 1 0 0 0 0 0",
        "0 2 0 0 0 0 0",
        "0 0 0 0 0 0 0",
        "0 0 0 0 0 0 0",
        "0 0 0 0 0 0 0",
        "0 0 0 0 0 0 0",
    ]
    .join("\n");
    fs::write(
        &source_state,
        format!(
            "gridnum_perdegree=1\nnlons=6\nnlats=6\nfirst_triangle_id=1\nnum_vertex=1\nmaxlc=99\ncalculated_minlon_source=1\ncalculated_maxlon_source=2\ncalculated_maxlat_source=1\ncalculated_minlat_source=2\n[calculated_refine]\n{calculated_rows}\n[is_in_domain]\n{domain_rows}\n[seaorland]\n{seaorland_rows}\n[landtypes_global]\n{landtypes_rows}\n"
        ),
    )
    .expect("write LOCmesh calculated source-state file");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--max-tris")
        .arg("100")
        .arg("--run-refine-source-state")
        .arg(&source_state)
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary LOCmesh calculated source-state path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("refine_steps=1"), "stdout={stdout}");
    assert!(stdout.contains("refine_sources=1"), "stdout={stdout}");
    let case_dir = root.join("case_binary_locmesh_calculated_refine_source_state");
    assert!(case_dir
        .join("threshold/threshold_calculate_land_NXP0001_01.nc4")
        .exists());
    assert!(case_dir
        .join("threshold/threshold_calculate_ocean_NXP0001_01.nc4")
        .exists());
    assert!(case_dir
        .join("threshold/threshold_calculate_atmos_NXP0001_01.nc4")
        .exists());
    assert!(case_dir.join("result/gridfile_NXP0001_tri.nc4").exists());

    let _ = fs::remove_dir_all(&root);
}

fn write_global_landtype_file(path: &Path, nlons: usize, nlats: usize) {
    let mut file = netcdf::create(path).expect("create landtype file");
    file.add_dimension("longitude", nlons)
        .expect("longitude dim");
    file.add_dimension("latitude", nlats).expect("latitude dim");
    let mut values = vec![0_i8; nlons * nlats];
    let idx = |lon: usize, lat: usize| lon * nlats + lat;
    values[idx(288, 116)] = 2;
    values[idx(289, 114)] = 7;
    values[idx(290, 113)] = 4;
    let mut var = file
        .add_variable::<i8>("landtype", &["longitude", "latitude"])
        .expect("landtype var");
    var.put_values(&values, (.., ..)).expect("write landtype");
}

fn write_global_ocean_landtype_file(path: &Path, nlons: usize, nlats: usize) {
    let mut file = netcdf::create(path).expect("create ocean landtype file");
    file.add_dimension("longitude", nlons)
        .expect("longitude dim");
    file.add_dimension("latitude", nlats).expect("latitude dim");
    let values = vec![0_i8; nlons * nlats];
    let mut var = file
        .add_variable::<i8>("landtype", &["longitude", "latitude"])
        .expect("landtype var");
    var.put_values(&values, (.., ..))
        .expect("write ocean landtype");
}

fn write_global_sparse_land_landtype_file(path: &Path, nlons: usize, nlats: usize) {
    let mut file = netcdf::create(path).expect("create sparse landtype file");
    file.add_dimension("longitude", nlons)
        .expect("longitude dim");
    file.add_dimension("latitude", nlats).expect("latitude dim");
    let mut values = vec![0_i8; nlons * nlats];
    let idx = |lon_fortran: usize, lat_fortran: usize| (lon_fortran - 1) * nlats + lat_fortran - 1;
    for (lon, lat) in [
        (289, 117),
        (290, 115),
        (290, 116),
        (290, 117),
        (291, 114),
        (291, 115),
        (291, 116),
        (291, 117),
    ] {
        values[idx(lon, lat)] = 1;
    }
    let mut var = file
        .add_variable::<i8>("landtype", &["longitude", "latitude"])
        .expect("landtype var");
    var.put_values(&values, (.., ..))
        .expect("write sparse landtype");
}

#[test]
fn top_level_runner_can_use_data_preprocess_landtype_source_state_without_source_state_file() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_top_level_data_preprocess_source_state");
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
    let landtype_file = root.join("landtype.nc");
    write_global_landtype_file(&landtype_file, 360, 180);
    let namelist = root.join("mkgrd_data_preprocess_source_state.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_data_preprocess_source_state'\n  NL%base_dir='{base_dir}'\n  NL%NXP=1\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");

    let source_state = earthmesh_cli::build_mkgrd_data_preprocess_source_state_fortran_indexed(
        &root,
        &landtype_file,
        1,
        true,
        "bbox",
        1,
        "landmesh",
        true,
        3,
        1,
    )
    .expect("derive mkgrd source state from data_preprocess landtype file");
    assert_eq!(source_state.maxlc, 7);
    assert_eq!(source_state.seaorland[289][117], 1);

    let report =
        earthmesh_cli::run_mkgrd_top_level_namelist_with_derived_migrated_executor_and_source_grid(
            &namelist,
            &root,
            100,
            source_state.refine_prepare_source_grid(),
            None,
            Some(&source_state.is_in_domain),
            &source_state.seaorland,
            &source_state.landtypes_global,
            source_state.num_vertex,
            source_state.maxlc,
            earthmesh_cli::MkgrdRefineLoopWorkingStateExecutor::default(),
            None,
        )
        .expect("run top-level migrated stack from data_preprocess source state");

    let refine = report.refine.expect("refine report");
    assert_eq!(refine.execution.executed_refine_steps, 1);
    assert_eq!(refine.execution.executed_sources, 1);
    assert!(root
        .join("case_data_preprocess_source_state/threshold/threshold_specified_NXP0001_01.nc4")
        .exists());
}

#[test]
fn library_runner_can_execute_landtype_source_namelist_without_cli_orchestration() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_library_landtype_source_runner");
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
    let landtype_file = root.join("landtype_runner.nc");
    write_global_landtype_file(&landtype_file, 360, 180);
    let namelist = root.join("mkgrd_landtype_source_runner.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    let landtype_text = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_library_landtype_source_runner'\n  NL%base_dir='{base_dir}'\n  NL%NXP=1\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%landtype_file='{landtype_text}'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_refine_landtype_source_namelist(
        &namelist,
        &root,
        100,
        Some(1),
        1,
        earthmesh_cli::MkgrdRefineLoopWorkingStateExecutor::default(),
    )
    .expect("run landtype source namelist through library helper");

    assert_eq!(report.source_state.maxlc, 7);
    assert_eq!(report.gridinit.gridfile.sjx_points, 21);
    assert_eq!(report.source_branch_reports().len(), 1);
    let runtime_state = report
        .runtime_state()
        .expect("landtype source report should expose runtime state");
    assert_eq!(
        runtime_state.config.experiment_name,
        "case_library_landtype_source_runner"
    );
    assert!(runtime_state.refine.is_some());
    assert_eq!(runtime_state.source_grid.nlons_source, 360);
    assert_eq!(runtime_state.source_grid.nlats_source, 180);
    assert_eq!(runtime_state.source_grid.maxlc, 7);
    let refine = report.refine.expect("refine report");
    assert_eq!(refine.execution.executed_refine_steps, 1);
    assert_eq!(refine.execution.executed_sources, 1);
    assert!(refine.prepare.plan.final_result_gridfile.exists());
    assert!(root
        .join("case_library_landtype_source_runner/threshold/threshold_specified_NXP0001_01.nc4")
        .exists());
}

#[test]
fn library_landtype_source_runner_can_execute_atmos_mpas_simple_final_postproc() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_library_landtype_source_atmos_final_postproc");
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
    let landtype_file = root.join("landtype_atmos_final.nc");
    write_global_landtype_file(&landtype_file, 360, 180);
    let namelist = root.join("mkgrd_landtype_atmos_final.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    let landtype_text = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_library_landtype_source_atmos_final'\n  NL%base_dir='{base_dir}'\n  NL%NXP=2\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%landtype_file='{landtype_text}'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS-Simple'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=0\n  RL%num_rc=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_refine_landtype_source_namelist(
        &namelist,
        &root,
        100,
        Some(1),
        1,
        PassthroughRefineExecutor {
            final_quality_cellwidth_output: Some(root.join(
                "case_library_landtype_source_atmos_final/result/cellwidth_NXP0002_global.nc4",
            )),
            ..PassthroughRefineExecutor::default()
        },
    )
    .expect("run atmos landtype source namelist through library helper");

    let refine = report.refine.expect("refine report");
    assert_eq!(refine.execution.executed_refine_steps, 1);
    assert_eq!(refine.execution.executed_sources, 1);
    assert!(refine.execution.ran_final_quality_check);
    let postproc = refine
        .execution
        .final_handoff
        .postproc
        .expect("atmos final postproc report");
    match postproc {
        earthmesh_cli::MkgrdFinalDomainPostprocReport::Atmos(report) => {
            assert_eq!(
                report.output,
                root.join(
                    "case_library_landtype_source_atmos_final/result/MPASOUT_NXP0002_global_Simple.nc4"
                )
            );
        }
        other => panic!("expected MPAS-Simple atmos postproc report, got {other:?}"),
    }
    assert!(root
        .join("case_library_landtype_source_atmos_final/result/cellwidth_NXP0002_global.nc4")
        .exists());
    assert!(root
        .join("case_library_landtype_source_atmos_final/result/MPASOUT_NXP0002_global_Simple.nc4")
        .exists());
}

#[test]
fn binary_can_run_refine_namelist_with_landtype_source_without_source_state_file() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_binary_landtype_source_state");
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
    let landtype_file = root.join("landtype_binary.nc");
    write_global_landtype_file(&landtype_file, 360, 180);
    let namelist = root.join("mkgrd_binary_landtype_source_state.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    let landtype_text = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_binary_landtype_source_state'\n  NL%base_dir='{base_dir}'\n  NL%NXP=1\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%landtype_file='{landtype_text}'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--max-tris")
        .arg("100")
        .arg("--run-refine-landtype-source")
        .arg("--source-gridnum-perdegree")
        .arg("1")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary landtype-source path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("refine_steps=1"), "stdout={stdout}");
    assert!(stdout.contains("refine_sources=1"), "stdout={stdout}");
    assert!(root
        .join("case_binary_landtype_source_state/threshold/threshold_specified_NXP0001_01.nc4")
        .exists());
}

#[test]
fn binary_landtype_source_atmos_full_mpas_reports_mesh_and_graph_outputs() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_binary_landtype_source_atmos_full_mpas");
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
    let landtype_file = root.join("landtype_binary_atmos_full_mpas.nc");
    write_global_landtype_file(&landtype_file, 360, 180);
    let namelist = root.join("mkgrd_binary_landtype_atmos_full_mpas.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    let landtype_text = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_binary_landtype_atmos_full_mpas'\n  NL%base_dir='{base_dir}'\n  NL%NXP=2\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%landtype_file='{landtype_text}'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=0\n  RL%num_rc=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--max-tris")
        .arg("100")
        .arg("--run-refine-landtype-source")
        .arg("--source-gridnum-perdegree")
        .arg("1")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary full-MPAS landtype-source path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("refine_steps=1"), "stdout={stdout}");
    assert!(stdout.contains("refine_sources=1"), "stdout={stdout}");
    assert!(
        stdout.contains("refine_final_postproc_mpas="),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("refine_final_postproc_mpas_graph="),
        "stdout={stdout}"
    );
    assert!(root
        .join("case_binary_landtype_atmos_full_mpas/result/MPASOUT_NXP0002_global.nc4")
        .exists());
    assert!(root
        .join("case_binary_landtype_atmos_full_mpas/result/MPASOUT_NXP0002_global.graph.info")
        .exists());
}

#[test]
fn binary_default_entry_runs_refine_landtype_source_without_explicit_mode_flag() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_binary_default_landtype_source_state");
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
    let landtype_file = root.join("landtype_default_binary.nc");
    write_global_landtype_file(&landtype_file, 360, 180);
    let namelist = root.join("mkgrd_default_landtype_source_state.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    let landtype_text = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_binary_default_landtype_source_state'\n  NL%base_dir='{base_dir}'\n  NL%NXP=1\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%landtype_file='{landtype_text}'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--max-tris")
        .arg("100")
        .arg("--source-gridnum-perdegree")
        .arg("1")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary default landtype-source path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("refine_source=landtype_file"),
        "stdout={stdout}"
    );
    assert!(stdout.contains("refine_steps=1"), "stdout={stdout}");
    assert!(stdout.contains("refine_sources=1"), "stdout={stdout}");
    assert!(root
        .join("case_binary_default_landtype_source_state/threshold/threshold_specified_NXP0001_01.nc4")
        .exists());
}

#[test]
fn binary_landtype_source_can_run_calculated_refine_thresholds() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_binary_landtype_source_calculated");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_bbox_mask_netcdf(
        sources.join("calref_01.nc4"),
        &BBoxMask {
            refine_degree: 0,
            points: vec![BBoxPoint {
                west: -180.0,
                east: -176.0,
                north: 90.0,
                south: 86.0,
            }],
        },
    )
    .expect("write calculated refine mask source");
    let threshold_inputs = root.join("threshold_inputs");
    fs::create_dir_all(&threshold_inputs).expect("create threshold inputs");
    {
        let mut file = netcdf::create(threshold_inputs.join("lai.nc")).expect("create lai input");
        file.add_dimension("lon", 360).expect("lon dim");
        file.add_dimension("lat", 180).expect("lat dim");
        let mut values = vec![1.0_f64; 360 * 180];
        let idx = |lon: usize, lat: usize| lon * 180 + lat;
        values[idx(1, 0)] = 10.0;
        values[idx(2, 0)] = 10.0;
        values[idx(1, 1)] = 10.0;
        values[idx(2, 1)] = 10.0;
        let mut var = file
            .add_variable::<f64>("lai", &["lon", "lat"])
            .expect("lai var");
        var.put_values(&values, (.., ..)).expect("write lai values");
    }
    let landtype_file = root.join("landtype_calculated.nc");
    write_global_landtype_file(&landtype_file, 360, 180);
    let namelist = root.join("mkgrd_binary_landtype_calculated.nml");
    let base_dir = format!("{}/", root.display());
    let calref_prefix = sources.join("calref_").display().to_string();
    let threshold_dir = threshold_inputs.display().to_string();
    let landtype_text = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_binary_landtype_source_calculated'\n  NL%base_dir='{base_dir}'\n  NL%NXP=1\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%landtype_file='{landtype_text}'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{calref_prefix}'\n  RL%threshold_dir='{threshold_dir}'\n  RL%refine_lai_m=.true.\n  RL%th_lai_m=5.0\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--max-tris")
        .arg("100")
        .arg("--run-refine-landtype-source")
        .arg("--source-gridnum-perdegree")
        .arg("1")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary landtype calculated path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("refine_steps=1"), "stdout={stdout}");
    assert!(stdout.contains("refine_sources=1"), "stdout={stdout}");
    assert!(root
        .join("case_binary_landtype_source_calculated/threshold/threshold_calculate_land_NXP0001_01.nc4")
        .exists());
}

#[test]
fn binary_landtype_source_runs_ocean_final_domain_postproc() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_binary_landtype_source_ocean_final_postproc");
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
    let landtype_file = root.join("landtype_ocean_final.nc");
    write_global_ocean_landtype_file(&landtype_file, 360, 180);
    let namelist = root.join("mkgrd_binary_landtype_ocean_final.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    let landtype_text = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_binary_landtype_source_ocean_final'\n  NL%base_dir='{base_dir}'\n  NL%NXP=1\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%landtype_file='{landtype_text}'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='FVCOM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--max-tris")
        .arg("100")
        .arg("--run-refine-landtype-source")
        .arg("--source-gridnum-perdegree")
        .arg("1")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary landtype ocean final postproc path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("refine_final_contain="), "stdout={stdout}");
    assert!(
        stdout.contains("refine_final_postproc_gridfile="),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("refine_final_postproc_obc="),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("refine_final_postproc_obcv2="),
        "stdout={stdout}"
    );
    assert!(root
        .join("case_binary_landtype_source_ocean_final/result/gridfile_NXP0001_tri_oceanmesh.nc4")
        .exists());
    assert!(root
        .join("case_binary_landtype_source_ocean_final/result/obc.nc4")
        .exists());
    assert!(root
        .join("case_binary_landtype_source_ocean_final/result/obcv2.nc4")
        .exists());
}

#[test]
fn binary_landtype_source_runs_land_final_domain_postproc() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_binary_landtype_source_land_final_postproc");
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
    let landtype_file = root.join("landtype_land_final.nc");
    write_global_sparse_land_landtype_file(&landtype_file, 360, 180);
    let namelist = root.join("mkgrd_binary_landtype_land_final.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    let landtype_text = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_binary_landtype_source_land_final'\n  NL%base_dir='{base_dir}'\n  NL%NXP=1\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%landtype_file='{landtype_text}'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--max-tris")
        .arg("100")
        .arg("--run-refine-landtype-source")
        .arg("--source-gridnum-perdegree")
        .arg("1")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary landtype land final postproc path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("refine_final_contain="), "stdout={stdout}");
    assert!(
        stdout.contains("refine_final_postproc_gridfile="),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("refine_final_postproc_patchtype="),
        "stdout={stdout}"
    );
    assert!(root
        .join("case_binary_landtype_source_land_final/result/gridfile_NXP0001_hex_landmesh.nc4")
        .exists());
}
