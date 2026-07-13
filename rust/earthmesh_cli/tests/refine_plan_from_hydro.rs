//! R8 refinement planner (earthmesh_refine_planner) driven by the real MERIT-Hydro
//! river/coast signal: a per-cell intersection / complete-mask GeoJSON ->
//! hydro_coast_score demand -> target_level plan. Pure (no NetCDF).

use earthmesh_cli::hydro_delivery_refine_workflow::plan_refinement_from_hydro_geojson;

#[test]
fn river_fraction_drives_target_level() {
    let dir = std::env::temp_dir().join(format!("em3_refplan_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // three cells with river_fraction 1.0 / 0.5 / 0.0
    std::fs::write(
        dir.join("cells.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"cell_id":"c0","river_fraction":1.0,"coastal_fraction":0.0},
         "geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1],[0,0]]]}},
        {"type":"Feature","properties":{"cell_id":"c1","river_fraction":0.5,"coastal_fraction":0.0},
         "geometry":{"type":"Polygon","coordinates":[[[1,0],[2,0],[2,1],[1,1],[1,0]]]}},
        {"type":"Feature","properties":{"cell_id":"c2","river_fraction":0.0,"coastal_fraction":0.0},
         "geometry":{"type":"Polygon","coordinates":[[[2,0],[3,0],[3,1],[2,1],[2,0]]]}}]}"#,
    )
    .unwrap();
    let out = dir.join("plan.json");
    let report =
        plan_refinement_from_hydro_geojson(dir.join("cells.geojson"), &out, 3, None).expect("plan");

    // target_level = round(demand * max_level): 1.0->3, 0.5->2 (round 1.5), 0.0->0
    assert_eq!(report.target_levels.level, vec![3, 2, 0]);
    assert_eq!(report.budget_used.cells_refined_after, 2);

    let json = std::fs::read_to_string(&out).unwrap();
    assert!(json.contains("\"kind\": \"earthmesh_refinement_plan\""));
    assert!(json.contains("\"cells_refined\": 2"), "{json}");
    assert!(json.contains("\"target_level\": 3"), "{json}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn coastal_fraction_drives_demand_and_budget_caps_cells() {
    let dir = std::env::temp_dir().join(format!("em3_refplan_b_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // two coastal cells; cell 0 demand 1.0, cell 1 demand 0.9. Budget keeps only 1.
    std::fs::write(
        dir.join("cells.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"river_fraction":0.0,"coastal_fraction":1.0},
         "geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1],[0,0]]]}},
        {"type":"Feature","properties":{"river_fraction":0.0,"coastal_fraction":0.9},
         "geometry":{"type":"Polygon","coordinates":[[[1,0],[2,0],[2,1],[1,1],[1,0]]]}}]}"#,
    )
    .unwrap();
    let out = dir.join("plan.json");
    let report = plan_refinement_from_hydro_geojson(dir.join("cells.geojson"), &out, 2, Some(1))
        .expect("plan");

    // coastal_fraction feeds the same hydro_coast_score demand; budget keeps the top cell.
    assert_eq!(report.budget_used.cells_refined_after, 1);
    assert!(report.budget_used.budget_hit, "budget should bind");
    assert!(
        report.target_levels.level[0] > 0,
        "highest-demand cell kept"
    );
    assert_eq!(
        report.target_levels.level[1], 0,
        "lower-demand cell dropped"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn duplicate_class_rows_use_one_unique_cell_budget() {
    let dir = std::env::temp_dir().join(format!("em3_refplan_unique_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let geometry = r#"{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1],[0,0]]]}"#;
    std::fs::write(
        dir.join("cells.geojson"),
        format!(
            r#"{{"type":"FeatureCollection","features":[
              {{"type":"Feature","properties":{{"cell_id":"c0","overlap_class":"R2","river_fraction":0.4}},"geometry":{geometry}}},
              {{"type":"Feature","properties":{{"cell_id":"c0","overlap_class":"R3","river_fraction":0.1}},"geometry":{geometry}}}
            ]}}"#
        ),
    )
    .unwrap();
    let output = dir.join("plan.json");
    let report = plan_refinement_from_hydro_geojson(dir.join("cells.geojson"), &output, 3, Some(1))
        .expect("plan unique hydro cell");

    assert_eq!(report.target_levels.level, vec![3]);
    assert_eq!(report.budget_used.cells_refined_after, 1);
    assert!(!report.budget_used.budget_hit);
    let json = std::fs::read_to_string(output).unwrap();
    assert!(json.contains("\"total_cells\": 1"), "{json}");
    assert!(json.contains("\"cells_refined\": 1"), "{json}");
    assert!(json.contains("\"cell_id\": \"c0\""), "{json}");
    let _ = std::fs::remove_dir_all(dir);
}
