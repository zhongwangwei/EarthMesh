use std::fs;
use std::path::{Path, PathBuf};

use earthmesh_cli::mkgrd_default_restart_handoff::rewrite_restart_refine_namelist_contents;

pub(crate) fn write_restart_refine_namelist(
    namelist: &str,
    workdir: &Path,
    initial_gridfile: &Path,
) -> Result<PathBuf, String> {
    let contents = fs::read_to_string(namelist)
        .map_err(|err| format!("failed to read namelist {namelist}: {err}"))?;
    let rewritten = rewrite_restart_refine_namelist_contents(&contents, initial_gridfile)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|err| err.to_string())?
        .as_nanos();
    let path = workdir.join(format!(
        "earthmesh_restart_refine_{}_{}.nml",
        std::process::id(),
        stamp
    ));
    fs::write(&path, format!("{rewritten}\n"))
        .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
    Ok(path)
}
