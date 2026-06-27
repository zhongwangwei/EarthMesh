pub(crate) fn line_intersection(
    a1: (f64, f64),
    a2: (f64, f64),
    b1: (f64, f64),
    b2: (f64, f64),
) -> Option<(f64, f64)> {
    let dax = a2.0 - a1.0;
    let day = a2.1 - a1.1;
    let dbx = b2.0 - b1.0;
    let dby = b2.1 - b1.1;
    let denominator = dax * dby - day * dbx;
    if denominator.abs() <= 1.0e-12 {
        return None;
    }
    let dx = b1.0 - a1.0;
    let dy = b1.1 - a1.1;
    let t = (dx * dby - dy * dbx) / denominator;
    Some((a1.0 + t * dax, a1.1 + t * day))
}

pub(crate) fn cross_2d(origin: (f64, f64), left: (f64, f64), right: (f64, f64)) -> f64 {
    (left.0 - origin.0) * (right.1 - origin.1) - (left.1 - origin.1) * (right.0 - origin.0)
}

pub(crate) fn segment_intersection_point(
    a1: (f64, f64),
    a2: (f64, f64),
    b1: (f64, f64),
    b2: (f64, f64),
) -> Option<(f64, f64)> {
    let point = line_intersection(a1, a2, b1, b2)?;
    if point_on_segment(point, a1, a2) && point_on_segment(point, b1, b2) {
        Some(point)
    } else {
        None
    }
}

pub(crate) fn point_on_segment(point: (f64, f64), start: (f64, f64), end: (f64, f64)) -> bool {
    are_collinear(start, point, end)
        && point.0 >= start.0.min(end.0) - 1.0e-12
        && point.0 <= start.0.max(end.0) + 1.0e-12
        && point.1 >= start.1.min(end.1) - 1.0e-12
        && point.1 <= start.1.max(end.1) + 1.0e-12
}

pub(crate) fn segment_parameter(point: (f64, f64), start: (f64, f64), end: (f64, f64)) -> f64 {
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    if dx.abs() >= dy.abs() {
        if dx.abs() <= 1.0e-12 {
            0.0
        } else {
            (point.0 - start.0) / dx
        }
    } else if dy.abs() <= 1.0e-12 {
        0.0
    } else {
        (point.1 - start.1) / dy
    }
}

pub(crate) fn point_in_rectangle(point: (f64, f64), rect: (f64, f64, f64, f64)) -> bool {
    point.0 > rect.0 && point.0 < rect.2 && point.1 > rect.1 && point.1 < rect.3
}

pub(crate) fn sort_dedup_f64(values: &mut Vec<f64>) {
    values.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    values.dedup_by(|left, right| (*left - *right).abs() <= 1.0e-12);
}

pub(crate) fn points_equal(left: (f64, f64), right: (f64, f64)) -> bool {
    (left.0 - right.0).abs() <= 1.0e-12 && (left.1 - right.1).abs() <= 1.0e-12
}

pub(crate) fn are_collinear(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> bool {
    ((b.0 - a.0) * (c.1 - b.1) - (b.1 - a.1) * (c.0 - b.0)).abs() <= 1.0e-12
}

pub(crate) fn ring_bbox(coordinates: &[(f64, f64)]) -> (f64, f64, f64, f64) {
    let mut min_lon = f64::INFINITY;
    let mut min_lat = f64::INFINITY;
    let mut max_lon = f64::NEG_INFINITY;
    let mut max_lat = f64::NEG_INFINITY;
    for (lon, lat) in coordinates {
        min_lon = min_lon.min(*lon);
        min_lat = min_lat.min(*lat);
        max_lon = max_lon.max(*lon);
        max_lat = max_lat.max(*lat);
    }
    (min_lon, min_lat, max_lon, max_lat)
}

pub(crate) fn rectangle_ring(rect: (f64, f64, f64, f64)) -> Vec<(f64, f64)> {
    vec![
        (rect.0, rect.1),
        (rect.2, rect.1),
        (rect.2, rect.3),
        (rect.0, rect.3),
    ]
}
