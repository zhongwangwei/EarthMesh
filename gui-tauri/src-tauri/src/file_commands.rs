//! File-system and native dialog command handlers.

use std::fs;
use std::path::Path;
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

use crate::dto::OpenedProject;
use earthmesh_project::ProjectConfig;

/// Native file picker for a data-layer source. Returns the chosen path, or
/// `None` if the user cancels.
#[tauri::command]
pub(crate) async fn pick_data_file(app: AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .add_filter(
            "Geospatial data",
            &[
                "nc", "nc4", "tif", "tiff", "grib", "grib2", "shp", "bin", "dat", "txt",
            ],
        )
        .blocking_pick_file()
        .map(|p| p.to_string())
}

/// Native folder picker for tiled data-layer sources (MERIT-Hydro, CaMa).
#[tauri::command]
pub(crate) async fn pick_data_folder(app: AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .blocking_pick_folder()
        .map(|p| p.to_string())
}

/// Open a project file (YAML or JSON), validate it, return canonical YAML.
#[tauri::command]
pub(crate) async fn open_project(app: AppHandle) -> Result<Option<OpenedProject>, String> {
    let picked = app
        .dialog()
        .file()
        .add_filter("EarthMesh project", &["yaml", "yml", "json"])
        .blocking_pick_file();
    let Some(fp) = picked else {
        return Ok(None);
    };
    let path = fp.to_string();
    read_project(path).map(Some)
}

/// Save a validated project YAML via a native dialog. Returns the chosen path.
#[tauri::command]
pub(crate) async fn save_project(app: AppHandle, yaml: String) -> Result<Option<String>, String> {
    let cfg = ProjectConfig::from_yaml(&yaml).map_err(|e| format!("invalid project: {e}"))?;
    let yaml = cfg.to_yaml()?;
    let picked = app
        .dialog()
        .file()
        .add_filter("EarthMesh project", &["yaml", "yml"])
        .blocking_save_file();
    let Some(fp) = picked else {
        return Ok(None);
    };
    let path = fp.to_string();
    fs::write(&path, yaml.as_bytes()).map_err(|e| format!("write {path}: {e}"))?;
    Ok(Some(path))
}

/// Open a folder/file in the OS file manager (Finder / Explorer / xdg-open).
#[tauri::command]
pub(crate) fn open_path(path: String) -> Result<(), String> {
    if !Path::new(&path).exists() {
        return Err(format!("path does not exist: {path}"));
    }
    #[cfg(target_os = "macos")]
    let prog = "open";
    #[cfg(target_os = "windows")]
    let prog = "explorer";
    #[cfg(all(unix, not(target_os = "macos")))]
    let prog = "xdg-open";
    std::process::Command::new(prog)
        .arg(&path)
        .spawn()
        .map_err(|e| format!("open {path}: {e}"))?;
    Ok(())
}

/// Read + validate a project file at a known path (no dialog) — used to reopen
/// a recent project. Returns canonical YAML.
#[tauri::command]
pub(crate) fn read_project(path: String) -> Result<OpenedProject, String> {
    let text = fs::read_to_string(&path).map_err(|e| format!("read {path}: {e}"))?;
    let cfg = ProjectConfig::from_yaml(&text)
        .or_else(|_| ProjectConfig::from_json(&text))
        .map_err(|e| format!("parse {path}: {e}"))?;
    let yaml = cfg.to_yaml()?;
    Ok(OpenedProject { path, yaml })
}
