use std::fs;
use std::io;
use std::path::Path;

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
