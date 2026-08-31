use earthmesh_refine_certified::coarsen::{
    audit_coupled_rotation_aware_width, audit_rotation_aware_width, build_stratified_annulus,
    extract_coupled_annulus, n6_legacy_mixed_fixture, parent_layer_trace_family_candidate,
    rotation_aware_width_report_json, TRANSITION_BAND_PLANNING_FAMILY,
};
use std::{fs, process::Command};

#[test]
fn legacy_w2_shared_vertices_are_fully_classified() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let stratified = build_stratified_annulus(&source, &component).unwrap();
    let report = audit_rotation_aware_width(&source, &stratified);
    assert_eq!(report.adjacent_trace_shared_occurrence_count, 14);
    assert_eq!(report.unique_shared_vertex_count, 14);
    assert_eq!(report.true_pinch_count, 0);
    assert_eq!(report.one_face_wedge_count, 12);
    assert_eq!(report.multi_face_wedge_count, 0);
    assert_eq!(report.anchor_junction_count, 2);
    assert_eq!(
        report.adjacent_trace_shared_occurrence_count,
        report.true_pinch_count
            + report.one_face_wedge_count
            + report.multi_face_wedge_count
            + report.anchor_junction_count
    );
}

#[test]
fn pr54_w3_candidate_shared_vertices_are_fully_classified() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let candidate = parent_layer_trace_family_candidate(&source, &component, 1, 2).unwrap();
    let coupled = extract_coupled_annulus(&source, &candidate).unwrap();
    let report = audit_coupled_rotation_aware_width(&source, &candidate, &coupled).unwrap();
    assert_eq!(report.adjacent_trace_shared_occurrence_count, 24);
    assert_eq!(report.unique_shared_vertex_count, 20);
    assert_eq!(report.true_pinch_count, 0);
    assert_eq!(report.one_face_wedge_count, 20);
    assert_eq!(report.multi_face_wedge_count, 0);
    assert_eq!(report.anchor_junction_count, 4);
    assert_eq!(
        report.adjacent_trace_shared_occurrence_count,
        report.true_pinch_count
            + report.one_face_wedge_count
            + report.multi_face_wedge_count
            + report.anchor_junction_count
    );
}

#[test]
fn anchor_rotation_and_json_are_stable() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let stratified = build_stratified_annulus(&source, &component).unwrap();
    let first = audit_rotation_aware_width(&source, &stratified);
    let second = audit_rotation_aware_width(&source, &stratified);
    assert_eq!(first, second);
    assert_eq!(
        rotation_aware_width_report_json(&first),
        rotation_aware_width_report_json(&second)
    );
}

#[test]
#[ignore = "explicit Frozen N6 PR55 rotation-aware width audit"]
fn frozen_n6_pr55_rotation_width_probe() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let legacy = build_stratified_annulus(&source, &component).unwrap();
    let candidate = parent_layer_trace_family_candidate(&source, &component, 1, 2).unwrap();
    let w3 = extract_coupled_annulus(&source, &candidate).unwrap();
    let legacy = audit_rotation_aware_width(&source, &legacy);
    let w3 = audit_coupled_rotation_aware_width(&source, &candidate, &w3).unwrap();
    assert_eq!(legacy.true_pinch_count, 0);
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
        "{{\"schema_version\":1,\"probe\":\"FrozenN6Pr55RotationAwareWidth\",\"commit_sha\":{},\"taskbook_sha256\":\"46d5f8d1ab439ce972186ba50798806b520fe9bdac3f675806d4cd18cff38e2b\",\"planning_family\":\"{}\",\"legacy_w2\":{},\"pr54_w3_candidate\":{},\"pr53_worst_100_near_true_pinch_fraction\":{:.12}}}",
        commit,
        TRANSITION_BAND_PLANNING_FAMILY.as_str(),
        rotation_aware_width_report_json(&legacy),
        rotation_aware_width_report_json(&w3),
        0.0,
    );
    if let Ok(path) = std::env::var("EARTHMESH_GEOMETRY_JSON") {
        fs::write(path, &json).unwrap();
    }
    eprintln!("{json}");
}
