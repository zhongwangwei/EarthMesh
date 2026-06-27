use std::collections::BTreeMap;

use crate::hydro_close_geometry::{line_intersection, remove_collinear_boundary_vertices};

use super::area::ring_area;

pub(crate) fn buffer_close_mask_line_for_refine_degree(
    coordinates: &[(f64, f64)],
    refine_degree: usize,
    buffer_deg_by_refine_degree: &BTreeMap<usize, f64>,
) -> Option<Vec<(f64, f64)>> {
    let buffer_deg = buffer_deg_by_refine_degree.get(&refine_degree).copied()?;
    if buffer_deg <= 0.0 {
        return None;
    }
    buffer_open_line_to_ring(coordinates, buffer_deg)
}

fn buffer_open_line_to_ring(
    coordinates: &[(f64, f64)],
    buffer_deg: f64,
) -> Option<Vec<(f64, f64)>> {
    if coordinates.len() < 2 || buffer_deg <= 0.0 {
        return None;
    }
    let mut normals = Vec::with_capacity(coordinates.len() - 1);
    for pair in coordinates.windows(2) {
        let dx = pair[1].0 - pair[0].0;
        let dy = pair[1].1 - pair[0].1;
        let length = (dx * dx + dy * dy).sqrt();
        if length <= f64::EPSILON {
            return None;
        }
        normals.push((-dy / length, dx / length));
    }

    let mut left_side = Vec::with_capacity(coordinates.len());
    let mut right_side = Vec::with_capacity(coordinates.len());
    for index in 0..coordinates.len() {
        left_side.push(buffered_polyline_vertex(
            coordinates,
            &normals,
            index,
            buffer_deg,
            1.0,
        )?);
        right_side.push(buffered_polyline_vertex(
            coordinates,
            &normals,
            index,
            buffer_deg,
            -1.0,
        )?);
    }
    right_side.reverse();
    left_side.extend(right_side);
    let ring = remove_collinear_boundary_vertices(left_side);
    (ring.len() >= 3 && ring_area(&ring) > 1.0e-12).then_some(ring)
}

fn buffered_polyline_vertex(
    coordinates: &[(f64, f64)],
    normals: &[(f64, f64)],
    index: usize,
    buffer_deg: f64,
    side: f64,
) -> Option<(f64, f64)> {
    let point = coordinates[index];
    if index == 0 {
        return Some((
            point.0 + normals[0].0 * buffer_deg * side,
            point.1 + normals[0].1 * buffer_deg * side,
        ));
    }
    if index + 1 == coordinates.len() {
        let normal = normals[index - 1];
        return Some((
            point.0 + normal.0 * buffer_deg * side,
            point.1 + normal.1 * buffer_deg * side,
        ));
    }
    let previous = normals[index - 1];
    let current = normals[index];
    let previous_start = (
        coordinates[index - 1].0 + previous.0 * buffer_deg * side,
        coordinates[index - 1].1 + previous.1 * buffer_deg * side,
    );
    let previous_end = (
        point.0 + previous.0 * buffer_deg * side,
        point.1 + previous.1 * buffer_deg * side,
    );
    let current_start = (
        point.0 + current.0 * buffer_deg * side,
        point.1 + current.1 * buffer_deg * side,
    );
    let current_end = (
        coordinates[index + 1].0 + current.0 * buffer_deg * side,
        coordinates[index + 1].1 + current.1 * buffer_deg * side,
    );
    if let Some(intersection) =
        line_intersection(previous_start, previous_end, current_start, current_end)
    {
        return intersection
            .0
            .is_finite()
            .then_some(intersection)
            .filter(|point| point.1.is_finite());
    }
    let average = (previous.0 + current.0, previous.1 + current.1);
    let length = (average.0 * average.0 + average.1 * average.1).sqrt();
    if length <= f64::EPSILON {
        Some((
            point.0 + current.0 * buffer_deg * side,
            point.1 + current.1 * buffer_deg * side,
        ))
    } else {
        Some((
            point.0 + average.0 / length * buffer_deg * side,
            point.1 + average.1 / length * buffer_deg * side,
        ))
    }
}
