use earthmesh_refine_redgreen::refine_weak_concav_segment_make_one_based;

fn cells_fixture(max_triangle: usize) -> Vec<[usize; 3]> {
    let mut cells = vec![[0, 0, 0]; max_triangle + 1];
    for triangle in 1..=max_triangle {
        cells[triangle] = [triangle * 10, triangle * 10 + 1, triangle * 10 + 2];
    }
    cells
}

#[test]
fn weak_concav_segment_make_records_one_plus_one_as_pair_entries() {
    let mut cells = cells_fixture(3);
    cells[2] = [10, 11, 12];
    cells[3] = [10, 11, 13];

    let result = refine_weak_concav_segment_make_one_based(2, 2, &cells, &[vec![2], vec![3]])
        .expect("1+1 weak concavity pair");

    assert_eq!(result.num_ref_weak_concav, 2);
    assert_eq!(result.num_weak_concav_segment, 0);
    assert_eq!(result.num_weak_concav_pair, 2);
    assert_eq!(result.weak_concav_pair, vec![[2, 0], [3, 0]]);
    assert_eq!(result.weak_concav_segment, vec![vec![2], vec![3]]);
    assert_eq!(result.n_weak_concav_segment, vec![1, 1]);
    assert_eq!(
        result.bdy_refine_segment,
        vec![Vec::<usize>::new(), Vec::<usize>::new()]
    );
    assert_eq!(result.n_bdy_refine_segment, vec![0, 0]);
}

#[test]
fn weak_concav_segment_make_moves_equal_non_singleton_segments() {
    let mut cells = cells_fixture(5);
    cells[4] = [100, 101, 102];
    cells[3] = [100, 101, 103];
    cells[5] = [200, 201, 202];
    cells[2] = [300, 301, 302];

    let result = refine_weak_concav_segment_make_one_based(2, 2, &cells, &[vec![2, 4], vec![3, 5]])
        .expect("n+n weak concavity segments");

    assert_eq!(result.num_ref_weak_concav, 2);
    assert_eq!(result.num_weak_concav_segment, 2);
    assert_eq!(result.num_weak_concav_pair, 0);
    assert_eq!(result.weak_concav_segment, vec![vec![2, 4], vec![3, 5]]);
    assert_eq!(result.n_weak_concav_segment, vec![2, 2]);
    assert!(result.weak_concav_pair.is_empty());
    assert_eq!(
        result.bdy_refine_segment,
        vec![Vec::<usize>::new(), Vec::<usize>::new()]
    );
}

#[test]
fn weak_concav_segment_make_extracts_pair_from_one_plus_n_and_keeps_remainder() {
    let mut cells = cells_fixture(5);
    cells[2] = [50, 51, 52];
    cells[3] = [50, 51, 53];
    cells[5] = [900, 901, 902];

    let result = refine_weak_concav_segment_make_one_based(3, 2, &cells, &[vec![2], vec![3, 4, 5]])
        .expect("1+n weak concavity split");

    assert_eq!(result.num_ref_weak_concav, 2);
    assert_eq!(result.num_weak_concav_segment, 0);
    assert_eq!(result.num_weak_concav_pair, 2);
    assert_eq!(result.weak_concav_pair, vec![[2, 0], [3, 0]]);
    assert_eq!(result.weak_concav_segment, vec![vec![2], vec![3]]);
    assert_eq!(
        result.bdy_refine_segment,
        vec![Vec::<usize>::new(), vec![4, 5]]
    );
    assert_eq!(result.n_bdy_refine_segment, vec![0, 2]);
}
