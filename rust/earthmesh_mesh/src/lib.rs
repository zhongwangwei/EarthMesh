//! Rust mesh kernels migrated from EarthMesh Fortran.

use earthmesh_core::rad_to_deg;

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
