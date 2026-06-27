use std::io;
use std::path::Path;

use crate::*;

/// Run the migrated namelist-level refine path using the standard migrated
/// executor stack: `MkgrdRefineSourceBranchExecutor` for source branches and
/// `MkgrdRefineLoopWorkingStateExecutor` for geometry/final-quality work.
pub fn run_mkgrd_refine_loop_namelist_with_migrated_executor<'a>(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    source_grid: MkgrdRefinePrepareSourceGridOptions<'_>,
    source_options: MkgrdRefineSourceBranchExecutorOptions<'a>,
    refine_executor: MkgrdRefineLoopWorkingStateExecutor,
    postproc_options: Option<MkgrdFinalDomainPostprocOptions<'_>>,
) -> io::Result<MkgrdRefineLoopNamelistRunReport> {
    run_mkgrd_refine_loop_namelist_with_migrated_executor_and_prepare_hook(
        namelist_source,
        workdir,
        source_grid,
        source_options,
        refine_executor,
        postproc_options,
        |_| Ok(()),
    )
}

/// Run the standard migrated refine-loop stack with an explicit hook after
/// namelist/workspace preparation and before file-backed execution.
///
/// This mirrors the Fortran ordering where `read_nl` prepares/cleans the working
/// directory before downstream grid state is consumed, while still allowing a
/// Rust gridinit or adapter caller to provide that state without hand-writing the
/// execution orchestration.
pub fn run_mkgrd_refine_loop_namelist_with_migrated_executor_and_prepare_hook<'a, F>(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    source_grid: MkgrdRefinePrepareSourceGridOptions<'_>,
    source_options: MkgrdRefineSourceBranchExecutorOptions<'a>,
    refine_executor: MkgrdRefineLoopWorkingStateExecutor,
    postproc_options: Option<MkgrdFinalDomainPostprocOptions<'_>>,
    after_prepare: F,
) -> io::Result<MkgrdRefineLoopNamelistRunReport>
where
    F: FnOnce(&MkgrdRefineLoopPrepareReport) -> io::Result<()>,
{
    let prepare =
        prepare_mkgrd_refine_loop_namelist_with_source_grid(namelist_source, workdir, source_grid)?;
    after_prepare(&prepare)?;
    let mut executor = mkgrd_migrated_refine_loop_executor_with_runtime_state(
        source_options,
        refine_executor,
        prepare.runtime_state.clone(),
    );
    let execution =
        run_mkgrd_refine_loop_execution(&prepare.plan, &mut executor, postproc_options)?;
    Ok(MkgrdRefineLoopNamelistRunReport { prepare, execution })
}

/// Build the non-destructive step schedule for the `mkgrd.F90` refine loop.
///
/// Execute the top-level `mkgrd.F90` refine-loop order using a pluggable kernel
/// executor for the heavy migrated/pending geometry branches.
///
/// This owns only orchestration: for each planned step, dispatch
/// `Area_judge_refine/Get_Contain/GetRef` source branches in order, then the
/// `refine_loop` step, then optional final quality check and final domain
/// handoff.  File names and early-exit truncation come from
/// `MkgrdRefineLoopIoPlan`.
pub fn run_mkgrd_refine_loop_execution(
    plan: &MkgrdRefineLoopIoPlan,
    executor: &mut impl MkgrdRefineLoopExecutor,
    postproc_options: Option<MkgrdFinalDomainPostprocOptions<'_>>,
) -> io::Result<MkgrdRefineLoopExecutionReport> {
    run_mkgrd_refine_loop_execution_with_final_domain_contain(
        plan,
        executor,
        None,
        postproc_options,
    )
}

