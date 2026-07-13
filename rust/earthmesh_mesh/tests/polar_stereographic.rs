use earthmesh_core::{deg_to_rad, EARTH_RADIUS_METERS};
use earthmesh_mesh::{
    circumcenter_spherical_mesh_one_based, lonlat_degrees_to_unit_xyz,
    project_to_polar_stereographic, spherical_centroid_degrees,
    spherical_circumcenter_from_barycenter, unproject_from_polar_stereographic, CartesianPoint,
    LonLatDegrees, PlanePoint, PoleBasis,
};

fn approx_eq(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual {actual} expected {expected} tolerance {tolerance}"
    );
}

fn circumcenter_for_triangle(triangle: [LonLatDegrees; 3]) -> CartesianPoint {
    let r = EARTH_RADIUS_METERS;
    let scale = |point: CartesianPoint| CartesianPoint::new(point.x * r, point.y * r, point.z * r);
    let barycenter = spherical_centroid_degrees(&triangle).expect("spherical centroid");
    let initial_centers = vec![
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 0.0, 0.0),
        scale(lonlat_degrees_to_unit_xyz(barycenter)),
    ];
    let vertex_points = vec![
        CartesianPoint::new(0.0, 0.0, 0.0),
        scale(lonlat_degrees_to_unit_xyz(triangle[0])),
        scale(lonlat_degrees_to_unit_xyz(triangle[1])),
        scale(lonlat_degrees_to_unit_xyz(triangle[2])),
    ];
    circumcenter_spherical_mesh_one_based(
        &initial_centers,
        &vertex_points,
        &[[0, 0, 0], [0, 0, 0], [1, 2, 3]],
    )
    .expect("local spherical circumcenter")[2]
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
fn spherical_circumcenter_rejects_zero_barycenter() {
    let r = EARTH_RADIUS_METERS;
    let vertices = [
        CartesianPoint::new(r, 0.0, 0.0),
        CartesianPoint::new(0.0, r, 0.0),
        CartesianPoint::new(0.0, 0.0, r),
    ];

    assert!(
        spherical_circumcenter_from_barycenter(CartesianPoint::new(0.0, 0.0, 0.0), vertices,)
            .is_none()
    );
}

#[test]
fn spherical_circumcenter_rejects_collinear_vertices() {
    let r = EARTH_RADIUS_METERS;
    let scale = |point: CartesianPoint| CartesianPoint::new(point.x * r, point.y * r, point.z * r);
    let barycenter = scale(lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 0.0)));
    let vertices = [
        scale(lonlat_degrees_to_unit_xyz(LonLatDegrees::new(-1.0, 0.0))),
        barycenter,
        scale(lonlat_degrees_to_unit_xyz(LonLatDegrees::new(1.0, 0.0))),
    ];

    assert!(spherical_circumcenter_from_barycenter(barycenter, vertices).is_none());
}

#[test]
fn spherical_circumcenter_accepts_polar_triangle_without_lon_envelope() {
    let center = circumcenter_for_triangle([
        LonLatDegrees::new(0.0, 80.0),
        LonLatDegrees::new(120.0, 80.0),
        LonLatDegrees::new(-120.0, 80.0),
    ]);
    assert!(earthmesh_mesh::xyz_to_lonlat_degrees(center).lat_degrees > 89.999999);
}

#[test]
fn spherical_circumcenter_accepts_dateline_triangle_without_lon_envelope() {
    let center = circumcenter_for_triangle([
        LonLatDegrees::new(179.0, 0.0),
        LonLatDegrees::new(-179.0, 0.0),
        LonLatDegrees::new(180.0, 2.0),
    ]);
    assert!(
        earthmesh_mesh::xyz_to_lonlat_degrees(center)
            .lon_degrees
            .abs()
            > 179.0
    );
}

#[test]
fn spherical_circumcenter_accepts_local_obtuse_triangle() {
    let center = circumcenter_for_triangle([
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(2.0, 0.0),
        LonLatDegrees::new(0.5, 0.4),
    ]);
    let lonlat = earthmesh_mesh::xyz_to_lonlat_degrees(center);
    assert!(lonlat.lon_degrees.is_finite() && lonlat.lat_degrees.is_finite());
}

#[test]
fn circumcenter_spherical_mesh_preserves_canonical_indexing_and_inout_slots() {
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

    let centers =
        circumcenter_spherical_mesh_one_based(&initial_centers, &vertex_points, &cells_on_triangle)
            .expect("valid triangle vertex indices");

    assert_eq!(centers[0], initial_centers[0]);
    assert_eq!(centers[1], initial_centers[1]);
    approx_eq(centers[2].x, initial_centers[2].x, 1.0e-6);
    approx_eq(centers[2].y, initial_centers[2].y, 1.0e-6);
    approx_eq(centers[2].z, initial_centers[2].z, 1.0e-6);
}

