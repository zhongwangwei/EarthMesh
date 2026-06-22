use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let started = now_epoch_secs();
    let command = env::args().collect::<Vec<_>>().join(" ");
    // Skip pure help / no-arg invocations; every real run records a manifest.
    let is_help = matches!(
        env::args().nth(1).as_deref(),
        None | Some("-h") | Some("--help")
    );
    let result = run();
    if !is_help {
        write_cli_run_manifest(&command, started, &result);
    }
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("earthmesh_cli: {err}");
            ExitCode::from(2)
        }
    }
}

/// Seconds since the Unix epoch as a string (no chrono dependency).
fn now_epoch_secs() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default()
}

/// Write a minimal reproducible `run_manifest.json` to the current directory.
/// Records command / cwd / status / timestamps / version / optional git sha.
/// Non-fatal: a write failure only warns.
fn write_cli_run_manifest(command: &str, started_at: String, result: &Result<(), String>) {
    use earthmesh_core::run_manifest::{RunManifest, RunStatus};
    let cwd = env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let mut manifest = RunManifest::new("", command, &cwd);
    manifest.started_at = Some(started_at);
    manifest.completed_at = Some(now_epoch_secs());
    manifest.git_sha = option_env!("EARTHMESH_GIT_SHA").map(|s| s.to_string());
    match result {
        Ok(()) => manifest.status = RunStatus::Completed,
        Err(err) => {
            manifest.status = RunStatus::Failed;
            manifest.add_warning(err);
        }
    }
    let out = Path::new(&cwd).join("run_manifest.json");
    if let Err(err) = manifest.write_json(&out) {
        eprintln!(
            "earthmesh_cli: warning: could not write {}: {err}",
            out.display()
        );
    }
}

