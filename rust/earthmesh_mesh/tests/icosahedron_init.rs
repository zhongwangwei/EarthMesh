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

#[test]
fn icosahedron_fill_diamonds_matches_fortran_first_southern_diamond_nxp1() {
    let connectivity = earthmesh_mesh::icosahedron_fill_diamonds_fortran(1)
        .expect("valid nxp fill_diamond connectivity");

    assert_eq!(connectivity.u_edges.len(), 32);
    assert_eq!(connectivity.w_faces.len(), 22);

    assert_eq!(connectivity.u_edges[3].im, [3, 4]);
    assert_eq!(connectivity.u_edges[3].iw[0..2], [2, 3]);
    assert_eq!(connectivity.u_edges[3].mrlu, 1);

    assert_eq!(connectivity.u_edges[2].im, [3, 2]);
    assert_eq!(connectivity.u_edges[2].iw[1], 2);
    assert_eq!(connectivity.u_edges[2].mrlu, 1);

    assert_eq!(connectivity.u_edges[4].im, [8, 3]);
    assert_eq!(connectivity.u_edges[4].iw[1], 3);
    assert_eq!(connectivity.u_edges[4].mrlu, 1);

    assert_eq!(connectivity.u_edges[5].iw[0], 2);
    assert_eq!(connectivity.u_edges[17].iw[0], 3);

    assert_eq!(connectivity.w_faces[2].iu, [3, 2, 5]);
    assert_eq!(connectivity.w_faces[2].mrlw, 1);
    assert_eq!(connectivity.w_faces[2].mrlw_orig, 1);
    assert_eq!(connectivity.w_faces[2].ngr, 1);

    assert_eq!(connectivity.w_faces[3].iu, [3, 17, 4]);
    assert_eq!(connectivity.w_faces[3].mrlw, 1);
    assert_eq!(connectivity.w_faces[3].mrlw_orig, 1);
    assert_eq!(connectivity.w_faces[3].ngr, 1);
}
