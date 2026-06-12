use earthmesh_cli::apply_isreverse_judge_fortran_indexed;

#[test]
fn isreverse_judge_adapter_marks_reverse_split_candidates_and_rewrites_segments() {
    let triangle_neighbors = vec![
        vec![0, 0, 0],
        vec![0, 0, 0],
        vec![4, 5, 7],
        vec![5, 6, 8],
        vec![2, 5, 9],
        vec![2, 3, 4],
        vec![3, 5, 10],
        vec![2, 11, 12],
        vec![3, 13, 14],
        vec![4, 15, 16],
        vec![6, 17, 18],
        vec![2, 3, 4],
        vec![2, 3, 4],
        vec![2, 3, 4],
        vec![2, 3, 4],
        vec![2, 3, 4],
        vec![2, 3, 4],
        vec![2, 3, 4],
        vec![2, 3, 4],
    ];
    let mrl_new = vec![0, 1, 1, 1, 4, 1, 4, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1];
    let mut segments = vec![vec![2, 3, 1], vec![0, 0, 0]];
    let n_segments = vec![2, 0];

    let report = apply_isreverse_judge_fortran_indexed(
        3,
        2,
        &triangle_neighbors,
        &mrl_new,
        &mut segments,
        &n_segments,
    )
    .expect("apply reverse one-into-two marker adapter");

    assert_eq!(report.marked_triangles, vec![5]);
    assert_eq!(report.active_segments, vec![0]);
    assert_eq!(report.rewritten_segments, vec![vec![3, 1, 1]]);
    assert_eq!(report.ref_sjx[5], 1);
    assert_eq!(report.ref_sjx.iter().sum::<i32>(), 1);
    assert_eq!(segments[0], vec![3, 1, 1]);
    assert_eq!(segments[1], vec![0, 0, 0]);
}

#[test]
fn isreverse_judge_adapter_keeps_fortran_placeholder_termination() {
    let triangle_neighbors = vec![
        vec![0, 0, 0],
        vec![0, 0, 0],
        vec![4, 5, 7],
        vec![5, 6, 8],
        vec![2, 5, 9],
        vec![2, 3, 4],
        vec![3, 5, 10],
        vec![2, 3, 4],
        vec![2, 3, 4],
        vec![2, 3, 4],
        vec![2, 3, 4],
    ];
    let mrl_new = vec![0, 1, 1, 1, 4, 1, 4, 1, 1, 1, 1];
    let mut segments = vec![vec![2, 1, 3]];
    let n_segments = vec![1];

    let report = apply_isreverse_judge_fortran_indexed(
        3,
        1,
        &triangle_neighbors,
        &mrl_new,
        &mut segments,
        &n_segments,
    )
    .expect("placeholder one terminates segment processing");

    assert!(report.marked_triangles.is_empty());
    assert_eq!(report.active_segments, vec![0]);
    assert_eq!(report.rewritten_segments, vec![vec![1, 1, 1]]);
    assert_eq!(segments[0], vec![1, 1, 1]);
}
