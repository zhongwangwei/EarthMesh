//! Quality summary parsing and serializable quality report DTOs.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Distribution summary for a metric (mean ± std over [min, max]).
#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
pub(crate) struct Stat {
    pub(crate) min: f64,
    pub(crate) max: f64,
    pub(crate) mean: f64,
    pub(crate) std: f64,
}

/// One quality gate: a metric value + its pass/warn/fail level.
#[derive(Debug, Serialize)]
pub(crate) struct Gate {
    pub(crate) metric: String,
    pub(crate) value: Option<f64>,
    pub(crate) level: String,
}

/// A mesh-quality summary for the dashboard, parsed from `quality_summary.json`.
#[derive(Debug, Serialize)]
pub(crate) struct MeshQuality {
    pub(crate) verdict: String,
    pub(crate) cell_view: String,
    pub(crate) cell_count: i64,
    pub(crate) vertex_count: i64,
    pub(crate) edge_count: i64,
    pub(crate) min_angle_deg: Option<f64>,
    pub(crate) max_angle_deg: Option<f64>,
    pub(crate) cell_area: Option<Stat>,
    pub(crate) edge_length_km: Option<Stat>,
    pub(crate) aspect_ratio: Option<Stat>,
    pub(crate) compactness: Option<Stat>,
    pub(crate) zero_area: i64,
    pub(crate) negative_area: i64,
    pub(crate) self_intersection: i64,
    pub(crate) invalid_polygon: i64,
    pub(crate) max_adjacent_resolution_ratio: f64,
    /// (name, count) for polygon side counts; informational, not a defect list.
    pub(crate) cell_sides: Vec<(String, i64)>,
    /// (name, count) for each topology issue counter.
    pub(crate) topology: Vec<(String, i64)>,
    pub(crate) gates: Vec<Gate>,
    pub(crate) report_path: Option<String>,
    pub(crate) worst_cells_path: Option<String>,
}

#[derive(Deserialize)]
struct QualitySummaryJson {
    verdict: String,
    cell_view: String,
    geometry: GeometryJson,
    topology: TopologyJson,
    gates: Vec<GateJson>,
}

#[derive(Deserialize)]
struct GateJson {
    metric: String,
    value: serde_json::Value,
    level: String,
}

#[derive(Deserialize)]
struct GeometryJson {
    cell_count: i64,
    vertex_count: i64,
    edge_count: i64,
    min_angle_deg: Option<f64>,
    max_angle_deg: Option<f64>,
    cell_area: Option<Stat>,
    edge_length_km: Option<Stat>,
    aspect_ratio: Option<Stat>,
    compactness: Option<Stat>,
    #[serde(default)]
    zero_area_cell_count: i64,
    #[serde(default)]
    negative_area_cell_count: i64,
    #[serde(default)]
    self_intersection_count: i64,
    #[serde(default)]
    invalid_polygon_count: i64,
}

#[derive(Deserialize)]
struct TopologyJson {
    #[serde(default)]
    triangle_cell_count: i64,
    #[serde(default)]
    quadrilateral_cell_count: i64,
    #[serde(default)]
    pentagon_cell_count: i64,
    #[serde(default)]
    hexagon_cell_count: i64,
    #[serde(default)]
    heptagon_cell_count: i64,
    #[serde(default)]
    other_polygon_cell_count: i64,
    #[serde(default)]
    duplicate_edge_count: i64,
    #[serde(default)]
    dangling_edge_count: i64,
    #[serde(default)]
    orphan_cell_count: i64,
    #[serde(default)]
    neighbor_reciprocity_failure_count: i64,
    #[serde(default)]
    abnormal_polygon_edge_count: i64,
    #[serde(default)]
    isolated_refined_cell_count: i64,
    #[serde(default)]
    transition_continuity_warning_count: i64,
    #[serde(default)]
    invalid_vertex_index_count: i64,
    #[serde(default)]
    invalid_cell_index_count: i64,
    #[serde(default)]
    max_adjacent_resolution_ratio: f64,
}

fn validate_level(level: &str, field: &str) -> Result<(), String> {
    match level {
        "pass" | "warn" | "fail" => Ok(()),
        _ => Err(format!(
            "quality JSON {field} must be pass, warn, or fail, got {level:?}"
        )),
    }
}

