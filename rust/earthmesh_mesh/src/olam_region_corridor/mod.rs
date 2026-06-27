use crate::{
    olam_corridor_segment_distance_meters, plane_segment_distance, CartesianPoint, LonLatDegrees,
    PlanePoint,
};

pub(crate) fn olam_closed_corridor_contains_cartesian(
    point: CartesianPoint,
    points: &[LonLatDegrees],
    radius: f64,
    corridor_radius_meters: f64,
) -> bool {
    if points.len() < 2 {
        return false;
    }
    points.windows(2).any(|segment| {
        olam_corridor_segment_distance_meters(point, segment[0], segment[1], radius).0
            < corridor_radius_meters
    }) || points
        .last()
        .zip(points.first())
        .is_some_and(|(&last, &first)| {
            olam_corridor_segment_distance_meters(point, last, first, radius).0
                < corridor_radius_meters
        })
}

pub(crate) fn olam_open_corridor_contains_cartesian(
    point: CartesianPoint,
    points: &[LonLatDegrees],
    radius: f64,
    corridor_radius_meters: f64,
) -> bool {
    if points.len() < 2 {
        return false;
    }
    points.windows(2).any(|segment| {
        olam_corridor_segment_distance_meters(point, segment[0], segment[1], radius).0
            < corridor_radius_meters
    })
}

pub(crate) fn olam_corridor_radius_at_segment(radius_meters: &[f64], idx: usize, t: f64) -> f64 {
    let start = radius_meters
        .get(idx)
        .copied()
        .or_else(|| radius_meters.last().copied())
        .unwrap_or(0.0);
    let end = radius_meters
        .get(idx + 1)
        .copied()
        .or_else(|| radius_meters.last().copied())
        .unwrap_or(start);
    (1.0 - t) * start + t * end
}

pub(crate) fn olam_cartesian_xy_segment_distance(
    point: CartesianPoint,
    start: LonLatDegrees,
    end: LonLatDegrees,
) -> (f64, f64) {
    plane_segment_distance(
        PlanePoint::new(point.x, point.y),
        PlanePoint::new(start.lon_degrees, start.lat_degrees),
        PlanePoint::new(end.lon_degrees, end.lat_degrees),
    )
}
