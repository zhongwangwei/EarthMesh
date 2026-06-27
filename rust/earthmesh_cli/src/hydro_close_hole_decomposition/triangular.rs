use crate::hydro_close_buffer::ring_area;

use super::support::ring_strictly_inside_rectangle;

pub(super) fn decompose_axis_aligned_exterior_horizontal_base_triangular_hole(
    outer: (f64, f64, f64, f64),
    hole: &[(f64, f64)],
) -> Option<Vec<Vec<(f64, f64)>>> {
    if hole.len() != 3 || !ring_strictly_inside_rectangle(hole, outer) {
        return None;
    }
    let mut base = None;
    let mut apex = None;
    for index in 0..3 {
        let left = hole[index];
        let right = hole[(index + 1) % 3];
        let candidate_apex = hole[(index + 2) % 3];
        if (left.1 - right.1).abs() <= 1.0e-12 {
            base = Some(if left.0 <= right.0 {
                (left, right)
            } else {
                (right, left)
            });
            apex = Some(candidate_apex);
            break;
        }
    }
    let ((base_left, base_right), apex) = (base?, apex?);
    if base_left.0 >= apex.0 || apex.0 >= base_right.0 {
        return None;
    }
    let base_y = base_left.1;
    let apex_y = apex.1;
    if (apex_y - base_y).abs() <= 1.0e-12 {
        return None;
    }
    let rectangles_and_polygons = if apex_y > base_y {
        [
            vec![
                (outer.0, outer.1),
                (outer.2, outer.1),
                (outer.2, base_y),
                (outer.0, base_y),
            ],
            vec![
                (outer.0, apex_y),
                (outer.2, apex_y),
                (outer.2, outer.3),
                (outer.0, outer.3),
            ],
            vec![(outer.0, base_y), base_left, apex, (outer.0, apex_y)],
            vec![base_right, (outer.2, base_y), (outer.2, apex_y), apex],
        ]
    } else {
        [
            vec![
                (outer.0, outer.1),
                (outer.2, outer.1),
                (outer.2, apex_y),
                (outer.0, apex_y),
            ],
            vec![
                (outer.0, base_y),
                (outer.2, base_y),
                (outer.2, outer.3),
                (outer.0, outer.3),
            ],
            vec![(outer.0, apex_y), apex, base_left, (outer.0, base_y)],
            vec![apex, (outer.2, apex_y), (outer.2, base_y), base_right],
        ]
    };
    let rings = rectangles_and_polygons
        .into_iter()
        .filter(|ring| ring_area(ring).abs() > 1.0e-12)
        .collect::<Vec<_>>();
    (!rings.is_empty()).then_some(rings)
}

pub(super) fn decompose_axis_aligned_exterior_vertical_base_triangular_hole(
    outer: (f64, f64, f64, f64),
    hole: &[(f64, f64)],
) -> Option<Vec<Vec<(f64, f64)>>> {
    if hole.len() != 3 || !ring_strictly_inside_rectangle(hole, outer) {
        return None;
    }
    let mut base = None;
    let mut apex = None;
    for index in 0..3 {
        let bottom = hole[index];
        let top = hole[(index + 1) % 3];
        let candidate_apex = hole[(index + 2) % 3];
        if (bottom.0 - top.0).abs() <= 1.0e-12 {
            base = Some(if bottom.1 <= top.1 {
                (bottom, top)
            } else {
                (top, bottom)
            });
            apex = Some(candidate_apex);
            break;
        }
    }
    let ((base_bottom, base_top), apex) = (base?, apex?);
    if base_bottom.1 >= apex.1 || apex.1 >= base_top.1 {
        return None;
    }
    let base_x = base_bottom.0;
    let apex_x = apex.0;
    if (apex_x - base_x).abs() <= 1.0e-12 {
        return None;
    }
    let rectangles_and_polygons = if apex_x > base_x {
        [
            vec![
                (outer.0, outer.1),
                (base_x, outer.1),
                (base_x, outer.3),
                (outer.0, outer.3),
            ],
            vec![
                (apex_x, outer.1),
                (outer.2, outer.1),
                (outer.2, outer.3),
                (apex_x, outer.3),
            ],
            vec![(base_x, outer.1), (apex_x, outer.1), apex, base_bottom],
            vec![base_top, apex, (apex_x, outer.3), (base_x, outer.3)],
        ]
    } else {
        [
            vec![
                (outer.0, outer.1),
                (apex_x, outer.1),
                (apex_x, outer.3),
                (outer.0, outer.3),
            ],
            vec![
                (base_x, outer.1),
                (outer.2, outer.1),
                (outer.2, outer.3),
                (base_x, outer.3),
            ],
            vec![(apex_x, outer.1), (base_x, outer.1), base_bottom, apex],
            vec![apex, base_top, (base_x, outer.3), (apex_x, outer.3)],
        ]
    };
    let rings = rectangles_and_polygons
        .into_iter()
        .filter(|ring| ring_area(ring).abs() > 1.0e-12)
        .collect::<Vec<_>>();
    (!rings.is_empty()).then_some(rings)
}
