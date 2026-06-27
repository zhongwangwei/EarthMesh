use std::io;
use std::path::Path;

use earthmesh_core::EarthmeshRuntimeState;

use crate::*;

/// Build the source-mask classification input required by the final regional
/// spring branch from the same numbered `mask_patch_*` sources used by the
/// legacy area-judge path.
pub fn build_mkgrd_final_quality_regional_source_mask_io(
    file_dir: impl AsRef<Path>,
    mask_patch_type: &str,
    iter: usize,
    mask_patch_ndm: usize,
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    lon_i: &[f64],
    lat_i: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
    first_triangle_id: usize,
) -> io::Result<MkgrdFinalQualityRegionalSourceMaskIoPlan> {
    let source = build_area_judge_area_sources_fortran_indexed(
        file_dir,
        "mask_patch",
        mask_patch_type,
        iter,
        mask_patch_ndm,
        lon_vertex,
        lat_vertex,
        lon_i,
        lat_i,
        gridnum_perdegree,
        nlons_source,
        nlats_source,
    )?;

    Ok(MkgrdFinalQualityRegionalSourceMaskIoPlan {
        source_lon_vertices: lon_vertex.to_vec(),
        source_lat_vertices: lat_vertex.to_vec(),
        mask_patch: bool_matrix_from_i32_area_mask(source.is_in_area),
        first_triangle_id,
    })
}

/// Attach a built final-regional source mask to an existing final quality plan.
///
/// Returns `true` when this call injected a new mask. Non-running, non-regional,
/// or already-enriched plans are left unchanged and return `false`.
pub fn enrich_mkgrd_final_quality_with_regional_source_mask_io(
    plan: &mut MkgrdFinalQualityCheckIoPlan,
    file_dir: impl AsRef<Path>,
    mask_patch_type: &str,
    iter: usize,
    mask_patch_ndm: usize,
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    lon_i: &[f64],
    lat_i: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
    first_triangle_id: usize,
) -> io::Result<bool> {
    if !plan.run_quality_check
        || plan.spring_mode != MkgrdFinalQualitySpringMode::RegionalFinal
        || plan.regional_source_mask.is_some()
    {
        return Ok(false);
    }

    plan.regional_source_mask = Some(build_mkgrd_final_quality_regional_source_mask_io(
        file_dir,
        mask_patch_type,
        iter,
        mask_patch_ndm,
        lon_vertex,
        lat_vertex,
        lon_i,
        lat_i,
        gridnum_perdegree,
        nlons_source,
        nlats_source,
        first_triangle_id,
    )?);
    Ok(true)
}

/// Attach a final-regional source mask to the final quality stage of a
/// top-level refine-loop I/O plan, using the refine-loop plan's `file_dir` as
/// the source for numbered legacy `mask_patch_*` files.
pub fn enrich_mkgrd_refine_loop_final_quality_with_regional_source_mask_io(
    plan: &mut MkgrdRefineLoopIoPlan,
    mask_patch_type: &str,
    iter: usize,
    mask_patch_ndm: usize,
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    lon_i: &[f64],
    lat_i: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
    first_triangle_id: usize,
) -> io::Result<bool> {
    let file_dir = plan.file_dir.clone();
    enrich_mkgrd_final_quality_with_regional_source_mask_io(
        &mut plan.final_quality_check,
        file_dir,
        mask_patch_type,
        iter,
        mask_patch_ndm,
        lon_vertex,
        lat_vertex,
        lon_i,
        lat_i,
        gridnum_perdegree,
        nlons_source,
        nlats_source,
        first_triangle_id,
    )
}

