use super::primitives::{
    are_collinear, point_in_rectangle, points_equal, ring_bbox, sort_dedup_f64,
};
use super::rings::point_in_ring;

pub(crate) fn rectilinear_cells_contain_cells(
    outer_cells: &[(f64, f64, f64, f64)],
    inner_cells: &[(f64, f64, f64, f64)],
) -> bool {
    inner_cells.iter().all(|inner| {
        let center = ((inner.0 + inner.2) * 0.5, (inner.1 + inner.3) * 0.5);
        outer_cells
            .iter()
            .any(|outer| point_in_rectangle(center, *outer))
    })
}

pub(crate) fn axis_aligned_rectangle_from_ring(
    coordinates: &[(f64, f64)],
) -> Option<(f64, f64, f64, f64)> {
    if coordinates.len() != 4 {
        return None;
    }
    let bbox = ring_bbox(coordinates);
    let (min_lon, min_lat, max_lon, max_lat) = bbox;
    if min_lon >= max_lon || min_lat >= max_lat {
        return None;
    }
    let corners = [
        (min_lon, min_lat),
        (max_lon, min_lat),
        (max_lon, max_lat),
        (min_lon, max_lat),
    ];
    coordinates
        .iter()
        .all(|point| corners.iter().any(|corner| points_equal(*point, *corner)))
        .then_some(bbox)
}

pub(crate) fn rectilinear_ring_cells(
    coordinates: &[(f64, f64)],
) -> Option<Vec<(f64, f64, f64, f64)>> {
    if let Some(rectangle) = axis_aligned_rectangle_from_ring(coordinates) {
        return Some(vec![rectangle]);
    }
    if coordinates.len() < 4 || !is_rectilinear_ring(coordinates) {
        return None;
    }
    let mut xs = coordinates.iter().map(|(lon, _)| *lon).collect::<Vec<_>>();
    let mut ys = coordinates.iter().map(|(_, lat)| *lat).collect::<Vec<_>>();
    sort_dedup_f64(&mut xs);
    sort_dedup_f64(&mut ys);
    if xs.len() < 2 || ys.len() < 2 {
        return None;
    }
    let mut rectangles = Vec::<(f64, f64, f64, f64)>::new();
    for col in 0..(xs.len() - 1) {
        for row in 0..(ys.len() - 1) {
            let center = ((xs[col] + xs[col + 1]) * 0.5, (ys[row] + ys[row + 1]) * 0.5);
            if point_in_ring(center, coordinates) {
                rectangles.push((xs[col], ys[row], xs[col + 1], ys[row + 1]));
            }
        }
    }
    (!rectangles.is_empty()).then_some(rectangles)
}

pub(crate) fn is_rectilinear_ring(coordinates: &[(f64, f64)]) -> bool {
    coordinates
        .iter()
        .all(|(lon, lat)| lon.is_finite() && lat.is_finite())
        && coordinates.iter().enumerate().all(|(index, point)| {
            let next = coordinates[(index + 1) % coordinates.len()];
            !points_equal(*point, next)
                && ((point.0 - next.0).abs() <= 1.0e-12 || (point.1 - next.1).abs() <= 1.0e-12)
        })
}

pub(crate) fn rectilinear_union_cells(
    rectangles: &[(f64, f64, f64, f64)],
) -> Option<Vec<(f64, f64, f64, f64)>> {
    let mut xs = rectangles
        .iter()
        .flat_map(|(min_lon, _, max_lon, _)| [*min_lon, *max_lon])
        .collect::<Vec<_>>();
    let mut ys = rectangles
        .iter()
        .flat_map(|(_, min_lat, _, max_lat)| [*min_lat, *max_lat])
        .collect::<Vec<_>>();
    sort_dedup_f64(&mut xs);
    sort_dedup_f64(&mut ys);
    if xs.len() < 2 || ys.len() < 2 {
        return None;
    }
    let mut cells = Vec::new();
    for col in 0..(xs.len() - 1) {
        for row in 0..(ys.len() - 1) {
            let rect = (xs[col], ys[row], xs[col + 1], ys[row + 1]);
            let center = ((rect.0 + rect.2) * 0.5, (rect.1 + rect.3) * 0.5);
            if rectangles
                .iter()
                .any(|candidate| point_in_rectangle(center, *candidate))
            {
                cells.push(rect);
            }
        }
    }
    (!cells.is_empty()).then_some(cells)
}

