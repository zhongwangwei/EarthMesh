//! Quality summary parsing and serializable quality report DTOs.

use serde::Serialize;
use std::path::Path;

/// Distribution summary for a metric (mean ± std over [min, max]).
#[derive(Serialize, Clone, Copy)]
pub(crate) struct Stat {
    pub(crate) min: f64,
    pub(crate) max: f64,
    pub(crate) mean: f64,
    pub(crate) std: f64,
    pub(crate) cv: f64,
}

/// One quality gate: a metric value + its pass/warn/fail level.
#[derive(Serialize)]
pub(crate) struct Gate {
    pub(crate) metric: String,
    pub(crate) value: f64,
    pub(crate) level: String,
}

/// A mesh-quality summary for the dashboard, parsed from `quality_summary.json`.
#[derive(Serialize)]
pub(crate) struct MeshQuality {
    pub(crate) verdict: String,
    pub(crate) cell_count: i64,
    pub(crate) vertex_count: i64,
    pub(crate) edge_count: i64,
    pub(crate) min_angle_deg: f64,
    pub(crate) max_angle_deg: f64,
    // Per-metric distribution summaries (for the box/range charts).
    pub(crate) cell_area: Stat,
    pub(crate) edge_length_km: Stat,
    pub(crate) aspect_ratio: Stat,
    pub(crate) compactness: Stat,
    // Degeneracy counts.
    pub(crate) zero_area: i64,
    pub(crate) negative_area: i64,
    pub(crate) self_intersection: i64,
    pub(crate) invalid_polygon: i64,
    pub(crate) max_adjacent_resolution_ratio: f64,
    /// (name, count) for each topology issue counter.
    pub(crate) topology: Vec<(String, i64)>,
    pub(crate) gates: Vec<Gate>,
    pub(crate) report_path: Option<String>,
    pub(crate) worst_cells_path: Option<String>,
}

impl Stat {
    fn from_json(value: &serde_json::Value, key: &str) -> Self {
        let s = &value[key];
        Self {
            min: s["min"].as_f64().unwrap_or(0.0),
            max: s["max"].as_f64().unwrap_or(0.0),
            mean: s["mean"].as_f64().unwrap_or(0.0),
            std: s["std"].as_f64().unwrap_or(0.0),
            cv: s["cv"].as_f64().unwrap_or(0.0),
        }
    }
}

/// Parse `quality_summary.json` text into a [`MeshQuality`]. `dir` is the report dir,
/// used to locate the .md / worst-cells artifacts written alongside it.
pub(crate) fn parse_quality_summary(text: &str, dir: &Path) -> Result<MeshQuality, String> {
    let v: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("parse quality json: {e}"))?;
    let geom = &v["geometry"];
    let exists = |name: &str| {
        let p = dir.join(name);
        p.exists().then(|| p.to_string_lossy().into_owned())
    };
    let gates = v["gates"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|g| Gate {
                    metric: g["metric"].as_str().unwrap_or("?").to_string(),
                    value: g["value"].as_f64().unwrap_or(0.0),
                    level: g["level"].as_str().unwrap_or("").to_string(),
                })
                .collect()
        })
        .unwrap_or_default();
    let topology = [
        "duplicate_edge_count",
        "dangling_edge_count",
        "orphan_cell_count",
        "neighbor_reciprocity_failure_count",
        "abnormal_polygon_edge_count",
        "isolated_refined_cell_count",
        "transition_continuity_warning_count",
        "invalid_vertex_index_count",
        "invalid_cell_index_count",
    ]
    .iter()
    .map(|k| {
        (
            k.trim_end_matches("_count").to_string(),
            v["topology"][*k].as_i64().unwrap_or(0),
        )
    })
    .collect::<Vec<_>>();
    Ok(MeshQuality {
        verdict: v["verdict"].as_str().unwrap_or("unknown").to_string(),
        cell_count: geom["cell_count"].as_i64().unwrap_or(0),
        vertex_count: geom["vertex_count"].as_i64().unwrap_or(0),
        edge_count: geom["edge_count"].as_i64().unwrap_or(0),
        min_angle_deg: geom["min_angle_deg"].as_f64().unwrap_or(0.0),
        max_angle_deg: geom["max_angle_deg"].as_f64().unwrap_or(0.0),
        cell_area: Stat::from_json(geom, "cell_area"),
        edge_length_km: Stat::from_json(geom, "edge_length_km"),
        aspect_ratio: Stat::from_json(geom, "aspect_ratio"),
        compactness: Stat::from_json(geom, "compactness"),
        zero_area: geom["zero_area_cell_count"].as_i64().unwrap_or(0),
        negative_area: geom["negative_area_cell_count"].as_i64().unwrap_or(0),
        self_intersection: geom["self_intersection_count"].as_i64().unwrap_or(0),
        invalid_polygon: geom["invalid_polygon_count"].as_i64().unwrap_or(0),
        max_adjacent_resolution_ratio: v["topology"]["max_adjacent_resolution_ratio"]
            .as_f64()
            .unwrap_or(0.0),
        topology,
        gates,
        report_path: exists("quality_report.md"),
        worst_cells_path: exists("worst_cells.geojson"),
    })
}
