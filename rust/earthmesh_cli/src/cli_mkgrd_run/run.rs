use std::env;
use std::fs;
use std::path::PathBuf;

use super::super::cli_args::{parse_nonnegative_i32, parse_positive_usize, usage};
use super::super::cli_mkgrd_output::{
    infer_restart_refine_initial_gridfile_arg, print_mask_restart_area_judge_report,
    print_mask_restart_ocean_report, print_mask_restart_patch_report, print_olam_refine_report,
    print_top_level_dispatch_report, write_olam_restart_refine_namelist,
};
use super::prepare::prepare_mkgrd_namelist;

pub(crate) fn run_mkgrd_or_project(
    first: String,
    mut args: impl Iterator<Item = String>,
) -> Result<(), String> {
    let namelist = prepare_mkgrd_namelist(first, &mut args)?;
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

        print_mask_restart_ocean_report(&report);
        return Ok(());
    }
    if run_mask_restart_patch {
        let report = earthmesh_cli::run_mkgrd_mask_restart_patch_namelist(
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
        print_mask_restart_area_judge_report(&report);
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
            if let Some(fvcom_2dm) = &report.fvcom_2dm {
                println!("fvcom_2dm={}", fvcom_2dm.output.display());
            }
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
