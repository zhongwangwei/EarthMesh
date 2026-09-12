use std::env;
use std::fs;
use std::path::PathBuf;

use super::super::cli_args::{parse_nonnegative_i32, parse_positive_usize, usage};
use super::super::cli_mkgrd_output::{
    infer_restart_refine_initial_gridfile_arg, print_mask_restart_area_judge_report,
    print_mask_restart_ocean_report, print_mask_restart_patch_report, print_refine_pipeline_report,
    print_top_level_dispatch_report, write_restart_refine_namelist,
};
use super::prepare::{prepare_mkgrd_namelist, PreparedMkgrdInput, ProjectRunSpec};

pub(crate) fn run_mkgrd_or_project(
    first: String,
    mut args: impl Iterator<Item = String>,
) -> Result<(), String> {
    let prepared = prepare_mkgrd_namelist(first, &mut args)?;
    let cleanup_dir = prepared.cleanup_dir.clone();
    let project_run_dir = prepared.project_run_dir.clone();
    let result = run_prepared_mkgrd(prepared, args);
    if let Some(path) = cleanup_dir {
        if let Err(err) = fs::remove_dir_all(&path) {
            if err.kind() != std::io::ErrorKind::NotFound {
                eprintln!("earthmesh_cli: warning: cleanup {}: {err}", path.display());
            }
        }
    }
    if let Err(message) = &result {
        // A failed run used to take its whole directory with it, including the
        // quality report the failure had just pointed at -- "report=<path>"
        // followed by nothing at that path. Diagnostics are what a failure is
        // for, so the directory stays and the message says where.
        if let Some(path) = project_run_dir {
            if quality_report_paths(&path).next().is_some() {
                eprintln!(
                    "earthmesh_cli: run directory kept for diagnosis: {}",
                    path.display()
                );
            } else if let Err(err) = fs::remove_dir_all(&path) {
                if err.kind() != std::io::ErrorKind::NotFound {
                    eprintln!("earthmesh_cli: warning: cleanup {}: {err}", path.display());
                }
            }
        }
        let _ = message;
    }
    result
}

/// Quality reports under a run directory, if the run got far enough to write
/// any. Their presence is what makes a failed run worth keeping.
fn quality_report_paths(run_dir: &std::path::Path) -> impl Iterator<Item = PathBuf> + use<> {
    let mut found = Vec::new();
    let mut stack = vec![run_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().is_some_and(|name| {
                name.to_string_lossy().starts_with("quality_summary")
                    || name.to_string_lossy() == "run_manifest.json"
            }) {
                found.push(path);
            }
        }
    }
    found.into_iter()
}

