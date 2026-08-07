use std::io;

use super::{normalize_cartesian_to_radius, CartesianPoint};

pub(crate) fn normalized_weighted_point(
    point1: CartesianPoint,
    weight1: f64,
    point2: CartesianPoint,
    weight2: f64,
    radius: f64,
) -> io::Result<CartesianPoint> {
    let total = weight1 + weight2;
    if total == 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cannot interpolate Method-C point with zero total weight",
        ));
    }
    normalize_cartesian_to_radius(
        CartesianPoint::new(
            (point1.x * weight1 + point2.x * weight2) / total,
            (point1.y * weight1 + point2.y * weight2) / total,
            (point1.z * weight1 + point2.z * weight2) / total,
        ),
        radius,
    )
}

pub(crate) fn weighted_point(
    point1: CartesianPoint,
    weight1: f64,
    point2: CartesianPoint,
    weight2: f64,
) -> io::Result<CartesianPoint> {
    let total = weight1 + weight2;
    if total == 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cannot interpolate Method-C point with zero total weight",
        ));
    }
    Ok(CartesianPoint::new(
        (point1.x * weight1 + point2.x * weight2) / total,
        (point1.y * weight1 + point2.y * weight2) / total,
        (point1.z * weight1 + point2.z * weight2) / total,
    ))
}

pub(crate) fn normalized_face_center(
    point1: CartesianPoint,
    point2: CartesianPoint,
    point3: CartesianPoint,
    radius: f64,
) -> io::Result<CartesianPoint> {
    normalize_cartesian_to_radius(
        CartesianPoint::new(
            (point1.x + point2.x + point3.x) / 3.0,
            (point1.y + point2.y + point3.y) / 3.0,
            (point1.z + point2.z + point3.z) / 3.0,
        ),
        radius,
    )
}
