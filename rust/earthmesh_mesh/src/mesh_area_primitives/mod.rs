use crate::CartesianPoint;

/// Port of `MOD_grid_preprocess:arc_length`.
///
/// Computes spherical arc length from Cartesian coordinates using the same
/// haversine form and float32 squaring emulation described in the Canonical code.
pub fn arc_length_unit_sphere(a: CartesianPoint, b: CartesianPoint) -> f64 {
    let r_a = (a.x * a.x + a.y * a.y + a.z * a.z).sqrt();
    let r_b = (b.x * b.x + b.y * b.y + b.z * b.z).sqrt();
    if !r_a.is_finite() || !r_b.is_finite() || r_a <= f64::EPSILON || r_b <= f64::EPSILON {
        return 0.0;
    }

    let lon_a = a.y.atan2(a.x);
    let lat_a = (a.z / r_a).clamp(-1.0, 1.0).asin();
    let lon_b = b.y.atan2(b.x);
    let lat_b = (b.z / r_b).clamp(-1.0, 1.0).asin();

    let dlat_half = 0.5 * (lat_a - lat_b);
    let dlon_half = 0.5 * (lon_a - lon_b);

    let sin_dlat_half_f32 = dlat_half.sin() as f32;
    let sin_dlon_half_f32 = dlon_half.sin() as f32;
    let term1 = (sin_dlat_half_f32 * sin_dlat_half_f32) as f64;
    let term2 = lat_b.cos() * lat_a.cos() * (sin_dlon_half_f32 * sin_dlon_half_f32) as f64;

    let arg = (term1 + term2).max(0.0).sqrt().clamp(0.0, 1.0);
    r_a * 2.0 * arg.asin()
}

/// Port of `MOD_grid_preprocess:triangle_signed_area_sphere`.
///
/// Despite the Canonical name, the l'Huilier implementation returns a
/// non-negative spherical excess for the three input points. It deliberately
/// reuses `arc_length_unit_sphere` so the same mixed-precision haversine
/// behavior is preserved.
pub fn spherical_triangle_area_unit(points: [CartesianPoint; 3]) -> f64 {
    let a = arc_length_unit_sphere(points[2], points[1]);
    let b = arc_length_unit_sphere(points[2], points[0]);
    let c = arc_length_unit_sphere(points[0], points[1]);
    let semiperimeter = (a + b + c) / 2.0;
    let tan_quarter_excess = (semiperimeter / 2.0).tan()
        * ((semiperimeter - a) / 2.0).tan()
        * ((semiperimeter - b) / 2.0).tan()
        * ((semiperimeter - c) / 2.0).tan();

    4.0 * tan_quarter_excess.max(0.0).sqrt().atan()
}

/// Port of the MPAS kite area primitive inside `MOD_grid_preprocess:GetArea`.
///
/// For one vertex/cell pair, Canonical computes the kite as the absolute area of
/// triangle `(vertex, edge1, cell)` plus triangle `(vertex, edge2, cell)`.
pub fn spherical_kite_area_unit(
    vertex: CartesianPoint,
    edge1: CartesianPoint,
    edge2: CartesianPoint,
    cell: CartesianPoint,
) -> f64 {
    spherical_triangle_area_unit([vertex, edge1, cell]).abs()
        + spherical_triangle_area_unit([vertex, edge2, cell]).abs()
}

/// Port of the `areaCell` fan triangulation inside `MOD_grid_preprocess:GetArea`.
///
/// Canonical pins `verticesOnCell(1, i)` and sums triangles
/// `(v1, vj+1, vj+2)` for `j = 1..num_edges-2`.
pub fn spherical_cell_area_from_vertices_unit(
    vertices: &[CartesianPoint],
    num_edges: usize,
) -> Option<f64> {
    if num_edges < 3 || num_edges > vertices.len() {
        return None;
    }

    let anchor = vertices[0];
    let mut area = 0.0;
    for j in 0..(num_edges - 2) {
        area += spherical_triangle_area_unit([anchor, vertices[j + 1], vertices[j + 2]]);
    }
    Some(area)
}
