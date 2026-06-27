use crate::hydro_close_geometry_utils::*;
use crate::hydro_close_hole_slabs::{
    decompose_axis_aligned_exterior_non_rectilinear_holes_vertical_slabs,
    decompose_axis_aligned_exterior_triangular_hole_vertical_slabs,
    decompose_axis_aligned_exterior_triangular_holes_vertical_slabs,
};

use super::support::{ring_rectangle_strictly_inside_rectangle, ring_strictly_inside_rectangle};
use super::triangular::{
    decompose_axis_aligned_exterior_horizontal_base_triangular_hole,
    decompose_axis_aligned_exterior_vertical_base_triangular_hole,
};

pub(crate) fn decompose_axis_aligned_rectangular_hole(
    exterior: &[(f64, f64)],
    holes: &[Vec<(f64, f64)>],
) -> Option<Vec<Vec<(f64, f64)>>> {
    if holes.is_empty() {
        return Some(vec![exterior.to_vec()]);
    }
    let outer = axis_aligned_rectangle_from_ring(exterior)?;
    let inner_rectangles = holes
        .iter()
        .map(|hole| axis_aligned_rectangle_from_ring(hole))
        .collect::<Option<Vec<_>>>();
    if let Some(inner_rectangles) = inner_rectangles {
        if inner_rectangles
            .iter()
            .all(|inner| ring_rectangle_strictly_inside_rectangle(*inner, outer))
        {
            if inner_rectangles.len() == 1 {
                let inner = inner_rectangles[0];
                let rectangles = [
                    (outer.0, outer.1, outer.2, inner.1),
                    (outer.0, inner.3, outer.2, outer.3),
                    (outer.0, inner.1, inner.0, inner.3),
                    (inner.2, inner.1, outer.2, inner.3),
                ];
                let rings = rectangles
                    .into_iter()
                    .filter(|rect| rect.2 - rect.0 > 1.0e-12 && rect.3 - rect.1 > 1.0e-12)
                    .map(rectangle_ring)
                    .collect::<Vec<_>>();
                return (!rings.is_empty()).then_some(rings);
            }
            return decompose_axis_aligned_exterior_rectilinear_holes_grid(outer, holes);
        }
    }
    if holes.len() == 1 {
        if let Some(rings) =
            decompose_axis_aligned_exterior_horizontal_base_triangular_hole(outer, &holes[0])
        {
            return Some(rings);
        }
        if let Some(rings) =
            decompose_axis_aligned_exterior_vertical_base_triangular_hole(outer, &holes[0])
        {
            return Some(rings);
        }
        if let Some(rings) =
            decompose_axis_aligned_exterior_triangular_hole_vertical_slabs(outer, &holes[0])
        {
            return Some(rings);
        }
    }
    if let Some(rings) =
        decompose_axis_aligned_exterior_triangular_holes_vertical_slabs(outer, holes)
    {
        return Some(rings);
    }
    if let Some(rings) =
        decompose_axis_aligned_exterior_non_rectilinear_holes_vertical_slabs(outer, holes)
    {
        return Some(rings);
    }
    decompose_axis_aligned_exterior_rectilinear_holes_grid(outer, holes)
}

fn decompose_axis_aligned_exterior_rectilinear_holes_grid(
    outer: (f64, f64, f64, f64),
    holes: &[Vec<(f64, f64)>],
) -> Option<Vec<Vec<(f64, f64)>>> {
    if holes
        .iter()
        .any(|hole| !is_rectilinear_ring(hole) || !ring_strictly_inside_rectangle(hole, outer))
    {
        return None;
    }
    let mut xs = vec![outer.0, outer.2];
    let mut ys = vec![outer.1, outer.3];
    for hole in holes {
        xs.extend(hole.iter().map(|(lon, _)| *lon));
        ys.extend(hole.iter().map(|(_, lat)| *lat));
    }
    sort_dedup_f64(&mut xs);
    sort_dedup_f64(&mut ys);
    if xs.len() < 2 || ys.len() < 2 {
        return None;
    }

    let mut rings = Vec::new();
    for row in 0..(ys.len() - 1) {
        for col in 0..(xs.len() - 1) {
            let rect = (xs[col], ys[row], xs[col + 1], ys[row + 1]);
            if rect.2 - rect.0 <= 1.0e-12 || rect.3 - rect.1 <= 1.0e-12 {
                continue;
            }
            let center = ((rect.0 + rect.2) * 0.5, (rect.1 + rect.3) * 0.5);
            if !point_in_rectangle(center, outer)
                || holes.iter().any(|hole| point_in_ring(center, hole))
            {
                continue;
            }
            rings.push(rectangle_ring(rect));
        }
    }
    (!rings.is_empty()).then_some(rings)
}
