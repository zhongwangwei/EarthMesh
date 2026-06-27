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

/// Write a minimal reproducible `run_manifest.json` to the current directory.
/// Records command / cwd / status / timestamps / version / optional git sha.
/// Non-fatal: a write failure only warns.
pub(super) fn write_cli_run_manifest(
    command: &str,
    started_at: String,
    result: &Result<(), String>,
) {
    use earthmesh_core::run_manifest::{RunManifest, RunStatus};
    let cwd = env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let mut manifest = RunManifest::new("", command, &cwd);
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
