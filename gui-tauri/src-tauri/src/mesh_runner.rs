//! Mesh execution command handlers.
//!
//! The GUI owns only project staging, process lifetime, and log/result capture.
//! Project lowering, masks, quality policy, AutoRefine, and hydro orchestration
//! are authoritative in `mkgrd.x --project`.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs};

use earthmesh_project::{read_shapefile_polygon_rings, DomainConfig, ProjectConfig, RegionShape};
use serde_json::json;
use tauri::{AppHandle, Emitter};

use crate::auto_refine::scan_auto_refine_decisions;
use crate::dto::RunResult;
use crate::engine::resolve_mkgrd;
use crate::mesh_process::{begin_run, clear_running_child, record_running_child, RunId, RunLease};

static RUN_DIR_SEQUENCE: AtomicU64 = AtomicU64::new(0);

type CapturedGridfile = Arc<Mutex<Option<(u8, String)>>>;

/// Stage the complete Project and execute the canonical CLI project workflow.
#[tauri::command]
pub(crate) async fn run_project(
    app: AppHandle,
    yaml: String,
    outdir: Option<String>,
) -> Result<RunResult, String> {
    let run = begin_run()?;
    let mut cfg = ProjectConfig::from_yaml(&yaml).map_err(|e| format!("invalid project: {e}"))?;
    absolutize_gui_project_inputs(&mut cfg)?;
    run_project_cli(app, cfg, outdir, &run).await
}

pub(crate) fn project_cli_yaml(cfg: &ProjectConfig) -> Result<String, String> {
    cfg.to_yaml()
}

pub(crate) fn project_cli_command(bin: &str, project_path: &Path, run_dir: &Path) -> Command {
    let mut command = Command::new(bin);
    command
        .arg("--project")
        .arg(project_path)
        .current_dir(run_dir);
    command
}

async fn run_project_cli(
    app: AppHandle,
    cfg: ProjectConfig,
    outdir: Option<String>,
    run: &RunLease,
) -> Result<RunResult, String> {
    let run_dir = project_run_dir(&cfg, outdir)?;
    let project_path = run_dir.join("project.yaml");
    fs::write(&project_path, project_cli_yaml(&cfg)?.as_bytes())
        .map_err(|err| format!("write {}: {err}", project_path.display()))?;

    let bin = resolve_mkgrd()?;
    let _ = app.emit(
        "mkgrd://log",
        "Project lowering, quality policy, AutoRefine, and hydro are delegated to the shared CLI."
            .to_string(),
    );
    let _ = app.emit(
        "mkgrd://log",
        format!("$ {bin} --project {}", project_path.display()),
    );
    let child = project_cli_command(&bin, &project_path, &run_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| {
            format!(
                "could not start '{bin} --project': {err}. Build mkgrd.x and put it on PATH, or set EARTHMESH_MKGRD to its full path."
            )
        })?;
    let log_app = app.clone();
    let (ok, code, gridfile) = capture_mesh_child_with_logger(child, run.id(), move |line| {
        let _ = log_app.emit("mkgrd://log", line);
    })?;
    let scan = scan_auto_refine_decisions(&run_dir);
    for warning in &scan.warnings {
        let _ = app.emit("mkgrd://log", format!("⚠ AutoRefine audit: {warning}"));
    }
    Ok(RunResult {
        ok,
        code,
        outdir: run_dir.to_string_lossy().into_owned(),
        gridfile,
        auto_refine_decisions: scan.decisions,
    })
}

pub(crate) fn absolutize_gui_project_inputs(cfg: &mut ProjectConfig) -> Result<(), String> {
    let cwd = env::current_dir().map_err(|err| format!("resolve GUI working directory: {err}"))?;
    map_project_input_paths(cfg, |path| absolutize_input_path(path, &cwd));
    Ok(())
}

pub(crate) fn absolutize_opened_project_inputs(cfg: &mut ProjectConfig, project_dir: &Path) {
    map_project_input_paths(cfg, |path| {
        let configured = path.trim();
        if configured.is_empty() {
            return;
        }
        let configured = Path::new(configured);
        let resolved = if configured.is_absolute() {
            configured.to_path_buf()
        } else {
            project_dir.join(configured)
        };
        *path = resolved.to_string_lossy().into_owned();
    });
}

