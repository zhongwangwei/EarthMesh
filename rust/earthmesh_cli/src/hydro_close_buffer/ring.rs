use std::collections::BTreeMap;

use crate::hydro_close_geometry::{line_intersection, ring_bbox};

use super::area::signed_ring_area;

pub(crate) fn buffer_close_mask_ring_for_refine_degree(
    coordinates: &[(f64, f64)],
    refine_degree: usize,
    buffer_deg_by_refine_degree: &BTreeMap<usize, f64>,
) -> Vec<(f64, f64)> {
    let buffer_deg = buffer_deg_by_refine_degree
        .get(&refine_degree)
        .copied()
        .unwrap_or(0.0);
    if buffer_deg <= 0.0 || coordinates.len() < 3 {
        return coordinates.to_vec();
    }
    offset_close_mask_ring(coordinates, buffer_deg).unwrap_or_else(|| {
        let (min_lon, min_lat, max_lon, max_lat) = ring_bbox(coordinates);
        vec![
            (min_lon - buffer_deg, min_lat - buffer_deg),
            (max_lon + buffer_deg, min_lat - buffer_deg),
            (max_lon + buffer_deg, max_lat + buffer_deg),
            (min_lon - buffer_deg, max_lat + buffer_deg),
        ]
    })
}

fn offset_close_mask_ring(coordinates: &[(f64, f64)], offset_deg: f64) -> Option<Vec<(f64, f64)>> {
    if coordinates.len() < 3 || offset_deg <= 0.0 {
        return Some(coordinates.to_vec());
    }
    let signed_area = signed_ring_area(coordinates);
    if signed_area.abs() <= f64::EPSILON {
        return None;
    }
    let clockwise = signed_area < 0.0;
    let mut shifted_edges = Vec::with_capacity(coordinates.len());
    for index in 0..coordinates.len() {
        let start = coordinates[index];
        let end = coordinates[(index + 1) % coordinates.len()];
        let dx = end.0 - start.0;
        let dy = end.1 - start.1;
        let length = (dx * dx + dy * dy).sqrt();
        if length <= f64::EPSILON {
            return None;
        }
        let outward = if clockwise {
            (-dy / length, dx / length)
        } else {
            (dy / length, -dx / length)
        };
        shifted_edges.push((
            (
                start.0 + outward.0 * offset_deg,
                start.1 + outward.1 * offset_deg,
            ),
            (
                end.0 + outward.0 * offset_deg,
                end.1 + outward.1 * offset_deg,
            ),
            outward,
        ));
    }
    let mut offset_ring = Vec::with_capacity(coordinates.len());
    for index in 0..coordinates.len() {
        let previous = &shifted_edges[(index + shifted_edges.len() - 1) % shifted_edges.len()];
        let current = &shifted_edges[index];
        let point = line_intersection(previous.0, previous.1, current.0, current.1).unwrap_or((
            coordinates[index].0 + (previous.2 .0 + current.2 .0) * 0.5 * offset_deg,
            coordinates[index].1 + (previous.2 .1 + current.2 .1) * 0.5 * offset_deg,
        ));
        if !point.0.is_finite() || !point.1.is_finite() {
            return None;
        }
        offset_ring.push(point);
    }
    Some(offset_ring)
}
