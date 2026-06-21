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
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("usage: earthmesh_cli"),
        "help should be printed to stdout"
    );
}
