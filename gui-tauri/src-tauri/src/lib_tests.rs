use super::*;
use earthmesh_project::{
    default_mask_sea_ratio, CloseBoundaryMode, CoupledMeshConfig, DomainConfig, HydroCoastConfig,
    MeshIntentPreset, ProjectConfig, ProjectLayerRole, RegionShape, ResolutionSpec,
    SpecifiedCloseRefinement, ViolationPolicy, DEFAULT_MIN_ANGLE_DEG, INTENT_PRESETS,
    METHOD_C_MAX_AUTO_REFINE_LEVEL, METHOD_C_MIN_BASE_NXP,
};
use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::{self, Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
};

static RUN_STATE_TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn gui_run_state_rejects_overlap_and_ignores_stale_pid_cleanup() {
    let _guard = RUN_STATE_TEST_LOCK.lock().expect("lock run-state test");
    let first = mesh_process::begin_run().expect("reserve first run");
    let first_id = first.id();
    assert!(mesh_process::begin_run()
        .unwrap_err()
        .contains("already starting"));
    mesh_process::record_running_child(first_id, 101).expect("record first child");
    assert!(mesh_process::begin_run().unwrap_err().contains("PID 101"));
    assert!(mesh_process::record_running_child(first_id, 102)
        .unwrap_err()
        .contains("PID 101"));
    mesh_process::clear_running_child(first_id, 999);
    assert_eq!(mesh_process::running_child_pid(), Some(101));
    mesh_process::clear_running_child(first_id, 101);
    drop(first);

    let second = mesh_process::begin_run().expect("reserve second run");
    let second_id = second.id();
    mesh_process::record_running_child(second_id, 202).expect("record second child");
    mesh_process::clear_running_child(first_id, 101);
    assert_eq!(mesh_process::running_child_pid(), Some(202));
    mesh_process::clear_running_child(second_id, 202);
    drop(second);
    assert_eq!(mesh_process::running_child_pid(), None);
}

#[test]
fn gui_run_directories_are_unique_even_under_an_explicit_output_base() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = env::temp_dir().join(format!(
        "earthmesh_gui_unique_runs_{}_{}",
        process::id(),
        nonce
    ));
    let output_base = root.join("chosen-output");
    let cfg = circle_project("same project");
    let chosen = output_base.to_string_lossy().into_owned();

    let first = mesh_runner::project_run_dir(&cfg, Some(chosen.clone())).expect("first run dir");
    let second = mesh_runner::project_run_dir(&cfg, Some(chosen)).expect("second run dir");

    assert_ne!(first, second);
    let canonical_base = fs::canonicalize(&output_base).unwrap();
    assert!(first.starts_with(&canonical_base));
    assert!(second.starts_with(&canonical_base));
    assert!(first.is_dir() && second.is_dir());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn staged_engine_paths_are_process_specific_and_published_from_a_temp_copy() {
    let first_source = Path::new("/tmp/earthmesh-engine-one");
    let second_source = Path::new("/tmp/earthmesh-engine-two");
    assert_ne!(
        engine::staged_engine_path(first_source, 101),
        engine::staged_engine_path(first_source, 202)
    );
    assert_ne!(
        engine::staged_engine_path(first_source, 101),
        engine::staged_engine_path(second_source, 101),
        "different source binaries must never share a staged cache path"
    );
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = env::temp_dir().join(format!(
        "earthmesh_gui_engine_stage_{}_{}",
        process::id(),
        nonce
    ));
    fs::create_dir_all(&root).unwrap();
    let source = root.join("source-engine");
    let destination = root.join("staged-engine");
    fs::write(&source, b"new-engine").unwrap();
    fs::write(&destination, b"old-engine").unwrap();

    engine::stage_engine_copy(&source, &destination).expect("publish engine copy");

    assert_eq!(fs::read(&destination).unwrap(), b"new-engine");
    assert!(fs::read_dir(&root).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains(".tmp-")
    }));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn bundled_engine_directory_precedes_a_stale_repository_build() {
    let repo = Path::new("/repo");
    let executable =
        Path::new("/Applications/EarthMesh Studio.app/Contents/MacOS/earthmesh_studio");
    let roots = engine::engine_search_roots(repo, Some(executable));

    assert_eq!(
        roots.first().map(PathBuf::as_path),
        Some(Path::new(
            "/Applications/EarthMesh Studio.app/Contents/MacOS"
        ))
    );
    assert_eq!(roots.get(1).map(PathBuf::as_path), Some(repo));
}

