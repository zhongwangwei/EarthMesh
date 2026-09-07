use std::process::Command;

#[test]
fn first_argument_help_exits_successfully() {
    let output = Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
        .arg("--help")
        .output()
        .expect("run earthmesh_cli --help");

    assert!(
        output.status.success(),
        "--help should exit successfully, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("usage: earthmesh_cli"),
        "help should be printed to stdout"
    );
}

#[test]
fn first_argument_version_reports_package_version() {
    let cwd = std::env::temp_dir().join(format!("earthmesh_cli_version_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cwd);
    std::fs::create_dir_all(&cwd).expect("create isolated version cwd");
    let output = Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
        .arg("--version")
        .current_dir(&cwd)
        .output()
        .expect("run earthmesh_cli --version");

    assert!(
        output.status.success(),
        "--version should exit successfully, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        env!("CARGO_PKG_VERSION")
    );
    assert!(
        !cwd.join("run_manifest.json").exists(),
        "--version must not write a run manifest"
    );
    let _ = std::fs::remove_dir_all(cwd);
}

#[test]
fn studio_protocol_probe_is_stable_and_side_effect_free() {
    let cwd = std::env::temp_dir().join(format!("earthmesh_cli_protocol_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cwd);
    std::fs::create_dir_all(&cwd).expect("create isolated protocol cwd");
    let output = Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
        .arg("--studio-protocol")
        .current_dir(&cwd)
        .output()
        .expect("run earthmesh_cli --studio-protocol");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "earthmesh-studio-engine/3"
    );
    assert!(!cwd.join("run_manifest.json").exists());
    let _ = std::fs::remove_dir_all(cwd);
}

#[test]
fn mkgrd_rejects_nonpositive_openmp_before_mesh_work() {
    let cwd = std::env::temp_dir().join(format!("earthmesh_cli_openmp_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cwd);
    std::fs::create_dir_all(&cwd).expect("create isolated openmp cwd");
    let namelist = cwd.join("mkgrd.nml");
    std::fs::write(
        &namelist,
        "&mkgrd\n  NL%mesh_type = 'atmosmesh'\n  NL%output_format = 'MPAS'\n  NL%openmp = 0\n/\n",
    )
    .expect("write openmp namelist");

    let output = Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
        .arg(&namelist)
        .current_dir(&cwd)
        .output()
        .expect("run earthmesh_cli with invalid openmp");

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("worker count must be positive"),
        "NL%openmp should configure the Rust worker pool: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_dir_all(cwd);
}

#[test]
fn studio_engine_accepts_sea_ratio_projects_before_mesh_execution() {
    use earthmesh_project::{
        DomainConfig, MeshIntentPreset, ProjectConfig, ProjectLayerRole, ResolutionSpec,
        ThresholdCriterionConfig,
    };

    let root = std::env::temp_dir().join(format!("earthmesh_cli_sea_ratio_{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut project = ProjectConfig::scaffold(
        "sea_ratio",
        MeshIntentPreset::CoastalOcean,
        DomainConfig::Global,
        ResolutionSpec::Nxp(80),
    );
    // Only project compilation is under test; no raster or mesh generation is needed.
    project
        .data_layers
        .iter_mut()
        .find(|l| l.role == ProjectLayerRole::LandType)
        .unwrap()
        .path = root.join("landtype.nc").to_string_lossy().into_owned();
    project.refinement.enabled = true;
    project.refinement.threshold_enabled = true;
    project.refinement.max_passes = 1;
    project
        .refinement
        .threshold_criteria
        .push(ThresholdCriterionConfig {
            id: "sea_ratio".into(),
            enabled: true,
            value: Some(0.05),
        });
    let path = root.join("project.yaml");
    std::fs::write(&path, project.to_yaml().unwrap()).unwrap();
    // Arguments are checked after project lowering; this sentinel stops before mesh work.
    let output = Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
        .arg("--project")
        .arg(&path)
        .arg("--stop-after-project-compilation")
        .current_dir(&root)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(
        stderr.contains("unknown argument --stop-after-project-compilation"),
        "{stderr}"
    );
    assert!(!stderr.contains("unknown threshold criterion"), "{stderr}");
    std::fs::remove_dir_all(root).unwrap();
}
