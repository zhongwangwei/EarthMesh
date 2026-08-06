use earthmesh_refine_redgreen::refine_onedivide_four_connection_one_based;

#[test]
fn onedivide_four_connection_marks_refined_triangle_and_parent_cells() {
    let cells_on_triangle = vec![[0, 0, 0], [0, 0, 0], [4, 5, 6], [6, 7, 8], [8, 9, 10]];
    let ref_sjx = vec![0, 0, 1, 0, 1];
    let mut ref_lbx = vec![0; 11];
    let mut mrl_new = vec![0, 1, 1, 1, 4];

    refine_onedivide_four_connection_one_based(
        1,
        4,
        &cells_on_triangle,
        &ref_sjx,
        &mut ref_lbx,
        &mut mrl_new,
    )
    .expect("apply one-into-four connection state update");

    assert_eq!(mrl_new[2], 4, "requested unrefined triangle is promoted");
    assert_eq!(mrl_new[3], 1, "unrequested triangle is unchanged");
    assert_eq!(mrl_new[4], 4, "already refined triangle is unchanged");
    assert_eq!(&ref_lbx[4..=6], &[1, 1, 1]);
    assert_eq!(
        ref_lbx[8], 0,
        "already-refined requested triangle is skipped like Canonical"
    );
}

#[test]
fn onedivide_four_connection_rejects_out_of_range_parent_cell() {
    let cells_on_triangle = vec![[0, 0, 0], [0, 0, 0], [4, 5, 99]];
    let ref_sjx = vec![0, 0, 1];
    let mut ref_lbx = vec![0; 10];
    let mut mrl_new = vec![0, 1, 1];

    let err = refine_onedivide_four_connection_one_based(
        1,
        2,
        &cells_on_triangle,
        &ref_sjx,
        &mut ref_lbx,
        &mut mrl_new,
    )
    .expect_err("invalid parent cell should be rejected");

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
}
