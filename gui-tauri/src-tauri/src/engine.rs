//! Engine discovery and launch-input preparation helpers.

use std::path::{Path, PathBuf};
use std::{env, fs};

use earthmesh_project::{ProjectConfig, ProjectLayerRole};

use crate::mesh_paths::existing_file_path;

pub(crate) fn stage_threshold_layers(
    cfg: &ProjectConfig,
    threshold_dir: &Path,
    source_base: &Path,
) -> Result<bool, String> {
    let mut staged_any = false;
    for layer in &cfg.data_layers {
        if !layer.enabled {
            continue;
        }
        let ProjectLayerRole::Threshold(field) = layer.role else {
            continue;
        };
        if !staged_any {
            fs::create_dir_all(threshold_dir)
                .map_err(|e| format!("mkdir {}: {e}", threshold_dir.display()))?;
            staged_any = true;
        }
        let src = resolve_source_path(&layer.path, source_base);
        let dst = threshold_dir.join(format!("{}.nc", field.stem()));
        fs::copy(&src, &dst).map_err(|e| {
            format!(
                "stage threshold layer '{}' to {}: {e}",
                layer.id,
                dst.display()
            )
        })?;
    }
    Ok(staged_any)
}

fn resolve_source_path(path: &str, source_base: &Path) -> PathBuf {
    let path = Path::new(path.trim());
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        existing_file_path(path.to_string_lossy().as_ref(), source_base)
            .unwrap_or_else(|| source_base.join(path))
    }
}

/// Locate the mesh-generator binary, in priority order:
///   1. `$EARTHMESH_MKGRD` (explicit override),
///   2. well-known build outputs relative to the repo root — `make build` copies
///      the CLI to `<repo>/mkgrd.x`; cargo leaves `earthmesh_cli` in its target
///      dirs — so a freshly built tree "just works" with no configuration,
///   3. next to the running executable (installed / bundled case),
///   4. bare `mkgrd.x`, letting the OS search `PATH`.
pub(crate) fn resolve_mkgrd() -> Result<String, String> {
    // Run the engine from a clean temp dir. The static netcdf/HDF5 build SIGKILLs
    // (OOM) when executed from certain source directories (observed in the dev
    // git-repo root) — an environment-level interaction with the C libraries, not a
    // logic bug. A copy under temp_dir runs reliably, so stage one and return it.
    let found = resolve_mkgrd_path();
    let src = Path::new(&found);
    if !src.is_file() {
        return Ok(found);
    }
    let dst = env::temp_dir().join("earthmesh_studio_engine.x");
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
        fs::copy(src, &dst).map_err(|err| {
            format!(
                "stage mesh engine {} -> {}: {err}",
                src.display(),
                dst.display()
            )
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dst, fs::Permissions::from_mode(0o755))
                .map_err(|err| format!("chmod {}: {err}", dst.display()))?;
        }
    }
    if dst.is_file() {
        return Ok(dst.to_string_lossy().into_owned());
    }
    Ok(found)
}

fn resolve_mkgrd_path() -> String {
    // Honor an explicit override, but only if it points at a real file — a
    // stale or placeholder $EARTHMESH_MKGRD (e.g. "/path/to/mkgrd.x") must not
    // shadow a real build; fall through to discovery instead.
    if let Ok(p) = env::var("EARTHMESH_MKGRD") {
        let p = p.trim();
        if !p.is_empty() && Path::new(p).is_file() {
            return p.to_string();
        }
    }
    // CARGO_MANIFEST_DIR is <repo>/gui-tauri/src-tauri at build time.
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let mut roots: Vec<PathBuf> = vec![
        repo.clone(),
        repo.join("rust/earthmesh_cli/target/release"),
        repo.join("rust/earthmesh_cli/target/debug"),
        repo.join("target/release"),
        repo.join("target/debug"),
    ];
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            roots.push(dir.to_path_buf());
        }
    }
    let names = ["mkgrd.x", "earthmesh_cli", "earthmesh_cli.exe", "mkgrd.exe"];
    for root in &roots {
        for n in &names {
            let cand = root.join(n);
            if cand.is_file() {
                return cand
                    .canonicalize()
                    .unwrap_or(cand)
                    .to_string_lossy()
                    .into_owned();
            }
        }
    }
    "mkgrd.x".to_string()
}