/// Parse `quality_summary.json` text into a [`MeshQuality`]. `dir` is the report dir,
/// used to locate the .md / worst-cells artifacts written alongside it.
pub(crate) fn parse_quality_summary(text: &str, dir: &Path) -> Result<MeshQuality, String> {
    let parsed: QualitySummaryJson = serde_json::from_str(text)
        .map_err(|error| format!("quality JSON schema error: {error}"))?;
    validate_level(&parsed.verdict, "verdict")?;
    if !matches!(parsed.cell_view.as_str(), "tri" | "hex") {
        return Err(format!(
            "quality JSON cell_view must be tri or hex, got {:?}",
            parsed.cell_view
        ));
    }
    let mut gates = Vec::with_capacity(parsed.gates.len());
    for (index, gate) in parsed.gates.into_iter().enumerate() {
        if gate.metric.trim().is_empty() {
            return Err(format!(
                "quality JSON gates[{index}].metric must not be empty"
            ));
        }
        validate_level(&gate.level, &format!("gates[{index}].level"))?;
        let value = match gate.value {
            serde_json::Value::Null => None,
            serde_json::Value::Number(number) => Some(number.as_f64().ok_or_else(|| {
                format!("quality JSON gates[{index}].value is outside the f64 range")
            })?),
            _ => {
                return Err(format!(
                    "quality JSON gates[{index}].value must be a number or null"
                ))
            }
        };
        gates.push(Gate {
            metric: gate.metric,
            value,
            level: gate.level,
        });
    }

    let exists = |name: &str| {
        let path = dir.join(name);
        path.exists().then(|| path.to_string_lossy().into_owned())
    };
    let geometry = parsed.geometry;
    for (field, value) in [
        ("cell_count", geometry.cell_count),
        ("vertex_count", geometry.vertex_count),
        ("edge_count", geometry.edge_count),
    ] {
        if value < 0 {
            return Err(format!(
                "quality JSON geometry.{field} must be non-negative"
            ));
        }
    }
    let topology = parsed.topology;
    let cell_sides = [
        ("triangle", topology.triangle_cell_count),
        ("quadrilateral", topology.quadrilateral_cell_count),
        ("pentagon", topology.pentagon_cell_count),
        ("hexagon", topology.hexagon_cell_count),
        ("heptagon", topology.heptagon_cell_count),
        ("other", topology.other_polygon_cell_count),
    ]
    .into_iter()
    .map(|(name, count)| (name.to_string(), count))
    .collect();
    let topology_issues = [
        ("duplicate_edge", topology.duplicate_edge_count),
        ("dangling_edge", topology.dangling_edge_count),
        ("orphan_cell", topology.orphan_cell_count),
        (
            "neighbor_reciprocity_failure",
            topology.neighbor_reciprocity_failure_count,
        ),
        (
            "abnormal_polygon_edge",
            topology.abnormal_polygon_edge_count,
        ),
        (
            "isolated_refined_cell",
            topology.isolated_refined_cell_count,
        ),
        (
            "transition_continuity_warning",
            topology.transition_continuity_warning_count,
        ),
        ("invalid_vertex_index", topology.invalid_vertex_index_count),
        ("invalid_cell_index", topology.invalid_cell_index_count),
    ]
    .into_iter()
    .map(|(name, count)| (name.to_string(), count))
    .collect();

    Ok(MeshQuality {
        verdict: parsed.verdict,
        cell_view: parsed.cell_view,
        cell_count: geometry.cell_count,
        vertex_count: geometry.vertex_count,
        edge_count: geometry.edge_count,
        min_angle_deg: geometry.min_angle_deg,
        max_angle_deg: geometry.max_angle_deg,
        cell_area: geometry.cell_area,
        edge_length_km: geometry.edge_length_km,
        aspect_ratio: geometry.aspect_ratio,
        compactness: geometry.compactness,
        zero_area: geometry.zero_area_cell_count,
        negative_area: geometry.negative_area_cell_count,
        self_intersection: geometry.self_intersection_count,
        invalid_polygon: geometry.invalid_polygon_count,
        max_adjacent_resolution_ratio: topology.max_adjacent_resolution_ratio,
        cell_sides,
        topology: topology_issues,
        gates,
        report_path: exists("quality_report.md"),
        worst_cells_path: exists("worst_cells.geojson"),
    })
}