#[test]
fn gui_scans_valid_auto_refine_decisions_and_keeps_malformed_artifacts_nonfatal() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = env::temp_dir().join(format!(
        "earthmesh_gui_auto_refine_scan_{}_{}",
        process::id(),
        nonce
    ));
    let pass_two = root.join("quality_auto_refine/pass_2");
    let pass_three = root.join("quality_auto_refine/pass_3");
    fs::create_dir_all(&pass_two).unwrap();
    fs::create_dir_all(&pass_three).unwrap();
    fs::write(
        pass_two.join("auto_refine_decision.json"),
        r#"{
          "schema_version": 1,
          "kind": "earthmesh_auto_refine_decision",
          "pass": 2,
          "decision": "rejected",
          "reason": "candidate regressed",
          "regressions": [{"metric":"aspect_ratio.max","preferred":"lower","baseline":3.0,"candidate":3.5,"delta":0.5}],
          "baseline_gridfile": "/tmp/base.grid",
          "candidate_gridfile": "/tmp/candidate.grid",
          "selected_gridfile": "/tmp/base.grid",
          "baseline_quality_report": "/tmp/base/quality_summary.json",
          "candidate_quality_report": "/tmp/candidate/quality_summary.json",
          "selected_quality_report": "/tmp/base/quality_summary.json",
          "baseline_verdict": "warn",
          "candidate_verdict": "fail",
          "selected_verdict": "warn"
        }"#,
    )
    .unwrap();
    fs::write(
        pass_three.join("auto_refine_decision.json"),
        "{not valid json",
    )
    .unwrap();

    let scan = auto_refine::scan_auto_refine_decisions(&root);
    assert_eq!(scan.decisions.len(), 1);
    assert_eq!(scan.warnings.len(), 1);
    let decision = &scan.decisions[0];
    assert_eq!(decision.schema_version, Some(1));
    assert_eq!(decision.pass, 2);
    assert_eq!(decision.decision, "rejected");
    assert_eq!(decision.regressions[0].metric, "aspect_ratio.max");
    assert_eq!(decision.regressions[0].delta, Some(0.5));
    assert!(decision
        .artifact_path
        .ends_with("auto_refine_decision.json"));
    assert!(scan.warnings[0].contains("parse"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn gui_accepts_legacy_auto_refine_decisions_and_rejects_future_schema_versions() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = env::temp_dir().join(format!(
        "earthmesh_gui_auto_refine_schema_{}_{}",
        process::id(),
        nonce
    ));
    let legacy = root.join("quality_auto_refine/pass_1");
    let future = root.join("quality_auto_refine/pass_2");
    fs::create_dir_all(&legacy).unwrap();
    fs::create_dir_all(&future).unwrap();
    fs::write(
        legacy.join("auto_refine_decision.json"),
        r#"{
          "kind": "earthmesh_auto_refine_decision",
          "pass": 1,
          "decision": "complete",
          "reason": "quality gates passed",
          "regressions": [],
          "baseline_gridfile": null,
          "candidate_gridfile": "/tmp/current.grid",
          "selected_gridfile": "/tmp/current.grid",
          "baseline_quality_report": null,
          "candidate_quality_report": "/tmp/quality_summary.json",
          "selected_quality_report": "/tmp/quality_summary.json",
          "baseline_verdict": null,
          "candidate_verdict": "pass",
          "selected_verdict": "pass"
        }"#,
    )
    .unwrap();
    // Version inspection must happen before typed decoding: a future artifact
    // may legitimately omit or reshape fields required by the v1 DTO.
    fs::write(
        future.join("auto_refine_decision.json"),
        r#"{"schema_version":2,"kind":"earthmesh_auto_refine_decision"}"#,
    )
    .unwrap();

    let scan = auto_refine::scan_auto_refine_decisions(&root);
    assert_eq!(scan.decisions.len(), 1);
    assert_eq!(scan.decisions[0].pass, 1);
    assert_eq!(scan.decisions[0].schema_version, None);
    assert!(scan
        .warnings
        .iter()
        .any(|warning| warning.contains("legacy AutoRefine decision")));
    assert!(scan
        .warnings
        .iter()
        .any(|warning| warning.contains("unsupported AutoRefine decision schema_version 2")));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn gui_stages_relative_hydro_root_as_an_absolute_project_input() {
    let mut cfg = ProjectConfig::from_yaml(&hydrology_yaml("hydro_root")).expect("project");
    cfg.hydro_coast = Some(HydroCoastConfig {
        merit_root: "fixtures/merit".to_string(),
        cama_root: Some("fixtures/cama".to_string()),
        merit_stride: 1,
        r3_width_m: 300.0,
        r2_width_m: 50.0,
    });
    mesh_runner::absolutize_gui_project_inputs(&mut cfg).expect("absolute hydro root");
    let hydro = cfg.hydro_coast.unwrap();
    let root = Path::new(&hydro.merit_root).to_path_buf();
    assert!(root.is_absolute());
    assert!(root.ends_with("fixtures/merit"));
    let cama_root = Path::new(hydro.cama_root.as_deref().unwrap());
    assert!(cama_root.is_absolute());
    assert!(cama_root.ends_with("fixtures/cama"));
}

#[test]
fn gui_absolutizes_every_project_file_before_staging_it_in_the_run_directory() {
    let mut cfg = circle_project("all_relative_inputs");
    cfg.data_layers[0].path = "fixtures/layer.nc".to_string();
    cfg.domain = DomainConfig::Regional {
        shape: RegionShape::Shapefile {
            path: "fixtures/domain.shp".to_string(),
        },
        sea_ratio: None,
    };
    cfg.refinement.specified_close = Some(SpecifiedCloseRefinement {
        path: "fixtures/refine.txt".to_string(),
        boundary: CloseBoundaryMode::Polyline,
    });
    cfg.hydro_coast = Some(HydroCoastConfig {
        merit_root: "fixtures/merit".to_string(),
        cama_root: Some("fixtures/hydro-cama".to_string()),
        merit_stride: 1,
        r3_width_m: 300.0,
        r2_width_m: 50.0,
    });
    cfg.coupling = Some(CoupledMeshConfig {
        cama_root: Some("fixtures/coupling-cama".to_string()),
        ..CoupledMeshConfig::default()
    });

    mesh_runner::absolutize_gui_project_inputs(&mut cfg).expect("absolute project inputs");

    assert!(Path::new(&cfg.data_layers[0].path).is_absolute());
    let DomainConfig::Regional {
        shape: RegionShape::Shapefile { path },
        ..
    } = &cfg.domain
    else {
        panic!("shapefile domain");
    };
    assert!(Path::new(path).is_absolute());
    assert!(Path::new(&cfg.refinement.specified_close.unwrap().path).is_absolute());
    let hydro = cfg.hydro_coast.unwrap();
    assert!(Path::new(&hydro.merit_root).is_absolute());
    assert!(Path::new(hydro.cama_root.as_deref().unwrap()).is_absolute());
    assert!(Path::new(cfg.coupling.unwrap().cama_root.as_deref().unwrap()).is_absolute());
}

#[test]
fn gui_resolves_preset_inputs_from_the_nearest_working_directory_ancestor() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = env::temp_dir().join(format!(
        "earthmesh_gui_ancestor_input_{}_{}",
        process::id(),
        nonce
    ));
    let cwd = root.join("gui-tauri/src-tauri");
    let landtype = root.join("input/landtype_igbp_update.nc");
    fs::create_dir_all(&cwd).unwrap();
    fs::create_dir_all(landtype.parent().unwrap()).unwrap();
    fs::write(&landtype, b"fixture").unwrap();

    let resolved =
        mesh_runner::resolve_gui_input_path(Path::new("input/landtype_igbp_update.nc"), &cwd);
    assert_eq!(resolved, landtype);
    let stale_absolute = cwd.join("input/landtype_igbp_update.nc");
    let unchanged = mesh_runner::resolve_gui_input_path(&stale_absolute, &cwd);
    assert_eq!(unchanged, stale_absolute);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn bundled_gui_resolves_preset_inputs_when_finder_cwd_is_root() {
    let root = env::temp_dir().join(format!(
        "earthmesh_gui_bundled_input_{}_{}",
        process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let landtype = root.join("input/landtype_igbp_update.nc");
    fs::create_dir_all(landtype.parent().unwrap()).unwrap();
    fs::write(&landtype, b"fixture").unwrap();

    let resolved = mesh_runner::resolve_gui_input_path_from(
        Path::new("input/landtype_igbp_update.nc"),
        Path::new("/"),
        &root,
    );

    assert_eq!(resolved, landtype);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn opened_project_paths_are_bound_to_the_project_directory() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = env::temp_dir().join(format!(
        "earthmesh_gui_opened_paths_{}_{}",
        process::id(),
        nonce
    ));
    let project_dir = root.join("projects");
    fs::create_dir_all(&project_dir).unwrap();
    let absolute_missing = root.join("missing/refine.txt");
    let mut cfg = circle_project("opened_paths");
    cfg.data_layers[0].path = "data/layer.nc".to_string();
    cfg.refinement.specified_close = Some(SpecifiedCloseRefinement {
        path: absolute_missing.to_string_lossy().into_owned(),
        boundary: CloseBoundaryMode::Polyline,
    });
    let project_path = project_dir.join("project.yaml");
    fs::write(&project_path, cfg.to_yaml().unwrap()).unwrap();

    let opened = read_project(project_path.to_string_lossy().into_owned()).expect("open project");
    let opened_cfg = ProjectConfig::from_yaml(&opened.yaml).expect("opened yaml");
    assert_eq!(
        Path::new(&opened_cfg.data_layers[0].path),
        project_dir.join("data/layer.nc")
    );
    assert_eq!(
        Path::new(&opened_cfg.refinement.specified_close.unwrap().path),
        absolute_missing
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn failed_kill_keeps_the_running_pid_for_a_retry() {
    let _guard = RUN_STATE_TEST_LOCK.lock().expect("lock run-state test");
    let run = mesh_process::begin_run().expect("reserve run");
    // Keep the fake PID positive after conversion to Linux pid_t. u32::MAX
    // becomes -1 there, and `kill(-1, SIGKILL)` targets every permitted process.
    let impossible_pid = i32::MAX as u32;
    mesh_process::record_running_child(run.id(), impossible_pid).expect("record impossible PID");
    assert!(mesh_process::kill_run().is_err());
    assert_eq!(mesh_process::running_child_pid(), Some(impossible_pid));
    mesh_process::clear_running_child(run.id(), impossible_pid);
    drop(run);
}

#[test]
fn gui_keeps_empty_optional_input_paths_empty() {
    let mut path = String::new();
    mesh_runner::absolutize_input_path(&mut path, Path::new("/tmp"));
    assert!(path.is_empty());
}

#[test]
fn parses_quality_summary_fields() {
    let json = r#"{
        "verdict": "warn",
        "cell_view": "hex",
        "geometry": { "cell_count": 1200, "vertex_count": 640, "edge_count": 1830, "min_angle_deg": 22.5 },
        "topology": {
            "triangle_cell_count": 1200,
            "pentagon_cell_count": 2,
            "hexagon_cell_count": 1188,
            "heptagon_cell_count": 3
        },
        "gates": [
            { "metric": "min_angle_deg", "value": 22.5, "level": "warn" },
            { "metric": "aspect_ratio", "value": 2.0, "level": "pass" },
            { "metric": "not_available", "value": null, "level": "pass" }
        ]
    }"#;
    let q = quality::parse_quality_summary(json, Path::new("/no/such/dir")).unwrap();
    assert_eq!(q.verdict, "warn");
    assert_eq!(q.cell_view, "hex");
    assert_eq!(q.cell_count, 1200);
    assert_eq!(q.vertex_count, 640);
    assert_eq!(q.min_angle_deg, Some(22.5));
    assert_eq!(q.max_angle_deg, None);
    assert!(q.compactness.is_none());
    assert_eq!(q.gates.len(), 3);
    assert_eq!(q.gates[0].level, "warn");
    assert_eq!(q.gates[2].value, None);
    assert!(q.cell_sides.contains(&("triangle".to_string(), 1200)));
    assert!(q.cell_sides.contains(&("pentagon".to_string(), 2)));
    assert!(q.cell_sides.contains(&("hexagon".to_string(), 1188)));
    assert!(q.report_path.is_none());
}

#[test]
fn quality_summary_rejects_missing_or_unknown_verdict() {
    let missing = r#"{
        "cell_view": "hex",
        "geometry": { "cell_count": 1, "vertex_count": 2, "edge_count": 3 },
        "topology": {},
        "gates": []
    }"#;
    assert!(
        quality::parse_quality_summary(missing, Path::new("/unused"))
            .unwrap_err()
            .contains("verdict")
    );

    let unknown = r#"{
        "verdict": "unknown",
        "cell_view": "hex",
        "geometry": { "cell_count": 1, "vertex_count": 2, "edge_count": 3 },
        "topology": {},
        "gates": []
    }"#;
    assert!(
        quality::parse_quality_summary(unknown, Path::new("/unused"))
            .unwrap_err()
            .contains("verdict")
    );
}

#[test]
fn quality_summary_rejects_missing_required_geometry_counts() {
    let json = r#"{
        "verdict": "pass",
        "cell_view": "hex",
        "geometry": { "cell_count": 1, "vertex_count": 2 },
        "topology": {},
        "gates": []
    }"#;
    assert!(quality::parse_quality_summary(json, Path::new("/unused"))
        .unwrap_err()
        .contains("edge_count"));

    let missing_gate_value = r#"{
        "verdict": "pass",
        "cell_view": "hex",
        "geometry": { "cell_count": 1, "vertex_count": 2, "edge_count": 3 },
        "topology": {},
        "gates": [{ "metric": "min_angle_deg", "level": "pass" }]
    }"#;
    assert!(
        quality::parse_quality_summary(missing_gate_value, Path::new("/unused"))
            .unwrap_err()
            .contains("value")
    );
}

