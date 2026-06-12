use earthmesh_cli::apply_sharp_concav_lop_judge_fortran_indexed;

fn child_rows() -> Vec<Vec<usize>> {
    let mut rows = vec![vec![0; 80]; 4];
    let mut set = |triangle: usize, vertices: [usize; 3]| {
        rows[1][triangle] = vertices[0];
        rows[2][triangle] = vertices[1];
        rows[3][triangle] = vertices[2];
    };
    set(20, [1, 2, 3]);
    set(21, [90, 91, 92]);
    set(30, [10, 11, 13]);
    set(31, [30, 31, 32]);
    set(40, [60, 61, 62]);
    set(50, [40, 41, 42]);
    set(51, [2, 3, 4]);
    set(60, [10, 11, 12]);
    set(61, [2, 3, 4]);
    rows
}

#[test]
fn sharp_concav_lop_adapter_builds_single_transition_pair_from_fortran_rows() {
    let mut mrl_new = vec![1; 8];
    mrl_new[6] = 4;
    let mut triangle_neighbors = vec![vec![1, 1, 1]; 8];
    triangle_neighbors[4] = vec![5, 6, 7];
    let mut sjx_child = vec![[0, 0]; 8];
    sjx_child[2] = [20, 21];
    sjx_child[3] = [30, 31];
    sjx_child[6] = [60, 61];
    let bdy_refine_segment = vec![vec![], vec![0, 4]];
    let bdy_refine_segment_old = vec![vec![], vec![0, 2, 3]];
    let n_bdy_refine_segment = vec![0, 2];
    let mut ref_temp = vec![vec![0; 9]; 2];
    let mut n_ref_temp = vec![0, 1];
    let mut num_ref = 0;

    let report = apply_sharp_concav_lop_judge_fortran_indexed(
        &mut num_ref,
        1,
        79,
        &mrl_new,
        &triangle_neighbors,
        &child_rows(),
        &sjx_child,
        &bdy_refine_segment,
        &bdy_refine_segment_old,
        &n_bdy_refine_segment,
        &mut ref_temp,
        &mut n_ref_temp,
    )
    .expect("apply strong-concavity LOP through CLI adapter");

    assert_eq!(ref_temp[1][1..=4], [20, 61, 60, 30]);
    assert_eq!(n_ref_temp[1], 4);
    assert_eq!(num_ref, 4);
    assert_eq!(report.num_ref_added, 4);
    assert_eq!(report.segment_lengths, vec![(1, 4)]);
    assert_eq!(report.written_segments, vec![(1, vec![20, 61, 60, 30])]);
}
