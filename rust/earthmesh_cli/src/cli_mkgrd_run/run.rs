use std::env;
use std::fs;
use std::path::PathBuf;

use super::super::cli_args::{parse_nonnegative_i32, parse_positive_usize, usage};
use super::super::cli_mkgrd_output::{
    infer_restart_refine_initial_gridfile_arg, print_mask_restart_area_judge_report,
    print_mask_restart_ocean_report, print_mask_restart_patch_report, print_refine_pipeline_report,
    print_top_level_dispatch_report, write_restart_refine_namelist,
};
use super::prepare::{prepare_mkgrd_namelist, ProjectRunSpec};

pub(crate) fn run_mkgrd_or_project(
    first: String,
    mut args: impl Iterator<Item = String>,
) -> Result<(), String> {
    let prepared = prepare_mkgrd_namelist(first, &mut args)?;
    let mut namelist = prepared.namelist;
    let mut project = prepared.project;
    let mut max_tris = 100_000usize;
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

    let workdir = env::current_dir().map_err(|err| err.to_string())?;
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
    if !has_explicit_execution_mode {
        let mut auto_refine_state = project
            .as_ref()
            .filter(|spec| {
                spec.config.quality.on_violation == earthmesh_project::ViolationPolicy::AutoRefine
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
            let quality = project_quality_report(spec, gridfile)?;
            let verdict = quality.verdict;
            let current_pass = auto_refine_state
                .as_ref()
                .map(earthmesh_project::AutoRefineState::current_pass)
                .ok_or_else(|| "auto_refine orchestration state was not initialized".to_string())?;
            eprintln!(
                "earthmesh_cli: auto_refine quality={} level={}",
                verdict.as_str(),
                current_pass
            );
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
                        "earthmesh_cli: warning: auto_refine rejected pass {current_pass} because the candidate did not strictly improve quality ({} -> {}); keeping the previous valid mesh",
                        previous_quality.verdict.as_str(),
                        verdict.as_str()
                    );
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
                    let adapter =
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
                        .map_err(|error| {
                            format!(
                                "auto_refine local quality repair pass {next_pass} failed: {error}"
                            )
                        })?;
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
        if let Some(spec) = project.as_ref() {
            if spec.config.hydro_execution_plan()?.is_some() {
                let gridfile = final_gridfile(&report).ok_or_else(|| {
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
                if spec.config.quality.on_violation == earthmesh_project::ViolationPolicy::Block
                    && closed.final_coupling_quality_verdict.as_deref() == Some("fail")
                {
                    return Err(format!(
                        "project coupling quality gate failed under block policy after hydro closed loop; report={}",
                        closed.manifest_path.display()
                    ));
                }
                if let Some(adapter) = closed.refinement {
                    report = earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(adapter.pipeline);
                }
            }
            if spec.config.quality.on_violation == earthmesh_project::ViolationPolicy::Block {
                let gridfile = final_gridfile(&report).ok_or_else(|| {
                    "project quality block policy requires a completed gridfile-producing run"
                        .to_string()
                })?;
                let verdict = project_quality_report(spec, gridfile)?.verdict;
                enforce_project_quality_policy(spec.config.quality.on_violation, verdict)?;
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
        _ => None,
    }
}

fn project_quality_report(
    spec: &ProjectRunSpec,
    gridfile: &std::path::Path,
) -> Result<earthmesh_quality::MeshQualityReport, String> {
    let out_dir = gridfile
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    earthmesh_cli::project_quality::write_project_quality_report(&spec.config, gridfile, out_dir)
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

fn enforce_project_quality_policy(
    policy: earthmesh_project::ViolationPolicy,
    verdict: earthmesh_quality::QualityLevel,
) -> Result<(), String> {
    if policy == earthmesh_project::ViolationPolicy::Block
        && verdict == earthmesh_quality::QualityLevel::Fail
    {
        return Err("project quality gate failed (verdict=fail, on_violation=block)".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use earthmesh_project::ViolationPolicy;
    use earthmesh_quality::QualityLevel;

    #[test]
    fn project_block_policy_rejects_failed_quality() {
        let err =
            enforce_project_quality_policy(ViolationPolicy::Block, QualityLevel::Fail).unwrap_err();
        assert!(err.contains("quality gate failed"));
    }

    #[test]
    fn project_warn_policy_keeps_failed_mesh() {
        enforce_project_quality_policy(ViolationPolicy::Warn, QualityLevel::Fail)
            .expect("warn policy must not block");
    }
}
