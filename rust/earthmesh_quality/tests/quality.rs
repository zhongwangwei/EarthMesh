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
    let mut r = compute(&two_square_mesh(), &QualityThresholds::default());
    r.cell_view = "hex".to_string();
    let json = io::to_summary_json(&r);
    for needle in [
        "earthmesh_mesh_quality",
        "\"cell_view\": \"hex\"",
        "\"verdict\": \"pass\"",
        "\"cell_count\": 2",
        "\"edge_count\": 7",
        "\"cell_area\"",
        "\"edge_length_km\"",
        "\"min_angle_deg\"",
        "\"duplicate_edge_count\": 0",
        "\"neighbor_reciprocity_failure_count\": 0",
        "\"quadrilateral_cell_count\": 2",
        "\"hexagon_cell_count\": 0",
        "\"gates\"",
    ] {
        assert!(json.contains(needle), "JSON missing `{needle}`:\n{json}");
    }
}

#[test]
fn quality_csv_output_has_rows_and_verdict() {
    let mut r = compute(&two_square_mesh(), &QualityThresholds::default());
    r.cell_view = "tri".to_string();
    let csv = io::to_summary_csv(&r);
    assert!(csv.starts_with("category,metric,value,level\n"));
    assert!(csv.contains("geometry,cell_count,2"));
    assert!(csv.contains("topology,quadrilateral_cell_count,2"));
    assert!(csv.contains("summary,cell_view,,tri"));
    assert!(csv.contains("summary,verdict,,pass"));
}

fn regular_cell(
    vertices: &mut Vec<Point>,
    sides: usize,
    center_x: f64,
    center_y: f64,
) -> QualityCell {
    let start = vertices.len();
    let radius = 0.1;
    for k in 0..sides {
        let a = 2.0 * std::f64::consts::PI * k as f64 / sides as f64;
        vertices.push(Point::new(
            center_x + radius * a.cos(),
            center_y + radius * a.sin(),
        ));
    }
    QualityCell {
        vertices: (start..start + sides).collect(),
        refine_level: Some(0),
        neighbors: vec![],
    }
}

#[test]
fn quality_reports_cell_side_counts() {
    let mut vertices = Vec::new();
    let cells = vec![
        regular_cell(&mut vertices, 3, 0.0, 0.0),
        regular_cell(&mut vertices, 4, 1.0, 0.0),
        regular_cell(&mut vertices, 5, 2.0, 0.0),
        regular_cell(&mut vertices, 6, 3.0, 0.0),
        regular_cell(&mut vertices, 7, 4.0, 0.0),
        regular_cell(&mut vertices, 8, 5.0, 0.0),
    ];
    let r = compute(
        &QualityMeshInput { vertices, cells },
        &QualityThresholds::default(),
    );

    assert_eq!(r.topology.triangle_cell_count, 1);
    assert_eq!(r.topology.quadrilateral_cell_count, 1);
    assert_eq!(r.topology.pentagon_cell_count, 1);
    assert_eq!(r.topology.hexagon_cell_count, 1);
    assert_eq!(r.topology.heptagon_cell_count, 1);
    assert_eq!(r.topology.other_polygon_cell_count, 1);
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
    let mut r = compute(&two_square_mesh(), &QualityThresholds::default());
    r.cell_view = "tri".to_string();
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
    assert!(md.contains("- cell view: `tri`"));
    assert!(md.contains("cell-side counts are informational"));
    assert!(md.contains("Verdict: PASS"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn report_md_omits_empty_cell_view() {
    let r = compute(&two_square_mesh(), &QualityThresholds::default());
    assert!(!io::to_report_md(&r).contains("- cell view: ``"));
}
