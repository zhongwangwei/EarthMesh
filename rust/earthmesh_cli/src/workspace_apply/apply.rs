use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use earthmesh_core::MkgrdWorkspacePlan;

/// Evidence report from applying the filesystem subset of `mkgrd.F90:read_nl`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorkspaceApplyReport {
    pub removed_file_dir: Option<PathBuf>,
    pub removed_filelists: Vec<PathBuf>,
    pub created_directories: Vec<PathBuf>,
    pub copied_namelist_to: Option<PathBuf>,
}

/// Apply the non-mask filesystem side effects described by a Rust read_nl plan.
///
/// This intentionally does not execute `Mask_make`; callers can inspect
/// `plan.mask_operations` and run the appropriate mask adapter after the safe
/// workspace setup has completed.
pub fn apply_read_nl_workspace_plan(
    plan: &MkgrdWorkspacePlan,
    namelist_source: &Path,
    workdir: &Path,
) -> io::Result<WorkspaceApplyReport> {
    let mut report = WorkspaceApplyReport::default();
    let workdir_for_io = workdir.to_path_buf();
    let canonical_workdir = workdir.canonicalize()?;
    let mut allowed_roots = vec![canonical_workdir];
    if let Some(parent) = namelist_source.parent() {
        // A bare filename (e.g. `mkgrd.x case.nml` run from the case dir) has an
        // empty parent; canonicalizing "" errors with ENOENT. Treat it as cwd.
        let parent = if parent.as_os_str().is_empty() {
            Path::new(".")
        } else {
            parent
        };
        allowed_roots.push(parent.canonicalize()?);
    }
    allowed_roots.sort();
    allowed_roots.dedup();
    let file_dir = workspace_bound_path(&plan.file_dir, &allowed_roots, "file_dir")?;

    if plan.remove_existing_file_dir && file_dir.exists() {
        let canonical_file_dir = file_dir.canonicalize()?;
        if allowed_roots.iter().any(|root| &canonical_file_dir == root) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("refusing to delete workspace root {}", file_dir.display()),
            ));
        }
        fs::remove_dir_all(&file_dir)?;
        report.removed_file_dir = Some(file_dir.clone());
    }

    if plan.remove_filelists && workdir_for_io.exists() {
        for entry in fs::read_dir(&workdir_for_io)? {
            let entry = entry?;
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            if name.ends_with("_filelist.txt") {
                fs::remove_file(&path)?;
                report.removed_filelists.push(path);
            }
        }
        report.removed_filelists.sort();
    }

    for directory in &plan.directories_to_create {
        let path = workspace_bound_path(directory, &allowed_roots, "directory")?;
        fs::create_dir_all(&path)?;
        report.created_directories.push(path);
    }

    let namelist_save_path = workspace_bound_path(
        &plan.namelist_save_path,
        &allowed_roots,
        "namelist_save_path",
    )?;
    crate::ensure_parent_dir(&namelist_save_path)?;
    fs::copy(namelist_source, &namelist_save_path)?;
    report.copied_namelist_to = Some(namelist_save_path);

    Ok(report)
}

fn workspace_bound_path(path: &str, allowed_roots: &[PathBuf], role: &str) -> io::Result<PathBuf> {
    let raw = PathBuf::from(path);
    let candidate = if raw.is_absolute() {
        raw
    } else {
        allowed_roots
            .first()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no allowed roots"))?
            .join(raw)
    };
    let normalized = canonicalize_existing_prefix(&candidate)?;
    if !allowed_roots
        .iter()
        .any(|root| normalized.starts_with(root))
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "{role} outside workdir or namelist directory: {}",
                normalized.display()
            ),
        ));
    }
    Ok(normalize_lexical(&candidate))
}

fn normalize_lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn canonicalize_existing_prefix(path: &Path) -> io::Result<PathBuf> {
    if path.exists() {
        return path.canonicalize();
    }

    let mut missing = Vec::new();
    let mut cursor = path;
    while !cursor.exists() {
        let Some(name) = cursor.file_name() else {
            return Ok(normalize_lexical(path));
        };
        missing.push(name.to_os_string());
        let Some(parent) = cursor.parent() else {
            return Ok(normalize_lexical(path));
        };
        cursor = parent;
    }

    let mut normalized = cursor.canonicalize()?;
    for component in missing.iter().rev() {
        normalized.push(component);
    }
    Ok(normalize_lexical(&normalized))
}
