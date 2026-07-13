use earthmesh_mesh::refine_iter_b_judge_one_based;

#[test]
fn iter_b_marks_unrefined_cell_hit_by_two_existing_four_refinements() {
    // Canonical-indexed rows. Cells 4 and 5 are already refined (mrl_new=4).
    // Both list cell 2 as a neighbor, so iterB gives cell 2 two transition
    // marks (2+2) and emits ref_sjx(2)=1.
    let ngrmm = vec![
        vec![0, 0, 0],
        vec![0, 0, 0],
        vec![3, 4, 5],
        vec![2, 4, 6],
        vec![2, 3, 6],
        vec![2, 7, 8],
        vec![3, 4, 7],
        vec![5, 6, 8],
        vec![5, 7, 6],
    ];
    let mrl_new = vec![0, 1, 1, 1, 4, 4, 1, 1, 1];

    let ref_sjx = refine_iter_b_judge_one_based(1, 1, &ngrmm, &mrl_new)
        .expect("calculate iterB refinement marks");

    assert_eq!(ref_sjx, vec![0, 0, 1, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn iter_b_keeps_existing_four_cells_unmarked_even_with_transition_marks() {
    let ngrmm = vec![
        vec![0, 0, 0],
        vec![0, 0, 0],
        vec![3, 4, 5],
        vec![2, 4, 5],
        vec![2, 3, 6],
        vec![2, 3, 6],
        vec![4, 5, 2],
    ];
    let mrl_new = vec![0, 1, 4, 1, 4, 4, 1];

    let ref_sjx = refine_iter_b_judge_one_based(1, 1, &ngrmm, &mrl_new)
        .expect("calculate iterB refinement marks");

    assert_eq!(ref_sjx[2], 0);
    assert_eq!(ref_sjx[4], 0);
    assert_eq!(ref_sjx[5], 0);
}

#[test]
fn iter_b_rejects_zero_neighbor_ids_like_invalid_canonical_connectivity() {
    let ngrmm = vec![
        vec![0, 0, 0],
        vec![0, 0, 0],
        vec![0, 3, 4],
        vec![2, 4, 2],
        vec![2, 3, 2],
    ];
    let mrl_new = vec![0, 1, 1, 4, 4];

    let err = refine_iter_b_judge_one_based(1, 1, &ngrmm, &mrl_new)
        .expect_err("zero neighbor should be rejected");

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(err.to_string().contains("invalid neighbor 0"));
}
