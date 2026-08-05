//! Output writers for [`MeshQualityReport`]: machine-readable (JSON/CSV/GeoJSON)
//! and human-readable (Markdown). Hand-rolled (no serde), matching the project's
//! existing JSON style.

use std::io;
use std::path::Path;

use crate::{
    AdaptiveDiagnostics, GeometryMetrics, HfieldDiagnostics, LevelCount, MeshQualityReport,
    RefineLevelQualitySummary, Stat5, TopologyMetrics,
};

fn esc(v: &str) -> String {
    let mut out = String::with_capacity(v.len() + 2);
    for c in v.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn num(v: f64) -> String {
    if v.is_finite() {
        format!("{v}")
    } else {
        "null".to_string()
    }
}

fn stat_json(s: &Stat5) -> String {
    format!(
        "{{\"min\":{},\"max\":{},\"mean\":{},\"std\":{},\"cv\":{}}}",
        num(s.min),
        num(s.max),
        num(s.mean),
        num(s.std),
        num(s.cv)
    )
}

fn refine_level_label(level: Option<u32>) -> String {
    level
        .map(|v| v.to_string())
        .unwrap_or_else(|| "unassigned".to_string())
}

fn refine_level_json(level: Option<u32>) -> String {
    level
        .map(|v| v.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn refine_group_json(g: &RefineLevelQualitySummary) -> String {
    format!(
        "{{\"refine_level\":{},\"cell_count\":{},\"cell_area\":{},\"cell_edge_length_cv\":{},\"angle_deviation_deg\":{},\"triangle_eta_local\":{},\"triangle_nsr_local\":{}}}",
        refine_level_json(g.refine_level),
        g.cell_count,
        stat_json(&g.cell_area),
        stat_json(&g.cell_edge_length_cv),
        stat_json(&g.angle_deviation_deg),
        stat_json(&g.triangle_eta),
        stat_json(&g.triangle_nsr)
    )
}

fn opt_f64_json(v: Option<f64>) -> String {
    v.map(num).unwrap_or_else(|| "null".to_string())
}

fn opt_u32_json(v: Option<u32>) -> String {
    v.map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn opt_isize_json(v: Option<isize>) -> String {
    v.map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn level_counts_json(counts: &[LevelCount]) -> String {
    let mut s = String::from("[");
    for (i, item) in counts.iter().enumerate() {
        let comma = if i + 1 < counts.len() { "," } else { "" };
        s.push_str(&format!(
            "{{\"level\":{},\"count\":{}}}{}",
            item.level, item.count, comma
        ));
    }
    s.push(']');
    s
}

fn hfield_json(h: &HfieldDiagnostics) -> String {
    format!(
        "{{\"enabled\":{},\"g\":{},\"max_level\":{},\"base_m\":{},\"cell_count\":{},\
         \"target_level_distribution\":{},\"actual_refine_level_distribution\":{},\
         \"missing_target_level_count\":{},\"extra_target_level_count\":{},\
         \"missing_actual_refine_level_count\":{},\"target_actual_mismatch_count\":{},\
         \"target_above_actual_count\":{},\"actual_above_target_count\":{},\
         \"max_target_actual_delta\":{},\"max_adjacent_target_level_jump\":{},\
         \"target_level_jump_gt_one_count\":{},\"max_adjacent_actual_level_jump\":{},\
         \"actual_level_jump_gt_one_count\":{}}}",
        if h.config.enabled { "true" } else { "false" },
        opt_f64_json(h.config.g),
        opt_u32_json(h.config.max_level),
        opt_f64_json(h.config.base_m),
        h.cell_count,
        level_counts_json(&h.target_level_distribution),
        level_counts_json(&h.actual_refine_level_distribution),
        h.missing_target_level_count,
        h.extra_target_level_count,
        h.missing_actual_refine_level_count,
        h.target_actual_mismatch_count,
        h.target_above_actual_count,
        h.actual_above_target_count,
        h.max_target_actual_delta,
        h.max_adjacent_target_level_jump,
        h.target_level_jump_gt_one_count,
        h.max_adjacent_actual_level_jump,
        h.actual_level_jump_gt_one_count
    )
}

fn adaptive_json(a: &AdaptiveDiagnostics) -> String {
    format!(
        "{{\"enabled\":{},\"max_level\":{},\"base_m\":{},\"coastline\":{},\
         \"pass_count\":{},\"circle_count\":{},\"cell_count\":{},\
         \"target_level_distribution\":{},\"actual_refine_level_distribution\":{},\
         \"missing_target_level_count\":{},\"extra_target_level_count\":{},\
         \"missing_actual_refine_level_count\":{},\"target_actual_mismatch_count\":{},\
         \"target_above_actual_count\":{},\"actual_above_target_count\":{},\
         \"max_target_actual_delta\":{},\"max_adjacent_target_level_jump\":{},\
         \"target_level_jump_gt_one_count\":{},\"max_adjacent_actual_level_jump\":{},\
         \"actual_level_jump_gt_one_count\":{}}}",
        if a.enabled { "true" } else { "false" },
        opt_u32_json(a.max_level),
        opt_f64_json(a.base_m),
        if a.coastline { "true" } else { "false" },
        a.pass_count,
        a.circle_count,
        a.cell_count,
        level_counts_json(&a.target_level_distribution),
        level_counts_json(&a.actual_refine_level_distribution),
        a.missing_target_level_count,
        a.extra_target_level_count,
        a.missing_actual_refine_level_count,
        a.target_actual_mismatch_count,
        a.target_above_actual_count,
        a.actual_above_target_count,
        a.max_target_actual_delta,
        a.max_adjacent_target_level_jump,
        a.target_level_jump_gt_one_count,
        a.max_adjacent_actual_level_jump,
        a.actual_level_jump_gt_one_count
    )
}

fn level_counts_label(counts: &[LevelCount]) -> String {
    if counts.is_empty() {
        return "none".to_string();
    }
    counts
        .iter()
        .map(|item| format!("{}: {}", item.level, item.count))
        .collect::<Vec<_>>()
        .join(", ")
}

fn opt_num_label(v: Option<f64>) -> String {
    v.map(|value| format!("{value}"))
        .unwrap_or_else(|| "n/a".to_string())
}

fn opt_u32_label(v: Option<u32>) -> String {
    v.map(|value| value.to_string())
        .unwrap_or_else(|| "n/a".to_string())
}

fn fixed_or_na(value: f64, precision: usize) -> String {
    if value.is_finite() {
        format!("{value:.precision$}")
    } else {
        "n/a".to_string()
    }
}

/// `quality_summary.json` content.
pub fn to_summary_json(r: &MeshQualityReport) -> String {
    let g: &GeometryMetrics = &r.geometry;
    let t: &TopologyMetrics = &r.topology;
    let mut s = String::new();
    s.push_str("{\n");
    s.push_str("  \"kind\": \"earthmesh_mesh_quality\",\n");
    s.push_str(&format!("  \"mesh_name\": \"{}\",\n", esc(&r.mesh_name)));
    s.push_str(&format!("  \"cell_view\": \"{}\",\n", esc(&r.cell_view)));
    s.push_str(&format!(
        "  \"tool_version\": \"{}\",\n",
        esc(&r.tool_version)
    ));
    s.push_str(&format!("  \"verdict\": \"{}\",\n", r.verdict.as_str()));
    s.push_str("  \"geometry\": {\n");
    s.push_str(&format!("    \"cell_count\": {},\n", g.cell_count));
    s.push_str(&format!("    \"vertex_count\": {},\n", g.vertex_count));
    s.push_str(&format!("    \"edge_count\": {},\n", g.edge_count));
    s.push_str(&format!(
        "    \"cell_area\": {},\n",
        stat_json(&g.cell_area)
    ));
    s.push_str(&format!(
        "    \"cell_area_ratio\": {},\n",
        num(g.cell_area_ratio)
    ));
    s.push_str(&format!(
        "    \"edge_length_km\": {},\n",
        stat_json(&g.edge_length_km)
    ));
    s.push_str(&format!(
        "    \"cell_edge_length_cv\": {},\n",
        stat_json(&g.cell_edge_length_cv)
    ));
    s.push_str(&format!(
        "    \"min_angle_deg\": {},\n",
        num(g.min_angle_deg)
    ));
    s.push_str(&format!(
        "    \"max_angle_deg\": {},\n",
        num(g.max_angle_deg)
    ));
    s.push_str(&format!(
        "    \"angle_deviation_deg\": {},\n",
        stat_json(&g.angle_deviation_deg)
    ));
    s.push_str(&format!(
        "    \"triangle_eta_local\": {},\n",
        stat_json(&g.triangle_eta)
    ));
    s.push_str(&format!(
        "    \"triangle_nsr_local\": {},\n",
        stat_json(&g.triangle_nsr)
    ));
    s.push_str(&format!(
        "    \"aspect_ratio\": {},\n",
        stat_json(&g.aspect_ratio)
    ));
    s.push_str(&format!(
        "    \"compactness\": {},\n",
        stat_json(&g.compactness)
    ));
    s.push_str(&format!(
        "    \"local_shape_metric_sample_count\": {},\n",
        g.local_shape_metric_sample_count
    ));
    s.push_str(&format!(
        "    \"local_shape_metric_excluded_cell_count\": {},\n",
        g.local_shape_metric_excluded_cell_count
    ));
    s.push_str(&format!(
        "    \"zero_area_cell_count\": {},\n",
        g.zero_area_cell_count
    ));
    s.push_str(&format!(
        "    \"negative_area_cell_count\": {},\n",
        g.negative_area_cell_count
    ));
    s.push_str(&format!(
        "    \"non_finite_cell_count\": {},\n",
        g.non_finite_cell_count
    ));
    s.push_str(&format!(
        "    \"self_intersection_count\": {},\n",
        g.self_intersection_count
    ));
    s.push_str(&format!(
        "    \"invalid_polygon_count\": {}\n",
        g.invalid_polygon_count
    ));
    s.push_str("  },\n");
    s.push_str("  \"topology\": {\n");
    s.push_str(&format!(
        "    \"euler_characteristic\": {},\n",
        t.euler_characteristic
    ));
    s.push_str(&format!(
        "    \"expected_euler_characteristic\": {},\n",
        opt_isize_json(t.expected_euler_characteristic)
    ));
    s.push_str(&format!(
        "    \"euler_characteristic_mismatch_count\": {},\n",
        t.euler_characteristic_mismatch_count
    ));
    s.push_str(&format!(
        "    \"connected_component_count\": {},\n",
        t.connected_component_count
    ));
    s.push_str(&format!(
        "    \"non_manifold_vertex_fan_count\": {},\n",
        t.non_manifold_vertex_fan_count
    ));
    s.push_str(&format!(
        "    \"invalid_vertex_index_count\": {},\n",
        t.invalid_vertex_index_count
    ));
    s.push_str(&format!(
        "    \"invalid_cell_index_count\": {},\n",
        t.invalid_cell_index_count
    ));
    s.push_str(&format!(
        "    \"duplicate_edge_count\": {},\n",
        t.duplicate_edge_count
    ));
    s.push_str(&format!(
        "    \"dangling_edge_count\": {},\n",
        t.dangling_edge_count
    ));
    s.push_str(&format!(
        "    \"boundary_edge_count\": {},\n",
        t.boundary_edge_count
    ));
    s.push_str(&format!(
        "    \"boundary_loop_count\": {},\n",
        t.boundary_loop_count
    ));
    s.push_str(&format!(
        "    \"boundary_vertex_degree_violation_count\": {},\n",
        t.boundary_vertex_degree_violation_count
    ));
    s.push_str(&format!(
        "    \"misoriented_shared_edge_count\": {},\n",
        t.misoriented_shared_edge_count
    ));
    s.push_str(&format!(
        "    \"neighbor_degree_mismatch_count\": {},\n",
        t.neighbor_degree_mismatch_count
    ));
    s.push_str(&format!(
        "    \"orphan_cell_count\": {},\n",
        t.orphan_cell_count
    ));
    s.push_str(&format!(
        "    \"neighbor_reciprocity_failure_count\": {},\n",
        t.neighbor_reciprocity_failure_count
    ));
    s.push_str(&format!(
        "    \"abnormal_polygon_edge_count\": {},\n",
        t.abnormal_polygon_edge_count
    ));
    s.push_str(&format!(
        "    \"triangle_cell_count\": {},\n",
        t.triangle_cell_count
    ));
    s.push_str(&format!(
        "    \"quadrilateral_cell_count\": {},\n",
        t.quadrilateral_cell_count
    ));
    s.push_str(&format!(
        "    \"pentagon_cell_count\": {},\n",
        t.pentagon_cell_count
    ));
    s.push_str(&format!(
        "    \"hexagon_cell_count\": {},\n",
        t.hexagon_cell_count
    ));
    s.push_str(&format!(
        "    \"heptagon_cell_count\": {},\n",
        t.heptagon_cell_count
    ));
    s.push_str(&format!(
        "    \"other_polygon_cell_count\": {},\n",
        t.other_polygon_cell_count
    ));
    s.push_str(&format!(
        "    \"isolated_refined_cell_count\": {},\n",
        t.isolated_refined_cell_count
    ));
    s.push_str(&format!(
        "    \"max_adjacent_resolution_ratio\": {},\n",
        num(t.max_adjacent_resolution_ratio)
    ));
    s.push_str(&format!(
        "    \"transition_continuity_warning_count\": {}\n",
        t.transition_continuity_warning_count
    ));
    s.push_str("  },\n");
    s.push_str("  \"refine_level_groups\": [\n");
    for (i, group) in r.refine_level_groups.iter().enumerate() {
        let comma = if i + 1 < r.refine_level_groups.len() {
            ","
        } else {
            ""
        };
        s.push_str(&format!("    {}{}\n", refine_group_json(group), comma));
    }
    s.push_str("  ],\n");
    if let Some(hfield) = &r.hfield {
        s.push_str(&format!("  \"hfield\": {},\n", hfield_json(hfield)));
    } else {
        s.push_str("  \"hfield\": null,\n");
    }
    if let Some(adaptive) = &r.adaptive {
        s.push_str(&format!("  \"adaptive\": {},\n", adaptive_json(adaptive)));
    } else {
        s.push_str("  \"adaptive\": null,\n");
    }
    s.push_str("  \"gates\": [\n");
    for (i, gate) in r.gates.iter().enumerate() {
        let comma = if i + 1 < r.gates.len() { "," } else { "" };
        s.push_str(&format!(
            "    {{\"metric\": \"{}\", \"value\": {}, \"level\": \"{}\"}}{}\n",
            esc(&gate.metric),
            num(gate.value),
            gate.level.as_str(),
            comma
        ));
    }
    s.push_str("  ],\n");
    s.push_str("  \"topology_issues\": [\n");
    for (i, issue) in r.topology_issues.iter().enumerate() {
        let comma = if i + 1 < r.topology_issues.len() {
            ","
        } else {
            ""
        };
        let cell = issue
            .cell_id
            .map(|c| c.to_string())
            .unwrap_or_else(|| "null".into());
        let vertex = issue
            .vertex_id
            .map(|v| v.to_string())
            .unwrap_or_else(|| "null".into());
        s.push_str(&format!(
            "    {{\"issue_type\": \"{}\", \"severity\": \"{}\", \"cell_id\": {}, \"vertex_id\": {}, \
             \"message\": \"{}\", \"suggested_fix\": \"{}\"}}{}\n",
            issue.issue_type.as_str(),
            issue.severity.as_str(),
            cell,
            vertex,
            esc(&issue.message),
            esc(&issue.suggested_fix),
            comma
        ));
    }
    s.push_str("  ]\n}\n");
    s
}

/// `quality_summary.csv` content: one row per metric/gate.
pub fn to_summary_csv(r: &MeshQualityReport) -> String {
    let mut s = String::from("category,metric,value,level\n");
    let g = &r.geometry;
    let t = &r.topology;
    let rows: Vec<(&str, &str, f64)> = vec![
        ("geometry", "cell_count", g.cell_count as f64),
        ("geometry", "vertex_count", g.vertex_count as f64),
        ("geometry", "edge_count", g.edge_count as f64),
        ("geometry", "cell_area_mean", g.cell_area.mean),
        ("geometry", "cell_area_cv", g.cell_area.cv),
        ("geometry", "cell_area_ratio", g.cell_area_ratio),
        ("geometry", "edge_length_km_min", g.edge_length_km.min),
        ("geometry", "edge_length_km_mean", g.edge_length_km.mean),
        (
            "geometry",
            "cell_edge_length_cv_max",
            g.cell_edge_length_cv.max,
        ),
        ("geometry", "min_angle_deg", g.min_angle_deg),
        ("geometry", "max_angle_deg", g.max_angle_deg),
        (
            "geometry",
            "angle_deviation_deg_max",
            g.angle_deviation_deg.max,
        ),
        ("geometry", "triangle_eta_local_min", g.triangle_eta.min),
        ("geometry", "triangle_nsr_local_min", g.triangle_nsr.min),
        ("geometry", "aspect_ratio_max", g.aspect_ratio.max),
        ("geometry", "compactness_min", g.compactness.min),
        (
            "geometry",
            "local_shape_metric_sample_count",
            g.local_shape_metric_sample_count as f64,
        ),
        (
            "geometry",
            "local_shape_metric_excluded_cell_count",
            g.local_shape_metric_excluded_cell_count as f64,
        ),
        (
            "geometry",
            "zero_area_cell_count",
            g.zero_area_cell_count as f64,
        ),
        (
            "geometry",
            "self_intersection_count",
            g.self_intersection_count as f64,
        ),
        (
            "geometry",
            "invalid_polygon_count",
            g.invalid_polygon_count as f64,
        ),
        (
            "topology",
            "euler_characteristic",
            t.euler_characteristic as f64,
        ),
        (
            "topology",
            "euler_characteristic_mismatch_count",
            t.euler_characteristic_mismatch_count as f64,
        ),
        (
            "topology",
            "connected_component_count",
            t.connected_component_count as f64,
        ),
        (
            "topology",
            "non_manifold_vertex_fan_count",
            t.non_manifold_vertex_fan_count as f64,
        ),
        (
            "topology",
            "invalid_vertex_index_count",
            t.invalid_vertex_index_count as f64,
        ),
        (
            "topology",
            "invalid_cell_index_count",
            t.invalid_cell_index_count as f64,
        ),
        (
            "topology",
            "duplicate_edge_count",
            t.duplicate_edge_count as f64,
        ),
        (
            "topology",
            "dangling_edge_count",
            t.dangling_edge_count as f64,
        ),
        (
            "topology",
            "boundary_edge_count",
            t.boundary_edge_count as f64,
        ),
        (
            "topology",
            "boundary_loop_count",
            t.boundary_loop_count as f64,
        ),
        (
            "topology",
            "boundary_vertex_degree_violation_count",
            t.boundary_vertex_degree_violation_count as f64,
        ),
        (
            "topology",
            "misoriented_shared_edge_count",
            t.misoriented_shared_edge_count as f64,
        ),
        (
            "topology",
            "neighbor_degree_mismatch_count",
            t.neighbor_degree_mismatch_count as f64,
        ),
        ("topology", "orphan_cell_count", t.orphan_cell_count as f64),
        (
            "topology",
            "neighbor_reciprocity_failure_count",
            t.neighbor_reciprocity_failure_count as f64,
        ),
        (
            "topology",
            "abnormal_polygon_edge_count",
            t.abnormal_polygon_edge_count as f64,
        ),
        (
            "topology",
            "triangle_cell_count",
            t.triangle_cell_count as f64,
        ),
        (
            "topology",
            "quadrilateral_cell_count",
            t.quadrilateral_cell_count as f64,
        ),
        (
            "topology",
            "pentagon_cell_count",
            t.pentagon_cell_count as f64,
        ),
        (
            "topology",
            "hexagon_cell_count",
            t.hexagon_cell_count as f64,
        ),
        (
            "topology",
            "heptagon_cell_count",
            t.heptagon_cell_count as f64,
        ),
        (
            "topology",
            "other_polygon_cell_count",
            t.other_polygon_cell_count as f64,
        ),
        (
            "topology",
            "isolated_refined_cell_count",
            t.isolated_refined_cell_count as f64,
        ),
        (
            "topology",
            "max_adjacent_resolution_ratio",
            t.max_adjacent_resolution_ratio,
        ),
        (
            "topology",
            "transition_continuity_warning_count",
            t.transition_continuity_warning_count as f64,
        ),
    ];
    for (cat, metric, value) in rows {
        s.push_str(&format!("{cat},{metric},{},\n", num(value)));
    }
    for gate in &r.gates {
        s.push_str(&format!(
            "gate,{},{},{}\n",
            gate.metric,
            num(gate.value),
            gate.level.as_str()
        ));
    }
    for group in &r.refine_level_groups {
        let level = refine_level_label(group.refine_level);
        for (metric, value) in [
            ("cell_count", group.cell_count as f64),
            ("cell_area_cv", group.cell_area.cv),
            ("cell_edge_length_cv_max", group.cell_edge_length_cv.max),
            ("angle_deviation_deg_max", group.angle_deviation_deg.max),
            ("triangle_eta_local_min", group.triangle_eta.min),
            ("triangle_nsr_local_min", group.triangle_nsr.min),
        ] {
            s.push_str(&format!("refine_level,{level}:{metric},{},\n", num(value)));
        }
    }
    if let Some(hfield) = &r.hfield {
        s.push_str(&format!(
            "hfield,enabled,{},\n",
            if hfield.config.enabled { 1 } else { 0 }
        ));
        s.push_str(&format!("hfield,g,{},\n", opt_f64_json(hfield.config.g)));
        s.push_str(&format!(
            "hfield,max_level,{},\n",
            opt_u32_json(hfield.config.max_level)
        ));
        s.push_str(&format!(
            "hfield,base_m,{},\n",
            opt_f64_json(hfield.config.base_m)
        ));
        s.push_str(&format!("hfield,cell_count,{},\n", hfield.cell_count));
        for item in &hfield.target_level_distribution {
            s.push_str(&format!(
                "hfield,target_level_{}_count,{},\n",
                item.level, item.count
            ));
        }
        for item in &hfield.actual_refine_level_distribution {
            s.push_str(&format!(
                "hfield,actual_refine_level_{}_count,{},\n",
                item.level, item.count
            ));
        }
        for (metric, value) in [
            (
                "missing_target_level_count",
                hfield.missing_target_level_count,
            ),
            ("extra_target_level_count", hfield.extra_target_level_count),
            (
                "missing_actual_refine_level_count",
                hfield.missing_actual_refine_level_count,
            ),
            (
                "target_actual_mismatch_count",
                hfield.target_actual_mismatch_count,
            ),
            (
                "target_above_actual_count",
                hfield.target_above_actual_count,
            ),
            (
                "actual_above_target_count",
                hfield.actual_above_target_count,
            ),
            (
                "max_target_actual_delta",
                hfield.max_target_actual_delta as usize,
            ),
            (
                "max_adjacent_target_level_jump",
                hfield.max_adjacent_target_level_jump as usize,
            ),
            (
                "target_level_jump_gt_one_count",
                hfield.target_level_jump_gt_one_count,
            ),
            (
                "max_adjacent_actual_level_jump",
                hfield.max_adjacent_actual_level_jump as usize,
            ),
            (
                "actual_level_jump_gt_one_count",
                hfield.actual_level_jump_gt_one_count,
            ),
        ] {
            s.push_str(&format!("hfield,{metric},{value},\n"));
        }
    }
    s.push_str(&format!("summary,cell_view,,{}\n", r.cell_view));
    s.push_str(&format!("summary,verdict,,{}\n", r.verdict.as_str()));
    s
}

/// `worst_cells.geojson` content (cell rings as polygons with properties).
pub fn to_worst_cells_geojson(r: &MeshQualityReport) -> String {
    let mut s = String::new();
    s.push_str("{\n  \"type\": \"FeatureCollection\",\n");
    s.push_str("  \"kind\": \"earthmesh_quality_worst_cells\",\n");
    s.push_str("  \"features\": [\n");
    for (i, wc) in r.worst_cells.iter().enumerate() {
        let comma = if i + 1 < r.worst_cells.len() { "," } else { "" };
        let mut coords = String::from("[");
        // GeoJSON polygons must be closed: repeat the first vertex.
        let mut ring = wc.ring.clone();
        if let Some(first) = ring.first().copied() {
            ring.push(first);
        }
        for (k, p) in ring.iter().enumerate() {
            let c2 = if k + 1 < ring.len() { "," } else { "" };
            coords.push_str(&format!("[{},{}]{}", num(p.x), num(p.y), c2));
        }
        coords.push(']');
        s.push_str(&format!(
            "    {{\"type\": \"Feature\", \"geometry\": {{\"type\": \"Polygon\", \"coordinates\": [{}]}}, \
             \"properties\": {{\"cell_index\": {}, \"metric\": \"{}\", \"value\": {}, \"level\": \"{}\"}}}}{}\n",
            coords,
            wc.cell_index,
            esc(&wc.metric),
            num(wc.value),
            wc.level.as_str(),
            comma
        ));
    }
    s.push_str("  ]\n}\n");
    s
}

/// Repairable quality defects as target-cell polygons for the existing
/// HField/Method-C per-cell refinement adapter. Structurally invalid cells are
/// deliberately excluded: subdividing an invalid polygon cannot repair it.
pub fn to_quality_repair_cells_geojson(r: &MeshQualityReport) -> String {
    let repairable = &r.repair_cells;
    let mut s = String::from(
        "{\n  \"type\": \"FeatureCollection\",\n  \"kind\": \"earthmesh_quality_repair_cells\",\n  \"features\": [\n",
    );
    for (index, worst) in repairable.iter().enumerate() {
        let mut ring = worst.ring.clone();
        if let Some(first) = ring.first().copied() {
            ring.push(first);
        }
        let coordinates = ring
            .iter()
            .map(|point| format!("[{},{}]", num(point.x), num(point.y)))
            .collect::<Vec<_>>()
            .join(",");
        let comma = if index + 1 < repairable.len() {
            ","
        } else {
            ""
        };
        s.push_str(&format!(
            "    {{\"type\": \"Feature\", \"geometry\": {{\"type\": \"Polygon\", \"coordinates\": [[{coordinates}]]}}, \"properties\": {{\"cell_id\": \"{}\", \"cell_index\": {}, \"center_lon\": {}, \"center_lat\": {}, \"metric\": \"{}\"}}}}{comma}\n",
            worst.cell_index,
            worst.cell_index,
            num(worst.centroid.x),
            num(worst.centroid.y),
            esc(&worst.metric),
        ));
    }
    s.push_str("  ]\n}\n");
    s
}

/// One additional refinement level for every repairable worst cell. The output
/// uses absolute zero-based target levels and is accepted by the existing
/// target-cell HField adapter.
pub fn to_quality_repair_plan_json(r: &MeshQualityReport) -> String {
    to_quality_repair_plan_json_capped(r, 5)
}

/// Project-aware repair plan capped to the refinement level supported by the
/// source mesh resolution.
pub fn to_quality_repair_plan_json_capped(r: &MeshQualityReport, max_level: u8) -> String {
    let repairable = &r.repair_cells;
    let mut s = format!(
        "{{\n  \"kind\": \"earthmesh_refinement_plan\",\n  \"total_cells\": {},\n  \"cells\": [\n",
        repairable.len()
    );
    for (index, worst) in repairable.iter().enumerate() {
        let target_level = worst
            .refine_level
            .unwrap_or(0)
            .saturating_add(1)
            .min(u32::from(max_level.min(5)));
        let comma = if index + 1 < repairable.len() {
            ","
        } else {
            ""
        };
        s.push_str(&format!(
            "    {{\"cell\": {index}, \"cell_id\": \"{}\", \"target_level\": {target_level}}}{comma}\n",
            worst.cell_index,
        ));
    }
    s.push_str("  ]\n}\n");
    s
}

/// `quality_report.md` content (human-readable).
pub fn to_report_md(r: &MeshQualityReport) -> String {
    let g = &r.geometry;
    let t = &r.topology;
    let mut s = String::new();
    s.push_str(&format!(
        "# Mesh Quality Report — {}\n\n",
        r.verdict.as_str().to_uppercase()
    ));
    s.push_str(&format!("- mesh: `{}`\n", r.mesh_name));
    if !r.cell_view.is_empty() {
        s.push_str(&format!("- cell view: `{}`\n", r.cell_view));
    }
    s.push_str(&format!("- tool: earthmesh_quality {}\n\n", r.tool_version));
    s.push_str("## Geometry\n\n");
    s.push_str(&format!(
        "- cells: {} · vertices: {} · edges: {}\n",
        g.cell_count, g.vertex_count, g.edge_count
    ));
    s.push_str(&format!(
        "- cell area (spherical km²): mean {:.4e}, CV {:.3}, max/min {:.2}\n",
        g.cell_area.mean, g.cell_area.cv, g.cell_area_ratio
    ));
    s.push_str(&format!(
        "- edge length (km): min {:.3}, mean {:.3}; max per-cell edge CV {:.3}\n",
        g.edge_length_km.min, g.edge_length_km.mean, g.cell_edge_length_cv.max
    ));
    s.push_str(&format!(
        "- min angle: {}° · max angle: {}° · max angle deviation: {}° · max aspect: {} · min compactness: {}\n",
        fixed_or_na(g.min_angle_deg, 2),
        fixed_or_na(g.max_angle_deg, 2),
        fixed_or_na(g.angle_deviation_deg.max, 2),
        fixed_or_na(g.aspect_ratio.max, 2),
        fixed_or_na(g.compactness.min, 3)
    ));
    s.push_str(&format!(
        "- local shape metric samples: {} · excluded coarse cells: {}\n",
        g.local_shape_metric_sample_count, g.local_shape_metric_excluded_cell_count
    ));
    if g.triangle_eta.max > 0.0 || g.triangle_nsr.max > 0.0 {
        s.push_str(&format!(
            "- local triangle quality: eta min {:.3} · NSR min {:.3}\n",
            g.triangle_eta.min, g.triangle_nsr.min
        ));
    }
    s.push_str(&format!(
        "- zero-area: {} · self-intersect: {} · invalid: {}\n\n",
        g.zero_area_cell_count, g.self_intersection_count, g.invalid_polygon_count
    ));
    s.push_str("## Topology\n\n");
    s.push_str(&format!(
        "- invalid vertex idx: {} · invalid cell idx: {} · duplicate edges: {} · dangling edges: {} · boundary edges: {}\n",
        t.invalid_vertex_index_count,
        t.invalid_cell_index_count,
        t.duplicate_edge_count,
        t.dangling_edge_count,
        t.boundary_edge_count
    ));
    s.push_str(&format!(
        "- orphan cells: {} · neighbor-reciprocity fails: {} · neighbor-degree mismatch: {} · misoriented shared edges: {} · abnormal polygons: {}\n",
        t.orphan_cell_count,
        t.neighbor_reciprocity_failure_count,
        t.neighbor_degree_mismatch_count,
        t.misoriented_shared_edge_count,
        t.abnormal_polygon_edge_count
    ));
    s.push_str(&format!(
        "- Euler characteristic: {} · expected: {} · connected components: {} · non-manifold vertex fans: {}\n",
        t.euler_characteristic,
        t.expected_euler_characteristic
            .map(|value| value.to_string())
            .unwrap_or_else(|| "n/a".into()),
        t.connected_component_count,
        t.non_manifold_vertex_fan_count
    ));
    s.push_str(&format!(
        "- cell sides: triangles {} · quads {} · pentagons {} · hexagons {} · heptagons {} · other {}\n",
        t.triangle_cell_count,
        t.quadrilateral_cell_count,
        t.pentagon_cell_count,
        t.hexagon_cell_count,
        t.heptagon_cell_count,
        t.other_polygon_cell_count
    ));
    s.push_str("- cell-side counts are informational; quality gates are listed below\n");
    s.push_str(&format!(
        "- isolated refined: {} · max adjacent res ratio: {:.2} · transition warnings: {}\n\n",
        t.isolated_refined_cell_count,
        t.max_adjacent_resolution_ratio,
        t.transition_continuity_warning_count
    ));
    s.push_str("## Gates\n\n| Metric | Value | Level |\n|--------|-------|-------|\n");
    for gate in &r.gates {
        s.push_str(&format!(
            "| {} | {} | {} |\n",
            gate.metric,
            num(gate.value),
            gate.level.as_str()
        ));
    }
    if !r.refine_level_groups.is_empty() {
        s.push_str(
            "\n## Refine-level groups\n\n| Level | Cells | Area CV | Edge CV max | Angle dev max | Tri eta local min | Tri NSR local min |\n",
        );
        s.push_str("|-------|-------|---------|-------------|---------------|-------------|-------------|\n");
        for group in &r.refine_level_groups {
            s.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} |\n",
                refine_level_label(group.refine_level),
                group.cell_count,
                fixed_or_na(group.cell_area.cv, 3),
                fixed_or_na(group.cell_edge_length_cv.max, 3),
                fixed_or_na(group.angle_deviation_deg.max, 3),
                fixed_or_na(group.triangle_eta.min, 3),
                fixed_or_na(group.triangle_nsr.min, 3)
            ));
        }
    }
    if let Some(hfield) = &r.hfield {
        s.push_str("\n## H-field diagnostics\n\n");
        s.push_str(&format!(
            "- effective config: enabled {} · g {} · max_level {} · base_m {}\n",
            hfield.config.enabled,
            opt_num_label(hfield.config.g),
            opt_u32_label(hfield.config.max_level),
            opt_num_label(hfield.config.base_m)
        ));
        s.push_str(&format!(
            "- target level distribution: {}\n",
            level_counts_label(&hfield.target_level_distribution)
        ));
        s.push_str(&format!(
            "- actual refine level distribution: {}\n",
            level_counts_label(&hfield.actual_refine_level_distribution)
        ));
        s.push_str(&format!(
            "- target/actual mismatch: {} (target>actual {}, actual>target {}, max delta {})\n",
            hfield.target_actual_mismatch_count,
            hfield.target_above_actual_count,
            hfield.actual_above_target_count,
            hfield.max_target_actual_delta
        ));
        s.push_str(&format!(
            "- missing target: {} · extra target: {} · missing actual refine level: {}\n",
            hfield.missing_target_level_count,
            hfield.extra_target_level_count,
            hfield.missing_actual_refine_level_count
        ));
        s.push_str(&format!(
            "- adjacent target level jump max: {} · >1 count: {}\n",
            hfield.max_adjacent_target_level_jump, hfield.target_level_jump_gt_one_count
        ));
        s.push_str(&format!(
            "- adjacent actual level jump max: {} · >1 count: {}\n",
            hfield.max_adjacent_actual_level_jump, hfield.actual_level_jump_gt_one_count
        ));
    }
    if !r.topology_issues.is_empty() {
        s.push_str(
            "\n## Topology issues\n\n| Type | Severity | Cell | Message | Suggested fix |\n",
        );
        s.push_str("|------|----------|------|---------|---------------|\n");
        for issue in &r.topology_issues {
            let cell = issue.cell_id.map(|c| c.to_string()).unwrap_or_default();
            s.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                issue.issue_type.as_str(),
                issue.severity.as_str(),
                cell,
                issue.message,
                issue.suggested_fix
            ));
        }
    }
    s.push_str(&format!(
        "\n**Verdict: {}**\n",
        r.verdict.as_str().to_uppercase()
    ));
    if !r.worst_cells.is_empty() {
        s.push_str(&format!(
            "\n{} worst cell(s) in `worst_cells.geojson`.\n",
            r.worst_cells.len()
        ));
    }
    s
}

/// Write quality reports plus the bounded local-refinement overlay consumed by
/// the existing target-cell HField adapter.
pub fn write_all(
    r: &MeshQualityReport,
    dir: impl AsRef<Path>,
) -> io::Result<Vec<std::path::PathBuf>> {
    let dir = dir.as_ref();
    std::fs::create_dir_all(dir)?;
    let outputs = [
        ("quality_summary.json", to_summary_json(r)),
        ("quality_summary.csv", to_summary_csv(r)),
        ("worst_cells.geojson", to_worst_cells_geojson(r)),
        (
            "quality_repair_cells.geojson",
            to_quality_repair_cells_geojson(r),
        ),
        ("quality_repair_plan.json", to_quality_repair_plan_json(r)),
        ("quality_report.md", to_report_md(r)),
    ];
    let mut written = Vec::new();
    for (name, content) in outputs {
        let path = dir.join(name);
        std::fs::write(&path, content)?;
        written.push(path);
    }
    Ok(written)
}
