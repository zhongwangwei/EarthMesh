use std::io;
use std::path::Path;

use earthmesh_geometry::{
    area_judge_first_self_intersection_fortran_indexed, Point as AreaJudgePoint,
};
use earthmesh_mesh::{
    area_judge_closed_curve_fill_fortran_indexed, area_judge_minmax_range_make_fortran_indexed,
    LonLatDegrees,
};

use super::dateline::{area_judge_check_crossing, area_judge_close_crosses_dateline};
use crate::*;

/// Build the close-curve source cells used by domain/refine/patch paths.
pub fn build_area_judge_close_area_source_cells_fortran_indexed(
    inputfile: impl AsRef<Path>,
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<AreaJudgeSparseAreaSourceReport> {
    let inputfile = inputfile.as_ref();
    let mask = match read_close_mask_netcdf(inputfile) {
        Ok(mask) => mask,
        Err(err) if err.to_string().contains("close_refine") => CloseMask {
            refine_degree: 0,
            points: read_close_mesh_netcdf(inputfile)?,
        },
        Err(err) => return Err(err),
    };
    validate_close_mask(&mask).map_err(|err| {
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
    if let Some(intersection) = area_judge_first_self_intersection_fortran_indexed(&geometry_points)
    {
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

    let bounds = area_judge_minmax_range_make_fortran_indexed(
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
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "close area bounds west/east/north/south = {edgew_temp}/{edgee_temp}/{edgen_temp}/{edges_temp} are outside source grid"
            ),
        )
    })?;
    let fill = area_judge_closed_curve_fill_fortran_indexed(
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
pub fn build_area_judge_close_area_source_fortran_indexed(
    inputfile: impl AsRef<Path>,
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<AreaJudgeAreaSourceReport> {
    let sparse = build_area_judge_close_area_source_cells_fortran_indexed(
        inputfile,
        lon_vertex,
        lat_vertex,
        gridnum_perdegree,
        nlons_source,
        nlats_source,
    )?;
    let mut is_in_area = vec![vec![0_i32; nlats_source + 1]; nlons_source + 1];
    for (lon_index, lat_index) in &sparse.cells {
        is_in_area[*lon_index][*lat_index] = 1;
    }

    Ok(AreaJudgeAreaSourceReport {
        is_in_area,
        bounds: sparse.bounds,
        numpatch: sparse.numpatch,
    })
}