fn run_prepared_mkgrd(
    prepared: PreparedMkgrdInput,
    mut args: impl Iterator<Item = String>,
) -> Result<(), String> {
    let project_workdir = prepared.project_run_dir.clone();
    let mut namelist = prepared.namelist;
    let mut project = prepared.project;
    let mut max_tris = match project.as_ref() {
        Some(spec) => project_triangle_budget(&spec.config)?,
        None => namelist_triangle_budget(&namelist),
    };
    let mut run_refine_passthrough = false;
    let mut run_refine_landtype_source = false;
    let mut run_mask_restart_ocean = false;
    let mut run_mask_restart_patch = false;
    let mut run_mask_restart_area_judge = false;
    let mut run_mask_restart_area_judge_refine = false;
    let mut run_mask_restart_area_judge_refine_landtype_source = false;
    let mut source_gridnum_perdegree: Option<usize> = None;
    let mut source_nlons: Option<usize> = None;
    let mut source_nlats: Option<usize> = None;
    let mut source_first_triangle_id: usize = 1;
    let mut restart_refine_initial_gridfile: Option<PathBuf> = None;
    let mut mask_restart_max_iter: i32 = 0;
    let mut mask_postproc_num_vertex: Option<usize> = None;
    let mut quiet = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--quiet" => {
                quiet = true;
            }
            "--max-tris" => {
                let value = args
                    .next()
                    .ok_or_else(|| usage("--max-tris requires a value"))?;
                max_tris = parse_positive_usize("--max-tris", &value)?;
            }
            "--run-refine-passthrough" => {
                run_refine_passthrough = true;
            }
            "--run-refine-landtype-source" => {
                run_refine_landtype_source = true;
            }
            "--run-mask-restart-ocean" => {
                run_mask_restart_ocean = true;
            }
            "--run-mask-restart-patch" => {
                run_mask_restart_patch = true;
            }
            "--run-mask-restart-area-judge" => {
                run_mask_restart_area_judge = true;
            }
            "--run-mask-restart-area-judge-refine" => {
                run_mask_restart_area_judge_refine = true;
            }
            "--run-mask-restart-area-judge-refine-landtype-source" => {
                run_mask_restart_area_judge_refine_landtype_source = true;
            }
            "--restart-refine-initial-gridfile" => {
                let value = args
                    .next()
                    .ok_or_else(|| usage("--restart-refine-initial-gridfile requires a value"))?;
                restart_refine_initial_gridfile = Some(PathBuf::from(value));
            }
            "--source-gridnum-perdegree" => {
                let value = args
                    .next()
                    .ok_or_else(|| usage("--source-gridnum-perdegree requires a value"))?;
                source_gridnum_perdegree =
                    Some(parse_positive_usize("--source-gridnum-perdegree", &value)?);
            }
            "--source-nlons" => {
                let value = args
                    .next()
                    .ok_or_else(|| usage("--source-nlons requires a value"))?;
                source_nlons = Some(parse_positive_usize("--source-nlons", &value)?);
            }
            "--source-nlats" => {
                let value = args
                    .next()
                    .ok_or_else(|| usage("--source-nlats requires a value"))?;
                source_nlats = Some(parse_positive_usize("--source-nlats", &value)?);
            }
            "--source-first-triangle-id" => {
                let value = args
                    .next()
                    .ok_or_else(|| usage("--source-first-triangle-id requires a value"))?;
                source_first_triangle_id =
                    parse_positive_usize("--source-first-triangle-id", &value)?;
            }
            "--mask-restart-max-iter" => {
                let value = args
                    .next()
                    .ok_or_else(|| usage("--mask-restart-max-iter requires a value"))?;
                mask_restart_max_iter = parse_nonnegative_i32("--mask-restart-max-iter", &value)?;
            }
            "--mask-postproc-num-vertex" => {
                let value = args
                    .next()
                    .ok_or_else(|| usage("--mask-postproc-num-vertex requires a value"))?;
                mask_postproc_num_vertex =
                    Some(parse_positive_usize("--mask-postproc-num-vertex", &value)?);
            }
            "-h" | "--help" => return Err(usage("")),
            other => return Err(usage(&format!("unknown argument {other}"))),
        }
    }

    let contents = fs::read_to_string(&namelist)
        .map_err(|err| format!("failed to read namelist {namelist}: {err}"))?;
    let config = earthmesh_core::EarthmeshConfig::from_mkgrd_namelist(&contents)
        .map_err(|err| format!("failed to parse namelist {namelist}: {err}"))?;
    let worker_count =
        usize::try_from(config.openmp).map_err(|_| "NL%openmp must be positive".to_string())?;
    earthmesh_mesh::configure_global_thread_pool(worker_count).map_err(|err| err.to_string())?;

    let workdir = match project_workdir {
        Some(path) => path,
        None => env::current_dir().map_err(|err| err.to_string())?,
    };
    let has_explicit_execution_mode = run_refine_passthrough
        || run_refine_landtype_source
        || run_mask_restart_ocean
        || run_mask_restart_patch
        || run_mask_restart_area_judge
        || run_mask_restart_area_judge_refine
        || run_mask_restart_area_judge_refine_landtype_source;
    if has_explicit_execution_mode
        && project.as_ref().is_some_and(|spec| {
            spec.config.quality.on_violation == earthmesh_project::ViolationPolicy::AutoRefine
        })
    {
        return Err(
            "project quality auto_refine is available only through the default --project execution path; explicit low-level execution modes cannot safely rerun the project"
                .to_string(),
        );
    }
    if has_explicit_execution_mode
        && project
            .as_ref()
            .is_some_and(|spec| spec.config.delivery.colm_mesh.is_some())
    {
        return Err(
            "project CoLM mesh delivery is available only through the default --project execution path; explicit low-level execution modes do not select the final project gridfile"
                .to_string(),
        );
    }
    if !has_explicit_execution_mode {
        let mut auto_refine_state = project
            .as_ref()
            .filter(|spec| {
                spec.config.quality.on_violation == earthmesh_project::ViolationPolicy::AutoRefine
                    && !spec.config.refinement.backend.owns_quality_repair()
            })
            .map(|spec| {
                let target_nxp = spec.config.try_lower()?.mkgrd.nxp;
                Ok::<_, String>(earthmesh_project::AutoRefineState::new(
                    spec.config.refinement.max_passes,
                    target_nxp,
                ))
            })
            .transpose()?;
        let mut pending_quality_repair: Option<(
            earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport,
            PathBuf,
        )> = None;
        let mut last_acceptable_report = None;
        let mut last_acceptable_quality = None;
        let mut report = loop {
            let (engine_result, candidate_namelist) = if let Some((report, path)) =
                pending_quality_repair.take()
            {
                (Ok(report), Some(path))
            } else {
                (
                    earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
                        PathBuf::from(&namelist),
                        &workdir,
                        max_tris,
                        mask_restart_max_iter,
                        restart_refine_initial_gridfile.as_deref(),
                        source_gridnum_perdegree,
                        source_first_triangle_id,
                        mask_postproc_num_vertex,
                    ),
                    None,
                )
            };
            let report = match engine_result {
                Ok(report) => report,
                Err(err) => {
                    let message = err.to_string();
                    if let Some(state) = auto_refine_state.as_mut() {
                        if let earthmesh_project::AutoRefineAction::AbortEngine { pass, message } =
                            state.transition(earthmesh_project::AutoRefineEvent::EngineFailed(
                                message.clone(),
                            ))
                        {
                            return Err(format!(
                                "auto_refine engine failed at pass {pass}: {message}"
                            ));
                        }
                    }
                    return Err(message);
                }
            };
            let was_quality_repair_candidate = candidate_namelist.is_some();
            let Some(spec) = project.as_mut() else {
                break report;
            };
            if spec.config.quality.on_violation != earthmesh_project::ViolationPolicy::AutoRefine {
                break report;
            }
            let Some(gridfile) = final_gridfile(&report) else {
                return Err(
                    "auto_refine requires a completed gridfile-producing project run".to_string(),
                );
            };
            let quality_namelist = candidate_namelist
                .as_deref()
                .unwrap_or_else(|| std::path::Path::new(&namelist));
            let quality = project_quality_report_with_namelist(spec, gridfile, quality_namelist)?;
            let verdict = quality.verdict;
            let current_pass = auto_refine_state
                .as_ref()
                .map(earthmesh_project::AutoRefineState::current_pass)
                .unwrap_or(spec.config.refinement.max_passes);
            eprintln!(
                "earthmesh_cli: auto_refine quality={} level={}",
                verdict.as_str(),
                current_pass
            );
            if spec.config.refinement.backend.owns_quality_repair() {
                let owner = match spec.config.refinement.backend {
                    earthmesh_project::RefinementBackend::Certified => "CMRC",
                    _ => unreachable!("only repair-owning backends enter this branch"),
                };
                let reason = match verdict {
                    earthmesh_quality::QualityLevel::Pass => "internal quality gates passed",
                    earthmesh_quality::QualityLevel::Warn => {
                        "transactional backend kept its certified mesh; Method-C local repair is not applicable"
                    }
                    earthmesh_quality::QualityLevel::Fail => {
                        return Err(format!(
                            "{owner} internal quality repair ended with verdict=fail"
                        ));
                    }
                };
                let reason = format!("{owner} {reason}");
                record_auto_refine_decision(
                    current_pass,
                    if verdict == earthmesh_quality::QualityLevel::Pass {
                        "complete"
                    } else {
                        "kept"
                    },
                    &reason,
                    None,
                    gridfile,
                    gridfile,
                    None,
                    verdict,
                    verdict,
                    &[],
                )?;
                if verdict != earthmesh_quality::QualityLevel::Pass {
                    eprintln!("earthmesh_cli: warning: {reason}; keeping the {owner} mesh");
                }
                break report;
            }
            if let Some(previous_quality) = last_acceptable_quality.take() {
                let regressions = quality.guarded_metric_regressions(&previous_quality);
                if earthmesh_cli::project_quality::select_auto_refine_candidate(
                    &previous_quality,
                    &quality,
                ) == earthmesh_cli::project_quality::AutoRefineCandidateSelection::Baseline
                {
                    let fallback = last_acceptable_report.take().ok_or_else(|| {
                        "auto_refine rollback report was not retained".to_string()
                    })?;
                    let selected_gridfile = final_gridfile(&fallback).ok_or_else(|| {
                        "auto_refine rollback gridfile was not retained".to_string()
                    })?;
                    record_auto_refine_decision(
                        current_pass,
                        "rejected",
                        "candidate did not strictly improve all guarded quality metrics",
                        Some(selected_gridfile),
                        gridfile,
                        selected_gridfile,
                        Some(previous_quality.verdict),
                        verdict,
                        previous_quality.verdict,
                        &regressions,
                    )?;
                    eprintln!(
                        "earthmesh_cli: warning: auto_refine rejected pass {current_pass} because the candidate did not strictly improve quality ({} -> {}); keeping the previous mesh",
                        previous_quality.verdict.as_str(),
                        verdict.as_str()
                    );
                    enforce_project_quality_policy(
                        spec.config.quality.on_violation,
                        previous_quality.verdict,
                    )?;
                    break fallback;
                }
                let baseline_gridfile = last_acceptable_report
                    .as_ref()
                    .and_then(final_gridfile)
                    .ok_or_else(|| "auto_refine baseline gridfile was not retained".to_string())?;
                record_auto_refine_decision(
                    current_pass,
                    "accepted",
                    "candidate strictly improved quality without guarded regressions",
                    Some(baseline_gridfile),
                    gridfile,
                    gridfile,
                    Some(previous_quality.verdict),
                    verdict,
                    verdict,
                    &regressions,
                )?;
                if let Some(path) = candidate_namelist {
                    namelist = path.to_string_lossy().into_owned();
                }
            }
            if quality.has_unrepairable_failure() {
                return Err(format!(
                    "auto_refine cannot repair final quality failure at level {current_pass}; report={}",
                    gridfile
                        .parent()
                        .unwrap_or_else(|| std::path::Path::new("."))
                        .join("quality_summary.json")
                        .display()
                ));
            }
            let event = if verdict == earthmesh_quality::QualityLevel::Pass {
                earthmesh_project::AutoRefineEvent::QualityPassed
            } else {
                earthmesh_project::AutoRefineEvent::QualityViolation
            };
            let Some(state) = auto_refine_state.as_mut() else {
                return Err("auto_refine orchestration state was not initialized".to_string());
            };
            match state.transition(event) {
                earthmesh_project::AutoRefineAction::Complete { .. } => {
                    if !was_quality_repair_candidate {
                        record_auto_refine_decision(
                            current_pass,
                            "complete",
                            "quality gates passed",
                            None,
                            gridfile,
                            gridfile,
                            None,
                            verdict,
                            verdict,
                            &[],
                        )?;
                    }
                    break report;
                }
                earthmesh_project::AutoRefineAction::Retry { next_pass } => {
                    if quality.repair_cells.is_empty() {
                        record_auto_refine_decision(
                            current_pass,
                            "kept",
                            "no locally repairable connected defect cells",
                            None,
                            gridfile,
                            gridfile,
                            None,
                            verdict,
                            verdict,
                            &[],
                        )?;
                        eprintln!(
                            "earthmesh_cli: warning: auto_refine found no locally repairable cells at pass {current_pass}; keeping the current mesh instead of applying an unscoped global refinement"
                        );
                        break report;
                    }
                    let parent_gridfile = refinement_parent_gridfile(&report).ok_or_else(|| {
                        "auto_refine local quality repair requires the unmasked Method-C parent gridfile"
                            .to_string()
                    })?;
                    let quality_dir = gridfile
                        .parent()
                        .unwrap_or_else(|| std::path::Path::new("."));
                    let repair_dir = quality_dir
                        .join("quality_auto_refine")
                        .join(format!("pass_{next_pass}"));
                    let adapter = match
                        earthmesh_cli::hydro_refinement_adapter::run_quality_refinement_adapter(
                            PathBuf::from(&namelist),
                            parent_gridfile,
                            quality_dir.join("quality_repair_cells.geojson"),
                            quality_dir.join("quality_repair_plan.json"),
                            repair_dir.join("adapter.nml"),
                            &workdir,
                            max_tris,
                            source_gridnum_perdegree,
                        )
                    {
                        Ok(adapter) => adapter,
                        Err(error) if keep_mesh_after_repair_error(verdict) => {
                            let reason = format!("local quality repair unavailable: {error}");
                            record_auto_refine_decision(
                                current_pass,
                                "kept",
                                &reason,
                                None,
                                gridfile,
                                gridfile,
                                None,
                                verdict,
                                verdict,
                                &[],
                            )?;
                            eprintln!(
                                "earthmesh_cli: warning: {reason}; keeping the current mesh"
                            );
                            break report;
                        }
                        Err(error) => {
                            return Err(format!(
                                "auto_refine local quality repair pass {next_pass} failed: {error}"
                            ));
                        }
                    };
                    eprintln!(
                        "earthmesh_cli: auto_refine applying {} local quality targets at pass {next_pass}",
                        quality.repair_cells.len()
                    );
                    last_acceptable_quality = Some(quality);
                    last_acceptable_report = Some(report);
                    pending_quality_repair = Some((
                        earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
                            adapter.pipeline,
                        ),
                        adapter.adapter_namelist,
                    ));
                }
                earthmesh_project::AutoRefineAction::CapReached { cap, .. } => {
                    if verdict == earthmesh_quality::QualityLevel::Fail {
                        return Err(format!(
                            "auto_refine reached the supported level cap {cap} with verdict=fail"
                        ));
                    }
                    eprintln!(
                        "earthmesh_cli: warning: auto_refine reached the supported level cap {cap}; keeping the last valid mesh"
                    );
                    if !was_quality_repair_candidate {
                        record_auto_refine_decision(
                            current_pass,
                            "cap_reached",
                            "supported AutoRefine level cap reached",
                            None,
                            gridfile,
                            gridfile,
                            None,
                            verdict,
                            verdict,
                            &[],
                        )?;
                    }
                    break report;
                }
                earthmesh_project::AutoRefineAction::AbortEngine { .. } => {
                    return Err(
                        "auto_refine quality transition produced an engine-failure action"
                            .to_string(),
                    );
                }
            }
        };
        let mut selected_project_gridfile =
            final_gridfile(&report).map(std::path::Path::to_path_buf);
        if let Some(spec) = project.as_ref() {
            if spec.config.hydro_execution_plan()?.is_some() {
                let gridfile = selected_project_gridfile.as_deref().ok_or_else(|| {
                    "project hydro closed loop requires a completed gridfile-producing run"
                        .to_string()
                })?;
                let refinement_parent = refinement_parent_gridfile(&report).unwrap_or(gridfile);
                let hydro_dir = earthmesh_project::project_hydro_output_dir(gridfile);
                let closed =
                    earthmesh_cli::project_hydro_closed_loop::run_project_hydro_closed_loop(
                        &spec.config,
                        &spec.path,
                        PathBuf::from(&namelist),
                        gridfile,
                        refinement_parent,
                        &hydro_dir,
                        &workdir,
                        max_tris,
                        source_gridnum_perdegree,
                    )
                    .map_err(|err| format!("project hydro closed loop: {err}"))?
                    .ok_or_else(|| {
                        "configured project hydro closed loop returned no report".to_string()
                    })?;
                eprintln!(
                    "earthmesh_cli: project hydro final gridfile={}",
                    closed.final_gridfile.display()
                );
                selected_project_gridfile = Some(closed.final_gridfile.clone());
                if spec.config.quality.on_violation == earthmesh_project::ViolationPolicy::Block
                    && closed.final_coupling_quality_verdict.as_deref() == Some("fail")
                {
                    return Err(format!(
                        "project coupling quality gate failed under block policy after hydro closed loop; report={}",
                        closed.manifest_path.display()
                    ));
                }
                if let Some(adapter) = closed.refinement {
                    namelist = adapter.adapter_namelist.to_string_lossy().into_owned();
                    report = earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(adapter.pipeline);
                }
            }
            if let Some(gridfile) = selected_project_gridfile.as_deref() {
                // The selected mesh may differ from the engine report after hydro or
                // AutoRefine. Audit it, not a rejected candidate or global parent.
                let out_dir = gridfile
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new("."))
                    .join("final_quality");
                let final_quality = earthmesh_cli::project_quality::admit_project_final_gridfile(
                    &spec.config,
                    gridfile,
                    &out_dir,
                    Some(std::path::Path::new(&namelist)),
                )?;
                println!(
                    "project_final_quality={}",
                    out_dir.join("quality_summary.json").display()
                );
                println!(
                    "project_final_quality_verdict={}",
                    final_quality.verdict.as_str()
                );
                if spec.config.target.model_format == earthmesh_project::ModelFormat::Fvcom {
                    if spec.config.target.cell == earthmesh_project::MeshCellKind::Tri {
                        let stem = gridfile.file_stem().unwrap_or_default().to_string_lossy();
                        let output = gridfile
                            .parent()
                            .unwrap_or_else(|| std::path::Path::new("."))
                            .join("standard")
                            .join(format!("FVCOM_{stem}.2dm"));
                        let fvcom = earthmesh_cli::regional_gridfile_writers::write_fvcom_from_final_gridfile(gridfile, &output)
                            .map_err(|err| format!("project FVCOM final delivery: {err}"))?;
                        let boundary_status = if final_quality.topology.boundary_edge_count == 0 {
                            "closed_mesh"
                        } else if fvcom.boundary_segments > 0 {
                            "open_chains_preserved"
                        } else {
                            eprintln!("earthmesh_cli: FVCOM has boundary edges but no classified open-boundary chains; boundary conditions and forcing are not certified by mesh export");
                            "no_open_chains_classified"
                        };
                        println!("fvcom_boundary_status={boundary_status}");
                        println!("fvcom_mesh_input={}", fvcom.output.display());
                        println!(
                            "fvcom_triangles={} fvcom_nodes={} fvcom_boundary_segments={}",
                            fvcom.triangles, fvcom.nodes, fvcom.boundary_segments
                        );
                    } else {
                        eprintln!("earthmesh_cli: FVCOM specialized export requires triangular cells; grid-only delivery");
                    }
                }
                if let Some(report) = write_project_colm_mesh_delivery(&spec.config, gridfile)? {
                    let pixels_per_degree = spec
                        .config
                        .delivery
                        .colm_mesh
                        .as_ref()
                        .map(|delivery| delivery.pixels_per_degree)
                        .unwrap_or_default();
                    println!("colm_mesh_input={}", report.output.display());
                    println!(
                        "colm_mesh_pixels_per_degree={} colm_mesh_shape={}x{} cells={} assigned_pixels={}",
                        pixels_per_degree, report.nlon, report.nlat, report.cells, report.assigned_pixels
                    );
                }
            } else {
                return Err("project final admission requires a selected gridfile".to_string());
            }
        }
        if !quiet {
            match &report {
                earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::Dispatch(report) => {
                    print_top_level_dispatch_report(report);
                }
                earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
                    run,
                ) => {
                    print_refine_pipeline_report(run);
                }
            }
        }
        return Ok(());
    }

    let refine_modes = run_refine_passthrough as u8 + run_refine_landtype_source as u8;
    if refine_modes > 1 {
        return Err(usage("refine execution flags are mutually exclusive"));
    }
    let mask_restart_modes = run_mask_restart_ocean as u8
        + run_mask_restart_patch as u8
        + run_mask_restart_area_judge as u8
        + run_mask_restart_area_judge_refine as u8
        + run_mask_restart_area_judge_refine_landtype_source as u8;
    if run_mask_restart_ocean && refine_modes > 0 {
        return Err(usage(
            "--run-mask-restart-ocean cannot be combined with refine execution flags",
        ));
    }
    if run_mask_restart_patch && refine_modes > 0 {
        return Err(usage(
            "--run-mask-restart-patch cannot be combined with refine execution flags",
        ));
    }
    if run_mask_restart_area_judge && refine_modes > 0 {
        return Err(usage(
            "--run-mask-restart-area-judge cannot be combined with refine execution flags",
        ));
    }
    if run_mask_restart_area_judge_refine && refine_modes > 0 {
        return Err(usage(
            "--run-mask-restart-area-judge-refine cannot be combined with other refine execution flags",
        ));
    }
    if run_mask_restart_area_judge_refine_landtype_source && refine_modes > 0 {
        return Err(usage(
            "--run-mask-restart-area-judge-refine-landtype-source cannot be combined with other refine execution flags",
        ));
    }
    if mask_restart_modes > 1 {
        return Err(usage("mask-restart execution flags are mutually exclusive"));
    }
    if run_mask_restart_ocean {
        let contents = fs::read_to_string(&namelist)
            .map_err(|err| format!("failed to read namelist {namelist}: {err}"))?;
        let config = earthmesh_core::EarthmeshConfig::from_mkgrd_namelist(&contents)
            .map_err(|err| format!("failed to parse namelist {namelist}: {err}"))?;
        let num_vertex = match mask_postproc_num_vertex {
            Some(value) => value,
            None => earthmesh_cli::mkgrd_default_restart_handoff::infer_mask_restart_ocean_num_vertex_from_config(&config)
                .map_err(|err| err.to_string())?,
        };
        let report = earthmesh_cli::mkgrd_mask_restart::run_mkgrd_mask_restart_ocean_namelist(
            PathBuf::from(&namelist),
            &workdir,
            mask_restart_max_iter,
            earthmesh_cli::mask_postproc_types::MaskPostprocOceanRunOptions {
                mask_sea_ratio: config.mask_sea_ratio,
                num_vertex,
            },
        )
        .map_err(|err| err.to_string())?;

        print_mask_restart_ocean_report(&report);
        return Ok(());
    }
    if run_mask_restart_patch {
        let report = earthmesh_cli::mkgrd_mask_restart::run_mkgrd_mask_restart_patch_namelist(
            PathBuf::from(&namelist),
            &workdir,
            mask_restart_max_iter,
        )
        .map_err(|err| err.to_string())?;

        print_mask_restart_patch_report(&report);
        return Ok(());
    }
    if run_mask_restart_area_judge {
        let report = match (source_gridnum_perdegree, source_nlons, source_nlats) {
            (Some(gridnum_perdegree), Some(nlons_source), Some(nlats_source)) => {
                earthmesh_cli::mkgrd_mask_restart::run_mkgrd_mask_restart_area_judge_global_source_namelist(
                    PathBuf::from(&namelist),
                    &workdir,
                    mask_restart_max_iter,
                    gridnum_perdegree,
                    nlons_source,
                    nlats_source,
                    mask_postproc_num_vertex,
                )
            }
            (None, None, None) => {
                earthmesh_cli::mkgrd_mask_restart::run_mkgrd_mask_restart_area_judge_configured_global_source_namelist(
                    PathBuf::from(&namelist),
                    &workdir,
                    mask_restart_max_iter,
                    mask_postproc_num_vertex,
                )
            }
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "--run-mask-restart-area-judge source-grid override requires all of --source-gridnum-perdegree, --source-nlons, and --source-nlats",
            )),
        }
        .map_err(|err| err.to_string())?;
        print_mask_restart_area_judge_report(&report);
        return Ok(());
    }
    if run_mask_restart_area_judge_refine {
        return Err(
            "--run-mask-restart-area-judge-refine source-state handoff was removed; use --run-mask-restart-area-judge-refine-landtype-source or Method-C-direct hfield refinement".to_string(),
        );
    }
    if run_mask_restart_area_judge_refine_landtype_source {
        let initial_gridfile = infer_restart_refine_initial_gridfile_arg(
            &namelist,
            restart_refine_initial_gridfile.as_deref(),
        )?;
        fs::metadata(&initial_gridfile).map_err(|err| {
            format!(
                "--restart-refine-initial-gridfile could not read {}: {err}",
                initial_gridfile.display()
            )
        })?;
        let refine_namelist =
            write_restart_refine_namelist(&namelist, &workdir, &initial_gridfile)?;
        let report = earthmesh_cli::run_refine_pipeline_namelist(
            &refine_namelist,
            &workdir,
            max_tris,
            source_gridnum_perdegree,
        )
        .map_err(|err| err.to_string())?;
        let _ = source_first_triangle_id;
        let _ = mask_postproc_num_vertex;
        println!("mask_restart_action=MethodCRefine");
        print_refine_pipeline_report(&report);
        return Ok(());
    }
    if run_refine_landtype_source {
        let namelist_path = PathBuf::from(&namelist);
        let report = earthmesh_cli::run_refine_pipeline_namelist(
            &namelist_path,
            &workdir,
            max_tris,
            source_gridnum_perdegree,
        )
        .map_err(|err| err.to_string())?;
        print_refine_pipeline_report(&report);
        return Ok(());
    }

    if run_refine_passthrough {
        let report = earthmesh_cli::run_refine_pipeline_namelist(
            PathBuf::from(namelist),
            &workdir,
            max_tris,
            source_gridnum_perdegree,
        )
        .map_err(|err| err.to_string())?;
        print_refine_pipeline_report(&report);
        return Ok(());
    }

    Err("internal error: explicit mkgrd execution mode was not dispatched".to_string())
}

