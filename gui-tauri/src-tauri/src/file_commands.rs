//! File-system and native dialog command handlers.

use std::fs;
use std::path::{Path, PathBuf};
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

use crate::dto::OpenedProject;
use earthmesh_project::ProjectConfig;

const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const PNG_IEND: &[u8; 12] = b"\0\0\0\0IEND\xaeB`\x82";
const MAX_PNG_BYTES: usize = 64 * 1024 * 1024;

/// Native file picker for a data-layer source. Returns the chosen path, or
/// `None` if the user cancels.
#[tauri::command]
pub(crate) async fn pick_data_file(app: AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .add_filter(
            "Geospatial data",
            &[
                "nc", "nc4", "nml", "tif", "tiff", "grib", "grib2", "shp", "bin", "dat", "txt",
                "csv",
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

/// Save PNG bytes through the native file dialog. Returns the chosen path, or
/// `None` when the user cancels.
#[tauri::command]
pub(crate) fn save_map_png(
    app: AppHandle,
    request: tauri::ipc::Request<'_>,
) -> Result<Option<String>, String> {
    let bytes = match request.body() {
        tauri::ipc::InvokeBody::Raw(bytes) => bytes.as_slice(),
        tauri::ipc::InvokeBody::Json(_) => {
            return Err("invalid PNG payload: raw IPC bytes are required".to_string());
        }
    };
    validate_png_bytes(bytes)?;
    let picked = app
        .dialog()
        .file()
        .add_filter("PNG image", &["png"])
        .set_file_name("EarthMesh-grid.png")
        .blocking_save_file();
    let Some(fp) = picked else {
        return Ok(None);
    };

    let path = ensure_png_extension(PathBuf::from(fp.to_string()));
    let display_path = path.to_string_lossy().into_owned();
    fs::write(&path, bytes).map_err(|e| format!("write {display_path}: {e}"))?;
    Ok(Some(display_path))
}

pub(crate) fn validate_png_bytes(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > MAX_PNG_BYTES {
        return Err(format!(
            "invalid PNG payload: file exceeds the {} MiB limit",
            MAX_PNG_BYTES / 1024 / 1024
        ));
    }
    if bytes.len() < 45 {
        return Err("invalid PNG payload: file is too short".to_string());
    }
    if !bytes.starts_with(PNG_SIGNATURE) {
        return Err("invalid PNG payload: missing PNG signature".to_string());
    }
    if bytes[8..12] != [0, 0, 0, 13] || &bytes[12..16] != b"IHDR" {
        return Err("invalid PNG payload: missing leading IHDR chunk".to_string());
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().expect("fixed-width slice"));
    let height = u32::from_be_bytes(bytes[20..24].try_into().expect("fixed-width slice"));
    if width == 0 || height == 0 {
        return Err("invalid PNG payload: image dimensions must be non-zero".to_string());
    }
    if !bytes.ends_with(PNG_IEND) {
        return Err("invalid PNG payload: missing terminal IEND chunk".to_string());
    }
    Ok(())
}

pub(crate) fn ensure_png_extension(mut path: PathBuf) -> PathBuf {
    let has_png_extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("png"));
    if !has_png_extension {
        path.set_extension("png");
    }
    path
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
    let path = PathBuf::from(path);
    let path = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map_err(|e| format!("resolve current directory: {e}"))?
            .join(path)
    };
    let display_path = path.to_string_lossy().into_owned();
    let text = fs::read_to_string(&path).map_err(|e| format!("read {display_path}: {e}"))?;
    let mut cfg = ProjectConfig::from_yaml(&text)
        .or_else(|_| ProjectConfig::from_json(&text))
        .map_err(|e| format!("parse {display_path}: {e}"))?;
    let project_dir = path
        .parent()
        .ok_or_else(|| format!("project path has no parent: {display_path}"))?;
    crate::mesh_runner::absolutize_opened_project_inputs(&mut cfg, project_dir);
    let yaml = cfg.to_yaml()?;
    Ok(OpenedProject {
        path: display_path,
        yaml,
    })
}
