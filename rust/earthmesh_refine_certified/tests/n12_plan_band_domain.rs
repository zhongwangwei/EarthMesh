use earthmesh_refine_certified::coarsen::{
    n12_lifted_plan_band_domain_audit_json, ResearchCecTopologyLimits,
};
use std::fs;

#[test]
fn frozen_prefix_builds_two_annuli_per_cycle_without_legacy_shells() {
    let evidence = include_str!("fixtures/n12_plan_band_domain.json");
    assert!(evidence.contains("\"plans_built\":6838"));
    assert!(evidence.contains("\"annular_bands\":13676"));
    assert!(evidence.contains("\"contracted_band0\":6838"));
    assert!(evidence.contains("\"band0_topology\":16,\"band0_source\":32"));
    assert!(evidence.contains("\"gate_passed\":true"));
    assert!(evidence.contains("\"coupled_annulus_used\":false"));
}

#[test]
#[ignore = "Post-PR101 fixed-budget plan-native band-domain audit"]
fn build_all_lifted_plan_band_domains() {
    let json =
        n12_lifted_plan_band_domain_audit_json(ResearchCecTopologyLimits::default()).unwrap();
    if let Ok(path) = std::env::var("EARTHMESH_N12_PLAN_BAND_JSON") {
        fs::write(path, &json).unwrap();
    }
    println!("{json}");
}
