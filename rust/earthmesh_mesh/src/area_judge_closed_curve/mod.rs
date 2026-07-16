use super::*;

/// Source cells selected by the closed-curve ray-crossing fill in
/// `MOD_Area_judge:IsInArea_close_Calculation`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaJudgeClosedCurveFill {
    pub cells: Vec<(usize, usize)>,
    pub patch_count: usize,
}

fn area_judge_ray_segment_intersection_lon(
    ray_lat: f64,
    start: LonLatDegrees,
    end: LonLatDegrees,
) -> Option<f64> {
    let lat1 = start.lat_degrees;
    let lat2 = end.lat_degrees;
    if (lat1 > ray_lat) == (lat2 > ray_lat) {
        return None;
    }

    Some(
        start.lon_degrees
            + (ray_lat - lat1) * (end.lon_degrees - start.lon_degrees) / (lat2 - lat1),
    )
}

/// Pure Rust source-cell fill for the closed-curve branch in
/// `MOD_Area_judge:IsInArea_close_Calculation`.
///
/// The helper mirrors the Canonical row scan after `minmax_range_make`: for each
/// source latitude row between the polygon north/south bounds, intersect a
/// left-to-right ray with every polygon segment, sort the intersection
/// longitudes, then mark cells between odd/even intersection pairs.  When
/// `restore_dateline_shift` is true, filled longitude indices are remapped with
/// the same half-world shift that Canonical applies after `CheckCrossing`.
pub fn area_judge_closed_curve_fill_one_based(
    close_points: &[LonLatDegrees],
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
    restore_dateline_shift: bool,
) -> Option<AreaJudgeClosedCurveFill> {
    if close_points.len() < 3 || lat_vertex.len() < 2 {
        return None;
    }

    let edgen_temp = close_points
        .iter()
        .map(|point| point.lat_degrees)
        .fold(f64::NEG_INFINITY, f64::max);
    let edges_temp = close_points
        .iter()
        .map(|point| point.lat_degrees)
        .fold(f64::INFINITY, f64::min);
    let maxlat_source = area_judge_source_find_one_based(
        edgen_temp,
        lat_vertex,
        AreaJudgeAxis::Latitude,
        gridnum_perdegree,
        nlats_source,
    )?;
    let minlat_source = area_judge_source_find_one_based(
        edges_temp,
        lat_vertex,
        AreaJudgeAxis::Latitude,
        gridnum_perdegree,
        nlats_source,
    )?;
    if minlat_source > lat_vertex.len() {
        return None;
    }
    if maxlat_source > minlat_source {
        return None;
    }
    if maxlat_source == minlat_source {
        return Some(AreaJudgeClosedCurveFill {
            cells: Vec::new(),
            patch_count: 0,
        });
    }

    let mut cells = Vec::new();
    let mut patch_count = 0usize;
    for lat_index in maxlat_source..minlat_source {
        let ray_lat = 0.5 * (lat_vertex[lat_index] + lat_vertex[lat_index + 1]);
        let mut intersections = Vec::new();
        for edge_index in 0..close_points.len() {
            let start = close_points[edge_index];
            let end = close_points[(edge_index + 1) % close_points.len()];
            if let Some(lon_intersect) =
                area_judge_ray_segment_intersection_lon(ray_lat, start, end)
            {
                intersections.push(lon_intersect);
            }
        }
        intersections.sort_by(f64::total_cmp);

        for pair in intersections.chunks_exact(2) {
            let minlon_source = area_judge_source_find_one_based(
                pair[0],
                lon_vertex,
                AreaJudgeAxis::Longitude,
                gridnum_perdegree,
                nlons_source,
            )?;
            let maxlon_source = area_judge_source_find_one_based(
                pair[1],
                lon_vertex,
                AreaJudgeAxis::Longitude,
                gridnum_perdegree,
                nlons_source,
            )?;
            if minlon_source > maxlon_source {
                return None;
            }
            patch_count += maxlon_source - minlon_source;
            for lon_index in minlon_source..maxlon_source {
                let restored_lon_index =
                    if restore_dateline_shift && lon_index < nlons_source / 2 + 1 {
                        lon_index + nlons_source / 2
                    } else if restore_dateline_shift {
                        lon_index - nlons_source / 2
                    } else {
                        lon_index
                    };
                cells.push((restored_lon_index, lat_index));
            }
        }
    }

    Some(AreaJudgeClosedCurveFill { cells, patch_count })
}
