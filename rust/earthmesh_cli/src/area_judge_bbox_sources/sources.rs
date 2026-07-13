use crate::grid_covers_area_judge_bounds_one_based;
use crate::merge_area_judge_source_bounds;
use crate::read_bbox_mask_netcdf;
use crate::validate_bbox_mask;
use crate::AreaJudgeAreaSourceReport;
use crate::AreaJudgePatchSourceReport;
use std::io;
use std::path::Path;

use earthmesh_mesh::{
    area_judge_apply_mask_patch_one_based, area_judge_minmax_range_make_one_based,
};

/// Build the bbox `IsInArea_grid` source mask used by domain/refine/patch paths.
pub fn build_area_judge_bbox_area_source_one_based(
    inputfile: impl AsRef<Path>,
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<AreaJudgeAreaSourceReport> {
    let mask = read_bbox_mask_netcdf(inputfile)?;
    validate_bbox_mask(&mask).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid bbox area source: {err}"),
        )
    })?;
    let mut is_in_area = vec![vec![0_i32; nlats_source + 1]; nlons_source + 1];
    let mut merged_bounds = None;
    let mut numpatch = 0usize;

    for point in &mask.points {
        let bounds = area_judge_minmax_range_make_one_based(
            point.west,
            point.east,
            point.north,
            point.south,
            lon_vertex,
            lat_vertex,
            gridnum_perdegree,
            nlons_source,
            nlats_source,
        )
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "bbox area bounds west/east/north/south = {}/{}/{}/{} are outside source grid",
                    point.west, point.east, point.north, point.south
                ),
            )
        })?;
        grid_covers_area_judge_bounds_one_based("bbox area mask", &is_in_area, bounds)?;
        for lon_index in bounds.minlon_source..=bounds.maxlon_source {
            for lat_index in bounds.maxlat_source..=bounds.minlat_source {
                is_in_area[lon_index][lat_index] = 1;
            }
        }
        numpatch += (bounds.maxlon_source - bounds.minlon_source + 1)
            * (bounds.minlat_source - bounds.maxlat_source + 1);
        merged_bounds = Some(merge_area_judge_source_bounds(merged_bounds, bounds));
    }

    let bounds = merged_bounds.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "bbox area source must contain at least one bbox point",
        )
    })?;

    Ok(AreaJudgeAreaSourceReport {
        is_in_area,
        bounds,
        numpatch,
    })
}

/// Build the bbox `IsInPaArea_grid` patch mask and apply it to `seaorland`.
///
/// This is the file-backed orchestration slice of
/// `MOD_Area_judge.F90:mask_patch_modify` for bbox patch sources: read the
/// bbox source, derive Canonical one-based source bounds through
/// `minmax_range_make`, fill the selected patch grid, then call the already
/// current `seaorland(i,j)=0` patch core.
pub fn apply_area_judge_bbox_patch_source_one_based(
    inputfile: impl AsRef<Path>,
    seaorland: &mut [Vec<i32>],
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<AreaJudgePatchSourceReport> {
    let source = build_area_judge_bbox_area_source_one_based(
        inputfile,
        lon_vertex,
        lat_vertex,
        gridnum_perdegree,
        nlons_source,
        nlats_source,
    )
    .map_err(|err| {
        io::Error::new(
            err.kind(),
            err.to_string().replace("bbox area", "bbox patch"),
        )
    })?;
    let report =
        area_judge_apply_mask_patch_one_based(seaorland, &source.is_in_area, source.bounds)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "seaorland or bbox patch mask does not cover selected source bounds",
                )
            })?;

    Ok(AreaJudgePatchSourceReport {
        bounds: source.bounds,
        patched_cells: report.patched_cells,
    })
}
