use crate::merge_area_judge_source_bounds;
use crate::read_circle_mask_netcdf;
use crate::require_len;
use crate::validate_circle_mask_geographic;
use crate::AreaJudgeAreaSourceReport;
use std::io;
use std::path::Path;

use earthmesh_geometry::{is_point_in_circle_km, Point as AreaJudgePoint};
use earthmesh_mesh::{
    area_judge_minmax_range_make_one_based, area_judge_source_find_one_based, AreaJudgeAxis,
};

use super::bounds::area_judge_circle_scan_bounds_canonical;

/// Build the circle `IsInArea_grid` source mask used by domain/refine/patch paths.
pub fn build_area_judge_circle_area_source_one_based(
    inputfile: impl AsRef<Path>,
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    lon_i: &[f64],
    lat_i: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<AreaJudgeAreaSourceReport> {
    let mask = read_circle_mask_netcdf(inputfile)?;
    validate_circle_mask_geographic(&mask).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid circle area source: {err}"),
        )
    })?;
    let mut is_in_area = vec![vec![false; nlats_source + 1]; nlons_source + 1];
    let mut merged_bounds = None;
    let mut numpatch = 0usize;

    for (&center, &radius_km) in mask.points.iter().zip(mask.radius_km.iter()) {
        let (edgew_temp, edgee_temp, edgen_temp, edges_temp) =
            area_judge_circle_scan_bounds_canonical(center, radius_km)?;
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
                    "circle area scan bounds west/east/north/south = {edgew_temp}/{edgee_temp}/{edgen_temp}/{edges_temp} are outside source grid"
                ),
            )
        })?;
        let minlon_source = area_judge_source_find_one_based(
            edgew_temp,
            lon_vertex,
            AreaJudgeAxis::Longitude,
            gridnum_perdegree,
            nlons_source,
        )
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "missing circle min longitude source",
            )
        })?;
        let maxlon_source = area_judge_source_find_one_based(
            edgee_temp,
            lon_vertex,
            AreaJudgeAxis::Longitude,
            gridnum_perdegree,
            nlons_source,
        )
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "missing circle max longitude source",
            )
        })?;
        let maxlat_source = area_judge_source_find_one_based(
            edgen_temp,
            lat_vertex,
            AreaJudgeAxis::Latitude,
            gridnum_perdegree,
            nlats_source,
        )
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "missing circle max latitude source",
            )
        })?;
        let minlat_source = area_judge_source_find_one_based(
            edges_temp,
            lat_vertex,
            AreaJudgeAxis::Latitude,
            gridnum_perdegree,
            nlats_source,
        )
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "missing circle min latitude source",
            )
        })?;
        if minlon_source >= maxlon_source {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "circle minlon_source must be smaller than maxlon_source",
            ));
        }
        if maxlat_source >= minlat_source {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "circle maxlat_source must be smaller than minlat_source",
            ));
        }
        require_len("circle area mask", is_in_area.len(), maxlon_source)?;
        require_len("circle longitude centers", lon_i.len(), maxlon_source)?;
        require_len("circle latitude centers", lat_i.len(), minlat_source)?;
        for row in is_in_area.iter().take(maxlon_source).skip(minlon_source) {
            require_len("circle area mask row", row.len(), minlat_source)?;
        }

        for lon_index in minlon_source..maxlon_source {
            for lat_index in maxlat_source..minlat_source {
                if is_in_area[lon_index][lat_index] {
                    continue;
                }
                let point = AreaJudgePoint::new(lon_i[lon_index], lat_i[lat_index]);
                let center = AreaJudgePoint::new(center.lon, center.lat);
                if is_point_in_circle_km(point, center, radius_km) {
                    is_in_area[lon_index][lat_index] = true;
                    numpatch += 1;
                }
            }
        }
        merged_bounds = Some(merge_area_judge_source_bounds(merged_bounds, bounds));
    }

    let bounds = merged_bounds.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "circle area source must contain at least one circle",
        )
    })?;

    Ok(AreaJudgeAreaSourceReport {
        is_in_area,
        bounds,
        numpatch,
    })
}
