//! Mesh execution and quality extraction command handlers.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs};

use earthmesh_project::{
    nxp_to_km, read_close_mask_nml_points, read_lonlat_text_points, read_shapefile_polygon_rings,
    write_close_mask_nml, CloseMaskFormat, DomainConfig, LoweredProject, MeshCellKind,
    MeshDomainKind, ProjectConfig, ProjectLayerRole, RegionShape,
};
use serde_json::json;
use tauri::{AppHandle, Emitter};

use crate::auto_refine::scan_auto_refine_decisions;
use crate::dto::RunResult;
use crate::engine::{resolve_mkgrd, stage_threshold_layers};
use crate::mesh_paths::existing_file_path;
use crate::mesh_process::{clear_running_child, record_running_child};

const SHAPEFILE_MASK_SIMPLIFY_TOLERANCE_DEG: f64 = 0.002;
const METHOD_C_MIN_BASE_NXP: i32 = earthmesh_project::METHOD_C_MIN_BASE_NXP;

/// Parse project YAML, lower to engine namelist, run mkgrd.x and parse emitted
/// outputs.
#[tauri::command]
pub(crate) async fn run_project(
    app: AppHandle,
    yaml: String,
    outdir: Option<String>,
) -> Result<RunResult, String> {
    let mut cfg = ProjectConfig::from_yaml(&yaml).map_err(|e| format!("invalid project: {e}"))?;
    absolutize_gui_project_inputs(&mut cfg)?;
    if uses_standard_project_cli(&cfg) {
        return run_auto_refine_project_cli(app, cfg, outdir).await;
    }
    let attempt = run_project_attempt(app.clone(), cfg.to_yaml()?, outdir).await?;
    finish_project_run(&app, &cfg, attempt)
}

pub(crate) fn absolutize_gui_project_inputs(cfg: &mut ProjectConfig) -> Result<(), String> {
    let cwd = env::current_dir().map_err(|err| format!("resolve GUI working directory: {err}"))?;
    for layer in &mut cfg.data_layers {
        absolutize_input_path(&mut layer.path, &cwd);
    }
    if let DomainConfig::Regional { shape, .. } = &mut cfg.domain {
        match shape {
            RegionShape::Shapefile { path } | RegionShape::Close { path, .. } => {
                absolutize_input_path(path, &cwd);
            }
            RegionShape::Bbox { .. } | RegionShape::Circle { .. } => {}
        }
    }
    if let Some(close) = cfg.refinement.specified_close.as_mut() {
        absolutize_input_path(&mut close.path, &cwd);
    }
    absolutize_hydro_inputs(cfg, &cwd);
    if let Some(cama_root) = cfg
        .coupling
        .as_mut()
        .and_then(|coupling| coupling.cama_root.as_mut())
    {
        absolutize_input_path(cama_root, &cwd);
    }
    Ok(())
}

fn absolutize_hydro_inputs(cfg: &mut ProjectConfig, cwd: &Path) {
    let Some(hydro) = cfg.hydro_coast.as_mut() else {
        return;
    };
    absolutize_input_path(&mut hydro.merit_root, cwd);
    if let Some(cama_root) = hydro.cama_root.as_mut() {
        absolutize_input_path(cama_root, cwd);
    }
}

fn absolutize_input_path(path: &mut String, cwd: &Path) {
    let configured = PathBuf::from(path.trim());
    if !configured.is_absolute() {
        *path = cwd.join(configured).to_string_lossy().into_owned();
    }
}

pub(crate) fn uses_standard_project_cli(cfg: &ProjectConfig) -> bool {
    cfg.quality.on_violation == earthmesh_project::ViolationPolicy::AutoRefine
}

async fn run_auto_refine_project_cli(
    app: AppHandle,
    cfg: ProjectConfig,
    outdir: Option<String>,
) -> Result<RunResult, String> {
    let run_dir = project_run_dir(&cfg, outdir)?;
    let project_path = run_dir.join("project.yaml");
    fs::write(&project_path, cfg.to_yaml()?.as_bytes())
        .map_err(|err| format!("write {}: {err}", project_path.display()))?;

    let bin = resolve_mkgrd()?;
    let _ = app.emit(
        "mkgrd://log",
        "AutoRefine uses the shared CLI local-repair loop with strict candidate rollback."
            .to_string(),
    );
    let _ = app.emit(
        "mkgrd://log",
        format!("$ {bin} --project {}", project_path.display()),
    );
    let child = Command::new(&bin)
        .arg("--project")
        .arg(&project_path)
        .current_dir(&run_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| {
            format!(
                "could not start '{bin} --project': {err}. Build mkgrd.x and put it on PATH, or set EARTHMESH_MKGRD to its full path."
            )
        })?;
    let (ok, code, gridfile) = capture_mesh_child(&app, child)?;
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

fn finish_project_run(
    app: &AppHandle,
    cfg: &ProjectConfig,
    mut attempt: RunResult,
) -> Result<RunResult, String> {
    if !attempt.ok || cfg.hydro_coast.is_none() {
        return Ok(attempt);
    }
    let gridfile_raw = attempt
        .gridfile
        .as_deref()
        .ok_or_else(|| "Project hydro stage requires the engine to report gridfile".to_string())?;
    let run_dir = PathBuf::from(&attempt.outdir);
    let gridfile =
        existing_file_path(gridfile_raw, &run_dir).unwrap_or_else(|| run_dir.join(gridfile_raw));
    let project_path = run_dir.join("project.yaml");
    let source_namelist = run_dir.join("mkgrd.nml");
    let hydro_dir = earthmesh_project::project_hydro_output_dir(&gridfile);
    let bin = resolve_mkgrd()?;
    let _ = app.emit(
        "mkgrd://log",
        format!(
            "$ {bin} --project-hydro-postprocess {} {} {} {}",
            project_path.display(),
            gridfile.display(),
            hydro_dir.display(),
            source_namelist.display()
        ),
    );
    let mut child = Command::new(&bin)
        .arg("--project-hydro-postprocess")
        .arg(&project_path)
        .arg(&gridfile)
        .arg(&hydro_dir)
        .arg(&source_namelist)
        .current_dir(&run_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("start Project hydro stage with {bin}: {err}"))?;
    if let Err(err) = record_running_child(child.id()) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(err);
    }
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let final_gridfile_seen = Arc::new(Mutex::new(None::<String>));
    let final_gridfile_capture = final_gridfile_seen.clone();
    let stdout_app = app.clone();
    let stdout_thread = thread::spawn(move || {
        if let Some(stdout) = stdout {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(path) = line.strip_prefix("project_hydro_final_gridfile=") {
                    if let Ok(mut final_gridfile) = final_gridfile_capture.lock() {
                        *final_gridfile = Some(path.trim().to_string());
                    }
                }
                let _ = stdout_app.emit("mkgrd://log", line);
            }
        }
    });
    let stderr_app = app.clone();
    let stderr_thread = thread::spawn(move || {
        if let Some(stderr) = stderr {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let _ = stderr_app.emit("mkgrd://log", format!("[hydro stderr] {line}"));
            }
        }
    });
    let wait_result = child.wait();
    clear_running_child();
    let _ = stdout_thread.join();
    let _ = stderr_thread.join();
    let status = wait_result.map_err(|err| format!("wait for Project hydro stage: {err}"))?;
    if !status.success() {
        return Err(format!(
            "Project hydro stage failed with {}",
            status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "signal".to_string())
        ));
    }
    if let Some(final_gridfile) = final_gridfile_seen
        .lock()
        .map_err(|_| "hydro final gridfile state lock poisoned".to_string())?
        .clone()
    {
        attempt.gridfile = Some(final_gridfile);
    }
    enforce_final_quality_policy(cfg, attempt.gridfile.as_deref(), &project_path)?;
    Ok(attempt)
}

