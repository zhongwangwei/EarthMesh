//! Small filesystem helpers shared by mesh quality/mesh output operations.

use std::path::{Path, PathBuf};

pub(crate) fn gridfile_dir(gridfile: &str) -> Result<PathBuf, String> {
    let path = Path::new(gridfile);
    if !path.is_file() {
        return Err(format!("gridfile not found: {gridfile}"));
    }
    Ok(path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(".")))
}

pub(crate) fn existing_file_path(path: &str, base: &Path) -> Option<PathBuf> {
    let path = path.trim();
    if path.is_empty() || path.eq_ignore_ascii_case("none") || path == "/tmp" {
        return None;
    }
    let path = Path::new(path);
    if path.is_absolute() {
        return path.is_file().then(|| clean_path(path.to_path_buf()));
    }
    [base.join(path), repo_root().join(path), path.to_path_buf()]
        .into_iter()
        .find(|candidate| candidate.is_file())
        .map(clean_path)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn clean_path(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}
