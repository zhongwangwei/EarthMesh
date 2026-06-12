use earthmesh_cli::lookup_m1w1_to_m11w11_fortran_indexed;

#[test]
fn m1w1_lookup_adapter_converts_fortran_rows_and_reports_child_pair() {
    let sjx_child = vec![[0, 0], [4, 5], [6, 7]];
    let ngrmw_new = vec![
        vec![0, 0, 0, 0, 10, 20, 30, 11],
        vec![0, 0, 0, 0, 11, 21, 31, 12],
        vec![0, 0, 0, 0, 12, 22, 32, 40],
        vec![0, 0, 0, 0, 0, 0, 0, 0],
    ];

    let report = lookup_m1w1_to_m11w11_fortran_indexed(1, 2, &sjx_child, &ngrmw_new, 7)
        .expect("lookup child adjacency through CLI adapter");

    assert_eq!(report.parent_pair, (1, 2));
    assert_eq!(report.child_pair, Some((4, 7)));
}

#[test]
fn m1w1_lookup_adapter_preserves_missing_adjacency() {
    let sjx_child = vec![[0, 0], [4, 5], [6, 7]];
    let ngrmw_new = vec![
        vec![0, 0, 0, 0, 10, 20, 30, 40],
        vec![0, 0, 0, 0, 11, 21, 31, 41],
        vec![0, 0, 0, 0, 12, 22, 32, 42],
        vec![0, 0, 0, 0, 0, 0, 0, 0],
    ];

    let report = lookup_m1w1_to_m11w11_fortran_indexed(1, 2, &sjx_child, &ngrmw_new, 7)
        .expect("lookup child adjacency through CLI adapter");

    assert_eq!(report.parent_pair, (1, 2));
    assert_eq!(report.child_pair, None);
}