fn map_project_input_paths(cfg: &mut ProjectConfig, mut visit: impl FnMut(&mut String)) {
    for layer in &mut cfg.data_layers {
        visit(&mut layer.path);
    }
    if let DomainConfig::Regional { shape, .. } = &mut cfg.domain {
        match shape {
            RegionShape::Shapefile { path } | RegionShape::Close { path, .. } => visit(path),
            RegionShape::Bbox { .. } | RegionShape::Circle { .. } => {}
        }
    }
    if let Some(close) = cfg.refinement.specified_close.as_mut() {
        visit(&mut close.path);
    }
    if let Some(hydro) = cfg.hydro_coast.as_mut() {
        visit(&mut hydro.merit_root);
        if let Some(cama_root) = hydro.cama_root.as_mut() {
            visit(cama_root);
        }
    }
    if let Some(cama_root) = cfg
        .coupling
        .as_mut()
        .and_then(|coupling| coupling.cama_root.as_mut())
    {
        visit(cama_root);
    }
}

pub(crate) fn absolutize_input_path(path: &mut String, cwd: &Path) {
    let configured = path.trim();
    if configured.is_empty() {
        return;
    }
    *path = resolve_gui_input_path(Path::new(configured), cwd)
        .to_string_lossy()
        .into_owned();
}

pub(crate) fn resolve_gui_input_path(configured: &Path, cwd: &Path) -> PathBuf {
    if configured.is_absolute() {
        return configured.to_path_buf();
    }

    let local = cwd.join(configured);
    if local.exists() {
        return local;
    }

    // Development builds normally start in gui-tauri/src-tauri while preset
    // inputs live at the repository root.
    cwd.ancestors()
        .skip(1)
        .map(|ancestor| ancestor.join(configured))
        .find(|candidate| candidate.exists())
        .unwrap_or(local)
}

pub(crate) fn project_run_dir(
    cfg: &ProjectConfig,
    outdir: Option<String>,
) -> Result<PathBuf, String> {
    let base = outdir
        .map(|path| path.trim_end_matches('/').to_string())
        .filter(|path| !path.is_empty())
        .unwrap_or_else(|| {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_secs())
                .unwrap_or(0);
            env::temp_dir()
                .join(format!("earthmesh_run_{timestamp}"))
                .to_string_lossy()
                .into_owned()
        });
    let name: String = cfg
        .metadata
        .name
        .trim()
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '_' | '-') {
                character
            } else {
                '_'
            }
        })
        .collect();
    let base = Path::new(&base);
    fs::create_dir_all(base).map_err(|error| format!("mkdir {}: {error}", base.display()))?;
    let name = if name.is_empty() { "mesh" } else { &name };
    let primary = base.join(name);
    let run_dir = match fs::create_dir(&primary) {
        Ok(()) => primary,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            create_unique_run_dir(base, name)?
        }
        Err(error) => return Err(format!("mkdir {}: {error}", primary.display())),
    };
    fs::canonicalize(&run_dir)
        .map_err(|error| format!("resolve run directory {}: {error}", run_dir.display()))
}

fn create_unique_run_dir(base: &Path, name: &str) -> Result<PathBuf, String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    for _ in 0..128 {
        let sequence = RUN_DIR_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let candidate = base.join(format!(
            "{name}-run-{}-{nanos}-{sequence}",
            std::process::id()
        ));
        match fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("mkdir {}: {error}", candidate.display())),
        }
    }
    Err(format!(
        "could not allocate a unique run directory under {}",
        base.display()
    ))
}

pub(crate) fn read_child_lines<R, F>(reader: R, stream: &str, mut on_line: F) -> Result<(), String>
where
    R: BufRead,
    F: FnMut(&str) -> Result<(), String>,
{
    for line in reader.lines() {
        let line = line.map_err(|error| format!("read child {stream}: {error}"))?;
        on_line(&line)?;
    }
    Ok(())
}

