fn sample_layout() -> earthmesh_cli::mask_postproc_types::MaskPostprocLayout {
    earthmesh_cli::mask_postproc_types::MaskPostprocLayout {
        ustr_points: 6,
        ustr_bounds: 10,
        center_points: vec![earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 }; 6],
        vertex_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 };
            10
        ],
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

    let info = earthmesh_cli::mask_postproc_patchtypes::build_earthmesh_info_one_based(
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
fn earthmesh_info_builder_matches_hex_refined_mesh_from_center_neighbors() {
    let layout = sample_layout();
    let is_in_domain = vec![0, 0, 1, 1, -1, 1];
    let seaorland = vec![0, 0, 1, -1, 0, 1];

    let info = earthmesh_cli::mask_postproc_patchtypes::build_earthmesh_info_one_based(
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
fn earthmesh_info_builder_accepts_hex_role_masks_at_cell_grain() {
    let mut layout = sample_layout();
    layout.ustr_points = 24;
    layout.ustr_bounds = 30;
    layout.center_points =
        vec![earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 }; 24];
    layout.vertex_points =
        vec![earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 }; 30];
    layout.center_neighbors = (0..24)
        .map(|source_id| match source_id {
            2 => vec![1, 2, 3],
            4 => vec![5, 6, 7],
            6 => vec![8, 9, 10],
            8 => vec![13, 14, 15],
            _ => vec![1, 1, 1],
        })
        .collect();
    layout.vertex_neighbors = vec![vec![1]; 30];
    layout.center_neighbor_counts = vec![3; 24];
    layout.vertex_neighbor_counts = vec![0; 30];
    let is_in_domain = vec![0, 0, 1, -1, 1, 0, 1, -1, 1];
    let seaorland = vec![0, 0, 1, 0, -1, 0, 1, 0, -1];

    let info = earthmesh_cli::mask_postproc_patchtypes::build_earthmesh_info_one_based(
        "hex",
        &[3, 6, 10],
        24,
        &layout,
        &is_in_domain,
        &seaorland,
    )
    .expect("build hex earthmesh info from cell-grain masks");

    assert_eq!(info.num_step_f, vec![3, 6, 10, 24]);
    assert_eq!(info.seaorland_ustr_f, vec![0, 0, 1, -1, 1, -1]);
    assert_eq!(info.refine_degree_f, vec![0, 0, 0, 1, 1, 2]);
}

#[test]
fn earthmesh_info_builder_rejects_short_role_mask() {
    let layout = sample_layout();
    let err = earthmesh_cli::mask_postproc_patchtypes::build_earthmesh_info_one_based(
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
