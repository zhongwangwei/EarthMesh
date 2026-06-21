use earthmesh_mesh::refine_isreverse_judge_fortran_indexed;

#[test]
fn isreverse_judge_marks_common_neighbors_and_rewrites_next_round_segments() {
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

    let ref_sjx = refine_isreverse_judge_fortran_indexed(
        3,
        2,
        &triangle_neighbors,
        &mrl_new,
        &mut segments,
        &n_segments,
    )
    .expect("judge reverse one-into-two markers");

    assert_eq!(
        ref_sjx[5], 1,
        "common neighbor of triangles 2 and 3 is marked"
    );
    assert_eq!(ref_sjx.iter().sum::<i32>(), 1);
    assert_eq!(
        segments[0],
        vec![3, 1, 1],
        "last non-refined neighbor of marked triangle is kept for next round"
    );
    assert_eq!(
        segments[1],
        vec![0, 0, 0],
        "inactive segment remains untouched"
    );
}

#[test]
fn isreverse_judge_skips_common_neighbor_without_unrefined_neighbor() {
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
    let mrl_new = vec![0, 1, 4, 4, 4, 1, 4, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1];
    let mut segments = vec![vec![2, 3, 1]];
    let n_segments = vec![2];

    let ref_sjx = refine_isreverse_judge_fortran_indexed(
        3,
        1,
        &triangle_neighbors,
        &mrl_new,
        &mut segments,
        &n_segments,
    )
    .expect("exhausted reverse boundary should not create an invalid split");

    assert_eq!(ref_sjx.iter().sum::<i32>(), 0);
    assert_eq!(segments[0], vec![1, 1, 1]);
}

#[test]
fn isreverse_judge_compacts_next_round_segments_after_exhausted_pair() {
    let mut triangle_neighbors = vec![vec![1, 1, 1]; 11];
    triangle_neighbors[2] = vec![5, 1, 1];
    triangle_neighbors[3] = vec![5, 6, 1];
    triangle_neighbors[4] = vec![6, 1, 1];
    triangle_neighbors[5] = vec![2, 3, 8];
    triangle_neighbors[6] = vec![3, 4, 7];
    let mut mrl_new = vec![1; 11];
    for triangle in [2, 3, 4, 8] {
        mrl_new[triangle] = 4;
    }
    let mut segments = vec![vec![2, 3, 4, 1]];
    let n_segments = vec![3];

    let ref_sjx = refine_isreverse_judge_fortran_indexed(
        4,
        1,
        &triangle_neighbors,
        &mrl_new,
        &mut segments,
        &n_segments,
    )
    .expect("reverse judge should keep later valid next-round candidates contiguous");

    assert_eq!(ref_sjx[5], 0, "exhausted common neighbor is skipped");
    assert_eq!(ref_sjx[6], 1, "later common neighbor is still marked");
    assert_eq!(
        segments[0],
        vec![7, 1, 1, 1],
        "next-round forward candidates must stay before the Fortran placeholder"
    );
}

#[test]
fn isreverse_judge_stops_at_fortran_placeholder_one() {
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

    let ref_sjx = refine_isreverse_judge_fortran_indexed(
        3,
        1,
        &triangle_neighbors,
        &mrl_new,
        &mut segments,
        &n_segments,
    )
    .expect("placeholder 1 terminates segment processing");

    assert_eq!(ref_sjx.iter().sum::<i32>(), 0);
    assert_eq!(segments[0], vec![1, 1, 1]);
}

#[test]
fn isreverse_judge_terminates_segment_without_common_triangle() {
    let triangle_neighbors = vec![
        vec![0, 0, 0],
        vec![0, 0, 0],
        vec![4, 5, 6],
        vec![7, 8, 9],
        vec![2, 5, 6],
        vec![2, 4, 6],
        vec![2, 4, 5],
        vec![3, 8, 9],
        vec![3, 7, 9],
        vec![3, 7, 8],
    ];
    let mrl_new = vec![0; 10];
    let mut segments = vec![vec![2, 3]];
    let n_segments = vec![2];

    let ref_sjx = refine_isreverse_judge_fortran_indexed(
        2,
        1,
        &triangle_neighbors,
        &mrl_new,
        &mut segments,
        &n_segments,
    )
    .expect("disconnected segment pair should terminate this segment");

    assert_eq!(ref_sjx.iter().sum::<i32>(), 0);
    assert_eq!(segments[0], vec![1, 1]);
}
