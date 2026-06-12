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
