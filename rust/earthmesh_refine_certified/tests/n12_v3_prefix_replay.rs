use earthmesh_refine_certified::coarsen::{
    n12_lifted_v3_prefix_replay_json, ResearchCecTopologyLimits,
};
use std::fs;

#[test]
fn frozen_prefix_reaches_meaningful_annular_downstream() {
    let evidence = include_str!("fixtures/n12_v3_prefix_replay.json");
    assert!(evidence.contains("\"essential_cycles\":6838"));
    assert!(evidence.contains("\"domains_built\":6838"));
    assert!(evidence.contains("\"reached_annular_reachability\":6838"));
    assert!(evidence.contains("\"downstream_incomplete\":6838"));
    assert!(evidence.contains("\"downstream_invalid\":0"));
    assert!(evidence.contains("\"legacy_sectorization_used\":false"));
    assert!(evidence.contains("\"shards_resumed\":false"));
    assert!(evidence.contains("\"gate_passed\":true"));
}

#[test]
#[ignore = "Post-PR107 fixed-budget Lifted-N12 V3 prefix replay"]
fn replay_lifted_v3_prefix() {
    let json = n12_lifted_v3_prefix_replay_json(ResearchCecTopologyLimits::default()).unwrap();
    if let Ok(path) = std::env::var("EARTHMESH_N12_V3_PREFIX_JSON") {
        fs::write(path, &json).unwrap();
    }
    println!("{json}");
}
