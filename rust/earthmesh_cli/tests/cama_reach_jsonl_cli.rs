use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp root");
    path
}

fn write_f32_grid(path: &Path, rows: &[Vec<f32>]) {
    let mut bytes = Vec::new();
    for row in rows {
        for value in row {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    fs::write(path, bytes).expect("write f32 grid");
}

fn write_i32_planes(path: &Path, planes: &[Vec<Vec<i32>>]) {
    let mut bytes = Vec::new();
    for plane in planes {
        for row in plane {
            for value in row {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
    }
    fs::write(path, bytes).expect("write i32 planes");
}

fn write_small_cama_map_dir(root: &Path) -> PathBuf {
    let map_dir = root.join("map");
    fs::create_dir_all(&map_dir).expect("create map dir");
    fs::write(
        map_dir.join("params.txt"),
        "4\n3\n1\n0.5\n100.0\n102.0\n10.0\n11.5\n",
    )
    .expect("write params");
    write_f32_grid(
        &map_dir.join("uparea.bin"),
        &[
            vec![0.0, 70.0, 80.0, 90.0],
            vec![0.0, 30.0, 40.0, 60.0],
            vec![0.0, 10.0, 20.0, 35.0],
        ],
    );
    write_f32_grid(
        &map_dir.join("width.bin"),
        &[
            vec![0.0, 7.0, 8.0, 9.0],
            vec![0.0, 3.0, 4.0, 6.0],
            vec![0.0, 1.0, 2.0, 3.5],
        ],
    );
    write_f32_grid(
        &map_dir.join("rivlen.bin"),
        &[
            vec![0.0, 700.0, 800.0, 900.0],
            vec![0.0, 300.0, 400.0, 600.0],
            vec![0.0, 100.0, 200.0, 350.0],
        ],
    );
    write_i32_planes(
        &map_dir.join("nextxy.bin"),
        &[
            vec![vec![0, 1, 2, 3], vec![0, 1, 2, 3], vec![0, 1, 0, 3]],
            vec![vec![0, 3, 2, 1], vec![0, 2, 1, 3], vec![0, 1, 0, 3]],
        ],
    );
    map_dir
}

#[test]
fn binary_can_export_cama_map_dir_to_hydro_source_jsonl() {
    let root = temp_root("cama_reach_jsonl_cli");
    let map_dir = write_small_cama_map_dir(&root);
    let output = root.join("hydro_source.jsonl");
    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let status = Command::new(exe)
        .arg("--cama-reach-jsonl")
        .arg(&map_dir)
        .arg(&output)
        .arg("--bbox")
        .args(["100.5", "10.0", "101.5", "11.0"])
        .arg("--target-dx-km")
        .arg("2.5")
        .arg("--uparea-to-km2")
        .arg("1000.0")
        .status()
        .expect("run earthmesh_cli cama reach jsonl export");

    assert!(status.success(), "earthmesh_cli should export CaMa JSONL");
    let text = fs::read_to_string(&output).expect("read exported jsonl");
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 4);
    assert!(lines[0].contains(r#""reach_id":"cama-0-1""#));
    assert!(lines[0].contains(r#""upstream_area_km2":10000"#));
    assert!(lines[0].contains(r#""width_m":1"#));
    assert!(
        lines[1].contains(r#""is_estuary":true""#) || lines[1].contains(r#""is_estuary":true"#)
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_can_export_cama_map_dir_to_point_geojson() {
    let root = temp_root("cama_reach_geojson_cli");
    let map_dir = write_small_cama_map_dir(&root);
    let output = root.join("hydro_source.geojson");
    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let status = Command::new(exe)
        .arg("--cama-reach-geojson")
        .arg(&map_dir)
        .arg(&output)
        .arg("--bbox")
        .args(["100.5", "10.0", "101.5", "11.0"])
        .arg("--target-dx-km")
        .arg("2.5")
        .arg("--uparea-to-km2")
        .arg("1000.0")
        .status()
        .expect("run earthmesh_cli cama reach geojson export");

    assert!(status.success(), "earthmesh_cli should export CaMa GeoJSON");
    let text = fs::read_to_string(&output).expect("read exported geojson");
    assert!(text.starts_with("{\"type\":\"FeatureCollection\",\"features\":["));
    assert!(text.contains(r#""geometry":{"type":"Point","coordinates":[100.75,10.25]}"#));
    assert!(text.contains(r#""downstream_x":0,"downstream_y":0"#));
    assert!(text.contains(r#""reach_id":"cama-0-1""#));
    assert!(text.contains(r#""upstream_area_km2":10000"#));
    assert!(text.contains(r#""river_class":"R2""#));
    assert!(text.contains(r#""source":"cama_reach_inventory""#));

    let _ = fs::remove_dir_all(root);
}
