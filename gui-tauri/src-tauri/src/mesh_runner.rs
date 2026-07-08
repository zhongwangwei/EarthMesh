//! Mesh execution and quality extraction command handlers.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs};

use earthmesh_project::{
    nxp_to_km, CloseMaskFormat, DomainConfig, LoweredProject, MeshCellKind, MeshDomainKind,
    ProjectConfig, ProjectLayerRole, RegionShape,
};
use serde_json::json;
use tauri::{AppHandle, Emitter};

use crate::dto::RunResult;
use crate::engine::{resolve_mkgrd, stage_threshold_layers};
use crate::mesh_paths::existing_file_path;
use crate::mesh_process::{clear_running_child, record_running_child};

const SHAPEFILE_MASK_SIMPLIFY_TOLERANCE_DEG: f64 = 0.002;
const METHOD_C_MIN_BASE_NXP: i32 = 10;
const METHOD_C_MAX_REFINEMENT_LEVEL: usize = 5;

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
    if cfg.expert.beta.is_none() && lowered.mkgrd.beta == 1.2 {
        lowered.mkgrd.beta = 1.0;
    }
    if cfg.expert.relax.is_none() && lowered.mkgrd.relax == 0.04 {
        lowered.mkgrd.relax = 0.035;
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

    normalize_engine_input_paths(&mut lowered, &run_dir);

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
        shape: RegionShape::Close { path, format },
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
    if let Err(err) = record_running_child(child.id()) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(err);
    }

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

    let wait_result = child.wait();
    clear_running_child();
    let _ = t1.join();
    let _ = t2.join();
    let status = wait_result.map_err(|e| format!("wait failed: {e}"))?;
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

fn read_close_mask_nml_points(path: &Path) -> std::io::Result<Vec<(f64, f64)>> {
    let text = fs::read_to_string(path)?;
    let mut lines = text.lines();
    let count = lines
        .next()
        .and_then(|line| line.split_once('='))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "missing close_num"))?;
    let _ = lines.next();
    let points = read_lonlat_rows(lines.take(count))?;
    if points.len() != count {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("close_num says {count}, found {}", points.len()),
        ));
    }
    Ok(points)
}

fn read_lonlat_text_points(path: &Path) -> std::io::Result<Vec<(f64, f64)>> {
    let text = fs::read_to_string(path)?;
    read_lonlat_rows(text.lines())
}

fn read_lonlat_rows<'a>(lines: impl Iterator<Item = &'a str>) -> std::io::Result<Vec<(f64, f64)>> {
    let mut points = Vec::new();
    for (index, line) in lines.enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts = line
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();
        if parts.len() < 2 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("lon/lat row {} needs two numbers", index + 1),
            ));
        }
        let lon = parts[0].parse::<f64>().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, format!("invalid lon: {e}"))
        })?;
        let lat = parts[1].parse::<f64>().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, format!("invalid lat: {e}"))
        })?;
        points.push((lon, lat));
    }
    if points.len() < 3 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "close lon/lat text needs at least three points",
        ));
    }
    Ok(points)
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

fn write_close_mask_nml(
    path: &Path,
    ring: &[(f64, f64)],
    refine_degree: usize,
) -> std::io::Result<()> {
    let mut body = format!(
        "close_num = {}\nclose_refine = {refine_degree}\n",
        ring.len()
    );
    for &(lon, lat) in ring {
        body.push_str(&format!("{lon:.10} {lat:.10}\n"));
    }
    fs::write(path, body)
}

fn write_specified_refinement_mask(
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
        for (index, ring) in read_shapefile_polygon_rings(Path::new(&close.path))?
            .into_iter()
            .map(|ring| simplify_shapefile_mask_ring(&ring))
            .enumerate()
        {
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
    let mut cap = 1;
    while cap < METHOD_C_MAX_REFINEMENT_LEVEL
        && METHOD_C_MIN_BASE_NXP * (1_i32 << (cap + 1)) <= target_nxp
    {
        cap += 1;
    }
    cap
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
            if !same_point(ring.first(), ring.last()) {
                if let Some(first) = ring.first().copied() {
                    ring.push(first);
                }
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

pub(crate) fn read_shapefile_polygon_rings(path: &Path) -> std::io::Result<Vec<Vec<(f64, f64)>>> {
    let bytes = fs::read(path)?;
    if bytes.len() < 100 || be_i32(&bytes, 0)? != 9994 || le_i32(&bytes, 28)? != 1000 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "not an ESRI shapefile",
        ));
    }
    let mut offset = 100;
    let mut out = Vec::new();
    while offset < bytes.len() {
        if offset + 8 > bytes.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "truncated shapefile record header",
            ));
        }
        let content_len = be_usize(&bytes, offset + 4)?
            .checked_mul(2)
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "record length overflow")
            })?;
        let start = offset + 8;
        let end = start.checked_add(content_len).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "record end overflow")
        })?;
        if end > bytes.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "truncated shapefile record content",
            ));
        }
        read_polygon_record(&bytes[start..end], &mut out)?;
        offset = end;
    }
    if out.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "shapefile contains no polygon rings",
        ));
    }
    Ok(out)
}