struct FailingReader;

impl io::Read for FailingReader {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("synthetic read failure"))
    }
}

#[test]
fn child_output_read_and_join_failures_are_not_silenced() {
    let read_error =
        mesh_runner::read_child_lines(io::BufReader::new(FailingReader), "stdout", |_| Ok(()))
            .unwrap_err();
    assert!(read_error.contains("stdout"));
    assert!(read_error.contains("synthetic read failure"));

    let join_error = mesh_runner::join_output_thread(
        thread::spawn(|| -> Result<(), String> { panic!("synthetic panic") }),
        "stderr",
    )
    .unwrap_err();
    assert!(join_error.contains("stderr"));
    assert!(join_error.contains("panicked"));
}

fn spawn_test_sidecar(stdout: &str, stderr: &str, code: i32) -> Child {
    #[cfg(unix)]
    let mut command = {
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("printf '%s\\n' \"$1\"; printf '%s\\n' \"$2\" >&2; exit \"$3\"")
            .arg("earthmesh-test-sidecar")
            .arg(stdout)
            .arg(stderr)
            .arg(code.to_string());
        command
    };
    #[cfg(windows)]
    let mut command = {
        let mut command = Command::new("cmd");
        command.arg("/C").arg(format!(
            "echo {stdout} & echo {stderr} 1>&2 & exit /B {code}"
        ));
        command
    };
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn synthetic sidecar")
}

#[test]
fn sidecar_success_reports_exit_and_gridfile() {
    let _guard = RUN_STATE_TEST_LOCK.lock().expect("lock run-state test");
    let run = mesh_process::begin_run().expect("reserve run");
    let logs = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&logs);
    let child = spawn_test_sidecar("gridfile=/tmp/gui-success.nc", "sidecar warning", 0);

    let (ok, code, gridfile) =
        mesh_runner::capture_mesh_child_with_logger(child, run.id(), move |line| {
            captured.lock().unwrap().push(line)
        })
        .expect("capture successful sidecar");

    assert!(ok);
    assert_eq!(code, Some(0));
    assert_eq!(gridfile.as_deref(), Some("/tmp/gui-success.nc"));
    assert!(logs
        .lock()
        .unwrap()
        .iter()
        .any(|line| line == "— exited with 0"));
}

#[test]
fn sidecar_nonzero_exit_is_a_completed_failed_result() {
    let _guard = RUN_STATE_TEST_LOCK.lock().expect("lock run-state test");
    let run = mesh_process::begin_run().expect("reserve run");
    let logs = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&logs);
    let child = spawn_test_sidecar("no grid", "synthetic failure", 7);

    let (ok, code, gridfile) =
        mesh_runner::capture_mesh_child_with_logger(child, run.id(), move |line| {
            captured.lock().unwrap().push(line)
        })
        .expect("capture nonzero sidecar");

    assert!(!ok);
    assert_eq!(code, Some(7));
    assert_eq!(gridfile, None);
    let logs = logs.lock().unwrap();
    assert!(logs.iter().any(|line| line == "[stderr] synthetic failure"));
    assert!(logs.iter().any(|line| line == "— exited with 7"));
}

#[test]
fn project_capabilities_expose_authoritative_runtime_limits() {
    let capabilities = project_capabilities().expect("project capabilities");
    assert_eq!(
        capabilities.intent_ids,
        INTENT_PRESETS
            .iter()
            .map(|intent| intent.id().to_string())
            .collect::<Vec<_>>()
    );
    assert_eq!(capabilities.default_sea_ratio, default_mask_sea_ratio());
    assert_eq!(capabilities.default_min_angle_deg, DEFAULT_MIN_ANGLE_DEG);
    assert_eq!(capabilities.method_c_min_base_nxp, METHOD_C_MIN_BASE_NXP);
    assert_eq!(
        capabilities.method_c_max_refinement_level,
        METHOD_C_MAX_AUTO_REFINE_LEVEL
    );
    assert_eq!(capabilities.default_openmp, 16);
    assert_eq!(capabilities.default_niter, 5000);
    assert_eq!(capabilities.default_beta, 1.2);
    assert_eq!(capabilities.default_relax, 0.04);
    assert_eq!(capabilities.default_hfield_g, 0.2);
    assert_eq!(
        capabilities.method_c_spring_nxp1_km,
        earthmesh_project::METHOD_C_SPRING_NXP1_KM
    );
    assert_eq!(
        capabilities.km_per_degree_equator,
        earthmesh_project::KM_PER_DEGREE_EQUATOR
    );
}

