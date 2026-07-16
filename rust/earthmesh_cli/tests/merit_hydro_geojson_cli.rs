use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

static NETCDF_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp root");
    path
}

fn write_merit_fixture(path: &Path) {
    write_merit_fixture_at(path, [100.0, 100.5]);
}

fn write_merit_fixture_at(path: &Path, longitude: [f64; 2]) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create MERIT tile fixture");
    file.add_dimension("longitude", 2)
        .expect("add longitude dimension");
    file.add_dimension("latitude", 2)
        .expect("add latitude dimension");
    file.add_variable::<f64>("longitude", &["longitude"])
        .expect("add longitude")
        .put_values(&longitude, ..)
        .expect("write longitude");
    file.add_variable::<f64>("latitude", &["latitude"])
        .expect("add latitude")
        .put_values(&[10.0, 10.5], ..)
        .expect("write latitude");
    file.add_variable::<i32>("dir", &["longitude", "latitude"])
        .expect("add dir")
        .put_values(&[1, 2, 3, 4], (.., ..))
        .expect("write dir");
    file.add_variable::<f64>("upa", &["longitude", "latitude"])
        .expect("add upa")
        .put_values(&[100.0, 50_000.0, 100.0, 8_000.0], (.., ..))
        .expect("write upa");
    file.add_variable::<f64>("elv", &["longitude", "latitude"])
        .expect("add elv")
        .put_values(&[1.0, 2.0, -9999.0, 4.0], (.., ..))
        .expect("write elv");
    file.add_variable::<f64>("wth", &["longitude", "latitude"])
        .expect("add wth")
        .put_values(&[10.0, 300.0, 10.0, 75.0], (.., ..))
        .expect("write wth");
    file.add_variable::<i32>("landtype_igbp", &["longitude", "latitude"])
        .expect("add landtype")
        .put_values(&[1, 1, 0, 1], (.., ..))
        .expect("write landtype");
}

#[test]
fn binary_reads_both_clipped_windows_for_a_dateline_crossing_bbox() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("merit_hydro_dateline_cli");
    let merit_root = root.join("merit");
    let output_dir = root.join("out");
    fs::create_dir_all(&merit_root).expect("create merit root");
    write_merit_fixture_at(&merit_root.join("n10e175.nc"), [179.0, 179.5]);
    write_merit_fixture_at(&merit_root.join("n10w180.nc"), [-179.5, -179.0]);

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--merit-hydro-geojson")
        .arg(&merit_root)
        .arg(&output_dir)
        .arg("--bbox")
        .args(["178.5", "10.0", "-178.5", "11.0"])
        .output()
        .expect("run dateline-crossing MERIT export");

    assert!(
        output.status.success(),
        "dateline export failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let summary =
        fs::read_to_string(output_dir.join("merit_mask_summary.json")).expect("read summary");
    assert!(summary.contains(r#""tile_count":2"#), "{summary}");
    let combined =
        fs::read_to_string(output_dir.join("merit_masks.geojson")).expect("read combined");
    assert!(combined.contains("179"), "missing east-dateline features");
    assert!(combined.contains("-179"), "missing west-dateline features");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_can_export_merit_root_to_geojson_layers() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("merit_hydro_geojson_cli");
    let merit_root = root.join("merit");
    let output_dir = root.join("out");
    fs::create_dir_all(&merit_root).expect("create merit root");
    write_merit_fixture(&merit_root.join("n10e100.nc"));
    fs::write(merit_root.join("notes.txt"), b"ignore me").expect("write ignored file");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let status = Command::new(exe)
        .arg("--merit-hydro-geojson")
        .arg(&merit_root)
        .arg(&output_dir)
        .arg("--bbox")
        .args(["100.0", "10.0", "101.0", "11.0"])
        .arg("--stride")
        .arg("1")
        .status()
        .expect("run earthmesh_cli merit hydro geojson export");

    assert!(
        status.success(),
        "earthmesh_cli should export MERIT GeoJSON layers"
    );
    let combined =
        fs::read_to_string(output_dir.join("merit_masks.geojson")).expect("read combined");
    assert!(combined.contains(r#""mask_class":"R3""#));
    assert!(combined.contains(r#""mask_class":"R2""#));
    assert!(combined.contains("COAST_OCEAN"));
    assert!(combined.contains(r#""source":"MERIT-Hydro""#));
    let river =
        fs::read_to_string(output_dir.join("merit_river_masks.geojson")).expect("read river");
    assert!(river.contains(r#""mask_class":"R3""#));
    assert!(river.contains(r#""mask_class":"R2""#));
    assert!(!river.contains("COAST_OCEAN"));
    let coast =
        fs::read_to_string(output_dir.join("merit_coast_masks.geojson")).expect("read coast");
    assert!(coast.contains("COAST_OCEAN"));
    let summary =
        fs::read_to_string(output_dir.join("merit_mask_summary.json")).expect("read summary");
    assert!(summary.contains(r#""tile_count":1"#));
    assert!(summary.contains(r#""feature_count":4"#));

    let _ = fs::remove_dir_all(root);
}
