use std::fs;
use std::io;
use std::path::Path;

use earthmesh_core::EarthmeshConfig;

use super::{mkgrd_refine_source_branch_options_from_prepare, runtime_refine_from_prepare};
use crate::*;

/// Build the iter-zero calculated-refinement Area_judge source for a
/// data_preprocess-derived source-state handoff using the already prepared
/// mkgrd/read_nl workspace state.
pub fn data_preprocess_source_state_calculated_refine_from_prepare(
    prepare: &MkgrdRefineLoopPrepareReport,
    state: &MkgrdDataPreprocessSourceState,
) -> io::Result<Option<AreaJudgeAreaSourceReport>> {
    let runtime_refine = runtime_refine_from_prepare(prepare)?;
    if !runtime_refine.refine_cal {
        return Ok(None);
    }
    let mask_refine_ndm = prepare.workspace_mask.mask_counts.mask_refine_ndm[0];
    if mask_refine_ndm == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "data_preprocess calculated refine requires mask_refine_ndm(0) > 0",
        ));
    }
    build_area_judge_calculated_refine_fortran_indexed(
        &prepare.plan.file_dir,
        0,
        runtime_refine.mask_refine_cal_type.as_str(),
        mask_refine_ndm,
        &state.is_in_domain,
        &state.lon_vertex,
        &state.lat_vertex,
        &state.lon_i,
        &state.lat_i,
        state.gridnum_perdegree,
        state.nlons_source,
        state.nlats_source,
    )
    .map(Some)
}

/// Derive migrated refine source-branch executor options for a
/// data_preprocess-derived source-state handoff and keep any internally built
/// calculated-refine report alive for the duration of the caller callback.
pub fn with_data_preprocess_source_state_refine_source_branch_options_from_prepare<R, F>(
    prepare: &MkgrdRefineLoopPrepareReport,
    state: &MkgrdDataPreprocessSourceState,
    run: F,
) -> io::Result<R>
where
    F: FnOnce(MkgrdRefineSourceBranchExecutorOptions<'_>) -> io::Result<R>,
{
    let calculated_report =
        data_preprocess_source_state_calculated_refine_from_prepare(prepare, state)?;
    let calculated_refine = calculated_report
        .as_ref()
        .map(|report| (report.is_in_area.as_slice(), report.bounds));
    let source_options = mkgrd_refine_source_branch_options_from_prepare(
        prepare,
        state.refine_prepare_source_grid(),
        calculated_refine,
        Some(&state.is_in_domain),
        &state.seaorland,
        &state.landtypes_global,
        state.num_vertex,
        state.maxlc,
    )?;
    run(source_options)
}

/// Seed the first refine-loop gridfile and run the migrated refine execution for
/// a data_preprocess-derived source-state handoff.
pub fn run_mkgrd_refine_loop_execution_with_data_preprocess_source_state<RefineExecutor>(
    prepare: &MkgrdRefineLoopPrepareReport,
    initial_mesh: &UnstructuredMesh,
    state: &MkgrdDataPreprocessSourceState,
    mesh_type: &str,
    mask_sea_ratio: f64,
    refine_executor: RefineExecutor,
) -> io::Result<MkgrdRefineLoopExecutionReport>
where
    RefineExecutor: MkgrdRefineLoopExecutor,
{
    let first_step = prepare.plan.steps.first().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "refine plan must include at least one refine-loop step",
        )
    })?;
    write_unstructured_mesh_netcdf(&first_step.refine_loop_input_gridfile, initial_mesh)?;

    let final_domain_area_grid = prepare
        .plan
        .file_dir
        .join("tmpfile")
        .join("final_domain_area_grid_from_landtype.nc4");
    with_data_preprocess_source_state_refine_source_branch_options_from_prepare(
        prepare,
        state,
        |source_options| {
            let mut executor = MkgrdCompositeRefineLoopExecutor::new(
                MkgrdRefineSourceBranchExecutor::new(source_options)
                    .with_runtime_state(prepare.runtime_state.clone()),
                refine_executor,
            );
            run_mkgrd_refine_loop_execution_with_data_preprocess_final_domain_handoff(
                &prepare.plan,
                &mut executor,
                state,
                mesh_type,
                &final_domain_area_grid,
                mask_sea_ratio,
                &prepare.config.output_format,
            )
        },
    )
}

/// Run the migrated direct landtype-source namelist path without CLI-side
/// orchestration.
///
/// This composes parsed mkgrd config -> data_preprocess source state ->
/// gridinit -> refine prepare -> migrated refine/final handoff, so command-line
/// front-ends only need to format the returned report.
pub fn run_mkgrd_refine_landtype_source_namelist<RefineExecutor>(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    max_tris: usize,
    source_gridnum_perdegree: Option<usize>,
    source_first_triangle_id: usize,
    refine_executor: RefineExecutor,
) -> io::Result<MkgrdRefineLandtypeSourceNamelistRunReport>
where
    RefineExecutor: MkgrdRefineLoopExecutor,
{
    let namelist_source = namelist_source.as_ref();
    let workdir = workdir.as_ref();
    let contents = fs::read_to_string(namelist_source)?;
    let config = EarthmeshConfig::from_mkgrd_namelist(&contents)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let source_state = build_mkgrd_data_preprocess_source_state_from_config_fortran_indexed(
        workdir,
        &config,
        source_gridnum_perdegree,
        source_first_triangle_id,
    )?;

    let gridinit = run_mkgrd_gridinit_global_namelist(namelist_source, workdir, max_tris)?;
    if !gridinit.config.refine {
        return Ok(MkgrdRefineLandtypeSourceNamelistRunReport {
            source_state,
            gridinit,
            refine: None,
        });
    }

    let initial_mesh = read_unstructured_mesh_netcdf(&gridinit.gridfile.output)?;
    let source_grid = source_state.refine_prepare_source_grid();
    let mut prepare =
        prepare_mkgrd_refine_loop_namelist_with_source_grid(namelist_source, workdir, source_grid)?;
    prepare.runtime_state.source_grid.maxlc =
        usize::try_from(source_state.maxlc).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "data_preprocess maxlc must be non-negative for runtime state, got {}",
                    source_state.maxlc
                ),
            )
        })?;
    let execution = run_mkgrd_refine_loop_execution_with_data_preprocess_source_state(
        &prepare,
        &initial_mesh,
        &source_state,
        config.mesh_type.trim(),
        0.0,
        refine_executor,
    )?;

    Ok(MkgrdRefineLandtypeSourceNamelistRunReport {
        source_state,
        gridinit,
        refine: Some(MkgrdRefineLoopNamelistRunReport { prepare, execution }),
    })
}

