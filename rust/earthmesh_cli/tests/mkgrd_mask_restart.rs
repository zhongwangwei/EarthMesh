use std::fs;
use std::path::PathBuf;

static NETCDF_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn mask_restart_ocean_without_patch_plans_remask_postproc_without_gridinit() {
    let root =
        std::env::temp_dir().join(format!("earthmesh_cli_mask_restart_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");

    let namelist = root.join("mkgrd_restart.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%output_format='FVCOM'\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.false.\n/\n"
        ),
    )
    .expect("write namelist");

    let report =
        earthmesh_cli::plan_mkgrd_mask_restart_namelist(&namelist, &root, 7).expect("restart plan");

    assert!(report.config.mask_restart);
    assert_eq!(report.remask.mesh_type, "oceanmesh");
    assert_eq!(report.remask.file_dir, root.join("case_restart/"));
    assert_eq!(report.remask.step, 8);
    assert!(!report.remask.refine);
    assert_eq!(
        report.remask.action,
        earthmesh_cli::MaskRestartAction::RunMaskPostproc
    );
    assert_eq!(report.runtime_state.config.experiment_name, "case_restart");
    assert!(
        !report.runtime_state.config.refine,
        "mask_restart runtime state should mirror Fortran refine=.false. override"
    );
    assert_eq!(
        report.runtime_state.step, 8,
        "mask_restart runtime state should carry the Fortran remask step max_iter + 1"
    );
    let dispatch_report =
        earthmesh_cli::MkgrdTopLevelDispatchRunReport::MaskRestartPlan(report.clone());
    assert_eq!(
        dispatch_report
            .runtime_state()
            .expect("plan-only restart dispatch should still expose runtime state")
            .config
            .experiment_name,
        "case_restart"
    );
    assert_eq!(
        dispatch_report
            .runtime_state()
            .expect("plan-only restart dispatch should still expose runtime state")
            .step,
        8
    );
    assert!(!report.workspace_plan.remove_existing_file_dir);
    assert!(report.workspace_plan.directories_to_create.is_empty());
    assert!(report.workspace_plan.mask_operations.is_empty());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn run_mask_restart_patch_namelist_executes_patch_mask_make_and_continues_mkgrd() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mask_restart_patch_run_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let source = root.join("patch_source.nc4");
    earthmesh_cli::write_bbox_mask_netcdf(
        &source,
        &earthmesh_cli::BBoxMask {
            refine_degree: 0,
            points: vec![earthmesh_cli::BBoxPoint {
                west: -1.0,
                east: 1.0,
                north: 1.0,
                south: -1.0,
            }],
        },
    )
    .expect("write patch source");

    let namelist = root.join("mkgrd_restart_patch.nml");
    let base_dir = format!("{}/", root.display());
    let source_path = source.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart_patch'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.true.\n  NL%mask_patch_type='bbox'\n  NL%mask_patch_fprefix='{source_path}'\n/\n"
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_mask_restart_patch_namelist(&namelist, &root, 7)
        .expect("run restart patch mask_make");

    assert_eq!(
        report.plan.remask.action,
        earthmesh_cli::MaskRestartAction::ContinueMkgrd
    );
    assert_eq!(report.plan.remask.step, 8);
    assert_eq!(report.workspace_mask.mask_reports.len(), 1);
    assert_eq!(report.workspace_mask.mask_counts.mask_patch_ndm[0], 1);
    assert_eq!(
        report.workspace_mask.mask_reports[0].outputs,
        vec![root.join("case_restart_patch/tmpfile/mask_patch_bbox_0_01.nc4")]
    );
    assert!(root
        .join("case_restart_patch/tmpfile/mask_patch_bbox_0_01.nc4")
        .exists());

    let _ = fs::remove_dir_all(&root);
}

fn restart_ocean_source_mesh() -> earthmesh_cli::UnstructuredMesh {
    let mut m_points = vec![earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 }; 8];
    for (idx, point) in m_points.iter_mut().enumerate() {
        point.lon = idx as f64;
        point.lat = idx as f64 * 0.5;
    }
    let mut w_points = vec![earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 }; 14];
    for (idx, point) in w_points.iter_mut().enumerate() {
        point.lon = 100.0 + idx as f64;
        point.lat = 40.0 + idx as f64 * 0.25;
    }
    let mut m_to_w = vec![[1, 1, 1]; 8];
    m_to_w[2] = [10, 11, 2];
    m_to_w[3] = [11, 12, 3];
    m_to_w[4] = [12, 13, 4];
    m_to_w[5] = [13, 10, 5];
    let mut w_to_m = vec![vec![1; 7]; 14];
    w_to_m[2] = vec![2, 1, 1, 1, 1, 1, 1];
    w_to_m[3] = vec![3, 1, 1, 1, 1, 1, 1];
    w_to_m[4] = vec![4, 1, 1, 1, 1, 1, 1];
    w_to_m[5] = vec![5, 1, 1, 1, 1, 1, 1];
    w_to_m[10] = vec![2, 5, 6, 7, 1, 1, 1];
    w_to_m[11] = vec![2, 3, 6, 7, 1, 1, 1];
    w_to_m[12] = vec![3, 4, 6, 7, 1, 1, 1];
    w_to_m[13] = vec![4, 5, 6, 7, 1, 1, 1];
    let mut n_w_to_m = vec![0; 14];
    n_w_to_m[2] = 1;
    n_w_to_m[3] = 1;
    n_w_to_m[4] = 1;
    n_w_to_m[5] = 1;
    n_w_to_m[10] = 5;
    n_w_to_m[11] = 5;
    n_w_to_m[12] = 5;
    n_w_to_m[13] = 5;
    earthmesh_cli::UnstructuredMesh {
        m_points,
        w_points,
        m_to_w,
        w_to_m,
        n_w_to_m,
    }
}

fn restart_land_postproc_source_mesh() -> earthmesh_cli::UnstructuredMesh {
    earthmesh_cli::UnstructuredMesh {
        m_points: vec![
            earthmesh_cli::LonLatPoint {
                lon: -176.497,
                lat: 86.497,
            },
            earthmesh_cli::LonLatPoint {
                lon: -176.497,
                lat: 86.497,
            },
        ],
        w_points: vec![
            earthmesh_cli::LonLatPoint {
                lon: -176.497,
                lat: 86.497,
            },
            earthmesh_cli::LonLatPoint {
                lon: -176.494,
                lat: 86.497,
            },
            earthmesh_cli::LonLatPoint {
                lon: -176.496,
                lat: 86.494,
            },
            earthmesh_cli::LonLatPoint {
                lon: -176.497,
                lat: 86.497,
            },
            earthmesh_cli::LonLatPoint {
                lon: -176.494,
                lat: 86.497,
            },
            earthmesh_cli::LonLatPoint {
                lon: -176.496,
                lat: 86.494,
            },
        ],
        m_to_w: vec![[1, 2, 3], [4, 5, 6]],
        w_to_m: vec![
            vec![1, 1],
            vec![1, 1],
            vec![1, 1],
            vec![2, 2],
            vec![2, 2],
            vec![2, 2],
        ],
        n_w_to_m: vec![2, 2, 2, 2, 2, 2],
    }
}