/// `--mesh-quality <gridfile.nc4> [out_dir]`: read a gridfile, build the quality
/// input from its triangle (M->W) view, and write quality_summary.json/.csv,
/// worst_cells.geojson and quality_report.md.
fn run_mesh_quality(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let gridfile = PathBuf::from(
        args.next()
            .ok_or_else(|| usage("--mesh-quality needs a gridfile path"))?,
    );
    let out_dir = args.next().map(PathBuf::from).unwrap_or_else(|| {
        gridfile
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    });

    let mesh = earthmesh_cli::read_gridfile_mesh_points(&gridfile)
        .map_err(|e| format!("read gridfile {}: {e}", gridfile.display()))?;
    let input = earthmesh_cli::quality_input_from_gridfile(&mesh);
    let report =
        earthmesh_quality::compute(&input, &earthmesh_quality::QualityThresholds::default());
    let written = earthmesh_quality::io::write_all(&report, &out_dir)
        .map_err(|e| format!("write quality report to {}: {e}", out_dir.display()))?;
    println!("mesh_quality_verdict={}", report.verdict.as_str());
    println!("mesh_quality_cells={}", report.geometry.cell_count);
    println!(
        "mesh_quality_min_angle_deg={}",
        report.geometry.min_angle_deg
    );
    for path in &written {
        println!("mesh_quality_output={}", path.display());
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let first = args
        .next()
        .ok_or_else(|| usage("missing command or mkgrd namelist path"))?;
    if first == "-h" || first == "--help" {
        println!("{}", usage(""));
        return Ok(());
    }
    if first == "--cama-reach-jsonl" || first == "--cama-reach-geojson" {
        return run_cama_reach_export(&first, args);
    }
    if first == "--merit-hydro-geojson" {
        return run_merit_hydro_geojson(args);
    }
    if first == "--hydro-close-recipe" {
        return run_hydro_close_recipe(args);
    }
    if first == "--hydro-close-mask-nmls" {
        return run_hydro_close_mask_nmls(args);
    }
    if first == "--hydro-composite-close-mask-nmls" {
        return run_hydro_composite_close_mask_nmls(args);
    }
    if first == "--colm-coupling-csv-to-netcdf" {
        return run_colm_coupling_csv_to_netcdf(args);
    }
    if first == "--colm-coupling-from-intersections" {
        return run_colm_coupling_from_intersections(args);
    }
    if first == "--hydro-mesh-qa" {
        return run_hydro_mesh_qa(args);
    }
    if first == "--hydro-refinement-eval" {
        return run_hydro_refinement_eval(args);
    }
    if first == "--hydro-sweep-recipes" {
        return run_hydro_sweep_recipes(args);
    }
    if first == "--hydro-sweep-rank" {
        return run_hydro_sweep_rank(args);
    }
    if first == "--hydro-delivery-manifest" {
        return run_hydro_delivery_manifest(args);
    }
    if first == "--hydro-cell-intersections" {
        return run_hydro_cell_intersections(args);
    }
    if first == "--mesh-quality" {
        return run_mesh_quality(args);
    }
    let namelist = first;
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
    let mut source_state_path: Option<PathBuf> = None;
    let mut restart_refine_source_state_path: Option<PathBuf> = None;
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
            "--run-refine-source-state" => {
                let value = args
                    .next()
                    .ok_or_else(|| usage("--run-refine-source-state requires a value"))?;
                source_state_path = Some(PathBuf::from(value));
            }
            "--restart-refine-source-state" => {
                let value = args
                    .next()
                    .ok_or_else(|| usage("--restart-refine-source-state requires a value"))?;
                restart_refine_source_state_path = Some(PathBuf::from(value));
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
        || source_state_path.is_some()
        || run_mask_restart_ocean
        || run_mask_restart_patch
        || run_mask_restart_area_judge
        || run_mask_restart_area_judge_refine
        || run_mask_restart_area_judge_refine_landtype_source;
    if !has_explicit_execution_mode {
        let report =
            earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
                PathBuf::from(&namelist),
                &workdir,
                max_tris,
                mask_restart_max_iter,
                restart_refine_source_state_path.as_deref(),
                restart_refine_initial_gridfile.as_deref(),
                source_gridnum_perdegree,
                source_first_triangle_id,
                mask_postproc_num_vertex,
            )
            .map_err(|err| err.to_string())?;
        if !quiet {
            match report {
                earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::Dispatch(report) => {
                    print_top_level_dispatch_report(&report);
                }
                earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(
                    run,
                ) => {
                    print_olam_refine_report(&run);
                }
            }
        }
        return Ok(());
    }

    let refine_modes = run_refine_passthrough as u8
        + source_state_path.is_some() as u8
        + run_refine_landtype_source as u8;
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
            None => earthmesh_cli::infer_mask_restart_ocean_num_vertex_from_config(&config)
                .map_err(|err| err.to_string())?,
        };
        let report = earthmesh_cli::run_mkgrd_mask_restart_ocean_namelist(
            PathBuf::from(&namelist),
            &workdir,
            mask_restart_max_iter,
            earthmesh_cli::MaskPostprocOceanRunOptions {
                mask_sea_ratio: config.mask_sea_ratio,
                num_vertex,
            },
        )
        .map_err(|err| err.to_string())?;

        println!("mask_restart_action={:?}", report.plan.remask.action);
        println!(
            "mask_postproc_result_gridfile={}",
            report.postproc.final_gridfile.output.display()
        );
        if let Some(obc) = &report.postproc.obc {
            println!("mask_postproc_obc={}", obc.output.display());
        }
        if let Some(obcv2) = &report.postproc.obcv2 {
            println!("mask_postproc_obcv2={}", obcv2.output.display());
        }
        return Ok(());
    }
    if run_mask_restart_patch {
        let report = earthmesh_cli::run_mkgrd_mask_restart_patch_namelist(
            PathBuf::from(&namelist),
            &workdir,
            mask_restart_max_iter,
        )
        .map_err(|err| err.to_string())?;

        println!("mask_restart_action={:?}", report.plan.remask.action);
        println!(
            "mask_patch_reports={}",
            report.workspace_mask.mask_reports.len()
        );
        println!(
            "mask_patch_ndm={}",
            report.workspace_mask.mask_counts.mask_patch_ndm[0]
        );
        return Ok(());
    }
    if run_mask_restart_area_judge {
        let report = match (source_gridnum_perdegree, source_nlons, source_nlats) {
            (Some(gridnum_perdegree), Some(nlons_source), Some(nlats_source)) => {
                earthmesh_cli::run_mkgrd_mask_restart_area_judge_global_source_namelist(
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
                earthmesh_cli::run_mkgrd_mask_restart_area_judge_configured_global_source_namelist(
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
        let restart = &report.restart;

        println!("mask_restart_action={:?}", restart.plan.remask.action);
        println!(
            "mask_patch_reports={}",
            restart.workspace_mask.mask_reports.len()
        );
        println!(
            "mask_restart_area_selected_cells={}",
            restart.area_write.selected_cells
        );
        println!(
            "mask_restart_area_grid={}",
            restart.area_write.output.display()
        );
        if let Some(postproc_report) = &report.postproc {
            println!(
                "mask_restart_contain={}",
                postproc_report.contain.output.display()
            );
            match &postproc_report.postproc {
                earthmesh_cli::MkgrdFinalDomainPostprocReport::Earth(postproc) => {
                    println!(
                        "mask_restart_postproc_gridfile={}",
                        postproc.final_gridfile.output.display()
                    );
                    println!(
                        "mask_restart_postproc_patchtype={}",
                        postproc.patchtype.output.display()
                    );
                    println!(
                        "mask_restart_postproc_earthmesh_info={}",
                        postproc.earthmesh_info.output.display()
                    );
                }
                earthmesh_cli::MkgrdFinalDomainPostprocReport::Land(postproc) => {
                    println!(
                        "mask_restart_postproc_gridfile={}",
                        postproc.final_gridfile.output.display()
                    );
                    println!(
                        "mask_restart_postproc_patchtype={}",
                        postproc.patchtype.output.display()
                    );
                }
                earthmesh_cli::MkgrdFinalDomainPostprocReport::Ocean(postproc) => {
                    println!(
                        "mask_restart_postproc_gridfile={}",
                        postproc.final_gridfile.output.display()
                    );
                    if let Some(obc) = &postproc.obc {
                        println!("mask_restart_postproc_obc={}", obc.output.display());
                    }
                    if let Some(obcv2) = &postproc.obcv2 {
                        println!("mask_restart_postproc_obcv2={}", obcv2.output.display());
                    }
                }
                earthmesh_cli::MkgrdFinalDomainPostprocReport::Atmos(postproc) => {
                    println!(
                        "mask_restart_postproc_mpas_simple={}",
                        postproc.output.display()
                    );
                }
                earthmesh_cli::MkgrdFinalDomainPostprocReport::AtmosFull(postproc) => {
                    println!(
                        "mask_restart_postproc_mpas={}",
                        postproc.mesh.output.display()
                    );
                    println!(
                        "mask_restart_postproc_mpas_graph={}",
                        postproc.graph_info.output.display()
                    );
                }
            }
        }
        return Ok(());
    }
    if run_mask_restart_area_judge_refine {
        let source_state_path = restart_refine_source_state_path.ok_or_else(|| {
            usage("--run-mask-restart-area-judge-refine requires --restart-refine-source-state")
        })?;
        let initial_gridfile = infer_restart_refine_initial_gridfile_arg(
            &namelist,
            restart_refine_initial_gridfile.as_deref(),
        )?;
        fs::metadata(&source_state_path).map_err(|err| {
            format!(
                "--restart-refine-source-state could not read {}: {err}",
                source_state_path.display()
            )
        })?;
        fs::metadata(&initial_gridfile).map_err(|err| {
            format!(
                "--restart-refine-initial-gridfile could not read {}: {err}",
                initial_gridfile.display()
            )
        })?;
        let olam_namelist =
            write_olam_restart_refine_namelist(&namelist, &workdir, &initial_gridfile)?;
        let report = earthmesh_cli::run_mkgrd_olam_specified_refine_global_source_namelist(
            &olam_namelist,
            &workdir,
            max_tris,
            source_gridnum_perdegree,
        )
        .map_err(|err| err.to_string())?;
        println!("mask_restart_action=OlamRefine");
        print_olam_refine_report(&report);
        return Ok(());
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
        let olam_namelist =
            write_olam_restart_refine_namelist(&namelist, &workdir, &initial_gridfile)?;
        let report = earthmesh_cli::run_mkgrd_olam_specified_refine_global_source_namelist(
            &olam_namelist,
            &workdir,
            max_tris,
            source_gridnum_perdegree,
        )
        .map_err(|err| err.to_string())?;
        let _ = source_first_triangle_id;
        let _ = mask_postproc_num_vertex;
        println!("mask_restart_action=OlamRefine");
        print_olam_refine_report(&report);
        return Ok(());
    }
    if run_refine_landtype_source {
        let namelist_path = PathBuf::from(&namelist);
        let report = earthmesh_cli::run_mkgrd_olam_specified_refine_global_source_namelist(
            &namelist_path,
            &workdir,
            max_tris,
            source_gridnum_perdegree,
        )
        .map_err(|err| err.to_string())?;
        print_olam_refine_report(&report);
        return Ok(());
    }

    if let Some(source_state_path) = source_state_path {
        fs::metadata(&source_state_path).map_err(|err| {
            format!(
                "--run-refine-source-state could not read {}: {err}",
                source_state_path.display()
            )
        })?;
        let report = earthmesh_cli::run_mkgrd_olam_specified_refine_global_source_namelist(
            PathBuf::from(&namelist),
            &workdir,
            max_tris,
            source_gridnum_perdegree,
        )
        .map_err(|err| err.to_string())?;
        print_olam_refine_report(&report);
        return Ok(());
    }
    if run_refine_passthrough {
        let report = earthmesh_cli::run_mkgrd_olam_specified_refine_global_source_namelist(
            PathBuf::from(namelist),
            &workdir,
            max_tris,
            source_gridnum_perdegree,
        )
        .map_err(|err| err.to_string())?;
        print_olam_refine_report(&report);
        return Ok(());
    }

    let report = earthmesh_cli::run_mkgrd_top_level_namelist(
        PathBuf::from(&namelist),
        &workdir,
        max_tris,
        mask_restart_max_iter,
    )
    .map_err(|err| err.to_string())?;

    match report {
        earthmesh_cli::MkgrdTopLevelDispatchRunReport::Gridinit(report) => {
            println!("gridfile={}", report.gridfile.output.display());
            println!("sjx_points={}", report.gridfile.sjx_points);
            println!("lbx_points={}", report.gridfile.lbx_points);
        }
        earthmesh_cli::MkgrdTopLevelDispatchRunReport::OlamRefineGlobalSource(report) => {
            print_olam_refine_report(&report);
        }
        earthmesh_cli::MkgrdTopLevelDispatchRunReport::MaskRestartPatch(report) => {
            println!("mask_restart_action={:?}", report.plan.remask.action);
            println!(
                "mask_patch_reports={}",
                report.workspace_mask.mask_reports.len()
            );
            println!(
                "mask_patch_ndm={}",
                report.workspace_mask.mask_counts.mask_patch_ndm[0]
            );
        }
        earthmesh_cli::MkgrdTopLevelDispatchRunReport::MaskRestartOcean(report) => {
            println!("mask_restart_action={:?}", report.plan.remask.action);
            println!(
                "mask_postproc_result_gridfile={}",
                report.postproc.final_gridfile.output.display()
            );
            if let Some(obc) = &report.postproc.obc {
                println!("mask_postproc_obc={}", obc.output.display());
            }
            if let Some(obcv2) = &report.postproc.obcv2 {
                println!("mask_postproc_obcv2={}", obcv2.output.display());
            }
        }
        earthmesh_cli::MkgrdTopLevelDispatchRunReport::MaskRestartAreaJudge(report) => {
            print_mask_restart_area_judge_report(&report);
        }
        earthmesh_cli::MkgrdTopLevelDispatchRunReport::MaskRestartPlan(report) => {
            println!("mask_restart_action={:?}", report.remask.action);
            println!("mask_restart_step={}", report.remask.step);
            println!("mask_restart_file_dir={}", report.remask.file_dir.display());
        }
    }
    Ok(())
}

fn print_top_level_dispatch_report(report: &earthmesh_cli::MkgrdTopLevelDispatchRunReport) {
    match report {
        earthmesh_cli::MkgrdTopLevelDispatchRunReport::Gridinit(report) => {
            println!("gridfile={}", report.gridfile.output.display());
            println!("sjx_points={}", report.gridfile.sjx_points);
            println!("lbx_points={}", report.gridfile.lbx_points);
        }
        earthmesh_cli::MkgrdTopLevelDispatchRunReport::OlamRefineGlobalSource(report) => {
            print_olam_refine_report(report);
        }
        earthmesh_cli::MkgrdTopLevelDispatchRunReport::MaskRestartPatch(report) => {
            println!("mask_restart_action={:?}", report.plan.remask.action);
            println!(
                "mask_patch_reports={}",
                report.workspace_mask.mask_reports.len()
            );
            println!(
                "mask_patch_ndm={}",
                report.workspace_mask.mask_counts.mask_patch_ndm[0]
            );
        }
        earthmesh_cli::MkgrdTopLevelDispatchRunReport::MaskRestartOcean(report) => {
            println!("mask_restart_action={:?}", report.plan.remask.action);
            println!(
                "mask_postproc_result_gridfile={}",
                report.postproc.final_gridfile.output.display()
            );
            if let Some(obc) = &report.postproc.obc {
                println!("mask_postproc_obc={}", obc.output.display());
            }
            if let Some(obcv2) = &report.postproc.obcv2 {
                println!("mask_postproc_obcv2={}", obcv2.output.display());
            }
        }
        earthmesh_cli::MkgrdTopLevelDispatchRunReport::MaskRestartAreaJudge(report) => {
            print_mask_restart_area_judge_report(report);
        }
        earthmesh_cli::MkgrdTopLevelDispatchRunReport::MaskRestartPlan(report) => {
            println!("mask_restart_action={:?}", report.remask.action);
            println!("mask_restart_step={}", report.remask.step);
            println!("mask_restart_file_dir={}", report.remask.file_dir.display());
        }
    }
}

fn print_olam_refine_report(report: &earthmesh_cli::MkgrdOlamSpecifiedRefineRunReport) {
    println!("refine_source=olam_global_source");
    println!("gridfile={}", report.output.output.display());
    println!("sjx_points={}", report.output.sjx_points);
    println!("lbx_points={}", report.output.lbx_points);
    println!("olam_regions={}", report.regions.len());
    println!("olam_max_level={}", report.max_level);
    println!("olam_transition_faces={}", report.transition_faces);
    println!("olam_spring_nest_passes={}", report.spring_nest_passes);
    println!(
        "olam_spring_nest_iterations={}",
        report.spring_nest_iterations
    );
    if let Some(raw_output) = &report.raw_output {
        println!("olam_raw_gridfile={}", raw_output.output.display());
    }
    if let Some(landtype_masked_cells) = report.landtype_masked_cells {
        println!("olam_landtype_masked_cells={landtype_masked_cells}");
    }
    if let Some(coupled) = &report.coupled_outputs {
        println!(
            "olam_land_gridfile={}",
            coupled.land_output.output.display()
        );
        println!(
            "olam_ocean_gridfile={}",
            coupled.ocean_output.output.display()
        );
        println!("olam_coupling_csv={}", coupled.coupling_csv.display());
        println!(
            "olam_coupling_netcdf={}",
            coupled.coupling_netcdf.output.display()
        );
        println!("olam_coupling_manifest={}", coupled.manifest.display());
        println!("olam_coupling_rows={}", coupled.coupling_netcdf.rows);
    }
}

fn write_olam_restart_refine_namelist(
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

fn print_mask_restart_area_judge_report(
    report: &earthmesh_cli::MkgrdRestartAreaJudgeGlobalSourceRunReport,
) {
    let restart = &report.restart;
    println!("mask_restart_action={:?}", restart.plan.remask.action);
    println!(
        "mask_patch_reports={}",
        restart.workspace_mask.mask_reports.len()
    );
    println!(
        "mask_restart_area_selected_cells={}",
        restart.area_write.selected_cells
    );
    println!(
        "mask_restart_area_grid={}",
        restart.area_write.output.display()
    );
    if let Some(postproc_report) = &report.postproc {
        println!(
            "mask_restart_contain={}",
            postproc_report.contain.output.display()
        );
        match &postproc_report.postproc {
            earthmesh_cli::MkgrdFinalDomainPostprocReport::Earth(postproc) => {
                println!(
                    "mask_restart_postproc_gridfile={}",
                    postproc.final_gridfile.output.display()
                );
                println!(
                    "mask_restart_postproc_patchtype={}",
                    postproc.patchtype.output.display()
                );
                println!(
                    "mask_restart_postproc_earthmesh_info={}",
                    postproc.earthmesh_info.output.display()
                );
            }
            earthmesh_cli::MkgrdFinalDomainPostprocReport::Land(postproc) => {
                println!(
                    "mask_restart_postproc_gridfile={}",
                    postproc.final_gridfile.output.display()
                );
                println!(
                    "mask_restart_postproc_patchtype={}",
                    postproc.patchtype.output.display()
                );
            }
            earthmesh_cli::MkgrdFinalDomainPostprocReport::Ocean(postproc) => {
                println!(
                    "mask_restart_postproc_gridfile={}",
                    postproc.final_gridfile.output.display()
                );
                if let Some(obc) = &postproc.obc {
                    println!("mask_restart_postproc_obc={}", obc.output.display());
                }
                if let Some(obcv2) = &postproc.obcv2 {
                    println!("mask_restart_postproc_obcv2={}", obcv2.output.display());
                }
            }
            earthmesh_cli::MkgrdFinalDomainPostprocReport::Atmos(postproc) => {
                println!(
                    "mask_restart_postproc_mpas_simple={}",
                    postproc.output.display()
                );
            }
            earthmesh_cli::MkgrdFinalDomainPostprocReport::AtmosFull(postproc) => {
                println!(
                    "mask_restart_postproc_mpas={}",
                    postproc.mesh.output.display()
                );
                println!(
                    "mask_restart_postproc_mpas_graph={}",
                    postproc.graph_info.output.display()
                );
            }
        }
    }
}

fn infer_restart_refine_initial_gridfile_arg(
    namelist: &str,
    explicit: Option<&std::path::Path>,
) -> Result<PathBuf, String> {
    if let Some(path) = explicit {
        return Ok(path.to_path_buf());
    }
    let contents = fs::read_to_string(namelist)
        .map_err(|err| format!("failed to read namelist {namelist}: {err}"))?;
    let config = earthmesh_core::EarthmeshConfig::from_mkgrd_namelist(&contents)
        .map_err(|err| format!("failed to parse namelist {namelist}: {err}"))?;
    earthmesh_cli::infer_restart_refine_initial_gridfile_from_config(&config)
        .map_err(|err| err.to_string())
}

fn run_merit_hydro_geojson(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let merit_root = PathBuf::from(
        args.next()
            .ok_or_else(|| usage("--merit-hydro-geojson requires a MERIT root directory"))?,
    );
    let output_dir = PathBuf::from(
        args.next()
            .ok_or_else(|| usage("--merit-hydro-geojson requires an output directory"))?,
    );
    let mut bbox: Option<earthmesh_cli::MeritLonLatBbox> = None;
    let mut stride = 1_usize;
    let mut thresholds = earthmesh_cli::MeritMaskThresholds::default();
    let mut include_surface_masks = true;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bbox" => {
                let west =
                    parse_f64_arg("--bbox west", &next_required_arg(&mut args, "--bbox west")?)?;
                let south = parse_f64_arg(
                    "--bbox south",
                    &next_required_arg(&mut args, "--bbox south")?,
                )?;
                let east =
                    parse_f64_arg("--bbox east", &next_required_arg(&mut args, "--bbox east")?)?;
                let north = parse_f64_arg(
                    "--bbox north",
                    &next_required_arg(&mut args, "--bbox north")?,
                )?;
                bbox = Some(earthmesh_cli::MeritLonLatBbox {
                    west,
                    east,
                    south,
                    north,
                });
            }
            "--stride" => {
                let value = next_required_arg(&mut args, "--stride")?;
                stride = parse_positive_usize("--stride", &value)?;
            }
            "--r2-width-m" => {
                let value = next_required_arg(&mut args, "--r2-width-m")?;
                thresholds.r2_width_m = parse_positive_f64("--r2-width-m", &value)?;
            }
            "--r3-width-m" => {
                let value = next_required_arg(&mut args, "--r3-width-m")?;
                thresholds.r3_width_m = parse_positive_f64("--r3-width-m", &value)?;
            }
            "--r2-upa-km2" => {
                let value = next_required_arg(&mut args, "--r2-upa-km2")?;
                thresholds.r2_upa_km2 = parse_positive_f64("--r2-upa-km2", &value)?;
            }
            "--r3-upa-km2" => {
                let value = next_required_arg(&mut args, "--r3-upa-km2")?;
                thresholds.r3_upa_km2 = parse_positive_f64("--r3-upa-km2", &value)?;
            }
            "--skip-surface-mask" => {
                include_surface_masks = false;
            }
            "-h" | "--help" => return Err(usage("")),
            other => return Err(usage(&format!("unknown MERIT-Hydro argument {other}"))),
        }
    }

    let bbox = bbox.ok_or_else(|| usage("--merit-hydro-geojson requires --bbox W S E N"))?;
    let tile_paths = earthmesh_cli::select_merit_hydro_tiles(&merit_root, bbox)
        .map_err(|err| err.to_string())?;
    if tile_paths.is_empty() {
        return Err(format!(
            "no MERIT-Hydro tiles in {} intersect bbox",
            merit_root.display()
        ));
    }
    let mut windows = Vec::with_capacity(tile_paths.len());
    for tile in &tile_paths {
        windows.push(
            earthmesh_cli::read_merit_hydro_window(tile, bbox, stride)
                .map_err(|err| err.to_string())?,
        );
    }
    let report = earthmesh_cli::write_merit_hydro_mask_geojson_layers(
        &windows,
        thresholds,
        &output_dir,
        include_surface_masks,
    )
    .map_err(|err| err.to_string())?;
    println!("merit_tile_count={}", report.window_count);
    println!("merit_masks={}", report.combined_geojson.display());
    println!("merit_river_masks={}", report.river_geojson.display());
    println!("merit_coast_masks={}", report.coast_geojson.display());
    if let Some(surface) = &report.surface_geojson {
        println!("merit_surface_masks={}", surface.display());
    }
    println!("merit_summary={}", report.summary_json.display());
    println!("merit_features={}", report.combined_feature_count);
    Ok(())
}

fn run_cama_reach_export(
    command: &str,
    mut args: impl Iterator<Item = String>,
) -> Result<(), String> {
    let output_label = match command {
        "--cama-reach-jsonl" => "JSONL",
        "--cama-reach-geojson" => "GeoJSON",
        _ => {
            return Err(usage(&format!(
                "unknown CaMa reach export command {command}"
            )))
        }
    };
    let map_dir = PathBuf::from(
        args.next()
            .ok_or_else(|| usage(&format!("{command} requires a map_dir")))?,
    );
    let output = PathBuf::from(
        args.next()
            .ok_or_else(|| usage(&format!("{command} requires an output {output_label} path")))?,
    );
    let mut bbox: Option<earthmesh_cli::CamaLonLatBbox> = None;
    let mut target_dx_km: Option<f64> = None;
    let mut uparea_to_km2 = 1.0e-6_f64;
    let mut y_reversed_storage = true;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bbox" => {
                let west =
                    parse_f64_arg("--bbox west", &next_required_arg(&mut args, "--bbox west")?)?;
                let south = parse_f64_arg(
                    "--bbox south",
                    &next_required_arg(&mut args, "--bbox south")?,
                )?;
                let east =
                    parse_f64_arg("--bbox east", &next_required_arg(&mut args, "--bbox east")?)?;
                let north = parse_f64_arg(
                    "--bbox north",
                    &next_required_arg(&mut args, "--bbox north")?,
                )?;
                bbox = Some(earthmesh_cli::CamaLonLatBbox {
                    west,
                    east,
                    south,
                    north,
                });
            }
            "--target-dx-km" => {
                let value = next_required_arg(&mut args, "--target-dx-km")?;
                target_dx_km = Some(parse_positive_f64("--target-dx-km", &value)?);
            }
            "--uparea-to-km2" => {
                let value = next_required_arg(&mut args, "--uparea-to-km2")?;
                uparea_to_km2 = parse_positive_f64("--uparea-to-km2", &value)?;
            }
            "--no-yrev" => {
                y_reversed_storage = false;
            }
            "-h" | "--help" => return Err(usage("")),
            other => {
                return Err(usage(&format!(
                    "unknown CaMa reach export argument {other}"
                )))
            }
        }
    }

    let bbox = bbox.ok_or_else(|| usage(&format!("{command} requires --bbox W S E N")))?;
    let target_dx_km =
        target_dx_km.ok_or_else(|| usage(&format!("{command} requires --target-dx-km")))?;
    let inventory = earthmesh_cli::read_cama_reach_inventory_from_map_dir(
        &map_dir,
        bbox,
        target_dx_km,
        uparea_to_km2,
        y_reversed_storage,
    )
    .map_err(|err| err.to_string())?;
    match command {
        "--cama-reach-jsonl" => {
            let report = earthmesh_cli::write_cama_reach_inventory_jsonl(&inventory, &output)
                .map_err(|err| err.to_string())?;
            println!("cama_reach_jsonl={}", report.output.display());
            println!("cama_reach_records={}", report.record_count);
        }
        "--cama-reach-geojson" => {
            let report =
                earthmesh_cli::write_cama_reach_inventory_point_geojson(&inventory, &output)
                    .map_err(|err| err.to_string())?;
            println!("cama_reach_geojson={}", report.output.display());
            println!("cama_reach_features={}", report.feature_count);
        }
        _ => unreachable!("validated CaMa reach export command"),
    }
    Ok(())
}

