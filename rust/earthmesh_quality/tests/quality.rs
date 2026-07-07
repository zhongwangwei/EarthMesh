//! Integration tests for the quality report + its output artifacts.

use earthmesh_geometry::Point;
use earthmesh_quality::{
    attach_hfield_diagnostics, compute, io, HfieldConfigDiagnostics, QualityCell, QualityLevel,
    QualityMeshInput, QualityThresholds,
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
        "\"cell_area_ratio\"",
        "\"edge_length_km\"",
        "\"cell_edge_length_cv\"",
        "\"min_angle_deg\"",
        "\"angle_deviation_deg\"",
        "\"triangle_eta\"",
        "\"triangle_nsr\"",
        "\"duplicate_edge_count\": 0",
        "\"boundary_edge_count\": 6",
        "\"misoriented_shared_edge_count\": 0",
        "\"neighbor_degree_mismatch_count\": 0",
        "\"neighbor_reciprocity_failure_count\": 0",
        "\"quadrilateral_cell_count\": 2",
        "\"hexagon_cell_count\": 0",
        "\"refine_level_groups\"",
        "\"refine_level\":0",
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
    assert!(csv.contains("geometry,triangle_eta_min,0"));
    assert!(csv.contains("topology,quadrilateral_cell_count,2"));
    assert!(csv.contains("refine_level,0:cell_count,2"));
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
fn quality_reports_triangle_eta_nsr_and_refine_groups() {
    let h = 3.0_f64.sqrt() / 2.0;
    let r = compute(
        &QualityMeshInput {
            vertices: vec![
                Point::new(0.0, 0.0),
                Point::new(1.0, 0.0),
                Point::new(0.5, h),
            ],
            cells: vec![QualityCell {
                vertices: vec![0, 1, 2],
                refine_level: Some(2),
                neighbors: vec![],
            }],
        },
        &QualityThresholds::default(),
    );

    assert!((r.geometry.triangle_eta.min - 1.0).abs() < 1e-12);
    assert!((r.geometry.triangle_nsr.min - 1.0).abs() < 1e-12);
    assert_eq!(r.refine_level_groups.len(), 1);
    assert_eq!(r.refine_level_groups[0].refine_level, Some(2));
    assert_eq!(r.refine_level_groups[0].cell_count, 1);
}

#[test]
fn hfield_diagnostics_report_target_actual_mismatch_and_jumps() {
    let mut mesh = two_square_mesh();
    mesh.cells[0].refine_level = Some(1);
    mesh.cells[1].refine_level = Some(0);

    let mut r = compute(&mesh, &QualityThresholds::default());
    attach_hfield_diagnostics(
        &mut r,
        &mesh,
        &[2, 0],
        HfieldConfigDiagnostics {
            enabled: true,
            g: Some(0.2),
            max_level: Some(2),
            base_m: Some(100_000.0),
        },
    );

    let hfield = r.hfield.as_ref().expect("hfield diagnostics");
    assert_eq!(hfield.target_actual_mismatch_count, 1);
    assert_eq!(hfield.target_above_actual_count, 1);
    assert_eq!(hfield.target_level_jump_gt_one_count, 1);
    assert_eq!(hfield.actual_level_jump_gt_one_count, 0);
    assert_eq!(hfield.target_level_distribution[0].level, 0);
    assert_eq!(hfield.target_level_distribution[0].count, 1);
    assert_eq!(hfield.target_level_distribution[1].level, 2);
    assert_eq!(hfield.target_level_distribution[1].count, 1);
    assert_eq!(hfield.actual_refine_level_distribution[0].level, 0);
    assert_eq!(hfield.actual_refine_level_distribution[0].count, 1);
    assert_eq!(hfield.actual_refine_level_distribution[1].level, 1);
    assert_eq!(hfield.actual_refine_level_distribution[1].count, 1);
    assert_eq!(r.verdict, QualityLevel::Warn);
    assert!(r.gates.iter().any(|gate| {
        gate.metric == "hfield_target_actual_mismatch_count" && gate.level == QualityLevel::Warn
    }));

    let json = io::to_summary_json(&r);
    assert!(json.contains("\"hfield\""));
    assert!(json.contains("\"target_actual_mismatch_count\":1"));
    assert!(json.contains("\"target_level_jump_gt_one_count\":1"));

    let csv = io::to_summary_csv(&r);
    assert!(csv.contains("hfield,target_actual_mismatch_count,1"));
    assert!(csv.contains("hfield,target_level_2_count,1"));
    assert!(csv.contains("hfield,actual_refine_level_1_count,1"));

    let md = io::to_report_md(&r);
    assert!(md.contains("H-field diagnostics"));
    assert!(md.contains("target/actual mismatch: 1"));
}

#[test]
fn quality_reports_skew_shape_metrics() {
    let vertices = vec![
        Point::new(0.0, 0.0),
        Point::new(4.0, 0.0),
        Point::new(4.1, 0.5),
        Point::new(2.0, 2.0),
        Point::new(0.0, 2.0),
        Point::new(-0.1, 1.0),
    ];
    let r = compute(
        &QualityMeshInput {
            vertices,
            cells: vec![QualityCell {
                vertices: vec![0, 1, 2, 3, 4, 5],
                refine_level: Some(0),
                neighbors: vec![],
            }],
        },
        &QualityThresholds::default(),
    );

    assert!(r.geometry.cell_edge_length_cv.max > 0.35);
    assert!(r.geometry.angle_deviation_deg.max > 35.0);
    assert!(r
        .gates
        .iter()
        .any(|g| { g.metric == "cell_edge_length_cv_max" && g.level == QualityLevel::Warn }));
    assert!(r
        .gates
        .iter()
        .any(|g| { g.metric == "angle_deviation_deg_max" && g.level == QualityLevel::Warn }));
}

#[test]
fn misoriented_shared_edge_is_reported() {
    let mut m = two_square_mesh();
    // Same geometric square as cell 1, but wound the wrong way so the shared
    // edge (1,2) is traversed in the same direction as cell 0.
    m.cells[1].vertices = vec![2, 5, 4, 1];
    let r = compute(&m, &QualityThresholds::default());

    assert_eq!(r.topology.misoriented_shared_edge_count, 1);
    assert!(r
        .gates
        .iter()
        .any(|g| { g.metric == "misoriented_shared_edge_count" && g.level == QualityLevel::Fail }));
    assert!(r
        .topology_issues
        .iter()
        .any(|i| i.issue_type.as_str() == "misoriented_shared_edge"));
}

#[test]
fn closed_cell_neighbor_mismatch_is_reported() {
    // Four triangles form a closed topological tetrahedron surface; with empty
    // neighbor lists every cell disagrees with edge-derived adjacency.
    let m = QualityMeshInput {
        vertices: vec![
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(0.0, 1.0),
            Point::new(1.0, 1.0),
        ],
        cells: vec![
            QualityCell {
                vertices: vec![0, 1, 2],
                refine_level: Some(0),
                neighbors: vec![],
            },
            QualityCell {
                vertices: vec![0, 3, 1],
                refine_level: Some(0),
                neighbors: vec![],
            },
            QualityCell {
                vertices: vec![1, 3, 2],
                refine_level: Some(0),
                neighbors: vec![],
            },
            QualityCell {
                vertices: vec![2, 3, 0],
                refine_level: Some(0),
                neighbors: vec![],
            },
        ],
    };
    let r = compute(&m, &QualityThresholds::default());

    assert_eq!(r.topology.neighbor_degree_mismatch_count, 4);
    assert!(r.gates.iter().any(|g| {
        g.metric == "neighbor_degree_mismatch_count" && g.level == QualityLevel::Fail
    }));
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
    assert!(md.contains("Refine-level groups"));
    assert!(md.contains("Verdict: PASS"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn report_md_omits_empty_cell_view() {
    let r = compute(&two_square_mesh(), &QualityThresholds::default());
    assert!(!io::to_report_md(&r).contains("- cell view: ``"));
}
