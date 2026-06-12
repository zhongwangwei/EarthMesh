#[cfg(feature = "extension-module")]
use pyo3::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}


/// Earth radius in kilometers used by `MOD_Area_judge:haversine`.
///
/// The Fortran routine initializes `erad = 6371229` meters and computes
/// `erad / 1000 * central_angle`.
pub const EARTH_RADIUS_KM: f64 = 6_371.229;

/// Port of `MOD_Area_judge:cross_product`.
#[inline]
pub fn cross_product_2d(p1: Point, p2: Point, p3: Point) -> f64 {
    (p2.x - p1.x) * (p3.y - p1.y) - (p2.y - p1.y) * (p3.x - p1.x)
}

/// Port of `MOD_Area_judge:haversine`.
///
/// Inputs use the same convention as the Fortran `point_i(2)` arrays:
/// `x = longitude degrees`, `y = latitude degrees`. The return value is km.
pub fn haversine_km(point_i: Point, point_c: Point) -> f64 {
    let px1 = point_i.x.to_radians();
    let py1 = point_i.y.to_radians();
    let px2 = point_c.x.to_radians();
    let py2 = point_c.y.to_radians();

    let v = (py1 / 2.0 - py2 / 2.0).sin().powi(2)
        + py2.cos() * py1.cos() * (px1 / 2.0 - px2 / 2.0).sin().powi(2);
    EARTH_RADIUS_KM * 2.0 * v.sqrt().atan2((1.0 - v).sqrt())
}

/// Port of `MOD_Area_judge:is_point_in_circle`.
#[inline]
pub fn is_point_in_circle_km(point: Point, center: Point, center_radius_km: f64) -> bool {
    haversine_km(point, center) <= center_radius_km
}

/// Port of `MOD_Area_judge:is_point_in_convex_polygon`.
///
/// Boundary points are considered inside, matching the Fortran behavior where
/// zero cross products do not flip the sign test.
pub fn is_point_in_convex_polygon(polygon: &[Point], point: Point) -> bool {
    if polygon.len() < 3 {
        return false;
    }

    let mut prev_cross = 0.0;
    for i in 0..polygon.len() {
        let p1 = polygon[i];
        let p2 = polygon[(i + 1) % polygon.len()];
        let cross = cross_product_2d(p1, p2, point);
        if cross != 0.0 {
            if prev_cross == 0.0 {
                prev_cross = cross;
            } else if (prev_cross > 0.0) != (cross > 0.0) {
                return false;
            }
        }
    }
    true
}

pub fn polygon_area(polygon: &[Point]) -> f64 {
    if polygon.len() < 3 {
        return 0.0;
    }
    let mut total = 0.0;
    for (index, point) in polygon.iter().enumerate() {
        let next = polygon[(index + 1) % polygon.len()];
        total += point.x * next.y - next.x * point.y;
    }
    total.abs() * 0.5
}

pub fn clip_convex_polygon(subject: &[Point], clip: &[Point]) -> Vec<Point> {
    let mut output = subject.to_vec();
    let clip_ccw = signed_area(clip) >= 0.0;
    for (index, edge_start) in clip.iter().enumerate() {
        let edge_end = clip[(index + 1) % clip.len()];
        let input = output;
        output = Vec::new();
        if input.is_empty() {
            break;
        }
        let mut previous = *input.last().expect("non-empty input polygon");
        for current in input {
            let current_inside = inside(current, *edge_start, edge_end, clip_ccw);
            let previous_inside = inside(previous, *edge_start, edge_end, clip_ccw);
            if current_inside {
                if !previous_inside {
                    output.push(line_intersection(previous, current, *edge_start, edge_end));
                }
                output.push(current);
            } else if previous_inside {
                output.push(line_intersection(previous, current, *edge_start, edge_end));
            }
            previous = current;
        }
    }
    output
}

pub fn intersection_area(a: &[Point], b: &[Point]) -> f64 {
    polygon_area(&clip_convex_polygon(a, b))
}

fn signed_area(polygon: &[Point]) -> f64 {
    if polygon.len() < 3 {
        return 0.0;
    }
    let mut total = 0.0;
    for (index, point) in polygon.iter().enumerate() {
        let next = polygon[(index + 1) % polygon.len()];
        total += point.x * next.y - next.x * point.y;
    }
    total * 0.5
}

fn inside(point: Point, edge_start: Point, edge_end: Point, clip_ccw: bool) -> bool {
    let cross = (edge_end.x - edge_start.x) * (point.y - edge_start.y)
        - (edge_end.y - edge_start.y) * (point.x - edge_start.x);
    if clip_ccw {
        cross >= -1.0e-12
    } else {
        cross <= 1.0e-12
    }
}

