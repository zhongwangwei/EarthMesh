use std::io;
use std::path::Path;

use super::global::initialize_area_judge_global_domain_fortran_indexed;
use crate::*;

/// Build the `Area_judge` domain mask for global or file-numbered domain sources.
pub fn build_area_judge_domain_fortran_indexed(
    file_dir: impl AsRef<Path>,
    mask_domain_global: bool,
    mask_domain_type: &str,
    mask_domain_ndm: usize,
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    lon_i: &[f64],
    lat_i: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<AreaJudgeDomainInitializationReport> {
    if mask_domain_global {
        return initialize_area_judge_global_domain_fortran_indexed(nlons_source, nlats_source);
    }

    let source = build_area_judge_area_sources_fortran_indexed(
        file_dir,
        "mask_domain",
        mask_domain_type,
        0,
        mask_domain_ndm,
        lon_vertex,
        lat_vertex,
        lon_i,
        lat_i,
        gridnum_perdegree,
        nlons_source,
        nlats_source,
    )?;
    Ok(AreaJudgeDomainInitializationReport {
        is_in_domain: source.is_in_area,
        bounds: source.bounds,
        numpatch: source.numpatch,
        nlons_select: source.bounds.maxlon_source - source.bounds.minlon_source + 1,
        nlats_select: source.bounds.minlat_source - source.bounds.maxlat_source + 1,
    })
}
