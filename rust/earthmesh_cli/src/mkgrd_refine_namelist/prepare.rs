use std::fs;
use std::io;
use std::path::Path;

use earthmesh_core::{
    EarthmeshConfig, EarthmeshRuntimeState, MaskCounterState, RefineConfig, SourceGridState,
};

use crate::*;

/// Prepare the migrated refine-loop path from one namelist file.
///
/// This executes the Rust `read_nl` workspace/mask side effects, builds the
/// refine-loop I/O plan, and enriches the final regional spring source mask
/// from the Fortran `mask_patch_ndm(1)` convention when that final branch is
/// active.
pub fn prepare_mkgrd_refine_loop_namelist(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    lon_i: &[f64],
    lat_i: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
    first_triangle_id: usize,
) -> io::Result<MkgrdRefineLoopPrepareReport> {
    prepare_mkgrd_refine_loop_namelist_with_source_grid(
        namelist_source,
        workdir,
        MkgrdRefinePrepareSourceGridOptions {
            lon_vertex,
            lat_vertex,
            lon_i,
            lat_i,
            gridnum_perdegree,
            nlons_source,
            nlats_source,
            first_triangle_id,
        },
    )
}

/// Prepare the migrated refine-loop path from one namelist file using a typed
/// source-grid options bundle.
pub fn prepare_mkgrd_refine_loop_namelist_with_source_grid(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    source_grid: MkgrdRefinePrepareSourceGridOptions<'_>,
) -> io::Result<MkgrdRefineLoopPrepareReport> {
    let namelist_source = namelist_source.as_ref();
    let workdir = workdir.as_ref();
    let contents = fs::read_to_string(namelist_source)?;
    let config = EarthmeshConfig::from_mkgrd_namelist(&contents)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    if !config.refine {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "refine-loop preparation requires NL%refine=.true.",
        ));
    }
    let refine = RefineConfig::from_mkrefine_namelist(
        &contents,
        config.mesh_type.trim(),
        config.mode_grid.trim(),
    )
    .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let mut runtime_state =
        EarthmeshRuntimeState::new(config.clone()).with_refine_config(refine.clone());
    runtime_state
        .try_nxp()
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    runtime_state.source_grid = SourceGridState {
        nlons_source: source_grid.nlons_source,
        nlats_source: source_grid.nlats_source,
        maxlc: 0,
    };
    let workspace_plan = config.read_nl_workspace_plan(Some(&refine));
    let max_iter_spc = final_quality_non_negative_usize(
        refine.max_iter_spc,
        "refine-loop preparation max_iter_spc must be non-negative",
    )?;
    let validate_refine_max_iter = refine.refine_spc && !config.mask_restart;
    let workspace_mask = apply_workspace_and_mask_operations(
        &workspace_plan,
        namelist_source,
        workdir,
        max_iter_spc,
        validate_refine_max_iter,
    )?;
    runtime_state.mask_counts = MaskCounterState {
        mask_domain_ndm: workspace_mask.mask_counts.mask_domain_ndm,
        mask_refine_ndm: workspace_mask.mask_counts.mask_refine_ndm,
        mask_patch_ndm: workspace_mask.mask_counts.mask_patch_ndm,
    };
    let mut plan = plan_mkgrd_refine_loop_io(&config, &refine)?;
    let runtime_state = runtime_state.with_step(plan.final_mask_postproc_step);
    let final_source_mask_injected =
        if plan.final_quality_check.spring_mode == MkgrdFinalQualitySpringMode::RegionalFinal {
            enrich_mkgrd_refine_loop_final_quality_with_regional_source_mask_io(
                &mut plan,
                &config.mask_patch_type,
                1,
                workspace_mask.mask_counts.mask_patch_ndm[1],
                source_grid.lon_vertex,
                source_grid.lat_vertex,
                source_grid.lon_i,
                source_grid.lat_i,
                source_grid.gridnum_perdegree,
                source_grid.nlons_source,
                source_grid.nlats_source,
                source_grid.first_triangle_id,
            )?
        } else {
            false
        };

    Ok(MkgrdRefineLoopPrepareReport {
        config,
        refine,
        runtime_state,
        workspace_mask,
        plan,
        final_source_mask_injected,
    })
}

/// Compatibility entry for the former global-source refine smoke path.
///
/// The source-grid dimension arguments are retained for older callers, but
/// refinement now always dispatches through the direct OLAM path.
pub fn run_mkgrd_refine_passthrough_global_source_namelist(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    max_tris: usize,
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
    first_triangle_id: usize,
) -> io::Result<MkgrdOlamSpecifiedRefineRunReport> {
    let _ = (nlons_source, nlats_source, first_triangle_id);
    run_mkgrd_olam_specified_refine_global_source_namelist(
        namelist_source,
        workdir,
        max_tris,
        Some(gridnum_perdegree),
    )
}

/// Execute a real specified-refinement top-level path using synthetic global
/// source geometry when no land-type source file is available.
pub fn run_mkgrd_atmos_specified_refine_global_source_namelist(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    max_tris: usize,
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
    first_triangle_id: usize,
) -> io::Result<MkgrdOlamSpecifiedRefineRunReport> {
    let _ = (nlons_source, nlats_source, first_triangle_id);
    run_mkgrd_olam_specified_refine_global_source_namelist(
        namelist_source,
        workdir,
        max_tris,
        Some(gridnum_perdegree),
    )
}