/// The gridinit ceiling a namelist run needs, read from the resolution it asked
/// for.
///
/// `project_triangle_budget` already sizes this from NXP and treats 100,000 as
/// a floor. The namelist path used the same literal as a fixed ceiling and
/// never looked at NXP at all, so the same number played opposite roles in the
/// two paths: a global run at NXP 71 or above -- 20 * 71^2 is 100,820
/// triangles -- died in gridinit with "triangle count exceeds max_tris", while
/// the identical resolution through `--project` was given room for it. Nothing
/// about 100,000 corresponds to a real limit; `max_tris` is only ever compared
/// against, never allocated from.
///
/// The floor stays, so this can only raise the ceiling, never lower it: a run
/// that passed before still passes. A namelist that cannot be read or parsed
/// falls back to the floor rather than failing here -- the later stages parse
/// it again and report the real problem with the context to explain it.
fn namelist_triangle_budget(namelist_source: &str) -> usize {
    const FLOOR: usize = 100_000;
    let Ok(contents) = fs::read_to_string(namelist_source) else {
        return FLOOR;
    };
    let Ok(config) = earthmesh_core::EarthmeshConfig::from_mkgrd_namelist(&contents) else {
        return FLOOR;
    };
    let Ok(nxp) = usize::try_from(config.nxp) else {
        return FLOOR;
    };
    20usize
        .checked_mul(nxp)
        .and_then(|scaled| scaled.checked_mul(nxp))
        .map_or(FLOOR, |base| base.max(FLOOR))
}

