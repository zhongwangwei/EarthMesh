//! coastal_band.py dissolve=True path: merge band grid cells into a MultiPolygon
//! GeoJSON via exact axis-aligned-box union. Pure geometry (no CaMa data).

use earthmesh_cli::write_coastal_band_dissolve_geojson;

#[test]
fn dissolves_2x2_band_into_single_polygon() {
    let dir = std::env::temp_dir().join(format!("em3_cbd_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let band = vec![vec![true, true], vec![true, true]];
    let out = dir.join("d.geojson");
    let polys =
        write_coastal_band_dissolve_geojson(&band, 0, 0, 100.0, 20.0, 1.0, &out).expect("d");
    assert_eq!(polys, 1, "2x2 block -> one polygon");
    let json = std::fs::read_to_string(&out).unwrap();
    assert!(json.contains("\"type\": \"MultiPolygon\""));
    assert!(json.contains("\"coastal_band_cell_count\": 4"));
    assert!(json.contains("\"mask_class\": \"COAST\""));
    // corners of the merged 2x2 square (100..102, 20..22)
    assert!(
        json.contains("[100, 20]") || json.contains("[100.0, 20.0]"),
        "{json}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn dissolves_donut_with_hole() {
    let dir = std::env::temp_dir().join(format!("em3_cbd_donut_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // 3x3 band with the center cell unselected -> outer ring + one hole.
    let band = vec![
        vec![true, true, true],
        vec![true, false, true],
        vec![true, true, true],
    ];
    let out = dir.join("d.geojson");
    let polys = write_coastal_band_dissolve_geojson(&band, 0, 0, 0.0, 0.0, 1.0, &out).expect("d");
    assert_eq!(polys, 1, "donut -> one outer polygon (with a hole)");
    let json = std::fs::read_to_string(&out).unwrap();
    assert!(json.contains("\"coastal_band_cell_count\": 8"));
    // MultiPolygon with one polygon that has 2 rings (outer + hole) => "]], [[" absent,
    // but two ring arrays inside one polygon. Just sanity-check the hole vertex (1,1).
    assert!(
        json.contains("[1, 1]") || json.contains("[1.0, 1.0]"),
        "hole ring present:\n{json}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
