use earthmesh_mesh::refine_iter_c_judge_one_based;

#[test]
fn iter_c_marks_pentagon_unrefined_triangles_when_refined_neighbors_exceed_limit() {
    let triangle_neighbors = vec![
        vec![0, 0, 0],
        vec![0, 0, 0],
        vec![3, 4, 5],
        vec![2, 4, 5],
        vec![2, 3, 6],
        vec![2, 3, 6],
        vec![4, 5, 2],
    ];
    let triangles_on_cell = vec![vec![], vec![], vec![2, 3, 4, 5, 6]];
    let edge_counts = vec![0, 0, 5];
    let mrl_new = vec![0, 1, 4, 4, 1, 1, 1];
    let ref_lbx = vec![0, 0, 1];

    let ref_sjx = refine_iter_c_judge_one_based(
        1,
        1,
        1,
        2,
        &triangle_neighbors,
        &triangles_on_cell,
        &edge_counts,
        &mrl_new,
        &ref_lbx,
    )
    .expect("calculate iterC pentagon marks");

    assert_eq!(ref_sjx[2], 0);
    assert_eq!(ref_sjx[3], 0);
    assert_eq!(ref_sjx[4], 1);
    assert_eq!(ref_sjx[5], 1);
    assert_eq!(ref_sjx[6], 1);
}

#[test]
fn iter_c_marks_opposite_hex_gap_triangles_when_two_refined_triangles_face_each_other() {
    let triangle_neighbors = vec![
        vec![0, 0, 0],
        vec![0, 0, 0],
        vec![3, 4, 5],
        vec![2, 4, 6],
        vec![2, 3, 5],
        vec![2, 4, 6],
        vec![3, 5, 2],
        vec![3, 5, 6],
    ];
    let triangles_on_cell = vec![vec![], vec![], vec![2, 3, 4, 5, 6, 7]];
    let edge_counts = vec![0, 0, 6];
    let mrl_new = vec![0, 1, 4, 1, 1, 4, 1, 1];
    let ref_lbx = vec![0, 0, 1];

    let ref_sjx = refine_iter_c_judge_one_based(
        1,
        1,
        1,
        2,
        &triangle_neighbors,
        &triangles_on_cell,
        &edge_counts,
        &mrl_new,
        &ref_lbx,
    )
    .expect("calculate iterC opposite-hex marks");

    assert_eq!(ref_sjx[3], 1);
    assert_eq!(ref_sjx[4], 1);
    assert_eq!(ref_sjx.iter().sum::<i32>(), 2);
}

#[test]
fn iter_c_marks_single_refined_hex_neighbors_when_incoming_transition_would_exceed_seven_edges() {
    // Cell 2 has exactly one refined triangle (2), so its state sum is 9.
    // Transition propagation from external refined triangles creates incoming
    // marks at cell slots 3 and 5 (triangles 6 and 8).  With the original
    // refined triangle this would exceed the seven-edge cap, so iterC marks
    // every unrefined triangle around cell 2.
    let triangle_neighbors = vec![
        vec![0, 0, 0],
        vec![0, 0, 0],
        vec![3, 7, 9],
        vec![2, 4, 5],
        vec![3, 5, 7],
        vec![3, 4, 7],
        vec![3, 4, 7],
        vec![2, 4, 5],
        vec![3, 4, 7],
        vec![2, 3, 7],
    ];
    let triangles_on_cell = vec![vec![], vec![], vec![2, 3, 6, 7, 8, 9], vec![]];
    let edge_counts = vec![0, 0, 6, 0];
    let mrl_new = vec![0, 1, 4, 1, 4, 4, 1, 1, 1, 1];
    let ref_lbx = vec![0, 0, 1, 0];

    let ref_sjx = refine_iter_c_judge_one_based(
        2,
        1,
        1,
        2,
        &triangle_neighbors,
        &triangles_on_cell,
        &edge_counts,
        &mrl_new,
        &ref_lbx,
    )
    .expect("calculate iterC single-refined hex transition marks");

    assert_eq!(ref_sjx[2], 0);
    for triangle in [3, 6, 7, 8, 9] {
        assert_eq!(ref_sjx[triangle], 1, "triangle {triangle} should be marked");
    }
}

#[test]
fn iter_c_rejects_invalid_triangle_neighbor_connectivity() {
    let triangle_neighbors = vec![vec![0, 0, 0], vec![0, 0, 0], vec![0, 3, 4]];
    let triangles_on_cell = vec![vec![], vec![], vec![2, 3, 4]];
    let edge_counts = vec![0, 0, 3];
    let mrl_new = vec![0, 1, 4, 1, 1];
    let ref_lbx = vec![0, 0, 1];

    let err = refine_iter_c_judge_one_based(
        1,
        1,
        1,
        2,
        &triangle_neighbors,
        &triangles_on_cell,
        &edge_counts,
        &mrl_new,
        &ref_lbx,
    )
    .expect_err("invalid zero triangle neighbor should be rejected");

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
}
