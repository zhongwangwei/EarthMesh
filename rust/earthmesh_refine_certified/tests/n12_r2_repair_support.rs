use earthmesh_refine_certified::coarsen::{
    n12_lifted_r2_repair_support_audit_json, ResearchV3FindOneLimits,
};
use std::fs;

#[test]
fn frozen_pair_matrix_separates_r2_depth_from_permanent_impossibility() {
    let evidence = include_str!("fixtures/n12_r2_repair_support.json");
    assert!(evidence.contains("\"global_pair_product\":27697152"));
    assert!(evidence.contains("\"permanent_impossible\":4734444"));
    assert!(evidence.contains("\"outside_repair_depth_r2\":22962708"));
    assert!(evidence.contains("\"r2_anchor_necessary_candidates\":4734444"));
    assert!(evidence.contains("\"r2_preflight_passed\":0"));
    assert!(evidence.contains("\"unaffected_degree_rejects\":4734444"));
    assert!(evidence.contains("\"preflight_rejected_by_k\":{\"6\":9030,\"7\":14144,\"8\":4711270}"));
    assert!(evidence.contains("\"pair_accounting_complete\":true"));
    assert!(evidence.contains("\"audit_gate_passed\":true"));
    assert!(evidence.contains("\"go_no_go\":\"GoSignatureDirectedConcreteExtraction\""));
    assert!(evidence.contains("\"cec_shards_resumed\":false"));
    assert!(evidence.contains("\"new_repair_solver_run\":false"));
    assert!(evidence.contains("\"geometry_attempted\":false"));
    assert!(evidence.contains("\"product_gate_changed\":false"));
}

#[test]
fn partial_balanced_subset_never_reports_csae_nogo() {
    let evidence = include_str!("fixtures/n12_r2_repair_support.json");
    assert!(evidence.contains("\"scope_conclusion\":\"BalancedSubsetEvidenceOnly\""));
    assert!(!evidence.contains("CsaeNoGo"));
}

#[test]
#[ignore = "Post-PR110 full Lifted-N12 R2 repair-support preflight"]
fn audit_lifted_r2_repair_support() {
    let mut limits = ResearchV3FindOneLimits::default();
    if let Ok(value) = std::env::var("EARTHMESH_N12_R2_PREFLIGHT_CYCLE_STATES") {
        limits.cycle_unique_states = value.parse().unwrap();
    }
    let json = n12_lifted_r2_repair_support_audit_json(limits).unwrap();
    println!("{json}");
    if let Ok(path) = std::env::var("EARTHMESH_N12_R2_PREFLIGHT_JSON") {
        fs::write(path, &json).unwrap();
    }
}
