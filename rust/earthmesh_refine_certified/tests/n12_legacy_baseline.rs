use earthmesh_refine_certified::coarsen::{
    n12_interior_control_fixture, n12_legacy_baseline_json, n12_lifted_n6_fixture,
    run_n12_legacy_baseline, ResearchLegacyLimits, ResearchN12OutcomeKind,
};

#[test]
fn research_n12_never_publishes_product() {
    let limits = ResearchLegacyLimits {
        face_band_states: 0,
        downstream_topology_states: 0,
    };
    for fixture in [
        n12_lifted_n6_fixture().unwrap(),
        n12_interior_control_fixture().unwrap(),
    ] {
        let report = run_n12_legacy_baseline(&fixture, limits);
        assert!(!report.product_grid_written);
        assert!(!report.ready_marker_written);
        assert!(!report.product_gate_changed);
    }
}

#[test]
fn legacy_budget_exhaustion_is_research_incomplete() {
    let fixture = n12_lifted_n6_fixture().unwrap();
    let report = run_n12_legacy_baseline(
        &fixture,
        ResearchLegacyLimits {
            face_band_states: 0,
            downstream_topology_states: 0,
        },
    );
    assert_eq!(
        report.outcome,
        ResearchN12OutcomeKind::ResearchSearchIncomplete
    );
    assert!(!report.reason.contains("no solution"));
}

#[test]
fn n12_legacy_baseline_is_deterministic() {
    let limits = ResearchLegacyLimits {
        face_band_states: 64,
        downstream_topology_states: 64,
    };
    assert_eq!(
        n12_legacy_baseline_json(limits).unwrap(),
        n12_legacy_baseline_json(limits).unwrap()
    );
}

#[test]
fn frozen_n12_legacy_baseline_matches_snapshot() {
    assert_eq!(
        n12_legacy_baseline_json(ResearchLegacyLimits::default()).unwrap(),
        include_str!("fixtures/n12_legacy_baseline.json").trim()
    );
}

#[test]
#[ignore = "bounded Alpha6 PR88 research probe"]
fn print_n12_legacy_baseline() {
    println!(
        "{}",
        n12_legacy_baseline_json(ResearchLegacyLimits::default()).unwrap()
    );
}
