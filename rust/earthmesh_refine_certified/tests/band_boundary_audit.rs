use earthmesh_refine_certified::coarsen::{
    audit_face_band_boundaries, build_face_band_problem, n12_lifted_n6_fixture,
    solve_exact_face_bands, BandClassificationFailure, FaceBandLimits, FaceBandSolveOutcome,
};

#[test]
fn lifted_bands_are_annuli_before_legacy_representation_fails() {
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
    let audits = audit_face_band_boundaries(&fixture.source, &fixture.component, &plan).unwrap();
    assert_eq!(audits.len(), 2);
    assert!(audits.iter().all(|audit| audit.is_topological_annulus()));
    assert!(matches!(
        audits[0].failure,
        Some(BandClassificationFailure::LowerTraceEdgeMismatch { .. })
    ));
    assert!(matches!(
        audits[1].failure,
        Some(BandClassificationFailure::DirectConnectorCapacityMissing { .. })
    ));
}
