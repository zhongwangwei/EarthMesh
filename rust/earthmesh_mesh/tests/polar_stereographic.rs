use earthmesh_core::{deg_to_rad, EARTH_RADIUS_METERS};
use earthmesh_mesh::{
    project_to_polar_stereographic, unproject_from_polar_stereographic, CartesianPoint, PlanePoint,
    PoleBasis,
};

fn approx_eq(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual {actual} expected {expected} tolerance {tolerance}"
    );
}

#[test]
fn polar_stereographic_projection_matches_icosahedron_de_ps_r8_equatorial_pole() {
    let pole = PoleBasis::from_lonlat_radians(0.0, 0.0);
    let projected =
        project_to_polar_stereographic(CartesianPoint::new(0.0, EARTH_RADIUS_METERS, 0.0), pole);

    approx_eq(projected.x, EARTH_RADIUS_METERS, 1.0e-9);
    approx_eq(projected.y, 0.0, 1.0e-9);
}

#[test]
fn polar_stereographic_plane_round_trip_preserves_plane_coordinates() {
    let pole = PoleBasis::from_lonlat_radians(deg_to_rad(113.0), deg_to_rad(22.0));
    let plane = PlanePoint::new(25_000.0, -18_000.0);

    let displacement = unproject_from_polar_stereographic(plane, pole);
    let projected = project_to_polar_stereographic(displacement, pole);

    approx_eq(projected.x, plane.x, 1.0e-8);
    approx_eq(projected.y, plane.y, 1.0e-8);
}

#[test]
fn polar_stereographic_unprojection_matches_icosahedron_ps_de_r8_identity_plane() {
    let pole = PoleBasis::from_lonlat_radians(0.0, 0.0);
    let unprojected = unproject_from_polar_stereographic(PlanePoint::new(1_000.0, 2_000.0), pole);

    // For the identity pole, ps_de returns displacement from pole point (R,0,0):
    // dxe=zq, dye=xq, dze=yq. Adding the pole point should land on the sphere.
    let absolute_x = EARTH_RADIUS_METERS + unprojected.x;
    let radius = (absolute_x.powi(2) + unprojected.y.powi(2) + unprojected.z.powi(2)).sqrt();

    approx_eq(radius, EARTH_RADIUS_METERS, 1.0e-6);
    assert!(
        unprojected.x < 0.0,
        "identity-pole ps_de zq should move x below the pole radius"
    );
}
