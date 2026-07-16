use std::env;
use std::path::Path;

/// Seconds since the Unix epoch as a string (no chrono dependency).
pub(super) fn now_epoch_secs() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default()
}

/// Write a minimal diagnostic `run_manifest.json` to the current directory.
/// Records exact argv / cwd / status / timestamps / version / optional git sha.
/// Command-specific content hashes belong in their dedicated manifests.
/// Non-fatal: a write failure only warns.
pub(super) fn write_cli_run_manifest(
    argv: &[String],
    started_at: String,
    result: &Result<(), String>,
) {
    use earthmesh_core::run_manifest::{RunManifest, RunStatus};
    let cwd = env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let mut manifest = RunManifest::new(&argv.join(" "), &cwd);
    manifest.argv = argv.to_vec();
    if let Some(input) = primary_input_config(argv) {
        let input = std::fs::canonicalize(input)
            .unwrap_or_else(|_| Path::new(input).to_path_buf())
            .display()
            .to_string();
        manifest.input_config = input.clone();
        manifest.add_input("input_config", &input);
    }
    manifest.started_at = Some(started_at);
    manifest.completed_at = Some(now_epoch_secs());
    manifest.git_sha = option_env!("EARTHMESH_GIT_SHA").map(|s| s.to_string());
    match result {
        Ok(()) => manifest.status = RunStatus::Completed,
        Err(err) => {
            manifest.status = RunStatus::Failed;
            manifest.add_warning(err);
        }
    }
    let out = Path::new(&cwd).join("run_manifest.json");
    if let Err(err) = manifest.write_json(&out) {
        eprintln!(
            "earthmesh_cli: warning: could not write {}: {err}",
            out.display()
        );
    }
}

fn primary_input_config(argv: &[String]) -> Option<&str> {
    if let Some(index) = argv.iter().position(|arg| arg == "--project") {
        return argv.get(index + 1).map(String::as_str);
    }
    argv.get(1)
        .filter(|arg| !arg.starts_with('-'))
        .map(String::as_str)
}
