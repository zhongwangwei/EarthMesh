use earthmesh_refine_certified::coarsen::{n12_lifted_v3_find_one_json, ResearchV3FindOneLimits};
use std::fs;

#[test]
fn frozen_find_one_is_typed_incomplete_without_resuming_cec_shards() {
    let evidence = include_str!("fixtures/n12_v3_find_one.json");
    assert!(evidence.contains("\"essential_cycles_examined\":6838"));
    assert!(evidence.contains("\"concrete_topologies_generated\":870400"));
    assert!(evidence.contains("\"ear_states\":1750528"));
    assert!(evidence.contains("\"downstream_incomplete\":6838"));
    assert!(evidence.contains("\"topology_closed\":false"));
    assert!(evidence.contains("\"shards_resumed\":false"));
    assert!(evidence.contains("\"geometry_attempted\":false"));
}

#[test]
#[ignore = "Post-PR108 fixed-budget Lifted-N12 V3 balanced-annulus find-one"]
fn find_one_lifted_v3_topology() {
    let mut limits = ResearchV3FindOneLimits::default();
    if let Ok(value) = std::env::var("EARTHMESH_N12_V3_CYCLE_STATES") {
        limits.cycle_unique_states = value.parse().unwrap();
    }
    if let Ok(value) = std::env::var("EARTHMESH_N12_V3_EAR_STATES") {
        limits.ear_states_per_topology = value.parse().unwrap();
    }
    let json = n12_lifted_v3_find_one_json(limits).unwrap();
    println!("{json}");
    if let Ok(path) = std::env::var("EARTHMESH_N12_V3_FIND_ONE_JSON") {
        fs::write(path, &json).unwrap();
    }
}
