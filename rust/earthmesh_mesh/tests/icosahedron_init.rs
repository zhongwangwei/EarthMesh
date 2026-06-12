use earthmesh_core::EARTH_RADIUS_METERS;
use earthmesh_mesh::{icosahedron_initial_grid_fortran, CartesianPoint};

fn approx_eq(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual {actual} expected {expected} tolerance {tolerance}"
    );
}

fn magnitude(point: CartesianPoint) -> f64 {
    (point.x * point.x + point.y * point.y + point.z * point.z).sqrt()
}

#[test]
fn icosahedron_initial_grid_counts_and_pentagon_indices_match_fortran_nxp1() {
    let grid = icosahedron_initial_grid_fortran(1).expect("valid nxp");

    assert_eq!(grid.nmd, 13);
    assert_eq!(grid.nud, 31);
    assert_eq!(grid.nwd, 21);
    assert_eq!(grid.impent, [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]);

    approx_eq(grid.m_points[2].x, 0.0, 1.0e-9);
    approx_eq(grid.m_points[2].y, 0.0, 1.0e-9);
    approx_eq(grid.m_points[2].z, -EARTH_RADIUS_METERS, 1.0e-9);
    approx_eq(grid.m_points[13].x, 0.0, 1.0e-9);
    approx_eq(grid.m_points[13].y, 0.0, 1.0e-9);
    approx_eq(grid.m_points[13].z, EARTH_RADIUS_METERS, 1.0e-9);
}

#[test]
fn icosahedron_initial_grid_projects_all_active_points_to_earth_radius() {
    let grid = icosahedron_initial_grid_fortran(2).expect("valid nxp");

    assert_eq!(grid.nmd, 43);
    assert_eq!(grid.nud, 121);
    assert_eq!(grid.nwd, 81);
    assert_eq!(grid.impent[0], 2);
    assert_eq!(grid.impent[11], 43);
    assert_eq!(grid.impent[1], 5);
    assert_eq!(grid.impent[6], 25);

    for point_id in 2..grid.m_points.len() {
        approx_eq(
            magnitude(grid.m_points[point_id]),
            EARTH_RADIUS_METERS,
            1.0e-6,
        );
    }
}
