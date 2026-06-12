use earthmesh_mesh::{arc_length_unit_sphere, lonlat_degrees_to_unit_xyz, normalize_lon_m180_180, LonLatDegrees};

fn approx_eq(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual {actual} expected {expected} tolerance {tolerance}"
    );
}

#[test]
fn normalize_lon_matches_mod_grid_preprocess_checklon_single_wrap() {
    approx_eq(normalize_lon_m180_180(181.0), -179.0, 0.0);
    approx_eq(normalize_lon_m180_180(-181.0), 179.0, 0.0);
    approx_eq(normalize_lon_m180_180(180.0), 180.0, 0.0);
    approx_eq(normalize_lon_m180_180(-180.0), -180.0, 0.0);
    // Fortran CheckLon performs one adjustment, not repeated modulo wrapping.
    approx_eq(normalize_lon_m180_180(541.0), 181.0, 0.0);
}

#[test]
fn arc_length_matches_mod_grid_preprocess_equator_quarter_turn() {
    let a = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 0.0));
    let b = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(90.0, 0.0));

    approx_eq(arc_length_unit_sphere(a, b), 1.5707962671902518, 1.0e-12);
}

#[test]
fn arc_length_matches_mod_grid_preprocess_one_degree_equator() {
    let a = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 0.0));
    let b = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(1.0, 0.0));

    approx_eq(arc_length_unit_sphere(a, b), 0.01745329238890877, 1.0e-12);
}

#[test]
fn arc_length_scales_by_input_radius_like_fortran() {
    let a = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 0.0));
    let b = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(1.0, 0.0));
    let scaled_a = earthmesh_mesh::CartesianPoint::new(a.x * 2.0, a.y * 2.0, a.z * 2.0);
    let scaled_b = earthmesh_mesh::CartesianPoint::new(b.x * 2.0, b.y * 2.0, b.z * 2.0);

    approx_eq(arc_length_unit_sphere(scaled_a, scaled_b), 2.0 * arc_length_unit_sphere(a, b), 1.0e-12);
}
