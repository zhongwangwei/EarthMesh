use earthmesh_refine_certified::coarsen::{
    audit_legacy_downstream_preflight, n12_lifted_downstream_reject_audit_json,
    n12_lifted_n6_fixture, DownstreamContractStage, DownstreamPreflightOutcome,
    ResearchCecTopologyLimits,
};
use std::fs;

#[test]
fn lifted_legacy_preflight_is_plan_independent_geometry_guard_blocked() {
    let fixture = n12_lifted_n6_fixture().unwrap();
    let DownstreamPreflightOutcome::ContractBlocked { stage, evidence } =
        audit_legacy_downstream_preflight(&fixture.source, &fixture.component)
    else {
        panic!("Lifted legacy adapter must reproduce the frozen block")
    };
    assert_eq!(stage, DownstreamContractStage::GeometryGuardOnly);
    assert!(evidence.plan_independent);
    assert!(evidence.geometry_guard_deferred);
    assert!(evidence.reason.unwrap().contains("inner_guard"));
}

#[test]
fn frozen_lifted_reject_distribution_matches_snapshot() {
    assert_eq!(
        n12_lifted_downstream_reject_audit_json(ResearchCecTopologyLimits::default()).unwrap(),
        include_str!("fixtures/n12_downstream_reject_audit.json").trim()
    );
}

#[test]
#[ignore = "Post-PR97 fixed-budget downstream rejection audit"]
fn print_lifted_downstream_reject_audit() {
    let json =
        n12_lifted_downstream_reject_audit_json(ResearchCecTopologyLimits::default()).unwrap();
    if let Ok(path) = std::env::var("EARTHMESH_N12_REJECT_AUDIT_JSON") {
        fs::write(path, &json).unwrap();
    }
    println!("{json}");
}
