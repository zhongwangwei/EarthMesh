use crate::coordinates::{
    lonlat_degrees_to_unit_xyz, xyz_to_lonlat_degrees, CartesianPoint, LonLatDegrees,
};

/// Port of `MOD_grid_preprocess:centroid_spherical_single`.
///
/// Converts lon/lat vertices to unit Cartesian vectors, averages components,
/// then converts the averaged vector back to lon/lat degrees.
pub fn spherical_centroid_degrees(points: &[LonLatDegrees]) -> Option<LonLatDegrees> {
    if points.is_empty() {
        return None;
    }

    let mut sx = 0.0;
    let mut sy = 0.0;
    let mut sz = 0.0;
    for point in points {
        let xyz = lonlat_degrees_to_unit_xyz(*point);
        sx += xyz.x;
        sy += xyz.y;
        sz += xyz.z;
    }
    let n = points.len() as f64;
    let centroid = CartesianPoint::new(sx / n, sy / n, sz / n);
    let norm = (centroid.x * centroid.x + centroid.y * centroid.y + centroid.z * centroid.z).sqrt();
    if !norm.is_finite() || norm <= 1.0e-12 {
        return None;
    }
    Some(xyz_to_lonlat_degrees(centroid))
}

/// Batch port of `MOD_grid_preprocess:centroid_spherical_calculation`.
///
/// Preserves the Canonical workflow where triangle ids start at `2`; slots `0`
/// and `1` remain initialized to `(0, 0)` just like an unwritten `mp` scratch
/// array in the current Rust call boundary.
pub fn centroid_spherical_mesh_one_based(
    cell_points: &[LonLatDegrees],
    cells_on_triangle: &[[usize; 3]],
) -> Option<Vec<LonLatDegrees>> {
    let mut centroids = vec![LonLatDegrees::new(0.0, 0.0); cells_on_triangle.len()];

    for triangle_id in 2..cells_on_triangle.len() {
        let cell_ids = cells_on_triangle[triangle_id];
        let triangle_points = [
            *cell_points.get(cell_ids[0])?,
            *cell_points.get(cell_ids[1])?,
            *cell_points.get(cell_ids[2])?,
        ];
        centroids[triangle_id] = spherical_centroid_degrees(&triangle_points)?;
    }

    Some(centroids)
}
