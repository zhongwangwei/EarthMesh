use crate::hydro_close_buffer::ring_area;
use crate::hydro_close_geometry_utils::*;

pub(super) fn merge_rectilinear_close_masks(
    left: &[(f64, f64)],
    right: &[(f64, f64)],
) -> Option<Vec<(f64, f64)>> {
    let mut rectangles = rectilinear_ring_cells(left)?;
    rectangles.extend(rectilinear_ring_cells(right)?);
    rectilinear_union_boundary(&rectangles)
}

pub(super) fn split_rectilinear_hole_union_cells(
    left: &[(f64, f64)],
    right: &[(f64, f64)],
) -> Option<Vec<Vec<(f64, f64)>>> {
    let mut rectangles = rectilinear_ring_cells(left)?;
    rectangles.extend(rectilinear_ring_cells(right)?);
    let boundary = rectilinear_union_boundary(&rectangles)?;
    let rectangle_area = rectangles
        .iter()
        .map(|rect| (rect.2 - rect.0) * (rect.3 - rect.1))
        .sum::<f64>();
    if ring_area(&boundary) <= rectangle_area + 1.0e-12 {
        return None;
    }
    let mut cells = rectilinear_union_cells(&rectangles)?;
    cells.sort_by(|left, right| {
        left.1
            .partial_cmp(&right.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                left.0
                    .partial_cmp(&right.0)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    Some(cells.into_iter().map(rectangle_ring).collect())
}