/// Execute the refine loop plus the data_preprocess-derived final-domain
/// handoff. This keeps the final Area_judge write, `Get_Contain(0)` option
/// assembly, and land/ocean postprocess option mapping inside the Rust library
/// boundary instead of the CLI front-end.
pub fn run_mkgrd_refine_loop_execution_with_data_preprocess_final_domain_handoff<'a>(
    plan: &MkgrdRefineLoopIoPlan,
    executor: &mut impl MkgrdRefineLoopExecutor,
    state: &'a MkgrdDataPreprocessSourceState,
    mesh_type: &str,
    area_grid_file: &'a Path,
    mask_sea_ratio: f64,
    output_format: &'a str,
) -> io::Result<MkgrdRefineLoopExecutionReport> {
    let final_postproc_request =
        data_preprocess_source_state_final_postproc_request(state, mesh_type)?;
    let postproc_options = data_preprocess_source_state_final_postproc_options(
        final_postproc_request.as_ref(),
        state,
        mask_sea_ratio,
        output_format,
    )?;
    let contain_options = if matches!(
        postproc_options,
        Some(MkgrdFinalDomainPostprocOptions::Atmos { .. })
    ) {
        None
    } else {
        write_data_preprocess_source_state_final_domain_contain_options(
            state,
            mesh_type,
            area_grid_file,
        )?
    };
    run_mkgrd_refine_loop_execution_with_final_domain_contain(
        plan,
        executor,
        contain_options,
        postproc_options,
    )
}

/// Execute the top-level `mkgrd.F90` refine-loop order and optionally generate
/// the final `Get_Contain(0)` domain containment file before the final
/// `mask_postproc(mesh_type)` handoff.
pub fn run_mkgrd_refine_loop_execution_with_final_domain_contain(
    plan: &MkgrdRefineLoopIoPlan,
    executor: &mut impl MkgrdRefineLoopExecutor,
    contain_options: Option<MkgrdFinalDomainContainOptions<'_>>,
    postproc_options: Option<MkgrdFinalDomainPostprocOptions<'_>>,
) -> io::Result<MkgrdRefineLoopExecutionReport> {
    let mut executed_sources = 0;
    let mut executed_refine_steps = 0;

    for (step_index, step) in plan.steps.iter().enumerate() {
        if !earthmesh_core::progress::report("refine", step_index, plan.steps.len()) {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
        }
        for source in &step.sources {
            executor.run_source_branch(step, source)?;
            executed_sources += 1;
        }
        if step.run_refine_loop {
            executor.run_refine_loop_step(step)?;
            executed_refine_steps += 1;
        }
        if step.stop_after_step {
            break;
        }
    }

    let effective_plan = effective_mkgrd_refine_loop_io_plan(plan)?;
    let ran_final_quality_check = effective_plan.final_quality_check.run_quality_check;
    if ran_final_quality_check {
        let mut final_quality_check = effective_plan.final_quality_check.clone();
        if let Some(runtime_state) = executor.runtime_state().cloned() {
            enrich_mkgrd_final_quality_with_global_distance_steps_io(
                &mut final_quality_check,
                &runtime_state,
                effective_plan.max_iter,
            )?;
        }
        executor.run_final_quality_check(&final_quality_check)?;
    }

    let final_handoff = run_mkgrd_refine_loop_final_domain_handoff_with_domain_contain(
        &effective_plan,
        contain_options,
        postproc_options,
    )?;
    let mut runtime_state = executor.runtime_state().cloned();
    if let (Some(runtime_state), Some(contain)) = (
        runtime_state.as_mut(),
        final_handoff.generated_contain.as_ref(),
    ) {
        let counts = &contain.runtime_counts;
        runtime_state
            .record_mesh_counts_for_step(
                effective_plan.final_mask_postproc_step,
                counts.current_num_mp_step,
                counts.current_num_wp_step,
            )
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
        if counts.previous_num_vertex > 0 {
            runtime_state
                .record_num_vertex(counts.previous_num_vertex)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
        }
    }
    Ok(MkgrdRefineLoopExecutionReport {
        executed_sources,
        source_branch_reports: executor.source_branch_reports().to_vec(),
        runtime_state,
        executed_refine_steps,
        ran_final_quality_check,
        final_handoff,
    })
}
