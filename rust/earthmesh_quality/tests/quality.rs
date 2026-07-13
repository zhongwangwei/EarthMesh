//! Integration tests for the quality report + its output artifacts.

use earthmesh_geometry::Point;
use earthmesh_quality::{
    attach_hfield_diagnostics, compute, compute_with_options, io, HfieldConfigDiagnostics,
    QualityCell, QualityComputationOptions, QualityLevel, QualityMeshInput, QualityThresholds,
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
        "\"triangle_eta_local\"",
        "\"triangle_nsr_local\"",
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
    assert!(csv.contains("geometry,triangle_eta_local_min,null"));
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
fn coarse_spherical_cells_keep_exact_angles_and_compactness() {
    let r = compute(
        &QualityMeshInput {
            vertices: vec![
                Point::new(0.0, 0.0),
                Point::new(90.0, 0.0),
                Point::new(0.0, 90.0),
            ],
            cells: vec![QualityCell {
                vertices: vec![0, 1, 2],
                refine_level: Some(2),
                neighbors: vec![],
            }],
        },
        &QualityThresholds::default(),
    );

    assert_eq!(r.geometry.local_shape_metric_sample_count, 0);
    assert_eq!(r.geometry.local_shape_metric_excluded_cell_count, 1);
    assert!((r.geometry.min_angle_deg - 90.0).abs() < 1.0e-12);
    assert!((r.geometry.max_angle_deg - 90.0).abs() < 1.0e-12);
    assert!(r.geometry.angle_deviation_deg.max < 1.0e-12);
    assert!(r.geometry.triangle_eta.min.is_nan());
    assert!((r.geometry.compactness.min - 7.0 / 9.0).abs() < 1.0e-12);
    let min_angle_gate = r
        .gates
        .iter()
        .find(|gate| gate.metric == "min_angle_deg")
        .expect("min-angle gate");
    assert_eq!(min_angle_gate.level, QualityLevel::Pass);
    assert!((min_angle_gate.value - 90.0).abs() < 1.0e-12);
    let json = io::to_summary_json(&r);
    assert!(json.contains("\"min_angle_deg\": 90"), "{json}");
    assert!(json.contains("\"local_shape_metric_sample_count\": 0"));
    let markdown = io::to_report_md(&r);
    assert!(markdown.contains("min angle: 90.00°"), "{markdown}");
    assert!(markdown.contains("local shape metric samples: 0"));
    assert_eq!(r.refine_level_groups.len(), 1);
    assert_eq!(r.refine_level_groups[0].refine_level, Some(2));
    assert_eq!(r.refine_level_groups[0].cell_count, 1);
}

#[test]
fn quality_reports_reflex_angle_for_local_concave_polygon() {
    let r = compute(
        &QualityMeshInput {
            vertices: vec![
                Point::new(0.0, 0.0),
                Point::new(2.0, 0.0),
                Point::new(1.0, 0.5),
                Point::new(2.0, 1.0),
                Point::new(0.0, 1.0),
            ],
            cells: vec![QualityCell {
                vertices: vec![0, 1, 2, 3, 4],
                refine_level: Some(0),
                neighbors: vec![],
            }],
        },
        &QualityThresholds::default(),
    );

    assert!(r.geometry.max_angle_deg > 180.0);
    assert_eq!(r.geometry.local_shape_metric_sample_count, 1);
    assert_eq!(r.geometry.local_shape_metric_excluded_cell_count, 0);
}

#[test]
fn topology_reports_euler_components_and_non_manifold_vertex_fan() {
    let mesh = QualityMeshInput {
        vertices: vec![
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(0.0, 1.0),
            Point::new(-1.0, 0.0),
            Point::new(0.0, -1.0),
        ],
        cells: vec![
            QualityCell {
                vertices: vec![0, 1, 2],
                refine_level: Some(0),
                neighbors: vec![],
            },
            QualityCell {
                vertices: vec![0, 3, 4],
                refine_level: Some(0),
                neighbors: vec![],
            },
        ],
    };

    let r = compute(&mesh, &QualityThresholds::default());
    assert_eq!(r.topology.euler_characteristic, 1);
    assert_eq!(r.topology.connected_component_count, 2);
    assert_eq!(r.topology.non_manifold_vertex_fan_count, 1);
    assert!(r
        .topology_issues
        .iter()
        .any(|issue| issue.issue_type.as_str() == "disconnected_mesh"));
    assert!(r
        .topology_issues
        .iter()
        .any(|issue| issue.issue_type.as_str() == "non_manifold_vertex_fan"));
}

#[test]
fn expected_euler_characteristic_is_opt_in_and_enforced() {
    let mesh = two_square_mesh(); // regional disk: V-E+F = 6-7+2 = 1
    let default_report = compute(&mesh, &QualityThresholds::default());
    assert!(default_report
        .gates
        .iter()
        .all(|gate| gate.metric != "euler_characteristic"));
    assert_eq!(default_report.topology.expected_euler_characteristic, None);

    let matching = compute_with_options(
        &mesh,
        &QualityThresholds::default(),
        QualityComputationOptions {
            expected_euler_characteristic: Some(1),
        },
    );
    assert_eq!(matching.topology.expected_euler_characteristic, Some(1));
    assert_eq!(matching.topology.euler_characteristic_mismatch_count, 0);
    assert!(matching
        .gates
        .iter()
        .any(|gate| { gate.metric == "euler_characteristic" && gate.level == QualityLevel::Pass }));

    let mismatching = compute_with_options(
        &mesh,
        &QualityThresholds::default(),
        QualityComputationOptions {
            expected_euler_characteristic: Some(2),
        },
    );
    assert_eq!(mismatching.topology.euler_characteristic_mismatch_count, 1);
    assert!(mismatching
        .gates
        .iter()
        .any(|gate| { gate.metric == "euler_characteristic" && gate.level == QualityLevel::Fail }));
    assert_eq!(mismatching.verdict, QualityLevel::Fail);
}

#[test]
fn polar_winding_uses_signed_spherical_orientation() {
    let mut ring = vec![
        Point::new(-120.0, 80.0),
        Point::new(0.0, 80.0),
        Point::new(120.0, 80.0),
    ];
    if earthmesh_geometry::signed_spherical_polygon_excess(&ring) < 0.0 {
        ring.reverse();
    }
    let mesh_for = |vertices: Vec<Point>| QualityMeshInput {
        vertices,
        cells: vec![QualityCell {
            vertices: vec![0, 1, 2],
            refine_level: Some(0),
            neighbors: vec![],
        }],
    };
    let ccw = compute(&mesh_for(ring.clone()), &QualityThresholds::default());
    ring.reverse();
    let cw = compute(&mesh_for(ring), &QualityThresholds::default());

    assert_eq!(ccw.geometry.negative_area_cell_count, 0);
    assert_eq!(ccw.geometry.invalid_polygon_count, 0);
    assert_eq!(ccw.geometry.zero_area_cell_count, 0);
    assert_eq!(cw.geometry.negative_area_cell_count, 1);
    assert_eq!(cw.geometry.invalid_polygon_count, 0);
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
        gate.metric == "hfield_target_above_actual_count" && gate.level == QualityLevel::Warn
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
fn conforming_over_refinement_is_diagnostic_not_warning() {
    let mut mesh = two_square_mesh();
    mesh.cells[0].refine_level = Some(1);
    mesh.cells[1].refine_level = Some(0);
    let mut report = compute(&mesh, &QualityThresholds::default());
    let base_verdict = report.verdict;

    attach_hfield_diagnostics(
        &mut report,
        &mesh,
        &[0, 0],
        HfieldConfigDiagnostics {
            enabled: true,
            g: Some(0.1),
            max_level: Some(1),
            base_m: Some(100_000.0),
        },
    );

    let hfield = report.hfield.as_ref().unwrap();
    assert_eq!(hfield.target_actual_mismatch_count, 1);
    assert_eq!(hfield.target_above_actual_count, 0);
    assert_eq!(hfield.actual_above_target_count, 1);
    assert_eq!(report.verdict, base_verdict);
    assert!(report
        .gates
        .iter()
        .filter(|gate| gate.metric.starts_with("hfield_"))
        .all(|gate| gate.level == QualityLevel::Pass));
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
fn edge_cv_warning_populates_worst_cells() {
    let vertices = vec![
        Point::new(0.0, 0.0),
        Point::new(4.0, 0.0),
        Point::new(4.1, 0.5),
        Point::new(2.0, 2.0),
        Point::new(0.0, 2.0),
        Point::new(-0.1, 1.0),
    ];
    let thresholds = QualityThresholds {
        min_angle_warn_deg: 0.0,
        aspect_ratio_warn: f64::INFINITY,
        angle_deviation_warn_deg: 180.0,
        ..QualityThresholds::default()
    };
    let report = compute(
        &QualityMeshInput {
            vertices,
            cells: vec![QualityCell {
                vertices: vec![0, 1, 2, 3, 4, 5],
                refine_level: Some(0),
                neighbors: vec![],
            }],
        },
        &thresholds,
    );

    assert_eq!(report.worst_cells.len(), 1);
    assert_eq!(report.worst_cells[0].metric, "cell_edge_length_cv");
    assert!(report.worst_cells[0].value > thresholds.cell_edge_cv_warn);
    let cells = io::to_quality_repair_cells_geojson(&report);
    let plan = io::to_quality_repair_plan_json(&report);
    assert!(cells.contains("earthmesh_quality_repair_cells"));
    assert!(cells.contains("\"cell_id\": \"0\""));
    assert!(plan.contains("earthmesh_refinement_plan"));
    assert!(plan.contains("\"target_level\": 1"));
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
    assert!(io::to_quality_repair_cells_geojson(&r).contains("\"features\": [\n  ]"));
    assert!(io::to_quality_repair_plan_json(&r).contains("\"total_cells\": 0"));
}

#[test]
fn invalid_worst_cells_do_not_hide_repairable_quality_targets() {
    let mesh = QualityMeshInput {
        vertices: vec![
            Point::new(0.0, 0.0),
            Point::new(2.0, 2.0),
            Point::new(2.0, 0.0),
            Point::new(0.0, 2.0),
            Point::new(10.0, 0.0),
            Point::new(14.0, 0.0),
            Point::new(14.1, 0.5),
            Point::new(12.0, 2.0),
            Point::new(10.0, 2.0),
            Point::new(9.9, 1.0),
        ],
        cells: vec![
            QualityCell {
                vertices: vec![0, 1, 2, 3],
                refine_level: Some(0),
                neighbors: vec![],
            },
            QualityCell {
                vertices: vec![4, 5, 6, 7, 8, 9],
                refine_level: Some(0),
                neighbors: vec![],
            },
        ],
    };
    let report = compute(
        &mesh,
        &QualityThresholds {
            min_angle_warn_deg: 0.0,
            aspect_ratio_warn: f64::INFINITY,
            angle_deviation_warn_deg: 180.0,
            worst_cells_limit: 1,
            ..QualityThresholds::default()
        },
    );

    assert_eq!(report.worst_cells.len(), 1);
    assert_eq!(report.worst_cells[0].metric, "self_intersection");
    assert_eq!(report.repair_cells.len(), 1);
    assert_eq!(report.repair_cells[0].metric, "cell_edge_length_cv");
    assert!(io::to_quality_repair_plan_json(&report).contains("\"total_cells\": 1"));
}

#[test]
fn quality_repair_plan_requests_one_level_above_the_measured_cell() {
    let input = QualityMeshInput {
        vertices: vec![
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(4.0, 0.1),
            Point::new(0.0, 1.0),
        ],
        cells: vec![QualityCell {
            vertices: vec![0, 1, 2, 3],
            refine_level: Some(2),
            neighbors: Vec::new(),
        }],
    };
    let report = compute(&input, &QualityThresholds::default());
    let plan = io::to_quality_repair_plan_json(&report);
    assert!(plan.contains("\"target_level\": 3"));
}

#[test]
fn write_all_produces_quality_and_repair_artifacts() {
    let mut r = compute(&two_square_mesh(), &QualityThresholds::default());
    r.cell_view = "tri".to_string();
    let dir = std::env::temp_dir().join(format!("em3_quality_test_{}", std::process::id()));
    let written = io::write_all(&r, &dir).expect("write_all");
    assert_eq!(written.len(), 6);
    for name in [
        "quality_summary.json",
        "quality_summary.csv",
        "worst_cells.geojson",
        "quality_repair_cells.geojson",
        "quality_repair_plan.json",
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

#[test]
fn quality_area_is_dateline_safe_and_spherical() {
    let one_by_one_equator = QualityMeshInput {
        vertices: vec![
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(0.0, 1.0),
        ],
        cells: vec![QualityCell {
            vertices: vec![0, 1, 2, 3],
            refine_level: Some(0),
            neighbors: vec![],
        }],
    };
    let one_by_one_high_lat = QualityMeshInput {
        vertices: vec![
            Point::new(0.0, 60.0),
            Point::new(1.0, 60.0),
            Point::new(1.0, 61.0),
            Point::new(0.0, 61.0),
        ],
        cells: one_by_one_equator.cells.clone(),
    };
    let two_by_one_dateline = QualityMeshInput {
        vertices: vec![
            Point::new(179.0, 0.0),
            Point::new(-179.0, 0.0),
            Point::new(-179.0, 1.0),
            Point::new(179.0, 1.0),
        ],
        cells: one_by_one_equator.cells.clone(),
    };

    let eq = compute(&one_by_one_equator, &QualityThresholds::default());
    let hi = compute(&one_by_one_high_lat, &QualityThresholds::default());
    let dl = compute(&two_by_one_dateline, &QualityThresholds::default());

    assert!(eq.geometry.cell_area.mean > hi.geometry.cell_area.mean);
    assert!((dl.geometry.cell_area.mean / eq.geometry.cell_area.mean - 2.0).abs() < 0.02);
    assert_eq!(dl.geometry.negative_area_cell_count, 0);
}
