//! Engine discovery and launch-input preparation helpers.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs};

static ENGINE_STAGE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Locate the mesh-generator binary, in priority order:
///   1. `$EARTHMESH_MKGRD` (explicit override),
///   2. next to the running executable (installed / bundled case),
///   3. well-known build outputs relative to the repo root — `make build` copies
///      the CLI to `<repo>/mkgrd.x`; cargo leaves `earthmesh_cli` in its target
///      dirs — so a freshly built tree "just works" with no configuration,
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

    // Outside a source checkout, the sidecar next to the application is the
    // packaged engine and must win over any build tree left on the machine.
    // Inside the checkout, however, choose the newest compatible build. This
    // prevents stale test stubs in target/debug from shadowing a real CLI.
    if current_exe
        .as_deref()
        .is_some_and(|exe| !path_is_within(exe, &repo))
    {
        if let Some(dir) = current_exe.as_deref().and_then(Path::parent) {
            for name in &names {
                let candidate = dir.join(name);
                if engine_candidate_is_compatible(&candidate) {
                    return Ok(canonical_string(candidate));
                }
            }
        }
    }

    // In a source checkout the staged Tauri sidecar is the intentional
    // release engine. Prefer it over a newer debug binary left by `cargo test`.
    if current_exe
        .as_deref()
        .is_some_and(|exe| path_is_within(exe, &repo))
    {
        if let Some(candidate) = source_sidecar_candidates(&repo)
            .into_iter()
            .filter(|path| engine_candidate_is_compatible(path))
            .max_by_key(|path| candidate_modified(path))
        {
            return Ok(canonical_string(candidate));
        }
    }

    // Kept so a failure can say what it turned down. A binary that is simply
    // absent says nothing; one that ran and named another version is the whole
    // answer, and reporting it as "nothing found" is what sends a user off to
    // rebuild the tree they already rebuilt.
    let mut rejected = Vec::new();
    let mut compatible = Vec::new();
    for root in &roots {
        for name in &names {
            let candidate = root.join(name);
            match inspect_engine_candidate(&candidate) {
                EngineCandidate::Compatible => compatible.push(candidate),
                EngineCandidate::Absent => {}
                other => rejected.push((candidate, other)),
            }
        }
    }
    if let Some(candidate) = compatible
        .into_iter()
        .max_by_key(|path| candidate_modified(path))
    {
        return Ok(canonical_string(candidate));
    }

    if let Some(paths) = env::var_os("PATH") {
        for dir in env::split_paths(&paths) {
            for name in &names {
                let candidate = dir.join(name);
                match inspect_engine_candidate(&candidate) {
                    EngineCandidate::Compatible => return Ok(canonical_string(candidate)),
                    EngineCandidate::Absent => {}
                    other => rejected.push((candidate, other)),
                }
            }
        }
    }

    Err(engine_not_found_message(&rejected))
}

/// The message a failed discovery leaves behind.
pub(crate) fn engine_not_found_message(rejected: &[(PathBuf, EngineCandidate)]) -> String {
    let expected = env!("CARGO_PKG_VERSION");
    if rejected.is_empty() {
        return format!(
            "no EarthMesh engine was found (expected version {expected}), and no candidate was present in any search location. Build earthmesh_cli, or set EARTHMESH_MKGRD to its full path"
        );
    }
    let mut message = format!(
        "no compatible EarthMesh engine was found (expected version {expected}). {} candidate(s) were found and turned down:",
        rejected.len()
    );
    for (path, reason) in rejected {
        let reason = match reason {
            EngineCandidate::WrongVersion(version) => format!("reports version {version}"),
            EngineCandidate::Unusable(detail) => format!("would not run: {detail}"),
            EngineCandidate::Compatible | EngineCandidate::Absent => continue,
        };
        message.push_str(&format!("\n  {} - {reason}", path.display()));
    }
    message.push_str(
        "\nRebuild that engine at the expected version, or set EARTHMESH_MKGRD to a full path",
    );
    message
}

pub(crate) fn engine_candidate_is_compatible(path: &Path) -> bool {
    matches!(inspect_engine_candidate(path), EngineCandidate::Compatible)
}

/// Why a candidate was accepted or turned down.
///
/// Absent and present-but-wrong-version are not the same situation, and the
/// second is the one a user cannot see: a stale build in a search root is a
/// real file that reports a real version, and reporting it as "nothing found"
/// sends them to rebuild whatever they rebuilt last time.
pub(crate) enum EngineCandidate {
    Compatible,
    /// Not a file, so not a candidate at all — never worth reporting.
    Absent,
    /// A binary that ran and named a different version.
    WrongVersion(String),
    /// A binary that would not run or said nothing usable.
    Unusable(String),
}

pub(crate) fn inspect_engine_candidate(path: &Path) -> EngineCandidate {
    if !path.is_file() {
        return EngineCandidate::Absent;
    }
    let output = match Command::new(path).arg("--version").output() {
        Ok(output) => output,
        Err(error) => return EngineCandidate::Unusable(error.to_string()),
    };
    if !output.status.success() {
        return EngineCandidate::Unusable(format!("--version exited {}", output.status));
    }
    match String::from_utf8(output.stdout) {
        Ok(version) if version.trim() == env!("CARGO_PKG_VERSION") => EngineCandidate::Compatible,
        Ok(version) => EngineCandidate::WrongVersion(version.trim().to_string()),
        Err(_) => EngineCandidate::Unusable("--version was not text".to_string()),
    }
}

fn path_is_within(path: &Path, root: &Path) -> bool {
    let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    path.starts_with(root)
}

fn candidate_modified(path: &Path) -> SystemTime {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .unwrap_or(UNIX_EPOCH)
}

fn canonical_string(path: PathBuf) -> String {
    path.canonicalize()
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

pub(crate) fn source_sidecar_candidates(repo: &Path) -> Vec<PathBuf> {
    let directory = repo.join("gui-tauri/src-tauri/binaries");
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("earthmesh_cli-"))
        })
        .collect()
}

pub(crate) fn engine_search_roots(repo: &Path, current_exe: Option<&Path>) -> Vec<PathBuf> {
    let mut roots = current_exe
        .and_then(Path::parent)
        .map(|dir| vec![dir.to_path_buf()])
        .unwrap_or_default();
    roots.extend([
        repo.to_path_buf(),
        repo.join("rust/earthmesh_cli/target/release"),
        repo.join("rust/earthmesh_cli/target/debug"),
        repo.join("target/release"),
        repo.join("target/debug"),
    ]);
    roots
}
