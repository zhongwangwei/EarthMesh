use crate::hydro_close_geometry_utils::sort_dedup_f64;

pub(super) fn line_segment_y_at_x(start: (f64, f64), end: (f64, f64), x: f64) -> Option<f64> {
    let dx = end.0 - start.0;
    if dx.abs() <= 1.0e-12 {
        return None;
    }
    let t = (x - start.0) / dx;
    if !(-1.0e-12..=1.0 + 1.0e-12).contains(&t) {
        return None;
    }
    Some(start.1 + t * (end.1 - start.1))
}

pub(crate) fn ring_y_spans_at_slab_boundary(
    ring: &[(f64, f64)],
    boundary_x: f64,
    interior_x: f64,
    expected_spans: usize,
) -> Option<Vec<(f64, f64)>> {
    let exact = ring_y_spans_at_x(ring, boundary_x);
    if exact.len() == expected_spans {
        return Some(exact);
    }
    let width = (interior_x - boundary_x).abs();
    if width <= 1.0e-12 {
        return None;
    }
    let direction = if interior_x >= boundary_x { 1.0 } else { -1.0 };
    let sampled_x = boundary_x + direction * width * 1.0e-9;
    let sampled = ring_y_spans_at_x(ring, sampled_x);
    (sampled.len() == expected_spans).then_some(sampled)
}

pub(crate) fn ring_y_spans_at_x(ring: &[(f64, f64)], x: f64) -> Vec<(f64, f64)> {
    let mut ys = Vec::new();
    for index in 0..ring.len() {
        let start = ring[index];
        let end = ring[(index + 1) % ring.len()];
        let min_x = start.0.min(end.0);
        let max_x = start.0.max(end.0);
        if x < min_x - 1.0e-12 || x > max_x + 1.0e-12 {
            continue;
        }
        let dx = end.0 - start.0;
        if dx.abs() <= 1.0e-12 {
            if (x - start.0).abs() <= 1.0e-12 {
                ys.push(start.1);
                ys.push(end.1);
            }
            continue;
        }
        let t = (x - start.0) / dx;
        if !(-1.0e-12..=1.0 + 1.0e-12).contains(&t) {
            continue;
        }
        ys.push(start.1 + t * (end.1 - start.1));
    }
    sort_dedup_f64(&mut ys);
    if ys.len() == 1 {
        return vec![(ys[0], ys[0])];
    }
    if ys.len() < 2 || ys.len() % 2 != 0 {
        return Vec::new();
    }
    ys.chunks(2)
        .filter_map(|pair| match pair {
            [low, high] => Some((*low, *high)),
            _ => None,
        })
        .collect()
}

pub(crate) fn triangle_y_span_at_x(triangle: &[(f64, f64)], x: f64) -> Option<(f64, f64)> {
    ring_y_span_at_x(triangle, x)
}

fn ring_y_span_at_x(ring: &[(f64, f64)], x: f64) -> Option<(f64, f64)> {
    let mut ys = Vec::new();
    for index in 0..ring.len() {
        let start = ring[index];
        let end = ring[(index + 1) % ring.len()];
        let min_x = start.0.min(end.0);
        let max_x = start.0.max(end.0);
        if x < min_x - 1.0e-12 || x > max_x + 1.0e-12 {
            continue;
        }
        let dx = end.0 - start.0;
        if dx.abs() <= 1.0e-12 {
            if (x - start.0).abs() <= 1.0e-12 {
                ys.push(start.1);
                ys.push(end.1);
            }
            continue;
        }
        let t = (x - start.0) / dx;
        if !(-1.0e-12..=1.0 + 1.0e-12).contains(&t) {
            continue;
        }
        ys.push(start.1 + t * (end.1 - start.1));
    }
    sort_dedup_f64(&mut ys);
    match ys.as_slice() {
        [] => None,
        [one] => Some((*one, *one)),
        values => Some((*values.first()?, *values.last()?)),
    }
}
