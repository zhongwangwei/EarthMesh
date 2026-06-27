use crate::hydro_close_buffer::ring_area;
use crate::hydro_close_geometry_utils::*;
use crate::hydro_close_hole_spans::{
    append_non_rectilinear_hole_edge_crossing_xs, ring_y_spans_at_slab_boundary, ring_y_spans_at_x,
};

use super::support::ring_strictly_inside_rectangle;

pub(crate) fn decompose_axis_aligned_exterior_non_rectilinear_holes_vertical_slabs(
    outer: (f64, f64, f64, f64),
    holes: &[Vec<(f64, f64)>],
) -> Option<Vec<Vec<(f64, f64)>>> {
    if holes.is_empty()
        || holes.iter().any(|hole| {
            hole.len() < 3
                || is_rectilinear_ring(hole)
                || !ring_strictly_inside_rectangle(hole, outer)
        })
    {
        return None;
    }
    let mut xs = vec![outer.0, outer.2];
    for hole in holes {
        xs.extend(hole.iter().map(|(lon, _)| *lon));
    }
    append_non_rectilinear_hole_edge_crossing_xs(&mut xs, holes, outer);
    sort_dedup_f64(&mut xs);
    if xs.len() < 2 {
        return None;
    }

    let mut rings = Vec::new();
    for col in 0..(xs.len() - 1) {
        let xl = xs[col];
        let xr = xs[col + 1];
        if xr - xl <= 1.0e-12 {
            continue;
        }
        let center_x = (xl + xr) * 0.5;
        let mut spans = Vec::<((f64, f64), (f64, f64))>::new();
        for hole in holes {
            let center_spans = ring_y_spans_at_x(hole, center_x);
            if center_spans.is_empty() {
                continue;
            }
            let left_spans = ring_y_spans_at_slab_boundary(hole, xl, center_x, center_spans.len())?;
            let right_spans =
                ring_y_spans_at_slab_boundary(hole, xr, center_x, center_spans.len())?;
            if left_spans.len() != center_spans.len() || right_spans.len() != center_spans.len() {
                return None;
            }
            spans.extend(left_spans.into_iter().zip(right_spans));
        }
        if spans.is_empty() {
            rings.push(rectangle_ring((xl, outer.1, xr, outer.3)));
            continue;
        }
        spans.sort_by(|left, right| {
            let left_center = (left.0 .0 + left.0 .1 + left.1 .0 + left.1 .1) * 0.25;
            let right_center = (right.0 .0 + right.0 .1 + right.1 .0 + right.1 .1) * 0.25;
            left_center
                .partial_cmp(&right_center)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut current_left = outer.1;
        let mut current_right = outer.1;
        for ((left_low, left_high), (right_low, right_high)) in spans {
            let starts_above_current =
                left_low >= current_left - 1.0e-12 && right_low >= current_right - 1.0e-12;
            let overlaps_current =
                left_low <= current_left + 1.0e-12 && right_low <= current_right + 1.0e-12;
            if starts_above_current {
                let segment = vec![
                    (xl, current_left),
                    (xr, current_right),
                    (xr, right_low),
                    (xl, left_low),
                ];
                if ring_area(&segment).abs() > 1.0e-12 {
                    rings.push(segment);
                }
                current_left = left_high;
                current_right = right_high;
            } else if overlaps_current {
                current_left = current_left.max(left_high);
                current_right = current_right.max(right_high);
            } else {
                return None;
            }
        }
        let top_segment = vec![
            (xl, current_left),
            (xr, current_right),
            (xr, outer.3),
            (xl, outer.3),
        ];
        if ring_area(&top_segment).abs() > 1.0e-12 {
            rings.push(top_segment);
        }
    }
    (!rings.is_empty()).then_some(rings)
}
