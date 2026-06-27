use std::io;
use std::path::Path;

use earthmesh_mesh::AreaJudgeSourceBounds;

use super::runtime_refine_from_prepare;
use crate::*;

/// Derive the specified-refinement source-branch options from a prepared
/// namelist/workspace report and caller-provided source classification state.
///
/// This removes duplicated Fortran module-state glue from adapters: file_dir,
/// mesh_type, mask_refine_spc_type, and mask_refine_ndm come from the prepared
/// Rust namelist/workspace state rather than being hand-copied by each caller.
pub fn mkgrd_specified_refine_source_options_from_prepare<'a>(
    prepare: &'a MkgrdRefineLoopPrepareReport,
    source_grid: MkgrdRefinePrepareSourceGridOptions<'a>,
    is_in_domain: &'a [Vec<i32>],
    seaorland: &'a [Vec<i32>],
    num_vertex: usize,
) -> io::Result<MkgrdSpecifiedRefineSourceExecutorOptions<'a>> {
    let runtime_refine = runtime_refine_from_prepare(prepare)?;
    if !runtime_refine.refine_spc {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "specified refine source options require RL%refine_spc=.true.",
        ));
    }
    let max_iter_spc = final_quality_non_negative_usize(
        runtime_refine.max_iter_spc,
        "specified refine max_iter_spc must be non-negative",
    )?;
    if max_iter_spc == 0 || max_iter_spc >= prepare.workspace_mask.mask_counts.mask_refine_ndm.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "specified refine max_iter_spc must fit mask_refine_ndm 1:9",
        ));
    }
    let mask_refine_ndm = prepare.workspace_mask.mask_counts.mask_refine_ndm[max_iter_spc];
    if mask_refine_ndm == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("mask_refine_ndm({max_iter_spc}) must be larger than zero"),
        ));
    }

    Ok(MkgrdSpecifiedRefineSourceExecutorOptions {
        file_dir: &prepare.plan.file_dir,
        mesh_type: &prepare.plan.mesh_type,
        mask_refine_spc_type: &runtime_refine.mask_refine_spc_type,
        mask_refine_ndm,
        mask_refine_ndm_by_iter: &prepare.workspace_mask.mask_counts.mask_refine_ndm,
        is_in_domain,
        seaorland,
        lon_vertex: source_grid.lon_vertex,
        lat_vertex: source_grid.lat_vertex,
        lon_i: source_grid.lon_i,
        lat_i: source_grid.lat_i,
        gridnum_perdegree: source_grid.gridnum_perdegree,
        nlons_source: source_grid.nlons_source,
        nlats_source: source_grid.nlats_source,
        num_vertex,
    })
}

