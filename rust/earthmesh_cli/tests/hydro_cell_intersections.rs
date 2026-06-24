//! Rust port of util/hydro_mesh/earthmesh_intersection.py: overlay cells x corridors
//! into a per-cell intersection GeoJSON, then feed that into the colm_coupling port —
//! proving the full mesh+masks -> intersections -> coupling pipeline runs in Rust
//! (shapely-free). Pure geometry (no NetCDF data).

use earthmesh_cli::{colm_coupling_rows_from_intersections, write_earthmesh_intersection_geojson};
use flate2::{write::GzEncoder, Compression};
use std::io::Write;
use std::path::Path;

fn write_gzip(path: &Path, content: &str) {
    let file = std::fs::File::create(path).unwrap();
    let mut encoder = GzEncoder::new(file, Compression::default());
    encoder.write_all(content.as_bytes()).unwrap();
    encoder.finish().unwrap();
}

#[test]
fn cell_river_overlap_fraction_and_coupling_chain() {
    let dir = std::env::temp_dir().join(format!("em3_xsect_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    // Cell = 2x2 square (area 4). R3 corridor = left half [0,1]x[0,2] (area 2).
    // Overlap area 2 -> river_fraction 0.5.
    std::fs::write(
        dir.join("cells.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"cell_id":"c1","source_areaCell":4.0},
         "geometry":{"type":"Polygon","coordinates":[[[0,0],[2,0],[2,2],[0,2],[0,0]]]}}
        ]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("corridors.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"river_class":"R3"},
         "geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,2],[0,2],[0,0]]]}}
        ]}"#,
    )
    .unwrap();

    let out = dir.join("intersections.geojson");
    let n = write_earthmesh_intersection_geojson(
        dir.join("cells.geojson"),
        dir.join("corridors.geojson"),
        &out,
        &["R3".to_string()],
        0.0,
        false,
        None,
    )
    .expect("intersections");
    assert_eq!(n, 1, "one cell x R3 overlap feature");

    let geojson = std::fs::read_to_string(&out).unwrap();
    assert!(geojson.contains("\"river_class\": \"R3\""), "{geojson}");
    assert!(geojson.contains("\"river_fraction\": 0.5"), "{geojson}");

    // Full Rust chain: feed the intersection GeoJSON into the colm_coupling port.
    let rows = colm_coupling_rows_from_intersections(&geojson, 0.0).expect("coupling");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0], "c1"); // cell_id
    assert_eq!(rows[0][2], "R3"); // river_class
    assert_eq!(rows[0][3], "0.5"); // river_fraction

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn min_fraction_filters_small_overlaps() {
    let dir = std::env::temp_dir().join(format!("em3_xsect_min_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // R3 corridor covers only a thin 0.1-wide strip -> fraction 0.05, below min 0.1.
    std::fs::write(
        dir.join("cells.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"cell_id":"c1"},
         "geometry":{"type":"Polygon","coordinates":[[[0,0],[2,0],[2,2],[0,2],[0,0]]]}}]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("corridors.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"river_class":"R3"},
         "geometry":{"type":"Polygon","coordinates":[[[0,0],[0.1,0],[0.1,2],[0,2],[0,0]]]}}]}"#,
    )
    .unwrap();
    let out = dir.join("x.geojson");
    let n = write_earthmesh_intersection_geojson(
        dir.join("cells.geojson"),
        dir.join("corridors.geojson"),
        &out,
        &["R3".to_string()],
        0.1,
        false,
        None,
    )
    .expect("x");
    assert_eq!(n, 0, "0.05 fraction filtered out by min_fraction 0.1");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn domain_bbox_clips_corridors() {
    let dir = std::env::temp_dir().join(format!("em3_xsect_dom_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // Cell 4x4 (area 16). R3 corridor [0,6]x[0,2] (wider than the cell).
    std::fs::write(
        dir.join("cells.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"cell_id":"c1"},
         "geometry":{"type":"Polygon","coordinates":[[[0,0],[4,0],[4,4],[0,4],[0,0]]]}}]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("corridors.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"river_class":"R3"},
         "geometry":{"type":"Polygon","coordinates":[[[0,0],[6,0],[6,2],[0,2],[0,0]]]}}]}"#,
    )
    .unwrap();
    let out = dir.join("x.geojson");
    // domain bbox [0,0,2,4] keeps only the left half -> corridor∩domain∩cell = [0,2]x[0,2]
    // area 4 -> fraction 4/16 = 0.25 (without domain it would be [0,4]x[0,2]=8 -> 0.5).
    let domain = vec![vec![(0.0, 0.0), (2.0, 0.0), (2.0, 4.0), (0.0, 4.0)]];
    write_earthmesh_intersection_geojson(
        dir.join("cells.geojson"),
        dir.join("corridors.geojson"),
        &out,
        &["R3".to_string()],
        0.0,
        false,
        Some(&domain),
    )
    .expect("x");
    let json = std::fs::read_to_string(&out).unwrap();
    assert!(
        json.contains("\"river_fraction\": 0.25"),
        "domain-clipped fraction:\n{json}"
    );
    assert!(
        json.contains("\"domain_clip_applied\": true"),
        "domain clip metadata:\n{json}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reads_gzipped_geojson_inputs() {
    let dir = std::env::temp_dir().join(format!("em3_xsect_gz_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let cells = r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"cell_id":"c1"},
         "geometry":{"type":"Polygon","coordinates":[[[0,0],[2,0],[2,2],[0,2],[0,0]]]}}]}"#;
    let corridors = r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"river_class":"R3"},
         "geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,2],[0,2],[0,0]]]}}]}"#;
    write_gzip(&dir.join("cells.geojson.gz"), cells);
    write_gzip(&dir.join("corridors.geojson.gz"), corridors);

    let out = dir.join("x.geojson");
    let n = write_earthmesh_intersection_geojson(
        dir.join("cells.geojson.gz"),
        dir.join("corridors.geojson.gz"),
        &out,
        &["R3".to_string()],
        0.0,
        false,
        None,
    )
    .expect("gzipped intersections");
    assert_eq!(n, 1);
    let json = std::fs::read_to_string(&out).unwrap();
    assert!(json.contains("\"river_fraction\": 0.5"), "{json}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn non_convex_domain_clips_corridor_exactly() {
    let dir = std::env::temp_dir().join(format!("em3_xsect_ncdom_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // Cell 4x4 (area 16). R3 corridor covers the whole cell [0,4]x[0,4].
    std::fs::write(
        dir.join("cells.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"cell_id":"c1"},
         "geometry":{"type":"Polygon","coordinates":[[[0,0],[4,0],[4,4],[0,4],[0,0]]]}}]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("corridors.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"river_class":"R3"},
         "geometry":{"type":"Polygon","coordinates":[[[0,0],[4,0],[4,4],[0,4],[0,0]]]}}]}"#,
    )
    .unwrap();
    // L-shaped (non-convex) domain = 4x4 minus the top-right 2x2 quadrant -> area 12.
    // corridor∩domain∩cell = the L = 12 -> fraction 12/16 = 0.75.
    let l_domain = vec![vec![
        (0.0, 0.0),
        (4.0, 0.0),
        (4.0, 2.0),
        (2.0, 2.0),
        (2.0, 4.0),
        (0.0, 4.0),
    ]];
    let out = dir.join("x.geojson");
    write_earthmesh_intersection_geojson(
        dir.join("cells.geojson"),
        dir.join("corridors.geojson"),
        &out,
        &["R3".to_string()],
        0.0,
        false,
        Some(&l_domain),
    )
    .expect("x");
    let json = std::fs::read_to_string(&out).unwrap();
    // triangulation accumulates tiny float error -> compare approximately to 12/16.
    let frac: f64 = json
        .split("\"river_fraction\": ")
        .nth(1)
        .and_then(|s| s.split([',', '}']).next())
        .and_then(|s| s.trim().parse().ok())
        .expect("river_fraction");
    assert!(
        (frac - 0.75).abs() < 1e-9,
        "L-domain fraction ~0.75, got {frac}\n{json}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn overlapping_same_class_corridors_use_union_not_sum() {
    let dir = std::env::temp_dir().join(format!("em3_xsect_union_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // Cell 4x4 (area 16). Two R3 corridors that overlap: [0,2]x[0,2] (4) and
    // [1,3]x[1,3] (4), overlap [1,2]x[1,2] (1). Union area = 4+4-1 = 7 -> fraction
    // 7/16 = 0.4375 (NOT the summed 8/16 = 0.5).
    std::fs::write(
        dir.join("cells.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"cell_id":"c1"},
         "geometry":{"type":"Polygon","coordinates":[[[0,0],[4,0],[4,4],[0,4],[0,0]]]}}]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("corridors.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"river_class":"R3"},
         "geometry":{"type":"Polygon","coordinates":[[[0,0],[2,0],[2,2],[0,2],[0,0]]]}},
        {"type":"Feature","properties":{"river_class":"R3"},
         "geometry":{"type":"Polygon","coordinates":[[[1,1],[3,1],[3,3],[1,3],[1,1]]]}}]}"#,
    )
    .unwrap();
    let out = dir.join("x.geojson");
    write_earthmesh_intersection_geojson(
        dir.join("cells.geojson"),
        dir.join("corridors.geojson"),
        &out,
        &["R3".to_string()],
        0.0,
        false,
        None,
    )
    .expect("x");
    let json = std::fs::read_to_string(&out).unwrap();
    assert!(
        json.contains("\"river_fraction\": 0.4375"),
        "expected union 7/16:\n{json}"
    );
    assert!(
        !json.contains("\"river_fraction\": 0.5"),
        "must not double-count overlap"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