fn project_triangle_budget(config: &earthmesh_project::ProjectConfig) -> Result<usize, String> {
    let target_nxp = config.try_lower()?.mkgrd.nxp;
    let nxp =
        usize::try_from(target_nxp).map_err(|_| "project NXP must be positive".to_string())?;
    let base = 20usize
        .checked_mul(nxp)
        .and_then(|value| value.checked_mul(nxp))
        .ok_or_else(|| "project base triangle count exceeds this platform".to_string())?;
    let passes = if config.quality.on_violation == earthmesh_project::ViolationPolicy::AutoRefine
        && !config.refinement.backend.owns_quality_repair()
    {
        earthmesh_project::auto_refine_level_cap(target_nxp)
    } else if config.refinement.enabled {
        config.refinement.max_passes
    } else {
        0
    };
    base.checked_shl(u32::from(passes) * 2)
        .map(|budget| budget.max(100_000))
        .ok_or_else(|| "project refined triangle budget exceeds this platform".to_string())
}

fn final_gridfile(
    report: &earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport,
) -> Option<&std::path::Path> {
    use earthmesh_cli::mkgrd_run_types::{
        MkgrdTopLevelDefaultRestartRefineRunReport as DefaultReport,
        MkgrdTopLevelDispatchRunReport as DispatchReport,
    };
    match report {
        DefaultReport::RefinePipeline(run) => Some(run.output.output.as_path()),
        DefaultReport::Dispatch(DispatchReport::Gridinit(run)) => {
            Some(run.gridfile.output.as_path())
        }
        DefaultReport::Dispatch(DispatchReport::RefinePipeline(run)) => {
            Some(run.output.output.as_path())
        }
        _ => None,
    }
}