fn read_polygon_record(content: &[u8], out: &mut Vec<Vec<(f64, f64)>>) -> std::io::Result<()> {
    if content.len() < 4 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "truncated shapefile record",
        ));
    }
    let shape_type = le_i32(content, 0)?;
    if shape_type == 0 {
        return Ok(());
    }
    if !matches!(shape_type, 5 | 15 | 25) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("unsupported shapefile shape type {shape_type}; expected Polygon"),
        ));
    }
    if content.len() < 44 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "truncated polygon record",
        ));
    }
    let num_parts = le_usize(content, 36)?;
    let num_points = le_usize(content, 40)?;
    if num_parts == 0 || num_points == 0 || num_parts > num_points {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid polygon parts/points count",
        ));
    }
    let parts_start = 44_usize;
    let points_start = parts_start
        .checked_add(num_parts.checked_mul(4).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "parts length overflow")
        })?)
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "parts offset overflow")
        })?;
    let points_end = points_start
        .checked_add(num_points.checked_mul(16).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "points length overflow")
        })?)
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "points offset overflow")
        })?;
    if points_end > content.len() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid polygon parts/points length",
        ));
    }
    let mut starts = Vec::with_capacity(num_parts + 1);
    for i in 0..num_parts {
        starts.push(le_usize(content, parts_start + i * 4)?);
    }
    starts.push(num_points);

    let mut rings = Vec::new();
    for pair in starts.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        if from >= to || to > num_points {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid polygon part index",
            ));
        }
        let mut ring = Vec::with_capacity(to - from);
        for point in from..to {
            let p = points_start + point * 16;
            ring.push((le_f64(content, p)?, le_f64(content, p + 8)?));
        }
        if same_point(ring.first(), ring.last()) {
            ring.pop();
        }
        if ring.len() >= 3 && ring.iter().all(|(x, y)| x.is_finite() && y.is_finite()) {
            rings.push(ring);
        }
    }
    let Some(outer_sign) = rings
        .iter()
        .map(|ring| signed_area(ring))
        .max_by(|a, b| a.abs().total_cmp(&b.abs()))
        .map(f64::signum)
    else {
        return Ok(());
    };
    out.extend(
        rings
            .into_iter()
            .filter(|ring| signed_area(ring).signum() == outer_sign),
    );
    Ok(())
}

fn be_i32(bytes: &[u8], offset: usize) -> std::io::Result<i32> {
    let chunk = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "truncated i32"))?;
    Ok(i32::from_be_bytes(chunk.try_into().unwrap()))
}

fn be_usize(bytes: &[u8], offset: usize) -> std::io::Result<usize> {
    let value = be_i32(bytes, offset)?;
    if value < 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "negative shapefile integer",
        ));
    }
    Ok(value as usize)
}

fn le_i32(bytes: &[u8], offset: usize) -> std::io::Result<i32> {
    let chunk = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "truncated i32"))?;
    Ok(i32::from_le_bytes(chunk.try_into().unwrap()))
}

fn le_usize(bytes: &[u8], offset: usize) -> std::io::Result<usize> {
    let value = le_i32(bytes, offset)?;
    if value < 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "negative shapefile integer",
        ));
    }
    Ok(value as usize)
}

fn le_f64(bytes: &[u8], offset: usize) -> std::io::Result<f64> {
    let chunk = bytes
        .get(offset..offset + 8)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "truncated f64"))?;
    Ok(f64::from_le_bytes(chunk.try_into().unwrap()))
}

fn same_point(a: Option<&(f64, f64)>, b: Option<&(f64, f64)>) -> bool {
    matches!((a, b), (Some(a), Some(b)) if (a.0 - b.0).abs() < 1e-12 && (a.1 - b.1).abs() < 1e-12)
}

fn signed_area(ring: &[(f64, f64)]) -> f64 {
    ring.iter()
        .zip(ring.iter().cycle().skip(1))
        .map(|(a, b)| a.0 * b.1 - b.0 * a.1)
        .sum::<f64>()
        * 0.5
}