#[test]
fn circumcenter_spherical_mesh_rejects_nonlocal_near_collinear_triangle() {
    let r = EARTH_RADIUS_METERS;
    let triangle = [
        LonLatDegrees::new(-21.591571, -51.613641),
        LonLatDegrees::new(-28.757277, -51.768891),
        LonLatDegrees::new(-24.323722, -51.622878),
    ];
    let barycenter = spherical_centroid_degrees(&triangle).expect("triangle centroid");
    let scale = |point: CartesianPoint| CartesianPoint::new(point.x * r, point.y * r, point.z * r);
    let initial_centers = vec![
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 0.0, 0.0),
        scale(lonlat_degrees_to_unit_xyz(barycenter)),
    ];
    let vertex_points = vec![
        CartesianPoint::new(0.0, 0.0, 0.0),
        scale(lonlat_degrees_to_unit_xyz(triangle[0])),
        scale(lonlat_degrees_to_unit_xyz(triangle[1])),
        scale(lonlat_degrees_to_unit_xyz(triangle[2])),
    ];
    let cells_on_triangle = vec![[0, 0, 0], [0, 0, 0], [1, 2, 3]];

    assert!(circumcenter_spherical_mesh_one_based(
        &initial_centers,
        &vertex_points,
        &cells_on_triangle,
    )
    .is_none());
}

#[test]
fn circumcenter_spherical_mesh_rejects_laterally_nonlocal_triangle() {
    let r = EARTH_RADIUS_METERS;
    let triangle = [
        LonLatDegrees::new(-65.469629, -36.870840),
        LonLatDegrees::new(-64.478027, -39.012391),
        LonLatDegrees::new(-64.196106, -40.931882),
    ];
    let barycenter = spherical_centroid_degrees(&triangle).expect("triangle centroid");
    let scale = |point: CartesianPoint| CartesianPoint::new(point.x * r, point.y * r, point.z * r);
    let initial_centers = vec![
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 0.0, 0.0),
        scale(lonlat_degrees_to_unit_xyz(barycenter)),
    ];
    let vertex_points = vec![
        CartesianPoint::new(0.0, 0.0, 0.0),
        scale(lonlat_degrees_to_unit_xyz(triangle[0])),
        scale(lonlat_degrees_to_unit_xyz(triangle[1])),
        scale(lonlat_degrees_to_unit_xyz(triangle[2])),
    ];
    let cells_on_triangle = vec![[0, 0, 0], [0, 0, 0], [1, 2, 3]];

    assert!(circumcenter_spherical_mesh_one_based(
        &initial_centers,
        &vertex_points,
        &cells_on_triangle,
    )
    .is_none());
}

#[test]
fn circumcenter_spherical_mesh_accepts_local_southern_triangle_without_latitude_envelope() {
    let r = EARTH_RADIUS_METERS;
    let triangle = [
        LonLatDegrees::new(165.165231, -63.551627),
        LonLatDegrees::new(161.942536, -62.451797),
        LonLatDegrees::new(171.024151, -63.893140),
    ];
    let barycenter = spherical_centroid_degrees(&triangle).expect("triangle centroid");
    let scale = |point: CartesianPoint| CartesianPoint::new(point.x * r, point.y * r, point.z * r);
    let initial_centers = vec![
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 0.0, 0.0),
        scale(lonlat_degrees_to_unit_xyz(barycenter)),
    ];
    let vertex_points = vec![
        CartesianPoint::new(0.0, 0.0, 0.0),
        scale(lonlat_degrees_to_unit_xyz(triangle[0])),
        scale(lonlat_degrees_to_unit_xyz(triangle[1])),
        scale(lonlat_degrees_to_unit_xyz(triangle[2])),
    ];
    let cells_on_triangle = vec![[0, 0, 0], [0, 0, 0], [1, 2, 3]];

    let centers =
        circumcenter_spherical_mesh_one_based(&initial_centers, &vertex_points, &cells_on_triangle)
            .expect("a vector-local circumcenter should not depend on a latitude envelope");
    let center = earthmesh_mesh::xyz_to_lonlat_degrees(centers[2]);
    assert!(center.lon_degrees.is_finite() && center.lat_degrees.is_finite());
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

    assert!(circumcenter_spherical_mesh_one_based(
        &initial_centers,
        &vertex_points,
        &cells_on_triangle,
    )
    .is_none());
}

#[test]
fn single_precision_polar_projection_matches_canonical_de_ps_identity_pole() {
    let pole = earthmesh_mesh::PoleBasisF32::from_lonlat_radians(0.0, 0.0);
    let projected = earthmesh_mesh::project_to_polar_stereographic_f32(
        earthmesh_mesh::CartesianPointF32::new(0.0, EARTH_RADIUS_METERS as f32, 0.0),
        pole,
    );

    assert!((projected.x - EARTH_RADIUS_METERS as f32).abs() <= 0.5);
    assert!(projected.y.abs() <= 0.5);
}

#[test]
fn single_precision_polar_unprojection_preserves_sphere_radius_like_canonical_ps_de() {
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
