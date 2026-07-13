#[test]
fn final_mask_postproc_gridfile_keeps_tri_orientation_for_unstructured_mesh_save() {
    let final_data = earthmesh_mesh::MaskPostprocFinalData {
        points_final: 3,
        bounds_final: 4,
        center_coordinates_final: vec![[0.0, 0.0], [1.0, 1.0], [2.0, 2.0], [3.0, 3.0]],
        vertex_coordinates_final: vec![
            [10.0, 10.0],
            [11.0, 11.0],
            [12.0, 12.0],
            [13.0, 13.0],
            [14.0, 14.0],
        ],
        center_neighbors_final: vec![vec![1, 1, 1], vec![1, 1, 1], vec![2, 3, 4], vec![2, 4, 3]],
        vertex_neighbors_final: vec![vec![1, 1], vec![1, 1], vec![2], vec![2, 3], vec![3]],
        center_neighbor_counts_final: vec![0, 0, 3, 3],
        vertex_neighbor_counts_final: vec![0, 0, 1, 2, 1],
    };

    let mesh = earthmesh_cli::mask_postproc_layout::unstructured_mesh_from_mask_postproc_final(
        &final_data,
        "tri",
    )
    .expect("tri final mesh");

    assert_eq!(
        mesh.m_points[2],
        earthmesh_cli::coordinate_types::LonLatPoint { lon: 2.0, lat: 2.0 }
    );
    assert_eq!(
        mesh.w_points[4],
        earthmesh_cli::coordinate_types::LonLatPoint {
            lon: 14.0,
            lat: 14.0
        }
    );
    assert_eq!(mesh.m_to_w[2], [2, 3, 4]);
    assert_eq!(mesh.w_to_m[3], vec![2, 3]);
    assert_eq!(mesh.n_w_to_m, vec![0, 0, 1, 2, 1]);
}

#[test]
fn final_mask_postproc_gridfile_swaps_hex_orientation_like_canonical_save_call() {
    let final_data = earthmesh_mesh::MaskPostprocFinalData {
        points_final: 2,
        bounds_final: 3,
        center_coordinates_final: vec![[0.0, 0.0], [20.0, 20.0], [30.0, 30.0]],
        vertex_coordinates_final: vec![[0.0, 0.0], [40.0, 40.0], [50.0, 50.0], [60.0, 60.0]],
        center_neighbors_final: vec![vec![1, 1, 1, 1], vec![1, 1, 1, 1], vec![1, 2, 3, 1]],
        vertex_neighbors_final: vec![vec![1, 1, 1], vec![1, 1, 1], vec![2, 2, 2], vec![2, 2, 2]],
        center_neighbor_counts_final: vec![0, 0, 3],
        vertex_neighbor_counts_final: vec![0, 0, 3, 3],
    };

    let mesh = earthmesh_cli::mask_postproc_layout::unstructured_mesh_from_mask_postproc_final(
        &final_data,
        "hex",
    )
    .expect("hex final mesh");

    assert_eq!(
        mesh.m_points[2],
        earthmesh_cli::coordinate_types::LonLatPoint {
            lon: 50.0,
            lat: 50.0
        }
    );
    assert_eq!(
        mesh.w_points[2],
        earthmesh_cli::coordinate_types::LonLatPoint {
            lon: 30.0,
            lat: 30.0
        }
    );
    assert_eq!(mesh.m_to_w[2], [2, 2, 2]);
    assert_eq!(mesh.w_to_m[2], vec![1, 2, 3, 1]);
    assert_eq!(mesh.n_w_to_m, vec![0, 0, 3]);
}

#[test]
fn final_mask_postproc_gridfile_rejects_unsupported_mode_and_non_triangle_m_rows() {
    let final_data = earthmesh_mesh::MaskPostprocFinalData {
        points_final: 2,
        bounds_final: 2,
        center_coordinates_final: vec![[0.0, 0.0], [1.0, 1.0], [2.0, 2.0]],
        vertex_coordinates_final: vec![[0.0, 0.0], [3.0, 3.0], [4.0, 4.0]],
        center_neighbors_final: vec![vec![1, 1], vec![1, 1], vec![1, 2]],
        vertex_neighbors_final: vec![vec![1, 1], vec![1, 1], vec![2]],
        center_neighbor_counts_final: vec![0, 0, 2],
        vertex_neighbor_counts_final: vec![0, 0, 1],
    };

    let err = earthmesh_cli::mask_postproc_layout::unstructured_mesh_from_mask_postproc_final(
        &final_data,
        "quad",
    )
    .expect_err("unsupported mode rejected");
    assert!(err.to_string().contains("tri or hex"));

    let err = earthmesh_cli::mask_postproc_layout::unstructured_mesh_from_mask_postproc_final(
        &final_data,
        "tri",
    )
    .expect_err("tri m_to_w must have three vertices");
    assert!(err.to_string().contains("three"));
}
