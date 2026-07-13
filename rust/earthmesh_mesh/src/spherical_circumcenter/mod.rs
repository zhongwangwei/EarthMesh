use earthmesh_core::EARTH_RADIUS_METERS;

use crate::coordinates::{cross, dot, magnitude, vector_between, CartesianPoint};

/// Port of one iteration of `MOD_grid_preprocess:circumcenter_spherical_calculation`.
///
/// `barycenter` and `vertices` are Earth-radius-scaled Cartesian coordinates.
/// The circumcenter is solved directly on the sphere: its unit vector is normal
/// to both vertex-difference vectors, so its angular distance to all three
/// vertices is equal. Of the two antipodal solutions, the one in the
/// barycenter's hemisphere is selected.
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
    if !earth_radius.is_finite() || earth_radius <= 0.0 {
        return None;
    }
    let barycenter_norm = magnitude(barycenter);
    if !barycenter_norm.is_finite() || barycenter_norm <= earth_radius * 1.0e-12 {
        return None;
    }
    if vertices
        .iter()
        .any(|vertex| !magnitude(*vertex).is_finite() || magnitude(*vertex) <= 0.0)
    {
        return None;
    }

    let unit_vertices = vertices.map(|vertex| {
        let radius = magnitude(vertex);
        CartesianPoint::new(vertex.x / radius, vertex.y / radius, vertex.z / radius)
    });
    let spherical_determinant = dot(unit_vertices[0], cross(unit_vertices[1], unit_vertices[2]));
    if !spherical_determinant.is_finite() || spherical_determinant.abs() <= 128.0 * f64::EPSILON {
        return None;
    }

    let mut normal = cross(
        vector_between(unit_vertices[0], unit_vertices[1]),
        vector_between(unit_vertices[0], unit_vertices[2]),
    );
    let normal_norm = magnitude(normal);
    if !normal_norm.is_finite() || normal_norm <= 128.0 * f64::EPSILON {
        return None;
    }
    if dot(normal, barycenter) < 0.0 {
        normal.x = -normal.x;
        normal.y = -normal.y;
        normal.z = -normal.z;
    }
    let expansion = earth_radius / normal_norm;
    Some(CartesianPoint::new(
        normal.x * expansion,
        normal.y * expansion,
        normal.z * expansion,
    ))
}
