//! Mesh execution and quality extraction command handlers.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs};

use earthmesh_project::{DomainConfig, ProjectConfig, RegionShape};
use tauri::{AppHandle, Emitter};

use crate::dto::RunResult;
use crate::engine::{resolve_mkgrd, stage_threshold_layers};
use crate::mesh_paths::is_real_file;
use crate::mesh_process::{clear_running_child, record_running_child};

/// Parse project YAML, lower to engine namelist, run mkgrd.x and parse emitted
/// outputs.
#[tauri::command]
pub(crate) async fn run_project(
    app: AppHandle,
    yaml: String,
    outdir: Option<String>,
) -> Result<RunResult, String> {
    // Validate, then lower to the Fortran namelist the engine actually reads:
    // mkgrd.x takes `<mkgrd.nml>` as a positional argument (no `--project` flag),
    // so we do the lowering here rather than relying on the CLI to do it.
    let cfg = ProjectConfig::from_yaml(&yaml).map_err(|e| format!("invalid project: {e}"))?;
    let mut lowered = cfg.try_lower()?;
    // Stabilize the spring smoothing. The config default (beta=1.2, relax=0.04) is
    // more aggressive than OLAM's proven-stable values and can OVER-relax: the
    // spring overshoots and folds the mesh locally, leaving overlapping/inverted
    // triangles that render as "fan" artifacts (the gridinit topology stays valid,
    // so nothing catches it). OLAM's ocean defaults (beta=1.0, relax=0.035) relax
    // cleanly. ProjectConfig does not expose these engine knobs yet, so GUI runs
    // pin the lowered engine defaults here instead of adding another UI switch.
    if lowered.mkgrd.beta == 1.2 {
        lowered.mkgrd.beta = 1.0;
    }
    if lowered.mkgrd.relax == 0.04 {
        lowered.mkgrd.relax = 0.035;
    }
    // The default config seeds several inputs with a "/tmp" placeholder. The
    // engine treats those as real (landtype: opens it as NetCDF; mode_file: a
    // "/tmp" dir "exists", so it tries to ingest an existing mesh). Normalize any
    // non-file path to 'none' so the engine skips landtype and generates a fresh
    // base mesh instead of ingesting a bogus one.
    if !is_real_file(&lowered.mkgrd.landtype_file) {
        lowered.mkgrd.landtype_file = "none".to_string();
    }
    if !is_real_file(&lowered.mkgrd.mode_file) {
        lowered.mkgrd.mode_file = "none".to_string();
    }

    // `outdir` is the BASE output path (the user's choice, or a temp dir). Every
    // file for this run lives in <base>/<project name>/ so outputs are grouped.
    let base = outdir
        .map(|p| p.trim_end_matches('/').to_string())
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| {
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            env::temp_dir()
                .join(format!("earthmesh_run_{ts}"))
                .to_string_lossy()
                .into_owned()
        });
    // Project name -> folder name (sanitized) = the engine's experiment_name.
    let exp: String = {
        let s: String = cfg
            .metadata
            .name
            .trim()
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        if s.is_empty() {
            "mesh".to_string()
        } else {
            s
        }
    };
    let run_dir = Path::new(&base).join(&exp);
    fs::create_dir_all(&run_dir).map_err(|e| format!("mkdir {}: {e}", run_dir.display()))?;
    let run_dir_str = run_dir.to_string_lossy().into_owned();

    // The engine CLEARS + recreates its output dir (`file_dir`). Put it in an
    // "output/" SUBfolder of run_dir so the engine never deletes run_dir itself —
    // which holds mkgrd.nml + project.yaml. file_dir = base_dir + experiment_name + "/".
    lowered.mkgrd.base_dir = format!("{run_dir_str}/");
    lowered.mkgrd.experiment_name = "output".to_string();
    let file_dir = run_dir.join("output");
    for sub in ["", "result", "contain", "restart"] {
        let _ = fs::create_dir_all(file_dir.join(sub));
    }

    let threshold_dir = run_dir.join("threshold");
    if stage_threshold_layers(&cfg, &threshold_dir)? {
        lowered.refine.threshold_dir = threshold_dir.to_string_lossy().into_owned();
        let _ = app.emit(
            "mkgrd://log",
            format!("✓ staged threshold layers in {}", threshold_dir.display()),
        );
    }

    // Regional bbox domain: the engine reads the region from a `.nml` mask file
    // (mask_domain_type='bbox' -> parse_bbox_mask_nml: `bbox_num`/`bbox_refine`
    // then rows of `west east north south`). Generate it from the project's bbox
    // so a regional run needs no external mask file and no netcdf in the GUI.
    if let DomainConfig::Regional {
        shape: RegionShape::Bbox { w, e, n, s },
        ..
    } = &cfg.domain
    {
        let mask_nml = run_dir.join("domain_bbox.nml");
        let body = format!("bbox_num = 1\nbbox_refine = 1\n{w} {e} {n} {s}\n");
        match fs::write(&mask_nml, body) {
            Ok(()) => {
                // From-scratch regional CLIP, no refinement. A non-global mask
                // domain (mask_domain_global=.false. + a bbox source) makes the
                // engine subset the base mesh to the box: with refine OFF the run
                // takes the dedicated pure-clip dispatch branch
                // (run_mkgrd_regional_clip_base_namelist), which generates the
                // global base grid and keeps only the in-box cells via the shared
                // write_regional_gridfile writer. One pass, every mesh type, and
                // no netcdf in the GUI — the engine parses this plain-text .nml.
                //
                // refine is forced OFF so the OLAM refine path (which would demand
                // a separate refinement region) is bypassed; refine_spc/cal are
                // cleared for the same reason. NOT mask_restart — that path is a
                // continuation that never clips atmos.
                lowered.mkgrd.mask_domain_global = false;
                lowered.mkgrd.mask_domain_type = "bbox".to_string();
                lowered.mkgrd.mask_domain_fprefix =
                    run_dir.join("domain_bbox").to_string_lossy().into_owned();
                lowered.mkgrd.mask_restart = false;
                lowered.mkgrd.refine = false;
                lowered.refine.refine_spc = false;
                lowered.refine.refine_cal = false;
                let _ = app.emit(
                    "mkgrd://log",
                    format!(
                        "✓ regional bbox clip (W {w}, E {e}, N {n}, S {s}) — keeping only in-box cells (no refinement)"
                    ),
                );
            }
            Err(err) => {
                let _ = app.emit("mkgrd://log", format!("⚠ could not write bbox mask: {err}"));
            }
        }
    }

    let namelist = lowered.to_namelist();
    // project.yaml (provenance) + mkgrd.nml (engine input) both live in run_dir.
    let yaml_path = run_dir.join("project.yaml");
    fs::write(&yaml_path, yaml.as_bytes())
        .map_err(|e| format!("write {}: {e}", yaml_path.display()))?;
    let nml_path = run_dir.join("mkgrd.nml");
    fs::write(&nml_path, namelist.as_bytes())
        .map_err(|e| format!("write {}: {e}", nml_path.display()))?;

    let bin = resolve_mkgrd();
    // Surface the staged engine's size + mtime so a stale temp copy (an old engine
    // still being run after a rebuild) is visible at a glance.
    if let Ok(md) = fs::metadata(&bin) {
        let mtime = md
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = app.emit(
            "mkgrd://log",
            format!("engine: {bin}  ({} bytes · mtime {mtime})", md.len()),
        );
    }
    if let Ok(p) = env::var("EARTHMESH_MKGRD") {
        let p = p.trim().to_string();
        if !p.is_empty() && !Path::new(&p).is_file() {
            let _ = app.emit(
                "mkgrd://log",
                format!("note: $EARTHMESH_MKGRD='{p}' is not a file — ignoring it; using '{bin}'."),
            );
        }
    }
    // Pre-run sanity: the engine opens NetCDF inputs, so a placeholder/missing
    // path yields a cryptic `netcdf -51` error. Surface it clearly up front.
    {
        if lowered.mkgrd.landtype_file == "none" {
            let _ = app.emit(
                "mkgrd://log",
                "note: no land-cover file set (landtype_file='none') — fine for atmosphere/uniform \
                 meshes; land & ocean meshes need one (set it in step 3, Data Layers)."
                    .to_string(),
            );
            // Only a GLOBAL mesh with refinement on needs data. A regional run is
            // a pure clip (refine off, no data) and a global uniform run is fine
            // too — so only warn for the global + refine case.
            if lowered.mkgrd.refine && lowered.mkgrd.mask_domain_global {
                let _ = app.emit(
                    "mkgrd://log",
                    "⚠ refinement is on but no source data is set — the engine needs a landtype or \
                     threshold input to refine. For a data-free test, run a uniform mesh (refine off)."
                        .to_string(),
                );
            }
        }
        for l in &cfg.data_layers {
            if l.enabled && l.path.trim().is_empty() {
                let _ = app.emit(
                    "mkgrd://log",
                    format!("⚠ data layer '{}' is enabled but has no file set.", l.id),
                );
            }
        }
        // Hidden regional domains still need a mask source. Editable bbox domains
        // generate a plain `.nml` mask above, so they pass this check.
        if !lowered.mkgrd.mask_domain_global {
            let mf = lowered.mkgrd.mask_domain_fprefix.trim();
            if mf.is_empty() || mf == "/tmp" || mf.eq_ignore_ascii_case("none") {
                let _ = app.emit(
                    "mkgrd://log",
                    "⚠ hidden regional domains need a mask source file — editable bbox domains \
                     generate one automatically, but this shape is preserved-only in the GUI. Use a \
                     global/bbox domain, or provide a .nml/.nc mask source."
                        .to_string(),
                );
            }
        }
    }
    let _ = app.emit(
        "mkgrd://log",
        format!(
            "--- generated mkgrd.nml (file_dir={}) ---\n{namelist}--- end mkgrd.nml ---",
            lowered.mkgrd.file_dir()
        ),
    );
    let _ = app.emit("mkgrd://log", format!("$ {bin} {}", nml_path.display()));

    let mut child = Command::new(&bin)
        .arg(&nml_path)
        .current_dir(&run_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            format!(
                "could not start '{bin}': {e}. Build mkgrd.x and put it on PATH, \
                 or set EARTHMESH_MKGRD to its full path."
            )
        })?;
    // Record the PID so `kill_run` can stop this engine run on request.
    record_running_child(child.id())?;

    let out = child.stdout.take();
    let err = child.stderr.take();
    let a1 = app.clone();
    let gridfile_seen = Arc::new(Mutex::new(None::<String>));
    let gf_capture = gridfile_seen.clone();
    let t1 = thread::spawn(move || {
        if let Some(o) = out {
            for line in BufReader::new(o).lines().map_while(Result::ok) {
                // The engine prints `gridfile=<path>` for the mesh it produced.
                if let Some(rest) = line.strip_prefix("gridfile=") {
                    if let Ok(mut gridfile) = gf_capture.lock() {
                        *gridfile = Some(rest.trim().to_string());
                    }
                }
                let _ = a1.emit("mkgrd://log", line);
            }
        }
    });
    let a2 = app.clone();
    let t2 = thread::spawn(move || {
        if let Some(e) = err {
            for line in BufReader::new(e).lines().map_while(Result::ok) {
                let _ = a2.emit("mkgrd://log", format!("[stderr] {line}"));
            }
        }
    });

    let status = child.wait().map_err(|e| format!("wait failed: {e}"))?;
    clear_running_child();
    let _ = t1.join();
    let _ = t2.join();
    let code = status.code();
    let _ = app.emit(
        "mkgrd://log",
        format!(
            "— exited with {}",
            code.map(|c| c.to_string())
                .unwrap_or_else(|| "signal".into())
        ),
    );
    let gridfile = gridfile_seen
        .lock()
        .map_err(|_| "run gridfile state lock poisoned".to_string())?
        .clone();
    Ok(RunResult {
        ok: status.success(),
        code,
        outdir: run_dir_str,
        gridfile,
    })
}
