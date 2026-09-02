use earthmesh_refine_certified::coarsen::{
    n12_lifted_sdce_incidence_contract_audit_json, ResearchCecTopologyLimits,
};
use std::fs;

#[test]
fn frozen_prefix_has_exact_nonempty_incidence_contracts() {
    let evidence = include_str!("fixtures/n12_sdce_incidence_contract.json");
    assert!(evidence.contains("\"essential_cycles\":6838"));
    assert!(evidence.contains("\"contracts_built\":6838"));
    assert!(evidence.contains("\"contract_vertices\":892213"));
    assert!(evidence.contains("\"contract_owner_tuples\":7420459"));
    assert!(evidence.contains("\"empty_vertex_domains\":0"));
    assert!(evidence.contains("\"invalid_cell_sums\":0"));
    assert!(evidence.contains("\"transition_charge_histogram\":{\"4\":6838}"));
    assert!(evidence.contains("\"adapter_mismatch\":0"));
    assert!(evidence.contains("\"gate_passed\":true"));
    assert!(evidence.contains("\"go_no_go\":\"GoGlobalIncidencePlanCsp\""));
}

#[test]
fn pr111_defects_are_reported_as_domains_not_production_special_cases() {
    let evidence = include_str!("fixtures/n12_sdce_incidence_contract.json");
    for slot in [48, 52, 78, 252, 256, 343] {
        assert!(evidence.contains(&format!("\"{slot}\":{{\"cycles_present\":6838")));
    }
    assert!(evidence.contains("\"concrete_witnesses\":false"));
    assert!(evidence.contains("\"backpointers\":false"));
    assert!(evidence.contains("\"necessary_relaxation_only\":true"));
    assert!(evidence.contains("\"concrete_topology_search_run\":false"));
    assert!(evidence.contains("\"geometry_attempted\":false"));
    assert!(evidence.contains("\"cec_shards_resumed\":false"));
}

#[test]
#[ignore = "PR112 fixed-prefix SDCE incidence-contract audit"]
fn audit_all_lifted_sdce_incidence_contracts() {
    let json = n12_lifted_sdce_incidence_contract_audit_json(ResearchCecTopologyLimits::default())
        .unwrap();
    if let Ok(path) = std::env::var("EARTHMESH_N12_SDCE_CONTRACT_JSON") {
        fs::write(path, &json).unwrap();
    }
    println!("{json}");
}