pub(crate) fn join_output_thread(
    handle: JoinHandle<Result<(), String>>,
    stream: &str,
) -> Result<(), String> {
    handle
        .join()
        .map_err(|_| format!("child {stream} reader thread panicked"))?
}

fn capture_reported_gridfile(
    captured: &CapturedGridfile,
    line: &str,
    stream: &str,
) -> Result<(), String> {
    let (priority, path) = if let Some(path) = line.strip_prefix("project_hydro_final_gridfile=") {
        (2, path)
    } else if let Some(path) = line.strip_prefix("earthmesh_cli: project hydro final gridfile=") {
        (2, path)
    } else if stream == "stdout" {
        match line.strip_prefix("gridfile=") {
            Some(path) => (1, path),
            None => return Ok(()),
        }
    } else {
        return Ok(());
    };
    let mut state = captured
        .lock()
        .map_err(|_| "run gridfile state lock poisoned".to_string())?;
    if state
        .as_ref()
        .map(|(current_priority, _)| priority >= *current_priority)
        .unwrap_or(true)
    {
        *state = Some((priority, path.trim().to_string()));
    }
    Ok(())
}

pub(crate) fn capture_mesh_child_with_logger<F>(
    mut child: Child,
    run_id: RunId,
    log: F,
) -> Result<(bool, Option<i32>, Option<String>), String>
where
    F: Fn(String) + Clone + Send + 'static,
{
    let pid = child.id();
    let stdout = child.stdout.take().ok_or_else(|| {
        let _ = child.kill();
        let _ = child.wait();
        "child stdout pipe was not available".to_string()
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        let _ = child.kill();
        let _ = child.wait();
        "child stderr pipe was not available".to_string()
    })?;
    if let Err(error) = record_running_child(run_id, pid) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }

    let gridfile_seen: CapturedGridfile = Arc::new(Mutex::new(None));

    let stdout_log = log.clone();
    let stdout_gridfile = Arc::clone(&gridfile_seen);
    let stdout_thread = thread::spawn(move || {
        read_child_lines(BufReader::new(stdout), "stdout", |line| {
            capture_reported_gridfile(&stdout_gridfile, line, "stdout")?;
            stdout_log(line.to_string());
            Ok(())
        })
    });

    let stderr_log = log.clone();
    let stderr_gridfile = Arc::clone(&gridfile_seen);
    let stderr_thread = thread::spawn(move || {
        read_child_lines(BufReader::new(stderr), "stderr", |line| {
            capture_reported_gridfile(&stderr_gridfile, line, "stderr")?;
            stderr_log(format!("[stderr] {line}"));
            Ok(())
        })
    });

    let wait_result = child.wait();
    clear_running_child(run_id, pid);
    let stdout_result = join_output_thread(stdout_thread, "stdout");
    let stderr_result = join_output_thread(stderr_thread, "stderr");
    let status = wait_result.map_err(|error| format!("wait failed: {error}"))?;
    stdout_result?;
    stderr_result?;

    let code = status.code();
    log(format!(
        "— exited with {}",
        code.map(|value| value.to_string())
            .unwrap_or_else(|| "signal".into())
    ));
    let gridfile = gridfile_seen
        .lock()
        .map_err(|_| "run gridfile state lock poisoned".to_string())?
        .as_ref()
        .map(|(_, path)| path.clone());
    Ok((status.success(), code, gridfile))
}

#[tauri::command]
pub(crate) fn shapefile_boundary_geojson(path: String) -> Result<serde_json::Value, String> {
    let rings = read_shapefile_polygon_rings(Path::new(&path))
        .map_err(|e| format!("read watershed shp boundary: {e}"))?;
    let features = rings
        .into_iter()
        .map(|mut ring| {
            if let Some(first) = ring.first().copied() {
                ring.push(first);
            }
            let coords: Vec<[f64; 2]> = ring.into_iter().map(|(lon, lat)| [lon, lat]).collect();
            json!({
                "type": "Feature",
                "properties": {},
                "geometry": { "type": "Polygon", "coordinates": [coords] }
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({ "type": "FeatureCollection", "features": features }))
}
