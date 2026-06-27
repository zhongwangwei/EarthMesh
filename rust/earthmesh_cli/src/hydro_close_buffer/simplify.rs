pub(crate) fn simplify_closed_ring(
    coordinates: Vec<(f64, f64)>,
    tolerance_deg: f64,
) -> Vec<(f64, f64)> {
    if tolerance_deg <= 0.0 || coordinates.len() <= 3 {
        return coordinates;
    }
    let mut keep = vec![false; coordinates.len()];
    keep[0] = true;
    simplify_ring_segment(
        &coordinates,
        0,
        coordinates.len() - 1,
        tolerance_deg,
        &mut keep,
    );
    simplify_ring_segment(
        &coordinates,
        coordinates.len() - 1,
        0,
        tolerance_deg,
        &mut keep,
    );
    let simplified = coordinates
        .iter()
        .copied()
        .zip(keep)
        .filter_map(|(point, keep)| keep.then_some(point))
        .collect::<Vec<_>>();
    let simplified = remove_near_collinear_closed_ring_vertices(simplified, tolerance_deg);
    if simplified.len() >= 3 {
        simplified
    } else {
        coordinates
    }
}

fn remove_near_collinear_closed_ring_vertices(
    mut coordinates: Vec<(f64, f64)>,
    tolerance_deg: f64,
) -> Vec<(f64, f64)> {
    if coordinates.len() <= 3 {
        return coordinates;
    }
    loop {
        let mut removed = false;
        let len = coordinates.len();
        for index in 0..len {
            let previous = coordinates[(index + len - 1) % len];
            let current = coordinates[index];
            let next = coordinates[(index + 1) % len];
            if point_line_distance_deg(current, previous, next) <= tolerance_deg {
                coordinates.remove(index);
                removed = true;
                break;
            }
        }
        if !removed || coordinates.len() <= 3 {
            return coordinates;
        }
    }
}

fn simplify_ring_segment(
    coordinates: &[(f64, f64)],
    start: usize,
    end: usize,
    tolerance_deg: f64,
    keep: &mut [bool],
) {
    let segment_indices = ring_segment_indices(coordinates.len(), start, end);
    if segment_indices.len() <= 2 {
        keep[start] = true;
        keep[end] = true;
        return;
    }
    let start_point = coordinates[start];
    let end_point = coordinates[end];
    let mut farthest_index = start;
    let mut farthest_distance = 0.0_f64;
    for &index in segment_indices
        .iter()
        .skip(1)
        .take(segment_indices.len() - 2)
    {
        let distance = point_line_distance_deg(coordinates[index], start_point, end_point);
        if distance > farthest_distance {
            farthest_distance = distance;
            farthest_index = index;
        }
    }
    keep[start] = true;
    keep[end] = true;
    if farthest_distance > tolerance_deg {
        keep[farthest_index] = true;
        simplify_ring_segment(coordinates, start, farthest_index, tolerance_deg, keep);
        simplify_ring_segment(coordinates, farthest_index, end, tolerance_deg, keep);
    }
}

fn ring_segment_indices(len: usize, start: usize, end: usize) -> Vec<usize> {
    let mut indices = vec![start];
    let mut index = start;
    while index != end {
        index = (index + 1) % len;
        indices.push(index);
    }
    indices
}

fn point_line_distance_deg(point: (f64, f64), line_start: (f64, f64), line_end: (f64, f64)) -> f64 {
    let (px, py) = point;
    let (x1, y1) = line_start;
    let (x2, y2) = line_end;
    let dx = x2 - x1;
    let dy = y2 - y1;
    let length_sq = dx * dx + dy * dy;
    if length_sq == 0.0 {
        return ((px - x1).powi(2) + (py - y1).powi(2)).sqrt();
    }
    let t = (((px - x1) * dx + (py - y1) * dy) / length_sq).clamp(0.0, 1.0);
    let proj_x = x1 + t * dx;
    let proj_y = y1 + t * dy;
    ((px - proj_x).powi(2) + (py - proj_y).powi(2)).sqrt()
}
