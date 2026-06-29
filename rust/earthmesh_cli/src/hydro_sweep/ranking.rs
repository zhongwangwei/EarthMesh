use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::hydro_workflow_types::RankedSweepCase;
use crate::{JsonNode, JsonParser};

fn promotion_bucket(status: &str) -> i32 {
    match status {
        "candidate" => 0,
        "blocked_background_cell_cap" => 1,
        _ => 2,
    }
}

fn rankable_row(report: &JsonNode, max_background_cells: Option<i64>) -> RankedSweepCase {
    let obj = report.as_object();
    let get_obj = |key: &str| obj.and_then(|o| o.get(key)).and_then(JsonNode::as_object);
    let background = get_obj("background_cells");
    let river = get_obj("river_intersections");
    let coast = get_obj("coast_intersections");
    let num = |o: Option<&std::collections::BTreeMap<String, JsonNode>>, k: &str| {
        o.and_then(|m| m.get(k))
            .and_then(JsonNode::as_f64)
            .unwrap_or(0.0)
    };
    let status = obj
        .and_then(|o| o.get("status"))
        .and_then(JsonNode::as_str)
        .unwrap_or("pass")
        .to_string();
    let background_cells = num(background, "cell_count") as i64;
    let promotion_status = if status != "pass" {
        "failed".to_string()
    } else if max_background_cells.is_some_and(|cap| background_cells > cap) {
        "blocked_background_cell_cap".to_string()
    } else {
        "candidate".to_string()
    };
    let retained_at = |deg: &str| {
        obj.and_then(|o| o.get("refinement_log"))
            .and_then(JsonNode::as_object)
            .and_then(|l| l.get(deg))
            .and_then(JsonNode::as_object)
            .and_then(|d| d.get("retained_triangles"))
            .and_then(JsonNode::as_f64)
            .unwrap_or(0.0) as i64
    };
    RankedSweepCase {
        case_name: obj
            .and_then(|o| o.get("case_name"))
            .and_then(JsonNode::as_str)
            .unwrap_or("")
            .to_string(),
        status,
        promotion_status,
        background_cell_count: background_cells,
        background_median_dx_km: num(background, "equivalent_cell_size_km_median"),
        river_overlap_cells: num(river, "feature_count") as i64,
        coast_overlap_cells: num(coast, "feature_count") as i64,
        retained: [retained_at("1"), retained_at("2"), retained_at("3")],
        rank: 0,
    }
}

/// Faithful port of `refinement_sweep.py::rank_sweep_reports`: sort eval reports into
/// promotion candidates (bucket, retained 3/2/1 desc, river/coast desc, median dx asc,
/// cell count asc, case name) and assign 1-based ranks.
fn rank_sweep_rows(mut rows: Vec<RankedSweepCase>) -> Vec<RankedSweepCase> {
    rows.sort_by(|a, b| {
        use std::cmp::Ordering;
        let key = |r: &RankedSweepCase| {
            (
                promotion_bucket(&r.promotion_status),
                -r.retained[2],
                -r.retained[1],
                -r.retained[0],
                -r.river_overlap_cells,
                -r.coast_overlap_cells,
            )
        };
        key(a)
            .cmp(&key(b))
            .then(
                a.background_median_dx_km
                    .partial_cmp(&b.background_median_dx_km)
                    .unwrap_or(Ordering::Equal),
            )
            .then(a.background_cell_count.cmp(&b.background_cell_count))
            .then(a.case_name.cmp(&b.case_name))
    });
    for (i, row) in rows.iter_mut().enumerate() {
        row.rank = i + 1;
    }
    rows
}

/// Faithful port of `refinement_sweep.py::write_sweep_ranking`.
pub fn write_sweep_ranking(
    report_paths: &[PathBuf],
    output_json: impl AsRef<Path>,
    max_background_cells: Option<i64>,
) -> io::Result<String> {
    let mut rows = Vec::new();
    for path in report_paths {
        let text = fs::read_to_string(path)?;
        let mut report = JsonParser::new(&text).parse()?;
        if let JsonNode::Object(map) = &mut report {
            if !map.contains_key("case_name") {
                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                map.insert("case_name".into(), JsonNode::String(stem));
            }
        }
        rows.push(rankable_row(&report, max_background_cells));
    }
    let ranked = rank_sweep_rows(rows);
    let recommended = ranked
        .iter()
        .find(|r| r.promotion_status == "candidate")
        .map(|r| r.case_name.clone());

    let mut s = String::from("{\n  \"kind\": \"earthmesh_refinement_sweep_ranking\",\n");
    match &recommended {
        Some(c) => s.push_str(&format!(
            "  \"recommended_case\": \"{}\",\n",
            c.replace('"', "\\\"")
        )),
        None => s.push_str("  \"recommended_case\": null,\n"),
    }
    s.push_str("  \"ranked_cases\": [\n");
    let n = ranked.len();
    for (i, r) in ranked.iter().enumerate() {
        let comma = if i + 1 < n { "," } else { "" };
        s.push_str(&format!(
            "    {{\"rank\": {}, \"case_name\": \"{}\", \"status\": \"{}\", \"promotion_status\": \"{}\", \
             \"background_cell_count\": {}, \"background_median_dx_km\": {}, \"river_overlap_cells\": {}, \
             \"coast_overlap_cells\": {}, \"retained_triangles\": {{\"1\": {}, \"2\": {}, \"3\": {}}}}}{}\n",
            r.rank,
            r.case_name.replace('"', "\\\""),
            r.status,
            r.promotion_status,
            r.background_cell_count,
            r.background_median_dx_km,
            r.river_overlap_cells,
            r.coast_overlap_cells,
            r.retained[0],
            r.retained[1],
            r.retained[2],
            comma
        ));
    }
    s.push_str("  ]\n}\n");
    crate::ensure_parent_dir(output_json.as_ref())?;
    fs::write(output_json, &s)?;
    Ok(recommended.unwrap_or_default())
}
