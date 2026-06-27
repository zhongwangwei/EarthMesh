use std::collections::BTreeMap;

use crate::hydro_workflow_types::{HydroBackgroundSummary, HydroIntersectionSummary};

fn opt_num_field(buf: &mut String, key: &str, value: Option<f64>) {
    if let Some(v) = value {
        if v.is_finite() {
            buf.push_str(&format!(",\n    \"{key}\": {v}"));
        }
    }
}

fn intersection_summary_json(summary: &HydroIntersectionSummary, kind: &str) -> String {
    let cc = summary
        .class_counts
        .iter()
        .map(|(k, v)| format!("\"{}\": {}", k.replace('"', "\\\""), v))
        .collect::<Vec<_>>()
        .join(", ");
    let mut s = format!(
        "{{\n    \"feature_count\": {},\n    \"class_counts\": {{{cc}}}",
        summary.feature_count
    );
    let (fmin, fmed, fmax, asum) = if kind == "river" {
        (
            "river_fraction_min",
            "river_fraction_median",
            "river_fraction_max",
            "estimated_river_area_m2_sum",
        )
    } else {
        (
            "coastal_fraction_min",
            "coastal_fraction_median",
            "coastal_fraction_max",
            "estimated_coastal_area_m2_sum",
        )
    };
    opt_num_field(&mut s, fmin, summary.fraction_min);
    opt_num_field(&mut s, fmed, summary.fraction_median);
    opt_num_field(&mut s, fmax, summary.fraction_max);
    opt_num_field(&mut s, asum, summary.area_sum);
    s.push_str("\n  }");
    s
}

/// Assemble the refinement-eval report JSON (faithful to `build_refinement_eval`).
pub fn build_refinement_eval_json(
    background: &HydroBackgroundSummary,
    river: &HydroIntersectionSummary,
    coast: Option<&HydroIntersectionSummary>,
    log: Option<&BTreeMap<String, BTreeMap<String, i64>>>,
) -> String {
    let mut s = String::from("{\n  \"kind\": \"earthmesh_hydro_refinement_eval\",\n");
    let mut bg = format!(
        "  \"background_cells\": {{\n    \"cell_count\": {}",
        background.cell_count
    );
    opt_num_field(
        &mut bg,
        "equivalent_cell_size_km_min",
        background.size_km_min,
    );
    opt_num_field(
        &mut bg,
        "equivalent_cell_size_km_median",
        background.size_km_median,
    );
    opt_num_field(
        &mut bg,
        "equivalent_cell_size_km_max",
        background.size_km_max,
    );
    bg.push_str("\n  }");
    s.push_str(&bg);
    s.push_str(",\n  \"river_intersections\": ");
    s.push_str(&intersection_summary_json(river, "river"));
    if let Some(c) = coast {
        s.push_str(",\n  \"coast_intersections\": ");
        s.push_str(&intersection_summary_json(c, "coast"));
    }
    if let Some(log) = log {
        s.push_str(",\n  \"refinement_log\": {");
        let degrees: Vec<String> = log
            .iter()
            .map(|(deg, metrics)| {
                let inner = metrics
                    .iter()
                    .map(|(k, v)| format!("\"{k}\": {v}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("\n    \"{deg}\": {{{inner}}}")
            })
            .collect();
        s.push_str(&degrees.join(","));
        s.push_str("\n  }");
    }
    s.push_str("\n}\n");
    s
}
