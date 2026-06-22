//! Rust port of util/hydro_mesh/qa_gates.py: delivery-package QA gates. Pure JSON
//! evaluation (no NetCDF data); writes a synthetic manifest + complete-mask geojson.

use earthmesh_cli::evaluate_hydro_mesh_qa;
use std::path::PathBuf;

fn setup(background: i64, river: i64, coast: i64, surfaces: &[&str]) -> (PathBuf, PathBuf) {
    // Unique per (background, river, coast, surfaces) so parallel tests don't collide.
    let dir = std::env::temp_dir().join(format!(
        "em3_qa_{}_{background}_{river}_{coast}_{}",
        std::process::id(),
        surfaces.join("_")
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("mkdir");
    let mask = dir.join("complete_cell_mask.geojson");
    let features: Vec<String> = surfaces
        .iter()
        .map(|s| {
            format!(
                "{{\"type\":\"Feature\",\"geometry\":null,\"properties\":{{\"surface_class\":\"{s}\"}}}}"
            )
        })
        .collect();
    std::fs::write(
        &mask,
        format!(
            "{{\"type\":\"FeatureCollection\",\"features\":[{}]}}",
            features.join(",")
        ),
    )
    .expect("write mask");
    let manifest = dir.join("delivery_manifest.json");
    std::fs::write(
        &manifest,
        format!(
            "{{\"metrics\":{{\"background_cell_count\":{background},\"river_overlap_cells\":{river},\"coast_overlap_cells\":{coast}}},\"files\":{{\"complete_cell_mask_geojson\":\"{}\"}}}}",
            mask.display()
        ),
    )
    .expect("write manifest");
    (manifest, dir)
}

fn check_status<'a>(report: &'a earthmesh_cli::HydroMeshQaReport, id: &str) -> Option<bool> {
    report.checks.iter().find(|c| c.id == id).map(|c| c.passed)
}

#[test]
fn all_gates_pass_for_complete_package() {
    let (manifest, dir) = setup(2, 1, 1, &["LAND", "OCEAN"]);
    let report = evaluate_hydro_mesh_qa(&manifest, None, 1, 1).expect("eval");
    assert_eq!(report.status, "pass", "checks: {:?}", report.checks);
    assert_eq!(check_status(&report, "land_ocean_both_present"), Some(true));
    assert_eq!(
        check_status(&report, "complete_mask_cell_count_matches_background"),
        Some(true)
    );
    assert_eq!(report.background_cell_count, 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unknown_surface_fails_gate() {
    let (manifest, dir) = setup(2, 1, 1, &["LAND", "URBAN"]); // URBAN is not LAND/OCEAN
    let report = evaluate_hydro_mesh_qa(&manifest, None, 1, 1).expect("eval");
    assert_eq!(report.status, "fail");
    assert_eq!(check_status(&report, "surface_classes_known"), Some(false));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn insufficient_river_cells_fails_gate() {
    let (manifest, dir) = setup(2, 0, 1, &["LAND", "OCEAN"]);
    let report = evaluate_hydro_mesh_qa(&manifest, None, 1, 1).expect("eval");
    assert_eq!(report.status, "fail");
    assert_eq!(check_status(&report, "river_cells_present"), Some(false));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cell_count_mismatch_fails_gate() {
    let (manifest, dir) = setup(5, 1, 1, &["LAND", "OCEAN"]); // background 5 != 2 features
    let report = evaluate_hydro_mesh_qa(&manifest, None, 1, 1).expect("eval");
    assert_eq!(report.status, "fail");
    assert_eq!(
        check_status(&report, "complete_mask_cell_count_matches_background"),
        Some(false)
    );
    let _ = std::fs::remove_dir_all(&dir);
}
