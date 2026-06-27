use earthmesh_core::EARTH_RADIUS_METERS;

use crate::coordinates::{CartesianPoint, CartesianPointF32};

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

/// Single-precision pole basis for `icosahedron.F90` `real` projection calls.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoleBasisF32 {
    pub cos_lat: f32,
    pub sin_lat: f32,
    pub cos_lon: f32,
    pub sin_lon: f32,
}

impl PoleBasisF32 {
    pub fn from_lonlat_radians(lon_radians: f32, lat_radians: f32) -> Self {
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

/// Single-precision point on the polar stereographic plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlanePointF32 {
    pub x: f32,
    pub y: f32,
}

impl PlanePointF32 {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Port of `icosahedron.F90:de_ps_r8`.
pub fn project_to_polar_stereographic(point: CartesianPoint, pole: PoleBasis) -> PlanePoint {
    project_to_polar_stereographic_with_radius(point, pole, EARTH_RADIUS_METERS)
}

pub(crate) fn project_to_polar_stereographic_with_radius(
    point: CartesianPoint,
    pole: PoleBasis,
    earth_radius: f64,
) -> PlanePoint {
    let xq = -pole.sin_lon * point.x + pole.cos_lon * point.y;
    let yq =
        pole.cos_lat * point.z - pole.sin_lat * (pole.cos_lon * point.x + pole.sin_lon * point.y);
    let zq =
        pole.sin_lat * point.z + pole.cos_lat * (pole.cos_lon * point.x + pole.sin_lon * point.y);

    let earth_diameter = earth_radius * 2.0;
    let t = earth_diameter / (earth_diameter + zq);

    PlanePoint::new(xq * t, yq * t)
}

/// Port of the single-precision `icosahedron.F90:de_ps`.
pub fn project_to_polar_stereographic_f32(
    point: CartesianPointF32,
    pole: PoleBasisF32,
) -> PlanePointF32 {
    let xq = -pole.sin_lon * point.x + pole.cos_lon * point.y;
    let yq =
        pole.cos_lat * point.z - pole.sin_lat * (pole.cos_lon * point.x + pole.sin_lon * point.y);
    let zq =
        pole.sin_lat * point.z + pole.cos_lat * (pole.cos_lon * point.x + pole.sin_lon * point.y);

    let earth_diameter = EARTH_RADIUS_METERS as f32 * 2.0;
    let t = earth_diameter / (earth_diameter + zq);

    PlanePointF32::new(xq * t, yq * t)
}

/// Port of `icosahedron.F90:ps_de_r8`.
pub fn unproject_from_polar_stereographic(point: PlanePoint, pole: PoleBasis) -> CartesianPoint {
    unproject_from_polar_stereographic_with_radius(point, pole, EARTH_RADIUS_METERS)
}

pub(crate) fn unproject_from_polar_stereographic_with_radius(
    point: PlanePoint,
    pole: PoleBasis,
    earth_radius: f64,
) -> CartesianPoint {
    let earth_diameter = earth_radius * 2.0;
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

/// Port of the single-precision `icosahedron.F90:ps_de`.
pub fn unproject_from_polar_stereographic_f32(
    point: PlanePointF32,
    pole: PoleBasisF32,
) -> CartesianPointF32 {
    let earth_diameter = EARTH_RADIUS_METERS as f32 * 2.0;
    let earth_diameter_sq = earth_diameter * earth_diameter;
    let t = earth_diameter_sq / (point.x * point.x + point.y * point.y + earth_diameter_sq);

    let xq = point.x * t;
    let yq = point.y * t;
    let zq = earth_diameter * (t - 1.0);

    CartesianPointF32::new(
        -pole.sin_lon * xq + pole.cos_lon * (-pole.sin_lat * yq + pole.cos_lat * zq),
        pole.cos_lon * xq - pole.sin_lon * (pole.sin_lat * yq - pole.cos_lat * zq),
        pole.cos_lat * yq + pole.sin_lat * zq,
    )
}
