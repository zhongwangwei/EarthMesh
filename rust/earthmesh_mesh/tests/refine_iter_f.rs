use earthmesh_mesh::refine_iter_f_judge_fortran_indexed;

#[test]
fn iter_f_marks_zero_state_triangles_inside_original_vertex_protection_ring() {
    let triangles_on_cell = vec![vec![], vec![], vec![2, 3, 4, 5, 6]];
    let edge_counts = vec![0, 0, 5];
    let mrl_new = vec![0, 1, 1, 0, 4, 0, 4];
    let impent = vec![2];

    let ref_sjx = refine_iter_f_judge_fortran_indexed(
        6,
        2,
        &triangles_on_cell,
        &edge_counts,
        &mrl_new,
        &impent,
        0,
    )
    .expect("calculate iterF protected-ring marks");

    assert_eq!(ref_sjx, vec![0, 0, 0, 1, 0, 1, 0]);
}

#[test]
fn iter_f_expands_protection_ring_by_boundary_cells_for_requested_layers() {
    let triangles_on_cell = vec![
        vec![],
        vec![],
        vec![2, 3, 4, 5, 6],
        vec![6, 7, 8],
        vec![8, 9, 10],
    ];
    let edge_counts = vec![0, 0, 5, 3, 3];
    let mrl_new = vec![0, 1, 1, 4, 4, 4, 4, 0, 0, 0, 0];
    let impent = vec![2];

    let ref_sjx = refine_iter_f_judge_fortran_indexed(
        10,
        4,
        &triangles_on_cell,
        &edge_counts,
        &mrl_new,
        &impent,
        1,
    )
    .expect("calculate iterF expanded protection marks");

    assert_eq!(ref_sjx[7], 1);
    assert_eq!(ref_sjx[8], 1);
    assert_eq!(ref_sjx[9], 0);
    assert_eq!(ref_sjx[10], 0);
}

#[test]
fn iter_f_does_not_mark_when_protected_region_has_no_one_state_triangles() {
    let triangles_on_cell = vec![vec![], vec![], vec![2, 3, 4, 5, 6]];
    let edge_counts = vec![0, 0, 5];
    let mrl_new = vec![0, 1, 0, 0, 4, 0, 4];
    let impent = vec![2];

    let ref_sjx = refine_iter_f_judge_fortran_indexed(
        6,
        2,
        &triangles_on_cell,
        &edge_counts,
        &mrl_new,
        &impent,
        0,
    )
    .expect("calculate iterF protected-ring marks");

    assert_eq!(ref_sjx, vec![0; mrl_new.len()]);
}