fn run_hydro_close_recipe(args: impl Iterator<Item = String>) -> Result<(), String> {
    let mut args = args.collect::<Vec<_>>().into_iter();
    let input_geojson = PathBuf::from(
        args.next()
            .ok_or_else(|| usage("--hydro-close-recipe requires an input GeoJSON"))?,
    );
    let output_prefix = PathBuf::from(
        args.next()
            .ok_or_else(|| usage("--hydro-close-recipe requires an output prefix"))?,
    );
    let output_json = PathBuf::from(
        args.next()
            .ok_or_else(|| usage("--hydro-close-recipe requires an output recipe JSON"))?,
    );
    let rest = args.collect::<Vec<_>>();
    let mut class_refine: Option<BTreeMap<String, usize>> = None;
    let mut buffer_deg_by_refine_degree = BTreeMap::<usize, f64>::new();
    let mut simplify_tolerance_deg = 0.0_f64;
    let mut example_namelist: Option<String> = None;

    let mut index = 0_usize;
    while index < rest.len() {
        match rest[index].as_str() {
            "--class-refine" => {
                index += 1;
                let start = index;
                let mut parsed = BTreeMap::<String, usize>::new();
                while index < rest.len() && !rest[index].starts_with("--") {
                    let (class, degree) = parse_key_usize_pair("--class-refine", &rest[index])?;
                    parsed.insert(class, degree);
                    index += 1;
                }
                if index == start {
                    return Err(usage("--class-refine requires at least one CLASS=DEGREE"));
                }
                class_refine = Some(parsed);
            }
            "--buffer-deg-by-refine-degree" => {
                index += 1;
                let start = index;
                while index < rest.len() && !rest[index].starts_with("--") {
                    let (degree, buffer) =
                        parse_usize_f64_pair("--buffer-deg-by-refine-degree", &rest[index])?;
                    if buffer < 0.0 {
                        return Err(usage(
                            "--buffer-deg-by-refine-degree buffers must be non-negative",
                        ));
                    }
                    buffer_deg_by_refine_degree.insert(degree, buffer);
                    index += 1;
                }
                if index == start {
                    return Err(usage(
                        "--buffer-deg-by-refine-degree requires at least one DEGREE=BUFFER",
                    ));
                }
            }
            "--simplify-tolerance-deg" => {
                index += 1;
                let value = rest
                    .get(index)
                    .ok_or_else(|| usage("--simplify-tolerance-deg requires a value"))?;
                simplify_tolerance_deg = parse_nonnegative_f64("--simplify-tolerance-deg", value)?;
                index += 1;
            }
            "--example-namelist" => {
                index += 1;
                example_namelist = Some(
                    rest.get(index)
                        .ok_or_else(|| usage("--example-namelist requires a value"))?
                        .clone(),
                );
                index += 1;
            }
            "-h" | "--help" => return Err(usage("")),
            other => {
                return Err(usage(&format!(
                    "unknown hydro close recipe argument {other}"
                )))
            }
        }
    }

    let report = earthmesh_cli::write_hydro_close_refinement_recipe_json(
        &output_json,
        earthmesh_cli::HydroCloseRefinementRecipeOptions {
            input_geojson,
            output_prefix,
            class_refine: class_refine
                .unwrap_or_else(earthmesh_cli::default_hydro_close_class_refine),
            buffer_deg_by_refine_degree,
            simplify_tolerance_deg,
            example_namelist,
        },
    )
    .map_err(|err| err.to_string())?;
    println!("hydro_close_recipe={}", report.output_json.display());
    println!("hydro_close_max_iter_spc={}", report.max_iter_spc);
    println!("hydro_close_class_count={}", report.class_count);
    println!("hydro_close_buffer_count={}", report.buffer_count);
    Ok(())
}