pub(crate) fn rectilinear_union_boundary(
    rectangles: &[(f64, f64, f64, f64)],
) -> Option<Vec<(f64, f64)>> {
    let mut xs = rectangles
        .iter()
        .flat_map(|(min_lon, _, max_lon, _)| [*min_lon, *max_lon])
        .collect::<Vec<_>>();
    let mut ys = rectangles
        .iter()
        .flat_map(|(_, min_lat, _, max_lat)| [*min_lat, *max_lat])
        .collect::<Vec<_>>();
    sort_dedup_f64(&mut xs);
    sort_dedup_f64(&mut ys);
    if xs.len() < 2 || ys.len() < 2 {
        return None;
    }
    let cols = xs.len() - 1;
    let rows = ys.len() - 1;
    let mut covered = vec![vec![false; rows]; cols];
    for col in 0..cols {
        for row in 0..rows {
            let center = ((xs[col] + xs[col + 1]) * 0.5, (ys[row] + ys[row + 1]) * 0.5);
            covered[col][row] = rectangles
                .iter()
                .any(|rect| point_in_rectangle(center, *rect));
        }
    }

    let mut edges = Vec::<((f64, f64), (f64, f64))>::new();
    for col in 0..cols {
        for row in 0..rows {
            if !covered[col][row] {
                continue;
            }
            if row == 0 || !covered[col][row - 1] {
                edges.push(((xs[col], ys[row]), (xs[col + 1], ys[row])));
            }
            if col == cols - 1 || !covered[col + 1][row] {
                edges.push(((xs[col + 1], ys[row]), (xs[col + 1], ys[row + 1])));
            }
            if row == rows - 1 || !covered[col][row + 1] {
                edges.push(((xs[col + 1], ys[row + 1]), (xs[col], ys[row + 1])));
            }
            if col == 0 || !covered[col - 1][row] {
                edges.push(((xs[col], ys[row + 1]), (xs[col], ys[row])));
            }
        }
    }
    trace_rectilinear_boundary(edges)
}

pub(crate) fn trace_rectilinear_boundary(
    edges: Vec<((f64, f64), (f64, f64))>,
) -> Option<Vec<(f64, f64)>> {
    trace_polygon_boundary(edges).map(remove_collinear_boundary_vertices)
}

pub(crate) fn trace_polygon_boundary(
    mut edges: Vec<((f64, f64), (f64, f64))>,
) -> Option<Vec<(f64, f64)>> {
    if edges.is_empty() {
        return None;
    }
    let start = edges
        .iter()
        .map(|(start, _)| *start)
        .min_by(|left, right| {
            left.1
                .partial_cmp(&right.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    left.0
                        .partial_cmp(&right.0)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        })?;
    let mut current = start;
    let mut boundary = Vec::<(f64, f64)>::new();
    while !edges.is_empty() {
        boundary.push(current);
        let edge_index = edges
            .iter()
            .position(|(edge_start, _)| points_equal(*edge_start, current))?;
        let (_, next) = edges.remove(edge_index);
        current = next;
        if points_equal(current, start) {
            break;
        }
    }
    if !edges.is_empty() || boundary.len() < 4 {
        return None;
    }
    Some(boundary)
}

pub(crate) fn remove_collinear_boundary_vertices(mut boundary: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    if boundary.len() <= 3 {
        return boundary;
    }
    loop {
        let mut removed = false;
        let len = boundary.len();
        for index in 0..len {
            let previous = boundary[(index + len - 1) % len];
            let current = boundary[index];
            let next = boundary[(index + 1) % len];
            if are_collinear(previous, current, next) {
                boundary.remove(index);
                removed = true;
                break;
            }
        }
        if !removed || boundary.len() <= 3 {
            return boundary;
        }
    }
}
