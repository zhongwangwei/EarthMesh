use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn write_olam_restart_refine_namelist(
    namelist: &str,
    workdir: &Path,
    initial_gridfile: &Path,
) -> Result<PathBuf, String> {
    let contents = fs::read_to_string(namelist)
        .map_err(|err| format!("failed to read namelist {namelist}: {err}"))?;
    let mut saw_mode_file = false;
    let mut saw_mode_file_description = false;
    let mut in_mkgrd = false;
    let initial_gridfile = initial_gridfile.display().to_string();
    let mut rewritten = Vec::new();
    for line in contents.lines() {
        let trimmed_lower = line.trim_start().to_ascii_lowercase();
        if trimmed_lower.starts_with("&mkgrd") {
            in_mkgrd = true;
            rewritten.push(line.to_string());
            continue;
        }
        if in_mkgrd && line.trim() == "/" {
            if !saw_mode_file {
                rewritten.push(format!("  NL%mode_file='{initial_gridfile}'"));
            }
            if !saw_mode_file_description {
                rewritten.push("  NL%mode_file_description='EarthMesh'".to_string());
            }
            in_mkgrd = false;
            rewritten.push(line.to_string());
            continue;
        }
        rewritten.push(
            if line
                .trim_start()
                .to_ascii_lowercase()
                .starts_with("nl%mask_restart")
            {
                "  NL%mask_restart=.false.".to_string()
            } else if line
                .trim_start()
                .to_ascii_lowercase()
                .starts_with("nl%mode_file_description")
            {
                saw_mode_file_description = true;
                "  NL%mode_file_description='EarthMesh'".to_string()
            } else if line
                .trim_start()
                .to_ascii_lowercase()
                .starts_with("nl%mode_file")
            {
                saw_mode_file = true;
                format!("  NL%mode_file='{initial_gridfile}'")
            } else {
                line.to_string()
            },
        );
    }
    let rewritten = rewritten.join("\n");
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|err| err.to_string())?
        .as_nanos();
    let path = workdir.join(format!(
        "earthmesh_olam_restart_refine_{}_{}.nml",
        std::process::id(),
        stamp
    ));
    fs::write(&path, format!("{rewritten}\n"))
        .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
    Ok(path)
}
