//! End-to-end hydro workflow: cells × corridors -> intersections -> CoLM coupling CSV,
//! an R8 refinement plan, and a manifest, in one call. Pure GeoJSON (no NetCDF), proving
//! the migrated overlay / coupling / planner pieces chain together.

use earthmesh_cli::run_hydro_workflow;

#[test]
fn cells_and_corridors_to_coupling_and_plan() {
    let dir = std::env::temp_dir().join(format!("em3_wf_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    // c0 = [0,2]^2 (area 4); c1 = [4,6]x[0,2] (no overlap).
    std::fs::write(
        dir.join("cells.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"cell_id":"c0"},
         "geometry":{"type":"Polygon","coordinates":[[[0,0],[2,0],[2,2],[0,2],[0,0]]]}},
        {"type":"Feature","properties":{"cell_id":"c1"},
         "geometry":{"type":"Polygon","coordinates":[[[4,0],[6,0],[6,2],[4,2],[4,0]]]}}]}"#,
    )
    .unwrap();
    // R3 corridor = left half of c0 -> river_fraction 0.5 for c0, no overlap for c1.
    std::fs::write(
        dir.join("corridors.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"river_class":"R3"},
         "geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,2],[0,2],[0,0]]]}}]}"#,
    )
    .unwrap();

    let out_dir = dir.join("workflow");
    let report = run_hydro_workflow(
        dir.join("cells.geojson"),
        dir.join("corridors.geojson"),
        &out_dir,
        &["R3".to_string()],
        0.0,
        false,
        None,
        3,
        None,
    )
    .expect("hydro workflow");

    // only c0 overlaps -> 1 intersection feature -> 1 coupling row -> 1 refined cell
    // (river_fraction 0.5 -> target_level round(0.5*3) = 2).
    assert_eq!(report.intersection_cells, 1);
    assert_eq!(report.coupling_rows, 1);
    assert_eq!(report.cells_refined, 1);
    assert_eq!(report.refinement_max_level, 2);

    // all four artifacts exist
    for p in [
        &report.intersections_path,
        &report.coupling_csv_path,
        &report.refinement_plan_path,
        &report.manifest_path,
    ] {
        assert!(p.exists(), "missing artifact {}", p.display());
    }

    let manifest = std::fs::read_to_string(&report.manifest_path).unwrap();
    assert!(manifest.contains("\"kind\": \"earthmesh_hydro_workflow\""));
    assert!(manifest.contains("\"coupling_rows\": 1"), "{manifest}");
    assert!(manifest.contains("\"cells_refined\": 1"), "{manifest}");

    let csv = std::fs::read_to_string(&report.coupling_csv_path).unwrap();
    assert!(
        csv.contains("c0") && csv.contains("R3") && csv.contains("0.5"),
        "{csv}"
    );

    let plan = std::fs::read_to_string(&report.refinement_plan_path).unwrap();
    assert!(plan.contains("\"target_level\": 2"), "{plan}");

    let _ = std::fs::remove_dir_all(&dir);
}