async fn run_project_attempt(
    app: AppHandle,
    yaml: String,
    outdir: Option<String>,
) -> Result<RunResult, String> {
    // Validate and use the shared project lowering. The GUI still stages regional
    // mask inputs in its selected run directory, but it must not invent different
    // global engine defaults from the CLI `--project` path.
    let cfg = ProjectConfig::from_yaml(&yaml).map_err(|e| format!("invalid project: {e}"))?;
    let mut lowered = cfg.try_lower()?;
    let run_dir = project_run_dir(&cfg, outdir)?;
    let run_dir_str = run_dir.to_string_lossy().into_owned();

    normalize_engine_input_paths(&mut lowered, &run_dir);

    // The engine CLEARS + recreates its output dir (`file_dir`). Put it in an
    // "output/" SUBfolder of run_dir so the engine never deletes run_dir itself —
    // which holds mkgrd.nml + project.yaml. file_dir = base_dir + experiment_name + "/".
    lowered.mkgrd.base_dir = format!("{run_dir_str}/");
    lowered.mkgrd.experiment_name = "output".to_string();
    let file_dir = run_dir.join("output");
    create_output_directories(&file_dir)?;

    let threshold_dir = run_dir.join("threshold");
    if stage_threshold_layers(&cfg, &threshold_dir, &run_dir)? {
        lowered.refine.threshold_dir = threshold_dir.to_string_lossy().into_owned();
        let _ = app.emit(
            "mkgrd://log",
            format!("✓ staged threshold layers in {}", threshold_dir.display()),
        );
    }

    let mut regional_specified_refine_used = false;

    // Regional bbox domain: keep bbox as the clip domain, but use a circle for
    // the local refinement target because Method-C expands circle parent halos
    // internally; bbox-in-bbox refinement crosses parent-boundary checks.
    if let DomainConfig::Regional {
        shape: RegionShape::Bbox { w, e, n, s },
        ..
    } = &cfg.domain
    {
        if !cfg.refinement.enabled {
            match configure_regional_bbox_domain_only(
                &mut lowered,
                *w,
                *e,
                *n,
                *s,
                &run_dir,
                "domain_bbox",
            ) {
                Ok(_) => {
                    let _ = app.emit(
                        "mkgrd://log",
                        format!(
                            "✓ regional bbox domain mask (W {w}, E {e}, N {n}, S {s}) — local refinement disabled"
                        ),
                    );
                }
                Err(err) => {
                    let _ = app.emit(
                        "mkgrd://log",
                        format!("⚠ could not write bbox domain mask: {err}"),
                    );
                }
            }
        } else {
            let target_nxp = lowered.mkgrd.nxp;
            let (base_nxp, refine_level) = regional_bbox_method_c_project_plan(target_nxp, &cfg);
            let local_nxp = base_nxp.saturating_mul(1_i32 << refine_level);
            let base_spacing_km = nxp_to_km(base_nxp);
            let local_spacing_km = base_spacing_km / (1_i32 << refine_level) as f64;
            let mask_family = if let Some(circle) = &cfg.refinement.specified_circle {
                regional_specified_refine_used = true;
                write_regional_bbox_circle_mask_family(
                    *w,
                    *e,
                    *n,
                    *s,
                    circle.lon,
                    circle.lat,
                    circle.radius_km,
                    &run_dir,
                    "domain_bbox",
                    "refine_circle",
                    refine_level,
                )
                .map(|(domain_prefix, refine_prefix)| {
                    (
                        domain_prefix,
                        "circle",
                        refine_prefix,
                        format!(
                            "note: specified circle refine target lon {}, lat {}, radius {:.1} km",
                            circle.lon, circle.lat, circle.radius_km
                        ),
                    )
                })
            } else {
                let (rw, re, rn, rs) = default_regional_bbox_refine_bbox(*w, *e, *n, *s);
                write_regional_bbox_inset_mask_family(
                *w,
                *e,
                *n,
                *s,
                rw,
                re,
                rn,
                rs,
                &run_dir,
                "domain_bbox",
                "refine_bbox",
                refine_level,
            )
            .map(|(domain_prefix, refine_prefix)| {
                (
                    domain_prefix,
                    "bbox",
                    refine_prefix,
                    format!(
                        "note: default bbox refine target W {rw:.3}, E {re:.3}, N {rn:.3}, S {rs:.3}"
                    ),
                )
            })
            };
            match mask_family {
                Ok((domain_prefix, refine_type, refine_prefix, refine_note)) => {
                    enable_regional_method_c_fast_path_with_refine_type(
                        &mut lowered,
                        "bbox",
                        refine_type,
                        &domain_prefix,
                        &refine_prefix,
                        base_nxp,
                        refine_level,
                    );
                    let _ = app.emit(
                    "mkgrd://log",
                    format!(
                        "✓ regional bbox Method-C fast path (W {w}, E {e}, N {n}, S {s}) — base NXP {base_nxp}, local refine level {refine_level}, local NXP {local_nxp}"
                    ),
                );
                    let _ = app.emit(
                    "mkgrd://log",
                    format!(
                        "note: Method-C writes NL%NXP/gridfile_NXP as the base grid ({base_nxp}, ≈{base_spacing_km:.1} km); local refined grid is NXP {local_nxp} (≈{local_spacing_km:.1} km)"
                    ),
                );
                    let _ = app.emit("mkgrd://log", refine_note);
                }
                Err(err) => {
                    let _ = app.emit(
                        "mkgrd://log",
                        format!("⚠ could not write bbox masks: {err}"),
                    );
                }
            }
        }
    }

    if let DomainConfig::Regional {
        shape: RegionShape::Shapefile { path },
        ..
    } = &cfg.domain
    {
        if !cfg.refinement.enabled {
            let domain_prefix = write_shapefile_close_domain_mask(path, &run_dir, "domain_shp")
                .map_err(|e| format!("convert watershed shp to close mask: {e}"))?;
            configure_regional_close_domain_only(&mut lowered, &domain_prefix);
            let _ = app.emit(
                "mkgrd://log",
                format!(
                    "✓ watershed SHP close domain mask prefix {} — local refinement disabled",
                    domain_prefix.display()
                ),
            );
        } else {
            let target_nxp = lowered.mkgrd.nxp;
            let target_spacing_km = nxp_to_km(target_nxp);
            let (base_nxp, refine_level) = regional_method_c_project_plan(target_nxp, &cfg);
            let base_spacing_km = nxp_to_km(base_nxp);
            let (domain_prefix, refine_prefix) = write_shapefile_close_masks_with_parent_masks(
                path,
                &run_dir,
                refine_level,
                base_nxp,
                !hfield_enabled(&lowered),
            )
            .map_err(|e| format!("convert watershed shp to close mask: {e}"))?;
            enable_regional_method_c_fast_path(
                &mut lowered,
                "close",
                &domain_prefix,
                &refine_prefix,
                base_nxp,
                refine_level,
            );
            let _ = app.emit(
                "mkgrd://log",
                format!(
                    "✓ watershed SHP Method-C fast path prefix {} — base NXP {base_nxp}, local refine level {refine_level}, target NXP {target_nxp}",
                    refine_prefix.display(),
                ),
            );
            let _ = app.emit(
                "mkgrd://log",
                format!(
                    "note: Method-C writes NL%NXP/gridfile_NXP as the base grid ({base_nxp}, ≈{base_spacing_km:.1} km); local target grid is NXP {target_nxp} (≈{target_spacing_km:.1} km)"
                ),
            );
        }
    }

    if let DomainConfig::Regional {
        shape: RegionShape::Close { path, format, .. },
        ..
    } = &cfg.domain
    {
        if !cfg.refinement.enabled {
            if let Some(domain_prefix) = write_close_domain_only_masks(path, *format, &run_dir)
                .map_err(|e| format!("prepare close domain mask: {e}"))?
            {
                configure_regional_close_domain_only(&mut lowered, &domain_prefix);
                let _ = app.emit(
                    "mkgrd://log",
                    format!(
                        "✓ close domain mask prefix {} — local refinement disabled",
                        domain_prefix.display()
                    ),
                );
            } else {
                lowered.mkgrd.mask_domain_type = "close".to_string();
                lowered.mkgrd.mask_domain_fprefix = path.clone();
                let _ = app.emit(
                    "mkgrd://log",
                    "note: close NetCDF domain is passed directly to the engine; local refinement disabled.".to_string(),
                );
            }
        } else {
            let target_nxp = lowered.mkgrd.nxp;
            let target_spacing_km = nxp_to_km(target_nxp);
            let (base_nxp, refine_level) = regional_method_c_project_plan(target_nxp, &cfg);
            let base_spacing_km = nxp_to_km(base_nxp);
            if let Some((domain_prefix, refine_prefix)) =
                write_close_domain_masks_with_parent_masks(
                    path,
                    *format,
                    &run_dir,
                    refine_level,
                    base_nxp,
                    !hfield_enabled(&lowered),
                )
                .map_err(|e| format!("prepare close domain mask: {e}"))?
            {
                enable_regional_method_c_fast_path(
                    &mut lowered,
                    "close",
                    &domain_prefix,
                    &refine_prefix,
                    base_nxp,
                    refine_level,
                );
                let _ = app.emit(
                    "mkgrd://log",
                    format!(
                        "✓ close domain Method-C fast path prefix {} — base NXP {base_nxp}, local refine level {refine_level}, target NXP {target_nxp}",
                        refine_prefix.display(),
                    ),
                );
                let _ = app.emit(
                    "mkgrd://log",
                    format!(
                        "note: Method-C writes NL%NXP/gridfile_NXP as the base grid ({base_nxp}, ≈{base_spacing_km:.1} km); local target grid is NXP {target_nxp} (≈{target_spacing_km:.1} km)"
                    ),
                );
            } else {
                lowered.mkgrd.mask_domain_type = "close".to_string();
                lowered.mkgrd.mask_domain_fprefix = path.clone();
                let _ = app.emit(
                    "mkgrd://log",
                    "note: close NetCDF domain is passed directly to the engine; GUI does not inspect NetCDF/HDF5.".to_string(),
                );
            }
        }
    }

    if cfg.refinement.specified_circle.is_some()
        || cfg.refinement.specified_bbox.is_some()
        || cfg.refinement.specified_close.is_some()
    {
        let regional_fast_path = matches!(
            cfg.domain,
            DomainConfig::Regional {
                shape: RegionShape::Bbox { .. }
                    | RegionShape::Shapefile { .. }
                    | RegionShape::Close { .. },
                ..
            }
        );
        if !regional_fast_path {
            let (mask_type, prefix) = write_specified_refinement_mask(
                &cfg.refinement,
                &run_dir,
                lowered.refine.max_iter_spc,
            )
            .map_err(|e| format!("write specified refinement mask: {e}"))?;
            lowered.refine.mask_refine_spc_type = mask_type.to_string();
            lowered.refine.mask_refine_spc_fprefix = prefix.to_string_lossy().into_owned();
            let refine_level = lowered.refine.max_iter_spc.max(1) as usize;
            set_refine_transition_rows(&mut lowered, refine_level);
        } else if !regional_specified_refine_used {
            let note = if matches!(
                cfg.domain,
                DomainConfig::Regional {
                    shape: RegionShape::Bbox { .. },
                    ..
                }
            ) {
                "note: regional bbox Method-C uses an inset bbox refine target by default; specified circles override it, while specified bbox/close shapes are ignored in regional mode."
            } else {
                "note: regional Method-C derives nested refine masks from the domain; separate specified-refine shapes are ignored in regional mode."
            };
            let _ = app.emit("mkgrd://log", note.to_string());
        }
    }

    let namelist = engine_namelist(&lowered);
    // project.yaml (provenance) + mkgrd.nml (engine input) both live in run_dir.
    let yaml_path = run_dir.join("project.yaml");
    fs::write(&yaml_path, yaml.as_bytes())
        .map_err(|e| format!("write {}: {e}", yaml_path.display()))?;
    let nml_path = run_dir.join("mkgrd.nml");
    fs::write(&nml_path, namelist.as_bytes())
        .map_err(|e| format!("write {}: {e}", nml_path.display()))?;

    let bin = resolve_mkgrd()?;
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

    let child = Command::new(&bin)
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
    let (ok, code, gridfile) = capture_mesh_child(&app, child)?;
    if ok {
        let resolved_gridfile = gridfile
            .as_deref()
            .and_then(|path| existing_file_path(path, &run_dir));
        let quality_gridfile = resolved_gridfile
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned());
        enforce_backend_quality_policy(
            &cfg,
            quality_gridfile.as_deref().or(gridfile.as_deref()),
            &yaml_path,
        )?;
    }
    Ok(RunResult {
        ok,
        code,
        outdir: run_dir_str,
        gridfile,
        auto_refine_decisions: Vec::new(),
    })
}

