use earthmesh_refine_certified::coarsen::{
    build_face_band_problem, build_stratified_transition_domain_v3, n12_lifted_n6_fixture,
    solve_exact_face_bands, FaceBandLimits, FaceBandSolveOutcome, TopologyBoundaryKind,
    TransitionCellDomain,
};

#[test]
fn lifted_v3_builds_two_annular_cells_without_legacy_sectors() {
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
    let v3 =
        build_stratified_transition_domain_v3(&fixture.source, &fixture.component, &plan).unwrap();
    assert_eq!(v3.cells.len(), 2);
    assert_eq!(v3.bands.len(), 2);
    assert_eq!(
        v3.link_contracts.len(),
        v3.topology_domain.boundary_contracts.len()
    );
    let cells = v3
        .cells
        .iter()
        .map(|cell| match cell {
            TransitionCellDomain::Annulus(cell) => cell,
            TransitionCellDomain::Disk(_) => panic!("Lifted W2 cells must be annular"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        cells[0].lower_boundary_kind,
        TopologyBoundaryKind::ContractedCoarseCycle
    );
    assert_eq!(cells[0].upper_cycle, cells[1].lower_cycle);
    assert_eq!(
        cells[1].upper_boundary_kind,
        TopologyBoundaryKind::SourceCycle
    );
    assert!(cells
        .iter()
        .all(|cell| !cell.fixed_outside_link_contracts.is_empty()));
}

#[test]
fn v3_source_has_no_legacy_shell_or_connector_call() {
    let source = include_str!("../src/coarsen/transition_cell_v3.rs");
    let compatibility_shell = ["coupled_annulus_from_", "topology_domain"].concat();
    let legacy_connector = ["monotone_", "connectors"].concat();
    assert!(!source.contains(&compatibility_shell));
    assert!(!source.contains(&legacy_connector));
}
