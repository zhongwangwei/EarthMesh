//! Rust port of util/hydro_mesh/cell_mask_merge.py: annotate every background cell with
//! surface_class (max-area LAND/OCEAN overlay) + dominant mask_class (river > coast >
//! surface). Pure geometry (no NetCDF data).

use earthmesh_cli::write_complete_cell_mask_geojson;

#[test]
fn complete_mask_assigns_surface_and_river_priority() {
    let dir = std::env::temp_dir().join(format!("em3_complete_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    // c1 over [0,2]x[0,2], c2 over [10,12]x[0,2].
    std::fs::write(
        dir.join("bg.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"cell_id":"c1"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[2,0],[2,2],[0,2],[0,0]]]}},
        {"type":"Feature","properties":{"cell_id":"c2"},"geometry":{"type":"Polygon","coordinates":[[[10,0],[12,0],[12,2],[10,2],[10,0]]]}}
        ]}"#,
    )
    .unwrap();
    // sparse river overlap: c1 is an R3 river cell.
    std::fs::write(
        dir.join("river.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"cell_id":"c1","river_class":"R3","river_fraction":0.5},
         "geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,2],[0,2],[0,0]]]}}
        ]}"#,
    )
    .unwrap();
    // surface: LAND over c1, OCEAN over c2.
    std::fs::write(
        dir.join("surface.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"surface_class":"LAND"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[2,0],[2,2],[0,2],[0,0]]]}},
        {"type":"Feature","properties":{"surface_class":"OCEAN"},"geometry":{"type":"Polygon","coordinates":[[[10,0],[12,0],[12,2],[10,2],[10,0]]]}}
        ]}"#,
    )
    .unwrap();

    let out = dir.join("complete.geojson");
    let n = write_complete_cell_mask_geojson(
        dir.join("bg.geojson"),
        &out,
        Some(&dir.join("river.geojson")),
        None,
        Some(&dir.join("surface.geojson")),
    )
    .expect("complete mask");
    assert_eq!(n, 2, "one feature per background cell");

    let json = std::fs::read_to_string(&out).unwrap();
    // Parse loosely: find each cell's block by cell_id and check its mask/surface class.
    // c1: river R3 beats LAND surface; c2: OCEAN surface, no hydro.
    assert!(json.contains("\"cell_id\": \"c1\""));
    assert!(
        json.contains("\"mask_class\": \"R3\""),
        "c1 should be R3:\n{json}"
    );
    assert!(json.contains("\"surface_class\": \"LAND\""));
    assert!(
        json.contains("\"mask_class\": \"OCEAN\""),
        "c2 should be OCEAN:\n{json}"
    );
    assert!(json.contains("\"surface_class\": \"OCEAN\""));
    assert!(json.contains("\"is_hydro_masked\": true")); // c1
    assert!(json.contains("\"is_hydro_masked\": false")); // c2
    let _ = std::fs::remove_dir_all(&dir);
}
