use crate::close_mask_netcdf_has_refine;
use crate::read_close_mask_netcdf;
use crate::read_close_mesh_netcdf;
use crate::validate_close_mask_geographic;
use crate::AreaJudgeAreaSourceReport;
use crate::AreaJudgeSparseAreaSourceReport;
use crate::CloseMask;
use std::io;
use std::path::Path;

use earthmesh_geometry::{area_judge_first_self_intersection_one_based, Point as AreaJudgePoint};
use earthmesh_mesh::{
    area_judge_closed_curve_fill_one_based, area_judge_minmax_range_make_one_based,
    area_judge_source_find_one_based, AreaJudgeAxis, AreaJudgeSourceBounds, LonLatDegrees,
};

use super::dateline::{area_judge_check_crossing, area_judge_close_crosses_dateline};

/// Build the close-curve source cells used by domain/refine/patch paths.
pub fn build_area_judge_close_area_source_cells_one_based(
    inputfile: impl AsRef<Path>,
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<AreaJudgeSparseAreaSourceReport> {
    let inputfile = inputfile.as_ref();
    let mask = if close_mask_netcdf_has_refine(inputfile)? {
        read_close_mask_netcdf(inputfile)?
    } else {
        CloseMask {
            refine_degree: 0,
            points: read_close_mesh_netcdf(inputfile)?,
        }
    };
    validate_close_mask_geographic(&mask).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid close area source: {err}"),
        )
    })?;
    let close_points = &mask.points;
    let geometry_points = close_points
        .iter()
        .map(|point| AreaJudgePoint::new(point.lon, point.lat))
        .collect::<Vec<_>>();
    if let Some(intersection) = area_judge_first_self_intersection_one_based(&geometry_points) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "close polygon self-intersects between segments {} and {}",
                intersection.first_segment_id, intersection.second_segment_id
            ),
        ));
    }

    let mut fill_points = close_points
        .iter()
        .map(|point| LonLatDegrees {
            lon_degrees: point.lon,
            lat_degrees: point.lat,
        })
        .collect::<Vec<_>>();
    let mut edgew_temp = fill_points
        .iter()
        .map(|point| point.lon_degrees)
        .fold(f64::INFINITY, f64::min);
    let mut edgee_temp = fill_points
        .iter()
        .map(|point| point.lon_degrees)
        .fold(f64::NEG_INFINITY, f64::max);
    let edgen_temp = fill_points
        .iter()
        .map(|point| point.lat_degrees)
        .fold(f64::NEG_INFINITY, f64::max);
    let edges_temp = fill_points
        .iter()
        .map(|point| point.lat_degrees)
        .fold(f64::INFINITY, f64::min);
    let restore_dateline_shift = area_judge_close_crosses_dateline(&fill_points);
    if restore_dateline_shift {
        edgew_temp = -180.0;
        edgee_temp = 180.0;
        area_judge_check_crossing(&mut fill_points);
    }

    let fill = area_judge_closed_curve_fill_one_based(
        &fill_points,
        lon_vertex,
        lat_vertex,
        gridnum_perdegree,
        nlons_source,
        nlats_source,
        restore_dateline_shift,
    )
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "close area source could not be converted to source-grid cells",
        )
    })?;
    let bounds = area_judge_minmax_range_make_one_based(
        edgew_temp,
        edgee_temp,
        edgen_temp,
        edges_temp,
        lon_vertex,
        lat_vertex,
        gridnum_perdegree,
        nlons_source,
        nlats_source,
    )
    .or_else(|| {
        // Canonical closed-curve scans legitimately execute zero iterations
        // when a polygon is smaller than one source cell.  Keep a safe,
        // non-inverted anchor bound for downstream range aggregation rather
        // than confusing that empty selection with an out-of-grid polygon.
        if !fill.cells.is_empty() {
            return None;
        }
        let lon_index = area_judge_source_find_one_based(
            edgew_temp,
            lon_vertex,
            AreaJudgeAxis::Longitude,
            gridnum_perdegree,
            nlons_source,
        )?
        .min(nlons_source);
        let lat_index = area_judge_source_find_one_based(
            edgen_temp,
            lat_vertex,
            AreaJudgeAxis::Latitude,
            gridnum_perdegree,
            nlats_source,
        )?
        .min(nlats_source);
        Some(AreaJudgeSourceBounds {
            minlon_source: lon_index,
            maxlon_source: lon_index,
            maxlat_source: lat_index,
            minlat_source: lat_index,
        })
    })
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "close area bounds west/east/north/south = {edgew_temp}/{edgee_temp}/{edgen_temp}/{edges_temp} are outside source grid"
            ),
        )
    })?;
    for &(lon_index, lat_index) in &fill.cells {
        if lon_index > nlons_source || lat_index > nlats_source {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("close area source cell ({lon_index},{lat_index}) is outside source grid"),
            ));
        }
    }

    Ok(AreaJudgeSparseAreaSourceReport {
        cells: fill.cells,
        bounds,
        numpatch: fill.patch_count,
    })
}

/// Build the close-curve `IsInArea_grid` source mask used by domain/refine/patch paths.
pub fn build_area_judge_close_area_source_one_based(
    inputfile: impl AsRef<Path>,
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<AreaJudgeAreaSourceReport> {
    let sparse = build_area_judge_close_area_source_cells_one_based(
        inputfile,
        lon_vertex,
        lat_vertex,
        gridnum_perdegree,
        nlons_source,
        nlats_source,
    )?;
    let mut is_in_area = vec![vec![false; nlats_source + 1]; nlons_source + 1];
    for (lon_index, lat_index) in &sparse.cells {
        is_in_area[*lon_index][*lat_index] = true;
    }

    Ok(AreaJudgeAreaSourceReport {
        is_in_area,
        bounds: sparse.bounds,
        numpatch: sparse.numpatch,
    })
}
