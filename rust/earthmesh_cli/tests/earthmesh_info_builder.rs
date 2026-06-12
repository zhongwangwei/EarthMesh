fn sample_layout() -> earthmesh_cli::MaskPostprocLayout {
    earthmesh_cli::MaskPostprocLayout {
        ustr_points: 6,
        ustr_bounds: 10,
        center_points: vec![earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 }; 6],
        vertex_points: vec![earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 }; 10],
        center_neighbors: vec![
            vec![1, 1, 1],
            vec![1, 1, 1],
            vec![2, 3, 4],
            vec![5, 6, 7],
            vec![7, 8, 9],
            vec![3, 4, 5],
        ],
        vertex_neighbors: vec![vec![1]; 10],
        center_neighbor_counts: vec![0, 0, 3, 3, 3, 3],
        vertex_neighbor_counts: vec![0; 10],
    }
}

#[test]
fn earthmesh_info_builder_matches_tri_refine_loop() {
    let layout = sample_layout();
    let is_in_domain = vec![0, 0, 1, 1, -1, 1];
    let seaorland = vec![0, 0, 1, -1, 0, 1];

    let info = earthmesh_cli::build_earthmesh_info_fortran_indexed(
        "tri",
        &[3, 5],
        6,
        &layout,
        &is_in_domain,
        &seaorland,
    )
    .expect("build tri earthmesh info");

    assert_eq!(info.num_step_f, vec![3, 3, 6]);
    assert_eq!(info.seaorland_ustr_f, vec![0, 0, 1, -1, 1]);
    assert_eq!(info.refine_degree_f, vec![0, 0, 0, 0, 1]);
}

#[test]
fn earthmesh_info_builder_matches_hex_refine_loop_from_center_neighbors() {
    let layout = sample_layout();
    let is_in_domain = vec![0, 0, 1, 1, -1, 1];
    let seaorland = vec![0, 0, 1, -1, 0, 1];

    let info = earthmesh_cli::build_earthmesh_info_fortran_indexed(
        "hex",
        &[3, 5, 8],
        6,
        &layout,
        &is_in_domain,
        &seaorland,
    )
    .expect("build hex earthmesh info");

    assert_eq!(info.num_step_f, vec![3, 5, 8, 6]);
    assert_eq!(info.seaorland_ustr_f, vec![0, 0, 1, -1, 1]);
    assert_eq!(info.refine_degree_f, vec![0, 0, 0, 1, 0]);
}

#[test]
fn earthmesh_info_builder_rejects_short_role_mask() {
    let layout = sample_layout();
    let err = earthmesh_cli::build_earthmesh_info_fortran_indexed(
        "tri",
        &[3],
        6,
        &layout,
        &[0, 0, 1, 1, 1, 1],
        &[0, 0, 1],
    )
    .expect_err("short seaorland rejected");

    assert!(err.to_string().contains("seaorland_ustr"));
}
