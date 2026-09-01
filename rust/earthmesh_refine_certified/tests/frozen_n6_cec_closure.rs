use earthmesh_refine_certified::coarsen::{
    frozen_n6_cec_closure_report_json, run_frozen_n6_cec_closure, FrozenN6CecClosureLimits,
    FrozenN6CecResumeMap,
};
use std::fs;

#[test]
fn frozen_n6_cec_closure_snapshot_keeps_incomplete_classes_distinct() {
    let snapshot = include_str!("fixtures/frozen_n6_cec_closure.json");
    assert!(snapshot.contains("\"targeted_unknowns\":659"));
    assert!(snapshot.contains("\"Closed\":0"));
    assert!(snapshot.contains("\"ExactNoSolution\":43"));
    assert!(snapshot.contains("\"CycleSearchIncomplete\":553"));
    assert!(snapshot.contains("\"DownstreamSearchIncomplete\":63"));
    assert!(snapshot.contains("\"total_unique_states\":9313694"));
    assert!(!snapshot.contains("\"InvalidInput\""));
}

#[test]
#[ignore = "Alpha6 PR94 full Frozen N6 659-family CEC classification"]
fn print_frozen_n6_cec_closure() {
    let report = run_frozen_n6_cec_closure(
        FrozenN6CecClosureLimits::default(),
        &FrozenN6CecResumeMap::new(),
    )
    .unwrap();
    let json = frozen_n6_cec_closure_report_json(&report);
    if let Ok(path) = std::env::var("EARTHMESH_CEC_CLOSURE_JSON") {
        fs::write(path, &json).unwrap();
    }
    println!("{json}");
}
