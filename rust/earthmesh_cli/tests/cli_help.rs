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
