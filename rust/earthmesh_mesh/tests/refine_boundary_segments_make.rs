use earthmesh_mesh::refine_boundary_segments_make_fortran_indexed;

#[test]
fn boundary_segments_make_splits_each_edge_when_transition_distance_is_one() {
    let mut triangles_on_cell = vec![Vec::<usize>::new(); 14];
    let mut edge_counts = vec![0usize; 14];
    let mut mrl = vec![0i32; 10];
    for triangle in 2..=5 {
        mrl[triangle] = 1;
    }
    for triangle in 6..=9 {
        mrl[triangle] = 4;
    }

    triangles_on_cell[10] = vec![5, 2, 6];
    triangles_on_cell[11] = vec![2, 3, 7];
    triangles_on_cell[12] = vec![3, 4, 8];
    triangles_on_cell[13] = vec![4, 5, 9];
    for cell in 10..=13 {
        edge_counts[cell] = triangles_on_cell[cell].len();
    }

    let segments = refine_boundary_segments_make_fortran_indexed(
        1,
        &[vec![10, 11, 12, 13]],
        &triangles_on_cell,
        &edge_counts,
        &mrl,
    )
    .expect("boundary refine segments");

    assert_eq!(segments.num_bdy_refine_segment, 4);
    assert_eq!(segments.n_bdy_refine_segment, vec![1, 1, 1, 1]);
    assert_eq!(
        segments.bdy_refine_segment,
        vec![vec![2], vec![3], vec![4], vec![5]]
    );
}

#[test]
fn boundary_segments_make_rotates_from_first_turn_and_splits_long_runs() {
    let mut triangles_on_cell = vec![Vec::<usize>::new(); 15];
    let mut edge_counts = vec![0usize; 15];
    let mut mrl = vec![0i32; 43];
    for triangle in 20..=24 {
        mrl[triangle] = 1;
    }
    for triangle in 30..=42 {
        mrl[triangle] = 4;
    }

    triangles_on_cell[10] = vec![24, 20, 30, 31, 32];
    triangles_on_cell[11] = vec![20, 21, 33, 34, 35];
    triangles_on_cell[12] = vec![21, 22, 36];
    triangles_on_cell[13] = vec![22, 23, 37, 38, 39];
    triangles_on_cell[14] = vec![23, 24, 40, 41, 42];
    for cell in 10..=14 {
        edge_counts[cell] = triangles_on_cell[cell].len();
    }

    let segments = refine_boundary_segments_make_fortran_indexed(
        3,
        &[vec![10, 11, 12, 13, 14]],
        &triangles_on_cell,
        &edge_counts,
        &mrl,
    )
    .expect("rotated split boundary refine segments");

    assert_eq!(segments.num_bdy_refine_segment, 2);
    assert_eq!(segments.n_bdy_refine_segment, vec![3, 2]);
    assert_eq!(
        segments.bdy_refine_segment,
        vec![vec![22, 23, 24], vec![20, 21]]
    );
}

#[test]
fn boundary_segments_make_rejects_edge_without_unrefined_triangle() {
    let mut triangles_on_cell = vec![Vec::<usize>::new(); 12];
    let mut edge_counts = vec![0usize; 12];
    let mut mrl = vec![0i32; 7];
    mrl[2] = 1;
    mrl[3] = 4;
    mrl[4] = 1;
    mrl[5] = 1;
    mrl[6] = 1;
    triangles_on_cell[10] = vec![2, 3];
    triangles_on_cell[11] = vec![4, 5];
    edge_counts[10] = 2;
    edge_counts[11] = 2;

    let err = refine_boundary_segments_make_fortran_indexed(
        1,
        &[vec![10, 11]],
        &triangles_on_cell,
        &edge_counts,
        &mrl,
    )
    .expect_err("missing common unrefined triangle should be rejected");

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
}