#[test]
fn mesh_analysis_workspaces_are_unique() {
    let root = env::temp_dir().join(format!(
        "earthmesh_gui_analysis_{}_{}",
        process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let first = mesh_outputs::create_unique_analysis_dir(&root, "quality").unwrap();
    let second = mesh_outputs::create_unique_analysis_dir(&root, "quality").unwrap();
    assert_ne!(first, second);
    assert!(first.is_dir() && second.is_dir());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn mesh_kind_rejects_invalid_values() {
    assert_eq!(mesh_outputs::checked_mesh_kind(None).unwrap(), "hex");
    assert_eq!(mesh_outputs::checked_mesh_kind(Some("tri")).unwrap(), "tri");
    assert_eq!(mesh_outputs::checked_mesh_kind(Some("hex")).unwrap(), "hex");
    assert!(mesh_outputs::checked_mesh_kind(Some("")).is_err());
    assert!(mesh_outputs::checked_mesh_kind(Some("square"))
        .unwrap_err()
        .contains("mesh kind must be tri or hex"));
    match mesh_quality(
        "missing.nc".to_string(),
        Some("square".to_string()),
        Some(25.0),
        Some("warn".to_string()),
    ) {
        Err(e) => assert!(e.contains("mesh kind must be tri or hex")),
        Ok(_) => panic!("invalid mesh_quality kind should fail"),
    }
    match mesh_cell_polygons("missing.nc".to_string(), "square".to_string(), None) {
        Err(e) => assert!(e.contains("mesh kind must be tri or hex")),
        Ok(_) => panic!("invalid mesh_cell_polygons kind should fail"),
    }
    match mesh_outputs::mesh_merit_cells(
        "missing.nc".to_string(),
        "square".to_string(),
        "missing-merit".to_string(),
        0.0,
        1.0,
        0.0,
        1.0,
        None,
        None,
    ) {
        Err(e) => assert!(e.contains("mesh kind must be tri or hex")),
        Ok(_) => panic!("invalid mesh_merit_cells kind should fail"),
    }
}

#[test]
fn explicit_missing_landtype_file_is_not_silently_replaced_by_merit_surface_data() {
    let root = env::temp_dir().join(format!(
        "earthmesh_gui_missing_landtype_{}_{}",
        process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let merit_root = root.join("merit");
    let gridfile = root.join("mesh.nc");
    let missing_landtype = root.join("missing-landtype.nc");
    fs::create_dir_all(&merit_root).unwrap();
    fs::write(&gridfile, b"grid fixture").unwrap();

    let error = mesh_outputs::mesh_merit_cells(
        gridfile.to_string_lossy().into_owned(),
        "hex".to_string(),
        merit_root.to_string_lossy().into_owned(),
        112.0,
        115.0,
        21.0,
        24.0,
        None,
        Some(missing_landtype.to_string_lossy().into_owned()),
    )
    .expect_err("an explicit missing landtype file must fail before MERIT fallback");

    assert!(error.contains("landtype file not found"), "{error}");
    assert!(error.contains("missing-landtype.nc"), "{error}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn gui_quality_config_uses_project_threshold_and_policy() {
    let block = mesh_outputs::quality_namelist_for_gui(27.5, "block").unwrap();
    assert!(block.contains("NL%min_angle_warn_deg = 27.5"));
    assert!(block.contains("NL%on_violation = 'block'"));

    let auto = mesh_outputs::quality_namelist_for_gui(31.0, "auto_refine").unwrap();
    assert!(auto.contains("NL%min_angle_warn_deg = 31"));
    assert!(auto.contains("NL%on_violation = 'warn'"));
}

#[test]
fn every_quality_policy_is_preserved_for_the_shared_project_cli() {
    for policy in [
        ViolationPolicy::Warn,
        ViolationPolicy::Block,
        ViolationPolicy::AutoRefine,
    ] {
        let mut cfg = circle_project("backend_shared_project_cli");
        cfg.quality.on_violation = policy;
        let yaml = mesh_runner::project_cli_yaml(&cfg).unwrap();
        let staged = ProjectConfig::from_yaml(&yaml).unwrap();
        assert_eq!(staged.quality.on_violation, policy);
    }

    let project_path = Path::new("/tmp/earthmesh-project.yaml");
    let run_dir = Path::new("/tmp/earthmesh-run");
    let command = mesh_runner::project_cli_command("mkgrd.x", project_path, run_dir);
    let args = command
        .get_args()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(args, ["--project", "/tmp/earthmesh-project.yaml"]);
    assert_eq!(command.get_current_dir(), Some(run_dir));
}
fn preset_yaml(name: &str, intent: MeshIntentPreset) -> String {
    scaffold_project(
        name.to_string(),
        intent.id().to_string(),
        Some(40),
        None,
        None,
    )
    .expect("scaffold project")
}
fn hydrology_yaml(name: &str) -> String {
    preset_yaml(name, MeshIntentPreset::HydrologyLand)
}
fn circle_project(name: &str) -> ProjectConfig {
    ProjectConfig::scaffold(
        name,
        MeshIntentPreset::HydrologyLand,
        DomainConfig::Regional {
            shape: RegionShape::Circle {
                lon: 113.0,
                lat: 22.5,
                radius_km: 100.0,
            },
            sea_ratio: None,
        },
        ResolutionSpec::Nxp(40),
    )
}
#[test]
fn set_domain_bbox_accepts_antimeridian_and_rejects_degenerate_coordinates() {
    let yaml = hydrology_yaml("bbox_test");
    let yaml = set_domain_bbox(yaml, 170.0, -170.0, 21.5, 23.5, None).expect("antimeridian bbox");
    let summary = project_summary(yaml).expect("summary");
    assert_eq!(summary.bbox, Some([170.0, -170.0, 21.5, 23.5]));
    let yaml = hydrology_yaml("bbox_test");
    let err = set_domain_bbox(yaml, 112.0, 112.0, 21.5, 23.5, None).unwrap_err();
    assert!(err.contains("bbox west and east must differ"));
    let yaml = hydrology_yaml("bbox_test");
    let err = set_domain_bbox(yaml, 112.0, 115.0, 21.5, 21.5, None).unwrap_err();
    assert!(err.contains("bbox south must be < north"));
}
#[test]
fn set_domain_bbox_rejects_invalid_sea_ratio() {
    let yaml = preset_yaml("sea_ratio_test", MeshIntentPreset::CoastalOcean);
    let err = set_domain_bbox(yaml, 112.0, 115.0, 21.5, 23.5, Some(1.5)).unwrap_err();
    assert!(err.contains("domain sea_ratio must be between 0 and 1"));
}
#[test]
fn project_summary_reports_regional_sea_ratio() {
    let yaml = preset_yaml("sea_ratio_test", MeshIntentPreset::CoastalOcean);
    let yaml = set_domain_bbox(yaml, 112.0, 115.0, 21.5, 23.5, Some(0.25)).expect("set bbox");
    let summary = project_summary(yaml).expect("summary");
    assert_eq!(summary.sea_ratio, Some(0.25));
}
#[test]
fn set_domain_bbox_uses_engine_default_sea_ratio_when_ui_omits_it() {
    let yaml = preset_yaml("default_sea_ratio_test", MeshIntentPreset::CoastalOcean);
    let yaml = set_domain_bbox(yaml, 112.0, 115.0, 21.5, 23.5, None).expect("set bbox");
    let summary = project_summary(yaml).expect("summary");
    assert_eq!(summary.sea_ratio, Some(default_mask_sea_ratio()));
}
#[test]
fn project_summary_reports_approx_km_resolution() {
    let yaml = scaffold_project(
        "km_test".to_string(),
        "HydrologyLand".to_string(),
        None,
        Some(100.0),
        None,
    )
    .expect("scaffold project");
    let summary = project_summary(yaml).expect("summary");
    assert_eq!(summary.nxp, None);
    assert_eq!(summary.approx_km, Some(100.0));
    assert_eq!(summary.effective_nxp, 80);
}

#[test]
fn project_summary_reports_approx_degree_resolution() {
    let yaml = scaffold_project(
        "degree_test".to_string(),
        "HydrologyLand".to_string(),
        None,
        None,
        Some(100.0 / earthmesh_project::KM_PER_DEGREE_EQUATOR),
    )
    .expect("scaffold project");
    let summary = project_summary(yaml).expect("summary");
    assert_eq!(summary.nxp, None);
    assert_eq!(summary.approx_km, None);
    assert_eq!(
        summary.approx_degree,
        Some(100.0 / earthmesh_project::KM_PER_DEGREE_EQUATOR)
    );
    assert_eq!(summary.effective_nxp, 80);
}

#[test]
fn scaffold_project_defaults_to_method_c_100km_nxp() {
    let yaml = scaffold_project(
        "default_resolution".to_string(),
        "HydrologyLand".to_string(),
        None,
        None,
        None,
    )
    .expect("scaffold project");
    let summary = project_summary(yaml).expect("summary");
    assert_eq!(summary.nxp, Some(80));
    assert_eq!(summary.effective_nxp, 80);
}

#[test]
fn project_summary_reports_target_cell_and_model_format() {
    let yaml = preset_yaml("ocean_test", MeshIntentPreset::CoastalOcean);
    let summary = project_summary(yaml).expect("summary");
    assert_eq!(summary.cell, "tri");
    assert_eq!(summary.quality_mode, "tri-strict");
    assert_eq!(summary.model_format, "FVCOM");
    let landcover = summary
        .layers
        .iter()
        .find(|layer| layer.id == "landcover")
        .expect("landcover layer");
    assert_eq!(landcover.role_kind, "landcover");
    assert_eq!(landcover.role, "land type");
    let threshold = summary
        .layers
        .iter()
        .find(|layer| layer.role_kind == "threshold")
        .expect("threshold layer");
    assert!(threshold.role.starts_with("threshold · "));
}
#[test]
fn project_summary_reports_hidden_regional_shape() {
    let cfg = circle_project("circle_test");
    let summary = project_summary(cfg.to_yaml().expect("yaml")).expect("summary");
    assert_eq!(summary.domain, "regional");
    assert_eq!(summary.domain_shape, "circle");
    assert_eq!(summary.bbox, None);
}

#[test]
fn set_domain_shapefile_reports_watershed_path() {
    let yaml = hydrology_yaml("watershed_test");
    let yaml = set_domain_shapefile(yaml, "input/watershed.shp".to_string(), Some(0.4))
        .expect("set watershed shapefile");
    let summary = project_summary(yaml).expect("summary");
    assert_eq!(summary.domain, "regional");
    assert_eq!(summary.domain_shape, "shapefile");
    assert_eq!(
        summary.watershed_path.as_deref(),
        Some("input/watershed.shp")
    );
    assert_eq!(summary.sea_ratio, Some(0.4));
}

#[test]
fn set_domain_close_reports_mask_source() {
    let yaml = hydrology_yaml("close_test");
    let yaml = set_domain_close(
        yaml,
        "input/Ocean/Ocean_ChinaSea_boundary.nml".to_string(),
        "nml".to_string(),
        Some(0.3),
    )
    .expect("set close domain");
    let summary = project_summary(yaml).expect("summary");
    assert_eq!(summary.domain_shape, "close");
    assert_eq!(summary.close_format, Some("nml".to_string()));
    assert_eq!(
        summary.watershed_path,
        Some("input/Ocean/Ocean_ChinaSea_boundary.nml".to_string())
    );
    assert_eq!(summary.sea_ratio, Some(0.3));
}

#[test]
fn shapefile_boundary_geojson_returns_polygon_outline() {
    let root = env::temp_dir().join(format!("earthmesh_studio_shp_geojson_{}", process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create root");
    let shp = root.join("watershed.shp");
    write_test_polygon_shp(&shp, &[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]);

    let geojson =
        mesh_runner::shapefile_boundary_geojson(shp.to_string_lossy().into_owned()).unwrap();
    assert_eq!(geojson["type"], "FeatureCollection");
    let features = geojson["features"].as_array().expect("features");
    assert_eq!(features.len(), 1);
    assert_eq!(features[0]["geometry"]["type"], "Polygon");
    assert_eq!(
        features[0]["geometry"]["coordinates"][0]
            .as_array()
            .expect("outer ring")
            .len(),
        5
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn shapefile_boundary_uses_prj_to_convert_web_mercator() {
    let root = env::temp_dir().join(format!("earthmesh_studio_shp_crs_{}", process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create root");
    let shp = root.join("projected.shp");
    write_test_polygon_shp(
        &shp,
        &[
            (0.0, 0.0),
            (111_319.490_793, 0.0),
            (111_319.490_793, 111_325.142_866),
            (0.0, 111_325.142_866),
        ],
    );
    assert!(
        mesh_runner::shapefile_boundary_geojson(shp.to_string_lossy().into_owned())
            .unwrap_err()
            .contains(".prj")
    );
    fs::write(
        shp.with_extension("prj"),
        r#"PROJCS["WGS_1984_Web_Mercator_Auxiliary_Sphere"]"#,
    )
    .expect("write prj");

    let geojson =
        mesh_runner::shapefile_boundary_geojson(shp.to_string_lossy().into_owned()).unwrap();
    let ring = geojson["features"][0]["geometry"]["coordinates"][0]
        .as_array()
        .unwrap();
    assert!((ring[2][0].as_f64().unwrap() - 1.0).abs() < 1e-6);
    assert!((ring[2][1].as_f64().unwrap() - 1.0).abs() < 1e-6);
}

fn write_test_polygon_shp(path: &Path, ring: &[(f64, f64)]) {
    let mut points = ring.to_vec();
    points.push(ring[0]);
    let content_bytes = 44 + 4 + points.len() * 16;
    let file_bytes = 100 + 8 + content_bytes;
    let mut out = Vec::with_capacity(file_bytes);

    out.extend(9994_i32.to_be_bytes());
    out.extend([0_u8; 20]);
    out.extend(((file_bytes / 2) as i32).to_be_bytes());
    out.extend(1000_i32.to_le_bytes());
    out.extend(5_i32.to_le_bytes());
    for value in [0.0_f64, 0.0, 4.0, 4.0, 0.0, 0.0, 0.0, 0.0] {
        out.extend(value.to_le_bytes());
    }

    out.extend(1_i32.to_be_bytes());
    out.extend(((content_bytes / 2) as i32).to_be_bytes());
    out.extend(5_i32.to_le_bytes());
    for value in [0.0_f64, 0.0, 4.0, 4.0] {
        out.extend(value.to_le_bytes());
    }
    out.extend(1_i32.to_le_bytes());
    out.extend((points.len() as i32).to_le_bytes());
    out.extend(0_i32.to_le_bytes());
    for (x, y) in points {
        out.extend(x.to_le_bytes());
        out.extend(y.to_le_bytes());
    }
    fs::write(path, out).expect("write test shp");
}
#[test]
fn scaffold_project_rejects_invalid_approx_km() {
    let err = scaffold_project(
        "bad_km".to_string(),
        "HydrologyLand".to_string(),
        None,
        Some(0.0),
        None,
    )
    .unwrap_err();
    assert!(err.contains("target resolution ApproxKm must be > 0"));
    let err = scaffold_project(
        "bad_nxp".to_string(),
        "HydrologyLand".to_string(),
        Some(0),
        None,
        None,
    )
    .unwrap_err();
    assert!(err.contains("target resolution Nxp must be > 0"));
    let err = scaffold_project(
        "bad_intent".to_string(),
        "TypoHydrologyLand".to_string(),
        Some(40),
        None,
        None,
    )
    .unwrap_err();
    assert!(err.contains("unknown mesh intent 'TypoHydrologyLand'"));
}
#[test]
fn set_project_metadata_writes_visible_project_fields() {
    let yaml = hydrology_yaml("old");
    let yaml = set_project_metadata(
        yaml,
        "new".to_string(),
        vec![" Alice ".to_string(), "".to_string()],
        "saved from UI".to_string(),
    )
    .expect("set metadata");
    let summary = project_summary(yaml).expect("summary");
    assert_eq!(summary.name, "new");
    assert_eq!(summary.authors, vec!["Alice"]);
    assert_eq!(summary.description, "saved from UI");
}

#[test]
fn set_target_cell_updates_project_cell_shape() {
    let yaml = hydrology_yaml("cell_shape");
    let yaml = set_target_cell(yaml, "tri".to_string()).expect("set cell");
    let summary = project_summary(yaml.clone()).expect("summary");
    assert_eq!(summary.cell, "tri");
    assert_eq!(summary.quality_mode, "tri-strict");
    let yaml = set_target_cell(yaml, "hex".to_string()).expect("set hex cell");
    let summary = project_summary(yaml.clone()).expect("summary");
    assert_eq!(summary.cell, "hex");
    assert_eq!(summary.quality_mode, "hex-cgrid");
    let cfg = ProjectConfig::from_yaml(&yaml).expect("yaml");
    assert_eq!(cfg.target.cell, earthmesh_project::MeshCellKind::Hex);
    assert!(set_target_cell(yaml, "square".to_string()).is_err());
}

#[test]
fn set_expert_updates_custom_overrides() {
    let yaml = hydrology_yaml("expert");
    let yaml = set_expert(
        yaml,
        Some(80),
        Some(4),
        Some(200),
        Some(120),
        Some(2),
        Some(3),
        Some(vec![4, 4, 3]),
        Some(vec![5, 4, 3]),
        Some("linear".to_string()),
        Some(1),
        Some(2),
        Some(0),
        Some(1),
        Some(1.123_456_789_012_3),
        Some(0.031_234_567_890_123),
        Some(true),
    )
    .expect("set expert");
    let summary = project_summary(yaml.clone()).expect("summary");
    assert_eq!(summary.expert_nxp, Some(80));
    assert_eq!(summary.expert_openmp, Some(4));
    assert_eq!(summary.expert_niter, Some(200));
    assert_eq!(summary.expert_niter_refine, Some(120));
    assert_eq!(summary.expert_max_iter_spc, Some(2));
    assert_eq!(summary.expert_max_iter_cal, Some(3));
    assert_eq!(summary.expert_halo, Some(vec![4, 4, 3]));
    assert_eq!(summary.expert_max_transition_row, Some(vec![5, 4, 3]));
    assert_eq!(summary.expert_set_dis_type, Some("linear".to_string()));
    assert_eq!(summary.expert_num_rc, Some(1));
    assert_eq!(summary.expert_vertex_pretect_layers, Some(2));
    assert_eq!(summary.expert_spring_global_type, Some(0));
    assert_eq!(summary.expert_spring_regional_type, Some(1));
    assert_eq!(summary.expert_beta, Some(1.123_456_789_012_3));
    assert_eq!(summary.expert_relax, Some(0.031_234_567_890_123));
    assert_eq!(summary.expert_weak_concav_eliminate, Some(true));
    assert!(set_expert(
        yaml,
        None,
        None,
        Some(0),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None
    )
    .is_err());
}

#[test]
fn close_boundary_command_updates_domain_and_round_trips_through_summary() {
    let yaml = preset_yaml("close_boundary_domain", MeshIntentPreset::CoastalOcean);
    let yaml = set_domain_close(
        yaml,
        "./masks/domain_close.nml".to_string(),
        "nml".to_string(),
        None,
    )
    .expect("set close domain");
    let yaml = set_close_boundary(
        yaml,
        "domain".to_string(),
        "spherical_chaikin".to_string(),
        Some(2),
        None,
        None,
        Some(0.25),
    )
    .expect("set close boundary");

    let cfg = ProjectConfig::from_yaml(&yaml).expect("project");
    let DomainConfig::Regional {
        shape: RegionShape::Close { boundary, .. },
        ..
    } = &cfg.domain
    else {
        panic!("expected close domain");
    };
    assert_eq!(
        boundary,
        &CloseBoundaryMode::SphericalChaikin {
            iterations: 2,
            max_segment_angle_deg: 0.25,
        }
    );
    let summary = project_summary(yaml).expect("summary");
    assert_eq!(summary.domain_close_boundary, Some(boundary.clone()));
}

#[test]
fn close_boundary_command_updates_specified_close_cap() {
    let yaml = preset_yaml("close_boundary_refine", MeshIntentPreset::CoastalOcean);
    let yaml = set_specified_refinement(
        yaml,
        true,
        Some("close".to_string()),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some("./masks/refine_close.nml".to_string()),
    )
    .expect("set specified close");
    let yaml = set_close_boundary(
        yaml,
        "specified".to_string(),
        "enclosing_cap".to_string(),
        None,
        Some(20.0),
        Some(80.0),
        Some(0.25),
    )
    .expect("set specified cap");

    let summary = project_summary(yaml).expect("summary");
    assert_eq!(
        summary.specified_refine_close_boundary,
        Some(CloseBoundaryMode::EnclosingCap {
            margin_km: 20.0,
            max_radius_deg: 80.0,
            max_segment_angle_deg: 0.25,
        })
    );
}

#[test]
fn set_specified_refinement_updates_project() {
    let yaml = hydrology_yaml("specified_refine");
    let yaml = set_specified_refinement(
        yaml,
        true,
        Some("radius".to_string()),
        Some(113.5),
        Some(22.0),
        Some(80.0),
        None,
        None,
        None,
        None,
        None,
    )
    .expect("set specified refinement");
    let summary = project_summary(yaml.clone()).expect("summary");
    assert!(summary.specified_refine_enabled);
    assert_eq!(summary.specified_refine_kind, "radius");
    assert_eq!(summary.specified_refine_lon, Some(113.5));
    assert_eq!(summary.specified_refine_lat, Some(22.0));
    assert_eq!(summary.specified_refine_radius_km, Some(80.0));
    assert!(set_specified_refinement(
        yaml,
        true,
        Some("radius".to_string()),
        Some(181.0),
        Some(22.0),
        Some(80.0),
        None,
        None,
        None,
        None,
        None
    )
    .is_err());
}

#[test]
fn set_specified_refinement_accepts_bbox_region() {
    let yaml = hydrology_yaml("specified_refine_bbox");
    let yaml = set_specified_refinement(
        yaml,
        true,
        Some("bbox".to_string()),
        None,
        None,
        None,
        Some(112.0),
        Some(115.0),
        Some(21.0),
        Some(24.0),
        None,
    )
    .expect("set specified bbox refinement");
    let summary = project_summary(yaml.clone()).expect("summary");
    assert!(summary.specified_refine_enabled);
    assert_eq!(summary.specified_refine_kind, "bbox");
    assert_eq!(
        summary.specified_refine_bbox,
        Some([112.0, 115.0, 21.0, 24.0])
    );
    let antimeridian = set_specified_refinement(
        yaml,
        true,
        Some("bbox".to_string()),
        None,
        None,
        None,
        Some(170.0),
        Some(-170.0),
        Some(21.0),
        Some(24.0),
        None,
    )
    .expect("set antimeridian specified bbox refinement");
    let summary = project_summary(antimeridian).expect("summary");
    assert_eq!(
        summary.specified_refine_bbox,
        Some([170.0, -170.0, 21.0, 24.0])
    );
}

#[test]
fn hfield_defaults_on_and_discrete_can_disable_it() {
    let yaml = hydrology_yaml("hfield_default");
    let summary = project_summary(yaml.clone()).expect("summary");
    assert!(summary.hfield_enabled);

    let yaml = set_hfield_refinement(yaml, false, None, None, None).expect("discrete hfield off");
    let summary = project_summary(yaml.clone()).expect("summary");
    assert!(!summary.hfield_enabled);
    let lowered = ProjectConfig::from_yaml(&yaml).expect("yaml").lower();
    assert!(!lowered.to_namelist().contains("&hfield"));
}

#[test]
fn hfield_origin_survives_opened_project_compose_and_canonical_save() {
    let mut base = circle_project("opened_hfield_origin");
    base.refinement.hfield = Some(earthmesh_project::HfieldRefinementRecipe {
        enabled: true,
        g: 0.15,
        max_level: 3,
        base_m: Some(12_000.0),
        origin_lon: Some(123.5),
        origin_lat: Some(-31.25),
    });
    let edited = ProjectConfig::scaffold(
        "edited_hfield_origin",
        MeshIntentPreset::HydrologyLand,
        DomainConfig::Global,
        ResolutionSpec::Nxp(80),
    );

    let composed = preserve_unexposed_project_fields(
        base.to_yaml().expect("base yaml"),
        edited.to_yaml().expect("edited yaml"),
        true,
    )
    .expect("preserve hidden h-field origin");
    let composed = set_hfield_refinement(composed, true, Some(0.25), Some(4), Some(10_000.0))
        .expect("apply visible h-field edits");
    let saved = validate_project(composed).expect("canonical save serialization");
    let recipe = ProjectConfig::from_yaml(&saved)
        .expect("saved yaml")
        .refinement
        .hfield
        .expect("h-field recipe");

    assert_eq!(recipe.g, 0.25);
    assert_eq!(recipe.max_level, 4);
    assert_eq!(recipe.base_m, Some(10_000.0));
    assert_eq!(recipe.origin_lon, Some(123.5));
    assert_eq!(recipe.origin_lat, Some(-31.25));
}

#[test]
fn set_hfield_refinement_rejects_invalid_explicit_base_size() {
    for invalid in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let error = set_hfield_refinement(
            hydrology_yaml("invalid_hfield_base"),
            true,
            None,
            None,
            Some(invalid),
        )
        .expect_err("invalid explicit base_m must not be converted to automatic sizing");
        assert!(error.contains("h-field base_m must be positive"), "{error}");
    }
}

#[test]
fn set_specified_refinement_accepts_close_shapefile() {
    let yaml = set_specified_refinement(
        hydrology_yaml("specified_refine_close"),
        true,
        Some("close".to_string()),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some("input/region.shp".to_string()),
    )
    .expect("set specified close shapefile");
    let summary = project_summary(yaml).expect("summary");
    assert_eq!(
        summary.specified_refine_path,
        Some("input/region.shp".to_string())
    );
}

#[test]
fn preserve_unexposed_project_fields_keeps_supported_hidden_opened_config() {
    let mut base = circle_project("opened");
    base.metadata.authors = vec!["EarthMesh team".to_string()];
    base.metadata.description = "advanced settings".to_string();
    base.target.kind = earthmesh_project::MeshDomainKind::Ocean;
    base.target.cell = earthmesh_project::MeshCellKind::Tri;
    base.target.model_format = earthmesh_project::ModelFormat::Fvcom;
    base.refinement.enabled = false;
    base.refinement.max_passes = 0;
    base.expert.openmp = Some(8);
    base.data_layers.push(earthmesh_project::ProjectDataLayer {
        id: "custom_threshold".to_string(),
        role: ProjectLayerRole::Threshold(earthmesh_project::ThresholdField::Lai),
        path: String::new(),
        enabled: false,
        threshold_value: None,
    });
    let mut edited = ProjectConfig::scaffold(
        "edited",
        MeshIntentPreset::HydrologyLand,
        DomainConfig::Global,
        ResolutionSpec::Nxp(80),
    );
    edited.refinement.enabled = false;
    edited.refinement.max_passes = 0;
    let merged = preserve_unexposed_project_fields(
        base.to_yaml().expect("base yaml"),
        edited.to_yaml().expect("edited yaml"),
        true,
    )
    .expect("preserve hidden fields");
    let merged = ProjectConfig::from_yaml(&merged).expect("merged yaml");
    assert_eq!(merged.metadata.name, "edited");
    assert_eq!(merged.target.resolution, ResolutionSpec::Nxp(80));
    assert_eq!(merged.target.cell, earthmesh_project::MeshCellKind::Tri);
    assert_eq!(
        merged.target.model_format,
        earthmesh_project::ModelFormat::Fvcom
    );
    assert_eq!(merged.expert.openmp, Some(8));
    assert!(merged.hydro_coast.is_none());
    assert!(merged.coupling.is_none());
    assert!(matches!(
        merged.domain,
        DomainConfig::Regional {
            shape: RegionShape::Circle { .. },
            ..
        }
    ));
    assert!(merged
        .data_layers
        .iter()
        .any(|layer| layer.id == "custom_threshold"));
}
#[test]
fn preserve_unexposed_project_fields_keeps_user_bbox_edit_over_hidden_circle() {
    let base = circle_project("opened");
    let edited = ProjectConfig::scaffold(
        "edited",
        MeshIntentPreset::HydrologyLand,
        DomainConfig::Regional {
            shape: RegionShape::Bbox {
                w: 108.0,
                e: 120.0,
                s: 18.0,
                n: 26.0,
            },
            sea_ratio: Some(0.25),
        },
        ResolutionSpec::Nxp(80),
    );
    let merged = preserve_unexposed_project_fields(
        base.to_yaml().expect("base yaml"),
        edited.to_yaml().expect("edited yaml"),
        true,
    )
    .expect("preserve hidden fields");
    let merged = ProjectConfig::from_yaml(&merged).expect("merged yaml");
    assert!(matches!(
        merged.domain,
        DomainConfig::Regional {
            shape: RegionShape::Bbox { .. },
            sea_ratio: Some(0.25),
        }
    ));
}
#[test]
fn preserve_unexposed_project_fields_allows_global_override_of_hidden_circle() {
    let base = circle_project("opened");
    let edited = ProjectConfig::scaffold(
        "edited",
        MeshIntentPreset::HydrologyLand,
        DomainConfig::Global,
        ResolutionSpec::Nxp(80),
    );
    let merged = preserve_unexposed_project_fields(
        base.to_yaml().expect("base yaml"),
        edited.to_yaml().expect("edited yaml"),
        false,
    )
    .expect("preserve hidden fields");
    let merged = ProjectConfig::from_yaml(&merged).expect("merged yaml");
    assert!(matches!(merged.domain, DomainConfig::Global));
}
#[test]
fn set_layer_path_rejects_enabled_empty_path() {
    let yaml = set_refinement(hydrology_yaml("layer_test"), false, true, 0)
        .expect("disable refinement before removing its last source");
    let err =
        set_layer_path(yaml.clone(), "landcover".to_string(), "".to_string(), true).unwrap_err();
    assert!(err.contains("data layer 'landcover' is enabled but has no path"));
    let yaml = set_layer_path(yaml, "landcover".to_string(), "".to_string(), false)
        .expect("disabled empty path is allowed");
    assert!(yaml.contains("enabled: false"));
}

#[test]
fn merit_layer_enables_the_project_hydro_plan() {
    let yaml = preset_yaml("merit_hydro", MeshIntentPreset::MeritHydroCoast);
    let yaml = set_domain_bbox(yaml, 112.0, 115.0, 21.0, 24.0, None).expect("regional domain");
    let yaml = set_layer_path(
        yaml,
        "merit".to_string(),
        "/data/merit-new".to_string(),
        true,
    )
    .expect("set MERIT root");
    let cfg = ProjectConfig::from_yaml(&yaml).expect("updated project");
    assert_eq!(
        cfg.hydro_execution_plan()
            .expect("hydro plan")
            .expect("enabled hydro plan")
            .merit_root,
        "/data/merit-new"
    );
}

#[test]
fn merit_layer_edit_keeps_hidden_hydro_options() {
    let mut base = ProjectConfig::scaffold(
        "opened_merit",
        MeshIntentPreset::MeritHydroCoast,
        DomainConfig::Regional {
            shape: RegionShape::Bbox {
                w: 112.0,
                e: 115.0,
                s: 21.0,
                n: 24.0,
            },
            sea_ratio: None,
        },
        ResolutionSpec::Nxp(40),
    );
    base.hydro_coast = Some(HydroCoastConfig {
        merit_root: "/data/merit-old".to_string(),
        cama_root: Some("/data/cama".to_string()),
        merit_stride: 1,
        r3_width_m: 450.0,
        r2_width_m: 75.0,
    });
    let edited = ProjectConfig::scaffold(
        "opened_merit",
        MeshIntentPreset::MeritHydroCoast,
        base.domain.clone(),
        ResolutionSpec::Nxp(40),
    );
    let yaml = preserve_unexposed_project_fields(
        base.to_yaml().expect("base yaml"),
        edited.to_yaml().expect("edited yaml"),
        false,
    )
    .expect("preserve hidden hydro options");
    let yaml = set_layer_path(
        yaml,
        "merit".to_string(),
        "/data/merit-new".to_string(),
        true,
    )
    .expect("update visible MERIT root");
    let cfg = ProjectConfig::from_yaml(&yaml).expect("updated project");
    let hydro = cfg.hydro_coast.expect("hydro config");
    assert_eq!(hydro.merit_root, "/data/merit-new");
    assert_eq!(hydro.cama_root.as_deref(), Some("/data/cama"));
    assert_eq!(hydro.r3_width_m, 450.0);
    assert_eq!(hydro.r2_width_m, 75.0);
}

#[test]
fn autofill_data_layers_from_folder_matches_v2_source_data_names() {
    let root = env::temp_dir().join(format!("earthmesh_studio_source_data_{}", process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("temp root");
    fs::write(root.join("landtype_usgs_update.nc"), b"land").expect("landtype");
    fs::write(root.join("LAI_BNU_161.nc"), b"lai").expect("lai");
    fs::write(root.join("k_s.nc"), b"ks").expect("ks");
    fs::write(root.join("k_solids.nc"), b"k_solids").expect("k_solids");
    fs::write(root.join("tkdry.nc"), b"tkdry").expect("tkdry");
    fs::write(root.join("tksatf.nc"), b"tksatf").expect("tksatf");
    fs::write(root.join("tksatu.nc"), b"tksatu").expect("tksatu");
    fs::write(root.join("slope_avg.nc"), b"slope").expect("slope");
    fs::write(root.join("dem.nc"), b"dem").expect("dem");
    fs::write(root.join("slope_max.nc"), b"slope_max").expect("slope_max");

    let yaml = preset_yaml("source_data", MeshIntentPreset::CarbonLand);
    let yaml = autofill_data_layers_from_folder(yaml, root.to_string_lossy().into_owned())
        .expect("autofill source_data");
    let cfg = ProjectConfig::from_yaml(&yaml).expect("parse yaml");

    let land = cfg
        .data_layers
        .iter()
        .find(|layer| matches!(layer.role, ProjectLayerRole::LandType))
        .expect("land layer");
    assert!(land.enabled);
    assert!(land.path.ends_with("landtype_usgs_update.nc"));

    let lai = cfg
        .data_layers
        .iter()
        .find(|layer| layer.id == "lai")
        .expect("lai layer");
    assert!(lai.enabled);
    assert!(lai.path.ends_with("LAI_BNU_161.nc"));

    let ks = cfg
        .data_layers
        .iter()
        .find(|layer| layer.id == "k_s")
        .expect("k_s layer");
    assert!(ks.enabled);
    assert!(ks.path.ends_with("k_s.nc"));
    for id in [
        "k_solids",
        "tkdry",
        "tksatf",
        "tksatu",
        "slope_avg",
        "dem",
        "slope_max",
    ] {
        let layer = cfg
            .data_layers
            .iter()
            .find(|layer| layer.id == id)
            .unwrap_or_else(|| panic!("{id} layer"));
        assert!(layer.enabled, "{id} should be enabled");
        assert!(
            layer.path.ends_with(&format!("{id}.nc")),
            "{id}: {}",
            layer.path
        );
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn set_threshold_value_updates_threshold_layer() {
    let yaml = hydrology_yaml("threshold_value");
    let yaml =
        set_threshold_value(yaml, "slope_avg".to_string(), Some(7.5)).expect("set threshold");
    let yaml =
        set_threshold_value(yaml, "landcover".to_string(), Some(8.0)).expect("set landcover");
    let cfg = ProjectConfig::from_yaml(&yaml).expect("parse yaml");
    let slope = cfg
        .data_layers
        .iter()
        .find(|layer| layer.id == "slope_avg")
        .expect("slope layer");
    assert_eq!(slope.threshold_value, Some(7.5));
    let landcover = cfg
        .data_layers
        .iter()
        .find(|layer| layer.id == "landcover")
        .expect("landcover layer");
    assert_eq!(landcover.threshold_value, Some(8.0));
    let err = set_threshold_value(yaml, "merit".to_string(), Some(1.0)).unwrap_err();
    assert!(err.contains("is not a refinement layer"));
}

#[test]
fn set_quality_rejects_invalid_min_angle() {
    let yaml = hydrology_yaml("quality_test");
    let err = set_quality(yaml, 0.0, "warn".to_string(), 1).unwrap_err();
    assert!(err.contains("quality min_angle_deg must be > 0"));
}

#[test]
fn set_quality_accepts_auto_refine_policy() {
    let yaml = circle_project("quality_auto_refine_test")
        .to_yaml()
        .expect("regional project yaml");
    let yaml = set_quality(yaml, 25.0, "auto_refine".to_string(), 3).unwrap();
    let summary = project_summary(yaml).unwrap();

    assert_eq!(summary.on_violation, "auto_refine");
    assert_eq!(summary.auto_refine_batch_cells, 3);

    let global = set_quality(
        hydrology_yaml("quality_auto_refine_global"),
        25.0,
        "auto_refine".to_string(),
        1,
    )
    .expect("global AutoRefine");
    assert_eq!(project_summary(global).unwrap().on_violation, "auto_refine");
}

#[test]
fn set_quality_rejects_an_empty_auto_refine_batch() {
    let err = set_quality(
        hydrology_yaml("quality_empty_batch"),
        25.0,
        "warn".to_string(),
        0,
    )
    .unwrap_err();
    assert!(err.contains("auto_refine_batch_cells must be > 0"));
}

#[test]
fn set_refinement_rejects_too_many_passes() {
    let yaml = hydrology_yaml("refine_test");
    let err = set_refinement(yaml, true, true, 6).unwrap_err();
    assert!(err.contains("refinement max_passes must be <= 5"));
}
#[test]
fn set_refinement_rejects_zero_passes_when_enabled() {
    let yaml = hydrology_yaml("refine_test");
    let err = set_refinement(yaml, true, true, 0).unwrap_err();
    assert!(err.contains("refinement max_passes must be > 0"));
}
#[test]
fn set_refinement_allows_zero_passes_when_disabled() {
    let yaml = hydrology_yaml("refine_test");
    let yaml =
        set_refinement(yaml, false, true, 8).expect("disabled refinement ignores pass count");
    let summary = project_summary(yaml).expect("summary");
    assert!(!summary.refine_enabled);
    assert_eq!(summary.max_passes, 0);
}

#[test]
fn set_refinement_persists_the_independent_threshold_switch() {
    let yaml = hydrology_yaml("threshold_master_switch");
    let yaml = set_refinement(yaml, false, false, 0).expect("disable threshold refinement");
    let summary = project_summary(yaml).expect("summary");
    assert!(!summary.refine_enabled);
    assert!(!summary.threshold_refine_enabled);
}

#[test]
fn mesh_merit_bbox_accepts_antimeridian_and_rejects_invalid_ranges() {
    assert!(mesh_outputs::validate_merit_mesh_bbox(170.0, -170.0, -10.0, 10.0).is_ok());
    for bbox in [
        [170.0, 170.0, -10.0, 10.0],
        [-181.0, 170.0, -10.0, 10.0],
        [170.0, -170.0, -91.0, 10.0],
        [170.0, -170.0, 10.0, 10.0],
    ] {
        let [w, e, s, n] = bbox;
        assert!(mesh_outputs::validate_merit_mesh_bbox(w, e, s, n)
            .unwrap_err()
            .contains("invalid MERIT-Hydro mesh bbox"));
    }
}

#[test]
fn list_criteria_reports_frontend_fields() {
    let criteria = list_criteria();
    let slope = criteria
        .iter()
        .find(|c| c.stem == "slope_avg")
        .expect("slope");
    assert_eq!(slope.label, "Slope");
    assert_eq!(slope.unit, "deg");
    assert_eq!(slope.default_value, 5.0);
    assert_eq!(slope.range_min, 0.0);
    assert_eq!(slope.range_max, 45.0);
    assert_eq!(slope.physical_process, "orographic / runoff routing");
    let dem = criteria.iter().find(|c| c.stem == "dem").expect("dem");
    assert_eq!(dem.label, "DEM");
    assert_eq!(dem.unit, "m");
    assert_eq!(dem.default_value, 500.0);
}
