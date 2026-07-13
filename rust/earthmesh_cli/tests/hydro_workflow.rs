//! End-to-end hydro workflow: cells × corridors -> intersections -> CoLM coupling CSV,
//! an R8 refinement plan, and a manifest, in one call. The default test is pure GeoJSON
//! (no NetCDF); a `#[ignore]` full-chain test also exercises the optional R7 mesh+land-type
//! coupling-quality branch against real fixtures (run with `make test-slow`).

use earthmesh_cli::hydro_delivery_refine_workflow::run_hydro_workflow;

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
    // R3 corridor covers slightly over half of c0. The margin avoids relying on
    // planar 0.5 equality after great-circle densification + equal-area projection.
    std::fs::write(
        dir.join("corridors.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"river_class":"R3"},
         "geometry":{"type":"Polygon","coordinates":[[[0,0],[1.1,0],[1.1,2],[0,2],[0,0]]]}}]}"#,
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
        None, // mesh (no R7 coupling-quality step in the geojson-only path)
        None, // landtype
        1,    // gridnum_perdegree (unused without mesh+landtype)
    )
    .expect("hydro workflow");

    // only c0 overlaps -> 1 intersection feature -> 1 coupling row -> 1 refined cell
    // R3 class demand is 1.0, so a physically narrow river is not rounded out.
    assert_eq!(report.intersection_cells, 1);
    assert_eq!(report.coupling_rows, 1);
    assert_eq!(report.cells_refined, 1);
    assert_eq!(report.refinement_max_level, 3);
    // no mesh + land-type given -> no R7 coupling-quality step
    assert!(report.coupling_quality_verdict.is_none());
    assert!(report.coupling_quality_path.is_none());

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
    assert!(
        manifest.contains(
            "\"overlay_semantics\": \"cell_local_lambert_azimuthal_equal_area_conservative\""
        ),
        "{manifest}"
    );
    assert!(
        manifest.contains("\"production_coupling\": true"),
        "{manifest}"
    );
    assert!(report.coupling_csv_path.ends_with("colm_coupling.csv"));
    assert!(manifest.contains("\"coupling_rows\": 1"), "{manifest}");
    assert!(manifest.contains("\"cells_refined\": 1"), "{manifest}");

    let csv = std::fs::read_to_string(&report.coupling_csv_path).unwrap();
    let columns = csv
        .lines()
        .nth(1)
        .expect("coupling row")
        .split(',')
        .collect::<Vec<_>>();
    assert_eq!(columns[0], "c0", "{csv}");
    assert_eq!(columns[2], "R3", "{csv}");
    assert!(
        (columns[3].parse::<f64>().unwrap() - 0.55).abs() < 1.0e-4,
        "{csv}"
    );
    assert!(columns[4].parse::<f64>().unwrap() > 0.0, "{csv}");
    assert!(columns[5].parse::<f64>().unwrap() > 0.0, "{csv}");

    let plan = std::fs::read_to_string(&report.refinement_plan_path).unwrap();
    assert!(plan.contains("\"target_level\": 3"), "{plan}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn class_specific_coupling_rows_share_one_refinement_cell() {
    let dir = std::env::temp_dir().join(format!("em3_wf_unique_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("cells.geojson"),
        r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","properties":{"cell_id":"c0","center_lon":1,"center_lat":1},
           "geometry":{"type":"Polygon","coordinates":[[[0,0],[2,0],[2,2],[0,2],[0,0]]]}}
        ]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("corridors.geojson"),
        r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","properties":{"river_class":"R2"},
           "geometry":{"type":"Polygon","coordinates":[[[0,0],[0.5,0],[0.5,2],[0,2],[0,0]]]}},
          {"type":"Feature","properties":{"river_class":"R3"},
           "geometry":{"type":"Polygon","coordinates":[[[1,0],[1.5,0],[1.5,2],[1,2],[1,0]]]}}
        ]}"#,
    )
    .unwrap();

    let report = run_hydro_workflow(
        dir.join("cells.geojson"),
        dir.join("corridors.geojson"),
        dir.join("workflow"),
        &["R2".to_string(), "R3".to_string()],
        0.0,
        false,
        None,
        3,
        Some(1),
        None,
        None,
        1,
    )
    .expect("hydro workflow with duplicate class rows");

    assert_eq!(report.intersection_cells, 2);
    assert_eq!(report.coupling_rows, 2, "coupling remains class-specific");
    assert_eq!(
        report.cells_refined, 1,
        "refinement budget is per mesh cell"
    );
    let plan = std::fs::read_to_string(&report.refinement_plan_path).unwrap();
    assert!(plan.contains("\"total_cells\": 1"), "{plan}");
    assert!(plan.contains("\"cell_id\": \"c0\""), "{plan}");
    let _ = std::fs::remove_dir_all(dir);
}

/// Full chain including the optional R7 mesh+land-type coupling-quality branch: synthetic
/// cells/corridors drive the hydro chain while a real EarthMesh gridfile + land-type
/// NetCDF drive coupling_quality.json. The slow-test runner provisions the NXP16
/// gridfile and requires a land-type NetCDF; run with `make test-slow`.
#[test]
#[ignore = "slow local-fixture full-chain workflow; run with make test-slow"]
fn full_chain_with_mesh_landtype_coupling_quality() {
    let required_fixture = |name: &str| {
        let path = std::env::var_os(name)
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| panic!("{name} must be set by scripts/run_slow_fixture_e2e.sh"));
        assert!(
            path.is_file(),
            "{name} fixture is missing: {}",
            path.display()
        );
        path
    };
    let gf = required_fixture("EARTHMESH_SLOW_GRIDFILE");
    let lt = required_fixture("EARTHMESH_LANDTYPE");
    let gridnum_perdegree = earthmesh_cli::mkgrd_gridinit_driver::landtype_gridnum_perdegree(&lt)
        .expect("infer land-type resolution");

    let dir = std::env::temp_dir().join(format!("em3_wf_full_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("cells.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"cell_id":"c0"},
         "geometry":{"type":"Polygon","coordinates":[[[0,0],[2,0],[2,2],[0,2],[0,0]]]}}]}"#,
    )
    .unwrap();
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
        Some(&gf),
        Some(&lt),
        gridnum_perdegree,
    )
    .expect("full-chain workflow");

    // hydro chain still works
    assert_eq!(report.intersection_cells, 1);
    assert!(report.coupling_rows >= 1);
    assert!(report.cells_refined >= 1);

    // R7 branch produced coupling_quality.json with a real verdict
    let verdict = report
        .coupling_quality_verdict
        .expect("coupling-quality verdict present");
    assert!(
        matches!(verdict.as_str(), "pass" | "warn" | "fail"),
        "{verdict}"
    );
    let cq_path = report
        .coupling_quality_path
        .expect("coupling-quality path present");
    assert!(cq_path.exists());
    let cq = std::fs::read_to_string(&cq_path).unwrap();
    assert!(
        cq.contains("\"kind\": \"earthmesh_coupling_quality\""),
        "{cq}"
    );

    // manifest lists all five artifacts + the verdict
    let manifest = std::fs::read_to_string(&report.manifest_path).unwrap();
    for needle in [
        "intersections_geojson",
        "colm_coupling_csv",
        "refinement_plan_json",
        "coupling_quality_json",
        "coupling_quality_verdict",
    ] {
        assert!(
            manifest.contains(needle),
            "manifest missing {needle}:\n{manifest}"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}
