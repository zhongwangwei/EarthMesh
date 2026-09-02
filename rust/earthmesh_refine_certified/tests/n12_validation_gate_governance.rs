use earthmesh_refine_certified::coarsen::{
    current_n12_validation_gate_report, decide_validation_gate, n12_validation_gate_report_json,
    ResearchCecTopologyOutcomeKind as Topology, ResearchGeometryOutcome as Geometry,
    ValidationGateGovernanceDecision as Decision,
};

#[test]
fn current_evidence_keeps_geometry_and_production_blocked() {
    let report = current_n12_validation_gate_report();
    assert_eq!(report.lifted_topology, Topology::ResearchTopologyClosed);
    assert_eq!(
        report.lifted_geometry,
        Some(Geometry::ContinuousSearchIncomplete)
    );
    assert!(report.interior_is_capacity_control);
    assert!(report.cec_shards_resumed);
    assert!(!report.cec_resume_complete);
    assert_eq!(report.remaining_cec_checkpoint_shards, 1_233);
    assert_eq!(report.decision, Decision::ContinuousGeometryBlocked);
    assert!(!report.product_gate_changed);
    assert!(!report.research_staircase_unlocked);
    assert!(!report.nxp80_unlocked);
    assert_eq!(
        n12_validation_gate_report_json(&report),
        include_str!("fixtures/n12_validation_gate_governance.json").trim()
    );
}

#[test]
fn governance_matrix_keeps_distinct_blockers() {
    let closed = Topology::ResearchTopologyClosed;
    let exact = Topology::ResearchExactNoSolution;
    let incomplete = Topology::ResearchCycleSearchIncomplete;
    assert_eq!(
        decide_validation_gate(incomplete, exact, true, None, None),
        Decision::TopologySolverBlocked
    );
    assert_eq!(
        decide_validation_gate(closed, exact, true, Some(Geometry::StrictCertified), None),
        Decision::N6StressN12Existence
    );
    assert_eq!(
        decide_validation_gate(
            closed,
            exact,
            true,
            Some(Geometry::ContinuousSearchIncomplete),
            None
        ),
        Decision::ContinuousGeometryBlocked
    );
    assert_eq!(
        decide_validation_gate(exact, closed, false, None, Some(Geometry::StrictCertified)),
        Decision::PentagonSpecificBlocked
    );
    assert_eq!(
        decide_validation_gate(exact, exact, false, None, None),
        Decision::KeepN6ExistenceGate
    );
    assert_eq!(
        decide_validation_gate(closed, exact, true, None, None),
        Decision::ContinuousGeometryBlocked
    );
    assert_eq!(
        decide_validation_gate(
            closed,
            incomplete,
            true,
            Some(Geometry::StrictCertified),
            None
        ),
        Decision::TopologySolverBlocked
    );
    assert_eq!(
        decide_validation_gate(closed, exact, false, Some(Geometry::StrictCertified), None),
        Decision::KeepN6ExistenceGate
    );
}
