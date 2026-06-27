use std::fs;
use std::path::{Path, PathBuf};

use super::super::cli_args::usage;

pub(super) fn prepare_mkgrd_namelist(
    first: String,
    args: &mut impl Iterator<Item = String>,
) -> Result<String, String> {
    let mut namelist = if first == "--project" {
        compile_project_arg(args)?
    } else {
        first
    };
    if let Some(lowered) = lower_datalayers_namelist_if_present(&namelist)? {
        namelist = lowered;
    }
    Ok(namelist)
}

fn compile_project_arg(args: &mut impl Iterator<Item = String>) -> Result<String, String> {
    let path = args
        .next()
        .ok_or_else(|| usage("--project needs a project.yaml or .json path"))?;
    let text = fs::read_to_string(&path).map_err(|e| format!("read project {path}: {e}"))?;
    let project = if path.ends_with(".json") {
        earthmesh_project::ProjectConfig::from_json(&text)?
    } else {
        earthmesh_project::ProjectConfig::from_yaml(&text)?
    };
    let nml_path = format!("{path}.nml");
    fs::write(&nml_path, project.try_lower()?.to_namelist())
        .map_err(|e| format!("write {nml_path}: {e}"))?;
    eprintln!("earthmesh_cli: compiled project -> {nml_path}");
    Ok(nml_path)
}

fn lower_datalayers_namelist_if_present(namelist: &str) -> Result<Option<String>, String> {
    let Ok(text) = fs::read_to_string(namelist) else {
        return Ok(None);
    };
    if !text.to_ascii_lowercase().contains("&datalayers") {
        return Ok(None);
    }

    let fallback = Path::new(namelist)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("threshold");
    let fallback = fallback.display().to_string();
    let lowered = earthmesh_core::lower_datalayers_namelist(&text, Some(fallback.as_str()))?;
    if !lowered.threshold_files.is_empty() {
        let th_dir = PathBuf::from(&lowered.threshold_dir);
        fs::create_dir_all(&th_dir)
            .map_err(|e| format!("create threshold dir {}: {e}", th_dir.display()))?;
        for (stem, src) in &lowered.threshold_files {
            let dst = th_dir.join(format!("{stem}.nc"));
            fs::copy(src, &dst)
                .map_err(|e| format!("stage threshold {src} -> {}: {e}", dst.display()))?;
        }
    }
    for warning in &lowered.warnings {
        eprintln!("earthmesh_cli: warning: {warning}");
    }
    let lowered_path = format!("{namelist}.lowered.nml");
    fs::write(&lowered_path, &lowered.namelist)
        .map_err(|e| format!("write lowered namelist {lowered_path}: {e}"))?;
    Ok(Some(lowered_path))
}
