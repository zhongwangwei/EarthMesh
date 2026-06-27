use crate::hydro_close_buffer::ring_area;

use super::primitives::{cross_2d, point_on_segment, points_equal, segment_intersection_point};

pub(crate) fn convex_polygon_intersection_area(
    left: &[(f64, f64)],
    right: &[(f64, f64)],
) -> Option<f64> {
    let mut points = Vec::<(f64, f64)>::new();
    points.extend(
        left.iter()
            .copied()
            .filter(|point| point_in_ring(*point, right) || point_on_ring_boundary(*point, right)),
    );
    points.extend(
        right
            .iter()
            .copied()
            .filter(|point| point_in_ring(*point, left) || point_on_ring_boundary(*point, left)),
    );
    for (left_start, left_end) in polygon_ring_edges(left)? {
        for (right_start, right_end) in polygon_ring_edges(right)? {
            if let Some(point) =
                segment_intersection_point(left_start, left_end, right_start, right_end)
            {
                points.push(point);
            }
        }
    }
    convex_hull_points(points).map(|ring| ring_area(&ring))
}

pub(crate) fn convex_hull_points<I>(points: I) -> Option<Vec<(f64, f64)>>
where
    I: IntoIterator<Item = (f64, f64)>,
{
    let mut points = points
        .into_iter()
        .filter(|(lon, lat)| lon.is_finite() && lat.is_finite())
        .collect::<Vec<_>>();
    points.sort_by(|left, right| {
        left.0
            .partial_cmp(&right.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                left.1
                    .partial_cmp(&right.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    points.dedup_by(|left, right| points_equal(*left, *right));
    if points.len() < 3 {
        return None;
    }

    let mut lower = Vec::<(f64, f64)>::new();
    for point in &points {
        while lower.len() >= 2
            && cross_2d(lower[lower.len() - 2], lower[lower.len() - 1], *point) <= 1.0e-12
        {
            lower.pop();
        }
        lower.push(*point);
    }
    let mut upper = Vec::<(f64, f64)>::new();
    for point in points.iter().rev() {
        while upper.len() >= 2
            && cross_2d(upper[upper.len() - 2], upper[upper.len() - 1], *point) <= 1.0e-12
        {
            upper.pop();
        }
        upper.push(*point);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    (lower.len() >= 3 && ring_area(&lower) > 1.0e-12).then_some(lower)
}

pub(crate) fn is_convex_ring(coordinates: &[(f64, f64)]) -> bool {
    if coordinates.len() < 3
        || !coordinates
            .iter()
            .all(|(lon, lat)| lon.is_finite() && lat.is_finite())
        || ring_area(coordinates) <= 1.0e-12
    {
        return false;
    }
    let mut sign = 0_i8;
    for index in 0..coordinates.len() {
        let previous = coordinates[(index + coordinates.len() - 1) % coordinates.len()];
        let current = coordinates[index];
        let next = coordinates[(index + 1) % coordinates.len()];
        let cross = cross_2d(previous, current, next);
        if cross.abs() <= 1.0e-12 {
            continue;
        }
        let current_sign = if cross > 0.0 { 1 } else { -1 };
        if sign == 0 {
            sign = current_sign;
        } else if sign != current_sign {
            return false;
        }
    }
    sign != 0
}

pub(crate) fn ring_contains_ring(outer: &[(f64, f64)], inner: &[(f64, f64)]) -> bool {
    outer.len() >= 3
        && inner.len() >= 3
        && inner
            .iter()
            .all(|point| point_on_ring_boundary(*point, outer) || point_in_ring(*point, outer))
}

pub(crate) fn point_on_ring_boundary(point: (f64, f64), coordinates: &[(f64, f64)]) -> bool {
    coordinates.iter().enumerate().any(|(index, start)| {
        let end = coordinates[(index + 1) % coordinates.len()];
        point_on_segment(point, *start, end)
    })
}

pub(crate) fn polygon_ring_edges(
    coordinates: &[(f64, f64)],
) -> Option<Vec<((f64, f64), (f64, f64))>> {
    if coordinates.len() < 3 {
        return None;
    }
    let mut edges = Vec::<((f64, f64), (f64, f64))>::new();
    for index in 0..coordinates.len() {
        let start = coordinates[index];
        let end = coordinates[(index + 1) % coordinates.len()];
        if points_equal(start, end) {
            return None;
        }
        edges.push((start, end));
    }
    Some(edges)
}

pub(crate) fn edge_midpoint_is_strictly_inside_ring(
    edge: ((f64, f64), (f64, f64)),
    coordinates: &[(f64, f64)],
) -> bool {
    let midpoint = ((edge.0 .0 + edge.1 .0) * 0.5, (edge.0 .1 + edge.1 .1) * 0.5);
    point_in_ring(midpoint, coordinates) && !point_on_ring_boundary(midpoint, coordinates)
}

pub(crate) fn point_in_ring(point: (f64, f64), coordinates: &[(f64, f64)]) -> bool {
    let (x, y) = point;
    let mut inside = false;
    for index in 0..coordinates.len() {
        let (xi, yi) = coordinates[index];
        let (xj, yj) = coordinates[(index + 1) % coordinates.len()];
        if (yi > y) != (yj > y) {
            let intersection_x = (xj - xi) * (y - yi) / (yj - yi) + xi;
            if x < intersection_x {
                inside = !inside;
            }
        }
    }
    inside
}
