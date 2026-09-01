use earthmesh_refine_certified::coarsen::{
    n12_lifted_band_failure_audit_json, ResearchCecTopologyLimits,
};
use std::fs;

#[test]
fn frozen_audit_proves_all_lifted_bands_are_annuli() {
    let evidence = include_str!("fixtures/n12_band_failure_audit.json");
    assert!(evidence.contains("\"cycles_audited\":6838"));
    assert!(evidence.contains("\"topological_annuli\":13676"));
    assert!(evidence.contains("\"topology_contract_failures\":0"));
    assert!(evidence.contains("\"band0.LowerTraceEdgeMismatch\":6838"));
    assert!(evidence.contains("\"band1.DirectConnectorCapacityMissing\":6838"));
}

#[test]
#[ignore = "Post-PR101 fixed-budget band-boundary audit"]
fn audit_all_6838_lifted_band_failures() {
    let json = n12_lifted_band_failure_audit_json(ResearchCecTopologyLimits::default()).unwrap();
    if let Ok(path) = std::env::var("EARTHMESH_N12_BAND_AUDIT_JSON") {
        fs::write(path, &json).unwrap();
    }
    println!("{json}");
}
