//! End-to-end port of coastal_band.py::write_coastal_band_geojson: a synthetic CaMa
//! map dir (params.txt + elevtn.bin) -> land mask -> coastal band -> dissolved GeoJSON.
//! Tiny synthetic binary; no external CaMa data.

use earthmesh_cli::write_coastal_band_geojson_from_cama;

#[test]
fn cama_elevtn_to_dissolved_coastal_band() {
    let dir = std::env::temp_dir().join(format!("em3_cb_e2e_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    // 4x4 grid, 1deg cells, origin (100E,20N). params.txt records:
    // nx ny nflp gsize west east south north
    std::fs::write(
        dir.join("params.txt"),
        "4\n4\n0\n1.0\n100.0\n104.0\n20.0\n24.0\n",
    )
    .unwrap();

    // elevtn.bin: float32 LE, logical row order (y=0 south first; we pass --no-yrev so
    // storage == logical). Left two columns land (10.0), right two ocean (undef -9999).
    let mut bytes = Vec::new();
    for _y in 0..4 {
        for x in 0..4 {
            let v: f32 = if x < 2 { 10.0 } else { -9999.0 };
            bytes.extend_from_slice(&v.to_le_bytes());
        }
    }
    std::fs::write(dir.join("elevtn.bin"), &bytes).unwrap();

    let out = dir.join("band.geojson");
    let polys = write_coastal_band_geojson_from_cama(
        &dir, &out, 100.0, // west
        20.0,  // south
        104.0, // east
        24.0,  // north
        1,     // radius_cells
        false, // y_reversed (synthetic storage == logical)
        true,  // dissolve
        -9999.0,
    )
    .expect("coastal band e2e");

    // Band = the two columns straddling the land/ocean boundary (x=1,2) over 4 rows
    // = 8 cells -> dissolves to one rectangle [101,103] x [20,24].
    assert_eq!(polys, 1, "dissolves to one polygon");
    let json = std::fs::read_to_string(&out).unwrap();
    assert!(json.contains("\"type\": \"MultiPolygon\""));
    assert!(json.contains("\"coastal_band_cell_count\": 8"), "{json}");
    assert!(json.contains("\"mask_class\": \"COAST\""));
    // merged rectangle corner at lon 101, lat 20
    assert!(
        json.contains("[101, 20]") || json.contains("[101.0, 20.0]"),
        "{json}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cama_elevtn_to_per_cell_coastal_band_no_dissolve() {
    let dir = std::env::temp_dir().join(format!("em3_cb_e2e_nd_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("params.txt"),
        "4\n4\n0\n1.0\n100.0\n104.0\n20.0\n24.0\n",
    )
    .unwrap();
    let mut bytes = Vec::new();
    for _y in 0..4 {
        for x in 0..4 {
            let v: f32 = if x < 2 { 10.0 } else { -9999.0 };
            bytes.extend_from_slice(&v.to_le_bytes());
        }
    }
    std::fs::write(dir.join("elevtn.bin"), &bytes).unwrap();

    let out = dir.join("band_cells.geojson");
    let cells = write_coastal_band_geojson_from_cama(
        &dir, &out, 100.0, 20.0, 104.0, 24.0, 1, false, false, -9999.0,
    )
    .expect("coastal band e2e no-dissolve");
    assert_eq!(cells, 8, "8 per-cell features");
    let json = std::fs::read_to_string(&out).unwrap();
    assert!(json.contains("\"coastal_side\": \"land\""));
    assert!(json.contains("\"coastal_side\": \"ocean\""));
    let _ = std::fs::remove_dir_all(&dir);
}