fn project_run_dir(cfg: &ProjectConfig, outdir: Option<String>) -> Result<PathBuf, String> {
    // `outdir` is the BASE output path. Every artifact is grouped under a
    // filesystem-safe project folder, including the shared CLI AutoRefine run.
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
    let run_dir = Path::new(&base).join(if name.is_empty() { "mesh" } else { &name });
    fs::create_dir_all(&run_dir)
        .map_err(|error| format!("mkdir {}: {error}", run_dir.display()))?;
    fs::canonicalize(&run_dir)
        .map_err(|error| format!("resolve run directory {}: {error}", run_dir.display()))
}

fn capture_mesh_child(
    app: &AppHandle,
    mut child: Child,
) -> Result<(bool, Option<i32>, Option<String>), String> {
    if let Err(error) = record_running_child(child.id()) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdout_app = app.clone();
    let gridfile_seen = Arc::new(Mutex::new(None::<String>));
    let gridfile_capture = gridfile_seen.clone();
    let stdout_thread = thread::spawn(move || {
        if let Some(stdout) = stdout {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(path) = line.strip_prefix("gridfile=") {
                    if let Ok(mut gridfile) = gridfile_capture.lock() {
                        *gridfile = Some(path.trim().to_string());
                    }
                }
                let _ = stdout_app.emit("mkgrd://log", line);
            }
        }
    });
    let stderr_app = app.clone();
    let stderr_thread = thread::spawn(move || {
        if let Some(stderr) = stderr {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let _ = stderr_app.emit("mkgrd://log", format!("[stderr] {line}"));
            }
        }
    });

    let wait_result = child.wait();
    clear_running_child();
    let _ = stdout_thread.join();
    let _ = stderr_thread.join();
    let status = wait_result.map_err(|error| format!("wait failed: {error}"))?;
    let code = status.code();
    let _ = app.emit(
        "mkgrd://log",
        format!(
            "— exited with {}",
            code.map(|value| value.to_string())
                .unwrap_or_else(|| "signal".into())
        ),
    );
    let gridfile = gridfile_seen
        .lock()
        .map_err(|_| "run gridfile state lock poisoned".to_string())?
        .clone();
    Ok((status.success(), code, gridfile))
}

