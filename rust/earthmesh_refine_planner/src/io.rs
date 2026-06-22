//! Output writers for [`RefinementReport`]: refinement_score.csv,
//! target_levels.geojson (centroid points), refinement_decision_report.json.
//! Hand-rolled (no serde), matching the project's JSON style.

use std::io;
use std::path::Path;

use crate::{CellFeatureTable, RefinementReport};

fn esc(v: &str) -> String {
    v.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', " ")
}
fn num(v: f64) -> String {
    if v.is_finite() {
        format!("{v}")
    } else {
        "null".to_string()
    }
}

/// `refinement_score.csv`: one row per cell.
pub fn to_refinement_score_csv(r: &RefinementReport) -> String {
    let mut s =
        String::from("cell,composite_score,target_level,final_level,rejection_reason,top_reason\n");
    for d in &r.decisions {
        s.push_str(&format!(
            "{},{},{},{},{},{}\n",
            d.cell,
            num(d.composite_score),
            d.target_level,
            d.final_level,
            d.rejection_reason.as_deref().unwrap_or(""),
            esc(&d.top_reason).replace(',', ";")
        ));
    }
    s
}

/// `target_levels.geojson`: centroid points carrying final level + why-refine reason
/// (so the GUI can show "why refine here"). Points (not polygons) for the MVP.
pub fn to_target_levels_geojson(r: &RefinementReport, features: &CellFeatureTable) -> String {
    let mut s = String::from("{\n  \"type\": \"FeatureCollection\",\n");
    s.push_str("  \"kind\": \"earthmesh_target_levels\",\n  \"features\": [\n");
    let n = r.decisions.len();
    for (i, d) in r.decisions.iter().enumerate() {
        let comma = if i + 1 < n { "," } else { "" };
        let p = features
            .centroids
            .get(d.cell)
            .copied()
            .unwrap_or(earthmesh_geometry::Point::new(0.0, 0.0));
        s.push_str(&format!(
            "    {{\"type\": \"Feature\", \"geometry\": {{\"type\": \"Point\", \"coordinates\": [{}, {}]}}, \
             \"properties\": {{\"cell\": {}, \"final_level\": {}, \"target_level\": {}, \
             \"composite_score\": {}, \"rejection_reason\": {}, \"why\": \"{}\"}}}}{}\n",
            num(p.x),
            num(p.y),
            d.cell,
            d.final_level,
            d.target_level,
            num(d.composite_score),
            d.rejection_reason
                .as_deref()
                .map(|r| format!("\"{}\"", esc(r)))
                .unwrap_or_else(|| "null".into()),
            esc(&d.top_reason),
            comma
        ));
    }
    s.push_str("  ]\n}\n");
    s
}

/// `refinement_decision_report.json`: summary + per-cell decisions.
pub fn to_refinement_decision_report_json(r: &RefinementReport) -> String {
    let b = &r.budget_used;
    let mut s = String::from("{\n  \"kind\": \"earthmesh_refinement_decision\",\n");
    s.push_str(&format!("  \"max_passes\": {},\n", r.max_passes));
    s.push_str(&format!(
        "  \"budget\": {{\"cells_refined_before\": {}, \"cells_refined_after\": {}, \"budget_hit\": {}}},\n",
        b.cells_refined_before, b.cells_refined_after, b.budget_hit
    ));
    s.push_str("  \"decisions\": [\n");
    let n = r.decisions.len();
    for (i, d) in r.decisions.iter().enumerate() {
        let comma = if i + 1 < n { "," } else { "" };
        s.push_str(&format!(
            "    {{\"cell\": {}, \"composite_score\": {}, \"target_level\": {}, \"final_level\": {}, \
             \"rejection_reason\": {}, \"why\": \"{}\"}}{}\n",
            d.cell,
            num(d.composite_score),
            d.target_level,
            d.final_level,
            d.rejection_reason
                .as_deref()
                .map(|r| format!("\"{}\"", esc(r)))
                .unwrap_or_else(|| "null".into()),
            esc(&d.top_reason),
            comma
        ));
    }
    s.push_str("  ]\n}\n");
    s
}

/// Write all three artifacts into `dir`.
pub fn write_all(
    r: &RefinementReport,
    features: &CellFeatureTable,
    dir: impl AsRef<Path>,
) -> io::Result<Vec<std::path::PathBuf>> {
    let dir = dir.as_ref();
    std::fs::create_dir_all(dir)?;
    let outputs = [
        ("refinement_score.csv", to_refinement_score_csv(r)),
        (
            "target_levels.geojson",
            to_target_levels_geojson(r, features),
        ),
        (
            "refinement_decision_report.json",
            to_refinement_decision_report_json(r),
        ),
    ];
    let mut written = Vec::new();
    for (name, content) in outputs {
        let path = dir.join(name);
        std::fs::write(&path, content)?;
        written.push(path);
    }
    Ok(written)
}
