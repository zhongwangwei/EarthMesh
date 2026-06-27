use std::fs;
use std::io;
use std::path::Path;

use earthmesh_core::MaskOperation;

use crate::*;

fn first_calculated_refine_area_output(plan: &MkgrdRefineLoopIoPlan) -> Option<&Path> {
    plan.steps
        .iter()
        .flat_map(|step| step.sources.iter())
        .find(|source| source.source == MkgrdRefineSource::CalculatedIterZero)
        .map(|source| source.area_judge_output.as_path())
}

/// Run a migrated refine loop using domain/sea-land state restored from an
/// `Area_judge` restart selected-grid file.
///
/// This is the Rust handoff for restart workflows that need the restored
/// `IsInDmArea_grid` and optional iter-zero calculated `IsInRfArea_grid` state
/// to feed the already-migrated `Area_judge_refine -> Get_Contain -> GetRef ->
/// refine_loop` executor stack.  The initial gridfile is copied into the first
/// planned refine-loop input after `read_nl` workspace preparation, preserving
/// the Fortran ordering without requiring callers to rebuild source-branch
/// options by hand.
pub fn run_mkgrd_refine_loop_namelist_with_area_judge_restart_grids_and_migrated_executor<
    RefineExecutor,
>(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    options: MkgrdAreaJudgeRestartRefineLoopOptions<'_>,
    refine_executor: &mut RefineExecutor,
    postproc_options: Option<MkgrdFinalDomainPostprocOptions<'_>>,
) -> io::Result<MkgrdAreaJudgeRestartRefineLoopRunReport>
where
    RefineExecutor: MkgrdRefineLoopExecutor,
{
    run_mkgrd_refine_loop_namelist_with_area_judge_restart_grids_and_migrated_executor_and_final_domain_contain(
        namelist_source,
        workdir,
        options,
        refine_executor,
        None,
        postproc_options,
    )
}

/// Run a migrated restart-grid refine loop and optionally generate final
/// domain containment before `mask_postproc(mesh_type)`.
pub fn run_mkgrd_refine_loop_namelist_with_area_judge_restart_grids_and_migrated_executor_and_final_domain_contain<
    RefineExecutor,
>(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    options: MkgrdAreaJudgeRestartRefineLoopOptions<'_>,
    refine_executor: &mut RefineExecutor,
    contain_options: Option<MkgrdFinalDomainContainOptions<'_>>,
    postproc_options: Option<MkgrdFinalDomainPostprocOptions<'_>>,
) -> io::Result<MkgrdAreaJudgeRestartRefineLoopRunReport>
where
    RefineExecutor: MkgrdRefineLoopExecutor,
{
    let mut prepare = prepare_mkgrd_refine_loop_namelist_with_source_grid(
        namelist_source,
        workdir,
        options.source_grid,
    )?;
    prepare.runtime_state.source_grid.maxlc = usize::try_from(options.maxlc).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "data_preprocess maxlc must be non-negative for runtime state, got {}",
                options.maxlc
            ),
        )
    })?;
    let runtime_refine = runtime_refine_from_prepare(&prepare)?.clone();
    if prepare.config.mask_restart {
        let max_iter_spc = usize::try_from(runtime_refine.max_iter_spc).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "restart refine handoff max_iter_spc must be non-negative",
            )
        })?;
        if runtime_refine.refine_setting == "specified" || runtime_refine.refine_setting == "mixed"
        {
            let operation = MaskOperation::new(
                "mask_refine",
                &runtime_refine.mask_refine_spc_type,
                &runtime_refine.mask_refine_spc_fprefix,
            );
            let report = apply_mask_operation(
                &operation,
                &prepare.plan.file_dir,
                max_iter_spc,
                &mut prepare.workspace_mask.mask_counts,
            )?;
            prepare.workspace_mask.mask_reports.push(report);
        }
        if runtime_refine.refine_setting == "calculate" || runtime_refine.refine_setting == "mixed"
        {
            let operation = MaskOperation::new(
                "mask_refine",
                &runtime_refine.mask_refine_cal_type,
                &runtime_refine.mask_refine_cal_fprefix,
            );
            let report = apply_mask_operation(
                &operation,
                &prepare.plan.file_dir,
                max_iter_spc,
                &mut prepare.workspace_mask.mask_counts,
            )?;
            prepare.workspace_mask.mask_reports.push(report);
        }
    }
    let calculated_refine_config = if runtime_refine.refine_cal {
        Some(AreaJudgeCalculatedRefineConfig {
            refine_setting: &runtime_refine.refine_setting,
            mask_refine_cal_type: &runtime_refine.mask_refine_cal_type,
            mask_refine_ndm: prepare.workspace_mask.mask_counts.mask_refine_ndm[0],
        })
    } else {
        None
    };
    let mask_patch = prepare
        .config
        .mask_patch_on
        .then_some(AreaJudgePatchConfig {
            mask_patch_type: &prepare.config.mask_patch_type,
            mask_patch_ndm: prepare.workspace_mask.mask_counts.mask_patch_ndm[0],
        });
    let refine_output = first_calculated_refine_area_output(&prepare.plan);
    let domain_output = prepare.plan.file_dir.join("result/IsInDmArea_grid.nc4");
    let restart = run_area_judge_restart_grids_fortran_indexed(AreaJudgeRestartGridsRunConfig {
        file_dir: &prepare.plan.file_dir,
        restart_input: options.restart_input,
        mask_patch,
        refine: runtime_refine.refine_cal,
        calculated_refine: calculated_refine_config,
        lon_vertex: options.source_grid.lon_vertex,
        lat_vertex: options.source_grid.lat_vertex,
        lon_i: options.source_grid.lon_i,
        lat_i: options.source_grid.lat_i,
        gridnum_perdegree: options.source_grid.gridnum_perdegree,
        nlons_source: options.source_grid.nlons_source,
        nlats_source: options.source_grid.nlats_source,
        domain_output: Some(&domain_output),
        refine_output,
    })?;

    let first_step = prepare.plan.steps.first().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "restart refine handoff requires at least one refine-loop step",
        )
    })?;
    if let Some(parent) = first_step.refine_loop_input_gridfile.parent() {
        fs::create_dir_all(parent)?;
    }
    if options.initial_gridfile != first_step.refine_loop_input_gridfile {
        fs::copy(
            options.initial_gridfile,
            &first_step.refine_loop_input_gridfile,
        )?;
    }

    let calculated_refine = restart
        .area
        .calculated_refine
        .as_ref()
        .map(|calculated| (calculated.is_in_area.as_slice(), calculated.bounds));
    let source_options = mkgrd_refine_source_branch_options_from_prepare(
        &prepare,
        options.source_grid,
        calculated_refine,
        Some(&restart.area.domain.is_in_domain),
        &restart.area.seaorland.seaorland,
        options.landtypes_global,
        options.num_vertex,
        options.maxlc,
    )?;
    let mut executor = MkgrdCompositeRefineLoopExecutor::new(
        MkgrdRefineSourceBranchExecutor::new(source_options),
        refine_executor,
    );
    let execution = run_mkgrd_refine_loop_execution_with_final_domain_contain(
        &prepare.plan,
        &mut executor,
        contain_options,
        postproc_options,
    )?;

    Ok(MkgrdAreaJudgeRestartRefineLoopRunReport {
        prepare,
        restart,
        execution,
    })
}
