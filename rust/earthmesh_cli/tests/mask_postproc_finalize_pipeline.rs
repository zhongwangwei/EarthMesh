#[test]
fn mask_postproc_finalize_pipeline_compacts_active_tri_centers_and_reindexes_vertices() {
    let layout = earthmesh_cli::mask_postproc_types::MaskPostprocLayout {
        ustr_points: 4,
        ustr_bounds: 6,
        center_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 1.0, lat: 1.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 2.0, lat: 2.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 3.0, lat: 3.0 },
        ],
        vertex_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 10.0,
                lat: 10.0,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 11.0,
                lat: 11.0,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 12.0,
                lat: 12.0,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 13.0,
                lat: 13.0,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 14.0,
                lat: 14.0,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
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

    let mesh =
        earthmesh_cli::mask_postproc_layout::finalize_mask_postproc_layout_to_unstructured_mesh(
            &layout,
            &is_in_domain,
            "tri",
        )
        .expect("finalize mask_postproc tri layout");

    assert_eq!(mesh.m_points.len(), 3);
    assert_eq!(mesh.w_points.len(), 5);
    assert_eq!(
        mesh.m_points[2],
        earthmesh_cli::coordinate_types::LonLatPoint { lon: 2.0, lat: 2.0 }
    );
    assert_eq!(
        mesh.w_points[2],
        earthmesh_cli::coordinate_types::LonLatPoint {
            lon: 12.0,
            lat: 12.0
        }
    );
    assert_eq!(
        mesh.w_points[3],
        earthmesh_cli::coordinate_types::LonLatPoint {
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
    let layout = earthmesh_cli::mask_postproc_types::MaskPostprocLayout {
        ustr_points: 4,
        ustr_bounds: 6,
        center_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 1.0, lat: 1.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 2.0, lat: 2.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 3.0, lat: 3.0 },
        ],
        vertex_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 10.0,
                lat: 10.0,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 11.0,
                lat: 11.0,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 12.0,
                lat: 12.0,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 13.0,
                lat: 13.0,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 14.0,
                lat: 14.0,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 15.0,
                lat: 15.0,
            },
        ],
        center_neighbors: vec![vec![1, 1, 1], vec![1, 1, 1], vec![2, 4, 5], vec![5, 6, 4]],
        vertex_neighbors: vec![vec![1], vec![1], vec![2], vec![], vec![2, 3], vec![2, 3]],
        center_neighbor_counts: vec![0, 0, 3, 3],
        vertex_neighbor_counts: vec![0, 0, 1, 0, 2, 2],
    };

    let report =
        earthmesh_cli::mask_postproc_layout::finalize_mask_postproc_layout_with_reindex_report(
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
    let layout = earthmesh_cli::mask_postproc_types::MaskPostprocLayout {
        ustr_points: 24,
        ustr_bounds: 14,
        center_points: (0..24)
            .map(|idx| earthmesh_cli::coordinate_types::LonLatPoint {
                lon: idx as f64,
                lat: idx as f64,
            })
            .collect(),
        vertex_points: (0..14)
            // Cyclic, valid geographic vertices; the fixture tests mask grain,
            // not acceptance of impossible latitudes or degenerate polygons.
            .map(|idx| {
                let angle = idx as f64 * std::f64::consts::TAU / 14.0;
                earthmesh_cli::coordinate_types::LonLatPoint {
                    lon: 100.0 + angle.cos(),
                    lat: 10.0 + angle.sin(),
                }
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

    let report =
        earthmesh_cli::mask_postproc_layout::finalize_mask_postproc_layout_with_reindex_report(
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
    let layout = earthmesh_cli::mask_postproc_types::MaskPostprocLayout {
        ustr_points: 2,
        ustr_bounds: 2,
        center_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 1.0, lat: 1.0 },
        ],
        vertex_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 1.0, lat: 1.0 },
        ],
        center_neighbors: vec![vec![1, 1, 1], vec![1, 1, 1]],
        vertex_neighbors: vec![vec![1], vec![1]],
        center_neighbor_counts: vec![0, 0],
        vertex_neighbor_counts: vec![0, 0],
    };

    let err =
        earthmesh_cli::mask_postproc_layout::finalize_mask_postproc_layout_to_unstructured_mesh(
            &layout,
            &[0],
            "tri",
        )
        .expect_err("short domain mask rejected");
    assert!(err.to_string().contains("IsInDmArea_ustr"));
}

