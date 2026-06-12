#[test]
fn mask_postproc_finalize_pipeline_compacts_active_tri_centers_and_reindexes_vertices() {
    let layout = earthmesh_cli::MaskPostprocLayout {
        ustr_points: 4,
        ustr_bounds: 6,
        center_points: vec![
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint { lon: 1.0, lat: 1.0 },
            earthmesh_cli::LonLatPoint { lon: 2.0, lat: 2.0 },
            earthmesh_cli::LonLatPoint { lon: 3.0, lat: 3.0 },
        ],
        vertex_points: vec![
            earthmesh_cli::LonLatPoint {
                lon: 10.0,
                lat: 10.0,
            },
            earthmesh_cli::LonLatPoint {
                lon: 11.0,
                lat: 11.0,
            },
            earthmesh_cli::LonLatPoint {
                lon: 12.0,
                lat: 12.0,
            },
            earthmesh_cli::LonLatPoint {
                lon: 13.0,
                lat: 13.0,
            },
            earthmesh_cli::LonLatPoint {
                lon: 14.0,
                lat: 14.0,
            },
            earthmesh_cli::LonLatPoint {
                lon: 15.0,
                lat: 15.0,
            },
        ],
        center_neighbors: vec![vec![1, 1, 1], vec![1, 1, 1], vec![2, 4, 5], vec![5, 6, 4]],
        vertex_neighbors: vec![vec![1], vec![1], vec![2], vec![], vec![2, 3], vec![2, 3]],
        center_neighbor_counts: vec![0, 0, 3, 3],
        vertex_neighbor_counts: vec![0, 0, 1, 0, 2, 2],
    };
    let is_in_domain = vec![0, 0, 1, -1];

    let mesh = earthmesh_cli::finalize_mask_postproc_layout_to_unstructured_mesh(
        &layout,
        &is_in_domain,
        "tri",
    )
    .expect("finalize mask_postproc tri layout");

    assert_eq!(mesh.m_points.len(), 3);
    assert_eq!(mesh.w_points.len(), 5);
    assert_eq!(
        mesh.m_points[2],
        earthmesh_cli::LonLatPoint { lon: 2.0, lat: 2.0 }
    );
    assert_eq!(
        mesh.w_points[2],
        earthmesh_cli::LonLatPoint {
            lon: 12.0,
            lat: 12.0
        }
    );
    assert_eq!(
        mesh.w_points[3],
        earthmesh_cli::LonLatPoint {
            lon: 14.0,
            lat: 14.0
        }
    );
    assert_eq!(mesh.m_to_w[2], [2, 3, 4]);
    assert_eq!(mesh.w_to_m[2][0], 2);
    assert_eq!(mesh.w_to_m[3][0], 2);
    assert_eq!(mesh.w_to_m[4][0], 2);
    assert_eq!(mesh.n_w_to_m, vec![0, 0, 1, 1, 1]);
}

#[test]
fn mask_postproc_finalize_pipeline_rejects_mask_length_mismatch() {
    let layout = earthmesh_cli::MaskPostprocLayout {
        ustr_points: 2,
        ustr_bounds: 2,
        center_points: vec![
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint { lon: 1.0, lat: 1.0 },
        ],
        vertex_points: vec![
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint { lon: 1.0, lat: 1.0 },
        ],
        center_neighbors: vec![vec![1, 1, 1], vec![1, 1, 1]],
        vertex_neighbors: vec![vec![1], vec![1]],
        center_neighbor_counts: vec![0, 0],
        vertex_neighbor_counts: vec![0, 0],
    };

    let err =
        earthmesh_cli::finalize_mask_postproc_layout_to_unstructured_mesh(&layout, &[0], "tri")
            .expect_err("short domain mask rejected");
    assert!(err.to_string().contains("IsInDmArea_ustr"));
}