fn refinement_parent_gridfile(
    report: &earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport,
) -> Option<&std::path::Path> {
    use earthmesh_cli::mkgrd_run_types::{
        MkgrdTopLevelDefaultRestartRefineRunReport as DefaultReport,
        MkgrdTopLevelDispatchRunReport as DispatchReport,
    };
    match report {
        DefaultReport::RefinePipeline(run)
        | DefaultReport::Dispatch(DispatchReport::RefinePipeline(run)) => {
            Some(run.refinement_parent_gridfile())
        }
        DefaultReport::Dispatch(DispatchReport::Gridinit(run)) => {
            Some(run.gridfile.output.as_path())
        }
        _ => None,
    }
}

fn project_quality_report_with_namelist(
    spec: &ProjectRunSpec,
    gridfile: &std::path::Path,
    quality_namelist: &std::path::Path,
) -> Result<earthmesh_quality::MeshQualityReport, String> {
    let out_dir = gridfile
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    earthmesh_cli::project_quality::write_project_quality_report_with_namelist(
        &spec.config,
        gridfile,
        out_dir,
        Some(quality_namelist),
    )
}

fn keep_mesh_after_repair_error(verdict: earthmesh_quality::QualityLevel) -> bool {
    verdict == earthmesh_quality::QualityLevel::Warn
}

