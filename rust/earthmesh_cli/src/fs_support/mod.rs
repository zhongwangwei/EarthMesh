use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Create the parent directory of `path` (recursively) when it has one.
///
/// Replaces the `if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }`
/// idiom that the gridfile/mask/report writers repeat before opening an output file.
pub(crate) fn ensure_parent_dir(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

/// Resolve a configured project input relative to the project file that owns it.
#[doc(hidden)]
pub fn resolve_project_path(project_path: &Path, configured: &str) -> PathBuf {
    let path = PathBuf::from(configured);
    if path.is_absolute() {
        path
    } else {
        project_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(path)
    }
}