fn line_intersection(a0: Point, a1: Point, b0: Point, b1: Point) -> Point {
    let denominator = (a0.x - a1.x) * (b0.y - b1.y) - (a0.y - a1.y) * (b0.x - b1.x);
    if denominator.abs() < 1.0e-12 {
        return a1;
    }
    let a_cross = a0.x * a1.y - a0.y * a1.x;
    let b_cross = b0.x * b1.y - b0.y * b1.x;
    Point::new(
        (a_cross * (b0.x - b1.x) - (a0.x - a1.x) * b_cross) / denominator,
        (a_cross * (b0.y - b1.y) - (a0.y - a1.y) * b_cross) / denominator,
    )
}

#[cfg(test)]
mod tests {
    use super::{intersection_area, polygon_area, Point};

    #[test]
    fn empty_or_short_polygons_have_zero_area() {
        assert_eq!(polygon_area(&[]), 0.0);
        assert_eq!(
            polygon_area(&[Point::new(0.0, 0.0), Point::new(1.0, 1.0)]),
            0.0
        );
    }

    #[test]
    fn nested_rectangle_intersection_keeps_inner_area() {
        let outer = vec![
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(4.0, 4.0),
            Point::new(0.0, 4.0),
        ];
        let inner = vec![
            Point::new(1.0, 1.0),
            Point::new(2.0, 1.0),
            Point::new(2.0, 2.0),
            Point::new(1.0, 2.0),
        ];

        assert_eq!(intersection_area(&outer, &inner), 1.0);
    }
}

#[cfg(feature = "extension-module")]
#[pyfunction(name = "polygon_area")]
fn py_polygon_area(vertices: Vec<(f64, f64)>) -> f64 {
    let points = tuples_to_points(vertices);
    polygon_area(&points)
}

#[cfg(feature = "extension-module")]
#[pyfunction(name = "intersection_area")]
fn py_intersection_area(subject: Vec<(f64, f64)>, clip: Vec<(f64, f64)>) -> f64 {
    let subject_points = tuples_to_points(subject);
    let clip_points = tuples_to_points(clip);
    intersection_area(&subject_points, &clip_points)
}

#[cfg(feature = "extension-module")]
fn tuples_to_points(vertices: Vec<(f64, f64)>) -> Vec<Point> {
    vertices
        .into_iter()
        .map(|(x, y)| Point::new(x, y))
        .collect()
}

#[cfg(feature = "extension-module")]
#[pymodule]
fn earthmesh_geometry(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(py_polygon_area, module)?)?;
    module.add_function(wrap_pyfunction!(py_intersection_area, module)?)?;
    Ok(())
}

/// Port of `MOD_Area_judge:ray_segment_intersect`.
///
/// Fortran returns the ray start longitude as a sentinel for no intersection.
/// Rust exposes that sentinel as `None` while keeping the same intersection math.
pub fn ray_segment_intersection_lon(
    ray_start: Point,
    lat1: f64,
    lon1: f64,
    lat2: f64,
    lon2: f64,
) -> Option<f64> {
    let lon_p = ray_start.x;
    let lat_p = ray_start.y;

    if lat1 == lat2 {
        return None;
    }
    if (lat1 > lat_p && lat2 > lat_p) || (lat1 < lat_p && lat2 < lat_p) {
        return None;
    }

    let m = (lat2 - lat1) / (lon2 - lon1);
    let lon_intersect = lon1 + (lat_p - lat1) / m;
    if lon_intersect == lon_p {
        None
    } else {
        Some(lon_intersect)
    }
}

/// Port of `MOD_Area_judge:cross_product2`.
#[inline]
pub fn cross_product_components(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    x1 * y2 - x2 * y1
}

/// Port of `MOD_Area_judge:segments_intersect`.
///
/// This intentionally uses the strict Fortran rule (`< 0` products), so
/// endpoint touches and collinear overlaps are not intersections.
pub fn segments_intersect_strict(a1: Point, a2: Point, b1: Point, b2: Point) -> bool {
    let cp1 = cross_product_components(a2.x - a1.x, a2.y - a1.y, b1.x - a1.x, b1.y - a1.y);
    let cp2 = cross_product_components(a2.x - a1.x, a2.y - a1.y, b2.x - a1.x, b2.y - a1.y);
    let cp3 = cross_product_components(b2.x - b1.x, b2.y - b1.y, a1.x - b1.x, a1.y - b1.y);
    let cp4 = cross_product_components(b2.x - b1.x, b2.y - b1.y, a2.x - b1.x, a2.y - b1.y);
    cp1 * cp2 < 0.0 && cp3 * cp4 < 0.0
}

/// Port of `MOD_Area_judge:CheckCrossing`.
///
/// This shifts longitudes by 180 degrees when a closed polygon crosses the
/// dateline so ray-intersection intervals can be processed on a continuous axis.
pub fn shift_longitudes_for_dateline_crossing(points: &[Point]) -> Vec<Point> {
    points
        .iter()
        .map(|point| {
            let shifted_lon = if point.x < 0.0 { point.x + 180.0 } else { point.x - 180.0 };
            Point::new(shifted_lon, point.y)
        })
        .collect()
}