pub(crate) fn enforce_backend_quality_policy(
    cfg: &ProjectConfig,
    gridfile: Option<&str>,
    project_path: &Path,
) -> Result<(), String> {
    if cfg.quality.on_violation != earthmesh_project::ViolationPolicy::Block {
        return Ok(());
    }
    // Hydro projects are quality-gated only after the closed loop has applied
    // the refinement plan and reported its final gridfile.
    if cfg.hydro_coast.is_some() {
        return Ok(());
    }
    enforce_final_quality_policy(cfg, gridfile, project_path)
}

fn enforce_final_quality_policy(
    cfg: &ProjectConfig,
    gridfile: Option<&str>,
    project_path: &Path,
) -> Result<(), String> {
    if cfg.quality.on_violation != earthmesh_project::ViolationPolicy::Block {
        return Ok(());
    }
    let gridfile = gridfile
        .ok_or_else(|| "quality block policy requires the engine to report gridfile".to_string())?;
    let quality = crate::mesh_outputs::project_mesh_quality(project_path, gridfile)?;
    if quality.verdict.eq_ignore_ascii_case("fail") {
        return Err("project quality gate failed under block policy".to_string());
    }
    Ok(())
}

pub(crate) fn create_output_directories(file_dir: &Path) -> Result<(), String> {
    for sub in ["", "result", "contain", "restart"] {
        let path = file_dir.join(sub);
        fs::create_dir_all(&path)
            .map_err(|err| format!("create output directory {}: {err}", path.display()))?;
    }
    Ok(())
}

fn hfield_enabled(lowered: &LoweredProject) -> bool {
    matches!(&lowered.hfield, Some(hfield) if hfield.enabled)
}

