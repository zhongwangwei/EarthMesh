use crate::hydro_close_geometry_utils::*;
use crate::hydro_close_hole_spans::{
    append_non_rectilinear_hole_edge_crossing_xs, ring_y_spans_at_slab_boundary, ring_y_spans_at_x,
};

use super::support::push_vertical_slab_ring;

pub(crate) fn decompose_non_axis_aligned_exterior_holes_vertical_slabs(
    exterior: &[(f64, f64)],
    holes: &[Vec<(f64, f64)>],
) -> Option<Vec<Vec<(f64, f64)>>> {
    if holes.is_empty()
        || exterior.len() < 3
        || !exterior
            .iter()
            .all(|(lon, lat)| lon.is_finite() && lat.is_finite())
        || axis_aligned_rectangle_from_ring(exterior).is_some()
        || holes.iter().any(|hole| {
            hole.len() < 3
                || !hole
                    .iter()
                    .all(|(lon, lat)| lon.is_finite() && lat.is_finite())
                || !ring_contains_ring(exterior, hole)
        })
    {
        return None;
    }

    let exterior_bbox = ring_bbox(exterior);
    let mut xs = exterior.iter().map(|(lon, _)| *lon).collect::<Vec<_>>();
    for hole in holes {
        xs.extend(hole.iter().map(|(lon, _)| *lon));
    }
    append_non_rectilinear_hole_edge_crossing_xs(&mut xs, holes, exterior_bbox);
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
        let exterior_center_spans = ring_y_spans_at_x(exterior, center_x);
        if exterior_center_spans.is_empty() {
            continue;
        }
        let exterior_left_spans =
            ring_y_spans_at_slab_boundary(exterior, xl, center_x, exterior_center_spans.len())?;
        let exterior_right_spans =
            ring_y_spans_at_slab_boundary(exterior, xr, center_x, exterior_center_spans.len())?;
        if exterior_left_spans.len() != exterior_center_spans.len()
            || exterior_right_spans.len() != exterior_center_spans.len()
        {
            return None;
        }

        let mut hole_spans = Vec::<((f64, f64), (f64, f64), (f64, f64))>::new();
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
            for (center, left, right) in center_spans
                .into_iter()
                .zip(left_spans)
                .zip(right_spans)
                .map(|((center, left), right)| (center, left, right))
            {
                hole_spans.push((center, left, right));
            }
        }

        for span_index in 0..exterior_center_spans.len() {
            let (outer_center_low, outer_center_high) = exterior_center_spans[span_index];
            let (outer_left_low, outer_left_high) = exterior_left_spans[span_index];
            let (outer_right_low, outer_right_high) = exterior_right_spans[span_index];
            let mut spans = hole_spans
                .iter()
                .copied()
                .filter(|(center, left, right)| {
                    center.0 >= outer_center_low - 1.0e-12
                        && center.1 <= outer_center_high + 1.0e-12
                        && left.0 >= outer_left_low - 1.0e-12
                        && left.1 <= outer_left_high + 1.0e-12
                        && right.0 >= outer_right_low - 1.0e-12
                        && right.1 <= outer_right_high + 1.0e-12
                })
                .map(|(_, left, right)| (left, right))
                .collect::<Vec<_>>();
            spans.sort_by(|left, right| {
                let left_center = (left.0 .0 + left.0 .1 + left.1 .0 + left.1 .1) * 0.25;
                let right_center = (right.0 .0 + right.0 .1 + right.1 .0 + right.1 .1) * 0.25;
                left_center
                    .partial_cmp(&right_center)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let mut current_left = outer_left_low;
            let mut current_right = outer_right_low;
            for ((left_low, left_high), (right_low, right_high)) in spans {
                let starts_above_current =
                    left_low >= current_left - 1.0e-12 && right_low >= current_right - 1.0e-12;
                let overlaps_current =
                    left_low <= current_left + 1.0e-12 && right_low <= current_right + 1.0e-12;
                if starts_above_current {
                    push_vertical_slab_ring(
                        &mut rings,
                        xl,
                        xr,
                        (current_left, current_right),
                        (left_low, right_low),
                    );
                    current_left = left_high;
                    current_right = right_high;
                } else if overlaps_current {
                    current_left = current_left.max(left_high);
                    current_right = current_right.max(right_high);
                } else {
                    return None;
                }
            }
            push_vertical_slab_ring(
                &mut rings,
                xl,
                xr,
                (current_left, current_right),
                (outer_left_high, outer_right_high),
            );
        }
    }
    (!rings.is_empty()).then_some(rings)
}
