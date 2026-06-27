use std::io;
use std::path::Path;

use earthmesh_mesh::AreaJudgeSourceBounds;

use super::{
    mkgrd_calculated_refine_source_options_from_prepare,
    mkgrd_refine_source_branch_options_from_prepare,
    mkgrd_specified_refine_source_options_from_prepare,
};
use crate::*;

/// Run a specified-refinement-only migrated namelist path while deriving the
/// source branch executor options from the post-`read_nl` prepare state.
pub fn run_mkgrd_refine_loop_namelist_with_specified_migrated_executor_and_prepare_hook<'a, F>(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    source_grid: MkgrdRefinePrepareSourceGridOptions<'a>,
    is_in_domain: &'a [Vec<i32>],
    seaorland: &'a [Vec<i32>],
    num_vertex: usize,
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
    let source_options = mkgrd_specified_refine_source_options_from_prepare(
        &prepare,
        source_grid,
        is_in_domain,
        seaorland,
        num_vertex,
    )?;
    let mut executor = mkgrd_migrated_refine_loop_executor_with_runtime_state(
        MkgrdRefineSourceBranchExecutorOptions {
            calculated: None,
            specified: Some(source_options),
        },
        refine_executor,
        prepare.runtime_state.clone(),
    );
    let execution =
        run_mkgrd_refine_loop_execution(&prepare.plan, &mut executor, postproc_options)?;
    Ok(MkgrdRefineLoopNamelistRunReport { prepare, execution })
}

/// Run a calculated-refinement-only migrated namelist path while deriving the
/// source branch executor options from the post-`read_nl` prepare state.
pub fn run_mkgrd_refine_loop_namelist_with_calculated_migrated_executor_and_prepare_hook<'a, F>(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    source_grid: MkgrdRefinePrepareSourceGridOptions<'a>,
    calculated_refine: (&'a [Vec<i32>], AreaJudgeSourceBounds),
    seaorland: &'a [Vec<i32>],
    landtypes_global: &'a [Vec<i32>],
    num_vertex: usize,
    maxlc: i32,
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
    let source_options = mkgrd_calculated_refine_source_options_from_prepare(
        &prepare,
        source_grid,
        calculated_refine,
        seaorland,
        landtypes_global,
        num_vertex,
        maxlc,
    )?;
    let mut executor = mkgrd_migrated_refine_loop_executor_with_runtime_state(
        MkgrdRefineSourceBranchExecutorOptions {
            calculated: Some(source_options),
            specified: None,
        },
        refine_executor,
        prepare.runtime_state.clone(),
    );
    let execution =
        run_mkgrd_refine_loop_execution(&prepare.plan, &mut executor, postproc_options)?;
    Ok(MkgrdRefineLoopNamelistRunReport { prepare, execution })
}

/// Run the standard migrated namelist path while deriving every active source
/// branch option from the post-`read_nl` prepare state.
pub fn run_mkgrd_refine_loop_namelist_with_derived_migrated_executor_and_prepare_hook<'a, F>(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    source_grid: MkgrdRefinePrepareSourceGridOptions<'a>,
    calculated_refine: Option<(&'a [Vec<i32>], AreaJudgeSourceBounds)>,
    is_in_domain: Option<&'a [Vec<i32>]>,
    seaorland: &'a [Vec<i32>],
    landtypes_global: &'a [Vec<i32>],
    num_vertex: usize,
    maxlc: i32,
    refine_executor: MkgrdRefineLoopWorkingStateExecutor,
    postproc_options: Option<MkgrdFinalDomainPostprocOptions<'_>>,
    after_prepare: F,
) -> io::Result<MkgrdRefineLoopNamelistRunReport>
where
    F: FnOnce(&MkgrdRefineLoopPrepareReport) -> io::Result<()>,
{
    run_mkgrd_refine_loop_namelist_with_derived_migrated_executor_and_final_domain_contain_and_prepare_hook(
        namelist_source,
        workdir,
        source_grid,
        calculated_refine,
        is_in_domain,
        seaorland,
        landtypes_global,
        num_vertex,
        maxlc,
        refine_executor,
        None,
        postproc_options,
        after_prepare,
    )
}

/// Run the standard migrated namelist path while deriving every active source
/// branch option from the post-`read_nl` prepare state, and optionally generate
/// the final `Get_Contain(0)` domain containment file before post-processing.
pub fn run_mkgrd_refine_loop_namelist_with_derived_migrated_executor_and_final_domain_contain_and_prepare_hook<
    'a,
    F,
>(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    source_grid: MkgrdRefinePrepareSourceGridOptions<'a>,
    calculated_refine: Option<(&'a [Vec<i32>], AreaJudgeSourceBounds)>,
    is_in_domain: Option<&'a [Vec<i32>]>,
    seaorland: &'a [Vec<i32>],
    landtypes_global: &'a [Vec<i32>],
    num_vertex: usize,
    maxlc: i32,
    refine_executor: MkgrdRefineLoopWorkingStateExecutor,
    contain_options: Option<MkgrdFinalDomainContainOptions<'_>>,
    postproc_options: Option<MkgrdFinalDomainPostprocOptions<'_>>,
    after_prepare: F,
) -> io::Result<MkgrdRefineLoopNamelistRunReport>
where
    F: FnOnce(&MkgrdRefineLoopPrepareReport) -> io::Result<()>,
{
    let prepare =
        prepare_mkgrd_refine_loop_namelist_with_source_grid(namelist_source, workdir, source_grid)?;
    after_prepare(&prepare)?;
    let source_options = mkgrd_refine_source_branch_options_from_prepare(
        &prepare,
        source_grid,
        calculated_refine,
        is_in_domain,
        seaorland,
        landtypes_global,
        num_vertex,
        maxlc,
    )?;
    let mut executor = mkgrd_migrated_refine_loop_executor_with_runtime_state(
        source_options,
        refine_executor,
        prepare.runtime_state.clone(),
    );
    let execution = run_mkgrd_refine_loop_execution_with_final_domain_contain(
        &prepare.plan,
        &mut executor,
        contain_options,
        postproc_options,
    )?;
    Ok(MkgrdRefineLoopNamelistRunReport { prepare, execution })
}
