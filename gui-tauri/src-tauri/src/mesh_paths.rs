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

pub(crate) fn is_real_file(p: &str) -> bool {
    let p = p.trim();
    !p.is_empty() && !p.eq_ignore_ascii_case("none") && p != "/tmp" && Path::new(p).is_file()
}
