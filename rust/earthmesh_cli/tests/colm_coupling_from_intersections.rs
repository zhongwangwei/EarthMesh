//! Rust port of util/hydro_mesh/colm_coupling.py must reproduce its semantics:
//! keep features with river_fraction >= min_fraction and non-empty cell_id + river_class,
//! and sort rows by (cell_id, river_class). Pure parsing (no NetCDF data needed).

use earthmesh_cli::{
    hydro_delivery_colm::colm_coupling_rows_from_intersections,
    hydro_delivery_colm::write_colm_coupling_csv_from_intersections,
};

const INTERSECTIONS: &str = r#"{
  "type": "FeatureCollection",
  "features": [
    {"type": "Feature", "geometry": null, "properties": {"cell_id": "c2", "cell_index": 2, "river_class": "R3", "river_fraction": 0.4}},
    {"type": "Feature", "geometry": null, "properties": {"cell_id": "c1", "cell_index": 1, "river_class": "R2", "river_fraction": 0.6}},
    {"type": "Feature", "geometry": null, "properties": {"cell_id": "c3", "cell_index": 3, "river_class": "R3", "river_fraction": 0.05}},
    {"type": "Feature", "geometry": null, "properties": {"cell_id": "c4", "cell_index": 4, "river_fraction": 0.9}},
    {"type": "Feature", "geometry": null, "properties": {"cell_id": "", "river_class": "R3", "river_fraction": 0.9}}
  ]
}"#;

#[test]
fn rows_are_filtered_and_sorted_like_python() {
    let rows = colm_coupling_rows_from_intersections(INTERSECTIONS, 0.1).expect("parse");

    // c3 dropped (below min_fraction), c4 dropped (no river_class), "" dropped (no cell_id).
    assert_eq!(rows.len(), 2, "rows: {rows:?}");

    // sorted by (cell_id, river_class): c1 before c2.
    assert_eq!(rows[0][0], "c1"); // cell_id (index 0)
    assert_eq!(rows[0][2], "R2"); // river_class (index 2)
    assert_eq!(rows[0][3], "0.6"); // river_fraction (index 3)
    assert_eq!(rows[1][0], "c2");
    assert_eq!(rows[1][2], "R3");
}

#[test]
fn min_fraction_zero_keeps_all_valid() {
    let rows = colm_coupling_rows_from_intersections(INTERSECTIONS, 0.0).expect("parse");
    // c3 now kept (0.05 >= 0); still drops c4 (no river_class) and "" (no cell_id).
    assert_eq!(rows.len(), 3, "rows: {rows:?}");
    assert!(rows.iter().any(|r| r[0] == "c3"));
}

#[test]
fn rejects_out_of_range_min_fraction() {
    assert!(colm_coupling_rows_from_intersections(INTERSECTIONS, 1.5).is_err());
}

#[test]
fn csv_writer_escapes_commas_quotes_and_newlines() {
    let dir = std::env::temp_dir().join(format!("em3_colm_csv_escape_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("intersections.geojson");
    let output = dir.join("coupling.csv");
    std::fs::write(
        &input,
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","geometry":null,"properties":{
          "cell_id":"c,1",
          "cell_index":1,
          "river_class":"R\"3\nmain",
          "river_fraction":0.6
        }}]}"#,
    )
    .unwrap();

    let rows = write_colm_coupling_csv_from_intersections(&input, &output, 0.0).expect("write csv");
    assert_eq!(rows, 1);
    let csv = std::fs::read_to_string(&output).unwrap();
    assert!(
        csv.contains("\"c,1\",1,\"R\"\"3\nmain\",0.6"),
        "escaped CSV:\n{csv}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