fn run_hydro_close_mask_nmls(args: impl Iterator<Item = String>) -> Result<(), String> {
    let mut args = args.collect::<Vec<_>>().into_iter();
    let input_geojson = PathBuf::from(
        args.next()
            .ok_or_else(|| usage("--hydro-close-mask-nmls requires an input GeoJSON"))?,
    );
    let output_prefix = PathBuf::from(
        args.next()
            .ok_or_else(|| usage("--hydro-close-mask-nmls requires an output prefix"))?,
    );
    let rest = args.collect::<Vec<_>>();
    let mut class_refine: Option<BTreeMap<String, usize>> = None;
    let mut max_rings_per_class: Option<usize> = None;
    let mut max_rings_by_class = BTreeMap::<String, usize>::new();
    let mut max_masks_per_refine_degree = Some(999_usize);
    let mut min_ring_separation_deg = 0.0_f64;
    let mut buffer_deg_by_refine_degree = BTreeMap::<usize, f64>::new();
    let mut simplify_tolerance_deg = 0.0_f64;
    let mut dissolve_overlapping_envelopes = false;
    let mut cumulative_refine = true;

    let mut index = 0_usize;
    while index < rest.len() {
        match rest[index].as_str() {
            "--class-refine" => {
                index += 1;
                let start = index;
                let mut parsed = BTreeMap::<String, usize>::new();
                while index < rest.len() && !rest[index].starts_with("--") {
                    let (class, degree) = parse_key_usize_pair("--class-refine", &rest[index])?;
                    parsed.insert(class, degree);
                    index += 1;
                }
                if index == start {
                    return Err(usage("--class-refine requires at least one CLASS=DEGREE"));
                }
                class_refine = Some(parsed);
            }
            "--max-rings-per-class" => {
                index += 1;
                let value = rest
                    .get(index)
                    .ok_or_else(|| usage("--max-rings-per-class requires a value"))?;
                max_rings_per_class =
                    Some(parse_nonnegative_usize("--max-rings-per-class", value)?);
                index += 1;
            }
            "--max-rings-by-class" => {
                index += 1;
                let start = index;
                while index < rest.len() && !rest[index].starts_with("--") {
                    let (class, cap) =
                        parse_key_nonnegative_usize_pair("--max-rings-by-class", &rest[index])?;
                    max_rings_by_class.insert(class, cap);
                    index += 1;
                }
                if index == start {
                    return Err(usage(
                        "--max-rings-by-class requires at least one CLASS=COUNT",
                    ));
                }
            }
            "--max-masks-per-refine-degree" => {
                index += 1;
                let value = rest
                    .get(index)
                    .ok_or_else(|| usage("--max-masks-per-refine-degree requires a value"))?;
                max_masks_per_refine_degree = Some(parse_nonnegative_usize(
                    "--max-masks-per-refine-degree",
                    value,
                )?);
                index += 1;
            }
            "--no-max-masks-per-refine-degree" => {
                max_masks_per_refine_degree = None;
                index += 1;
            }
            "--min-ring-separation-deg" => {
                index += 1;
                let value = rest
                    .get(index)
                    .ok_or_else(|| usage("--min-ring-separation-deg requires a value"))?;
                min_ring_separation_deg =
                    parse_nonnegative_f64("--min-ring-separation-deg", value)?;
                index += 1;
            }
            "--buffer-deg-by-refine-degree" => {
                index += 1;
                let start = index;
                while index < rest.len() && !rest[index].starts_with("--") {
                    let (degree, buffer) =
                        parse_usize_f64_pair("--buffer-deg-by-refine-degree", &rest[index])?;
                    if degree == 0 || buffer < 0.0 {
                        return Err(usage(
                            "--buffer-deg-by-refine-degree requires positive degrees and non-negative buffers",
                        ));
                    }
                    buffer_deg_by_refine_degree.insert(degree, buffer);
                    index += 1;
                }
                if index == start {
                    return Err(usage(
                        "--buffer-deg-by-refine-degree requires at least one DEGREE=BUFFER",
                    ));
                }
            }
            "--simplify-tolerance-deg" => {
                index += 1;
                let value = rest
                    .get(index)
                    .ok_or_else(|| usage("--simplify-tolerance-deg requires a value"))?;
                simplify_tolerance_deg = parse_nonnegative_f64("--simplify-tolerance-deg", value)?;
                index += 1;
            }
            "--dissolve-overlapping-envelopes" => {
                dissolve_overlapping_envelopes = true;
                index += 1;
            }
            "--non-cumulative-refine" => {
                cumulative_refine = false;
                index += 1;
            }
            "-h" | "--help" => return Err(usage("")),
            other => {
                return Err(usage(&format!(
                    "unknown hydro close-mask NML argument {other}"
                )))
            }
        }
    }

    let report = earthmesh_cli::write_hydro_close_mask_nmls(
        &input_geojson,
        &output_prefix,
        earthmesh_cli::HydroCloseMaskNmlOptions {
            class_refine: class_refine
                .unwrap_or_else(earthmesh_cli::default_hydro_close_class_refine),
            max_rings_per_class,
            max_rings_by_class,
            max_masks_per_refine_degree,
            min_ring_separation_deg,
            buffer_deg_by_refine_degree,
            simplify_tolerance_deg,
            dissolve_overlapping_envelopes,
            cumulative_refine,
        },
    )
    .map_err(|err| err.to_string())?;
    println!("hydro_close_mask_prefix={}", report.output_prefix.display());
    println!("hydro_close_mask_files={}", report.files.len());
    println!("hydro_close_mask_specs={}", report.spec_count);
    for file in &report.files {
        println!("hydro_close_mask_file={}", file.display());
    }
    Ok(())
}

