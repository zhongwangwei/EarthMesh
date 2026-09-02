use earthmesh_refine_certified::coarsen::{
    n12_lifted_sdce_gipc_audit_json, ResearchCecTopologyLimits,
};
use std::fs;

#[test]
fn frozen_prefix_finds_a_zero_ear_plan_for_every_cycle() {
    let evidence = include_str!("fixtures/n12_sdce_gipc.json");
    assert!(evidence.contains("\"essential_cycles\":6838"));
    assert!(evidence.contains("\"incidence_plans_found\":6838"));
    assert!(evidence.contains("\"exact_no_plan\":0"));
    assert!(evidence.contains("\"search_incomplete\":0"));
    assert!(evidence.contains("\"invalid\":0"));
    assert!(evidence.contains("\"gipc_states\":899051"));
    assert!(evidence.contains("\"cycle_accounting_complete\":true"));
    assert!(evidence.contains("\"gate_passed\":true"));
    assert!(evidence.contains("\"go_no_go\":\"GoPierExactWitness\""));
}

#[test]
fn frozen_selected_plans_remove_every_pr111_degree_defect() {
    let evidence = include_str!("fixtures/n12_sdce_gipc.json");
    for slot in [48, 52, 78, 252, 256, 343] {
        assert!(evidence.contains(&format!("\"{slot}\":{{\"6\":6838}}")));
    }
    assert!(evidence.contains("\"zero_ear_family_only\":true"));
    assert!(evidence.contains("\"concrete_topology_search_run\":false"));
    assert!(evidence.contains("\"topology_closed\":false"));
    assert!(evidence.contains("\"geometry_attempted\":false"));
    assert!(evidence.contains("\"cec_shards_resumed\":false"));
}

#[test]
#[ignore = "PR113 full fixed-prefix zero-ear GIPC audit"]
fn audit_all_lifted_sdce_incidence_plans() {
    let json = n12_lifted_sdce_gipc_audit_json(ResearchCecTopologyLimits::default()).unwrap();
    if let Ok(path) = std::env::var("EARTHMESH_N12_SDCE_GIPC_JSON") {
        fs::write(path, &json).unwrap();
    }
    println!("{json}");
}
