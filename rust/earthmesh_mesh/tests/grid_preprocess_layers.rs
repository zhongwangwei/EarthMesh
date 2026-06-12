use earthmesh_mesh::{distance_layers, find_frac_index_fortran, DistanceLayerSpacing};

fn approx_eq(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual {actual} expected {expected} tolerance {tolerance}"
    );
}

#[test]
fn find_frac_index_matches_fortran_ascending_grid() {
    let hit = find_frac_index_fortran(&[-180.0, -90.0, 0.0, 90.0, 180.0], -45.0)
        .expect("point inside ascending grid");

    assert_eq!(hit.index, 2);
    approx_eq(hit.frac, 0.5, 1.0e-15);
}

#[test]
fn find_frac_index_matches_fortran_descending_grid() {
    let hit = find_frac_index_fortran(&[90.0, 45.0, 0.0, -45.0, -90.0], 22.5)
        .expect("point inside descending grid");

    assert_eq!(hit.index, 2);
    approx_eq(hit.frac, 0.5, 1.0e-15);
}

#[test]
fn find_frac_index_clamps_boundary_fraction_like_fortran() {
    let left = find_frac_index_fortran(&[0.0, 10.0, 20.0], 0.0).expect("left boundary");
    assert_eq!(left.index, 1);
    approx_eq(left.frac, 0.0, 1.0e-15);

    let right = find_frac_index_fortran(&[0.0, 10.0, 20.0], 20.0).expect("right boundary");
    assert_eq!(right.index, 2);
    approx_eq(right.frac, 1.0, 1.0e-15);
}

#[test]
fn find_frac_index_rejects_points_outside_grid() {
    assert!(find_frac_index_fortran(&[0.0, 10.0, 20.0], -1.0).is_none());
    assert!(find_frac_index_fortran(&[0.0, 10.0, 20.0], 21.0).is_none());
}

#[test]
fn distance_layers_match_fortran_linear_formula() {
    let layers = distance_layers(4, 100.0, DistanceLayerSpacing::Linear).expect("layers");

    assert_eq!(layers.len(), 4);
    approx_eq(layers[0], 62.5, 1.0e-12);
    approx_eq(layers[1], 75.0, 1.0e-12);
    approx_eq(layers[2], 87.5, 1.0e-12);
    approx_eq(layers[3], 100.0, 1.0e-12);
}

#[test]
fn distance_layers_match_fortran_nonlinear_formulas_at_bounds() {
    let nonlinear1 = distance_layers(4, 100.0, DistanceLayerSpacing::Power).expect("layers");
    let nonlinear2 = distance_layers(4, 100.0, DistanceLayerSpacing::Exponential).expect("layers");
    let nonlinear3 = distance_layers(4, 100.0, DistanceLayerSpacing::Logarithmic).expect("layers");

    approx_eq(nonlinear1[3], 100.0, 1.0e-12);
    approx_eq(nonlinear2[3], 100.0, 1.0e-12);
    approx_eq(nonlinear3[3], 100.0, 1.0e-12);
    assert!(nonlinear1[0] > 50.0 && nonlinear1[0] < 100.0);
    assert!(nonlinear2[0] > 50.0 && nonlinear2[0] < 100.0);
    assert!(nonlinear3[0] > 50.0 && nonlinear3[0] < 100.0);
}

#[test]
fn distance_layers_reject_zero_length_output() {
    assert!(distance_layers(0, 100.0, DistanceLayerSpacing::Linear).is_none());
}