fn run_hydro_composite_close_mask_nmls(args: impl Iterator<Item = String>) -> Result<(), String> {
    let mut args = args.collect::<Vec<_>>().into_iter();
    let recipe_json = PathBuf::from(
        args.next()
            .ok_or_else(|| usage("--hydro-composite-close-mask-nmls requires a recipe JSON"))?,
    );
    let output_prefix = PathBuf::from(
        args.next()
            .ok_or_else(|| usage("--hydro-composite-close-mask-nmls requires an output prefix"))?,
    );
    let rest = args.collect::<Vec<_>>();
    let mut summary_json: Option<PathBuf> = None;
    let mut index = 0_usize;
    while index < rest.len() {
        match rest[index].as_str() {
            "--summary-json" => {
                index += 1;
                summary_json = Some(PathBuf::from(
                    rest.get(index)
                        .ok_or_else(|| usage("--summary-json requires a value"))?,
                ));
                index += 1;
            }
            "-h" | "--help" => return Err(usage("")),
            other => {
                return Err(usage(&format!(
                    "unknown hydro composite close-mask NML argument {other}"
                )))
            }
        }
    }

    let report = earthmesh_cli::write_hydro_composite_close_mask_nmls(
        &recipe_json,
        &output_prefix,
        summary_json.as_ref(),
    )
    .map_err(|err| err.to_string())?;
    println!(
        "hydro_composite_close_mask_prefix={}",
        report.output_prefix.display()
    );
    println!("hydro_composite_close_mask_files={}", report.files.len());
    if let Some(path) = &report.summary_json {
        println!("hydro_composite_close_mask_summary={}", path.display());
    }
    for file in &report.files {
        println!("hydro_composite_close_mask_file={}", file.display());
    }
    Ok(())
}

