use earthmesh_core::deg_to_rad;

use crate::coordinates::{dot, magnitude, CartesianPoint};
use crate::{spherical_circumcenter_from_barycenter, spring_global_debug};

fn angular_distance_radians(a: CartesianPoint, b: CartesianPoint) -> Option<f64> {
    let mag = magnitude(a) * magnitude(b);
    if mag == 0.0 {
        return None;
    }
    Some((dot(a, b) / mag).clamp(-1.0, 1.0).acos())
}

/// Whether a triangle's circumcentre is close enough to it to be usable.
///
/// The gridfile writer's own admissibility test. Exposed so a backend can
/// refuse a transaction that would produce such a triangle, rather than
/// discovering it at the writer with the cause long gone.
pub fn circumcenter_is_local_enough(
    barycenter: CartesianPoint,
    circumcenter: CartesianPoint,
    vertices: [CartesianPoint; 3],
) -> bool {
    let Some(center_distance) = angular_distance_radians(barycenter, circumcenter) else {
        return false;
    };
    let max_vertex_distance = vertices
        .iter()
        .filter_map(|vertex| angular_distance_radians(barycenter, *vertex))
        .fold(0.0_f64, f64::max);
    if max_vertex_distance == 0.0 {
        return false;
    }

    let distances = vertices.map(|vertex| angular_distance_radians(circumcenter, vertex));
    let [Some(distance0), Some(distance1), Some(distance2)] = distances else {
        return false;
    };
    let min_distance = distance0.min(distance1).min(distance2);
    let max_distance = distance0.max(distance1).max(distance2);
    let equidistance_tolerance = 256.0 * f64::EPSILON.sqrt();
    let same_hemisphere = dot(barycenter, circumcenter) > 0.0;
    let local_distance =
        center_distance <= deg_to_rad(5.0) || center_distance <= 2.5 * max_vertex_distance;

    let equidistance_residual = max_distance - min_distance;
    let valid =
        same_hemisphere && local_distance && equidistance_residual <= equidistance_tolerance;
    if !valid {
        spring_global_debug(&format!(
            "circumcenter locality rejected: center_distance={center_distance:.9e}, \
             max_vertex_distance={max_vertex_distance:.9e}, \
             equidistance_residual={equidistance_residual:.9e}, \
             same_hemisphere={same_hemisphere}"
        ));
    }
    valid
}

/// Batch port of `MOD_grid_preprocess:circumcenter_spherical_calculation`.
///
/// Returns a copy of the incoming M-point Cartesian centers with triangle ids
/// `2..len` replaced by spherical circumcenters, preserving the Canonical inout
/// behavior for slots not visited by the loop.
pub fn circumcenter_spherical_mesh_one_based(
    initial_centers: &[CartesianPoint],
    vertex_points: &[CartesianPoint],
    cells_on_triangle: &[[usize; 3]],
) -> Option<Vec<CartesianPoint>> {
    if cells_on_triangle.len() > initial_centers.len() {
        return None;
    }

    let mut centers = initial_centers.to_vec();
    for triangle_id in 2..cells_on_triangle.len() {
        let vertex_ids = cells_on_triangle[triangle_id];
        let vertices = [
            *vertex_points.get(vertex_ids[0])?,
            *vertex_points.get(vertex_ids[1])?,
            *vertex_points.get(vertex_ids[2])?,
        ];
        let barycenter = centers[triangle_id];
        let circumcenter =
            match spherical_circumcenter_from_barycenter(centers[triangle_id], vertices) {
                Some(center) => center,
                None => {
                    spring_global_debug(&format!("circumcenter failed for triangle {triangle_id}"));
                    return None;
                }
            };
        if !circumcenter_is_local_enough(barycenter, circumcenter, vertices) {
            spring_global_debug(&format!(
                "circumcenter for triangle {triangle_id} is outside the local triangle"
            ));
            return None;
        }
        centers[triangle_id] = circumcenter;
    }

    Some(centers)
}
