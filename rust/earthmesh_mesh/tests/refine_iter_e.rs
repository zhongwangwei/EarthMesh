use earthmesh_mesh::refine_iter_e_judge_fortran_indexed;

fn base_mrl_new() -> Vec<i32> {
    vec![0, 1, 1, 4, 4, 1, 1, 1, 4, 4, 1, 1, 1, 1]
}

#[test]
fn iter_e_marks_previous_triangle_when_first_opposite_cell_also_has_convex_pair() {
    let cells_on_triangle = vec![
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
        [2, 3, 1],
        [2, 4, 1],
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
        [3, 2, 1],
        [3, 4, 1],
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
    ];
    let triangles_on_cell = vec![
        vec![],
        vec![],
        vec![2, 3, 4, 5, 6, 7],
        vec![8, 9, 10, 11, 12, 13],
        vec![],
    ];
    let edge_counts = vec![0, 0, 6, 6, 0];
    let mrl_new = base_mrl_new();
    let ref_lbx = vec![0, 0, 1, 1, 0];

    let ref_sjx = refine_iter_e_judge_fortran_indexed(
        1,
        4,
        &cells_on_triangle,
        &triangles_on_cell,
        &edge_counts,
        &mrl_new,
        &ref_lbx,
    )
    .expect("calculate iterE convex-pair correction marks");

    assert_eq!(ref_sjx[2], 1);
    assert_eq!(ref_sjx.iter().sum::<i32>(), 1);
}

#[test]
fn iter_e_marks_after_pair_when_second_opposite_cell_has_convex_pair() {
    let cells_on_triangle = vec![
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
        [2, 5, 1],
        [2, 4, 1],
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
        [4, 2, 1],
        [4, 5, 1],
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
    ];
    let triangles_on_cell = vec![
        vec![],
        vec![],
        vec![2, 3, 4, 5, 6, 7],
        vec![],
        vec![8, 9, 10, 11, 12, 13],
        vec![],
    ];
    let edge_counts = vec![0, 0, 6, 0, 6, 0];
    let mrl_new = base_mrl_new();
    let ref_lbx = vec![0, 0, 1, 0, 1, 0];

    let ref_sjx = refine_iter_e_judge_fortran_indexed(
        1,
        5,
        &cells_on_triangle,
        &triangles_on_cell,
        &edge_counts,
        &mrl_new,
        &ref_lbx,
    )
    .expect("calculate iterE convex-pair correction marks");

    assert_eq!(ref_sjx[5], 1);
    assert_eq!(ref_sjx.iter().sum::<i32>(), 1);
}

#[test]
fn iter_e_rejects_triangle_ids_missing_from_cells_on_triangle() {
    let cells_on_triangle = vec![[0, 0, 0], [0, 0, 0], [0, 0, 0]];
    let triangles_on_cell = vec![vec![], vec![], vec![2, 3, 4, 5, 6, 7]];
    let edge_counts = vec![0, 0, 6];
    let mrl_new = vec![0, 1, 1, 4, 4, 1, 1, 1];
    let ref_lbx = vec![0, 0, 1];

    let err = refine_iter_e_judge_fortran_indexed(
        1,
        2,
        &cells_on_triangle,
        &triangles_on_cell,
        &edge_counts,
        &mrl_new,
        &ref_lbx,
    )
    .expect_err("missing triangle connectivity should be rejected");

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
}
