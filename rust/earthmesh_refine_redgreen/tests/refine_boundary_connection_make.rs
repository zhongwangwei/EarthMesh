use earthmesh_refine_redgreen::refine_boundary_connection_make_one_based;

#[test]
fn refine_boundary_connection_make_builds_closed_curve_from_unrefined_boundary_triangles() {
    let sjx_points = 9;
    let lbx_points = 13;
    let mut mrl = vec![0; sjx_points + 1];
    let mut triangle_neighbors = vec![vec![1, 1, 1]; sjx_points + 1];
    let mut cells_on_triangle = vec![[0, 0, 0]; sjx_points + 1];

    for triangle in 2..=9 {
        mrl[triangle] = 1;
    }
    for refined in [6, 7, 8, 9] {
        mrl[refined] = 4;
    }

    triangle_neighbors[2] = vec![6, 3, 5];
    triangle_neighbors[3] = vec![7, 4, 2];
    triangle_neighbors[4] = vec![8, 5, 3];
    triangle_neighbors[5] = vec![9, 2, 4];
    cells_on_triangle[2] = [10, 11, 99];
    cells_on_triangle[3] = [11, 12, 99];
    cells_on_triangle[4] = [12, 13, 99];
    cells_on_triangle[5] = [13, 10, 99];
    cells_on_triangle[6] = [10, 11, 90];
    cells_on_triangle[7] = [11, 12, 91];
    cells_on_triangle[8] = [12, 13, 92];
    cells_on_triangle[9] = [13, 10, 93];

    let boundary = refine_boundary_connection_make_one_based(
        1,
        sjx_points,
        lbx_points,
        &mrl,
        &triangle_neighbors,
        &cells_on_triangle,
    )
    .expect("refine boundary connection make");

    assert_eq!(boundary.bdy_num_in, 5);
    assert_eq!(boundary.boundary_order, vec![1, 10, 11, 12, 13]);
    assert_eq!(boundary.boundary_neighbors[10], vec![11, 13]);
    assert_eq!(boundary.boundary_neighbors[11], vec![10, 12]);
    assert_eq!(boundary.curves.num_closed_curve, 1);
    assert_eq!(boundary.curves.num_bdy_long, [5, 1, 1]);
    assert_eq!(boundary.curves.close_curves[1], vec![10, 11, 12, 13]);
    assert_eq!(boundary.curves.n_close_curve[1], 4);
}

#[test]
fn refine_boundary_connection_make_rejects_open_boundary_vertex_degree_one() {
    let sjx_points = 4;
    let lbx_points = 12;
    let mut mrl = vec![0; sjx_points + 1];
    let mut triangle_neighbors = vec![vec![1, 1, 1]; sjx_points + 1];
    let mut cells_on_triangle = vec![[0, 0, 0]; sjx_points + 1];
    mrl[2] = 1;
    mrl[3] = 4;
    mrl[4] = 1;
    triangle_neighbors[2] = vec![3, 4, 4];
    cells_on_triangle[2] = [10, 11, 99];
    cells_on_triangle[3] = [10, 11, 90];

    let err = refine_boundary_connection_make_one_based(
        1,
        sjx_points,
        lbx_points,
        &mrl,
        &triangle_neighbors,
        &cells_on_triangle,
    )
    .expect_err("open refine boundary should be rejected");

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
}
