use earthmesh_mesh::{lonlat_degrees_to_unit_xyz, lonlat_points_to_unit_xyz, xyz_to_lonlat_degrees, LonLatDegrees};

fn approx_eq(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual {actual} expected {expected} tolerance {tolerance}"
    );
}

#[test]
fn lonlat_to_unit_xyz_matches_mod_grid_preprocess_axes() {
    let prime = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 0.0));
    approx_eq(prime.x, 1.0, 1.0e-15);
    approx_eq(prime.y, 0.0, 1.0e-15);
    approx_eq(prime.z, 0.0, 1.0e-15);

    let east = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(90.0, 0.0));
    approx_eq(east.x, 0.0, 1.0e-15);
    approx_eq(east.y, 1.0, 1.0e-15);
    approx_eq(east.z, 0.0, 1.0e-15);

    let north = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(42.0, 90.0));
    approx_eq(north.x, 0.0, 1.0e-15);
    approx_eq(north.y, 0.0, 1.0e-15);
    approx_eq(north.z, 1.0, 1.0e-15);
}

#[test]
fn lonlat_to_unit_xyz_round_trips_through_xyz2lonlat() {
    let lonlat = LonLatDegrees::new(113.25, 22.5);
    let xyz = lonlat_degrees_to_unit_xyz(lonlat);
    let round_trip = xyz_to_lonlat_degrees(xyz);

    approx_eq(round_trip.lon_degrees, lonlat.lon_degrees, 1.0e-12);
    approx_eq(round_trip.lat_degrees, lonlat.lat_degrees, 1.0e-12);
}

#[test]
fn batch_lonlat_to_unit_xyz_preserves_order() {
    let xyz = lonlat_points_to_unit_xyz(&[
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(90.0, 0.0),
        LonLatDegrees::new(0.0, 90.0),
    ]);

    assert_eq!(xyz.len(), 3);
    approx_eq(xyz[0].x, 1.0, 1.0e-15);
    approx_eq(xyz[1].y, 1.0, 1.0e-15);
    approx_eq(xyz[2].z, 1.0, 1.0e-15);
}

#[test]
fn spherical_centroid_matches_fortran_vector_average_on_equator() {
    let centroid = earthmesh_mesh::spherical_centroid_degrees(&[
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(90.0, 0.0),
    ])
    .expect("non-empty centroid");

    approx_eq(centroid.lon_degrees, 45.0, 1.0e-12);
    approx_eq(centroid.lat_degrees, 0.0, 1.0e-12);
}

#[test]
fn spherical_centroid_matches_fortran_vector_average_for_triangle() {
    let centroid = earthmesh_mesh::spherical_centroid_degrees(&[
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(90.0, 0.0),
        LonLatDegrees::new(0.0, 90.0),
    ])
    .expect("non-empty centroid");

    approx_eq(centroid.lon_degrees, 45.0, 1.0e-12);
    approx_eq(centroid.lat_degrees, 35.264389682754654, 1.0e-12);
}

#[test]
fn spherical_centroid_rejects_empty_input() {
    assert!(earthmesh_mesh::spherical_centroid_degrees(&[]).is_none());
}
