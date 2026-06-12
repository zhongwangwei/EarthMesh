//! Rust mesh kernels migrated from EarthMesh Fortran.

use earthmesh_core::{deg_to_rad, rad_to_deg};

/// Earth-centered Cartesian point using the same axis convention as `mkgrd.F90`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CartesianPoint {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl CartesianPoint {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

/// Longitude/latitude pair in degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LonLatDegrees {
    pub lon_degrees: f64,
    pub lat_degrees: f64,
}

impl LonLatDegrees {
    pub const fn new(lon_degrees: f64, lat_degrees: f64) -> Self {
        Self { lon_degrees, lat_degrees }
    }
}


/// Port of `MOD_grid_preprocess:lonlat2xyz` for a single unit-sphere point.
///
/// The Fortran routine intentionally returns unit vectors; callers multiply by
/// `erad8` when Earth-radius-scaled coordinates are required.
pub fn lonlat_degrees_to_unit_xyz(lonlat: LonLatDegrees) -> CartesianPoint {
    let lon_rad = deg_to_rad(lonlat.lon_degrees);
    let lat_rad = deg_to_rad(lonlat.lat_degrees);
    CartesianPoint::new(
        lat_rad.cos() * lon_rad.cos(),
        lat_rad.cos() * lon_rad.sin(),
        lat_rad.sin(),
    )
}

/// Batch port of `MOD_grid_preprocess:lonlat2xyz`, preserving input order.
pub fn lonlat_points_to_unit_xyz(points: &[LonLatDegrees]) -> Vec<CartesianPoint> {
    points.iter().copied().map(lonlat_degrees_to_unit_xyz).collect()
}

/// Convert Earth-centered Cartesian coordinates to lon/lat degrees.
///
/// This ports the scalar formula used by `mkgrd.F90:grid_xyz2lonlat`:
///
/// - `raxis = sqrt(x ** 2 + y ** 2)`
/// - `lat = atan2(z, raxis) * piu180`
/// - `lon = atan2(y, x) * piu180`
#[inline]
pub fn xyz_to_lonlat_degrees(point: CartesianPoint) -> LonLatDegrees {
    let raxis = point.x.hypot(point.y);
    LonLatDegrees {
        lon_degrees: rad_to_deg(point.y.atan2(point.x)),
        lat_degrees: rad_to_deg(point.z.atan2(raxis)),
    }
}

/// Convert a slice of Earth-centered Cartesian points to lon/lat degrees while
/// preserving point order.
pub fn xyz_points_to_lonlat_degrees(points: &[CartesianPoint]) -> Vec<LonLatDegrees> {
    points.iter().copied().map(xyz_to_lonlat_degrees).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_conversion_preserves_order() {
        let points = [CartesianPoint::new(1.0, 0.0, 0.0), CartesianPoint::new(0.0, 1.0, 0.0)];
        let lonlat = xyz_points_to_lonlat_degrees(&points);
        assert_eq!(lonlat.len(), 2);
        assert_eq!(lonlat[0].lon_degrees, 0.0);
        assert_eq!(lonlat[1].lon_degrees, 90.0);
    }
}

/// Precomputed sine/cosine basis for an icosahedron polar stereographic pole.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoleBasis {
    pub cos_lat: f64,
    pub sin_lat: f64,
    pub cos_lon: f64,
    pub sin_lon: f64,
}

impl PoleBasis {
    pub fn from_lonlat_radians(lon_radians: f64, lat_radians: f64) -> Self {
        Self {
            cos_lat: lat_radians.cos(),
            sin_lat: lat_radians.sin(),
            cos_lon: lon_radians.cos(),
            sin_lon: lon_radians.sin(),
        }
    }
}

/// Point on the polar stereographic plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlanePoint {
    pub x: f64,
    pub y: f64,
}

