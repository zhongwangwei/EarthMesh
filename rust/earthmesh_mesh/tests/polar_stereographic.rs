use earthmesh_core::{deg_to_rad, EARTH_RADIUS_METERS};
use earthmesh_mesh::{
    circumcenter_spherical_mesh_fortran_indexed, project_to_polar_stereographic,
    spherical_circumcenter_from_barycenter, unproject_from_polar_stereographic, CartesianPoint,
    PlanePoint, PoleBasis,
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

#[test]
fn spherical_circumcenter_matches_symmetric_octant_triangle() {
    let r = EARTH_RADIUS_METERS;
    let inv_sqrt_3 = 1.0 / 3.0_f64.sqrt();
    let barycenter = CartesianPoint::new(r * inv_sqrt_3, r * inv_sqrt_3, r * inv_sqrt_3);
    let vertices = [
        CartesianPoint::new(r, 0.0, 0.0),
        CartesianPoint::new(0.0, r, 0.0),
        CartesianPoint::new(0.0, 0.0, r),
    ];

    let circumcenter = spherical_circumcenter_from_barycenter(barycenter, vertices)
        .expect("non-degenerate spherical triangle circumcenter");

    approx_eq(circumcenter.x, barycenter.x, 1.0e-6);
    approx_eq(circumcenter.y, barycenter.y, 1.0e-6);
    approx_eq(circumcenter.z, barycenter.z, 1.0e-6);
    approx_eq(
        (circumcenter.x.powi(2) + circumcenter.y.powi(2) + circumcenter.z.powi(2)).sqrt(),
        r,
        1.0e-6,
    );
}

#[test]
fn circumcenter_spherical_mesh_preserves_fortran_indexing_and_inout_slots() {
    let r = EARTH_RADIUS_METERS;
    let inv_sqrt_3 = 1.0 / 3.0_f64.sqrt();
    let initial_centers = vec![
        CartesianPoint::new(1.0, 2.0, 3.0),
        CartesianPoint::new(4.0, 5.0, 6.0),
        CartesianPoint::new(r * inv_sqrt_3, r * inv_sqrt_3, r * inv_sqrt_3),
    ];
    let vertex_points = vec![
        CartesianPoint::new(-999.0, -999.0, -999.0),
        CartesianPoint::new(-999.0, -999.0, -999.0),
        CartesianPoint::new(r, 0.0, 0.0),
        CartesianPoint::new(0.0, r, 0.0),
        CartesianPoint::new(0.0, 0.0, r),
    ];
    let cells_on_triangle = vec![[0, 0, 0], [0, 0, 0], [2, 3, 4]];

    let centers = circumcenter_spherical_mesh_fortran_indexed(
        &initial_centers,
        &vertex_points,
        &cells_on_triangle,
    )
    .expect("valid triangle vertex references");

    assert_eq!(centers[0], initial_centers[0]);
    assert_eq!(centers[1], initial_centers[1]);
    approx_eq(centers[2].x, initial_centers[2].x, 1.0e-6);
    approx_eq(centers[2].y, initial_centers[2].y, 1.0e-6);
    approx_eq(centers[2].z, initial_centers[2].z, 1.0e-6);
}

#[test]
fn circumcenter_spherical_mesh_rejects_out_of_range_vertex_id() {
    let r = EARTH_RADIUS_METERS;
    let initial_centers = vec![
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(r, 0.0, 0.0),
    ];
    let vertex_points = vec![
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(r, 0.0, 0.0),
    ];
    let cells_on_triangle = vec![[0, 0, 0], [0, 0, 0], [2, 3, 4]];

    assert!(circumcenter_spherical_mesh_fortran_indexed(
        &initial_centers,
        &vertex_points,
        &cells_on_triangle,
    )
    .is_none());
}

#[test]
fn single_precision_polar_projection_matches_fortran_de_ps_identity_pole() {
    let pole = earthmesh_mesh::PoleBasisF32::from_lonlat_radians(0.0, 0.0);
    let projected = earthmesh_mesh::project_to_polar_stereographic_f32(
        earthmesh_mesh::CartesianPointF32::new(0.0, EARTH_RADIUS_METERS as f32, 0.0),
        pole,
    );

    assert!((projected.x - EARTH_RADIUS_METERS as f32).abs() <= 0.5);
    assert!(projected.y.abs() <= 0.5);
}

#[test]
fn single_precision_polar_unprojection_preserves_sphere_radius_like_fortran_ps_de() {
    let pole = earthmesh_mesh::PoleBasisF32::from_lonlat_radians(0.0, 0.0);
    let unprojected = earthmesh_mesh::unproject_from_polar_stereographic_f32(
        earthmesh_mesh::PlanePointF32::new(100_000.0, 200_000.0),
        pole,
    );
    let absolute_x = EARTH_RADIUS_METERS as f32 + unprojected.x;
    let radius =
        (absolute_x * absolute_x + unprojected.y * unprojected.y + unprojected.z * unprojected.z)
            .sqrt();

    assert!((radius - EARTH_RADIUS_METERS as f32).abs() <= 1.0);
    assert!(unprojected.x < 0.0);
}
