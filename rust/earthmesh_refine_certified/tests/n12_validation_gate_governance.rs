use earthmesh_refine_certified::coarsen::{
    current_n12_validation_gate_report, decide_validation_gate, n12_validation_gate_report_json,
    ResearchCecTopologyOutcomeKind as Topology, ResearchGeometryOutcome as Geometry,
    ValidationGateGovernanceDecision as Decision,
};

#[test]
fn current_evidence_keeps_topology_and_production_blocked() {
    let report = current_n12_validation_gate_report();
    assert_eq!(report.decision, Decision::TopologySolverBlocked);
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
        decide_validation_gate(incomplete, exact, None, None),
        Decision::TopologySolverBlocked
    );
    assert_eq!(
        decide_validation_gate(
            closed,
            closed,
            Some(Geometry::StrictCertified),
            Some(Geometry::StrictCertified)
        ),
        Decision::N6StressN12Existence
    );
    assert_eq!(
        decide_validation_gate(
            closed,
            closed,
            Some(Geometry::ContinuousSearchIncomplete),
            Some(Geometry::ContinuousSearchIncomplete)
        ),
        Decision::ContinuousGeometryBlocked
    );
    assert_eq!(
        decide_validation_gate(exact, closed, None, Some(Geometry::StrictCertified)),
        Decision::PentagonSpecificBlocked
    );
    assert_eq!(
        decide_validation_gate(exact, exact, None, None),
        Decision::KeepN6ExistenceGate
    );
}
