use earthmesh_cli::apply_weak_concav_pair_special_fortran_indexed;

fn base_inputs() -> (
    Vec<Vec<usize>>,
    Vec<Vec<usize>>,
    Vec<i32>,
    Vec<i32>,
    Vec<[usize; 2]>,
    Vec<Vec<usize>>,
) {
    let mut triangle_neighbors = vec![vec![1, 1, 1]; 13];
    let mut ngrmw = vec![vec![0; 13]; 4];
    let mut mrl_new = vec![0; 13];
    let ref_sjx = vec![0; 13];
    for triangle in 1..=12 {
        mrl_new[triangle] = 1;
        ngrmw[1][triangle] = triangle * 10;
        ngrmw[2][triangle] = triangle * 10 + 1;
        ngrmw[3][triangle] = triangle * 10 + 2;
    }

    triangle_neighbors[2] = vec![4, 5, 6];
    mrl_new[4] = 4;
    triangle_neighbors[5] = vec![7, 8, 4];
    ngrmw[1][3] = 100;
    ngrmw[2][3] = 101;
    ngrmw[3][3] = 102;
    ngrmw[1][7] = 100;
    ngrmw[2][7] = 200;
    ngrmw[3][7] = 201;
    ngrmw[1][8] = 300;
    ngrmw[2][8] = 301;
    ngrmw[3][8] = 302;

    triangle_neighbors[3] = vec![9, 10, 11];
    mrl_new[9] = 4;
    triangle_neighbors[10] = vec![11, 12, 9];
    ngrmw[1][2] = 500;
    ngrmw[2][2] = 501;
    ngrmw[3][2] = 502;
    ngrmw[1][11] = 500;
    ngrmw[2][11] = 600;
    ngrmw[3][11] = 601;
    ngrmw[1][12] = 700;
    ngrmw[2][12] = 701;
    ngrmw[3][12] = 702;

    let weak_concav_pair = vec![[0, 0], [2, 0], [3, 0]];
    let weak_concav_segment = vec![vec![0; 2]; 5];

    (
        triangle_neighbors,
        ngrmw,
        mrl_new,
        ref_sjx,
        weak_concav_pair,
        weak_concav_segment,
    )
}

#[test]
fn weak_concav_pair_special_adapter_updates_fortran_row_state() {
    let (
        triangle_neighbors,
        ngrmw,
        mut mrl_new,
        mut ref_sjx,
        mut weak_concav_pair,
        mut weak_concav_segment,
    ) = base_inputs();

    let report = apply_weak_concav_pair_special_fortran_indexed(
        2,
        4,
        12,
        &triangle_neighbors,
        &ngrmw,
        &mut mrl_new,
        &mut ref_sjx,
        &mut weak_concav_pair,
        &mut weak_concav_segment,
    )
    .expect("apply weak concavity special-case through CLI adapter");

    assert_eq!(weak_concav_pair[1], [2, 5]);
    assert_eq!(weak_concav_pair[2], [3, 10]);
    assert_eq!(ref_sjx[5], 1);
    assert_eq!(ref_sjx[10], 1);
    assert_eq!(weak_concav_segment[3][0], 7);
    assert_eq!(weak_concav_segment[4][0], 11);
    assert_eq!(mrl_new[8], 4);
    assert_eq!(mrl_new[12], 4);

    assert_eq!(report.updated_pairs, vec![[2, 5], [3, 10]]);
    assert_eq!(report.marked_ref_sjx_triangles, vec![5, 10]);
    assert_eq!(report.deferred_renew_triangles, vec![8, 12]);
    assert_eq!(report.segment_first_slots, vec![(3, 7), (4, 11)]);
}
