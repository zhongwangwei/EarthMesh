use crate::HydroMeshQaReport;

/// Serialize a [`HydroMeshQaReport`] to JSON (`kind=earthmesh_hydro_mesh_qa_report`).
pub fn hydro_mesh_qa_report_json(report: &HydroMeshQaReport) -> String {
    let esc = |v: &str| v.replace('\\', "\\\\").replace('"', "\\\"");
    let mut s = String::from("{\n  \"kind\": \"earthmesh_hydro_mesh_qa_report\",\n");
    s.push_str(&format!("  \"status\": \"{}\",\n", report.status));
    s.push_str("  \"thresholds\": {\n");
    s.push_str(&format!(
        "    \"min_river_cells\": {},\n    \"min_coast_cells\": {},\n    \"max_unknown_surface_cells\": 0,\n    \"require_land_ocean_both_present\": true\n  }},\n",
        report.min_river_cells, report.min_coast_cells
    ));
    s.push_str("  \"metrics\": {\n");
    s.push_str(&format!(
        "    \"background_cell_count\": {},\n    \"complete_mask_cell_count\": {},\n    \"river_overlap_cells\": {},\n    \"coast_overlap_cells\": {},\n",
        report.background_cell_count,
        report.complete_mask_cell_count,
        report.river_overlap_cells,
        report.coast_overlap_cells
    ));
    let sc = report
        .surface_class_counts
        .iter()
        .map(|(k, v)| format!("\"{}\": {}", esc(k), v))
        .collect::<Vec<_>>()
        .join(", ");
    s.push_str(&format!("    \"surface_class_counts\": {{{sc}}}"));
    if let Some(rows) = report.colm_rows_written {
        s.push_str(&format!(",\n    \"colm_rows_written\": {rows}"));
    }
    s.push_str("\n  },\n  \"checks\": [\n");
    for (i, c) in report.checks.iter().enumerate() {
        let comma = if i + 1 < report.checks.len() { "," } else { "" };
        let expected = c
            .expected
            .as_ref()
            .map(|e| format!(", \"expected\": \"{}\"", esc(e)))
            .unwrap_or_default();
        s.push_str(&format!(
            "    {{\"id\": \"{}\", \"status\": \"{}\", \"observed\": \"{}\"{}}}{}\n",
            esc(&c.id),
            if c.passed { "pass" } else { "fail" },
            esc(&c.observed),
            expected,
            comma
        ));
    }
    s.push_str("  ]\n}\n");
    s
}
