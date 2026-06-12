#[test]
fn ocean_tri_renewal_composes_boundary_metadata_without_changing_single_ocean_curve() {
    let layout = earthmesh_cli::MaskPostprocLayout {
        ustr_points: 6,
        ustr_bounds: 14,
        center_points: vec![earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 }; 6],
        vertex_points: vec![earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 }; 14],
        center_neighbors: vec![
            vec![1, 1, 1],
            vec![1, 1, 1],
            vec![10, 11, 1],
            vec![11, 12, 1],
            vec![12, 13, 1],
            vec![13, 10, 1],
        ],
        vertex_neighbors: vec![
            vec![1, 1, 1],
            vec![1, 1, 1],
            vec![1, 1, 1],
            vec![1, 1, 1],
            vec![1, 1, 1],
            vec![1, 1, 1],
            vec![1, 1, 1],
            vec![1, 1, 1],
            vec![1, 1, 1],
            vec![1, 1, 1],
            vec![2, 1, 1],
            vec![3, 1, 1],
            vec![4, 1, 1],
            vec![5, 1, 1],
        ],
        center_neighbor_counts: vec![0, 0, 2, 2, 2, 2],
        vertex_neighbor_counts: vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 3, 3, 3],
    };

    let report = earthmesh_cli::renew_mask_postproc_ocean_domain_fortran_indexed(
        &layout,
        &[0, -1, 1, 1, 1, 1],
        "tri",
    )
    .expect("renew ocean mask");

    assert_eq!(report.is_in_domain_ustr, vec![0, -1, 1, 1, 1, 1]);
    assert_eq!(report.renewed.points_next, 5);
    assert_eq!(report.renewed.bounds_next, 5);
    let boundary = report.boundary.expect("tri boundary metadata");
    assert_eq!(boundary.boundary_order, vec![1, 10, 11, 12, 13]);
    assert_eq!(boundary.curves.num_bdy_long, [5, 1, 1]);
    let isolated = report.isolated.expect("tri isolated-ocean metadata");
    assert!(isolated.removed_curve_ids.is_empty());
    assert_eq!(isolated.bdy_long_order[1..5], [10, 11, 12, 13]);
}

#[test]
fn ocean_hex_renewal_skips_tri_only_boundary_special_cases() {
    let layout = earthmesh_cli::MaskPostprocLayout {
        ustr_points: 3,
        ustr_bounds: 4,
        center_points: vec![earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 }; 3],
        vertex_points: vec![earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 }; 4],
        center_neighbors: vec![vec![1; 7], vec![1; 7], vec![2, 3, 1, 1, 1, 1, 1]],
        vertex_neighbors: vec![vec![1, 1, 1], vec![1, 1, 1], vec![2, 1, 1], vec![2, 1, 1]],
        center_neighbor_counts: vec![0, 0, 2],
        vertex_neighbor_counts: vec![0, 0, 1, 1],
    };

    let report = earthmesh_cli::renew_mask_postproc_ocean_domain_fortran_indexed(
        &layout,
        &[0, -1, 1],
        "hex",
    )
    .expect("hex ocean renewal skips tri-only logic");

    assert_eq!(report.is_in_domain_ustr, vec![0, -1, 1]);
    assert_eq!(report.renewed.points_next, 2);
    assert_eq!(report.renewed.bounds_next, 3);
    assert!(report.boundary.is_none());
    assert!(report.isolated.is_none());
}
