//! Rust port of util/hydro_mesh/earthmesh_intersection.py: overlay cells x corridors
//! into a per-cell intersection GeoJSON, then feed that into the colm_coupling port —
//! proving the full mesh+masks -> intersections -> coupling pipeline runs in Rust
//! (shapely-free). Pure geometry (no NetCDF data).

use earthmesh_cli::{
    hydro_delivery_colm::colm_coupling_rows_from_intersections,
    hydro_delivery_colm::write_colm_coupling_csv_from_intersections,
    hydro_delivery_intersections::write_earthmesh_intersection_geojson,
};
use flate2::{write::GzEncoder, Compression};
use std::io::Write;
use std::path::Path;

fn write_gzip(path: &Path, content: &str) {
    let file = std::fs::File::create(path).unwrap();
    let mut encoder = GzEncoder::new(file, Compression::default());
    encoder.write_all(content.as_bytes()).unwrap();
    encoder.finish().unwrap();
}

fn property_numbers(json: &str, property: &str) -> Vec<f64> {
    json.split(&format!("\"{property}\": "))
        .skip(1)
        .filter_map(|tail| {
            tail.split([',', '}'])
                .next()
                .and_then(|value| value.trim().parse().ok())
        })
        .collect()
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
    let fraction = property_numbers(&geojson, "river_fraction")[0];
    assert!((fraction - 0.5).abs() < 2.0e-4, "{geojson}");

    // Full Rust chain: feed the intersection GeoJSON into the colm_coupling port.
    let rows = colm_coupling_rows_from_intersections(&geojson, 0.0).expect("coupling");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0], "c1"); // cell_id
    assert_eq!(rows[0][2], "R3"); // river_class
    assert!((rows[0][3].parse::<f64>().unwrap() - fraction).abs() < 1.0e-12);
    let river_area_m2: f64 = rows[0][4].parse().expect("production river area");
    let cell_area_m2: f64 = rows[0][5].parse().expect("production cell area");
    assert_eq!(rows[0][9], "spherical_equal_area_m2");
    assert!(
        (river_area_m2 / cell_area_m2 - fraction).abs() < 1.0e-12,
        "CoLM physical areas must match the spherical overlap fraction: {:?}",
        rows[0]
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn intersection_geojson_escapes_string_controls() {
    let dir = std::env::temp_dir().join(format!("em3_xsect_escape_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("cells.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"cell_id":"c\"1\nx"},
         "geometry":{"type":"Polygon","coordinates":[[[0,0],[2,0],[2,2],[0,2],[0,0]]]}}
        ]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("corridors.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"river_class":"R\"3\nmain"},
         "geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,2],[0,2],[0,0]]]}}
        ]}"#,
    )
    .unwrap();

    let out = dir.join("intersections.geojson");
    let n = write_earthmesh_intersection_geojson(
        dir.join("cells.geojson"),
        dir.join("corridors.geojson"),
        &out,
        &["R\"3\nmain".to_string()],
        0.0,
        false,
        None,
    )
    .expect("intersections");
    assert_eq!(n, 1);
    let geojson = std::fs::read_to_string(&out).unwrap();
    assert!(geojson.contains(r#""cell_id": "c\"1\nx""#), "{geojson}");
    assert!(
        geojson.contains(r#""river_class": "R\"3\nmain""#),
        "{geojson}"
    );
    let rows = colm_coupling_rows_from_intersections(&geojson, 0.0).expect("valid json");
    assert_eq!(rows[0][0], "c\"1\nx");
    assert_eq!(rows[0][2], "R\"3\nmain");
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
    let fraction = property_numbers(&json, "river_fraction")[0];
    assert!(
        (fraction - 0.25).abs() < 5.0e-4,
        "spherical domain-clipped fraction near one quarter, got {fraction}:\n{json}"
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
    assert!(
        (property_numbers(&json, "river_fraction")[0] - 0.5).abs() < 2.0e-4,
        "{json}"
    );
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
    // Spherical area is latitude-weighted, so the top-right quadrant is slightly
    // smaller than one planar quarter; the retained L is correspondingly > 0.75.
    let frac: f64 = json
        .split("\"river_fraction\": ")
        .nth(1)
        .and_then(|s| s.split([',', '}']).next())
        .and_then(|s| s.trim().parse().ok())
        .expect("river_fraction");
    assert!(
        (frac - 0.75).abs() < 2.0e-3,
        "spherical L-domain fraction near 0.75, got {frac}\n{json}"
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
    let fraction = property_numbers(&json, "river_fraction")[0];
    assert!(
        (fraction - 0.4375).abs() < 2.0e-3,
        "spherical union should remain close to the small-domain 7/16 limit: {fraction}\n{json}"
    );
    assert!(fraction < 0.49, "must not double-count overlap");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn antimeridian_overlap_is_wrap_independent() {
    let dir = std::env::temp_dir().join(format!("em3_xsect_dateline_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("cells.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"cell_id":"date-line"},
         "geometry":{"type":"Polygon","coordinates":[[[179,10],[-179,10],[-179,12],[179,12],[179,10]]]}}]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("corridors.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"river_class":"R3"},
         "geometry":{"type":"Polygon","coordinates":[[[178.5,9],[180,9],[180,13],[178.5,13],[178.5,9]]]}}]}"#,
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
    .expect("antimeridian intersection");
    let json = std::fs::read_to_string(&out).unwrap();
    let fractions = property_numbers(&json, "river_fraction");
    assert_eq!(fractions.len(), 1, "{json}");
    assert!((0.0..=1.0).contains(&fractions[0]), "{json}");
    assert!(
        (fractions[0] - 0.5).abs() < 2.0e-5,
        "dateline half-cell fraction: {}\n{json}",
        fractions[0]
    );
    assert!(
        json.contains("\"overlay_method\": \"cell_local_lambert_azimuthal_equal_area\""),
        "{json}"
    );

    // The identical geometry expressed without a longitude wrap and with both
    // rings reversed must produce the same conservative result.
    std::fs::write(
        dir.join("cells_shifted.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"cell_id":"shifted"},
         "geometry":{"type":"Polygon","coordinates":[[[179,10],[179,12],[181,12],[181,10],[179,10]]]}}]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("corridors_shifted.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"river_class":"R3"},
         "geometry":{"type":"Polygon","coordinates":[[[178.5,9],[178.5,13],[180,13],[180,9],[178.5,9]]]}}]}"#,
    )
    .unwrap();
    let shifted_out = dir.join("shifted.geojson");
    write_earthmesh_intersection_geojson(
        dir.join("cells_shifted.geojson"),
        dir.join("corridors_shifted.geojson"),
        &shifted_out,
        &["R3".to_string()],
        0.0,
        false,
        None,
    )
    .expect("shifted/reversed antimeridian intersection");
    let shifted = std::fs::read_to_string(&shifted_out).unwrap();
    let shifted_fraction = property_numbers(&shifted, "river_fraction")[0];
    assert!(
        (shifted_fraction - fractions[0]).abs() < 1.0e-12,
        "longitude representation or winding changed fraction: {json}\n{shifted}"
    );
    let area = property_numbers(&json, "cell_area_sr")[0];
    let shifted_area = property_numbers(&shifted, "cell_area_sr")[0];
    assert!(
        (area - shifted_area).abs() <= area * 1.0e-12,
        "longitude representation or winding changed area: {area} vs {shifted_area}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn high_latitude_overlap_uses_equal_area_not_lonlat_area() {
    let dir = std::env::temp_dir().join(format!("em3_xsect_polar_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("cells.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"cell_id":"polar"},
         "geometry":{"type":"Polygon","coordinates":[[[-20,80],[20,80],[20,84],[-20,84],[-20,80]]]}}]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("corridors.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"river_class":"R3"},
         "geometry":{"type":"Polygon","coordinates":[[[-30,79],[0,79],[0,85],[-30,85],[-30,79]]]}}]}"#,
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
    .expect("high-latitude intersection");
    let json = std::fs::read_to_string(&out).unwrap();
    let fraction = property_numbers(&json, "river_fraction")[0];
    assert!((0.0..=1.0).contains(&fraction), "{json}");
    assert!(
        (fraction - 0.5).abs() < 2.0e-5,
        "polar half-cell fraction: {fraction}\n{json}"
    );
    let cell_area = property_numbers(&json, "cell_area_m2")[0];
    let intersection_area = property_numbers(&json, "intersection_area_m2")[0];
    assert!(cell_area.is_finite() && cell_area > 0.0, "{json}");
    assert!(
        (intersection_area / cell_area - fraction).abs() < 1.0e-12,
        "physical areas and fraction disagree: {json}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn same_class_union_is_cell_area_conservative() {
    let dir = std::env::temp_dir().join(format!("em3_xsect_conserve_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("cells.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"cell_id":"whole"},
         "geometry":{"type":"Polygon","coordinates":[[[10,50],[12,50],[12,52],[10,52],[10,50]]]}}]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("corridors.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"river_class":"R3"},
         "geometry":{"type":"Polygon","coordinates":[[[9,49],[11.2,49],[11.2,53],[9,53],[9,49]]]}},
        {"type":"Feature","properties":{"river_class":"R3"},
         "geometry":{"type":"Polygon","coordinates":[[[10.8,49],[13,49],[13,53],[10.8,53],[10.8,49]]]}}]}"#,
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
    .expect("conservative union");
    let json = std::fs::read_to_string(&out).unwrap();
    let fraction = property_numbers(&json, "river_fraction")[0];
    let cell_area = property_numbers(&json, "cell_area_sr")[0];
    let intersection_area = property_numbers(&json, "intersection_area_sr")[0];
    assert!((fraction - 1.0).abs() < 1.0e-12, "{json}");
    assert!(intersection_area <= cell_area, "{json}");
    assert!(
        (intersection_area - cell_area).abs() <= cell_area * 1.0e-12,
        "full union must conserve the cell area: {json}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn merit_and_cama_metadata_follow_real_clipped_overlap_without_double_counting() {
    let dir = std::env::temp_dir().join(format!("em3_xsect_cama_metadata_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("cells.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"cell_id":"a"},
         "geometry":{"type":"Polygon","coordinates":[[[0,0],[2,0],[2,2],[0,2],[0,0]]]}},
        {"type":"Feature","properties":{"cell_id":"b"},
         "geometry":{"type":"Polygon","coordinates":[[[3,0],[5,0],[5,2],[3,2],[3,0]]]}}
        ]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("corridors.geojson"),
        r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"river_class":"R3","source":"MERIT-Hydro"},
         "geometry":{"type":"Polygon","coordinates":[[[0,0],[1.5,0],[1.5,2],[0,2],[0,0]]]}},
        {"type":"Feature","properties":{"river_class":"R3","source":"MERIT-Hydro"},
         "geometry":{"type":"Polygon","coordinates":[[[3,0],[4,0],[4,2],[3,2],[3,0]]]}},
        {"type":"Feature","properties":{"river_class":"R3","source":"CaMa-Flood","is_estuary":true,"reach_id":"reach-estuary"},
         "geometry":{"type":"Polygon","coordinates":[[[1,0],[2,0],[2,2],[1,2],[1,0]]]}},
        {"type":"Feature","properties":{"river_class":"R3","source":"CaMa-Flood","is_estuary":true,"reach_id":"reach-far"},
         "geometry":{"type":"Polygon","coordinates":[[[10,0],[11,0],[11,2],[10,2],[10,0]]]}}
        ]}"#,
    )
    .unwrap();

    let intersections = dir.join("intersections.geojson");
    let count = write_earthmesh_intersection_geojson(
        dir.join("cells.geojson"),
        dir.join("corridors.geojson"),
        &intersections,
        &["R3".to_string()],
        0.0,
        false,
        None,
    )
    .expect("MERIT+CaMa overlay");
    assert_eq!(count, 2);
    let json = std::fs::read_to_string(&intersections).unwrap();
    let river_fractions = property_numbers(&json, "river_fraction");
    assert_eq!(river_fractions.len(), 2, "{json}");
    assert!(
        river_fractions[0] <= 1.0 && (river_fractions[0] - 1.0).abs() < 2.0e-4,
        "same-class overlap must be unioned, not summed: {json}"
    );
    let estuary_fractions = property_numbers(&json, "estuary_fraction");
    assert!((estuary_fractions[0] - 0.5).abs() < 2.0e-4, "{json}");
    assert_eq!(estuary_fractions[1], 0.0, "{json}");
    assert_eq!(json.matches("\"is_estuary\": true").count(), 1, "{json}");
    assert_eq!(json.matches("\"is_estuary\": false").count(), 1, "{json}");
    assert!(
        json.contains("\"corridor_sources\": \"CaMa-Flood;MERIT-Hydro\""),
        "{json}"
    );
    assert!(
        json.contains("\"corridor_sources\": \"MERIT-Hydro\""),
        "{json}"
    );
    assert!(json.contains("\"reach_ids\": \"reach-estuary\""), "{json}");
    assert!(!json.contains("reach-far"), "{json}");

    let rows = colm_coupling_rows_from_intersections(&json, 0.0).expect("CoLM rows");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].len(), 14);
    assert_eq!(rows[0][0], "a");
    assert_eq!(rows[0][10], "CaMa-Flood;MERIT-Hydro");
    assert_eq!(rows[0][11], "true");
    assert!((rows[0][12].parse::<f64>().unwrap() - 0.5).abs() < 2.0e-4);
    assert_eq!(rows[0][13], "reach-estuary");
    assert_eq!(rows[1][0], "b");
    assert_eq!(rows[1][10], "MERIT-Hydro");
    assert_eq!(rows[1][11], "false");
    assert_eq!(rows[1][12], "0");
    assert_eq!(rows[1][13], "");

    let coupling_csv = dir.join("colm_coupling.csv");
    write_colm_coupling_csv_from_intersections(&intersections, &coupling_csv, 0.0)
        .expect("write metadata-preserving CoLM CSV");
    let csv = std::fs::read_to_string(coupling_csv).unwrap();
    assert!(
        csv.lines()
            .next()
            .unwrap()
            .ends_with("area_normalization,corridor_sources,is_estuary,estuary_fraction,reach_ids"),
        "{csv}"
    );
    assert!(csv.contains("CaMa-Flood;MERIT-Hydro,true"), "{csv}");

    let _ = std::fs::remove_dir_all(dir);
}
