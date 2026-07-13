use earthmesh_mesh::refine_array_length_calculation_one_based;

#[test]
fn array_length_calculation_combines_halo_sizing_with_refine_close_curves() {
    let sjx_points = 9;
    let lbx_points = 13;
    let mut mrl = vec![0; sjx_points + 1];
    let mut triangle_neighbors = vec![vec![1, 1, 1]; sjx_points + 1];
    let mut cells_on_triangle = vec![[0, 0, 0]; sjx_points + 1];
    let mut triangles_on_cell = vec![Vec::<usize>::new(); lbx_points + 1];
    let mut edge_counts = vec![0usize; lbx_points + 1];

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

    triangles_on_cell[10] = vec![5, 2, 6];
    triangles_on_cell[11] = vec![2, 3, 7];
    triangles_on_cell[12] = vec![3, 4, 8];
    triangles_on_cell[13] = vec![4, 5, 9];
    for cell in 10..=13 {
        edge_counts[cell] = triangles_on_cell[cell].len();
    }

    let result = refine_array_length_calculation_one_based(
        1,
        1,
        9,
        sjx_points,
        lbx_points,
        &mrl,
        &triangle_neighbors,
        &cells_on_triangle,
        &triangles_on_cell,
        &edge_counts,
        0,
    )
    .expect("Array_length_calculation pure wrapper");

    assert_eq!(result.halo.boundary_refine, vec![10, 11, 12, 13]);
    assert_eq!(result.halo.boundary_refine_transition, Vec::<usize>::new());
    assert_eq!(result.halo.num_transition_row_triangles, 4);
    assert_eq!(result.boundary.curves.num_closed_curve, 1);
    assert_eq!(result.boundary.curves.close_curves[1], vec![10, 11, 12, 13]);
}
