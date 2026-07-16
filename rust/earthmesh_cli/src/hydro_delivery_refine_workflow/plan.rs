use crate::json_escape_string;
use crate::json_number;
use crate::read_text_maybe_gzip;
use std::fs;
use std::io;
use std::path::Path;

use super::feature_table::hydro_refine_feature_set;

fn hydro_refine_plan_json(
    report: &earthmesh_refine_planner::RefinementReport,
    cell_ids: &[String],
) -> String {
    let levels = &report.target_levels.level;
    let max_level = levels.iter().copied().max().unwrap_or(0);
    let mut hist = vec![0usize; max_level as usize + 1];
    for &l in levels {
        hist[l as usize] += 1;
    }
    let hist_items: Vec<String> = hist
        .iter()
        .enumerate()
        .map(|(l, c)| format!("\"{l}\": {c}"))
        .collect();
    let rows: Vec<String> = report
        .decisions
        .iter()
        .map(|d| {
            let cell_id = cell_ids.get(d.cell).map(String::as_str).unwrap_or("");
            format!(
                "    {{\"cell\": {}, \"cell_id\": \"{}\", \"target_level\": {}, \"composite_score\": {}, \"why\": \"{}\"}}",
                d.cell,
                json_escape_string(cell_id),
                d.final_level,
                json_number(d.composite_score),
                json_escape_string(&d.top_reason),
            )
        })
        .collect();
    format!(
        "{{\n  \"kind\": \"earthmesh_refinement_plan\",\n  \"total_cells\": {},\n  \
         \"cells_refined\": {},\n  \"max_level\": {},\n  \"budget_hit\": {},\n  \
         \"level_histogram\": {{{}}},\n  \"cells\": [\n{}\n  ]\n}}\n",
        levels.len(),
        report.budget_used.cells_refined_after,
        max_level,
        report.budget_used.budget_hit,
        hist_items.join(", "),
        rows.join(",\n"),
    )
}

/// R8 refinement planner driven by the real MERIT-Hydro river/coast signal: read a
/// per-cell hydro intersection / complete-mask GeoJSON, score each cell with the
/// `hydro_coast_score` criterion, and write a `target_level` plan
/// (`earthmesh_refinement_plan` JSON). `max_level` caps the level
/// (`target_level = round(demand * max_level)`); `max_refined_cells` optionally budgets
/// the highest-demand cells. The per-cell overlay has no adjacency, so the planner's
/// topology passes are disabled (they would drop isolated high-demand river/coast cells).
pub fn plan_refinement_from_hydro_geojson(
    geojson: impl AsRef<Path>,
    output_json: impl AsRef<Path>,
    max_level: u8,
    max_refined_cells: Option<usize>,
) -> io::Result<earthmesh_refine_planner::RefinementReport> {
    if max_level == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "refinement max_level must be in 1..=255",
        ));
    }
    use earthmesh_refine_planner as rp;
    let features = hydro_refine_feature_set(&read_text_maybe_gzip(geojson.as_ref())?)?;
    let criteria = vec![rp::hydro_coast_score_criterion()];
    let cfg = rp::CompositeScoreConfig {
        weights: vec![("hydro_coast_score".to_string(), 1.0)],
        combine: rp::CombineRule::WeightedMax,
        max_passes: max_level,
    };
    let budget = rp::RefinementBudget {
        max_refined_cells,
        ..Default::default()
    };
    let quality = rp::QualityConstraint {
        no_isolated_refined: false,
        smooth_transition: false,
        ..Default::default()
    };
    let report = rp::plan(
        &features.table,
        &criteria,
        &cfg,
        &budget,
        &quality,
        rp::MeshDomain::Coupled,
    )
    .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    crate::ensure_parent_dir(output_json.as_ref())?;
    fs::write(
        output_json,
        hydro_refine_plan_json(&report, &features.table.cell_ids),
    )?;
    Ok(report)
}