fn restart_atmos_mpas_simple_source_mesh() -> earthmesh_cli::UnstructuredMesh {
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

fn restart_atmos_mpas_full_source_mesh() -> earthmesh_cli::UnstructuredMesh {
    earthmesh_cli::UnstructuredMesh {
        m_points: vec![
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint { lon: 0.2, lat: 0.2 },
            earthmesh_cli::LonLatPoint { lon: 0.8, lat: 0.2 },
            earthmesh_cli::LonLatPoint { lon: 0.2, lat: 0.8 },
            earthmesh_cli::LonLatPoint { lon: 0.8, lat: 0.8 },
        ],
        w_points: vec![
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint { lon: 1.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 1.0 },
            earthmesh_cli::LonLatPoint { lon: 1.0, lat: 1.0 },
        ],
        m_to_w: vec![
            [1, 1, 1],
            [1, 1, 1],
            [2, 3, 4],
            [2, 3, 5],
            [2, 4, 5],
            [3, 4, 5],
        ],
        w_to_m: vec![
            vec![1],
            vec![1],
            vec![2, 3, 4],
            vec![2, 3, 5],
            vec![2, 4, 5],
            vec![3, 4, 5],
        ],
        n_w_to_m: vec![1, 1, 3, 3, 3, 3],
    }
}

fn write_cellwidth_fixture(path: &std::path::Path, values: &[f64]) {
    let mut file = netcdf::create(path).expect("create cellwidth fixture");
    file.add_dimension("num_dbx", values.len())
        .expect("num_dbx dim");
    let mut var = file
        .add_variable::<f64>("cellwidth", &["num_dbx"])
        .expect("cellwidth var");
    var.put_values(values, ..).expect("cellwidth values");
}

fn prepare_restart_ocean_inputs(root: &PathBuf, case_name: &str, nxp: usize) {
    let case_dir = root.join(case_name);
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(case_dir.join("contain")).expect("create contain dir");
    let io_plan =
        earthmesh_cli::plan_mask_postproc_domain_io(&case_dir, nxp, "tri", "oceanmesh", false)
            .expect("postproc io plan");
    earthmesh_cli::write_unstructured_mesh_netcdf(
        &io_plan.source_gridfile,
        &restart_ocean_source_mesh(),
    )
    .expect("write source gridfile");
    earthmesh_cli::write_contain_netcdf(
        &io_plan.contain_domain,
        &earthmesh_cli::ContainMesh {
            ustr_id: vec![
                vec![0, 0, 1],
                vec![0, 0, 1],
                vec![1, 0, 1],
                vec![1, 0, 1],
                vec![1, 0, 1],
                vec![1, 0, 1],
                vec![0, 0, 1],
                vec![0, 0, 1],
            ],
            ustr_ii: vec![vec![0, 0, 0]],
            is_in_area_ustr: vec![0, -1, 1, 1, 1, 1, -1, -1],
        },
    )
    .expect("write contain domain");
}

#[test]
fn run_mask_restart_ocean_namelist_executes_postproc_outputs() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mask_restart_run_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let case_dir = root.join("case_restart");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(case_dir.join("contain")).expect("create contain dir");

    let namelist = root.join("mkgrd_restart_run.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%output_format='FVCOM'\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.false.\n/\n"
        ),
    )
    .expect("write namelist");

    let io_plan =
        earthmesh_cli::plan_mask_postproc_domain_io(&case_dir, 16, "tri", "oceanmesh", false)
            .expect("postproc io plan");
    earthmesh_cli::write_unstructured_mesh_netcdf(
        &io_plan.source_gridfile,
        &restart_ocean_source_mesh(),
    )
    .expect("write source gridfile");
    earthmesh_cli::write_contain_netcdf(
        &io_plan.contain_domain,
        &earthmesh_cli::ContainMesh {
            ustr_id: vec![
                vec![0, 0, 1],
                vec![0, 0, 1],
                vec![1, 0, 1],
                vec![1, 0, 1],
                vec![1, 0, 1],
                vec![1, 0, 1],
                vec![0, 0, 1],
                vec![0, 0, 1],
            ],
            ustr_ii: vec![vec![0, 0, 0]],
            is_in_area_ustr: vec![0, -1, 1, 1, 1, 1, -1, -1],
        },
    )
    .expect("write contain domain");

    let report = earthmesh_cli::run_mkgrd_mask_restart_ocean_namelist(
        &namelist,
        &root,
        7,
        earthmesh_cli::MaskPostprocOceanRunOptions {
            mask_sea_ratio: 0.5,
            num_vertex: 1,
        },
    )
    .expect("run mask_restart ocean postproc");

    assert_eq!(
        report.plan.remask.action,
        earthmesh_cli::MaskRestartAction::RunMaskPostproc
    );
    assert_eq!(
        report.postproc.final_gridfile.output,
        io_plan.result_gridfile
    );
    assert!(io_plan.result_gridfile.exists());
    assert!(io_plan.obc_output.unwrap().exists());
    assert!(io_plan.obcv2_output.unwrap().exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn library_infers_mask_restart_ocean_num_vertex_from_restart_contain_file() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mask_restart_ocean_num_vertex_api_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    prepare_restart_ocean_inputs(&root, "case_restart_ocean_num_vertex_api", 16);

    let namelist = root.join("mkgrd_restart_ocean_num_vertex_api.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart_ocean_num_vertex_api'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%output_format='FVCOM'\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.false.\n/\n"
        ),
    )
    .expect("write namelist");
    let contents = fs::read_to_string(&namelist).expect("read namelist");
    let config =
        earthmesh_core::EarthmeshConfig::from_mkgrd_namelist(&contents).expect("parse config");

    let num_vertex = earthmesh_cli::infer_mask_restart_ocean_num_vertex_from_config(&config)
        .expect("infer num_vertex from contain file");

    assert_eq!(num_vertex, 1);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_can_run_mask_restart_ocean_postproc_branch() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mask_restart_binary_run_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    prepare_restart_ocean_inputs(&root, "case_restart_binary", 16);

    let namelist = root.join("mkgrd_restart_binary_run.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart_binary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%output_format='FVCOM'\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.false.\n  NL%mask_sea_ratio=0.5\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--run-mask-restart-ocean")
        .arg("--mask-restart-max-iter")
        .arg("7")
        .arg("--mask-postproc-num-vertex")
        .arg("1")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary mask_restart ocean path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("mask_restart_action=RunMaskPostproc"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("mask_postproc_result_gridfile="),
        "stdout={stdout}"
    );
    let case_dir = root.join("case_restart_binary");
    assert!(case_dir.join("result/gridfile_NXP0016_tri.nc4").exists());
    assert!(case_dir.join("result/obc.nc4").exists());
    assert!(case_dir.join("result/obcv2.nc4").exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_mask_restart_ocean_postproc_infers_num_vertex_when_arg_is_omitted() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mask_restart_ocean_inferred_num_vertex_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    prepare_restart_ocean_inputs(&root, "case_restart_ocean_inferred_num_vertex", 16);

    let namelist = root.join("mkgrd_restart_ocean_inferred_num_vertex.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart_ocean_inferred_num_vertex'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%output_format='FVCOM'\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.false.\n  NL%mask_sea_ratio=0.5\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--run-mask-restart-ocean")
        .arg("--mask-restart-max-iter")
        .arg("7")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary mask_restart ocean inferred num_vertex path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("mask_restart_action=RunMaskPostproc"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("mask_postproc_result_gridfile="),
        "stdout={stdout}"
    );
    let case_dir = root.join("case_restart_ocean_inferred_num_vertex");
    assert!(case_dir.join("result/gridfile_NXP0016_tri.nc4").exists());
    assert!(case_dir.join("result/obc.nc4").exists());
    assert!(case_dir.join("result/obcv2.nc4").exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_default_entry_runs_mask_restart_ocean_postproc_branch() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_default_mask_restart_ocean_binary_run_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    prepare_restart_ocean_inputs(&root, "case_default_restart_ocean_binary", 16);

    let namelist = root.join("mkgrd_default_restart_ocean_binary_run.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_default_restart_ocean_binary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%output_format='FVCOM'\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.false.\n  NL%mask_sea_ratio=0.5\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--mask-restart-max-iter")
        .arg("7")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary default mask_restart ocean path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("mask_restart_action=RunMaskPostproc"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("mask_postproc_result_gridfile="),
        "stdout={stdout}"
    );
    let case_dir = root.join("case_default_restart_ocean_binary");
    assert!(case_dir.join("result/gridfile_NXP0016_tri.nc4").exists());
    assert!(case_dir.join("result/obc.nc4").exists());
    assert!(case_dir.join("result/obcv2.nc4").exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_can_run_mask_restart_patch_preprocessing_branch() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mask_restart_patch_binary_run_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let source = root.join("patch_source.nc4");
    earthmesh_cli::write_bbox_mask_netcdf(
        &source,
        &earthmesh_cli::BBoxMask {
            refine_degree: 0,
            points: vec![earthmesh_cli::BBoxPoint {
                west: -1.0,
                east: 1.0,
                north: 1.0,
                south: -1.0,
            }],
        },
    )
    .expect("write patch source");

    let namelist = root.join("mkgrd_restart_patch_binary_run.nml");
    let base_dir = format!("{}/", root.display());
    let source_path = source.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart_patch_binary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.true.\n  NL%mask_patch_type='bbox'\n  NL%mask_patch_fprefix='{source_path}'\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--run-mask-restart-patch")
        .arg("--mask-restart-max-iter")
        .arg("7")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary mask_restart patch path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("mask_restart_action=ContinueMkgrd"),
        "stdout={stdout}"
    );
    assert!(stdout.contains("mask_patch_reports=1"), "stdout={stdout}");
    assert!(root
        .join("case_restart_patch_binary/tmpfile/mask_patch_bbox_0_01.nc4")
        .exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn top_level_dispatch_runs_mask_restart_patch_branch_without_gridinit_error() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_top_dispatch_mask_restart_patch_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let source = root.join("patch_source.nc4");
    earthmesh_cli::write_bbox_mask_netcdf(
        &source,
        &earthmesh_cli::BBoxMask {
            refine_degree: 0,
            points: vec![earthmesh_cli::BBoxPoint {
                west: -1.0,
                east: 1.0,
                north: 1.0,
                south: -1.0,
            }],
        },
    )
    .expect("write patch source");

    let namelist = root.join("mkgrd_top_dispatch_restart_patch.nml");
    let base_dir = format!("{}/", root.display());
    let source_path = source.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_top_dispatch_restart_patch'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.true.\n  NL%mask_patch_type='bbox'\n  NL%mask_patch_fprefix='{source_path}'\n/\n"
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist(&namelist, &root, 100, 7)
        .expect("top-level dispatcher should run mask_restart patch branch");
    let runtime_state = report
        .runtime_state()
        .expect("top-level mask_restart patch dispatch should expose runtime state");
    assert_eq!(
        runtime_state.config.experiment_name,
        "case_top_dispatch_restart_patch"
    );

    let earthmesh_cli::MkgrdTopLevelDispatchRunReport::MaskRestartPatch(patch) = report else {
        panic!("expected mask_restart patch branch, got {report:?}");
    };
    assert_eq!(
        patch.plan.remask.action,
        earthmesh_cli::MaskRestartAction::ContinueMkgrd
    );
    assert_eq!(patch.workspace_mask.mask_counts.mask_patch_ndm[0], 1);
    assert!(root
        .join("case_top_dispatch_restart_patch/tmpfile/mask_patch_bbox_0_01.nc4")
        .exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn top_level_dispatch_runs_mask_restart_ocean_postproc_branch_without_plan_only() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_top_dispatch_mask_restart_ocean_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    prepare_restart_ocean_inputs(&root, "case_top_dispatch_restart_ocean", 16);

    let namelist = root.join("mkgrd_top_dispatch_restart_ocean.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_top_dispatch_restart_ocean'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%output_format='FVCOM'\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.false.\n  NL%mask_sea_ratio=0.5\n/\n"
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist(&namelist, &root, 100, 7)
        .expect("top-level dispatcher should run mask_restart ocean postproc branch");
    let runtime_state = report
        .runtime_state()
        .expect("top-level ocean mask_restart postproc should expose runtime state");
    assert_eq!(
        runtime_state.config.experiment_name,
        "case_top_dispatch_restart_ocean"
    );

    assert!(
        !matches!(
            report,
            earthmesh_cli::MkgrdTopLevelDispatchRunReport::MaskRestartPlan(_)
        ),
        "top-level ocean restart must execute postproc instead of returning a plan-only report"
    );
    let case_dir = root.join("case_top_dispatch_restart_ocean");
    assert!(case_dir.join("result/gridfile_NXP0016_tri.nc4").exists());
    assert!(case_dir.join("result/obc.nc4").exists());
    assert!(case_dir.join("result/obcv2.nc4").exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn top_level_dispatch_runs_non_ocean_mask_restart_area_judge_continuation_without_plan_only() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_top_dispatch_mask_restart_area_judge_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let case_dir = root.join("case_top_dispatch_restart_area_judge");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    earthmesh_cli::write_area_judge_grid_netcdf(
        &restart_input,
        &earthmesh_cli::AreaJudgeGridPayload {
            bounds: earthmesh_mesh::AreaJudgeSourceBounds {
                minlon_source: 2,
                maxlon_source: 3,
                maxlat_source: 2,
                minlat_source: 3,
            },
            longitude: vec![-178.5, -177.5],
            latitude: vec![88.5, 87.5],
            is_in_area_select: vec![vec![1, 1], vec![1, 1]],
            seaorland_select: Some(vec![vec![1, 0], vec![1, 1]]),
        },
    )
    .expect("write restart domain");

    let namelist = root.join("mkgrd_top_dispatch_restart_area_judge.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_top_dispatch_restart_area_judge'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.false.\n/\n"
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist(&namelist, &root, 100, 7)
        .expect("top-level dispatcher should run non-ocean mask_restart Area_judge branch");

    let earthmesh_cli::MkgrdTopLevelDispatchRunReport::MaskRestartAreaJudge(report) = report else {
        panic!("top-level non-ocean restart must execute Area_judge instead of returning a plan");
    };
    assert_eq!(report.restart.area_write.output, restart_input);
    assert_eq!(report.restart.area_write.selected_cells, 4);
    assert!(report.postproc.is_none());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn top_level_dispatch_runs_patch_on_area_judge_final_postproc_from_persisted_contain() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_top_dispatch_patch_area_judge_postproc_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let case_dir = root.join("case_top_dispatch_patch_area_judge_postproc");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(case_dir.join("contain")).expect("create contain dir");
    fs::create_dir_all(case_dir.join("patchtype")).expect("create patchtype dir");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    earthmesh_cli::write_area_judge_grid_netcdf(
        &restart_input,
        &earthmesh_cli::AreaJudgeGridPayload {
            bounds: earthmesh_mesh::AreaJudgeSourceBounds {
                minlon_source: 421,
                maxlon_source: 422,
                maxlat_source: 421,
                minlat_source: 422,
            },
            longitude: vec![-176.495_833_333_333_34, -176.487_5],
            latitude: vec![86.495_833_333_333_34, 86.487_5],
            is_in_area_select: vec![vec![1, 1], vec![1, 1]],
            seaorland_select: Some(vec![vec![1, 0], vec![0, 0]]),
        },
    )
    .expect("write restart domain");
    let io_plan =
        earthmesh_cli::plan_mask_postproc_domain_io(&case_dir, 16, "tri", "landmesh", true)
            .expect("postproc io plan");
    earthmesh_cli::write_unstructured_mesh_netcdf(
        &io_plan.source_gridfile,
        &restart_land_postproc_source_mesh(),
    )
    .expect("write postproc source mesh");
    earthmesh_cli::write_contain_netcdf(
        &io_plan.contain_domain,
        &earthmesh_cli::ContainMesh {
            ustr_id: vec![vec![0, 0], vec![1, 1]],
            ustr_ii: vec![vec![421, 421]],
            is_in_area_ustr: vec![0, 1],
        },
    )
    .expect("write persisted contain boundary");
    let patch_source = root.join("patch_source.nc4");
    earthmesh_cli::write_bbox_mask_netcdf(
        &patch_source,
        &earthmesh_cli::BBoxMask {
            refine_degree: 0,
            points: vec![earthmesh_cli::BBoxPoint {
                west: -10.0,
                east: -9.9,
                north: 10.0,
                south: 9.9,
            }],
        },
    )
    .expect("write patch source");

    let namelist = root.join("mkgrd_top_dispatch_patch_area_judge_postproc.nml");
    let base_dir = format!("{}/", root.display());
    let patch_source = patch_source.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_top_dispatch_patch_area_judge_postproc'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.true.\n  NL%mask_patch_type='bbox'\n  NL%mask_patch_fprefix='{patch_source}'\n/\n"
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist(&namelist, &root, 100, 7)
        .expect("top-level dispatcher should run patch-on Area_judge and final postprocess");

    let earthmesh_cli::MkgrdTopLevelDispatchRunReport::MaskRestartAreaJudge(report) = report else {
        panic!("expected top-level patch-on restart to continue through Area_judge");
    };
    assert_eq!(report.restart.workspace_mask.mask_reports.len(), 1);
    assert!(case_dir.join("tmpfile/mask_patch_bbox_0_01.nc4").exists());
    let postproc = report.postproc.expect("final postproc report");
    assert_eq!(postproc.contain.output, io_plan.contain_domain);
    match postproc.postproc {
        earthmesh_cli::MkgrdFinalDomainPostprocReport::Land(postproc) => {
            assert_eq!(postproc.final_gridfile.output, io_plan.result_gridfile);
            assert!(postproc.patchtype.output.exists());
        }
        other => panic!("expected land postproc report, got {other:?}"),
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn top_level_dispatch_runs_patch_on_ocean_area_judge_final_postproc_from_persisted_contain() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_top_dispatch_patch_ocean_area_judge_postproc_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let case_dir = root.join("case_top_dispatch_patch_ocean_area_judge_postproc");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(case_dir.join("contain")).expect("create contain dir");
    fs::create_dir_all(case_dir.join("tmpfile")).expect("create tmpfile dir");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    earthmesh_cli::write_area_judge_grid_netcdf(
        &restart_input,
        &earthmesh_cli::AreaJudgeGridPayload {
            bounds: earthmesh_mesh::AreaJudgeSourceBounds {
                minlon_source: 421,
                maxlon_source: 422,
                maxlat_source: 421,
                minlat_source: 422,
            },
            longitude: vec![-176.495_833_333_333_34, -176.487_5],
            latitude: vec![86.495_833_333_333_34, 86.487_5],
            is_in_area_select: vec![vec![1, 1], vec![1, 1]],
            seaorland_select: Some(vec![vec![1, 1], vec![1, 1]]),
        },
    )
    .expect("write restart domain");
    let io_plan =
        earthmesh_cli::plan_mask_postproc_domain_io(&case_dir, 16, "tri", "oceanmesh", true)
            .expect("postproc io plan");
    earthmesh_cli::write_unstructured_mesh_netcdf(
        &io_plan.source_gridfile,
        &restart_ocean_source_mesh(),
    )
    .expect("write ocean postproc source mesh");
    earthmesh_cli::write_contain_netcdf(
        &io_plan.contain_domain,
        &earthmesh_cli::ContainMesh {
            ustr_id: vec![
                vec![0, 0, 1],
                vec![0, 0, 1],
                vec![1, 0, 1],
                vec![1, 0, 1],
                vec![1, 0, 1],
                vec![1, 0, 1],
                vec![0, 0, 1],
                vec![0, 0, 1],
            ],
            ustr_ii: vec![vec![421, 421]],
            is_in_area_ustr: vec![0, -1, 1, 1, 1, 1, -1, -1],
        },
    )
    .expect("write persisted contain boundary");
    let patch_source = root.join("patch_source.nc4");
    earthmesh_cli::write_bbox_mask_netcdf(
        &patch_source,
        &earthmesh_cli::BBoxMask {
            refine_degree: 0,
            points: vec![earthmesh_cli::BBoxPoint {
                west: -177.0,
                east: -176.0,
                north: 87.0,
                south: 86.0,
            }],
        },
    )
    .expect("write patch source");

    let namelist = root.join("mkgrd_top_dispatch_patch_ocean_area_judge_postproc.nml");
    let base_dir = format!("{}/", root.display());
    let patch_source = patch_source.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_top_dispatch_patch_ocean_area_judge_postproc'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%output_format='FVCOM'\n  NL%gridnum_perdegree=120\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.true.\n  NL%mask_patch_type='bbox'\n  NL%mask_patch_fprefix='{patch_source}'\n  NL%mask_sea_ratio=0.5\n/\n"
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist(&namelist, &root, 100, 7)
        .expect("top-level dispatcher should run patch-on ocean Area_judge and final postprocess");

    let earthmesh_cli::MkgrdTopLevelDispatchRunReport::MaskRestartAreaJudge(report) = report else {
        panic!("expected top-level patch-on ocean restart to continue through Area_judge");
    };
    let postproc = report.postproc.expect("ocean final postproc report");
    assert_eq!(postproc.contain.output, io_plan.contain_domain);
    assert_eq!(postproc.contain.runtime_counts.previous_num_vertex, 1);
    match postproc.postproc {
        earthmesh_cli::MkgrdFinalDomainPostprocReport::Ocean(postproc) => {
            assert_eq!(postproc.final_gridfile.output, io_plan.result_gridfile);
            assert!(postproc.obc.expect("ocean obc output").output.exists());
            assert!(postproc.obcv2.expect("ocean obcv2 output").output.exists());
        }
        other => panic!("expected ocean postproc report, got {other:?}"),
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn default_restart_dispatch_runs_non_ocean_area_judge_final_postproc_when_num_vertex_is_supplied() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_default_restart_area_judge_postproc_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let case_dir = root.join("case_default_restart_area_judge_postproc");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(case_dir.join("contain")).expect("create contain dir");
    fs::create_dir_all(case_dir.join("patchtype")).expect("create patchtype dir");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    earthmesh_cli::write_area_judge_grid_netcdf(
        &restart_input,
        &earthmesh_cli::AreaJudgeGridPayload {
            bounds: earthmesh_mesh::AreaJudgeSourceBounds {
                minlon_source: 421,
                maxlon_source: 422,
                maxlat_source: 421,
                minlat_source: 422,
            },
            longitude: vec![-176.495_833_333_333_34, -176.487_5],
            latitude: vec![86.495_833_333_333_34, 86.487_5],
            is_in_area_select: vec![vec![1, 1], vec![1, 1]],
            seaorland_select: Some(vec![vec![1, 0], vec![0, 0]]),
        },
    )
    .expect("write restart domain");
    let io_plan =
        earthmesh_cli::plan_mask_postproc_domain_io(&case_dir, 16, "tri", "landmesh", false)
            .expect("postproc io plan");
    earthmesh_cli::write_unstructured_mesh_netcdf(
        &io_plan.source_gridfile,
        &restart_land_postproc_source_mesh(),
    )
    .expect("write postproc source mesh");

    let namelist = root.join("mkgrd_default_restart_area_judge_postproc.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_default_restart_area_judge_postproc'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.false.\n/\n"
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist,
        &root,
        100,
        7,
        None,
        None,
        None,
        1,
        Some(1),
    )
    .expect("default dispatcher should continue through final postprocess");
    let runtime_state = report
        .runtime_state()
        .expect("default mask_restart Area_judge dispatch should expose runtime state");
    assert_eq!(
        runtime_state.config.experiment_name,
        "case_default_restart_area_judge_postproc"
    );
    assert_eq!(
        runtime_state.num_mp_step[0], 2,
        "final Get_Contain(0) should write current mesh cell count back to runtime state"
    );
    assert_eq!(
        runtime_state.num_wp_step[0], 6,
        "final Get_Contain(0) should write current mesh vertex count back to runtime state"
    );

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::Dispatch(
        earthmesh_cli::MkgrdTopLevelDispatchRunReport::MaskRestartAreaJudge(report),
    ) = report
    else {
        panic!("expected default dispatch to run mask_restart Area_judge postproc");
    };
    let postproc = report.postproc.expect("final postproc report");
    assert_eq!(postproc.contain.output, io_plan.contain_domain);
    match postproc.postproc {
        earthmesh_cli::MkgrdFinalDomainPostprocReport::Land(postproc) => {
            assert_eq!(postproc.final_gridfile.output, io_plan.result_gridfile);
            assert!(postproc.patchtype.output.exists());
        }
        other => panic!("expected land postproc report, got {other:?}"),
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn default_restart_dispatch_runs_atmos_mpas_simple_final_postproc_when_num_vertex_is_supplied() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_default_restart_atmos_postproc_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let case_dir = root.join("case_default_restart_atmos_postproc");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(case_dir.join("contain")).expect("create contain dir");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    earthmesh_cli::write_area_judge_grid_netcdf(
        &restart_input,
        &earthmesh_cli::AreaJudgeGridPayload {
            bounds: earthmesh_mesh::AreaJudgeSourceBounds {
                minlon_source: 421,
                maxlon_source: 422,
                maxlat_source: 421,
                minlat_source: 422,
            },
            longitude: vec![-176.495_833_333_333_34, -176.487_5],
            latitude: vec![86.495_833_333_333_34, 86.487_5],
            is_in_area_select: vec![vec![1, 1], vec![1, 1]],
            seaorland_select: Some(vec![vec![1, 0], vec![0, 0]]),
        },
    )
    .expect("write restart domain");
    let gridfile = case_dir.join("result/gridfile_NXP0009_tri.nc4");
    earthmesh_cli::write_unstructured_mesh_netcdf(
        &gridfile,
        &restart_atmos_mpas_simple_source_mesh(),
    )
    .expect("write atmos source gridfile");
    write_cellwidth_fixture(
        &case_dir.join("result/cellwidth_NXP0009_global.nc4"),
        &[12.0, 24.0, 48.0],
    );

    let namelist = root.join("mkgrd_default_restart_atmos_postproc.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_default_restart_atmos_postproc'\n  NL%base_dir='{base_dir}'\n  NL%NXP=9\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='tri'\n  NL%output_format='MPAS-Simple'\n  NL%gridnum_perdegree=120\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.false.\n/\n"
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist,
        &root,
        100,
        7,
        None,
        None,
        None,
        1,
        Some(1),
    )
    .expect("default dispatcher should run atmos final postprocess");

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::Dispatch(
        earthmesh_cli::MkgrdTopLevelDispatchRunReport::MaskRestartAreaJudge(report),
    ) = report
    else {
        panic!("expected default dispatch to run atmos mask_restart Area_judge postproc");
    };
    let postproc = report.postproc.expect("final postproc report");
    assert_eq!(
        postproc.contain.output,
        case_dir.join("contain/contain_atmosmesh_domain_NXP0009_tri.nc4")
    );
    match postproc.postproc {
        earthmesh_cli::MkgrdFinalDomainPostprocReport::Atmos(postproc) => {
            assert_eq!(
                postproc.output,
                case_dir.join("result/MPASOUT_NXP0009_global_Simple.nc4")
            );
            assert!(postproc.output.exists());
        }
        other => panic!("expected atmos postproc report, got {other:?}"),
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn default_restart_dispatch_runs_atmos_mpas_final_postproc_when_num_vertex_is_supplied() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_default_restart_atmos_mpas_postproc_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let case_dir = root.join("case_default_restart_atmos_mpas_postproc");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(case_dir.join("contain")).expect("create contain dir");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    earthmesh_cli::write_area_judge_grid_netcdf(
        &restart_input,
        &earthmesh_cli::AreaJudgeGridPayload {
            bounds: earthmesh_mesh::AreaJudgeSourceBounds {
                minlon_source: 421,
                maxlon_source: 422,
                maxlat_source: 421,
                minlat_source: 422,
            },
            longitude: vec![-176.495_833_333_333_34, -176.487_5],
            latitude: vec![86.495_833_333_333_34, 86.487_5],
            is_in_area_select: vec![vec![1, 1], vec![1, 1]],
            seaorland_select: Some(vec![vec![1, 0], vec![0, 0]]),
        },
    )
    .expect("write restart domain");
    let gridfile = case_dir.join("result/gridfile_NXP0009_hex.nc4");
    let mesh = restart_atmos_mpas_full_source_mesh();
    earthmesh_cli::write_unstructured_mesh_netcdf(&gridfile, &mesh)
        .expect("write atmos source gridfile");
    earthmesh_cli::write_cellwidth_netcdf(
        case_dir.join("result/cellwidth_NXP0009_global.nc4"),
        &earthmesh_cli::CellwidthMesh {
            cell_points: mesh.w_points.clone(),
            cellwidth: vec![100.0; mesh.w_points.len()],
        },
    )
    .expect("write cellwidth");

    let namelist = root.join("mkgrd_default_restart_atmos_mpas_postproc.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_default_restart_atmos_mpas_postproc'\n  NL%base_dir='{base_dir}'\n  NL%NXP=9\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%output_format='MPAS'\n  NL%gridnum_perdegree=120\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.false.\n/\n"
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist,
        &root,
        100,
        7,
        None,
        None,
        None,
        1,
        Some(1),
    )
    .expect("default dispatcher should run full MPAS atmos final postprocess");

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::Dispatch(
        earthmesh_cli::MkgrdTopLevelDispatchRunReport::MaskRestartAreaJudge(report),
    ) = report
    else {
        panic!("expected default dispatch to run atmos mask_restart Area_judge postproc");
    };
    let postproc = report.postproc.expect("final postproc report");
    assert_eq!(
        postproc.contain.output,
        case_dir.join("contain/contain_atmosmesh_domain_NXP0009_hex.nc4")
    );
    match postproc.postproc {
        earthmesh_cli::MkgrdFinalDomainPostprocReport::AtmosFull(postproc) => {
            assert_eq!(
                postproc.mesh.output,
                case_dir.join("result/MPASOUT_NXP0009_global.nc4")
            );
            assert_eq!(
                postproc.graph_info.output,
                case_dir.join("result/MPASOUT_NXP0009_global.graph.info")
            );
            assert!(postproc.mesh.output.exists());
            assert!(postproc.graph_info.output.exists());
        }
        other => panic!("expected full atmos postproc report, got {other:?}"),
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn default_restart_dispatch_infers_non_ocean_area_judge_postproc_num_vertex_from_persisted_contain()
{
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_default_restart_area_judge_infer_num_vertex_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let case_dir = root.join("case_default_restart_area_judge_infer_num_vertex");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(case_dir.join("contain")).expect("create contain dir");
    fs::create_dir_all(case_dir.join("patchtype")).expect("create patchtype dir");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    earthmesh_cli::write_area_judge_grid_netcdf(
        &restart_input,
        &earthmesh_cli::AreaJudgeGridPayload {
            bounds: earthmesh_mesh::AreaJudgeSourceBounds {
                minlon_source: 421,
                maxlon_source: 422,
                maxlat_source: 421,
                minlat_source: 422,
            },
            longitude: vec![-176.495_833_333_333_34, -176.487_5],
            latitude: vec![86.495_833_333_333_34, 86.487_5],
            is_in_area_select: vec![vec![1, 1], vec![1, 1]],
            seaorland_select: Some(vec![vec![1, 0], vec![0, 0]]),
        },
    )
    .expect("write restart domain");
    let io_plan =
        earthmesh_cli::plan_mask_postproc_domain_io(&case_dir, 16, "tri", "landmesh", false)
            .expect("postproc io plan");
    earthmesh_cli::write_unstructured_mesh_netcdf(
        &io_plan.source_gridfile,
        &restart_land_postproc_source_mesh(),
    )
    .expect("write postproc source mesh");
    earthmesh_cli::write_contain_netcdf(
        &io_plan.contain_domain,
        &earthmesh_cli::ContainMesh {
            ustr_id: vec![vec![0, 0], vec![1, 1]],
            ustr_ii: vec![vec![421, 421]],
            is_in_area_ustr: vec![0, 1],
        },
    )
    .expect("write persisted contain boundary");

    let namelist = root.join("mkgrd_default_restart_area_judge_infer_num_vertex.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_default_restart_area_judge_infer_num_vertex'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.false.\n/\n"
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 100, 7, None, None, None, 1, None,
    )
    .expect("default dispatcher should infer num_vertex and run final postprocess");

    let runtime_counts = report
        .final_domain_contain_runtime_counts()
        .expect("default dispatch report should expose final contain runtime counts");
    assert_eq!(runtime_counts.previous_num_vertex, 1);
    let runtime_state = report
        .runtime_state()
        .expect("default dispatch should expose final runtime state");
    assert_eq!(
        runtime_state.num_mp_step[0], 2,
        "default dispatch runtime state should include final Get_Contain(0) cell-count writeback"
    );
    assert_eq!(
        runtime_state.num_wp_step[0], 6,
        "default dispatch runtime state should include final Get_Contain(0) vertex-count writeback"
    );

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::Dispatch(
        earthmesh_cli::MkgrdTopLevelDispatchRunReport::MaskRestartAreaJudge(report),
    ) = report
    else {
        panic!("expected default dispatch to run inferred mask_restart Area_judge postproc");
    };
    let postproc = report.postproc.expect("final postproc report");
    assert_eq!(postproc.contain.output, io_plan.contain_domain);
    assert_eq!(postproc.contain.runtime_counts.previous_num_vertex, 1);
    match postproc.postproc {
        earthmesh_cli::MkgrdFinalDomainPostprocReport::Land(postproc) => {
            assert_eq!(postproc.final_gridfile.output, io_plan.result_gridfile);
            assert!(postproc.patchtype.output.exists());
        }
        other => panic!("expected land postproc report, got {other:?}"),
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn default_restart_dispatch_runs_patch_on_area_judge_final_postproc_from_persisted_contain() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_default_restart_patch_area_judge_postproc_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let case_dir = root.join("case_default_restart_patch_area_judge_postproc");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(case_dir.join("contain")).expect("create contain dir");
    fs::create_dir_all(case_dir.join("patchtype")).expect("create patchtype dir");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    earthmesh_cli::write_area_judge_grid_netcdf(
        &restart_input,
        &earthmesh_cli::AreaJudgeGridPayload {
            bounds: earthmesh_mesh::AreaJudgeSourceBounds {
                minlon_source: 421,
                maxlon_source: 422,
                maxlat_source: 421,
                minlat_source: 422,
            },
            longitude: vec![-176.495_833_333_333_34, -176.487_5],
            latitude: vec![86.495_833_333_333_34, 86.487_5],
            is_in_area_select: vec![vec![1, 1], vec![1, 1]],
            seaorland_select: Some(vec![vec![1, 0], vec![0, 0]]),
        },
    )
    .expect("write restart domain");
    let io_plan =
        earthmesh_cli::plan_mask_postproc_domain_io(&case_dir, 16, "tri", "landmesh", true)
            .expect("postproc io plan");
    earthmesh_cli::write_unstructured_mesh_netcdf(
        &io_plan.source_gridfile,
        &restart_land_postproc_source_mesh(),
    )
    .expect("write postproc source mesh");
    earthmesh_cli::write_contain_netcdf(
        &io_plan.contain_domain,
        &earthmesh_cli::ContainMesh {
            ustr_id: vec![vec![0, 0], vec![1, 1]],
            ustr_ii: vec![vec![421, 421]],
            is_in_area_ustr: vec![0, 1],
        },
    )
    .expect("write persisted contain boundary");
    let patch_source = root.join("patch_source.nc4");
    earthmesh_cli::write_bbox_mask_netcdf(
        &patch_source,
        &earthmesh_cli::BBoxMask {
            refine_degree: 0,
            points: vec![earthmesh_cli::BBoxPoint {
                west: -10.0,
                east: -9.9,
                north: 10.0,
                south: 9.9,
            }],
        },
    )
    .expect("write patch source");

    let namelist = root.join("mkgrd_default_restart_patch_area_judge_postproc.nml");
    let base_dir = format!("{}/", root.display());
    let patch_source = patch_source.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_default_restart_patch_area_judge_postproc'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.true.\n  NL%mask_patch_type='bbox'\n  NL%mask_patch_fprefix='{patch_source}'\n/\n"
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 100, 7, None, None, None, 1, None,
    )
    .expect("default dispatcher should run patch-on Area_judge and final postprocess");

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::Dispatch(
        earthmesh_cli::MkgrdTopLevelDispatchRunReport::MaskRestartAreaJudge(report),
    ) = report
    else {
        panic!("expected default patch-on restart to continue through Area_judge");
    };
    assert_eq!(report.restart.workspace_mask.mask_reports.len(), 1);
    assert!(case_dir.join("tmpfile/mask_patch_bbox_0_01.nc4").exists());
    let postproc = report.postproc.expect("final postproc report");
    assert_eq!(postproc.contain.output, io_plan.contain_domain);
    assert_eq!(postproc.contain.runtime_counts.previous_num_vertex, 1);
    match postproc.postproc {
        earthmesh_cli::MkgrdFinalDomainPostprocReport::Land(postproc) => {
            assert_eq!(postproc.final_gridfile.output, io_plan.result_gridfile);
            assert!(postproc.patchtype.output.exists());
        }
        other => panic!("expected land postproc report, got {other:?}"),
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_default_entry_reports_patch_on_area_judge_final_postproc_outputs() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_binary_default_restart_patch_area_judge_postproc_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let case_dir = root.join("case_binary_default_restart_patch_area_judge_postproc");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(case_dir.join("contain")).expect("create contain dir");
    fs::create_dir_all(case_dir.join("patchtype")).expect("create patchtype dir");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    earthmesh_cli::write_area_judge_grid_netcdf(
        &restart_input,
        &earthmesh_cli::AreaJudgeGridPayload {
            bounds: earthmesh_mesh::AreaJudgeSourceBounds {
                minlon_source: 421,
                maxlon_source: 422,
                maxlat_source: 421,
                minlat_source: 422,
            },
            longitude: vec![-176.495_833_333_333_34, -176.487_5],
            latitude: vec![86.495_833_333_333_34, 86.487_5],
            is_in_area_select: vec![vec![1, 1], vec![1, 1]],
            seaorland_select: Some(vec![vec![1, 0], vec![0, 0]]),
        },
    )
    .expect("write restart domain");
    let io_plan =
        earthmesh_cli::plan_mask_postproc_domain_io(&case_dir, 16, "tri", "landmesh", true)
            .expect("postproc io plan");
    earthmesh_cli::write_unstructured_mesh_netcdf(
        &io_plan.source_gridfile,
        &restart_land_postproc_source_mesh(),
    )
    .expect("write postproc source mesh");
    earthmesh_cli::write_contain_netcdf(
        &io_plan.contain_domain,
        &earthmesh_cli::ContainMesh {
            ustr_id: vec![vec![0, 0], vec![1, 1]],
            ustr_ii: vec![vec![421, 421]],
            is_in_area_ustr: vec![0, 1],
        },
    )
    .expect("write persisted contain boundary");
    let patch_source = root.join("patch_source.nc4");
    earthmesh_cli::write_bbox_mask_netcdf(
        &patch_source,
        &earthmesh_cli::BBoxMask {
            refine_degree: 0,
            points: vec![earthmesh_cli::BBoxPoint {
                west: -10.0,
                east: -9.9,
                north: 10.0,
                south: 9.9,
            }],
        },
    )
    .expect("write patch source");

    let namelist = root.join("mkgrd_binary_default_restart_patch_area_judge_postproc.nml");
    let base_dir = format!("{}/", root.display());
    let patch_source = patch_source.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_binary_default_restart_patch_area_judge_postproc'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.true.\n  NL%mask_patch_type='bbox'\n  NL%mask_patch_fprefix='{patch_source}'\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--mask-restart-max-iter")
        .arg("7")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary default patch-on restart Area_judge postproc path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("mask_patch_reports=1"), "stdout={stdout}");
    assert!(stdout.contains("mask_restart_contain="), "stdout={stdout}");
    assert!(
        stdout.contains("mask_restart_postproc_gridfile="),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("mask_restart_postproc_patchtype="),
        "stdout={stdout}"
    );
    assert!(
        io_plan.contain_domain.exists(),
        "missing contain file {}",
        io_plan.contain_domain.display()
    );
    assert!(
        io_plan.result_gridfile.exists(),
        "missing final gridfile {}",
        io_plan.result_gridfile.display()
    );
    assert!(io_plan.patchtype_output.clone().unwrap().exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_default_entry_reports_inferred_non_ocean_area_judge_final_postproc_outputs() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_binary_default_restart_area_judge_infer_num_vertex_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let case_dir = root.join("case_binary_default_restart_area_judge_infer_num_vertex");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(case_dir.join("contain")).expect("create contain dir");
    fs::create_dir_all(case_dir.join("patchtype")).expect("create patchtype dir");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    earthmesh_cli::write_area_judge_grid_netcdf(
        &restart_input,
        &earthmesh_cli::AreaJudgeGridPayload {
            bounds: earthmesh_mesh::AreaJudgeSourceBounds {
                minlon_source: 421,
                maxlon_source: 422,
                maxlat_source: 421,
                minlat_source: 422,
            },
            longitude: vec![-176.495_833_333_333_34, -176.487_5],
            latitude: vec![86.495_833_333_333_34, 86.487_5],
            is_in_area_select: vec![vec![1, 1], vec![1, 1]],
            seaorland_select: Some(vec![vec![1, 0], vec![0, 0]]),
        },
    )
    .expect("write restart domain");
    let io_plan =
        earthmesh_cli::plan_mask_postproc_domain_io(&case_dir, 16, "tri", "landmesh", false)
            .expect("postproc io plan");
    earthmesh_cli::write_unstructured_mesh_netcdf(
        &io_plan.source_gridfile,
        &restart_land_postproc_source_mesh(),
    )
    .expect("write postproc source mesh");
    earthmesh_cli::write_contain_netcdf(
        &io_plan.contain_domain,
        &earthmesh_cli::ContainMesh {
            ustr_id: vec![vec![0, 0], vec![1, 1]],
            ustr_ii: vec![vec![421, 421]],
            is_in_area_ustr: vec![0, 1],
        },
    )
    .expect("write persisted contain boundary");

    let namelist = root.join("mkgrd_binary_default_restart_area_judge_infer_num_vertex.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_binary_default_restart_area_judge_infer_num_vertex'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.false.\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--mask-restart-max-iter")
        .arg("7")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary default restart Area_judge inferred postproc path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("mask_restart_contain="), "stdout={stdout}");
    assert!(
        stdout.contains("mask_restart_postproc_gridfile="),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("mask_restart_postproc_patchtype="),
        "stdout={stdout}"
    );
    assert!(
        io_plan.contain_domain.exists(),
        "missing contain file {}",
        io_plan.contain_domain.display()
    );
    assert!(
        io_plan.result_gridfile.exists(),
        "missing final gridfile {}",
        io_plan.result_gridfile.display()
    );
    assert!(io_plan.patchtype_output.clone().unwrap().exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_explicit_area_judge_reports_inferred_non_ocean_final_postproc_outputs() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_binary_explicit_area_judge_infer_num_vertex_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let case_dir = root.join("case_binary_explicit_area_judge_infer_num_vertex");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(case_dir.join("contain")).expect("create contain dir");
    fs::create_dir_all(case_dir.join("patchtype")).expect("create patchtype dir");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    earthmesh_cli::write_area_judge_grid_netcdf(
        &restart_input,
        &earthmesh_cli::AreaJudgeGridPayload {
            bounds: earthmesh_mesh::AreaJudgeSourceBounds {
                minlon_source: 421,
                maxlon_source: 422,
                maxlat_source: 421,
                minlat_source: 422,
            },
            longitude: vec![-176.495_833_333_333_34, -176.487_5],
            latitude: vec![86.495_833_333_333_34, 86.487_5],
            is_in_area_select: vec![vec![1, 1], vec![1, 1]],
            seaorland_select: Some(vec![vec![1, 0], vec![0, 0]]),
        },
    )
    .expect("write restart domain");
    let io_plan =
        earthmesh_cli::plan_mask_postproc_domain_io(&case_dir, 16, "tri", "landmesh", false)
            .expect("postproc io plan");
    earthmesh_cli::write_unstructured_mesh_netcdf(
        &io_plan.source_gridfile,
        &restart_land_postproc_source_mesh(),
    )
    .expect("write postproc source mesh");
    earthmesh_cli::write_contain_netcdf(
        &io_plan.contain_domain,
        &earthmesh_cli::ContainMesh {
            ustr_id: vec![vec![0, 0], vec![1, 1]],
            ustr_ii: vec![vec![421, 421]],
            is_in_area_ustr: vec![0, 1],
        },
    )
    .expect("write persisted contain boundary");

    let namelist = root.join("mkgrd_binary_explicit_area_judge_infer_num_vertex.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_binary_explicit_area_judge_infer_num_vertex'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.false.\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--run-mask-restart-area-judge")
        .arg("--mask-restart-max-iter")
        .arg("7")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary explicit restart Area_judge inferred postproc path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("mask_restart_contain="), "stdout={stdout}");
    assert!(
        stdout.contains("mask_restart_postproc_gridfile="),
        "stdout={stdout}"
    );
    assert!(
        io_plan.contain_domain.exists(),
        "missing contain file {}",
        io_plan.contain_domain.display()
    );
    assert!(
        io_plan.result_gridfile.exists(),
        "missing final gridfile {}",
        io_plan.result_gridfile.display()
    );
    assert!(io_plan.patchtype_output.clone().unwrap().exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_explicit_area_judge_source_override_reports_inferred_non_ocean_final_postproc_outputs() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_binary_explicit_area_judge_override_infer_num_vertex_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let case_dir = root.join("case_binary_explicit_area_judge_override_infer_num_vertex");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(case_dir.join("contain")).expect("create contain dir");
    fs::create_dir_all(case_dir.join("patchtype")).expect("create patchtype dir");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    earthmesh_cli::write_area_judge_grid_netcdf(
        &restart_input,
        &earthmesh_cli::AreaJudgeGridPayload {
            bounds: earthmesh_mesh::AreaJudgeSourceBounds {
                minlon_source: 421,
                maxlon_source: 422,
                maxlat_source: 421,
                minlat_source: 422,
            },
            longitude: vec![-176.495_833_333_333_34, -176.487_5],
            latitude: vec![86.495_833_333_333_34, 86.487_5],
            is_in_area_select: vec![vec![1, 1], vec![1, 1]],
            seaorland_select: Some(vec![vec![1, 0], vec![0, 0]]),
        },
    )
    .expect("write restart domain");
    let io_plan =
        earthmesh_cli::plan_mask_postproc_domain_io(&case_dir, 16, "tri", "landmesh", false)
            .expect("postproc io plan");
    earthmesh_cli::write_unstructured_mesh_netcdf(
        &io_plan.source_gridfile,
        &restart_land_postproc_source_mesh(),
    )
    .expect("write postproc source mesh");
    earthmesh_cli::write_contain_netcdf(
        &io_plan.contain_domain,
        &earthmesh_cli::ContainMesh {
            ustr_id: vec![vec![0, 0], vec![1, 1]],
            ustr_ii: vec![vec![421, 421]],
            is_in_area_ustr: vec![0, 1],
        },
    )
    .expect("write persisted contain boundary");

    let namelist = root.join("mkgrd_binary_explicit_area_judge_override_infer_num_vertex.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_binary_explicit_area_judge_override_infer_num_vertex'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.false.\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--run-mask-restart-area-judge")
        .arg("--mask-restart-max-iter")
        .arg("7")
        .arg("--source-gridnum-perdegree")
        .arg("120")
        .arg("--source-nlons")
        .arg("43200")
        .arg("--source-nlats")
        .arg("21600")
        .current_dir(&root)
        .output()
        .expect(
            "run earthmesh_cli binary explicit restart Area_judge override inferred postproc path",
        );

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("mask_restart_contain="), "stdout={stdout}");
    assert!(
        stdout.contains("mask_restart_postproc_gridfile="),
        "stdout={stdout}"
    );
    assert!(
        io_plan.contain_domain.exists(),
        "missing contain file {}",
        io_plan.contain_domain.display()
    );
    assert!(
        io_plan.result_gridfile.exists(),
        "missing final gridfile {}",
        io_plan.result_gridfile.display()
    );
    assert!(io_plan.patchtype_output.clone().unwrap().exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_default_entry_dispatches_mask_restart_patch_branch() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_binary_top_dispatch_mask_restart_patch_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let source = root.join("patch_source.nc4");
    earthmesh_cli::write_bbox_mask_netcdf(
        &source,
        &earthmesh_cli::BBoxMask {
            refine_degree: 0,
            points: vec![earthmesh_cli::BBoxPoint {
                west: -2.0,
                east: 2.0,
                north: 2.0,
                south: -2.0,
            }],
        },
    )
    .expect("write patch source");

    let namelist = root.join("mkgrd_binary_top_dispatch_restart_patch.nml");
    let base_dir = format!("{}/", root.display());
    let source_path = source.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_binary_top_dispatch_restart_patch'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.true.\n  NL%mask_patch_type='bbox'\n  NL%mask_patch_fprefix='{source_path}'\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--mask-restart-max-iter")
        .arg("7")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("mask_restart_action=ContinueMkgrd"),
        "stdout={stdout}"
    );
    assert!(stdout.contains("mask_patch_reports=1"), "stdout={stdout}");
    assert!(root
        .join("case_binary_top_dispatch_restart_patch/tmpfile/mask_patch_bbox_0_01.nc4")
        .exists());

    let _ = fs::remove_dir_all(&root);
}