/// `--hydro-cell-intersections <cells.geojson> <corridors.geojson> <out.geojson>
/// [--classes R2,R3] [--min-fraction F] [--unit-sphere-area]`:
/// overlay EarthMesh cells x river/coast corridors -> per-cell intersection GeoJSON
/// (the input that --colm-coupling-from-intersections consumes). Port of
/// earthmesh_intersection.py.
fn run_hydro_cell_intersections(args: impl Iterator<Item = String>) -> Result<(), String> {
    let rest = args.collect::<Vec<_>>();
    let mut positional: Vec<PathBuf> = Vec::new();
    let mut classes: Vec<String> = vec!["R2".into(), "R3".into()];
    let mut min_fraction = 0.0f64;
    let mut unit_sphere = false;
    let mut i = 0usize;
    while i < rest.len() {
        match rest[i].as_str() {
            "--classes" => {
                i += 1;
                classes = rest
                    .get(i)
                    .ok_or_else(|| usage("--classes requires a value"))?
                    .split(',')
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| s.trim().to_string())
                    .collect();
            }
            "--min-fraction" => {
                i += 1;
                min_fraction = rest
                    .get(i)
                    .and_then(|v| v.parse().ok())
                    .ok_or_else(|| usage("--min-fraction requires a number"))?;
            }
            "--unit-sphere-area" => unit_sphere = true,
            other if other.starts_with("--") => {
                return Err(usage(&format!(
                    "unknown --hydro-cell-intersections option: {other}"
                )))
            }
            other => positional.push(PathBuf::from(other)),
        }
        i += 1;
    }
    if positional.len() != 3 {
        return Err(usage(
            "--hydro-cell-intersections needs <cells.geojson> <corridors.geojson> <out.geojson>",
        ));
    }
    let count = earthmesh_cli::write_earthmesh_intersection_geojson(
        &positional[0],
        &positional[1],
        &positional[2],
        &classes,
        min_fraction,
        unit_sphere,
    )
    .map_err(|err| format!("cell intersections: {err}"))?;
    println!("hydro_cell_intersection_features={count}");
    println!("hydro_cell_intersection_output={}", positional[2].display());
    Ok(())
}

/// `--hydro-delivery-manifest --case-name <n> --eval-json <e> --ranking-json <r>
/// --output-json <m> [--file role=path ...] [--source role=path ...]`:
/// assemble the delivery-package manifest (port of refinement_package.py::_build_manifest).
fn run_hydro_delivery_manifest(args: impl Iterator<Item = String>) -> Result<(), String> {
    let rest = args.collect::<Vec<_>>();
    let mut case_name = String::new();
    let mut eval_json: Option<PathBuf> = None;
    let mut ranking_json: Option<PathBuf> = None;
    let mut output_json: Option<PathBuf> = None;
    let mut files: Vec<(String, String)> = Vec::new();
    let mut source_files: Vec<(String, String)> = Vec::new();
    let mut i = 0usize;
    let split_kv = |s: &str| -> Result<(String, String), String> {
        s.split_once('=')
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .ok_or_else(|| usage("expected role=path"))
    };
    while i < rest.len() {
        let need = |i: &mut usize| -> Result<String, String> {
            *i += 1;
            rest.get(*i)
                .cloned()
                .ok_or_else(|| usage("flag requires a value"))
        };
        match rest[i].as_str() {
            "--case-name" => case_name = need(&mut i)?,
            "--eval-json" => eval_json = Some(PathBuf::from(need(&mut i)?)),
            "--ranking-json" => ranking_json = Some(PathBuf::from(need(&mut i)?)),
            "--output-json" => output_json = Some(PathBuf::from(need(&mut i)?)),
            "--file" => files.push(split_kv(&need(&mut i)?)?),
            "--source" => source_files.push(split_kv(&need(&mut i)?)?),
            other => {
                return Err(usage(&format!(
                    "unknown --hydro-delivery-manifest option: {other}"
                )))
            }
        }
        i += 1;
    }
    let eval_json =
        eval_json.ok_or_else(|| usage("--hydro-delivery-manifest requires --eval-json"))?;
    let ranking_json =
        ranking_json.ok_or_else(|| usage("--hydro-delivery-manifest requires --ranking-json"))?;
    let output_json =
        output_json.ok_or_else(|| usage("--hydro-delivery-manifest requires --output-json"))?;
    earthmesh_cli::write_hydro_delivery_manifest(
        &case_name,
        &eval_json,
        &ranking_json,
        &output_json,
        &files,
        &source_files,
    )
    .map_err(|err| format!("delivery manifest: {err}"))?;
    println!("hydro_delivery_manifest_output={}", output_json.display());
    Ok(())
}

fn parse_int_csv(value: &str) -> Result<Vec<i64>, String> {
    let parsed: Vec<i64> = value
        .split(',')
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().parse::<i64>())
        .collect::<Result<_, _>>()
        .map_err(|_| usage("expected comma-separated integers"))?;
    if parsed.is_empty() {
        return Err(usage("expected at least one integer value"));
    }
    Ok(parsed)
}

/// `--hydro-sweep-recipes --river-geojson <r> --coast-geojson <c> --output-dir <d>
/// [--r2-caps 40,60,80] [--coast-caps 10,20,40] [--r3-cap 19]`:
/// write composite close-mask recipes for an R2 x COAST sweep (port of refinement_sweep.py).
fn run_hydro_sweep_recipes(args: impl Iterator<Item = String>) -> Result<(), String> {
    let rest = args.collect::<Vec<_>>();
    let mut river: Option<String> = None;
    let mut coast: Option<String> = None;
    let mut output_dir: Option<PathBuf> = None;
    let mut r2_caps = vec![40i64, 60, 80];
    let mut coast_caps = vec![10i64, 20, 40];
    let mut r3_cap = 19i64;
    let mut i = 0usize;
    let next = |rest: &[String], i: &mut usize, flag: &str| -> Result<String, String> {
        *i += 1;
        rest.get(*i)
            .cloned()
            .ok_or_else(|| usage(&format!("{flag} requires a value")))
    };
    while i < rest.len() {
        match rest[i].as_str() {
            "--river-geojson" => river = Some(next(&rest, &mut i, "--river-geojson")?),
            "--coast-geojson" => coast = Some(next(&rest, &mut i, "--coast-geojson")?),
            "--output-dir" => {
                output_dir = Some(PathBuf::from(next(&rest, &mut i, "--output-dir")?))
            }
            "--r2-caps" => r2_caps = parse_int_csv(&next(&rest, &mut i, "--r2-caps")?)?,
            "--coast-caps" => coast_caps = parse_int_csv(&next(&rest, &mut i, "--coast-caps")?)?,
            "--r3-cap" => {
                r3_cap = next(&rest, &mut i, "--r3-cap")?
                    .parse()
                    .map_err(|_| usage("--r3-cap requires an integer"))?
            }
            other => {
                return Err(usage(&format!(
                    "unknown --hydro-sweep-recipes option: {other}"
                )))
            }
        }
        i += 1;
    }
    let river = river.ok_or_else(|| usage("--hydro-sweep-recipes requires --river-geojson"))?;
    let coast = coast.ok_or_else(|| usage("--hydro-sweep-recipes requires --coast-geojson"))?;
    let output_dir =
        output_dir.ok_or_else(|| usage("--hydro-sweep-recipes requires --output-dir"))?;
    let count = earthmesh_cli::write_sweep_recipes(
        &output_dir,
        &river,
        &coast,
        r2_caps,
        coast_caps,
        r3_cap,
    )
    .map_err(|err| format!("sweep recipes: {err}"))?;
    println!("hydro_sweep_cases={count}");
    println!("hydro_sweep_output_dir={}", output_dir.display());
    Ok(())
}

/// `--hydro-sweep-rank <report1.json> [report2.json ...] --output-json <out.json>
/// [--max-background-cells N]`: rank refinement-eval reports (port of refinement_sweep.py).
fn run_hydro_sweep_rank(args: impl Iterator<Item = String>) -> Result<(), String> {
    let rest = args.collect::<Vec<_>>();
    let mut reports: Vec<PathBuf> = Vec::new();
    let mut output_json: Option<PathBuf> = None;
    let mut max_background: Option<i64> = None;
    let mut i = 0usize;
    while i < rest.len() {
        match rest[i].as_str() {
            "--output-json" => {
                i += 1;
                output_json = Some(PathBuf::from(
                    rest.get(i)
                        .ok_or_else(|| usage("--output-json requires a value"))?,
                ));
            }
            "--max-background-cells" => {
                i += 1;
                max_background = Some(
                    rest.get(i)
                        .and_then(|v| v.parse().ok())
                        .ok_or_else(|| usage("--max-background-cells requires an integer"))?,
                );
            }
            other if other.starts_with("--") => {
                return Err(usage(&format!(
                    "unknown --hydro-sweep-rank option: {other}"
                )))
            }
            other => reports.push(PathBuf::from(other)),
        }
        i += 1;
    }
    if reports.is_empty() {
        return Err(usage(
            "--hydro-sweep-rank requires at least one report path",
        ));
    }
    let output_json =
        output_json.ok_or_else(|| usage("--hydro-sweep-rank requires --output-json"))?;
    let recommended = earthmesh_cli::write_sweep_ranking(&reports, &output_json, max_background)
        .map_err(|err| format!("sweep ranking: {err}"))?;
    println!("hydro_sweep_recommended={recommended}");
    println!("hydro_sweep_ranking_output={}", output_json.display());
    Ok(())
}