#[allow(clippy::too_many_arguments)]
fn record_auto_refine_decision(
    pass: u8,
    decision: &str,
    reason: &str,
    baseline_gridfile: Option<&std::path::Path>,
    candidate_gridfile: &std::path::Path,
    selected_gridfile: &std::path::Path,
    baseline_verdict: Option<earthmesh_quality::QualityLevel>,
    candidate_verdict: earthmesh_quality::QualityLevel,
    selected_verdict: earthmesh_quality::QualityLevel,
    regressions: &[earthmesh_quality::QualityMetricRegression],
) -> Result<(), String> {
    let out_dir = candidate_gridfile
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let quality_report_for = |gridfile: &std::path::Path| {
        gridfile
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join("quality_summary.json")
    };
    let baseline_quality_report = baseline_gridfile.map(quality_report_for);
    let candidate_quality_report = quality_report_for(candidate_gridfile);
    let selected_quality_report = quality_report_for(selected_gridfile);
    earthmesh_cli::project_quality::write_auto_refine_decision(
        out_dir,
        &earthmesh_cli::project_quality::AutoRefineDecision {
            pass,
            decision,
            reason,
            regressions,
            baseline_gridfile,
            candidate_gridfile,
            selected_gridfile,
            baseline_quality_report: baseline_quality_report.as_deref(),
            candidate_quality_report: &candidate_quality_report,
            selected_quality_report: &selected_quality_report,
            baseline_verdict,
            candidate_verdict,
            selected_verdict,
        },
    )?;
    Ok(())
}

