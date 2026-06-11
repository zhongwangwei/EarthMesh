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
