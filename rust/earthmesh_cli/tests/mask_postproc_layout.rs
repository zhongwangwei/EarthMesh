#[test]
fn mask_postproc_layout_uses_tri_centers_and_hex_swaps_centers_vertices_like_fortran() {
    let mesh = earthmesh_cli::UnstructuredMesh {
        m_points: vec![
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint {
                lon: 10.0,
                lat: 1.0,
            },
        ],
        w_points: vec![
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint {
                lon: 20.0,
                lat: 2.0,
            },
            earthmesh_cli::LonLatPoint {
                lon: 30.0,
                lat: 3.0,
            },
        ],
        m_to_w: vec![[1, 1, 1], [1, 2, 3]],
        w_to_m: vec![vec![1, 1, 1], vec![1, 2], vec![2, 1]],
        n_w_to_m: vec![1, 2, 2],
    };

    let tri = earthmesh_cli::mask_postproc_layout_from_unstructured_mesh(&mesh, "tri")
        .expect("tri layout");
    assert_eq!(tri.ustr_points, 2);
    assert_eq!(tri.ustr_bounds, 3);
    assert_eq!(tri.center_points, mesh.m_points);
    assert_eq!(tri.vertex_points, mesh.w_points);
    assert_eq!(tri.center_neighbors, vec![vec![1, 1, 1], vec![1, 2, 3]]);
    assert_eq!(
        tri.vertex_neighbors,
        vec![vec![1, 1, 1], vec![1, 2], vec![2, 1]]
    );
    assert_eq!(tri.center_neighbor_counts, vec![3, 3]);
    assert_eq!(tri.vertex_neighbor_counts, vec![1, 2, 2]);

    let hex = earthmesh_cli::mask_postproc_layout_from_unstructured_mesh(&mesh, "hex")
        .expect("hex layout");
    assert_eq!(hex.ustr_points, 3);
    assert_eq!(hex.ustr_bounds, 2);
    assert_eq!(hex.center_points, mesh.w_points);
    assert_eq!(hex.vertex_points, mesh.m_points);
    assert_eq!(
        hex.center_neighbors,
        vec![vec![1, 1, 1], vec![1, 2], vec![2, 1]]
    );
    assert_eq!(hex.vertex_neighbors, vec![vec![1, 1, 1], vec![1, 2, 3]]);
    assert_eq!(hex.center_neighbor_counts, vec![1, 2, 2]);
    assert_eq!(hex.vertex_neighbor_counts, vec![3, 3]);
}

#[test]
fn mask_postproc_layout_rejects_unsupported_mode_grid_and_negative_connectivity() {
    let mesh = earthmesh_cli::UnstructuredMesh {
        m_points: vec![earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 }],
        w_points: vec![earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 }],
        m_to_w: vec![[1, -1, 1]],
        w_to_m: vec![vec![1]],
        n_w_to_m: vec![1],
    };

    let bad_mode = earthmesh_cli::mask_postproc_layout_from_unstructured_mesh(&mesh, "quad")
        .expect_err("unsupported mode rejected");
    assert!(bad_mode.to_string().contains("tri or hex"));

    let bad_connectivity = earthmesh_cli::mask_postproc_layout_from_unstructured_mesh(&mesh, "tri")
        .expect_err("negative connectivity rejected");
    assert!(bad_connectivity.to_string().contains("negative"));
}
