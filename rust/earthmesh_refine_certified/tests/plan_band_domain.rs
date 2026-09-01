use earthmesh_refine_certified::coarsen::{
    build_face_band_problem, build_plan_band_domains, n12_lifted_n6_fixture,
    solve_exact_face_bands, FaceBandLimits, FaceBandSolveOutcome, PlanBandTopologyKind,
    TopologyBoundary,
};

#[test]
fn lifted_plan_builds_two_plan_native_annuli() {
    let fixture = n12_lifted_n6_fixture().unwrap();
    let problem = build_face_band_problem(&fixture.source, &fixture.component, 2).unwrap();
    let FaceBandSolveOutcome::Closed(plan, _) = solve_exact_face_bands(
        &problem,
        FaceBandLimits {
            maximum_states: 16_384,
        },
    ) else {
        panic!("frozen Lifted W2 plan must close")
    };
    let bands = build_plan_band_domains(&fixture.source, &fixture.component, &plan).unwrap();
    assert_eq!(bands.len(), 2);
    assert!(bands.iter().all(|band| {
        band.euler == 0
            && band.source_boundary_cycles.len() == 2
            && band.topology_kind == PlanBandTopologyKind::Annulus
    }));
    assert_eq!(
        bands[0].upper_boundary.source_edges(),
        bands[1].lower_boundary.source_edges()
    );

    let TopologyBoundary::ContractedCoarseCycle {
        topology_edges,
        source_expansion,
        ..
    } = &bands[0].lower_boundary
    else {
        panic!("band 0 must expose coarse contraction")
    };
    assert_eq!(topology_edges.len(), 16);
    assert_eq!(source_expansion.coarse_edges.len(), 16);
    assert!(source_expansion
        .coarse_edges
        .iter()
        .all(|edge| (2..=3).contains(&edge.source_path.len())));
    assert_eq!(bands[0].lower_boundary.source_edges().len(), 32);
    assert!(matches!(
        bands[1].upper_boundary,
        TopologyBoundary::SourceCycle(_)
    ));
}
