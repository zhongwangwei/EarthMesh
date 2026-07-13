use crate::build_area_judge_area_sources_one_based;
use crate::AreaJudgeAreaSourceReport;
use std::io;
use std::path::Path;

use super::validation::validate_area_judge_refine_within_domain_one_based;

/// Build calculated `mask_refine` sources and validate they stay inside domain.
pub fn build_area_judge_calculated_refine_one_based(
    file_dir: impl AsRef<Path>,
    iter: usize,
    mask_refine_cal_type: &str,
    mask_refine_ndm: usize,
    is_in_domain: &[Vec<i32>],
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    lon_i: &[f64],
    lat_i: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<AreaJudgeAreaSourceReport> {
    let refine = build_area_judge_area_sources_one_based(
        file_dir,
        "mask_refine",
        mask_refine_cal_type,
        iter,
        mask_refine_ndm,
        lon_vertex,
        lat_vertex,
        lon_i,
        lat_i,
        gridnum_perdegree,
        nlons_source,
        nlats_source,
    )?;
    validate_area_judge_refine_within_domain_one_based(
        &refine.is_in_area,
        is_in_domain,
        refine.bounds,
    )?;
    Ok(refine)
}
