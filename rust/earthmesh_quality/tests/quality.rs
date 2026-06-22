//! Integration tests for the quality report + its output artifacts.

use earthmesh_geometry::Point;
use earthmesh_quality::{
    compute, io, QualityCell, QualityLevel, QualityMeshInput, QualityThresholds,
};

fn two_square_mesh() -> QualityMeshInput {
    QualityMeshInput {
        vertices: vec![
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(0.0, 1.0),
            Point::new(2.0, 0.0),
            Point::new(2.0, 1.0),
        ],
        cells: vec![
            QualityCell {
                vertices: vec![0, 1, 2, 3],
                refine_level: Some(0),
                neighbors: vec![1],
            },
            QualityCell {
                vertices: vec![1, 4, 5, 2],
                refine_level: Some(0),
                neighbors: vec![0],
            },
        ],
    }
}

#[test]
fn quality_json_output_has_required_fields() {
    let r = compute(&two_square_mesh(), &QualityThresholds::default());
    let json = io::to_summary_json(&r);
    for needle in [
        "earthmesh_mesh_quality",
        "\"verdict\": \"pass\"",
        "\"cell_count\": 2",
        "\"edge_count\": 7",
        "\"cell_area\"",
        "\"edge_length_km\"",
        "\"min_angle_deg\"",
        "\"duplicate_edge_count\": 0",
        "\"neighbor_reciprocity_failure_count\": 0",
        "\"gates\"",
    ] {
        assert!(json.contains(needle), "JSON missing `{needle}`:\n{json}");
    }
}

#[test]
fn quality_csv_output_has_rows_and_verdict() {
    let r = compute(&two_square_mesh(), &QualityThresholds::default());
    let csv = io::to_summary_csv(&r);
    assert!(csv.starts_with("category,metric,value,level\n"));
    assert!(csv.contains("geometry,cell_count,2"));
    assert!(csv.contains("summary,verdict,,pass"));
}

#[test]
fn worst_cells_geojson_output_for_bad_mesh() {
    // a self-intersecting (bow-tie) cell should appear as a worst cell
    let m = QualityMeshInput {
        vertices: vec![
            Point::new(0.0, 0.0),
            Point::new(2.0, 2.0),
            Point::new(2.0, 0.0),
            Point::new(0.0, 2.0),
        ],
        cells: vec![QualityCell {
            vertices: vec![0, 1, 2, 3],
            refine_level: Some(0),
            neighbors: vec![],
        }],
    };
    let r = compute(&m, &QualityThresholds::default());
    assert_eq!(r.verdict, QualityLevel::Fail);
    let geojson = io::to_worst_cells_geojson(&r);
    assert!(geojson.contains("\"type\": \"FeatureCollection\""));
    assert!(geojson.contains("earthmesh_quality_worst_cells"));
    assert!(geojson.contains("\"type\": \"Polygon\""));
    assert!(geojson.contains("self_intersection"));
}

#[test]
fn write_all_produces_four_artifacts() {
    let r = compute(&two_square_mesh(), &QualityThresholds::default());
    let dir = std::env::temp_dir().join(format!("em3_quality_test_{}", std::process::id()));
    let written = io::write_all(&r, &dir).expect("write_all");
    assert_eq!(written.len(), 4);
    for name in [
        "quality_summary.json",
        "quality_summary.csv",
        "worst_cells.geojson",
        "quality_report.md",
    ] {
        let p = dir.join(name);
        assert!(p.exists(), "missing {name}");
        assert!(!std::fs::read_to_string(&p).unwrap().is_empty());
    }
    let md = std::fs::read_to_string(dir.join("quality_report.md")).unwrap();
    assert!(md.contains("Mesh Quality Report"));
    assert!(md.contains("Verdict: PASS"));
    let _ = std::fs::remove_dir_all(&dir);
}