/// Attach final-global spring distance masks by re-running the same source-mask
/// classification that Fortran performs inside `set_distsOnEdge_global`.
pub fn enrich_mkgrd_final_quality_with_global_distance_steps_io(
    plan: &mut MkgrdFinalQualityCheckIoPlan,
    runtime_state: &EarthmeshRuntimeState,
    max_iter: usize,
) -> io::Result<bool> {
    if !plan.run_quality_check
        || plan.spring_mode != MkgrdFinalQualitySpringMode::Global
        || plan
            .global_spring
            .as_ref()
            .is_some_and(|spring| !spring.distance_steps.is_empty())
    {
        return Ok(false);
    }

    let refine = runtime_state.refine.as_ref().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "final global spring distance steps require runtime refine config",
        )
    })?;
    let gridnum_perdegree = source_gridnum_perdegree_from_dims(
        runtime_state.source_grid.nlons_source,
        runtime_state.source_grid.nlats_source,
    )
    .or_else(|_| {
        usize::try_from(runtime_state.config.gridnum_perdegree).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "final global spring requires positive gridnum_perdegree",
            )
        })
    })?;
    let axes = build_global_source_axes_fortran_indexed(
        gridnum_perdegree,
        runtime_state.source_grid.nlons_source,
        runtime_state.source_grid.nlats_source,
    )?;
    let mesh = normalize_unstructured_mesh_legacy_placeholders(&read_unstructured_mesh_netcdf(
        &plan.input_gridfile,
    )?)?;
    let triangle_lonlat = lonlat_degrees_from_points(&mesh.m_points);

    let mut distance_steps = Vec::new();
    for iter in 1..=max_iter {
        if iter > plan.step {
            continue;
        }
        if refine.exit_loop_step.get(iter).copied().unwrap_or(false) {
            continue;
        }
        let mut mask_patch_ndm = *runtime_state
            .mask_counts
            .mask_patch_ndm
            .get(iter)
            .unwrap_or(&0);
        if mask_patch_ndm == 0 {
            mask_patch_ndm = count_area_judge_area_sources(
                runtime_state.config.file_dir(),
                "mask_patch",
                &runtime_state.config.mask_patch_type,
                iter,
            );
        }
        if mask_patch_ndm == 0 {
            continue;
        }
        let halo = usize::try_from(*refine.halo.get(iter).unwrap_or(&0)).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("final global spring halo({iter}) must be non-negative"),
            )
        })?;
        let source = build_area_judge_area_sources_fortran_indexed(
            runtime_state.config.file_dir(),
            "mask_patch",
            &runtime_state.config.mask_patch_type,
            iter,
            mask_patch_ndm,
            &axes.lon_vertex,
            &axes.lat_vertex,
            &axes.lon_i,
            &axes.lat_i,
            axes.gridnum_perdegree,
            axes.nlons_source,
            axes.nlats_source,
        )?;
        let refinement_flags = earthmesh_mesh::refine_sjx_regional_make_fortran_indexed(
            earthmesh_mesh::RefineRegionalMaskInput {
                triangle_lonlat: &triangle_lonlat,
                source_lon_vertices: &axes.lon_vertex,
                source_lat_vertices: &axes.lat_vertex,
                mask_patch: &bool_matrix_from_i32_area_mask(source.is_in_area),
                first_triangle_id: runtime_state.num_mp_step[iter - 1],
            },
        )
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to classify final global spring refinement flags for iter {iter}"),
            )
        })?;
        if std::env::var_os("EARTHMESH_FINAL_GLOBAL_DEBUG").is_some() {
            let refined_count = refinement_flags.iter().filter(|flag| **flag).count();
            eprintln!(
                "final_global_distance_step iter={iter} ndm={mask_patch_ndm} halo={halo} refined={refined_count} first_triangle={} first_cell={}",
                runtime_state.num_mp_step[iter - 1],
                runtime_state.num_wp_step[iter - 1]
            );
        }
        distance_steps.push(MkgrdFinalQualityGlobalDistanceStepIoPlan {
            active: true,
            halo,
            refinement_flags,
            num_vertex_in: runtime_state.num_mp_step[iter - 1],
            num_center_in: runtime_state.num_wp_step[iter - 1],
        });
    }

    if let Some(global_spring) = plan.global_spring.as_mut() {
        global_spring.distance_steps = distance_steps;
        Ok(true)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Final_Grid_Quality_Check global spring requires global_spring controls",
        ))
    }
}

fn source_gridnum_perdegree_from_dims(
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<usize> {
    if nlons_source == 0 || nlats_source == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "final global spring requires nonzero source-grid dimensions",
        ));
    }
    if !nlons_source.is_multiple_of(360) || !nlats_source.is_multiple_of(180) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "final global spring source-grid dimensions must be global, got {nlons_source}x{nlats_source}"
            ),
        ));
    }
    let lon_gridnum = nlons_source / 360;
    let lat_gridnum = nlats_source / 180;
    if lon_gridnum == 0 || lon_gridnum != lat_gridnum {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "final global spring source-grid longitude/latitude resolutions differ: {lon_gridnum} vs {lat_gridnum}"
            ),
        ));
    }
    Ok(lon_gridnum)
}

fn count_area_judge_area_sources(
    file_dir: impl AsRef<Path>,
    type_select: &str,
    mask_type: &str,
    iter: usize,
) -> usize {
    let file_dir = file_dir.as_ref();
    let mut count = 0usize;
    for source_index in 1.. {
        let Ok(path) =
            area_judge_area_source_path(file_dir, type_select, mask_type, iter, source_index)
        else {
            break;
        };
        if path.exists() {
            count = source_index;
        } else {
            break;
        }
    }
    count
}

fn bool_matrix_from_i32_area_mask(mask: Vec<Vec<i32>>) -> Vec<Vec<bool>> {
    mask.into_iter()
        .map(|row| row.into_iter().map(|value| value != 0).collect())
        .collect()
}
