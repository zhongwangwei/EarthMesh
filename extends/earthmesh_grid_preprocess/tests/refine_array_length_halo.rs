use earthmesh_grid_preprocess::refine_array_length_halo_one_based;

#[test]
fn array_length_halo_marks_initial_boundary_and_expands_transition_rows() {
    let mrl_new = vec![0, 1, 4, 1, 1];
    let mut triangles_on_cell = vec![vec![]; 5];
    triangles_on_cell[2] = vec![2, 3];
    triangles_on_cell[3] = vec![3, 4];
    triangles_on_cell[4] = vec![2, 4];
    let edge_counts = vec![0, 0, 2, 2, 2];

    let sizing = refine_array_length_halo_one_based(
        1,
        1,
        4,
        4,
        &mrl_new,
        &triangles_on_cell,
        &edge_counts,
        0,
    )
    .expect("halo sizing core");

    assert_eq!(sizing.initial_boundary_mask, vec![0, 0, 1, 0, 1]);
    assert_eq!(sizing.transition_boundary_mask, vec![0, 0, 0, 0, 0]);
    assert_eq!(sizing.boundary_refine, vec![2, 4]);
    assert_eq!(sizing.boundary_refine_transition, Vec::<usize>::new());
    assert_eq!(sizing.num_transition_row_triangles, 2);
    assert_eq!(sizing.expanded_mrl[3], 4);
    assert_eq!(sizing.expanded_mrl[4], 4);
}

#[test]
fn array_length_halo_zero_distance_keeps_transition_boundary_as_initial_boundary() {
    let mrl_new = vec![0, 1, 4, 1, 1];
    let mut triangles_on_cell = vec![vec![]; 5];
    triangles_on_cell[2] = vec![2, 3];
    triangles_on_cell[3] = vec![3, 4];
    triangles_on_cell[4] = vec![2, 4];
    let edge_counts = vec![0, 0, 2, 2, 2];

    let sizing = refine_array_length_halo_one_based(
        0,
        1,
        4,
        4,
        &mrl_new,
        &triangles_on_cell,
        &edge_counts,
        5,
    )
    .expect("zero-distance halo sizing core");

    assert_eq!(sizing.boundary_refine, vec![2, 4]);
    assert_eq!(sizing.boundary_refine_transition, vec![2, 4]);
    assert_eq!(sizing.num_transition_row_triangles, 5);
}

#[test]
fn array_length_halo_rejects_cell_neighbor_count_that_exceeds_row_storage() {
    let mrl_new = vec![0, 1, 4];
    let triangles_on_cell = vec![vec![], vec![], vec![2]];
    let edge_counts = vec![0, 0, 2];

    let err = refine_array_length_halo_one_based(
        1,
        1,
        2,
        2,
        &mrl_new,
        &triangles_on_cell,
        &edge_counts,
        0,
    )
    .expect_err("edge count must address triangles_on_cell row");

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
}
