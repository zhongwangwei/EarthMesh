use std::io;
use std::path::Path;

use crate::*;

/// Compose the non-restart `MOD_Area_judge.F90:Area_judge` branch.
pub fn build_area_judge_non_restart_fortran_indexed(
    file_dir: impl AsRef<Path>,
    mask_domain_global: bool,
    mask_domain_type: &str,
    mask_domain_ndm: usize,
    mask_patch: Option<AreaJudgePatchConfig<'_>>,
    refine: bool,
    calculated_refine: Option<AreaJudgeCalculatedRefineConfig<'_>>,
    landtypes_global: &[Vec<i32>],
    mesh_type: &str,
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    lon_i: &[f64],
    lat_i: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<AreaJudgeNonRestartReport> {
    let mut base = build_area_judge_base_state_fortran_indexed(
        &file_dir,
        mask_domain_global,
        mask_domain_type,
        mask_domain_ndm,
        landtypes_global,
        mesh_type,
        refine,
        lon_vertex,
        lat_vertex,
        lon_i,
        lat_i,
        gridnum_perdegree,
        nlons_source,
        nlats_source,
    )?;

    let patch = mask_patch
        .map(|config| {
            apply_area_judge_patch_sources_fortran_indexed(
                &file_dir,
                config.mask_patch_type,
                0,
                config.mask_patch_ndm,
                &mut base.seaorland.seaorland,
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
                &base.domain.is_in_domain,
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

    Ok(AreaJudgeNonRestartReport {
        domain: base.domain,
        seaorland: base.seaorland,
        patch,
        calculated_refine,
    })
}
