use std::io;
use std::path::Path;

use earthmesh_mesh::AreaJudgeSourceBounds;

use crate::*;

/// Run the migrated top-level `mkgrd.x` namelist path with the standard
/// migrated refine executor stack.
///
/// This combines the top-level gridinit/read_nl cleanup preservation with the
/// prepared-state source-branch option derivation, so adapters can enter the
/// Rust path without manually reconstructing Fortran module state for
/// `Area_judge_refine`, `Get_Contain`, and `GetRef`.
pub fn run_mkgrd_top_level_namelist_with_derived_migrated_executor_and_source_grid<'a>(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    max_tris: usize,
    source_grid: MkgrdRefinePrepareSourceGridOptions<'a>,
    calculated_refine: Option<(&'a [Vec<i32>], AreaJudgeSourceBounds)>,
    is_in_domain: Option<&'a [Vec<i32>]>,
    seaorland: &'a [Vec<i32>],
    landtypes_global: &'a [Vec<i32>],
    num_vertex: usize,
    maxlc: i32,
    refine_executor: MkgrdRefineLoopWorkingStateExecutor,
    postproc_options: Option<MkgrdFinalDomainPostprocOptions<'_>>,
) -> io::Result<MkgrdTopLevelNamelistRunReport> {
    run_mkgrd_top_level_namelist_with_derived_migrated_executor_and_source_grid_and_final_domain_contain_and_prepare_hook(
        namelist_source,
        workdir,
        max_tris,
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
        |_| Ok(()),
    )
}

/// Run the migrated top-level `mkgrd.x` namelist path with the standard
/// migrated refine executor stack and optional final `Get_Contain(0)` domain
/// generation.
///
/// The hook runs after `read_nl` cleanup and after the initial gridinit mesh has
/// been restored to the first refine input, so callers can provide file-backed
/// final-domain inputs without racing the workspace recreation.
pub fn run_mkgrd_top_level_namelist_with_derived_migrated_executor_and_source_grid_and_final_domain_contain_and_prepare_hook<
    'a,
    F,
>(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    max_tris: usize,
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
) -> io::Result<MkgrdTopLevelNamelistRunReport>
where
    F: FnOnce(&MkgrdRefineLoopPrepareReport) -> io::Result<()>,
{
    let namelist_source = namelist_source.as_ref();
    let workdir = workdir.as_ref();
    let gridinit = run_mkgrd_gridinit_global_namelist(namelist_source, workdir, max_tris)?;
    if !gridinit.config.refine {
        return Ok(MkgrdTopLevelNamelistRunReport {
            gridinit,
            refine: None,
        });
    }

    let initial_mesh = read_unstructured_mesh_netcdf(&gridinit.gridfile.output)?;
    let prepare =
        prepare_mkgrd_refine_loop_namelist_with_source_grid(namelist_source, workdir, source_grid)?;
    let first_step = prepare.plan.steps.first().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "refine plan must include at least one refine-loop step",
        )
    })?;
    write_unstructured_mesh_netcdf(&first_step.refine_loop_input_gridfile, &initial_mesh)?;
    after_prepare(&prepare)?;

    let execution = {
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
        run_mkgrd_refine_loop_execution_with_final_domain_contain(
            &prepare.plan,
            &mut executor,
            contain_options,
            postproc_options,
        )?
    };

    Ok(MkgrdTopLevelNamelistRunReport {
        gridinit,
        refine: Some(MkgrdRefineLoopNamelistRunReport { prepare, execution }),
    })
}
