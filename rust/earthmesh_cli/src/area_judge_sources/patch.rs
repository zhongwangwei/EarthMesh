use crate::apply_area_judge_bbox_patch_source_one_based;
use crate::apply_area_judge_circle_patch_source_one_based;
use crate::apply_area_judge_close_patch_source_one_based;
use crate::apply_area_judge_lambert_patch_source_one_based;
use crate::AreaJudgePatchModifyReport;
use std::io;
use std::path::Path;

use super::bounds::merge_area_judge_source_bounds;
use super::paths::area_judge_patch_source_path;

/// Apply the file-numbered source loop from `MOD_Area_judge:mask_patch_modify`.
pub fn apply_area_judge_patch_sources_one_based(
    file_dir: impl AsRef<Path>,
    mask_patch_type: &str,
    iter: usize,
    ndm: usize,
    seaorland: &mut [Vec<bool>],
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    lon_i: &[f64],
    lat_i: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<AreaJudgePatchModifyReport> {
    let mut source_reports = Vec::with_capacity(ndm);
    let mut bounds = None;
    let mut patched_cells = 0usize;

    for source_index in 1..=ndm {
        let source = area_judge_patch_source_path(&file_dir, mask_patch_type, iter, source_index)?;
        let report = match mask_patch_type {
            "bbox" => apply_area_judge_bbox_patch_source_one_based(
                &source,
                seaorland,
                lon_vertex,
                lat_vertex,
                gridnum_perdegree,
                nlons_source,
                nlats_source,
            )?,
            "circle" => apply_area_judge_circle_patch_source_one_based(
                &source,
                seaorland,
                lon_vertex,
                lat_vertex,
                lon_i,
                lat_i,
                gridnum_perdegree,
                nlons_source,
                nlats_source,
            )?,
            "close" => apply_area_judge_close_patch_source_one_based(
                &source,
                seaorland,
                lon_vertex,
                lat_vertex,
                gridnum_perdegree,
                nlons_source,
                nlats_source,
            )?,
            "lambert" => apply_area_judge_lambert_patch_source_one_based(
                &source,
                seaorland,
                lon_vertex,
                lat_vertex,
                lon_i,
                lat_i,
                gridnum_perdegree,
                nlons_source,
                nlats_source,
            )?,
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unsupported mask_patch_type {other}"),
                ));
            }
        };
        bounds = Some(merge_area_judge_source_bounds(bounds, report.bounds));
        patched_cells += report.patched_cells;
        source_reports.push(report);
    }

    Ok(AreaJudgePatchModifyReport {
        source_reports,
        bounds,
        patched_cells,
    })
}