impl PlanePoint {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// Port of `icosahedron.F90:de_ps_r8`.
pub fn project_to_polar_stereographic(point: CartesianPoint, pole: PoleBasis) -> PlanePoint {
    let xq = -pole.sin_lon * point.x + pole.cos_lon * point.y;
    let yq = pole.cos_lat * point.z
        - pole.sin_lat * (pole.cos_lon * point.x + pole.sin_lon * point.y);
    let zq = pole.sin_lat * point.z
        + pole.cos_lat * (pole.cos_lon * point.x + pole.sin_lon * point.y);

    let earth_diameter = earthmesh_core::EARTH_RADIUS_METERS * 2.0;
    let t = earth_diameter / (earth_diameter + zq);

    PlanePoint::new(xq * t, yq * t)
}

/// Port of `icosahedron.F90:ps_de_r8`.
pub fn unproject_from_polar_stereographic(point: PlanePoint, pole: PoleBasis) -> CartesianPoint {
    let earth_diameter = earthmesh_core::EARTH_RADIUS_METERS * 2.0;
    let earth_diameter_sq = earth_diameter * earth_diameter;
    let t = earth_diameter_sq / (point.x * point.x + point.y * point.y + earth_diameter_sq);

    let xq = point.x * t;
    let yq = point.y * t;
    let zq = earth_diameter * (t - 1.0);

    CartesianPoint::new(
        -pole.sin_lon * xq + pole.cos_lon * (-pole.sin_lat * yq + pole.cos_lat * zq),
        pole.cos_lon * xq - pole.sin_lon * (pole.sin_lat * yq - pole.cos_lat * zq),
        pole.cos_lat * yq + pole.sin_lat * zq,
    )
}

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
    Some(xyz_to_lonlat_degrees(centroid))
}

/// Port of `MOD_grid_preprocess:CheckLon`.
///
/// The Fortran routine performs a single +/-360 adjustment rather than a full
/// modulo normalization. Preserve that behavior for parity.
pub fn normalize_lon_m180_180(lon_degrees: f64) -> f64 {
    if lon_degrees > 180.0 {
        lon_degrees - 360.0
    } else if lon_degrees < -180.0 {
        lon_degrees + 360.0
    } else {
        lon_degrees
    }
}

/// Port of `MOD_grid_preprocess:arc_length`.
///
/// Computes spherical arc length from Cartesian coordinates using the same
/// haversine form and float32 squaring emulation described in the Fortran code.
pub fn arc_length_unit_sphere(a: CartesianPoint, b: CartesianPoint) -> f64 {
    let r_a = (a.x * a.x + a.y * a.y + a.z * a.z).sqrt();
    let r_b = (b.x * b.x + b.y * b.y + b.z * b.z).sqrt();

    let lon_a = a.y.atan2(a.x);
    let lat_a = (a.z / r_a).asin();
    let lon_b = b.y.atan2(b.x);
    let lat_b = (b.z / r_b).asin();

    let dlat_half = 0.5 * (lat_a - lat_b);
    let dlon_half = 0.5 * (lon_a - lon_b);

    let sin_dlat_half_f32 = dlat_half.sin() as f32;
    let sin_dlon_half_f32 = dlon_half.sin() as f32;
    let term1 = (sin_dlat_half_f32 * sin_dlat_half_f32) as f64;
    let term2 = lat_b.cos() * lat_a.cos() * (sin_dlon_half_f32 * sin_dlon_half_f32) as f64;

    let arg = (term1 + term2).sqrt();
    r_a * 2.0 * arg.asin()
}

/// Output of `MOD_grid_preprocess:Get_Length_Angle`.
#[derive(Debug, Clone, PartialEq)]
pub struct PolygonLengthAngleMetrics {
    pub angles_degrees: Vec<f64>,
    pub edge_lengths_meters: Vec<f64>,
}

/// Port of `MOD_grid_preprocess:Get_Length_Angle`.
///
/// For each polygon vertex, this builds the same `(previous, current, next)`
/// triplet as the Fortran cyclic buffer, computes the spherical angle using the
/// half-angle formula, and records the current-to-next edge length scaled by
/// `erad8`.
pub fn polygon_length_angle_metrics(points: &[LonLatDegrees]) -> Option<PolygonLengthAngleMetrics> {
    let num_edges = points.len();
    if num_edges < 3 {
        return None;
    }

    let mut angles_degrees = Vec::with_capacity(num_edges);
    let mut edge_lengths_meters = Vec::with_capacity(num_edges);

    for i in 0..num_edges {
        let previous = points[(i + num_edges - 1) % num_edges];
        let current = points[i];
        let next = points[(i + 1) % num_edges];

        let previous_xyz = lonlat_degrees_to_unit_xyz(previous);
        let current_xyz = lonlat_degrees_to_unit_xyz(current);
        let next_xyz = lonlat_degrees_to_unit_xyz(next);

        let length1 = arc_length_unit_sphere(next_xyz, current_xyz);
        let length2 = arc_length_unit_sphere(next_xyz, previous_xyz);
        let length3 = arc_length_unit_sphere(previous_xyz, current_xyz);
        let semiperimeter = 0.5 * (length1 + length2 + length3);
        let angle_arg = ((semiperimeter - length1).sin() * (semiperimeter - length3).sin()
            / (length1.sin() * length3.sin()))
            .sqrt();
        angles_degrees.push(rad_to_deg(2.0 * angle_arg.asin()));
        edge_lengths_meters.push(length1 * earthmesh_core::EARTH_RADIUS_METERS);
    }

    Some(PolygonLengthAngleMetrics {
        angles_degrees,
        edge_lengths_meters,
    })
}

