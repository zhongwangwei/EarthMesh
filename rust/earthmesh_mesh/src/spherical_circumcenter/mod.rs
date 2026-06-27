use earthmesh_core::EARTH_RADIUS_METERS;

use crate::coordinates::{magnitude, CartesianPoint};
use crate::{
    project_to_polar_stereographic_with_radius, unproject_from_polar_stereographic_with_radius,
    PlanePoint, PoleBasis,
};

/// Port of one iteration of `MOD_grid_preprocess:circumcenter_spherical_calculation`.
///
/// `barycenter` and `vertices` are Earth-radius-scaled Cartesian coordinates.
/// The algorithm mirrors the Fortran global-domain branch: build a local polar
/// stereographic plane at the spherical barycenter, solve the 2-D circumcenter,
/// unproject it back to an Earth displacement, then renormalize to the Earth
/// radius.
pub fn spherical_circumcenter_from_barycenter(
    barycenter: CartesianPoint,
    vertices: [CartesianPoint; 3],
) -> Option<CartesianPoint> {
    let barycenter_radius = magnitude(barycenter);
    let earth_radius = if barycenter_radius.is_finite() && barycenter_radius > 0.0 {
        barycenter_radius
    } else {
        vertices
            .iter()
            .map(|vertex| magnitude(*vertex))
            .find(|radius| radius.is_finite() && *radius > 0.0)
            .unwrap_or(EARTH_RADIUS_METERS)
    };
    spherical_circumcenter_from_barycenter_with_radius(barycenter, vertices, earth_radius)
}

pub(crate) fn spherical_circumcenter_from_barycenter_with_radius(
    barycenter: CartesianPoint,
    vertices: [CartesianPoint; 3],
    earth_radius: f64,
) -> Option<CartesianPoint> {
    let raxis = barycenter.x.hypot(barycenter.y);
    if raxis == 0.0 {
        return Some(barycenter);
    }

    let pole = PoleBasis {
        cos_lat: raxis / earth_radius,
        sin_lat: barycenter.z / earth_radius,
        cos_lon: barycenter.x / raxis,
        sin_lon: barycenter.y / raxis,
    };

    let mut projected = [PlanePoint::new(0.0, 0.0); 3];
    for (slot, vertex) in projected.iter_mut().zip(vertices) {
        let displacement = CartesianPoint::new(
            vertex.x - barycenter.x,
            vertex.y - barycenter.y,
            vertex.z - barycenter.z,
        );
        *slot = project_to_polar_stereographic_with_radius(displacement, pole, earth_radius);
    }

    let [p1, p2, p3] = projected;
    let dx12 = p2.x - p1.x;
    let dx13 = p3.x - p1.x;
    let dx23 = p3.x - p2.x;
    let s1 = p1.x * p1.x + p1.y * p1.y;
    let s2 = p2.x * p2.x + p2.y * p2.y;
    let s3 = p3.x * p3.x + p3.y * p3.y;

    let y_denom = dx13 * p2.y - dx12 * p3.y - dx23 * p1.y;
    if y_denom == 0.0 {
        return Some(barycenter);
    }
    let ycc = 0.5 * (dx13 * s2 - dx12 * s3 - dx23 * s1) / y_denom;

    let xcc = if dx12.abs() > dx13.abs() {
        if dx12 == 0.0 {
            return Some(barycenter);
        }
        (s2 - s1 - ycc * 2.0 * (p2.y - p1.y)) / (2.0 * dx12)
    } else {
        if dx13 == 0.0 {
            return Some(barycenter);
        }
        (s3 - s1 - ycc * 2.0 * (p3.y - p1.y)) / (2.0 * dx13)
    };

    let displacement = unproject_from_polar_stereographic_with_radius(
        PlanePoint::new(xcc, ycc),
        pole,
        earth_radius,
    );
    let mut circumcenter = CartesianPoint::new(
        displacement.x + barycenter.x,
        displacement.y + barycenter.y,
        displacement.z + barycenter.z,
    );

    let radius = magnitude(circumcenter);
    if radius == 0.0 {
        return Some(barycenter);
    }
    let expansion = earth_radius / radius;
    circumcenter.x *= expansion;
    circumcenter.y *= expansion;
    circumcenter.z *= expansion;
    Some(circumcenter)
}
