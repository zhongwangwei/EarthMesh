use earthmesh_refine_redgreen::refine_iter_g_judge_one_based;

#[test]
fn iter_g_marks_unrefined_triangles_in_six_edge_weak_concavity() {
    // Canonical-indexed rows: polygons start after num_center.
    // Polygon 2 has six adjacent triangles with sum 18 = 4+4+4+4+1+1,
    // so iterG marks the two still-unrefined triangles.
    let triangles_on_cell = vec![vec![], vec![], vec![2, 3, 4, 5, 6, 7]];
    let edge_counts = vec![0, 0, 6];
    let mrl_new = vec![0, 1, 4, 4, 1, 4, 4, 1];

    let ref_sjx = refine_iter_g_judge_one_based(1, 2, &triangles_on_cell, &edge_counts, &mrl_new)
        .expect("calculate iterG weak-concavity marks");

    assert_eq!(ref_sjx, vec![0, 0, 0, 0, 1, 0, 0, 1]);
}

#[test]
fn iter_g_ignores_non_hexagonal_or_non_eighteen_sum_cells() {
    let triangles_on_cell = vec![vec![], vec![], vec![2, 3, 4, 5, 6], vec![2, 3, 4, 5, 6, 7]];
    let edge_counts = vec![0, 0, 5, 6];
    let mrl_new = vec![0, 1, 4, 4, 1, 4, 1, 1];

    let ref_sjx = refine_iter_g_judge_one_based(1, 3, &triangles_on_cell, &edge_counts, &mrl_new)
        .expect("calculate iterG weak-concavity marks");

    assert_eq!(ref_sjx, vec![0; mrl_new.len()]);
}

#[test]
fn iter_g_rejects_edge_count_that_exceeds_available_neighbors() {
    let triangles_on_cell = vec![vec![], vec![], vec![2, 3, 4, 5, 6]];
    let edge_counts = vec![0, 0, 6];
    let mrl_new = vec![0, 1, 4, 4, 1, 4, 4];

    let err = refine_iter_g_judge_one_based(1, 2, &triangles_on_cell, &edge_counts, &mrl_new)
        .expect_err("short neighbor row should be rejected");

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
}
