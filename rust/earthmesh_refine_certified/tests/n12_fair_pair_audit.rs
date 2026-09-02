use earthmesh_refine_certified::coarsen::{
    n12_lifted_fair_pair_audit_json, ResearchV3FindOneLimits,
};
use std::fs;

#[test]
fn frozen_pair_matrix_identifies_high_ear_anchor_repair_work() {
    let evidence = include_str!("fixtures/n12_fair_pair_audit.json");
    assert!(evidence.contains("\"essential_cycles\":6838"));
    assert!(evidence.contains("\"cell_topologies\":870400"));
    assert!(evidence.contains("\"global_pair_product\":27697152"));
    assert!(evidence.contains("\"zero_ear_pairs\":0"));
    assert!(evidence.contains("\"repairable_pairs\":4734444"));
    assert!(evidence.contains("\"impossible_pairs\":22962708"));
    assert!(evidence.contains("\"low_ear_pairs\":{\"6\":9030,\"7\":14144,\"8\":4711270}"));
    assert!(evidence.contains("\"first_pair_ear_outcome\":\"SearchIncomplete\""));
    assert!(evidence.contains("\"pair_accounting_complete\":true"));
    assert!(evidence.contains("\"errors\":{}"));
    assert!(evidence.contains("\"audit_gate_passed\":true"));
    assert!(evidence.contains("\"go_no_go\":\"GoAnchorRepairVariants\""));
    assert!(evidence.contains("\"cec_shards_resumed\":false"));
    assert!(evidence.contains("\"search_result_changed\":false"));
    assert!(evidence.contains("\"geometry_attempted\":false"));
    assert!(evidence.contains("\"product_gate_changed\":false"));
}

#[test]
#[ignore = "Post-PR109 full 64x64 Lifted-N12 pair and first-ear audit"]
fn audit_lifted_pair_product() {
    let mut limits = ResearchV3FindOneLimits::default();
    if let Ok(value) = std::env::var("EARTHMESH_N12_PAIR_AUDIT_CYCLE_STATES") {
        limits.cycle_unique_states = value.parse().unwrap();
    }
    let json = n12_lifted_fair_pair_audit_json(limits).unwrap();
    println!("{json}");
    if let Ok(path) = std::env::var("EARTHMESH_N12_PAIR_AUDIT_JSON") {
        fs::write(path, &json).unwrap();
    }
}
