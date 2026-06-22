//! Output writers for [`MeshQualityReport`]: machine-readable (JSON/CSV/GeoJSON)
//! and human-readable (Markdown). Hand-rolled (no serde), matching the project's
//! existing JSON style.

use std::io;
use std::path::Path;

use crate::{GeometryMetrics, MeshQualityReport, Stat5, TopologyMetrics};

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

/// `quality_summary.json` content.
pub fn to_summary_json(r: &MeshQualityReport) -> String {
    let g: &GeometryMetrics = &r.geometry;
    let t: &TopologyMetrics = &r.topology;
    let mut s = String::new();
    s.push_str("{\n");
    s.push_str("  \"kind\": \"earthmesh_mesh_quality\",\n");
    s.push_str(&format!("  \"mesh_name\": \"{}\",\n", esc(&r.mesh_name)));
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
        "    \"edge_length_km\": {},\n",
        stat_json(&g.edge_length_km)
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
        "    \"aspect_ratio\": {},\n",
        stat_json(&g.aspect_ratio)
    ));
    s.push_str(&format!(
        "    \"compactness\": {},\n",
        stat_json(&g.compactness)
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
        ("geometry", "edge_length_km_min", g.edge_length_km.min),
        ("geometry", "edge_length_km_mean", g.edge_length_km.mean),
        ("geometry", "min_angle_deg", g.min_angle_deg),
        ("geometry", "max_angle_deg", g.max_angle_deg),
        ("geometry", "aspect_ratio_max", g.aspect_ratio.max),
        ("geometry", "compactness_min", g.compactness.min),
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

/// `quality_report.md` content (human-readable).
pub fn to_report_md(r: &MeshQualityReport) -> String {
    let g = &r.geometry;
    let t = &r.topology;
    let mut s = String::new();
    s.push_str(&format!(
        "# Mesh Quality Report — {}\n\n",
        r.verdict.as_str().to_uppercase()
    ));
    s.push_str(&format!(
        "- mesh: `{}`\n- tool: earthmesh_quality {}\n\n",
        r.mesh_name, r.tool_version
    ));
    s.push_str("## Geometry\n\n");
    s.push_str(&format!(
        "- cells: {} · vertices: {} · edges: {}\n",
        g.cell_count, g.vertex_count, g.edge_count
    ));
    s.push_str(&format!(
        "- cell area (planar deg²): mean {:.4e}, CV {:.3}\n",
        g.cell_area.mean, g.cell_area.cv
    ));
    s.push_str(&format!(
        "- edge length (km): min {:.3}, mean {:.3}\n",
        g.edge_length_km.min, g.edge_length_km.mean
    ));
    s.push_str(&format!(
        "- min angle: {:.2}° · max angle: {:.2}° · max aspect: {:.2} · min compactness: {:.3}\n",
        g.min_angle_deg, g.max_angle_deg, g.aspect_ratio.max, g.compactness.min
    ));
    s.push_str(&format!(
        "- zero-area: {} · self-intersect: {} · invalid: {}\n\n",
        g.zero_area_cell_count, g.self_intersection_count, g.invalid_polygon_count
    ));
    s.push_str("## Topology\n\n");
    s.push_str(&format!(
        "- invalid vertex idx: {} · invalid cell idx: {} · duplicate edges: {} · dangling edges: {}\n",
        t.invalid_vertex_index_count,
        t.invalid_cell_index_count,
        t.duplicate_edge_count,
        t.dangling_edge_count
    ));
    s.push_str(&format!(
        "- orphan cells: {} · neighbor-reciprocity fails: {} · abnormal polygons: {}\n",
        t.orphan_cell_count, t.neighbor_reciprocity_failure_count, t.abnormal_polygon_edge_count
    ));
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

/// Write all four artifacts into `dir`: quality_summary.json/.csv,
/// worst_cells.geojson, quality_report.md.
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