pub(crate) fn engine_namelist(lowered: &LoweredProject) -> String {
    let nml = lowered.to_namelist();
    // GUI already lowered + staged data layers. Keeping this provenance block in
    // the engine input makes the CLI lower it again, which can re-enable
    // calculated refinement after Method-C intentionally disabled it.
    if let Some((engine, _datalayers)) = nml.split_once("\n&datalayers\n") {
        format!("{engine}\n")
    } else {
        nml
    }
}

#[cfg(test)]
pub(crate) fn write_shapefile_close_masks(
    shp: impl AsRef<Path>,
    run_dir: impl AsRef<Path>,
    refine_degree: usize,
    base_nxp: i32,
) -> std::io::Result<(PathBuf, PathBuf)> {
    write_shapefile_close_masks_with_parent_masks(shp, run_dir, refine_degree, base_nxp, true)
}

pub(crate) fn write_shapefile_close_masks_with_parent_masks(
    shp: impl AsRef<Path>,
    run_dir: impl AsRef<Path>,
    refine_degree: usize,
    base_nxp: i32,
    include_parent_masks: bool,
) -> std::io::Result<(PathBuf, PathBuf)> {
    let rings = read_shapefile_polygon_rings(shp.as_ref())?;
    write_close_mask_family(
        &rings,
        run_dir,
        "domain_shp",
        "refine_shp",
        refine_degree,
        base_nxp,
        include_parent_masks,
    )
}

pub(crate) fn write_shapefile_close_domain_mask(
    shp: impl AsRef<Path>,
    run_dir: impl AsRef<Path>,
    domain_stem: &str,
) -> std::io::Result<PathBuf> {
    let rings = read_shapefile_polygon_rings(shp.as_ref())?;
    write_close_domain_mask(&rings, run_dir, domain_stem)
}

pub(crate) fn write_close_domain_only_masks(
    path: &str,
    format: CloseMaskFormat,
    run_dir: &Path,
) -> std::io::Result<Option<PathBuf>> {
    let resolved = existing_file_path(path, run_dir).unwrap_or_else(|| PathBuf::from(path));
    let rings = match format {
        CloseMaskFormat::PolygonShp => read_shapefile_polygon_rings(&resolved)?,
        CloseMaskFormat::Nml => vec![read_close_mask_nml_points(&resolved)?],
        CloseMaskFormat::LonLatText => vec![read_lonlat_text_points(&resolved)?],
        CloseMaskFormat::Netcdf => return Ok(None),
    };
    write_close_domain_mask(&rings, run_dir, "domain_close").map(Some)
}

#[cfg(test)]
pub(crate) fn write_close_domain_masks(
    path: &str,
    format: CloseMaskFormat,
    run_dir: &Path,
    refine_degree: usize,
    base_nxp: i32,
) -> std::io::Result<Option<(PathBuf, PathBuf)>> {
    write_close_domain_masks_with_parent_masks(path, format, run_dir, refine_degree, base_nxp, true)
}

pub(crate) fn write_close_domain_masks_with_parent_masks(
    path: &str,
    format: CloseMaskFormat,
    run_dir: &Path,
    refine_degree: usize,
    base_nxp: i32,
    include_parent_masks: bool,
) -> std::io::Result<Option<(PathBuf, PathBuf)>> {
    let resolved = existing_file_path(path, run_dir).unwrap_or_else(|| PathBuf::from(path));
    let rings = match format {
        CloseMaskFormat::PolygonShp => read_shapefile_polygon_rings(&resolved)?,
        CloseMaskFormat::Nml => vec![read_close_mask_nml_points(&resolved)?],
        CloseMaskFormat::LonLatText => vec![read_lonlat_text_points(&resolved)?],
        CloseMaskFormat::Netcdf => return Ok(None),
    };
    write_close_mask_family(
        &rings,
        run_dir,
        "domain_close",
        "refine_close",
        refine_degree,
        base_nxp,
        include_parent_masks,
    )
    .map(Some)
}

fn write_close_domain_mask(
    rings: &[Vec<(f64, f64)>],
    run_dir: impl AsRef<Path>,
    domain_stem: &str,
) -> std::io::Result<PathBuf> {
    let domain_prefix = run_dir.as_ref().join(domain_stem);
    for (index, ring) in rings.iter().enumerate() {
        let ring = simplify_shapefile_mask_ring(ring);
        let path = run_dir
            .as_ref()
            .join(format!("{domain_stem}_{:03}.nml", index + 1));
        write_close_mask_nml(&path, &ring, 0)?;
    }
    Ok(domain_prefix)
}

