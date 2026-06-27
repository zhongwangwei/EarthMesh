use crate::hydro_close_geometry_utils::point_in_rectangle;

pub(super) fn ring_rectangle_strictly_inside_rectangle(
    inner: (f64, f64, f64, f64),
    outer: (f64, f64, f64, f64),
) -> bool {
    inner.0 > outer.0 && inner.1 > outer.1 && inner.2 < outer.2 && inner.3 < outer.3
}

pub(super) fn ring_strictly_inside_rectangle(
    ring: &[(f64, f64)],
    rectangle: (f64, f64, f64, f64),
) -> bool {
    ring.iter()
        .all(|point| point_in_rectangle(*point, rectangle))
}
