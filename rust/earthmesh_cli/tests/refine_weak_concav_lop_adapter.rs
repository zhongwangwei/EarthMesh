use earthmesh_cli::apply_weak_concav_lop_judge_fortran_indexed;

#[test]
fn weak_concav_lop_adapter_builds_pair_only_segment_from_fortran_rows() {
    let mrl_new = vec![1; 10];
    let triangle_neighbors = vec![vec![1, 1, 1]; 10];
    let mut ngrmw_new = vec![vec![0; 90]; 4];
    ngrmw_new[1][20] = 1;
    ngrmw_new[2][20] = 2;
    ngrmw_new[3][20] = 3;
    ngrmw_new[1][61] = 2;
    ngrmw_new[2][61] = 3;
    ngrmw_new[3][61] = 4;
    let mut sjx_child = vec![[0, 0]; 10];
    sjx_child[2] = [20, 21];
    sjx_child[6] = [60, 61];
    let mut weak_segment = vec![vec![]];
    let weak_segment_old = vec![vec![]];
    let n_weak_segment = vec![0];
    let weak_pair = vec![[0, 0], [2, 6]];
    let mut ref_temp = vec![vec![0; 6]; 3];
    let mut n_ref_temp = vec![0; 3];
    let mut num_ref = 0;

    let report = apply_weak_concav_lop_judge_fortran_indexed(
        &mut num_ref,
        1,
        0,
        0,
        1,
        89,
        &mrl_new,
        &triangle_neighbors,
        &ngrmw_new,
        &sjx_child,
        &mut weak_segment,
        &weak_segment_old,
        &n_weak_segment,
        &weak_pair,
        &mut ref_temp,
        &mut n_ref_temp,
    )
    .expect("apply weak-concavity LOP through CLI adapter");

    assert_eq!(ref_temp[2][1..=2], [20, 61]);
    assert_eq!(n_ref_temp[2], 2);
    assert_eq!(num_ref, 2);
    assert_eq!(report.num_ref_added, 2);
    assert_eq!(report.segment_lengths, vec![(2, 2)]);
    assert_eq!(report.written_segments, vec![(2, vec![20, 61])]);
}