/// `--hydro-refinement-eval <background.geojson> <intersections.geojson> <out.json>
/// [--coast-intersections-geojson <g>] [--log-path <l>] [--file-area-m2]`:
/// summarize hydro-refinement cells + river/coast overlaps (port of refinement_eval.py).
fn run_hydro_refinement_eval(args: impl Iterator<Item = String>) -> Result<(), String> {
    let rest = args.collect::<Vec<_>>();
    let mut positional: Vec<PathBuf> = Vec::new();
    let mut coast: Option<PathBuf> = None;
    let mut log_path: Option<PathBuf> = None;
    let mut unit_sphere = true;
    let mut i = 0usize;
    while i < rest.len() {
        match rest[i].as_str() {
            "--coast-intersections-geojson" => {
                i += 1;
                coast = Some(PathBuf::from(rest.get(i).ok_or_else(|| {
                    usage("--coast-intersections-geojson requires a value")
                })?));
            }
            "--log-path" => {
                i += 1;
                log_path = Some(PathBuf::from(
                    rest.get(i)
                        .ok_or_else(|| usage("--log-path requires a value"))?,
                ));
            }
            "--file-area-m2" => unit_sphere = false,
            other if other.starts_with("--") => {
                return Err(usage(&format!(
                    "unknown --hydro-refinement-eval option: {other}"
                )))
            }
            other => positional.push(PathBuf::from(other)),
        }
        i += 1;
    }
    if positional.len() != 3 {
        return Err(usage(
            "--hydro-refinement-eval needs <background.geojson> <intersections.geojson> <out.json>",
        ));
    }
    earthmesh_cli::write_refinement_eval_json(
        &positional[0],
        &positional[1],
        &positional[2],
        coast.as_deref(),
        log_path.as_deref(),
        unit_sphere,
    )
    .map_err(|err| format!("refinement eval: {err}"))?;
    println!("hydro_refinement_eval_output={}", positional[2].display());
    Ok(())
}

/// `--hydro-mesh-qa --delivery-manifest <m.json> --output-json <out.json>
/// [--colm-summary-json <s.json>] [--min-river-cells N] [--min-coast-cells N]`:
/// evaluate delivery-package QA gates (Rust port of util/hydro_mesh/qa_gates.py).
fn run_hydro_mesh_qa(args: impl Iterator<Item = String>) -> Result<(), String> {
    let rest = args.collect::<Vec<_>>();
    let mut delivery_manifest: Option<PathBuf> = None;
    let mut output_json: Option<PathBuf> = None;
    let mut colm_summary: Option<PathBuf> = None;
    let mut min_river: i64 = 1;
    let mut min_coast: i64 = 1;
    let mut i = 0usize;
    while i < rest.len() {
        match rest[i].as_str() {
            "--delivery-manifest" => {
                i += 1;
                delivery_manifest =
                    Some(PathBuf::from(rest.get(i).ok_or_else(|| {
                        usage("--delivery-manifest requires a value")
                    })?));
            }
            "--output-json" => {
                i += 1;
                output_json = Some(PathBuf::from(
                    rest.get(i)
                        .ok_or_else(|| usage("--output-json requires a value"))?,
                ));
            }
            "--colm-summary-json" => {
                i += 1;
                colm_summary =
                    Some(PathBuf::from(rest.get(i).ok_or_else(|| {
                        usage("--colm-summary-json requires a value")
                    })?));
            }
            "--min-river-cells" => {
                i += 1;
                min_river = rest
                    .get(i)
                    .and_then(|v| v.parse().ok())
                    .ok_or_else(|| usage("--min-river-cells requires an integer"))?;
            }
            "--min-coast-cells" => {
                i += 1;
                min_coast = rest
                    .get(i)
                    .and_then(|v| v.parse().ok())
                    .ok_or_else(|| usage("--min-coast-cells requires an integer"))?;
            }
            other => return Err(usage(&format!("unknown --hydro-mesh-qa option: {other}"))),
        }
        i += 1;
    }
    let delivery_manifest =
        delivery_manifest.ok_or_else(|| usage("--hydro-mesh-qa requires --delivery-manifest"))?;
    let output_json = output_json.ok_or_else(|| usage("--hydro-mesh-qa requires --output-json"))?;
    let report = earthmesh_cli::write_hydro_mesh_qa_report(
        &delivery_manifest,
        &output_json,
        colm_summary.as_deref(),
        min_river,
        min_coast,
    )
    .map_err(|err| format!("hydro mesh qa: {err}"))?;
    println!("hydro_mesh_qa_status={}", report.status);
    println!("hydro_mesh_qa_output={}", output_json.display());
    Ok(())
}

/// `--colm-coupling-from-intersections <intersection.geojson> <out.csv> [min_fraction]`:
/// assemble a CoLM coupling CSV from an EarthMesh cell×river intersection GeoJSON
/// (Rust port of util/hydro_mesh/colm_coupling.py).
fn run_colm_coupling_from_intersections(args: impl Iterator<Item = String>) -> Result<(), String> {
    let mut args = args.collect::<Vec<_>>().into_iter();
    let input_geojson = PathBuf::from(args.next().ok_or_else(|| {
        usage("--colm-coupling-from-intersections requires an input intersection GeoJSON")
    })?);
    let output_csv = PathBuf::from(
        args.next()
            .ok_or_else(|| usage("--colm-coupling-from-intersections requires an output CSV"))?,
    );
    let min_fraction = match args.next() {
        Some(value) => value
            .parse::<f64>()
            .map_err(|_| usage("min_fraction must be a number in [0,1]"))?,
        None => 0.0,
    };
    let rows = earthmesh_cli::write_colm_coupling_csv_from_intersections(
        &input_geojson,
        &output_csv,
        min_fraction,
    )
    .map_err(|err| format!("write coupling csv {}: {err}", output_csv.display()))?;
    println!("colm_coupling_rows={rows}");
    println!("colm_coupling_output={}", output_csv.display());
    Ok(())
}