fn project_colm_mesh_kind(
    config: &earthmesh_project::ProjectConfig,
) -> Result<Option<earthmesh_cli::unstructured_mesh_support::GridfileCellKind>, String> {
    let Some(_colm_mesh) = &config.delivery.colm_mesh else {
        return Ok(None);
    };
    if config.target.model_format != earthmesh_project::ModelFormat::CoLM {
        return Err("delivery colm_mesh requires target.model_format=CoLM".to_string());
    }
    Ok(Some(match config.target.cell {
        earthmesh_project::MeshCellKind::Tri => {
            earthmesh_cli::unstructured_mesh_support::GridfileCellKind::Tri
        }
        earthmesh_project::MeshCellKind::Hex => {
            earthmesh_cli::unstructured_mesh_support::GridfileCellKind::Hex
        }
    }))
}

fn project_colm_mesh_output_path(gridfile: &std::path::Path) -> std::path::PathBuf {
    let directory = gridfile
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let stem = gridfile
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("gridfile");
    directory
        .join("standard")
        .join(format!("CoLM_{stem}_mesh.nc"))
}

fn write_project_colm_mesh_delivery(
    config: &earthmesh_project::ProjectConfig,
    gridfile: &std::path::Path,
) -> Result<Option<earthmesh_cli::colm_mesh_input::ColmMeshInputReport>, String> {
    let Some(kind) = project_colm_mesh_kind(config)? else {
        return Ok(None);
    };
    let colm_mesh = config
        .delivery
        .colm_mesh
        .as_ref()
        .expect("kind is present only when colm delivery is configured");
    if colm_mesh.pixels_per_degree == 0 {
        return Err("delivery colm_mesh pixels_per_degree must be positive".to_string());
    }
    let output = project_colm_mesh_output_path(gridfile);
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "create CoLM mesh delivery directory {}: {err}",
                parent.display()
            )
        })?;
    }
    earthmesh_cli::colm_mesh_input::write_colm_mesh_from_gridfile_with_kind(
        gridfile,
        &output,
        colm_mesh.pixels_per_degree,
        kind,
    )
    .map(Some)
    .map_err(|err| {
        format!(
            "write CoLM mesh input from selected project gridfile {} at {} px/degree: {err}",
            gridfile.display(),
            colm_mesh.pixels_per_degree
        )
    })
}

pub(crate) use earthmesh_cli::project_quality::enforce_project_quality_policy;

#[cfg(test)]
mod tests {
    use super::*;
    use earthmesh_project::{
        ColmMeshDeliveryConfig, DomainConfig, MeshCellKind, MeshIntentPreset, ModelFormat,
        ProjectConfig, RefinementBackend, ResolutionSpec, ViolationPolicy,
    };
    use earthmesh_quality::QualityLevel;
    use std::path::Path;
    use std::sync::Mutex;

    static NETCDF_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn write_tiny_hex_gridfile(path: &Path) {
        let mut file = earthmesh_cli::create_netcdf_quiet(path).unwrap();
        for (name, length) in [
            ("sjx_points", 4),
            ("lbx_points", 1),
            ("dimb", 3),
            ("dimc", 4),
        ] {
            file.add_dimension(name, length).unwrap();
        }
        file.add_variable::<f64>("GLONM", &["sjx_points"])
            .unwrap()
            .put_values(&[100.0, 104.0, 104.0, 100.0], ..)
            .unwrap();
        file.add_variable::<f64>("GLATM", &["sjx_points"])
            .unwrap()
            .put_values(&[20.0, 20.0, 22.0, 22.0], ..)
            .unwrap();
        file.add_variable::<f64>("GLONW", &["lbx_points"])
            .unwrap()
            .put_values(&[102.0], ..)
            .unwrap();
        file.add_variable::<f64>("GLATW", &["lbx_points"])
            .unwrap()
            .put_values(&[21.0], ..)
            .unwrap();
        file.add_variable::<i32>("itab_m%iw", &["sjx_points", "dimb"])
            .unwrap()
            .put_values(&[1; 12], ..)
            .unwrap();
        file.add_variable::<i32>("itab_w%im", &["lbx_points", "dimc"])
            .unwrap()
            .put_values(&[1, 2, 3, 4], ..)
            .unwrap();
        file.add_variable::<i32>("n_ngrwm", &["lbx_points"])
            .unwrap()
            .put_values(&[4], ..)
            .unwrap();
        file.close().unwrap();
    }

    #[test]
    fn project_block_policy_rejects_failed_quality() {
        let err =
            enforce_project_quality_policy(ViolationPolicy::Block, QualityLevel::Fail).unwrap_err();
        assert!(err.contains("quality gate failed"));
    }

    #[test]
    fn auto_refine_keeps_warn_but_not_fail_when_repair_is_unavailable() {
        assert!(keep_mesh_after_repair_error(QualityLevel::Warn));
        assert!(!keep_mesh_after_repair_error(QualityLevel::Fail));
    }

    #[test]
    fn certified_backend_is_not_a_method_c_quality_repair_candidate() {
        assert!(RefinementBackend::Certified.owns_quality_repair());
        assert!(!RefinementBackend::MethodC.owns_quality_repair());
        assert!(!RefinementBackend::RedGreen.owns_quality_repair());
    }

    #[test]
    fn project_warn_policy_keeps_failed_mesh() {
        enforce_project_quality_policy(ViolationPolicy::Warn, QualityLevel::Fail)
            .expect("warn policy must not block");
    }