#[test]
fn hex_finalization_orients_shared_edges_without_changing_geometry() {
    use earthmesh_cli::{coordinate_types::LonLatPoint, mask_postproc_types::MaskPostprocLayout};
    use earthmesh_geometry::{try_spherical_polygon_area, Point};
    // Two adjacent hexagons; reverse only the left ring. Repeat across the
    // antimeridian and near the pole so planar longitude winding cannot pass.
    for (lon_offset, lat_offset) in [(0.0, 0.0), (180.0, 0.0), (180.0, 85.0)] {
        let point = |lon: f64, lat: f64| LonLatPoint {
            lon: (lon + lon_offset + 180.0).rem_euclid(360.0) - 180.0,
            lat: lat + lat_offset,
        };
        let zero = LonLatPoint { lon: 0.0, lat: 0.0 };
        let x = 3.0_f64.sqrt() / 2.0;
        let vertices = vec![
            zero,
            zero,
            point(0.0, 0.5),
            point(-x, 1.0),
            point(-2.0 * x, 0.5),
            point(-2.0 * x, -0.5),
            point(-x, -1.0),
            point(0.0, -0.5),
            point(x, -1.0),
            point(2.0 * x, -0.5),
            point(2.0 * x, 0.5),
            point(x, 1.0),
        ];
        let left = vec![2, 7, 6, 5, 4, 3, 1];
        let right = vec![2, 7, 8, 9, 10, 11, 1];
        let layout = MaskPostprocLayout {
            ustr_points: 4,
            ustr_bounds: vertices.len(),
            center_points: vec![zero, zero, point(-x, 0.0), point(x, 0.0)],
            vertex_points: vertices.clone(),
            center_neighbors: vec![vec![1; 7], vec![1; 7], left.clone(), right.clone()],
            vertex_neighbors: vec![vec![1; 3]; vertices.len()],
            center_neighbor_counts: vec![0, 0, 6, 6],
            vertex_neighbor_counts: vec![0; vertices.len()],
        };
        let report =
            earthmesh_cli::mask_postproc_layout::finalize_mask_postproc_layout_with_reindex_report(
                &layout,
                &[0, 0, 1, 1],
                "hex",
            )
            .unwrap();
        let mesh = &report.mesh;
        assert_eq!(mesh.w_points, layout.center_points);
        assert_eq!(mesh.m_points, vertices);
        assert_eq!(mesh.n_w_to_m, vec![0, 0, 6, 6]);
        for (cell, original) in [(2, left), (3, right)] {
            let ring = &mesh.w_to_m[cell][..6];
            let area = |ids: &[i32]| {
                try_spherical_polygon_area(
                    &ids.iter()
                        .map(|&id| {
                            let p = mesh.m_points[id as usize];
                            Point::new(p.lon, p.lat)
                        })
                        .collect::<Vec<_>>(),
                )
                .unwrap()
                .signed_minor_sr
            };
            let original = original.iter().map(|&v| v as i32).collect::<Vec<_>>();
            assert!(area(ring) > 0.0, "cell {cell} must wind outward");
            assert!((area(ring) - area(&original[..6]).abs()).abs() < 1e-12);
            let edges = |ids: &[i32]| {
                let mut edges = (0..ids.len())
                    .map(|i| {
                        let (a, b) = (ids[i], ids[(i + 1) % ids.len()]);
                        (a.min(b), a.max(b))
                    })
                    .collect::<Vec<_>>();
                edges.sort_unstable();
                edges
            };
            assert_eq!(edges(ring), edges(&original[..6]));
            assert_eq!(mesh.w_to_m[cell][6], 1, "padding must stay untouched");
            assert_eq!(ring[0], original[0], "keep the ring's starting vertex");
        }
        let direction = |ring: &[i32]| {
            (0..ring.len())
                .find_map(|i| match (ring[i], ring[(i + 1) % ring.len()]) {
                    (2, 7) => Some(1),
                    (7, 2) => Some(-1),
                    _ => None,
                })
                .unwrap()
        };
        assert_eq!(
            direction(&mesh.w_to_m[2][..6]),
            -direction(&mesh.w_to_m[3][..6])
        );
        assert_eq!(
            mesh.w_to_m[3],
            vec![2, 7, 8, 9, 10, 11, 1],
            "CCW ring unchanged"
        );
        let again =
            earthmesh_cli::mask_postproc_layout::mask_postproc_layout_from_unstructured_mesh(
                mesh, "hex",
            )
            .unwrap();
        let again = earthmesh_cli::mask_postproc_layout::finalize_mask_postproc_layout_to_unstructured_mesh(&again, &[0,0,1,1], "hex").unwrap();
        assert_eq!(
            again.w_to_m, mesh.w_to_m,
            "orientation normalization must be idempotent"
        );
        assert_eq!(again.m_to_w, mesh.m_to_w, "incidence must remain unchanged");
        for invalid in 0..4 {
            let mut broken = layout.clone();
            match invalid {
                0 => broken.vertex_points[2].lat = f64::NAN,
                1 => broken.center_neighbors[2].swap(2, 3), // self-intersection
                2 => broken.vertex_points[2] = broken.vertex_points[7],
                _ => broken.center_neighbors[2][3] = 1,
            }
            let err = earthmesh_cli::mask_postproc_layout::finalize_mask_postproc_layout_with_reindex_report(
                &broken, &[0, 0, 1, 1], "hex").expect_err("invalid ring must not be silently reordered or published");
            let expected = if invalid == 3 {
                "final hex cell 2 references placeholder 1"
            } else {
                "final hex cell 2 has invalid polygon geometry"
            };
            assert!(err.to_string().contains(expected), "{err}");
        }
    }
}
