//! Rust port of util/hydro_mesh/refinement_package.py::_build_manifest: assemble the
//! delivery-package manifest (the file qa_gates consumes) from eval + ranking JSON.

use earthmesh_cli::write_hydro_delivery_manifest;

#[test]
fn builds_delivery_manifest_from_eval_and_ranking() {
    let dir = std::env::temp_dir().join(format!("em3_pkg_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    std::fs::write(
        dir.join("eval.json"),
        r#"{"kind":"earthmesh_hydro_refinement_eval",
        "background_cells":{"cell_count":64},
        "river_intersections":{"feature_count":13},
        "coast_intersections":{"feature_count":75},
        "refinement_log":{"3":{"retained_triangles":64}}}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("ranking.json"),
        r#"{"kind":"earthmesh_refinement_sweep_ranking","recommended_case":"caseB","ranked_cases":[]}"#,
    )
    .unwrap();

    let out = dir.join("delivery_manifest.json");
    write_hydro_delivery_manifest(
        "gba",
        dir.join("eval.json"),
        dir.join("ranking.json"),
        &out,
        &[("eval_json".to_string(), "e.json".to_string())],
        &[("river_geojson".to_string(), "r.geojson".to_string())],
    )
    .expect("manifest");

    let json = std::fs::read_to_string(&out).unwrap();
    for needle in [
        "earthmesh_hydro_coast_delivery_package",
        "\"case_name\": \"gba\"",
        "\"recommended_case\": \"caseB\"",
        "\"background_cell_count\": 64",
        "\"river_overlap_cells\": 13",
        "\"coast_overlap_cells\": 75",
        "\"3\": 64",
        "\"river_geojson\": \"r.geojson\"",
        "\"comparison_reports\": []",
    ] {
        assert!(json.contains(needle), "missing `{needle}` in:\n{json}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