fn run_colm_coupling_csv_to_netcdf(args: impl Iterator<Item = String>) -> Result<(), String> {
    let mut args = args.collect::<Vec<_>>().into_iter();
    let input_csv = PathBuf::from(
        args.next()
            .ok_or_else(|| usage("--colm-coupling-csv-to-netcdf requires an input CSV"))?,
    );
    let output_netcdf = PathBuf::from(
        args.next()
            .ok_or_else(|| usage("--colm-coupling-csv-to-netcdf requires an output NetCDF"))?,
    );
    let rest = args.collect::<Vec<_>>();
    let mut case_name = String::new();
    let mut delivery_manifest = PathBuf::new();
    let mut restart_template_netcdf: Option<PathBuf> = None;
    let mut forcing_template_netcdf: Option<PathBuf> = None;
    let mut index = 0_usize;
    while index < rest.len() {
        match rest[index].as_str() {
            "--case-name" => {
                index += 1;
                case_name = rest
                    .get(index)
                    .ok_or_else(|| usage("--case-name requires a value"))?
                    .clone();
                index += 1;
            }
            "--delivery-manifest" => {
                index += 1;
                delivery_manifest = PathBuf::from(
                    rest.get(index)
                        .ok_or_else(|| usage("--delivery-manifest requires a value"))?,
                );
                index += 1;
            }
            "--restart-template-netcdf" => {
                index += 1;
                restart_template_netcdf =
                    Some(PathBuf::from(rest.get(index).ok_or_else(|| {
                        usage("--restart-template-netcdf requires a value")
                    })?));
                index += 1;
            }
            "--forcing-template-netcdf" => {
                index += 1;
                forcing_template_netcdf =
                    Some(PathBuf::from(rest.get(index).ok_or_else(|| {
                        usage("--forcing-template-netcdf requires a value")
                    })?));
                index += 1;
            }
            "-h" | "--help" => return Err(usage("")),
            other => {
                return Err(usage(&format!(
                    "unknown CoLM coupling NetCDF argument {other}"
                )))
            }
        }
    }

    let report = earthmesh_cli::write_colm_coupling_netcdf_from_csv(
        &input_csv,
        &output_netcdf,
        &case_name,
        &delivery_manifest,
    )
    .map_err(|err| err.to_string())?;
    println!("colm_coupling_netcdf={}", report.output.display());
    println!("colm_coupling_rows={}", report.rows);
    let mut restart_template_output: Option<PathBuf> = None;
    let mut forcing_template_output: Option<PathBuf> = None;
    if let Some(restart_template_netcdf) = restart_template_netcdf {
        let restart_report = earthmesh_cli::write_colm_restart_template_netcdf_from_csv(
            &input_csv,
            &restart_template_netcdf,
            &case_name,
        )
        .map_err(|err| err.to_string())?;
        println!(
            "colm_restart_template_netcdf={}",
            restart_report.output.display()
        );
        println!("colm_restart_template_rows={}", restart_report.rows);
        restart_template_output = Some(restart_report.output);
    }
    if let Some(forcing_template_netcdf) = forcing_template_netcdf {
        let forcing_report = earthmesh_cli::write_colm_forcing_template_netcdf_from_csv(
            &input_csv,
            &forcing_template_netcdf,
            &case_name,
        )
        .map_err(|err| err.to_string())?;
        println!(
            "colm_forcing_template_netcdf={}",
            forcing_report.output.display()
        );
        println!("colm_forcing_template_rows={}", forcing_report.rows);
        forcing_template_output = Some(forcing_report.output);
    }
    if !delivery_manifest.as_os_str().is_empty() {
        let manifest = earthmesh_cli::write_colm_package_delivery_manifest(
            &delivery_manifest,
            &case_name,
            report.rows,
            &report.output,
            restart_template_output.as_deref(),
            forcing_template_output.as_deref(),
        )
        .map_err(|err| err.to_string())?;
        println!("colm_delivery_manifest={}", manifest.display());
    }
    Ok(())
}

fn next_required_arg(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<String, String> {
    args.next()
        .ok_or_else(|| usage(&format!("{flag} requires a value")))
}

fn parse_f64_arg(flag: &str, value: &str) -> Result<f64, String> {
    value
        .parse::<f64>()
        .map_err(|_| usage(&format!("{flag} must be a finite number")))
        .and_then(|parsed| {
            if parsed.is_finite() {
                Ok(parsed)
            } else {
                Err(usage(&format!("{flag} must be a finite number")))
            }
        })
}

fn parse_positive_f64(flag: &str, value: &str) -> Result<f64, String> {
    let parsed = parse_f64_arg(flag, value)?;
    if parsed <= 0.0 {
        return Err(usage(&format!("{flag} must be positive")));
    }
    Ok(parsed)
}

fn parse_nonnegative_f64(flag: &str, value: &str) -> Result<f64, String> {
    let parsed = parse_f64_arg(flag, value)?;
    if parsed < 0.0 {
        return Err(usage(&format!("{flag} must be non-negative")));
    }
    Ok(parsed)
}

fn parse_positive_usize(flag: &str, value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| usage(&format!("{flag} must be a positive integer")))?;
    if parsed == 0 {
        return Err(usage(&format!("{flag} must be a positive integer")));
    }
    Ok(parsed)
}

fn parse_nonnegative_usize(flag: &str, value: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .map_err(|_| usage(&format!("{flag} must be a non-negative integer")))
}

fn parse_key_usize_pair(flag: &str, value: &str) -> Result<(String, usize), String> {
    let (key, raw_value) = value
        .split_once('=')
        .ok_or_else(|| usage(&format!("{flag} values must use KEY=VALUE syntax")))?;
    let key = key.trim();
    if key.is_empty() {
        return Err(usage(&format!("{flag} keys must not be empty")));
    }
    Ok((
        key.to_string(),
        parse_positive_usize(flag, raw_value.trim())?,
    ))
}

fn parse_key_nonnegative_usize_pair(flag: &str, value: &str) -> Result<(String, usize), String> {
    let (key, raw_value) = value
        .split_once('=')
        .ok_or_else(|| usage(&format!("{flag} values must use KEY=VALUE syntax")))?;
    let key = key.trim();
    if key.is_empty() {
        return Err(usage(&format!("{flag} keys must not be empty")));
    }
    Ok((
        key.to_string(),
        parse_nonnegative_usize(flag, raw_value.trim())?,
    ))
}

fn parse_usize_f64_pair(flag: &str, value: &str) -> Result<(usize, f64), String> {
    let (raw_key, raw_value) = value
        .split_once('=')
        .ok_or_else(|| usage(&format!("{flag} values must use DEGREE=VALUE syntax")))?;
    Ok((
        parse_positive_usize(flag, raw_key.trim())?,
        parse_f64_arg(flag, raw_value.trim())?,
    ))
}

fn parse_nonnegative_i32(flag: &str, value: &str) -> Result<i32, String> {
    let parsed = value
        .parse::<i32>()
        .map_err(|_| usage(&format!("{flag} must be a non-negative integer")))?;
    if parsed < 0 {
        return Err(usage(&format!("{flag} must be a non-negative integer")));
    }
    Ok(parsed)
}

fn usage(message: &str) -> String {
    let prefix = if message.is_empty() {
        String::new()
    } else {
        format!("{message}\n")
    };
    format!(
        "{prefix}usage: earthmesh_cli --cama-reach-jsonl <map_dir> <output.jsonl> --bbox W S E N --target-dx-km KM [--uparea-to-km2 SCALE] [--no-yrev]
       earthmesh_cli --cama-reach-geojson <map_dir> <output.geojson> --bbox W S E N --target-dx-km KM [--uparea-to-km2 SCALE] [--no-yrev]
       earthmesh_cli --merit-hydro-geojson <merit_root> <output_dir> --bbox W S E N [--stride N] [--r2-width-m M] [--r3-width-m M] [--r2-upa-km2 KM2] [--r3-upa-km2 KM2] [--skip-surface-mask]
       earthmesh_cli --hydro-close-recipe <input.geojson> <output_prefix> <recipe.json> [--class-refine CLASS=DEGREE ...] [--buffer-deg-by-refine-degree DEGREE=BUFFER ...] [--simplify-tolerance-deg DEG] [--example-namelist FILE]
       earthmesh_cli --hydro-close-mask-nmls <input.geojson> <output_prefix> [--class-refine CLASS=DEGREE ...] [--max-rings-per-class N] [--max-rings-by-class CLASS=COUNT ...] [--max-masks-per-refine-degree N | --no-max-masks-per-refine-degree] [--min-ring-separation-deg DEG] [--buffer-deg-by-refine-degree DEGREE=BUFFER ...] [--simplify-tolerance-deg DEG] [--dissolve-overlapping-envelopes] [--non-cumulative-refine]
       earthmesh_cli --hydro-composite-close-mask-nmls <recipe.json> <output_prefix> [--summary-json PATH]
       earthmesh_cli --colm-coupling-csv-to-netcdf <colm_coupling_cells.csv> <colm_coupling_cells.nc> [--case-name NAME] [--delivery-manifest PATH] [--restart-template-netcdf PATH] [--forcing-template-netcdf PATH]
       earthmesh_cli <mkgrd.nml> [--quiet] [--max-tris N] [--run-refine-passthrough --source-gridnum-perdegree N --source-nlons N --source-nlats N [--source-first-triangle-id N] | --run-refine-source-state PATH | --run-refine-landtype-source [--source-gridnum-perdegree N] [--source-first-triangle-id N] | --run-mask-restart-ocean [--mask-postproc-num-vertex N] [--mask-restart-max-iter N] | --run-mask-restart-patch [--mask-restart-max-iter N] | --run-mask-restart-area-judge [--source-gridnum-perdegree N --source-nlons N --source-nlats N] [--mask-restart-max-iter N] | --run-mask-restart-area-judge-refine --restart-refine-source-state PATH [--restart-refine-initial-gridfile PATH] | --run-mask-restart-area-judge-refine-landtype-source [--restart-refine-initial-gridfile PATH] [--source-gridnum-perdegree N] [--source-first-triangle-id N] | --restart-refine-initial-gridfile PATH [--restart-refine-source-state PATH] [--source-gridnum-perdegree N] [--source-first-triangle-id N]]"
    )
}
