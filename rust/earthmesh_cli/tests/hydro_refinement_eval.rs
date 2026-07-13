//! Rust port of util/hydro_mesh/refinement_eval.py: background-cell + river/coast
//! overlap summaries and mkgrd log parsing. Pure JSON/text (no NetCDF data).

use earthmesh_cli::{
    hydro_refinement_eval::parse_refinement_log, hydro_refinement_eval::write_refinement_eval_json,
};

#[test]
fn summarizes_background_and_river_intersections() {
    let dir = std::env::temp_dir().join(format!("em3_refeval_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("mkdir");

    // 1e6 m^2 -> 1 km, 4e6 m^2 -> 2 km equivalent cell size.
    std::fs::write(
        dir.join("bg.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","geometry":null,"properties":{"normalized_cell_area_m2":1000000.0}},
        {"type":"Feature","geometry":null,"properties":{"normalized_cell_area_m2":4000000.0}}
        ]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("rivers.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","geometry":null,"properties":{"river_class":"R3","river_fraction":0.4,"estimated_river_area_m2":100.0}},
        {"type":"Feature","geometry":null,"properties":{"river_class":"R2","river_fraction":0.6,"estimated_river_area_m2":200.0}},
        {"type":"Feature","geometry":null,"properties":{"river_class":"R3","river_fraction":0.2,"estimated_river_area_m2":50.0}}
        ]}"#,
    )
    .unwrap();

    let out = dir.join("eval.json");
    write_refinement_eval_json(
        dir.join("bg.geojson"),
        dir.join("rivers.geojson"),
        &out,
        None,
        None,
        true,
    )
    .expect("eval");
    let json = std::fs::read_to_string(&out).unwrap();

    for needle in [
        "earthmesh_hydro_refinement_eval",
        "\"cell_count\": 2",
        "\"equivalent_cell_size_km_min\": 1",
        "\"equivalent_cell_size_km_median\": 1.5",
        "\"equivalent_cell_size_km_max\": 2",
        "\"feature_count\": 3",
        "\"R2\": 1",
        "\"R3\": 2",
        "\"river_fraction_min\": 0.2",
        "\"river_fraction_median\": 0.4",
        "\"river_fraction_max\": 0.6",
        "\"estimated_river_area_m2_sum\": 350",
    ] {
        assert!(json.contains(needle), "missing `{needle}` in:\n{json}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn parses_refinement_log_per_degree() {
    let log = "start\n\
refine_degree = 3\n\
需要细化的三角形个数 1234\n\
before  num_ref = 100\n\
after num_ref = 80\n\
去除孤立细化三角形后 64\n";
    let result = parse_refinement_log(log);
    let d3 = result.get("3").expect("degree 3");
    assert_eq!(d3.get("selected_triangles"), Some(&1234));
    assert_eq!(d3.get("before_nested_cleanup_triangles"), Some(&100));
    assert_eq!(d3.get("after_nested_cleanup_triangles"), Some(&80));
    assert_eq!(d3.get("retained_triangles"), Some(&64));
}
