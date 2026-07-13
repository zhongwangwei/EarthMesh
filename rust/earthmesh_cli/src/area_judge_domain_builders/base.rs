use crate::AreaJudgeBaseStateReport;
use std::io;
use std::path::Path;

use super::domain::build_area_judge_domain_one_based;
use super::seaorland::build_area_judge_seaorland_one_based;

/// Compose the non-restart `Area_judge` base state before patches and refine masks.
pub fn build_area_judge_base_state_one_based(
    file_dir: impl AsRef<Path>,
    mask_domain_global: bool,
    mask_domain_type: &str,
    mask_domain_ndm: usize,
    landtypes_global: &[Vec<i32>],
    mesh_type: &str,
    refine: bool,
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    lon_i: &[f64],
    lat_i: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<AreaJudgeBaseStateReport> {
    let domain = build_area_judge_domain_one_based(
        file_dir,
        mask_domain_global,
        mask_domain_type,
        mask_domain_ndm,
        lon_vertex,
        lat_vertex,
        lon_i,
        lat_i,
        gridnum_perdegree,
        nlons_source,
        nlats_source,
    )?;
    let seaorland = build_area_judge_seaorland_one_based(
        &domain.is_in_domain,
        landtypes_global,
        domain.bounds,
        mesh_type,
        refine,
    )?;

    Ok(AreaJudgeBaseStateReport { domain, seaorland })
}
