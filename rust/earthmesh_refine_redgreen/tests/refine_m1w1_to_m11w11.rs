use earthmesh_refine_redgreen::refine_m1w1_to_m11w11_one_based;

#[test]
fn m1w1_to_m11w11_returns_first_adjacent_child_pair() {
    let sjx_child = vec![[0, 0], [4, 5], [6, 7]];
    let mut child_vertices = vec![[0, 0, 0]; 8];
    child_vertices[4] = [10, 11, 12];
    child_vertices[5] = [20, 21, 22];
    child_vertices[6] = [30, 31, 32];
    child_vertices[7] = [11, 12, 40];

    let pair = refine_m1w1_to_m11w11_one_based(1, 2, &sjx_child, &child_vertices)
        .expect("valid child lookup")
        .expect("adjacent children exist");

    assert_eq!(pair, (4, 7));
}

#[test]
fn m1w1_to_m11w11_skips_zero_child_slots_and_continues_search() {
    let sjx_child = vec![[0, 0], [0, 5], [6, 7]];
    let mut child_vertices = vec![[0, 0, 0]; 8];
    child_vertices[5] = [20, 21, 22];
    child_vertices[6] = [30, 31, 32];
    child_vertices[7] = [21, 22, 40];

    let pair = refine_m1w1_to_m11w11_one_based(1, 2, &sjx_child, &child_vertices)
        .expect("valid child lookup")
        .expect("adjacent nonzero children exist");

    assert_eq!(pair, (5, 7));
}

#[test]
fn m1w1_to_m11w11_returns_none_when_child_triangles_do_not_share_edge() {
    let sjx_child = vec![[0, 0], [4, 5], [6, 7]];
    let mut child_vertices = vec![[0, 0, 0]; 8];
    child_vertices[4] = [10, 11, 12];
    child_vertices[5] = [20, 21, 22];
    child_vertices[6] = [30, 31, 32];
    child_vertices[7] = [40, 41, 42];

    let pair = refine_m1w1_to_m11w11_one_based(1, 2, &sjx_child, &child_vertices)
        .expect("valid child lookup");

    assert_eq!(pair, None);
}

#[test]
fn m1w1_to_m11w11_rejects_out_of_range_parent_or_child_ids() {
    let sjx_child = vec![[0, 0], [99, 5], [6, 7]];
    let child_vertices = vec![[0, 0, 0]; 8];

    let err = refine_m1w1_to_m11w11_one_based(1, 2, &sjx_child, &child_vertices)
        .expect_err("child ids must address child vertex connectivity");

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
}