fn write_close_mask_family(
    rings: &[Vec<(f64, f64)>],
    run_dir: impl AsRef<Path>,
    domain_stem: &str,
    refine_stem: &str,
    refine_degree: usize,
    base_nxp: i32,
    include_parent_masks: bool,
) -> std::io::Result<(PathBuf, PathBuf)> {
    let domain_prefix = run_dir.as_ref().join(domain_stem);
    let refine_prefix = run_dir.as_ref().join(refine_stem);
    for (index, ring) in rings.iter().enumerate() {
        let ring = simplify_shapefile_mask_ring(ring);
        let path = run_dir
            .as_ref()
            .join(format!("{domain_stem}_{:03}.nml", index + 1));
        write_close_mask_nml(&path, &ring, refine_degree)?;
    }
    if include_parent_masks && refine_degree > 1 {
        for level in 1..refine_degree {
            let bbox = expanded_parent_bbox_ring(rings, level, refine_degree, base_nxp);
            let path = run_dir
                .as_ref()
                .join(format!("{refine_stem}_{level:03}_001.nml"));
            write_close_mask_nml(&path, &bbox, level)?;
        }
    }
    for (index, ring) in rings.iter().enumerate() {
        let ring = simplify_shapefile_mask_ring(ring);
        let path = run_dir.as_ref().join(format!(
            "{refine_stem}_{refine_degree:03}_{:03}.nml",
            index + 1
        ));
        write_close_mask_nml(&path, &ring, refine_degree)?;
    }
    Ok((domain_prefix, refine_prefix))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn write_regional_bbox_circle_mask_family(
    w: f64,
    e: f64,
    n: f64,
    s: f64,
    circle_lon: f64,
    circle_lat: f64,
    circle_radius_km: f64,
    run_dir: impl AsRef<Path>,
    domain_stem: &str,
    refine_stem: &str,
    refine_degree: usize,
) -> std::io::Result<(PathBuf, PathBuf)> {
    let domain_prefix = run_dir.as_ref().join(domain_stem);
    let refine_prefix = run_dir.as_ref().join(refine_stem);
    write_bbox_mask_nml(
        &run_dir.as_ref().join(format!("{domain_stem}_001.nml")),
        w,
        e,
        n,
        s,
        0,
    )?;
    write_circle_mask_nml(
        &run_dir.as_ref().join(format!("{refine_stem}_001.nml")),
        circle_lon,
        circle_lat,
        circle_radius_km,
        refine_degree,
    )?;
    Ok((domain_prefix, refine_prefix))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn write_regional_bbox_inset_mask_family(
    w: f64,
    e: f64,
    n: f64,
    s: f64,
    refine_w: f64,
    refine_e: f64,
    refine_n: f64,
    refine_s: f64,
    run_dir: impl AsRef<Path>,
    domain_stem: &str,
    refine_stem: &str,
    refine_degree: usize,
) -> std::io::Result<(PathBuf, PathBuf)> {
    let domain_prefix = run_dir.as_ref().join(domain_stem);
    let refine_prefix = run_dir.as_ref().join(refine_stem);
    write_bbox_mask_nml(
        &run_dir.as_ref().join(format!("{domain_stem}_001.nml")),
        w,
        e,
        n,
        s,
        0,
    )?;
    write_bbox_mask_nml(
        &run_dir.as_ref().join(format!("{refine_stem}_001.nml")),
        refine_w,
        refine_e,
        refine_n,
        refine_s,
        refine_degree,
    )?;
    Ok((domain_prefix, refine_prefix))
}

pub(crate) fn configure_regional_bbox_domain_only(
    lowered: &mut LoweredProject,
    w: f64,
    e: f64,
    n: f64,
    s: f64,
    run_dir: impl AsRef<Path>,
    domain_stem: &str,
) -> std::io::Result<PathBuf> {
    let domain_prefix = run_dir.as_ref().join(domain_stem);
    write_bbox_mask_nml(
        &run_dir.as_ref().join(format!("{domain_stem}_001.nml")),
        w,
        e,
        n,
        s,
        0,
    )?;
    lowered.mkgrd.refine = false;
    lowered.mkgrd.mask_domain_type = "bbox".to_string();
    lowered.mkgrd.mask_domain_fprefix = domain_prefix.to_string_lossy().into_owned();
    lowered.refine.refine_spc = false;
    lowered.refine.refine_cal = false;
    Ok(domain_prefix)
}

pub(crate) fn configure_regional_close_domain_only(
    lowered: &mut LoweredProject,
    domain_prefix: &Path,
) {
    lowered.mkgrd.refine = false;
    lowered.mkgrd.mask_domain_global = false;
    lowered.mkgrd.mask_domain_type = "close".to_string();
    lowered.mkgrd.mask_domain_fprefix = domain_prefix.to_string_lossy().into_owned();
    lowered.refine.refine_spc = false;
    lowered.refine.refine_cal = false;
    lowered.refine.max_iter_spc = 0;
    lowered.refine.max_iter_cal = 0;
}

pub(crate) fn default_regional_bbox_refine_bbox(
    w: f64,
    e: f64,
    n: f64,
    s: f64,
) -> (f64, f64, f64, f64) {
    let lon_pad = ((e - w).abs() * 0.12).clamp(0.25, 5.0);
    let lat_pad = ((n - s).abs() * 0.12).clamp(0.25, 5.0);
    let refine_w = (w + lon_pad).min((w + e) * 0.5);
    let refine_e = (e - lon_pad).max((w + e) * 0.5);
    let refine_n = (n - lat_pad).max((n + s) * 0.5);
    let refine_s = (s + lat_pad).min((n + s) * 0.5);
    (refine_w, refine_e, refine_n, refine_s)
}

fn expanded_parent_bbox_ring(
    rings: &[Vec<(f64, f64)>],
    level: usize,
    refine_degree: usize,
    base_nxp: i32,
) -> Vec<(f64, f64)> {
    let (mut w, mut e, mut s, mut n) = (180.0_f64, -180.0_f64, 90.0_f64, -90.0_f64);
    for (lon, lat) in rings.iter().flatten().copied() {
        w = w.min(lon);
        e = e.max(lon);
        s = s.min(lat);
        n = n.max(lat);
    }
    let mid_lat = ((s + n) * 0.5).to_radians();
    let meters = parent_halo_meters(level, refine_degree, base_nxp);
    let lat_pad = meters / 111_320.0;
    let lon_pad = lat_pad / mid_lat.cos().abs().max(0.2);
    w = (w - lon_pad).max(-180.0);
    e = (e + lon_pad).min(180.0);
    s = (s - lat_pad).max(-90.0);
    n = (n + lat_pad).min(90.0);
    vec![(w, s), (e, s), (e, n), (w, n)]
}

fn parent_halo_meters(level: usize, refine_degree: usize, base_nxp: i32) -> f64 {
    let base_spacing = std::f64::consts::TAU * 6_371_000.0 / (5.0 * base_nxp.max(1) as f64);
    (level..refine_degree)
        .map(|level| {
            let rows = if level == 3 { 3.0 } else { 4.0 };
            rows * base_spacing / 2.0_f64.powi((level - 1) as i32)
        })
        .sum()
}

fn write_bbox_mask_nml(
    path: &Path,
    w: f64,
    e: f64,
    n: f64,
    s: f64,
    refine_degree: usize,
) -> std::io::Result<()> {
    fs::write(
        path,
        format!("bbox_num = 1\nbbox_refine = {refine_degree}\n{w:.10} {e:.10} {n:.10} {s:.10}\n"),
    )
}

fn write_circle_mask_nml(
    path: &Path,
    lon: f64,
    lat: f64,
    radius_km: f64,
    refine_degree: usize,
) -> std::io::Result<()> {
    fs::write(
        path,
        format!(
            "circle_num = 1\ncircle_refine = {refine_degree}\n{lon:.10} {lat:.10} {radius_km:.10}\n"
        ),
    )
}

pub(crate) fn write_specified_refinement_mask(
    refinement: &earthmesh_project::RefinementRecipe,
    run_dir: &Path,
    refine_level: i32,
) -> std::io::Result<(&'static str, std::path::PathBuf)> {
    let level = refine_level.max(1);
    let level_usize = level as usize;
    if let Some(circle) = &refinement.specified_circle {
        let prefix = run_dir.join("specified_circle");
        let path = run_dir.join("specified_circle_001.nml");
        write_circle_mask_nml(&path, circle.lon, circle.lat, circle.radius_km, level_usize)?;
        return Ok(("circle", prefix));
    }
    if let Some(bbox) = &refinement.specified_bbox {
        let prefix = run_dir.join("specified_bbox");
        let path = run_dir.join("specified_bbox_001.nml");
        fs::write(
            path,
            format!(
                "bbox_num = 1\nbbox_refine = {level}\n{} {} {} {}\n",
                bbox.w, bbox.e, bbox.n, bbox.s
            ),
        )?;
        return Ok(("bbox", prefix));
    }
    if let Some(close) = &refinement.specified_close {
        let prefix = run_dir.join("specified_close");
        let resolved =
            existing_file_path(&close.path, run_dir).unwrap_or_else(|| PathBuf::from(&close.path));
        let extension = resolved
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if matches!(extension.as_str(), "nc" | "nc4") {
            fs::copy(
                &resolved,
                run_dir.join(format!("specified_close_001.{extension}")),
            )?;
            return Ok(("close", prefix));
        }
        let rings = match extension.as_str() {
            "shp" => read_shapefile_polygon_rings(&resolved)?
                .into_iter()
                .map(|ring| simplify_shapefile_mask_ring(&ring))
                .collect(),
            "nml" => vec![read_close_mask_nml_points(&resolved)?],
            "txt" | "csv" => vec![read_lonlat_text_points(&resolved)?],
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "unsupported specified close mask format",
                ))
            }
        };
        for (index, ring) in rings.into_iter().enumerate() {
            let path = run_dir.join(format!("specified_close_{:03}.nml", index + 1));
            write_close_mask_nml(&path, &ring, level_usize)?;
        }
        return Ok(("close", prefix));
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        "specified refinement shape is missing",
    ))
}