/// Derive the calculated-refinement source-branch options from a prepared
/// namelist/workspace report and caller-provided source classification state.
///
/// This keeps adapter glue flat: the `read_nl`-owned flags, thresholds,
/// mesh/file paths, and GetRef component configs come from one prepared Rust
/// state, while large source rasters still remain caller-owned inputs.
pub fn mkgrd_calculated_refine_source_options_from_prepare<'a>(
    prepare: &'a MkgrdRefineLoopPrepareReport,
    source_grid: MkgrdRefinePrepareSourceGridOptions<'a>,
    calculated_refine: (&'a [Vec<i32>], AreaJudgeSourceBounds),
    seaorland: &'a [Vec<i32>],
    landtypes_global: &'a [Vec<i32>],
    num_vertex: usize,
    maxlc: i32,
) -> io::Result<MkgrdCalculatedRefineSourceExecutorOptions<'a>> {
    let runtime_refine = runtime_refine_from_prepare(prepare)?;
    if !runtime_refine.refine_cal {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "calculated refine source options require RL%refine_cal=.true.",
        ));
    }
    let max_iter_cal = final_quality_non_negative_usize(
        runtime_refine.max_iter_cal,
        "calculated refine max_iter_cal must be non-negative",
    )?;
    if max_iter_cal == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "calculated refine max_iter_cal must be larger than zero",
        ));
    }

    Ok(MkgrdCalculatedRefineSourceExecutorOptions {
        file_dir: &prepare.plan.file_dir,
        mesh_type: &prepare.plan.mesh_type,
        threshold_dir: Path::new(runtime_refine.threshold_dir.as_str()),
        calculated_refine,
        seaorland,
        lon_vertex: source_grid.lon_vertex,
        lat_vertex: source_grid.lat_vertex,
        lon_i: source_grid.lon_i,
        lat_i: source_grid.lat_i,
        num_vertex,
        landtypes_global,
        refine_onelayer_lnd: &runtime_refine.refine_onelayer_lnd,
        th_onelayer_lnd: &runtime_refine.th_onelayer_lnd,
        refine_twolayer_lnd: &runtime_refine.refine_twolayer_lnd,
        th_twolayer_lnd: &runtime_refine.th_twolayer_lnd,
        refine_onelayer_ocn: &runtime_refine.refine_onelayer_ocn,
        th_onelayer_ocn: &runtime_refine.th_onelayer_ocn,
        refine_onelayer_atmos: &runtime_refine.refine_onelayer_atmos,
        th_onelayer_atmos: &runtime_refine.th_onelayer_atmos,
        land_basic_config: GetRefLandBasicConfig {
            num_vertex,
            maxlc,
            refine_num_landtypes: runtime_refine.refine_num_landtypes,
            th_num_landtypes: runtime_refine.th_num_landtypes,
            refine_area_mainland: runtime_refine.refine_area_mainland,
            th_area_mainland: runtime_refine.th_area_mainland,
        },
        ocean_config: GetRefOceanThresholdConfig {
            num_vertex,
            maxlc,
            refine_sea_ratio: runtime_refine.refine_sea_ratio,
            th_sea_ratio: runtime_refine.th_sea_ratio,
        },
        atmos_config: GetRefAtmosThresholdConfig { num_vertex, maxlc },
    })
}

/// Derive all enabled migrated refine source-branch executors from a prepared
/// namelist/workspace report.
///
/// This is the adapter-facing replacement for carrying Fortran module state
/// across the `read_nl -> Area_judge_refine/Get_Contain/GetRef` boundary: the
/// prepared report decides which branches are active, and callers provide only
/// the large source-grid classification arrays still owned outside `mkgrd`.
pub fn mkgrd_refine_source_branch_options_from_prepare<'a>(
    prepare: &'a MkgrdRefineLoopPrepareReport,
    source_grid: MkgrdRefinePrepareSourceGridOptions<'a>,
    calculated_refine: Option<(&'a [Vec<i32>], AreaJudgeSourceBounds)>,
    is_in_domain: Option<&'a [Vec<i32>]>,
    seaorland: &'a [Vec<i32>],
    landtypes_global: &'a [Vec<i32>],
    num_vertex: usize,
    maxlc: i32,
) -> io::Result<MkgrdRefineSourceBranchExecutorOptions<'a>> {
    let runtime_refine = runtime_refine_from_prepare(prepare)?;
    let calculated = if runtime_refine.refine_cal {
        Some(mkgrd_calculated_refine_source_options_from_prepare(
            prepare,
            source_grid,
            calculated_refine.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "calculated refine branch is active but no calculated_refine state was provided",
                )
            })?,
            seaorland,
            landtypes_global,
            num_vertex,
            maxlc,
        )?)
    } else {
        None
    };
    let specified = if runtime_refine.refine_spc {
        Some(mkgrd_specified_refine_source_options_from_prepare(
            prepare,
            source_grid,
            is_in_domain.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "specified refine branch is active but no is_in_domain state was provided",
                )
            })?,
            seaorland,
            num_vertex,
        )?)
    } else {
        None
    };

    Ok(MkgrdRefineSourceBranchExecutorOptions {
        calculated,
        specified,
    })
}
