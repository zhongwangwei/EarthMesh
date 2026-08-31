use earthmesh_refine_certified::coarsen::{
    effective_band_error_json, n6_legacy_mixed_fixture, plan_effective_transition_bands,
    transition_band_plan_json, EffectiveBandError, TransitionBandMode,
};
use std::{fs, process::Command};

#[test]
fn frozen_n6_legacy_w2_is_nominal_not_positive_width() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let plan = plan_effective_transition_bands(
        &source,
        &component,
        TransitionBandMode::LegacyTwoLogicalBands,
    )
    .unwrap();
    assert_eq!(plan.traces.len(), 3);
    assert_eq!(plan.adjacent_shared_vertices, 14);
    assert_eq!(plan.adjacent_shared_edges, 0);
    assert_eq!(plan.effective_band_count_min, 0);
}

#[test]
fn frozen_n6_w3_fails_closed_when_no_positive_width_plan_exists() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let error = plan_effective_transition_bands(
        &source,
        &component,
        TransitionBandMode::ThreeEffectiveBands,
    )
    .unwrap_err();
    let EffectiveBandError::InsufficientAnnulusWidth {
        mode,
        best_effective_band_count,
        adjacent_shared_edges,
        outward_expansions,
        ..
    } = error
    else {
        panic!("expected InsufficientAnnulusWidth");
    };
    assert_eq!(mode, TransitionBandMode::ThreeEffectiveBands);
    assert!(best_effective_band_count < 3);
    assert_eq!(adjacent_shared_edges, 0);
    assert_eq!(outward_expansions, 4);
}

#[test]
fn frozen_n6_w4_stops_when_the_global_w3_prerequisite_fails() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let error = plan_effective_transition_bands(
        &source,
        &component,
        TransitionBandMode::FourEffectiveBandsNearSingularities,
    )
    .unwrap_err();
    let EffectiveBandError::InsufficientAnnulusWidth {
        mode,
        best_effective_band_count,
        adjacent_shared_edges,
        outward_expansions,
        reason,
        ..
    } = error
    else {
        panic!("expected InsufficientAnnulusWidth");
    };
    assert_eq!(
        mode,
        TransitionBandMode::FourEffectiveBandsNearSingularities
    );
    assert!(best_effective_band_count < 3);
    assert_eq!(adjacent_shared_edges, 0);
    assert_eq!(outward_expansions, 4);
    assert!(reason.starts_with("global W3 prerequisite failed; local W4 zones were not evaluated"));
}

#[test]
fn frozen_n6_w3_failure_json_is_byte_stable() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let first = plan_effective_transition_bands(
        &source,
        &component,
        TransitionBandMode::ThreeEffectiveBands,
    );
    let second = plan_effective_transition_bands(
        &source,
        &component,
        TransitionBandMode::ThreeEffectiveBands,
    );
    assert_eq!(first, second);
    let first = effective_band_error_json(&first.unwrap_err());
    let second = effective_band_error_json(&second.unwrap_err());
    assert_eq!(first, second);
    assert!(first.contains("\"planning_family\":\"ParentLayerTraceFamily\""));
    assert!(first.contains("\"outcome\":\"InsufficientAnnulusWidth\""));
    assert!(first.contains("UnsupportedInteriorIcosahedronVertex"));
}

#[test]
#[ignore = "explicit Frozen N6 PR55 W3/W4 planning gate"]
fn frozen_n6_pr55_transition_band_planner_probe() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let legacy = plan_effective_transition_bands(
        &source,
        &component,
        TransitionBandMode::LegacyTwoLogicalBands,
    )
    .unwrap();
    let w3 = plan_effective_transition_bands(
        &source,
        &component,
        TransitionBandMode::ThreeEffectiveBands,
    )
    .unwrap_err();
    let w4 = plan_effective_transition_bands(
        &source,
        &component,
        TransitionBandMode::FourEffectiveBandsNearSingularities,
    )
    .unwrap_err();
    let commit = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
        .map(|value| format!("\"{value}\""))
        .unwrap_or_else(|| "null".into());
    let json = format!(
        "{{\"schema_version\":1,\"probe\":\"FrozenN6Pr55TransitionBandPlanner\",\"commit_sha\":{},\"legacy\":{},\"w3\":{},\"w4\":{}}}",
        commit,
        transition_band_plan_json(&legacy),
        effective_band_error_json(&w3),
        effective_band_error_json(&w4),
    );
    if let Ok(path) = std::env::var("EARTHMESH_GEOMETRY_JSON") {
        fs::write(path, &json).unwrap();
    }
    eprintln!("{json}");
}
