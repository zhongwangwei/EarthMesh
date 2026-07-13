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
    let output = Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
        .arg("--version")
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
}
