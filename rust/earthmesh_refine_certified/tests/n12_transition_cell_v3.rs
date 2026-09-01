use earthmesh_refine_certified::coarsen::{
    n12_lifted_transition_cell_v3_audit_json, ResearchCecTopologyLimits,
};
use std::fs;

#[test]
fn frozen_prefix_builds_only_plan_native_annular_cells() {
    let evidence = include_str!("fixtures/n12_transition_cell_v3.json");
    assert!(evidence.contains("\"domains_built\":6838"));
    assert!(evidence.contains("\"annular_cells\":13676"));
    assert!(evidence.contains("\"disk_cells\":0"));
    assert!(evidence.contains("\"coupled_annulus_used\":false"));
    assert!(evidence.contains("\"legacy_monotone_connectors_used\":false"));
    assert!(evidence.contains("\"gate_passed\":true"));
}

#[test]
#[ignore = "Post-PR101 fixed-budget TransitionCell V3 audit"]
fn build_all_lifted_v3_domains() {
    let json =
        n12_lifted_transition_cell_v3_audit_json(ResearchCecTopologyLimits::default()).unwrap();
    if let Ok(path) = std::env::var("EARTHMESH_N12_TRANSITION_CELL_V3_JSON") {
        fs::write(path, &json).unwrap();
    }
    println!("{json}");
}
