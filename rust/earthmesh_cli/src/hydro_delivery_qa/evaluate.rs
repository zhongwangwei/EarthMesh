use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::{geojson_feature_nodes, HydroMeshQaCheck, HydroMeshQaReport, JsonNode, JsonParser};

fn manifest_metric_i64(metrics: Option<&BTreeMap<String, JsonNode>>, key: &str) -> i64 {
    metrics
        .and_then(|m| m.get(key))
        .and_then(JsonNode::as_f64)
        .unwrap_or(0.0) as i64
}

/// Evaluate delivery-package QA gates. Faithful to `qa_gates.py::evaluate_hydro_mesh_qa`.
pub fn evaluate_hydro_mesh_qa(
    delivery_manifest_json: impl AsRef<Path>,
    colm_summary_json: Option<&Path>,
    min_river_cells: i64,
    min_coast_cells: i64,
) -> io::Result<HydroMeshQaReport> {
    let manifest_text = fs::read_to_string(delivery_manifest_json.as_ref())?;
    let manifest = JsonParser::new(&manifest_text).parse()?;
    let manifest_obj = manifest.as_object();
    let metrics = manifest_obj
        .and_then(|m| m.get("metrics"))
        .and_then(JsonNode::as_object);
    let background_count = manifest_metric_i64(metrics, "background_cell_count");
    let river_cells = manifest_metric_i64(metrics, "river_overlap_cells");
    let coast_cells = manifest_metric_i64(metrics, "coast_overlap_cells");

    let complete_mask_path = manifest_obj
        .and_then(|m| m.get("files"))
        .and_then(JsonNode::as_object)
        .and_then(|f| f.get("complete_cell_mask_geojson"))
        .and_then(JsonNode::as_str)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from);

    let mut surface_counts: BTreeMap<String, i64> = BTreeMap::new();
    let mut complete_feature_count: i64 = 0;
    let mut unknown_surface_count: i64 = 0;
    if let Some(path) = &complete_mask_path {
        let text = fs::read_to_string(path)?;
        let root = JsonParser::new(&text).parse()?;
        for feature in geojson_feature_nodes(&root) {
            complete_feature_count += 1;
            let surface = feature
                .as_object()
                .and_then(|o| o.get("properties"))
                .and_then(JsonNode::as_object)
                .and_then(|p| p.get("surface_class"))
                .and_then(JsonNode::as_str)
                .filter(|s| !s.is_empty())
                .unwrap_or("UNKNOWN")
                .to_string();
            if surface != "LAND" && surface != "OCEAN" {
                unknown_surface_count += 1;
            }
            *surface_counts.entry(surface).or_insert(0) += 1;
        }
    }

    let surface_counts_str = format!(
        "{{{}}}",
        surface_counts
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let mut checks = vec![
        HydroMeshQaCheck {
            id: "complete_mask_present".into(),
            passed: complete_mask_path.is_some(),
            observed: complete_mask_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            expected: None,
        },
        HydroMeshQaCheck {
            id: "complete_mask_cell_count_matches_background".into(),
            passed: complete_feature_count == background_count,
            observed: complete_feature_count.to_string(),
            expected: Some(background_count.to_string()),
        },
        HydroMeshQaCheck {
            id: "surface_classes_known".into(),
            passed: unknown_surface_count == 0,
            observed: unknown_surface_count.to_string(),
            expected: Some("0".into()),
        },
        HydroMeshQaCheck {
            id: "land_ocean_both_present".into(),
            passed: surface_counts.get("LAND").copied().unwrap_or(0) > 0
                && surface_counts.get("OCEAN").copied().unwrap_or(0) > 0,
            observed: surface_counts_str,
            expected: Some("{LAND: >0, OCEAN: >0}".into()),
        },
        HydroMeshQaCheck {
            id: "river_cells_present".into(),
            passed: river_cells >= min_river_cells,
            observed: river_cells.to_string(),
            expected: Some(format!(">={min_river_cells}")),
        },
        HydroMeshQaCheck {
            id: "coast_cells_present".into(),
            passed: coast_cells >= min_coast_cells,
            observed: coast_cells.to_string(),
            expected: Some(format!(">={min_coast_cells}")),
        },
    ];

    let mut colm_rows_written = None;
    if let Some(colm_path) = colm_summary_json {
        let text = fs::read_to_string(colm_path)?;
        let colm = JsonParser::new(&text).parse()?;
        let colm_obj = colm.as_object();
        let rows_written = colm_obj
            .and_then(|c| c.get("rows_written"))
            .and_then(JsonNode::as_f64)
            .unwrap_or(0.0) as i64;
        let unknown_colm = colm_obj
            .and_then(|c| c.get("surface_class_counts"))
            .and_then(JsonNode::as_object)
            .and_then(|s| s.get("UNKNOWN"))
            .and_then(JsonNode::as_f64)
            .unwrap_or(0.0) as i64;
        colm_rows_written = Some(rows_written);
        checks.push(HydroMeshQaCheck {
            id: "colm_rows_match_background".into(),
            passed: rows_written == background_count,
            observed: rows_written.to_string(),
            expected: Some(background_count.to_string()),
        });
        checks.push(HydroMeshQaCheck {
            id: "colm_surface_unknown_zero".into(),
            passed: unknown_colm == 0,
            observed: unknown_colm.to_string(),
            expected: Some("0".into()),
        });
    }

    let status = if checks.iter().all(|c| c.passed) {
        "pass"
    } else {
        "fail"
    }
    .to_string();

    Ok(HydroMeshQaReport {
        status,
        background_cell_count: background_count,
        complete_mask_cell_count: complete_feature_count,
        surface_class_counts: surface_counts,
        river_overlap_cells: river_cells,
        coast_overlap_cells: coast_cells,
        min_river_cells,
        min_coast_cells,
        colm_rows_written,
        checks,
    })
}
