use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn infer_restart_refine_initial_gridfile_arg(
    namelist: &str,
    explicit: Option<&Path>,
) -> Result<PathBuf, String> {
    if let Some(path) = explicit {
        return Ok(path.to_path_buf());
    }
    let contents = fs::read_to_string(namelist)
        .map_err(|err| format!("failed to read namelist {namelist}: {err}"))?;
    let config = earthmesh_core::EarthmeshConfig::from_mkgrd_namelist(&contents)
        .map_err(|err| format!("failed to parse namelist {namelist}: {err}"))?;
    earthmesh_cli::infer_restart_refine_initial_gridfile_from_config(&config)
        .map_err(|err| err.to_string())
}