pub(crate) fn normalize_engine_input_paths(lowered: &mut LoweredProject, run_dir: &Path) {
    for layer in &mut lowered.data_layers.layers {
        if let Some(path) = existing_file_path(&layer.path, run_dir) {
            layer.path = path.to_string_lossy().into_owned();
        }
    }
    lowered.mkgrd.landtype_file =
        existing_run_file(&lowered.mkgrd.landtype_file, run_dir).unwrap_or_else(|| "none".into());
    if Path::new(&lowered.mkgrd.landtype_file)
        .file_name()
        .and_then(|name| name.to_str())
        == Some("landtype_igbp_update.nc")
    {
        lowered.mkgrd.gridnum_perdegree = 240;
    }
    lowered.mkgrd.mode_file =
        existing_run_file(&lowered.mkgrd.mode_file, run_dir).unwrap_or_else(|| "none".into());
}

pub(crate) fn existing_run_file(path: &str, run_dir: &Path) -> Option<String> {
    existing_file_path(path, run_dir).map(|path| path.to_string_lossy().into_owned())
}

pub(crate) fn simplify_shapefile_mask_ring(ring: &[(f64, f64)]) -> Vec<(f64, f64)> {
    simplify_closed_ring(ring.to_vec(), SHAPEFILE_MASK_SIMPLIFY_TOLERANCE_DEG)
}

fn simplify_closed_ring(coordinates: Vec<(f64, f64)>, tolerance_deg: f64) -> Vec<(f64, f64)> {
    if tolerance_deg <= 0.0 || coordinates.len() <= 3 {
        return coordinates;
    }
    let mut keep = vec![false; coordinates.len()];
    keep[0] = true;
    simplify_ring_segment(
        &coordinates,
        0,
        coordinates.len() - 1,
        tolerance_deg,
        &mut keep,
    );
    simplify_ring_segment(
        &coordinates,
        coordinates.len() - 1,
        0,
        tolerance_deg,
        &mut keep,
    );
    let simplified = coordinates
        .iter()
        .copied()
        .zip(keep)
        .filter_map(|(point, keep)| keep.then_some(point))
        .collect::<Vec<_>>();
    let simplified = remove_near_collinear_closed_ring_vertices(simplified, tolerance_deg);
    if simplified.len() >= 3 {
        simplified
    } else {
        coordinates
    }
}

fn remove_near_collinear_closed_ring_vertices(
    mut coordinates: Vec<(f64, f64)>,
    tolerance_deg: f64,
) -> Vec<(f64, f64)> {
    if coordinates.len() <= 3 {
        return coordinates;
    }
    loop {
        let mut removed = false;
        let len = coordinates.len();
        for index in 0..len {
            let previous = coordinates[(index + len - 1) % len];
            let current = coordinates[index];
            let next = coordinates[(index + 1) % len];
            if point_line_distance_deg(current, previous, next) <= tolerance_deg {
                coordinates.remove(index);
                removed = true;
                break;
            }
        }
        if !removed || coordinates.len() <= 3 {
            return coordinates;
        }
    }
}

fn simplify_ring_segment(
    coordinates: &[(f64, f64)],
    start: usize,
    end: usize,
    tolerance_deg: f64,
    keep: &mut [bool],
) {
    let segment_indices = ring_segment_indices(coordinates.len(), start, end);
    if segment_indices.len() <= 2 {
        keep[start] = true;
        keep[end] = true;
        return;
    }
    let start_point = coordinates[start];
    let end_point = coordinates[end];
    let mut farthest_index = start;
    let mut farthest_distance = 0.0_f64;
    for &index in segment_indices
        .iter()
        .skip(1)
        .take(segment_indices.len() - 2)
    {
        let distance = point_line_distance_deg(coordinates[index], start_point, end_point);
        if distance > farthest_distance {
            farthest_distance = distance;
            farthest_index = index;
        }
    }
    keep[start] = true;
    keep[end] = true;
    if farthest_distance > tolerance_deg {
        keep[farthest_index] = true;
        simplify_ring_segment(coordinates, start, farthest_index, tolerance_deg, keep);
        simplify_ring_segment(coordinates, farthest_index, end, tolerance_deg, keep);
    }
}

