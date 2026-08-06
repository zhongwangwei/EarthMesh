use earthmesh_refine_redgreen::refine_weak_concav_lop_judge_one_based;

#[test]
fn weak_concav_lop_judge_builds_pair_only_segments() {
    let mrl_new = vec![1; 10];
    let triangle_neighbors = vec![vec![1, 1, 1]; 10];
    let mut vertices = vec![[0, 0, 0]; 90];
    vertices[20] = [1, 2, 3];
    vertices[61] = [2, 3, 4];
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

    refine_weak_concav_lop_judge_one_based(
        &mut num_ref,
        1,
        0,
        0,
        1,
        &mrl_new,
        &triangle_neighbors,
        &vertices,
        &sjx_child,
        &mut weak_segment,
        &weak_segment_old,
        &n_weak_segment,
        &weak_pair,
        &mut ref_temp,
        &mut n_ref_temp,
    )
    .expect("pair-only weak concavity LOP segment");

    assert_eq!(ref_temp[2][1..=2], [20, 61]);
    assert_eq!(n_ref_temp[2], 2);
    assert_eq!(num_ref, 2);
}

#[test]
fn weak_concav_lop_judge_builds_intersegment_and_internal_pairs() {
    let mut mrl_new = vec![1; 10];
    mrl_new[8] = 4;
    let mut triangle_neighbors = vec![vec![1, 1, 1]; 10];
    triangle_neighbors[7] = vec![5, 8, 9];
    let mut vertices = vec![[0, 0, 0]; 100];
    vertices[20] = [1, 2, 3];
    vertices[61] = [2, 3, 4];
    vertices[30] = [10, 11, 12];
    vertices[81] = [11, 12, 13];
    let mut sjx_child = vec![[0, 0]; 10];
    sjx_child[2] = [20, 21];
    sjx_child[3] = [30, 31];
    sjx_child[6] = [60, 61];
    sjx_child[8] = [80, 81];
    let mut weak_segment = vec![vec![], vec![7], vec![1]];
    let weak_segment_old = vec![vec![], vec![3, 2], vec![6]];
    let n_weak_segment = vec![0, 1, 0];
    let weak_pair = vec![[0, 0]];
    let mut ref_temp = vec![vec![0; 8]; 4];
    let mut n_ref_temp = vec![0; 4];
    let mut num_ref = 0;

    refine_weak_concav_lop_judge_one_based(
        &mut num_ref,
        1,
        2,
        2,
        0,
        &mrl_new,
        &triangle_neighbors,
        &vertices,
        &sjx_child,
        &mut weak_segment,
        &weak_segment_old,
        &n_weak_segment,
        &weak_pair,
        &mut ref_temp,
        &mut n_ref_temp,
    )
    .expect("weak concavity intersegment and internal LOP pairs");

    assert_eq!(ref_temp[2][1..=4], [20, 61, 30, 81]);
    assert_eq!(n_ref_temp[2], 4);
    assert_eq!(num_ref, 4);
}

#[test]
fn weak_concav_lop_judge_clears_empty_odd_segment_pair_after_intersegment_pair() {
    let mrl_new = vec![1; 10];
    let triangle_neighbors = vec![vec![1, 1, 1]; 10];
    let mut vertices = vec![[0, 0, 0]; 90];
    vertices[20] = [1, 2, 3];
    vertices[61] = [2, 3, 4];
    let mut sjx_child = vec![[0, 0]; 10];
    sjx_child[2] = [20, 21];
    sjx_child[6] = [60, 61];
    let mut weak_segment = vec![vec![], vec![9], vec![8]];
    let weak_segment_old = vec![vec![], vec![2], vec![6]];
    let n_weak_segment = vec![0, 0, 0];
    let weak_pair = vec![[0, 0]];
    let mut ref_temp = vec![vec![0; 6]; 4];
    let mut n_ref_temp = vec![0; 4];
    let mut num_ref = 0;

    refine_weak_concav_lop_judge_one_based(
        &mut num_ref,
        1,
        2,
        2,
        0,
        &mrl_new,
        &triangle_neighbors,
        &vertices,
        &sjx_child,
        &mut weak_segment,
        &weak_segment_old,
        &n_weak_segment,
        &weak_pair,
        &mut ref_temp,
        &mut n_ref_temp,
    )
    .expect("empty odd weak-concavity pair is cleared after intersegment mapping");

    assert_eq!(ref_temp[2][1..=2], [20, 61]);
    assert_eq!(weak_segment[1][0], 1);
    assert_eq!(weak_segment[2][0], 1);
    assert_eq!(n_ref_temp[2], 2);
    assert_eq!(num_ref, 2);
}
