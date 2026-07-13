use crate::build_area_judge_bbox_area_source_one_based;
use crate::build_area_judge_circle_area_source_one_based;
use crate::build_area_judge_close_area_source_cells_one_based;
use crate::build_area_judge_lambert_area_source_one_based;
use crate::grid_covers_area_judge_bounds_one_based;
use crate::require_len;
use crate::AreaJudgeAreaSourceReport;
use std::io;
use std::path::Path;

use super::bounds::merge_area_judge_source_bounds;
use super::paths::area_judge_area_source_path;

/// Build and merge the file-numbered `IsInArea_*_Calculation` source loop.
pub fn build_area_judge_area_sources_one_based(
    file_dir: impl AsRef<Path>,
    type_select: &str,
    mask_type: &str,
    iter: usize,
    ndm: usize,
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    lon_i: &[f64],
    lat_i: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<AreaJudgeAreaSourceReport> {
    if ndm == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "area source dispatch requires at least one source",
        ));
    }

    let mut is_in_area = vec![vec![0_i32; nlats_source + 1]; nlons_source + 1];
    let mut bounds = None;
    let mut numpatch = 0usize;

    for source_index in 1..=ndm {
        let source =
            area_judge_area_source_path(&file_dir, type_select, mask_type, iter, source_index)?;
        if mask_type == "close" {
            let report = build_area_judge_close_area_source_cells_one_based(
                &source,
                lon_vertex,
                lat_vertex,
                gridnum_perdegree,
                nlons_source,
                nlats_source,
            )?;
            for (lon_index, lat_index) in &report.cells {
                is_in_area[*lon_index][*lat_index] = 1;
            }
            bounds = Some(merge_area_judge_source_bounds(bounds, report.bounds));
            numpatch += report.numpatch;
            continue;
        }
        let report = match mask_type {
            "bbox" => build_area_judge_bbox_area_source_one_based(
                &source,
                lon_vertex,
                lat_vertex,
                gridnum_perdegree,
                nlons_source,
                nlats_source,
            )?,
            "circle" => build_area_judge_circle_area_source_one_based(
                &source,
                lon_vertex,
                lat_vertex,
                lon_i,
                lat_i,
                gridnum_perdegree,
                nlons_source,
                nlats_source,
            )?,
            "lambert" => build_area_judge_lambert_area_source_one_based(
                &source,
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
                    format!("unsupported mask_type {other}"),
                ));
            }
        };
        grid_covers_area_judge_bounds_one_based(
            "area source dispatch mask",
            &report.is_in_area,
            report.bounds,
        )?;
        require_len(
            "area source dispatch source mask",
            report.is_in_area.len(),
            nlons_source + 1,
        )?;
        for lon_index in 1..=nlons_source {
            require_len(
                &format!("area source dispatch source mask[{lon_index}]"),
                report.is_in_area[lon_index].len(),
                nlats_source + 1,
            )?;
            for lat_index in 1..=nlats_source {
                if report.is_in_area[lon_index][lat_index] != 0 {
                    is_in_area[lon_index][lat_index] = 1;
                }
            }
        }
        bounds = Some(merge_area_judge_source_bounds(bounds, report.bounds));
        numpatch += report.numpatch;
    }

    let bounds = bounds.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "area source dispatch requires at least one source",
        )
    })?;

    Ok(AreaJudgeAreaSourceReport {
        is_in_area,
        bounds,
        numpatch,
    })
}
