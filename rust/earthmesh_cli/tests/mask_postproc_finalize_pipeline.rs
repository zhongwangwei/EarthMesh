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
fn mask_postproc_finalize_report_exposes_vertex_mapping_for_ocean_boundary_writers() {
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

    let report = earthmesh_cli::finalize_mask_postproc_layout_with_reindex_report(
        &layout,
        &[0, 0, 1, -1],
        "tri",
    )
    .expect("finalize mask_postproc report");

    assert_eq!(report.vertex_reindex.sorted_vertices, vec![1, 2, 4, 5]);
    assert_eq!(report.vertex_reindex.vertex_mapping[1], 1);
    assert_eq!(report.vertex_reindex.vertex_mapping[2], 2);
    assert_eq!(report.vertex_reindex.vertex_mapping[3], 0);
    assert_eq!(report.vertex_reindex.vertex_mapping[4], 3);
    assert_eq!(report.vertex_reindex.vertex_mapping[5], 4);
    assert_eq!(report.final_data.vertex_coordinates_final[1], [0.0, 0.0]);
    assert_eq!(report.final_data.center_neighbors_final[2], vec![2, 3, 4]);
    assert_eq!(report.mesh.m_to_w[2], [2, 3, 4]);
}

#[test]
fn mask_postproc_finalize_accepts_hex_role_masks_at_cell_grain() {
    let layout = earthmesh_cli::MaskPostprocLayout {
        ustr_points: 24,
        ustr_bounds: 14,
        center_points: (0..24)
            .map(|idx| earthmesh_cli::LonLatPoint {
                lon: idx as f64,
                lat: idx as f64,
            })
            .collect(),
        vertex_points: (0..14)
            .map(|idx| earthmesh_cli::LonLatPoint {
                lon: 100.0 + idx as f64,
                lat: 100.0 + idx as f64,
            })
            .collect(),
        center_neighbors: (0..24)
            .map(|source_id| match source_id {
                2 => vec![2, 3, 4, 5, 6, 7],
                4 => vec![2, 3, 8, 9, 10, 11],
                6 => vec![4, 5, 8, 9, 12, 13],
                8 => vec![6, 7, 10, 11, 12, 13],
                _ => vec![1, 1, 1, 1, 1, 1],
            })
            .collect(),
        vertex_neighbors: vec![vec![1; 6]; 14],
        center_neighbor_counts: vec![6; 24],
        vertex_neighbor_counts: vec![0; 14],
    };
    let is_in_domain = vec![0, 0, 1, -1, 1, 0, 1, -1, 1];

    let report = earthmesh_cli::finalize_mask_postproc_layout_with_reindex_report(
        &layout,
        &is_in_domain,
        "hex",
    )
    .expect("finalize hex layout from cell-grain masks");

    assert_eq!(report.final_data.points_final, 5);
    assert_eq!(report.mesh.w_points.len(), 6);
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