    #[test]
    fn project_auto_refine_policy_rejects_failed_selected_mesh() {
        let err = enforce_project_quality_policy(ViolationPolicy::AutoRefine, QualityLevel::Fail)
            .unwrap_err();
        assert!(err.contains("on_violation=auto_refine"), "{err}");
    }

    #[test]
    fn project_colm_mesh_delivery_is_opt_in_and_uses_target_cell_kind() {
        let mut project = ProjectConfig::scaffold(
            "colm_kind",
            MeshIntentPreset::Custom,
            DomainConfig::Global,
            ResolutionSpec::Nxp(40),
        );
        assert_eq!(project_colm_mesh_kind(&project).unwrap(), None);

        project.delivery.colm_mesh = Some(ColmMeshDeliveryConfig {
            pixels_per_degree: 240,
        });
        project.quality.on_violation = ViolationPolicy::Warn;
        project.target.cell = MeshCellKind::Hex;
        assert_eq!(
            project_colm_mesh_kind(&project).unwrap(),
            Some(earthmesh_cli::unstructured_mesh_support::GridfileCellKind::Hex)
        );

        project.target.cell = MeshCellKind::Tri;
        assert_eq!(
            project_colm_mesh_kind(&project).unwrap(),
            Some(earthmesh_cli::unstructured_mesh_support::GridfileCellKind::Tri)
        );
    }

    #[test]
    fn project_colm_mesh_delivery_rejects_non_colm_and_names_standard_output() {
        let mut project = ProjectConfig::scaffold(
            "colm_kind",
            MeshIntentPreset::Custom,
            DomainConfig::Global,
            ResolutionSpec::Nxp(40),
        );
        project.delivery.colm_mesh = Some(ColmMeshDeliveryConfig {
            pixels_per_degree: 240,
        });
        project.target.model_format = ModelFormat::Fvcom;
        assert!(project_colm_mesh_kind(&project)
            .unwrap_err()
            .contains("target.model_format=CoLM"));

        let path = project_colm_mesh_output_path(std::path::Path::new(
            "/tmp/run/gridfile_NXP0040_hex_landmesh.nc4",
        ));
        assert_eq!(
            path,
            std::path::Path::new("/tmp/run/standard/CoLM_gridfile_NXP0040_hex_landmesh_mesh.nc")
        );
    }

    #[test]
    fn project_colm_mesh_delivery_writes_tiny_final_gridfile_next_to_selected_path() {
        let _guard = NETCDF_TEST_LOCK.lock().expect("netcdf lock");
        let root = std::env::temp_dir().join(format!(
            "earthmesh_cli_colm_delivery_write_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let gridfile = root.join("final_selected.nc4");
        write_tiny_hex_gridfile(&gridfile);
        let mut project = ProjectConfig::scaffold(
            "colm_write",
            MeshIntentPreset::Custom,
            DomainConfig::Global,
            ResolutionSpec::Nxp(40),
        );
        project.delivery.colm_mesh = Some(ColmMeshDeliveryConfig {
            pixels_per_degree: 1,
        });
        project.quality.on_violation = ViolationPolicy::Warn;
        let report = write_project_colm_mesh_delivery(&project, &gridfile)
            .expect("delivery write")
            .expect("delivery enabled");
        let expected = root.join("standard/CoLM_final_selected_mesh.nc");
        assert_eq!(report.output, expected);
        assert!(expected.is_file());
        assert_eq!(report.cells, 1);
        assert!(report.assigned_pixels > 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn project_colm_mesh_delivery_rejects_explicit_low_level_modes() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_cli_colm_delivery_low_level_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let mut project = ProjectConfig::scaffold(
            "colm_low_level",
            MeshIntentPreset::Custom,
            DomainConfig::Global,
            ResolutionSpec::Nxp(40),
        );
        project.delivery.colm_mesh = Some(ColmMeshDeliveryConfig {
            pixels_per_degree: 240,
        });
        project.quality.on_violation = ViolationPolicy::Warn;
        let path = root.join("project.yaml");
        fs::write(&path, project.to_yaml().unwrap()).unwrap();

        let err = run_mkgrd_or_project(
            "--project".into(),
            vec![
                path.to_string_lossy().into_owned(),
                "--run-refine-passthrough".into(),
            ]
            .into_iter(),
        )
        .unwrap_err();
        assert!(err.contains("CoLM mesh delivery"), "{err}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn project_triangle_budget_covers_requested_resolution_and_refinement() {
        let mut project = ProjectConfig::scaffold(
            "budget",
            MeshIntentPreset::AtmosphereMpas,
            DomainConfig::Global,
            ResolutionSpec::Nxp(801),
        );
        project.refinement.enabled = true;
        project.refinement.max_passes = 3;
        project.refinement.specified_circle =
            Some(earthmesh_project::SpecifiedCircleRefinements::One(
                earthmesh_project::SpecifiedCircleRefinement {
                    lon: 0.0,
                    lat: 0.0,
                    radius_km: 100.0,
                },
            ));

        project.quality.on_violation = ViolationPolicy::Warn;
        assert_eq!(project_triangle_budget(&project).unwrap(), 821_249_280);

        project.quality.on_violation = ViolationPolicy::AutoRefine;
        assert_eq!(
            project_triangle_budget(&project).unwrap(),
            20 * 801 * 801 * 4usize.pow(5)
        );
    }

    #[test]
    fn failed_project_command_removes_staging_directory() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_cli_project_command_cleanup_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let project = ProjectConfig::scaffold(
            "cleanup",
            MeshIntentPreset::AtmosphereMpas,
            DomainConfig::Global,
            ResolutionSpec::Nxp(40),
        );
        let path = root.join("project.yaml");
        fs::write(&path, project.to_yaml().unwrap()).unwrap();

        let error = run_mkgrd_or_project(
            "--project".into(),
            vec![path.to_string_lossy().into_owned(), "--unknown".into()].into_iter(),
        )
        .unwrap_err();

        assert!(error.contains("unknown argument"));
        assert!(fs::read_dir(&root).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains("earthmesh-run")
        }));
        let _ = fs::remove_dir_all(root);
    }
}
