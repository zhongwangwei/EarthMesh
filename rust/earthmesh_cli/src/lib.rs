//! Rust orchestration adapters for replacing `mkgrd.x` side effects.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

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
    let file_dir = PathBuf::from(&plan.file_dir);

    if plan.remove_existing_file_dir && file_dir.exists() {
        fs::remove_dir_all(&file_dir)?;
        report.removed_file_dir = Some(file_dir.clone());
    }

    if plan.remove_filelists && workdir.exists() {
        for entry in fs::read_dir(workdir)? {
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
        let path = PathBuf::from(directory);
        fs::create_dir_all(&path)?;
        report.created_directories.push(path);
    }

    let namelist_save_path = PathBuf::from(&plan.namelist_save_path);
    if let Some(parent) = namelist_save_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(namelist_source, &namelist_save_path)?;
    report.copied_namelist_to = Some(namelist_save_path);

    Ok(report)
}
