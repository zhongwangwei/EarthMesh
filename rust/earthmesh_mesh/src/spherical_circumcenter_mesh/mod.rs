use earthmesh_core::deg_to_rad;

use crate::coordinates::{dot, magnitude, xyz_to_lonlat_degrees, CartesianPoint};
use crate::{spherical_circumcenter_from_barycenter, spring_global_debug};

fn angular_distance_radians(a: CartesianPoint, b: CartesianPoint) -> Option<f64> {
    let mag = magnitude(a) * magnitude(b);
    if mag == 0.0 {
        return None;
    }
    Some((dot(a, b) / mag).clamp(-1.0, 1.0).acos())
}

fn circumcenter_is_local_enough(
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

    (center_distance <= deg_to_rad(5.0) || center_distance <= 2.5 * max_vertex_distance)
        && circumcenter_fits_local_lonlat_envelope(barycenter, circumcenter, vertices)
}

fn unwrap_lon_around(lon_degrees: f64, reference_degrees: f64) -> f64 {
    if lon_degrees - reference_degrees > 180.0 {
        lon_degrees - 360.0
    } else if lon_degrees - reference_degrees < -180.0 {
        lon_degrees + 360.0
    } else {
        lon_degrees
    }
}

fn circumcenter_fits_local_lonlat_envelope(
    barycenter: CartesianPoint,
    circumcenter: CartesianPoint,
    vertices: [CartesianPoint; 3],
) -> bool {
    let barycenter_lonlat = xyz_to_lonlat_degrees(barycenter);
    let circumcenter_lonlat = xyz_to_lonlat_degrees(circumcenter);
    let circumcenter_lon = unwrap_lon_around(
        circumcenter_lonlat.lon_degrees,
        barycenter_lonlat.lon_degrees,
    );
    let mut min_lon = f64::MAX;
    let mut max_lon = f64::MIN;
    let mut min_lat = f64::MAX;
    let mut max_lat = f64::MIN;

    for vertex in vertices {
        let vertex_lonlat = xyz_to_lonlat_degrees(vertex);
        let vertex_lon =
            unwrap_lon_around(vertex_lonlat.lon_degrees, barycenter_lonlat.lon_degrees);
        min_lon = min_lon.min(vertex_lon);
        max_lon = max_lon.max(vertex_lon);
        min_lat = min_lat.min(vertex_lonlat.lat_degrees);
        max_lat = max_lat.max(vertex_lonlat.lat_degrees);
    }

    let lon_margin = ((max_lon - min_lon) * 1.5).max(1.0);
    let lat_margin = ((max_lat - min_lat) * 1.5).max(1.0);
    circumcenter_lon >= min_lon - lon_margin
        && circumcenter_lon <= max_lon + lon_margin
        && circumcenter_lonlat.lat_degrees >= min_lat - lat_margin
        && circumcenter_lonlat.lat_degrees <= max_lat + lat_margin
}

/// Batch port of `MOD_grid_preprocess:circumcenter_spherical_calculation`.
///
/// Returns a copy of the incoming M-point Cartesian centers with triangle ids
/// `2..len` replaced by spherical circumcenters, preserving the Fortran inout
/// behavior for slots not visited by the loop.
pub fn circumcenter_spherical_mesh_fortran_indexed(
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
        centers[triangle_id] = if circumcenter_is_local_enough(barycenter, circumcenter, vertices) {
            circumcenter
        } else {
            spring_global_debug(&format!(
                "circumcenter for triangle {triangle_id} is outside local triangle; using barycenter"
            ));
            barycenter
        };
    }

    Some(centers)
}
