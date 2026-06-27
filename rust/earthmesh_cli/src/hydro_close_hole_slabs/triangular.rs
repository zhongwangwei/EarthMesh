use crate::hydro_close_buffer::ring_area;
use crate::hydro_close_geometry_utils::*;
use crate::hydro_close_hole_spans::triangle_y_span_at_x;

use super::support::ring_strictly_inside_rectangle;

pub(crate) fn decompose_axis_aligned_exterior_triangular_hole_vertical_slabs(
    outer: (f64, f64, f64, f64),
    hole: &[(f64, f64)],
) -> Option<Vec<Vec<(f64, f64)>>> {
    decompose_axis_aligned_exterior_triangular_holes_vertical_slabs(outer, &[hole.to_vec()])
}

pub(crate) fn decompose_axis_aligned_exterior_triangular_holes_vertical_slabs(
    outer: (f64, f64, f64, f64),
    holes: &[Vec<(f64, f64)>],
) -> Option<Vec<Vec<(f64, f64)>>> {
    if holes.is_empty()
        || holes
            .iter()
            .any(|hole| hole.len() != 3 || !ring_strictly_inside_rectangle(hole, outer))
    {
        return None;
    }
    let mut xs = vec![outer.0, outer.2];
    for hole in holes {
        xs.extend(hole.iter().map(|(lon, _)| *lon));
    }
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
            if triangle_y_span_at_x(hole, center_x).is_none() {
                continue;
            }
            let left_span = triangle_y_span_at_x(hole, xl)?;
            let right_span = triangle_y_span_at_x(hole, xr)?;
            spans.push((left_span, right_span));
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
