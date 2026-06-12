use earthmesh_cli::apply_onedivide_four_connection_fortran_indexed;

#[test]
fn onedivide_four_connection_marks_only_selected_unrefined_triangles_and_vertices() {
    let num_vertex = 1;
    let sjx_points = 4;
    let ref_sjx = vec![0, 0, 1, 1, 1];
    let ngrmw = vec![
        vec![0, 0, 0, 0, 0],
        vec![0, 1, 2, 3, 1],
        vec![0, 1, 3, 4, 2],
        vec![0, 1, 4, 5, 3],
    ];
    let mut ref_lbx = vec![0, 0, 0, 0, 0, 0];
    let mut mrl_new = vec![0, 1, 1, 4, 0];

    let report = apply_onedivide_four_connection_fortran_indexed(
        num_vertex,
        sjx_points,
        &ref_sjx,
        &ngrmw,
        &mut ref_lbx,
        &mut mrl_new,
    )
    .expect("apply one-into-four connection");

    assert_eq!(report.marked_triangles, vec![2]);
    assert_eq!(report.marked_vertices, vec![2, 3, 4]);
    assert_eq!(mrl_new, vec![0, 1, 4, 4, 0]);
    assert_eq!(ref_lbx, vec![0, 0, 1, 1, 1, 0]);
}

#[test]
fn onedivide_four_connection_rejects_missing_fortran_indexed_slots() {
    let mut ref_lbx = vec![0, 0, 0];
    let mut mrl_new = vec![0, 1];
    let err = apply_onedivide_four_connection_fortran_indexed(
        1,
        2,
        &[0, 1],
        &[vec![0, 0], vec![0, 1], vec![0, 2], vec![0, 3]],
        &mut ref_lbx,
        &mut mrl_new,
    )
    .expect_err("sjx_points beyond state length should fail");

    assert!(err.to_string().contains("sjx_points"));
}
