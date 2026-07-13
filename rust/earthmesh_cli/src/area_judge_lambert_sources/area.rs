use crate::area_judge_check_crossing;
use crate::area_judge_close_crosses_dateline;
use crate::read_mode4_mesh_netcdf;
use crate::require_len;
use crate::validate_mode4_mesh_for_area_judge;
use crate::AreaJudgeAreaSourceReport;
use std::io;
use std::path::Path;

use earthmesh_geometry::{is_point_in_convex_polygon, Point as AreaJudgePoint};
use earthmesh_mesh::{
    area_judge_minmax_range_make_one_based, area_judge_source_find_one_based, AreaJudgeAxis,
    LonLatDegrees,
};

fn area_judge_checked_source_index_minus_one(index: usize) -> usize {
    index.saturating_sub(1).max(1)
}

/// Build the Lambert/mode4 `IsInArea_grid` source mask used by domain/refine/patch paths.
pub fn build_area_judge_lambert_area_source_one_based(
    inputfile: impl AsRef<Path>,
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    lon_i: &[f64],
    lat_i: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<AreaJudgeAreaSourceReport> {
    require_len("lon_i", lon_i.len(), nlons_source + 1)?;
    require_len("lat_i", lat_i.len(), nlats_source + 1)?;

    let mesh = read_mode4_mesh_netcdf(inputfile)?;
    validate_mode4_mesh_for_area_judge(&mesh).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid lambert area source: {err}"),
        )
    })?;

    let mesh_points = &mesh.lonlat_bound[1..];
    let mut edgew_temp = mesh_points
        .iter()
        .map(|point| point.lon)
        .fold(f64::INFINITY, f64::min);
    let mut edgee_temp = mesh_points
        .iter()
        .map(|point| point.lon)
        .fold(f64::NEG_INFINITY, f64::max);
    let edgen_temp = mesh_points
        .iter()
        .map(|point| point.lat)
        .fold(f64::NEG_INFINITY, f64::max);
    let edges_temp = mesh_points
        .iter()
        .map(|point| point.lat)
        .fold(f64::INFINITY, f64::min);
    let global_points = mesh_points
        .iter()
        .map(|point| LonLatDegrees {
            lon_degrees: point.lon,
            lat_degrees: point.lat,
        })
        .collect::<Vec<_>>();
    if area_judge_close_crosses_dateline(&global_points) {
        edgew_temp = -180.0;
        edgee_temp = 180.0;
    }

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
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "lambert area bounds west/east/north/south = {edgew_temp}/{edgee_temp}/{edgen_temp}/{edges_temp} are outside source grid"
            ),
        )
    })?;

    let mut is_in_area = vec![vec![0_i32; nlats_source + 1]; nlons_source + 1];
    let mut numpatch = 0_usize;
    for cell_index in 1..mesh.mode_points() {
        if mesh.n_ngr[cell_index] < 4 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("lambert mode4 cell {cell_index} must have at least four vertices"),
            ));
        }

        let mut cell_points = mesh.ngr_bound[cell_index]
            .iter()
            .map(|&bound_index| {
                let bound_index = usize::try_from(bound_index).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "lambert mode4 cell {cell_index} has negative vertex id {bound_index}"
                        ),
                    )
                })?;
                mesh.lonlat_bound.get(bound_index - 1).copied().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "lambert mode4 cell {cell_index} canonicals out-of-range vertex {bound_index}"
                        ),
                    )
                })
            })
            .collect::<io::Result<Vec<_>>>()?;

        let restore_dateline_shift = area_judge_close_crosses_dateline(
            &cell_points
                .iter()
                .map(|point| LonLatDegrees {
                    lon_degrees: point.lon,
                    lat_degrees: point.lat,
                })
                .collect::<Vec<_>>(),
        );
        if restore_dateline_shift {
            let mut shifted = cell_points
                .iter()
                .map(|point| LonLatDegrees {
                    lon_degrees: point.lon,
                    lat_degrees: point.lat,
                })
                .collect::<Vec<_>>();
            area_judge_check_crossing(&mut shifted);
            for (point, shifted_point) in cell_points.iter_mut().zip(shifted) {
                point.lon = shifted_point.lon_degrees;
                point.lat = shifted_point.lat_degrees;
            }
        }

        let minlon_source = area_judge_source_find_one_based(
            cell_points
                .iter()
                .map(|point| point.lon)
                .fold(f64::INFINITY, f64::min),
            lon_vertex,
            AreaJudgeAxis::Longitude,
            gridnum_perdegree,
            nlons_source,
        )
        .map(area_judge_checked_source_index_minus_one)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("lambert mode4 cell {cell_index} west edge is outside source grid"),
            )
        })?;
        let maxlon_source = area_judge_source_find_one_based(
            cell_points
                .iter()
                .map(|point| point.lon)
                .fold(f64::NEG_INFINITY, f64::max),
            lon_vertex,
            AreaJudgeAxis::Longitude,
            gridnum_perdegree,
            nlons_source,
        )
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("lambert mode4 cell {cell_index} east edge is outside source grid"),
            )
        })?;
        let maxlat_source = area_judge_source_find_one_based(
            cell_points
                .iter()
                .map(|point| point.lat)
                .fold(f64::NEG_INFINITY, f64::max),
            lat_vertex,
            AreaJudgeAxis::Latitude,
            gridnum_perdegree,
            nlats_source,
        )
        .map(area_judge_checked_source_index_minus_one)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("lambert mode4 cell {cell_index} north edge is outside source grid"),
            )
        })?;
        let minlat_source = area_judge_source_find_one_based(
            cell_points
                .iter()
                .map(|point| point.lat)
                .fold(f64::INFINITY, f64::min),
            lat_vertex,
            AreaJudgeAxis::Latitude,
            gridnum_perdegree,
            nlats_source,
        )
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("lambert mode4 cell {cell_index} south edge is outside source grid"),
            )
        })?;

        let polygon = cell_points
            .iter()
            .map(|point| AreaJudgePoint::new(point.lon, point.lat))
            .collect::<Vec<_>>();
        for lon_index in minlon_source..maxlon_source {
            for lat_index in maxlat_source..minlat_source {
                let point = AreaJudgePoint::new(lon_i[lon_index], lat_i[lat_index]);
                if !is_point_in_convex_polygon(&polygon, point) {
                    continue;
                }
                let restored_lon_index =
                    if restore_dateline_shift && lon_index < nlons_source / 2 + 1 {
                        lon_index + nlons_source / 2
                    } else if restore_dateline_shift {
                        lon_index - nlons_source / 2
                    } else {
                        lon_index
                    };
                require_len(
                    "lambert area mask",
                    is_in_area.len(),
                    restored_lon_index + 1,
                )?;
                require_len(
                    &format!("lambert area mask[{restored_lon_index}]"),
                    is_in_area[restored_lon_index].len(),
                    lat_index + 1,
                )?;
                is_in_area[restored_lon_index][lat_index] = 1;
                numpatch += 1;
            }
        }
    }

    Ok(AreaJudgeAreaSourceReport {
        is_in_area,
        bounds,
        numpatch,
    })
}
