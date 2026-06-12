use earthmesh_core::EARTH_RADIUS_METERS;
use earthmesh_mesh::{xyz_to_lonlat_degrees, CartesianPoint};

fn approx_eq(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual {actual} expected {expected} tolerance {tolerance}"
    );
}

#[test]
fn xyz_to_lonlat_matches_mkgrd_equator_axes() {
    let east = xyz_to_lonlat_degrees(CartesianPoint::new(EARTH_RADIUS_METERS, 0.0, 0.0));
    approx_eq(east.lon_degrees, 0.0, 1.0e-12);
    approx_eq(east.lat_degrees, 0.0, 1.0e-12);

    let north_quadrant = xyz_to_lonlat_degrees(CartesianPoint::new(0.0, EARTH_RADIUS_METERS, 0.0));
    approx_eq(north_quadrant.lon_degrees, 90.0, 1.0e-12);
    approx_eq(north_quadrant.lat_degrees, 0.0, 1.0e-12);
}

#[test]
fn xyz_to_lonlat_matches_mkgrd_poles_and_quadrants() {
    let north_pole = xyz_to_lonlat_degrees(CartesianPoint::new(0.0, 0.0, EARTH_RADIUS_METERS));
    approx_eq(north_pole.lat_degrees, 90.0, 1.0e-12);

    let southwest = xyz_to_lonlat_degrees(CartesianPoint::new(-1.0, -1.0, 1.0));
    approx_eq(southwest.lon_degrees, -135.0, 1.0e-12);
    approx_eq(southwest.lat_degrees, 35.264389682754654, 1.0e-12);
}
