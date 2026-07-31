#[test]
fn ocean_tri_renewal_composes_boundary_metadata_without_changing_single_ocean_curve() {
    let layout = earthmesh_cli::mask_postproc_types::MaskPostprocLayout {
        ustr_points: 6,
        ustr_bounds: 15,
        center_points: vec![earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 }; 6],
        vertex_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 };
            15
        ],
        center_neighbors: vec![
            vec![1, 1, 1],
            vec![1, 1, 1],
            vec![10, 11, 14],
            vec![11, 12, 14],
            vec![12, 13, 14],
            vec![13, 10, 14],
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
            vec![2, 5, 1],
            vec![2, 3, 1],
            vec![3, 4, 1],
            vec![4, 5, 1],
            vec![2, 3, 4, 5],
        ],
        center_neighbor_counts: vec![0, 0, 3, 3, 3, 3],
        vertex_neighbor_counts: vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 3, 3, 3, 4],
    };

    let report = earthmesh_cli::mask_postproc_ocean::renew_mask_postproc_ocean_domain_one_based(
        &layout,
        &[0, -1, 1, 1, 1, 1],
        "tri",
    )
    .expect("renew ocean mask");

    assert_eq!(report.is_in_domain_ustr, vec![0, -1, 1, 1, 1, 1]);
    assert_eq!(report.renewed.points_next, 5);
    assert_eq!(report.renewed.bounds_next, 6);
    let boundary = report.boundary.expect("tri boundary metadata");
    assert_eq!(boundary.boundary_order, vec![1, 10, 11, 12, 13]);
    assert_eq!(boundary.curves.num_bdy_long, [5, 1, 1]);
    let isolated = report.isolated.expect("tri isolated-ocean metadata");
    assert!(isolated.removed_curve_ids.is_empty());
    assert_eq!(isolated.bdy_long_order[1..5], [10, 11, 12, 13]);

    let clipped_baseline =
        earthmesh_cli::mask_postproc_ocean::renew_mask_postproc_ocean_domain_one_based(
            &layout,
            &[0, -1, 1, 1, 1, -1],
            "tri",
        )
        .expect("renew clipped ocean baseline");
    let clipped = earthmesh_cli::mask_postproc_ocean::
        renew_mask_postproc_ocean_domain_one_based_with_hard_demand(
            &layout,
            &[0, -1, 1, 1, 1, -1],
            "tri",
            &[false, false, false, true],
        )
        .expect("projected demand outside ocean product support is not immutable");
    assert_eq!(
        clipped.is_in_domain_ustr,
        clipped_baseline.is_in_domain_ustr
    );
    assert!(clipped.excluded_unsupported_hard_demand_cells.is_empty());
}

#[test]
fn ocean_hex_renewal_skips_tri_only_boundary_special_cases() {
    let layout = earthmesh_cli::mask_postproc_types::MaskPostprocLayout {
        ustr_points: 5,
        ustr_bounds: 12,
        center_points: vec![earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 }; 5],
        vertex_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 };
            12
        ],
        center_neighbors: vec![
            vec![1; 4],
            vec![1; 4],
            vec![2, 3, 4, 5],
            vec![3, 2, 6, 7],
            vec![8, 9, 10, 11],
        ],
        vertex_neighbors: vec![
            vec![1, 1, 1],
            vec![1, 1, 1],
            vec![2, 3, 1],
            vec![2, 3, 1],
            vec![2, 1, 1],
            vec![2, 1, 1],
            vec![3, 1, 1],
            vec![3, 1, 1],
            vec![4, 1, 1],
            vec![4, 1, 1],
            vec![4, 1, 1],
            vec![4, 1, 1],
        ],
        center_neighbor_counts: vec![0, 0, 4, 4, 4],
        vertex_neighbor_counts: vec![0, 0, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3],
    };

    let report = earthmesh_cli::mask_postproc_ocean::
        renew_mask_postproc_ocean_domain_one_based_with_hard_demand(
            &layout,
            &[0, -1, 1, 1, 1],
            "hex",
            &[false, false, true],
        )
        .expect("hex ocean renewal excludes orphan demand before generic cleanup");

    assert_eq!(report.is_in_domain_ustr, vec![0, -1, 1, 1, -1]);
    assert_eq!(report.renewed.points_next, 3);
    assert_eq!(report.renewed.bounds_next, 7);
    assert!(report.boundary.is_none());
    assert!(report.isolated.is_none());
    assert_eq!(report.excluded_unsupported_hard_demand_cells, vec![4]);
}

#[test]
fn ocean_renewal_does_not_promote_orphan_hard_demand() {
    let point = earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 };
    let layout = earthmesh_cli::mask_postproc_types::MaskPostprocLayout {
        ustr_points: 5,
        ustr_bounds: 5,
        center_points: vec![point; 5],
        vertex_points: vec![point; 5],
        center_neighbors: vec![
            vec![1, 1, 1],
            vec![1, 1, 1],
            vec![2, 3, 4],
            vec![2, 3, 4],
            vec![2, 3, 4],
        ],
        vertex_neighbors: vec![
            vec![1, 1, 1],
            vec![1, 1, 1],
            vec![2, 3, 4],
            vec![2, 3, 4],
            vec![2, 3, 4],
        ],
        center_neighbor_counts: vec![0, 0, 3, 3, 3],
        vertex_neighbor_counts: vec![0, 0, 3, 3, 3],
    };

    let error = earthmesh_cli::mask_postproc_ocean::
        renew_mask_postproc_ocean_domain_one_based_with_hard_demand(
            &layout,
            &[0, -1, 1, -1, -1],
            "tri",
            &[true, false, false],
        )
        .expect_err("an orphan-only ocean product must remain empty");

    let failure = earthmesh_cli::masked_topology_cleanup::domain_topology_failure(&error)
        .expect("typed domain-topology failure");
    assert_eq!(
        failure.kind(),
        earthmesh_cli::masked_topology_cleanup::DomainTopologyFailureKind::NoRetainedCells
    );
    assert_eq!(failure.center_id(), None);
}
