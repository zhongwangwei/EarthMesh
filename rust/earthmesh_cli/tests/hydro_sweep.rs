//! Rust port of util/hydro_mesh/refinement_sweep.py: recipe sweep + report ranking.
//! Pure JSON (no NetCDF data).

use earthmesh_cli::{write_sweep_ranking, write_sweep_recipes};
use std::path::PathBuf;

fn report(case: &str, status: &str, retained3: i64, cells: i64) -> String {
    format!(
        "{{\"case_name\":\"{case}\",\"status\":\"{status}\",\
\"background_cells\":{{\"cell_count\":{cells},\"equivalent_cell_size_km_median\":2.0}},\
\"river_intersections\":{{\"feature_count\":5}},\
\"coast_intersections\":{{\"feature_count\":3}},\
\"refinement_log\":{{\"3\":{{\"retained_triangles\":{retained3}}}}}}}"
    )
}

#[test]
fn ranks_candidates_by_retained_triangles() {
    let dir = std::env::temp_dir().join(format!("em3_sweep_rank_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut paths = Vec::new();
    for (case, status, ret3) in [
        ("caseA", "pass", 10),
        ("caseB", "pass", 20),
        ("caseC", "fail", 99),
    ] {
        let p = dir.join(format!("{case}.json"));
        std::fs::write(&p, report(case, status, ret3, 100)).unwrap();
        paths.push(p);
    }
    let out = dir.join("ranking.json");
    let recommended = write_sweep_ranking(&paths, &out, None).expect("rank");

    // Higher retained_triangles[3] ranks first; failed case is last; B is recommended.
    assert_eq!(recommended, "caseB");
    let json = std::fs::read_to_string(&out).unwrap();
    let pos = |needle: &str| json.find(needle).unwrap_or(usize::MAX);
    assert!(pos("\"case_name\": \"caseB\"") < pos("\"case_name\": \"caseA\""));
    assert!(pos("\"case_name\": \"caseA\"") < pos("\"case_name\": \"caseC\""));
    assert!(json.contains("\"recommended_case\": \"caseB\""));
    assert!(json.contains("\"promotion_status\": \"failed\""));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn max_background_cells_blocks_promotion() {
    let dir = std::env::temp_dir().join(format!("em3_sweep_block_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // big cell count exceeds the cap -> blocked, not candidate.
    let p = dir.join("big.json");
    std::fs::write(&p, report("big", "pass", 10, 1000)).unwrap();
    let out = dir.join("r.json");
    let recommended = write_sweep_ranking(&[p], &out, Some(64)).expect("rank");
    assert_eq!(recommended, ""); // no candidate
    let json = std::fs::read_to_string(&out).unwrap();
    assert!(json.contains("\"promotion_status\": \"blocked_background_cell_cap\""));
    assert!(json.contains("\"recommended_case\": null"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn writes_sweep_recipes_and_manifest() {
    let dir = std::env::temp_dir().join(format!("em3_sweep_recipes_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let count = write_sweep_recipes(
        &dir,
        "rivers.geojson",
        "coast.geojson",
        vec![40],
        vec![10],
        19,
    )
    .expect("recipes");
    assert_eq!(count, 1);
    let recipe = std::fs::read_to_string(dir.join("r2cap40_coast10_recipe.json")).unwrap();
    assert!(recipe.contains("\"R2\": 40"));
    assert!(recipe.contains("\"COAST\": 10"));
    assert!(recipe.contains("\"R3\": 19"));
    let manifest = std::fs::read_to_string(dir.join("sweep_manifest.json")).unwrap();
    assert!(manifest.contains("\"case_count\": 1"));
    assert!(manifest.contains("earthmesh_refinement_sweep_manifest"));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = PathBuf::new();
}
