use std::io;
use std::path::Path;

use crate::*;

/// Compose the restart `MOD_Area_judge.F90:Area_judge` branch from a saved domain grid.
pub fn build_area_judge_restart_fortran_indexed(
    file_dir: impl AsRef<Path>,
    restart_input: impl AsRef<Path>,
    mask_patch: Option<AreaJudgePatchConfig<'_>>,
    refine: bool,
    calculated_refine: Option<AreaJudgeCalculatedRefineConfig<'_>>,
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    lon_i: &[f64],
    lat_i: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<AreaJudgeRestartReport> {
    let restart = run_area_judge_restart_grid_fortran_indexed(AreaJudgeRestartGridRunConfig {
        input: restart_input.as_ref(),
        nlons_source,
        nlats_source,
    })?;
    let bounds = restart.expanded.bounds;
    let numpatch = (bounds.minlon_source..=bounds.maxlon_source)
        .flat_map(|lon_index| {
            (bounds.maxlat_source..=bounds.minlat_source)
                .map(move |lat_index| (lon_index, lat_index))
        })
        .filter(|(lon_index, lat_index)| restart.expanded.is_in_domain[*lon_index][*lat_index] != 0)
        .count();
    let domain = AreaJudgeDomainInitializationReport {
        is_in_domain: restart.expanded.is_in_domain,
        bounds,
        numpatch,
        nlons_select: restart.expanded.nlons_select,
        nlats_select: restart.expanded.nlats_select,
    };
    let mut seaorland = restart.expanded.seaorland;
    let sum_land_grid = seaorland
        .iter()
        .flat_map(|row| row.iter())
        .copied()
        .sum::<i32>();

    let patch = mask_patch
        .map(|config| {
            apply_area_judge_patch_sources_fortran_indexed(
                &file_dir,
                config.mask_patch_type,
                0,
                config.mask_patch_ndm,
                &mut seaorland,
                lon_vertex,
                lat_vertex,
                lon_i,
                lat_i,
                gridnum_perdegree,
                nlons_source,
                nlats_source,
            )
        })
        .transpose()?;

    let calculated_refine = match (refine, calculated_refine) {
        (true, Some(config)) if config.refine_setting != "specified" => {
            Some(build_area_judge_calculated_refine_fortran_indexed(
                &file_dir,
                0,
                config.mask_refine_cal_type,
                config.mask_refine_ndm,
                &domain.is_in_domain,
                lon_vertex,
                lat_vertex,
                lon_i,
                lat_i,
                gridnum_perdegree,
                nlons_source,
                nlats_source,
            )?)
        }
        _ => None,
    };

    Ok(AreaJudgeRestartReport {
        domain,
        seaorland: AreaJudgeSeaOrLandReport {
            seaorland,
            sum_land_grid,
        },
        patch,
        calculated_refine,
    })
}
