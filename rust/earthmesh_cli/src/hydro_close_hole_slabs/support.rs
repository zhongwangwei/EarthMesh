use crate::hydro_close_buffer::ring_area;
use crate::hydro_close_geometry_utils::point_in_rectangle;

pub(super) fn push_vertical_slab_ring(
    rings: &mut Vec<Vec<(f64, f64)>>,
    xl: f64,
    xr: f64,
    lower: (f64, f64),
    upper: (f64, f64),
) {
    let segment = vec![(xl, lower.0), (xr, lower.1), (xr, upper.1), (xl, upper.0)];
    if ring_area(&segment).abs() > 1.0e-12 {
        rings.push(segment);
    }
}

pub(super) fn ring_strictly_inside_rectangle(
    ring: &[(f64, f64)],
    rectangle: (f64, f64, f64, f64),
) -> bool {
    ring.iter()
        .all(|point| point_in_rectangle(*point, rectangle))
}
