use earthmesh_refine_redgreen::refine_sharp_concav_lop_judge_one_based;

fn child_vertices() -> Vec<[usize; 3]> {
    let mut vertices = vec![[0, 0, 0]; 80];
    vertices[20] = [1, 2, 3];
    vertices[21] = [90, 91, 92];
    vertices[30] = [10, 11, 13];
    vertices[31] = [30, 31, 32];
    vertices[40] = [60, 61, 62];
    vertices[50] = [40, 41, 42];
    vertices[51] = [2, 3, 4];
    vertices[60] = [10, 11, 12];
    vertices[61] = [2, 3, 4];
    vertices
}

#[test]
fn sharp_concav_lop_judge_builds_single_transition_pair() {
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

    refine_sharp_concav_lop_judge_one_based(
        &mut num_ref,
        1,
        &mrl_new,
        &triangle_neighbors,
        &child_vertices(),
        &sjx_child,
        &bdy_refine_segment,
        &bdy_refine_segment_old,
        &n_bdy_refine_segment,
        &mut ref_temp,
        &mut n_ref_temp,
    )
    .expect("strong concavity LOP segment generation");

    assert_eq!(ref_temp[1][1..=4], [20, 61, 60, 30]);
    assert_eq!(n_ref_temp[1], 4);
    assert_eq!(num_ref, 4);
}

#[test]
fn sharp_concav_lop_judge_mirrors_other_end_for_longer_transition_degree() {
    let mut mrl_new = vec![1; 9];
    mrl_new[5] = 4;
    mrl_new[6] = 4;
    let mut triangle_neighbors = vec![vec![1, 1, 1]; 9];
    triangle_neighbors[7] = vec![1, 5, 2];
    triangle_neighbors[8] = vec![1, 6, 2];
    let mut sjx_child = vec![[0, 0]; 9];
    sjx_child[2] = [20, 21];
    sjx_child[3] = [30, 31];
    sjx_child[4] = [40, 41];
    sjx_child[5] = [50, 51];
    sjx_child[6] = [60, 61];
    let bdy_refine_segment = vec![vec![], vec![0, 7, 8]];
    let bdy_refine_segment_old = vec![vec![], vec![0, 2, 3, 4]];
    let n_bdy_refine_segment = vec![0, 3];
    let mut ref_temp = vec![vec![0; 12]; 2];
    let mut n_ref_temp = vec![0, 2];
    let mut num_ref = 0;
    let mut vertices = child_vertices();
    vertices[40] = [80, 81, 62];
    vertices[50] = [10, 11, 42];
    vertices[60] = [80, 81, 82];
    vertices[61] = [30, 31, 33];

    refine_sharp_concav_lop_judge_one_based(
        &mut num_ref,
        1,
        &mrl_new,
        &triangle_neighbors,
        &vertices,
        &sjx_child,
        &bdy_refine_segment,
        &bdy_refine_segment_old,
        &n_bdy_refine_segment,
        &mut ref_temp,
        &mut n_ref_temp,
    )
    .expect("strong concavity LOP longer segment generation");

    assert_eq!(ref_temp[1][1..=4], [20, 51, 60, 40]);
    assert_eq!(n_ref_temp[1], 4);
    assert_eq!(num_ref, 4);
}

#[test]
fn sharp_concav_lop_judge_terminates_placeholder_segment() {
    let mrl_new = vec![1; 8];
    let triangle_neighbors = vec![vec![1, 1, 1]; 8];
    let sjx_child = vec![[0, 0]; 8];
    let bdy_refine_segment = vec![vec![], vec![0, 1, 1]];
    let bdy_refine_segment_old = vec![vec![], vec![0, 2, 3, 4]];
    let n_bdy_refine_segment = vec![0, 3];
    let mut ref_temp = vec![vec![0; 12]; 2];
    let mut n_ref_temp = vec![0, 2];
    let mut num_ref = 0;

    refine_sharp_concav_lop_judge_one_based(
        &mut num_ref,
        1,
        &mrl_new,
        &triangle_neighbors,
        &child_vertices(),
        &sjx_child,
        &bdy_refine_segment,
        &bdy_refine_segment_old,
        &n_bdy_refine_segment,
        &mut ref_temp,
        &mut n_ref_temp,
    )
    .expect("placeholder segment should terminate LOP generation");

    assert_eq!(n_ref_temp[1], 0);
    assert_eq!(num_ref, 0);
    assert!(ref_temp[1].iter().all(|&value| value == 0));
}

#[test]
fn sharp_concav_lop_judge_skips_missing_child_adjacency() {
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
    let vertices = (0..80)
        .map(|idx| [idx * 3 + 1000, idx * 3 + 1001, idx * 3 + 1002])
        .collect::<Vec<_>>();

    refine_sharp_concav_lop_judge_one_based(
        &mut num_ref,
        1,
        &mrl_new,
        &triangle_neighbors,
        &vertices,
        &sjx_child,
        &bdy_refine_segment,
        &bdy_refine_segment_old,
        &n_bdy_refine_segment,
        &mut ref_temp,
        &mut n_ref_temp,
    )
    .expect("missing child adjacency candidate should be skipped");

    assert_eq!(num_ref, 0);
    assert_eq!(n_ref_temp[1], 0);
    assert!(ref_temp[1].iter().all(|&value| value == 0));
}
