use earthmesh_refine_certified::coarsen::{n12_lifted_v2_replay_json, ResearchCecTopologyLimits};
use std::fs;

#[test]
fn frozen_replay_records_the_pr101_stop_gate() {
    let evidence = include_str!("fixtures/n12_lifted_v2_replay.json");
    assert!(evidence.contains("\"inner_guard_rejects\":0"));
    assert!(evidence.contains("\"StratifiedSectorization\":6838"));
    assert!(evidence.contains("\"reached_meaningful_downstream\":false"));
    assert!(evidence.contains("\"gate_passed\":false"));
}

#[test]
#[ignore = "Post-PR97 fixed-budget Adapter V2 replay"]
fn replay_first_16384_lifted_n12_states() {
    let json = n12_lifted_v2_replay_json(ResearchCecTopologyLimits::default()).unwrap();
    if let Ok(path) = std::env::var("EARTHMESH_N12_V2_REPLAY_JSON") {
        fs::write(path, &json).unwrap();
    }
    println!("{json}");
}