fn ring_segment_indices(len: usize, start: usize, end: usize) -> Vec<usize> {
    let mut indices = vec![start];
    let mut index = start;
    while index != end {
        index = (index + 1) % len;
        indices.push(index);
    }
    indices
}

fn point_line_distance_deg(point: (f64, f64), line_start: (f64, f64), line_end: (f64, f64)) -> f64 {
    let (px, py) = point;
    let (x1, y1) = line_start;
    let (x2, y2) = line_end;
    let dx = x2 - x1;
    let dy = y2 - y1;
    let length_sq = dx * dx + dy * dy;
    if length_sq == 0.0 {
        return ((px - x1).powi(2) + (py - y1).powi(2)).sqrt();
    }
    let t = (((px - x1) * dx + (py - y1) * dy) / length_sq).clamp(0.0, 1.0);
    let proj_x = x1 + t * dx;
    let proj_y = y1 + t * dy;
    ((px - proj_x).powi(2) + (py - proj_y).powi(2)).sqrt()
}

pub(crate) fn regional_method_c_plan(target_nxp: i32) -> (i32, usize) {
    let level = if target_nxp >= 192 {
        3
    } else if target_nxp >= 64 {
        2
    } else {
        1
    };
    let divisor = 1_i32 << level;
    let base_nxp = ((target_nxp + divisor - 1) / divisor).max(6);
    (base_nxp, level as usize)
}

pub(crate) fn regional_method_c_project_plan(target_nxp: i32, cfg: &ProjectConfig) -> (i32, usize) {
    let active_refine_source = cfg.refinement.enabled && project_has_method_c_refine_source(cfg);
    let default_level = if !active_refine_source && is_ocean_tri_close_domain(cfg) {
        if target_nxp >= 192 {
            2
        } else {
            1
        }
    } else {
        regional_method_c_plan(target_nxp).1
    };
    let explicit_spc_level = cfg
        .expert
        .max_iter_spc
        .and_then(|level| usize::try_from(level).ok())
        .filter(|level| *level > 0);
    let mut level = if let Some(level) = explicit_spc_level {
        level
    } else if active_refine_source && cfg.refinement.max_passes > 0 {
        usize::from(cfg.refinement.max_passes)
    } else {
        default_level
    };
    level = level.clamp(1, regional_method_c_level_cap(target_nxp));
    let divisor = 1_i32 << level;
    let base_nxp = ((target_nxp + divisor - 1) / divisor).max(METHOD_C_MIN_BASE_NXP);
    (base_nxp, level)
}

pub(crate) fn regional_bbox_method_c_project_plan(
    target_nxp: i32,
    cfg: &ProjectConfig,
) -> (i32, usize) {
    let (_, level) = regional_method_c_project_plan(target_nxp, cfg);
    (target_nxp.max(METHOD_C_MIN_BASE_NXP), level)
}

fn regional_method_c_level_cap(target_nxp: i32) -> usize {
    usize::from(earthmesh_project::auto_refine_level_cap(target_nxp))
}

fn project_has_method_c_refine_source(cfg: &ProjectConfig) -> bool {
    cfg.refinement.specified_circle.is_some()
        || cfg.refinement.specified_bbox.is_some()
        || cfg.refinement.specified_close.is_some()
        || cfg.data_layers.iter().any(|layer| {
            layer.enabled
                && !layer.path.trim().is_empty()
                && matches!(layer.role, ProjectLayerRole::Threshold(_))
        })
}

fn is_ocean_tri_close_domain(cfg: &ProjectConfig) -> bool {
    cfg.target.kind == MeshDomainKind::Ocean
        && cfg.target.cell == MeshCellKind::Tri
        && matches!(
            cfg.domain,
            DomainConfig::Regional {
                shape: RegionShape::Close { .. },
                ..
            }
        )
}

pub(crate) fn enable_regional_method_c_fast_path(
    lowered: &mut LoweredProject,
    mask_type: &str,
    domain_prefix: &Path,
    refine_prefix: &Path,
    base_nxp: i32,
    refine_level: usize,
) {
    enable_regional_method_c_fast_path_with_refine_type(
        lowered,
        mask_type,
        mask_type,
        domain_prefix,
        refine_prefix,
        base_nxp,
        refine_level,
    );
}

pub(crate) fn enable_regional_method_c_fast_path_with_refine_type(
    lowered: &mut LoweredProject,
    domain_mask_type: &str,
    refine_mask_type: &str,
    domain_prefix: &Path,
    refine_prefix: &Path,
    base_nxp: i32,
    refine_level: usize,
) {
    lowered.mkgrd.nxp = base_nxp;
    lowered.mkgrd.mask_domain_global = false;
    lowered.mkgrd.mask_domain_type = domain_mask_type.to_string();
    lowered.mkgrd.mask_domain_fprefix = domain_prefix.to_string_lossy().into_owned();
    lowered.mkgrd.mask_restart = false;
    lowered.mkgrd.refine = true;
    lowered.refine.refine_spc = true;
    lowered.refine.refine_cal = false;
    lowered.refine.max_iter_spc = refine_level as i32;
    lowered.refine.max_iter_cal = 0;
    set_refine_transition_rows(lowered, refine_level);
    lowered.refine.mask_refine_spc_type = refine_mask_type.to_string();
    lowered.refine.mask_refine_spc_fprefix = refine_prefix.to_string_lossy().into_owned();
    lowered.refine.is_transition = true;
    lowered.refine.weak_concav_eliminate = true;
    lowered.refine.spring_global_type = 0;
    lowered.refine.spring_regional_type = 1;
    if !lowered.refine.niter_refine_specified {
        lowered.refine.niter_refine = 2000;
        lowered.refine.niter_refine_specified = true;
    }
}

fn set_refine_transition_rows(lowered: &mut LoweredProject, refine_level: usize) {
    let smooth_ocean_tri =
        lowered.mkgrd.mesh_type == "oceanmesh" && lowered.mkgrd.mode_grid == "tri";
    for level in 1..=refine_level.min(3) {
        let rows = if smooth_ocean_tri || level == 3 { 3 } else { 4 };
        lowered.refine.halo[level] = rows;
        lowered.refine.max_transition_row[level] = rows;
    }
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
