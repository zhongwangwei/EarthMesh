//! Engine discovery and launch-input preparation helpers.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::{env, fs};

static ENGINE_STAGE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Locate the mesh-generator binary, in priority order:
///   1. `$EARTHMESH_MKGRD` (explicit override),
///   2. next to the running executable (installed / bundled case),
///   3. Cargo workspace build outputs relative to the repo root — release
///      first, then `<repo>/mkgrd.x`, and debug last — so a freshly built tree
///      "just works" without a stale legacy per-crate target taking priority,
///   4. bare `mkgrd.x`, letting the OS search `PATH`.
pub(crate) fn resolve_mkgrd() -> Result<String, String> {
    // Run the engine from a clean temp dir. The static netcdf/HDF5 build SIGKILLs
    // (OOM) when executed from certain source directories (observed in the dev
    // git-repo root) — an environment-level interaction with the C libraries, not a
    // logic bug. A copy under temp_dir runs reliably, so stage one and return it.
    let found = resolve_mkgrd_path()?;
    let src = Path::new(&found);
    if !src.is_file() {
        return Ok(found);
    }
    let dst = staged_engine_path(src, std::process::id());
    let stale = match (fs::metadata(src), fs::metadata(&dst)) {
        (Ok(s), Ok(d)) => {
            // Refresh when the built engine differs in size OR is newer than the
            // staged copy. A size-only check silently kept a stale engine after a
            // rebuild that happened to land on the same byte count — so a fresh
            // `make build` looked like it "did nothing" in the GUI.
            let src_newer = match (s.modified(), d.modified()) {
                (Ok(sm), Ok(dm)) => sm > dm,
                _ => true,
            };
            s.len() != d.len() || src_newer
        }
        _ => true,
    };
    if stale {
        stage_engine_copy(src, &dst)?;
    }
    if dst.is_file() {
        return Ok(dst.to_string_lossy().into_owned());
    }
    Ok(found)
}

pub(crate) fn staged_engine_path(src: &Path, pid: u32) -> PathBuf {
    let source = fs::canonicalize(src).unwrap_or_else(|_| src.to_path_buf());
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    env::temp_dir().join(format!(
        "earthmesh_studio_engine-{pid}-{:016x}.x",
        hasher.finish()
    ))
}

pub(crate) fn stage_engine_copy(src: &Path, dst: &Path) -> Result<(), String> {
    let sequence = ENGINE_STAGE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temp = dst.with_file_name(format!(
        ".{}.tmp-{}-{sequence}",
        dst.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("earthmesh_studio_engine"),
        std::process::id()
    ));
    let result = (|| {
        fs::copy(src, &temp).map_err(|err| {
            format!(
                "stage mesh engine {} -> {}: {err}",
                src.display(),
                temp.display()
            )
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&temp, fs::Permissions::from_mode(0o755))
                .map_err(|err| format!("chmod {}: {err}", temp.display()))?;
        }
        #[cfg(windows)]
        if dst.exists() {
            fs::remove_file(dst)
                .map_err(|err| format!("replace staged engine {}: {err}", dst.display()))?;
        }
        fs::rename(&temp, dst).map_err(|err| {
            format!(
                "publish staged mesh engine {} -> {}: {err}",
                temp.display(),
                dst.display()
            )
        })
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn resolve_mkgrd_path() -> Result<String, String> {
    // Honor an explicit override, but only if it points at a real file — a
    // stale or placeholder $EARTHMESH_MKGRD (e.g. "/path/to/mkgrd.x") must not
    // shadow a real build; fall through to discovery instead.
    if let Ok(p) = env::var("EARTHMESH_MKGRD") {
        let p = p.trim();
        if !p.is_empty() && engine_candidate_is_compatible(Path::new(p)) {
            return Ok(p.to_string());
        }
    }
    // CARGO_MANIFEST_DIR is <repo>/gui-tauri/src-tauri at build time.
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let current_exe = env::current_exe().ok();
    let roots = engine_search_roots(&repo, current_exe.as_deref());
    let names = ["mkgrd.x", "earthmesh_cli", "earthmesh_cli.exe", "mkgrd.exe"];

    if let Some(candidate) = first_compatible_engine(&roots, &names) {
        return Ok(canonical_string(candidate));
    }

    if let Some(paths) = env::var_os("PATH") {
        for dir in env::split_paths(&paths) {
            for name in &names {
                let candidate = dir.join(name);
                if engine_candidate_is_compatible(&candidate) {
                    return Ok(canonical_string(candidate));
                }
            }
        }
    }

    Err(format!(
        "no compatible EarthMesh engine was found (expected version {}). Build earthmesh_cli or set EARTHMESH_MKGRD to its full path",
        env!("CARGO_PKG_VERSION")
    ))
}

pub(crate) fn engine_candidate_is_compatible(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    Command::new(path)
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .is_some_and(|version| version.trim() == env!("CARGO_PKG_VERSION"))
}

fn path_is_within(path: &Path, root: &Path) -> bool {
    let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    path.starts_with(root)
}

pub(crate) fn first_compatible_engine(roots: &[PathBuf], names: &[&str]) -> Option<PathBuf> {
    for root in roots {
        for name in names {
            let candidate = root.join(name);
            if engine_candidate_is_compatible(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn canonical_string(path: PathBuf) -> String {
    path.canonicalize()
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

pub(crate) fn engine_search_roots(repo: &Path, current_exe: Option<&Path>) -> Vec<PathBuf> {
    let mut roots = current_exe
        .filter(|exe| !path_is_within(exe, repo))
        .and_then(Path::parent)
        .map(|dir| vec![dir.to_path_buf()])
        .unwrap_or_default();
    roots.extend([
        repo.join("target/release"),
        repo.to_path_buf(),
        repo.join("target/debug"),
    ]);
    roots
}