/// Run the migrated direct compact source-state namelist path without CLI-side
/// orchestration.
///
/// This centralizes compact source-state parsing, source-axis reconstruction,
/// calculated-refine metadata wiring, final-domain contain/postprocess option
/// construction, and the final-domain Area_judge payload write hook before the
/// migrated top-level refine runner executes.
pub fn run_mkgrd_refine_compact_source_state_namelist(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    source_state_path: impl AsRef<Path>,
    max_tris: usize,
    refine_executor: MkgrdRefineLoopWorkingStateExecutor,
) -> io::Result<MkgrdRefineCompactSourceStateNamelistRunReport> {
    let namelist_source = namelist_source.as_ref();
    let workdir = workdir.as_ref();
    let source_state_path = source_state_path.as_ref();
    let contents = fs::read_to_string(namelist_source)?;
    let config = EarthmeshConfig::from_mkgrd_namelist(&contents)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let source_state = read_mkgrd_compact_source_state(source_state_path)?;
    let axes = source_state.build_global_source_axes()?;
    let source_grid = axes.refine_prepare_source_grid(source_state.first_triangle_id);
    let calculated_refine = source_state
        .calculated_refine
        .as_deref()
        .map(|calculated_refine| {
            (
                calculated_refine,
                source_state
                    .calculated_bounds
                    .expect("validated calculated bounds"),
            )
        });
    let final_domain_area_grid = source_state_path.with_extension("final_domain_area_grid.nc4");
    let final_domain_area_payload =
        compact_source_state_final_domain_area_payload_fortran_indexed(&source_state, &axes)?;
    let contain_options =
        compact_source_state_final_contain_options(&source_state, &axes, &final_domain_area_grid);
    let final_postproc_request = compact_source_state_final_postproc_request(&source_state)?;
    let postproc_options = match &final_postproc_request {
        None => None,
        Some(MkgrdCompactSourceStateFinalPostprocRequest::Ocean { num_vertex }) => Some(
            MkgrdFinalDomainPostprocOptions::Ocean(MaskPostprocOceanRunOptions {
                mask_sea_ratio: 0.0,
                num_vertex: *num_vertex,
            }),
        ),
        Some(MkgrdCompactSourceStateFinalPostprocRequest::Atmos) => {
            Some(MkgrdFinalDomainPostprocOptions::Atmos {
                output_format: config.output_format.trim(),
            })
        }
        Some(MkgrdCompactSourceStateFinalPostprocRequest::Earth(context)) => {
            Some(MkgrdFinalDomainPostprocOptions::EarthFromFinalGrid(
                MkgrdFinalDomainEarthAutoPostprocOptions {
                    mask_sea_ratio: config.mask_sea_ratio,
                    minlon_dm_area: context.minlon_dm_area,
                    maxlat_dm_area: context.maxlat_dm_area,
                    nlons_dm_select: context.nlons_dm_select,
                    nlats_dm_select: context.nlats_dm_select,
                    lon_vertex: &axes.lon_vertex,
                    lat_vertex: &axes.lat_vertex,
                    lon_i: &axes.lon_i,
                    lat_i: &axes.lat_i,
                },
            ))
        }
        Some(MkgrdCompactSourceStateFinalPostprocRequest::Land(context)) => Some(
            MkgrdFinalDomainPostprocOptions::Land(MaskPostprocLandRunOptions {
                seaorland: &context.selected_seaorland,
                minlon_dm_area: context.minlon_dm_area,
                maxlat_dm_area: context.maxlat_dm_area,
                nlons_dm_select: context.nlons_dm_select,
                nlats_dm_select: context.nlats_dm_select,
                lon_vertex: &axes.lon_vertex,
                lat_vertex: &axes.lat_vertex,
                lon_i: &axes.lon_i,
                lat_i: &axes.lat_i,
            }),
        ),
    };
    let report = run_mkgrd_top_level_namelist_with_derived_migrated_executor_and_source_grid_and_final_domain_contain_and_prepare_hook(
        namelist_source,
        workdir,
        max_tris,
        source_grid,
        calculated_refine,
        Some(&source_state.is_in_domain),
        &source_state.seaorland,
        &source_state.landtypes_global,
        source_state.num_vertex,
        source_state.maxlc,
        refine_executor,
        contain_options,
        postproc_options,
        |_prepare| {
            if let Some(payload) = final_domain_area_payload.as_ref() {
                write_area_judge_grid_netcdf(&final_domain_area_grid, payload)?;
            }
            Ok(())
        },
    )?;

    Ok(MkgrdRefineCompactSourceStateNamelistRunReport {
        source_state,
        gridinit: report.gridinit,
        refine: report.refine,
    })
}
