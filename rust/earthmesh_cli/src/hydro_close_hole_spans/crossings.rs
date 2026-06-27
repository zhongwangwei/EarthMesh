use crate::hydro_close_geometry_utils::segment_intersection_point;

use super::spans::{line_segment_y_at_x, ring_y_spans_at_x};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RingVerticalBoundaryRole {
    Low,
    High,
}

pub(crate) fn append_non_rectilinear_hole_edge_crossing_xs(
    xs: &mut Vec<f64>,
    holes: &[Vec<(f64, f64)>],
    outer: (f64, f64, f64, f64),
) {
    for left_hole_index in 0..holes.len() {
        for right_hole_index in (left_hole_index + 1)..holes.len() {
            let left_hole = &holes[left_hole_index];
            let right_hole = &holes[right_hole_index];
            for left_edge_index in 0..left_hole.len() {
                let left_start = left_hole[left_edge_index];
                let left_end = left_hole[(left_edge_index + 1) % left_hole.len()];
                let Some(left_role) =
                    ring_edge_vertical_boundary_role(left_hole, left_start, left_end)
                else {
                    continue;
                };
                for right_edge_index in 0..right_hole.len() {
                    let right_start = right_hole[right_edge_index];
                    let right_end = right_hole[(right_edge_index + 1) % right_hole.len()];
                    let Some(right_role) =
                        ring_edge_vertical_boundary_role(right_hole, right_start, right_end)
                    else {
                        continue;
                    };
                    if left_role == right_role {
                        continue;
                    }
                    let Some((x, _)) =
                        segment_intersection_point(left_start, left_end, right_start, right_end)
                    else {
                        continue;
                    };
                    if x > outer.0 + 1.0e-12 && x < outer.2 - 1.0e-12 {
                        xs.push(x);
                    }
                }
            }
        }
    }
}

fn ring_edge_vertical_boundary_role(
    ring: &[(f64, f64)],
    start: (f64, f64),
    end: (f64, f64),
) -> Option<RingVerticalBoundaryRole> {
    if (end.0 - start.0).abs() <= 1.0e-12 {
        return None;
    }
    let sample_x = (start.0 + end.0) * 0.5;
    let edge_y = line_segment_y_at_x(start, end, sample_x)?;
    let spans = ring_y_spans_at_x(ring, sample_x);
    for (low, high) in spans {
        if (edge_y - low).abs() <= 1.0e-9 {
            return Some(RingVerticalBoundaryRole::Low);
        }
        if (edge_y - high).abs() <= 1.0e-9 {
            return Some(RingVerticalBoundaryRole::High);
        }
    }
    None
}
