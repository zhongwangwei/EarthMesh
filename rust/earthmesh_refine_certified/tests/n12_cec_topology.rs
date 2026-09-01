use earthmesh_refine_certified::coarsen::{
    n12_cec_topology_probe_json, n12_interior_control_fixture, n12_lifted_n6_fixture,
    run_n12_cec_topology_probe, ResearchCecTopologyLimits, ResearchCecTopologyOutcomeKind,
};
use std::fs;

#[test]
fn cec_topology_probe_is_research_only_and_budget_safe() {
    let limits = ResearchCecTopologyLimits {
        cycle_unique_states: 0,
        downstream_topology_states: 0,
    };
    for fixture in [
        n12_lifted_n6_fixture().unwrap(),
        n12_interior_control_fixture().unwrap(),
    ] {
        let report = run_n12_cec_topology_probe(&fixture, limits).unwrap();
        assert_ne!(
            report.outcome,
            ResearchCecTopologyOutcomeKind::ResearchTopologyClosed
        );
        assert!(!report.geometry_attempted);
        assert!(!report.product_grid_written);
        assert!(!report.ready_marker_written);
        assert!(!report.product_gate_changed);
    }
}

#[test]
fn frozen_n12_cec_topology_probe_matches_snapshot() {
    assert_eq!(
        n12_cec_topology_probe_json(ResearchCecTopologyLimits::default()).unwrap(),
        include_str!("fixtures/n12_cec_topology_probe.json").trim()
    );
}

#[test]
#[ignore = "Alpha6 PR95 bounded N12 CEC topology probe"]
fn print_n12_cec_topology_probe() {
    let json = n12_cec_topology_probe_json(ResearchCecTopologyLimits::default()).unwrap();
    if let Ok(path) = std::env::var("EARTHMESH_N12_CEC_TOPOLOGY_JSON") {
        fs::write(path, &json).unwrap();
    }
    println!("{json}");
}