/// Mesh-quality aggregate produced by Fortran `TriMeshQuality`/`PolyMeshQuality`.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshQualitySummary {
    pub cell_metrics: Vec<PolygonLengthAngleMetrics>,
    pub extreme_angles_degrees: (f64, f64),
    pub average_min_max_angles_degrees: (f64, f64),
    pub angle_stddev_degrees: f64,
    pub angle_less_flags: Vec<bool>,
    pub angle_more_flags: Vec<bool>,
}

fn polygon_quality_summary(
    cells: &[Vec<LonLatDegrees>],
    regular_angle_degrees: f64,
    lower_threshold_degrees: f64,
    upper_threshold_degrees: f64,
) -> Option<MeshQualitySummary> {
    if cells.is_empty() {
        return None;
    }

    let mut cell_metrics = Vec::with_capacity(cells.len());
    let mut angle_less_flags = Vec::with_capacity(cells.len());
    let mut angle_more_flags = Vec::with_capacity(cells.len());
    let mut global_min = f64::INFINITY;
    let mut global_max = f64::NEG_INFINITY;
    let mut sum_min = 0.0;
    let mut sum_max = 0.0;
    let mut sum_squared = 0.0;
    let mut angle_count = 0usize;

    for cell in cells {
        let metrics = polygon_length_angle_metrics(cell)?;
        let cell_min = metrics
            .angles_degrees
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        let cell_max = metrics
            .angles_degrees
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        global_min = global_min.min(cell_min);
        global_max = global_max.max(cell_max);
        sum_min += cell_min;
        sum_max += cell_max;
        sum_squared += metrics
            .angles_degrees
            .iter()
            .map(|angle| (angle - regular_angle_degrees).powi(2))
            .sum::<f64>();
        angle_count += metrics.angles_degrees.len();
        angle_less_flags.push(cell_min < lower_threshold_degrees);
        angle_more_flags.push(cell_max > upper_threshold_degrees);
        cell_metrics.push(metrics);
    }

    let cell_count = cells.len() as f64;
    Some(MeshQualitySummary {
        cell_metrics,
        extreme_angles_degrees: (global_min, global_max),
        average_min_max_angles_degrees: (sum_min / cell_count, sum_max / cell_count),
        angle_stddev_degrees: (sum_squared / angle_count as f64).sqrt(),
        angle_less_flags,
        angle_more_flags,
    })
}

/// Port of the aggregation core in `MOD_grid_preprocess:TriMeshQuality`.
pub fn triangle_mesh_quality(triangles: &[[LonLatDegrees; 3]]) -> Option<MeshQualitySummary> {
    let cells: Vec<Vec<LonLatDegrees>> = triangles.iter().map(|triangle| triangle.to_vec()).collect();
    polygon_quality_summary(&cells, 60.0, 45.0, 75.0)
}

/// Port of the aggregation core in `MOD_grid_preprocess:PolyMeshQuality`.
///
/// All cells in the input should have the same edge count, matching each
/// Fortran call for pentagons, hexagons, or heptagons. The regular angle is
/// `(num_edges - 2) * 180 / num_edges`, with 0.9/1.1 threshold bands.
pub fn polygon_mesh_quality(cells: &[Vec<LonLatDegrees>]) -> Option<MeshQualitySummary> {
    let first = cells.first()?;
    let num_edges = first.len();
    if num_edges < 3 || cells.iter().any(|cell| cell.len() != num_edges) {
        return None;
    }

    let regular_angle = (num_edges as f64 - 2.0) * 180.0 / num_edges as f64;
    polygon_quality_summary(cells, regular_angle, regular_angle * 0.9, regular_angle * 1.1)
}
