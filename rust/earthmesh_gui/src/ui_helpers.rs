//! Pure (no-egui) GUI helpers: quality-dashboard model that reads the existing
//! `quality_summary.json` / `run_manifest.json` (read-only, no schema change), the
//! target-template catalog, workflow steps, and tooltip text. Fully unit-testable.
//!
//! Staged API: the workflow-step model is wired incrementally as the stepper nav
//! lands, so allow dead_code here.
#![allow(dead_code)]

use std::path::Path;

// ----------------------------- minimal JSON field scan -----------------------------

/// Value of a `"key": "string"` field (first match), unescaped minimally.
pub fn json_string(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let start = text.find(&needle)? + needle.len();
    let rest = &text[start..];
    let colon = rest.find(':')?;
    let after = rest[colon + 1..].trim_start();
    let after = after.strip_prefix('"')?;
    let end = after.find('"')?;
    Some(after[..end].replace("\\\"", "\"").replace("\\\\", "\\"))
}

/// Value of a `"key": number` field (first match).
pub fn json_number(text: &str, key: &str) -> Option<f64> {
    let needle = format!("\"{key}\"");
    let start = text.find(&needle)? + needle.len();
    let rest = &text[start..];
    let colon = rest.find(':')?;
    let after = rest[colon + 1..].trim_start();
    let end = after
        .find(|c: char| {
            !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == 'e' || c == 'E')
        })
        .unwrap_or(after.len());
    after[..end].parse().ok()
}

/// Strings inside the `"key": [ ... ]` array (first match).
pub fn json_string_array(text: &str, key: &str) -> Vec<String> {
    let needle = format!("\"{key}\"");
    let Some(pos) = text.find(&needle) else {
        return Vec::new();
    };
    let rest = &text[pos + needle.len()..];
    let Some(open) = rest.find('[') else {
        return Vec::new();
    };
    let Some(close) = rest[open..].find(']') else {
        return Vec::new();
    };
    let body = &rest[open + 1..open + close];
    let mut out = Vec::new();
    let mut chars = body.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c == '"' {
            if let Some(endrel) = body[i + 1..].find('"') {
                out.push(body[i + 1..i + 1 + endrel].to_string());
                // skip to after the closing quote
                while let Some(&(j, _)) = chars.peek() {
                    if j > i + 1 + endrel {
                        break;
                    }
                    chars.next();
                }
            }
        }
    }
    out
}

/// `(key1, key2)` value pairs from objects in an array, e.g. gate metric+level.
pub fn json_pairs(text: &str, k1: &str, k2: &str) -> Vec<(String, String)> {
    let n1 = format!("\"{k1}\"");
    let mut out = Vec::new();
    let mut cursor = 0;
    while let Some(rel) = text[cursor..].find(&n1) {
        let abs = cursor + rel;
        let v1 = json_string(&text[abs..], k1);
        let v2 = json_string(&text[abs..], k2);
        if let (Some(a), Some(b)) = (v1, v2) {
            out.push((a, b));
        }
        cursor = abs + n1.len();
    }
    out
}

// ----------------------------- quality dashboard model -----------------------------

#[derive(Clone, Debug, Default, PartialEq)]
pub struct QualityDashboard {
    /// pass / warn / fail / unknown
    pub verdict: String,
    pub headline: Vec<(String, String)>,
    pub top_warnings: Vec<String>,
    pub worst_cells_path: Option<String>,
    pub quality_report_path: Option<String>,
    pub manifest_status: Option<String>,
    pub manifest_warnings: Vec<String>,
    pub next_steps: Vec<String>,
    pub has_quality: bool,
}

impl QualityDashboard {
    /// Build from the JSON texts (None = file absent).
    pub fn parse(quality_text: Option<&str>, manifest_text: Option<&str>) -> QualityDashboard {
        let mut d = QualityDashboard::default();

        if let Some(q) = quality_text {
            d.has_quality = true;
            d.verdict = json_string(q, "verdict").unwrap_or_else(|| "unknown".into());
            for key in ["cell_count", "vertex_count", "edge_count", "min_angle_deg"] {
                if let Some(v) = json_number(q, key) {
                    d.headline.push((key.to_string(), trim_num(v)));
                }
            }
            // gate metrics whose level is warn/fail
            for (metric, level) in json_pairs(q, "metric", "level") {
                if level == "warn" || level == "fail" {
                    d.top_warnings.push(format!("{metric} [{level}]"));
                }
            }
            // topology issues (type + severity)
            for (kind, sev) in json_pairs(q, "issue_type", "severity") {
                d.top_warnings.push(format!("topology: {kind} [{sev}]"));
            }
        } else {
            d.verdict = "unknown".into();
        }

        if let Some(m) = manifest_text {
            d.manifest_status = json_string(m, "status");
            d.manifest_warnings = json_string_array(m, "warnings");
        }

        d.next_steps = d.compute_next_steps();
        d
    }

    /// Build from a run output directory (reads the standard artifact filenames).
    pub fn from_dir(dir: &Path) -> QualityDashboard {
        let read = |name: &str| std::fs::read_to_string(dir.join(name)).ok();
        let quality = read("quality_summary.json");
        let manifest = read("run_manifest.json");
        let mut d = QualityDashboard::parse(quality.as_deref(), manifest.as_deref());
        let exists = |name: &str| {
            let p = dir.join(name);
            p.exists().then(|| p.display().to_string())
        };
        d.worst_cells_path = exists("worst_cells.geojson");
        d.quality_report_path = exists("quality_report.md");
        d
    }

    fn compute_next_steps(&self) -> Vec<String> {
        let mut steps = Vec::new();
        if !self.has_quality {
            steps.push(
                "Run `earthmesh_cli --mesh-quality <gridfile>` to produce a quality report.".into(),
            );
            return steps;
        }
        match self.verdict.as_str() {
            "fail" => steps.push(
                "FAIL: fix catastrophic topology/geometry issues (see worst cells) before using the mesh.".into(),
            ),
            "warn" => steps.push(
                "WARN: review the listed warnings; consider tuning refinement or thresholds.".into(),
            ),
            "pass" => steps.push("PASS: mesh quality gates passed.".into()),
            _ => steps.push("Quality verdict unknown — re-run the quality check.".into()),
        }
        if self.worst_cells_path.is_some() {
            steps.push("Inspect worst_cells.geojson on the map to locate the worst cells.".into());
        }
        if !self.manifest_warnings.is_empty() {
            steps.push("Address run_manifest warnings (e.g. missing inputs).".into());
        }
        steps
    }
}

fn trim_num(v: f64) -> String {
    if (v.fract()).abs() < 1e-9 {
        format!("{}", v as i64)
    } else {
        format!("{v:.3}")
    }
}

// ------------------------- coupling-quality / refinement-plan -------------------------

/// Read-only summary of `coupling_quality.json` (R7 land/ocean coupling validator).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CouplingQualitySummary {
    pub present: bool,
    /// pass / warn / fail / unknown
    pub verdict: String,
    pub fields: Vec<(String, String)>,
}

impl CouplingQualitySummary {
    pub fn parse(text: &str) -> CouplingQualitySummary {
        let mut s = CouplingQualitySummary {
            present: true,
            verdict: json_string(text, "verdict").unwrap_or_else(|| "unknown".into()),
            fields: Vec::new(),
        };
        for key in [
            "total_land_cells",
            "total_ocean_cells",
            "mixed_coastline_cells",
            "coast_overlap_cells",
            "orphan_land_cells",
            "orphan_ocean_cells",
            "coastline_preservation_score",
            "river_ocean_connectivity_score",
        ] {
            if let Some(v) = json_number(text, key) {
                s.fields.push((key.to_string(), trim_num(v)));
            }
        }
        s
    }

    pub fn from_dir(dir: &Path) -> CouplingQualitySummary {
        std::fs::read_to_string(dir.join("coupling_quality.json"))
            .ok()
            .map(|t| CouplingQualitySummary::parse(&t))
            .unwrap_or_default()
    }
}

/// Read-only summary of `refinement_plan.json` (R8 hydro-driven target_level plan).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RefinementPlanSummary {
    pub present: bool,
    pub fields: Vec<(String, String)>,
}

impl RefinementPlanSummary {
    pub fn parse(text: &str) -> RefinementPlanSummary {
        let mut s = RefinementPlanSummary {
            present: true,
            fields: Vec::new(),
        };
        for key in ["total_cells", "cells_refined", "max_level"] {
            if let Some(v) = json_number(text, key) {
                s.fields.push((key.to_string(), trim_num(v)));
            }
        }
        s
    }

    pub fn from_dir(dir: &Path) -> RefinementPlanSummary {
        std::fs::read_to_string(dir.join("refinement_plan.json"))
            .ok()
            .map(|t| RefinementPlanSummary::parse(&t))
            .unwrap_or_default()
    }
}

// ----------------------------- target templates -----------------------------

/// A target-mesh starting template. Plain fields so this module stays core-free; the
/// app applies them to `EarthmeshConfig`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TargetTemplate {
    pub id: &'static str,
    /// i18n key for the display name.
    pub name_key: &'static str,
    pub mesh_type: &'static str,
    pub mode_grid: &'static str,
    pub output_format: &'static str,
    pub global: bool,
    pub default_nxp: i32,
    pub refine: bool,
    /// i18n key for a one-line description.
    pub help_key: &'static str,
}

pub fn target_templates() -> &'static [TargetTemplate] {
    &[
        TargetTemplate {
            id: "global_atmos",
            name_key: "tpl.global_atmos",
            mesh_type: "atmosmesh",
            mode_grid: "hex",
            output_format: "MPAS",
            global: true,
            default_nxp: 64,
            refine: false,
            help_key: "tpl.global_atmos.help",
        },
        TargetTemplate {
            id: "regional_land",
            name_key: "tpl.regional_land",
            mesh_type: "landmesh",
            mode_grid: "hex",
            output_format: "CoLM",
            global: false,
            default_nxp: 64,
            refine: true,
            help_key: "tpl.regional_land.help",
        },
        TargetTemplate {
            id: "regional_ocean",
            name_key: "tpl.regional_ocean",
            mesh_type: "oceanmesh",
            mode_grid: "tri",
            output_format: "FVCOM",
            global: false,
            default_nxp: 64,
            refine: true,
            help_key: "tpl.regional_ocean.help",
        },
        TargetTemplate {
            id: "coupled",
            name_key: "tpl.coupled",
            mesh_type: "LOCmesh",
            mode_grid: "hex",
            output_format: "CoLM",
            global: false,
            default_nxp: 64,
            refine: true,
            help_key: "tpl.coupled.help",
        },
        TargetTemplate {
            id: "merit_hydro",
            name_key: "tpl.merit_hydro",
            mesh_type: "LOCmesh",
            mode_grid: "hex",
            output_format: "CoLM",
            global: false,
            default_nxp: 112,
            refine: true,
            help_key: "tpl.merit_hydro.help",
        },
        TargetTemplate {
            id: "estuary",
            name_key: "tpl.estuary",
            mesh_type: "oceanmesh",
            mode_grid: "tri",
            output_format: "FVCOM",
            global: false,
            default_nxp: 112,
            refine: true,
            help_key: "tpl.estuary.help",
        },
        TargetTemplate {
            id: "hydrology_land",
            name_key: "tpl.hydrology_land",
            mesh_type: "landmesh",
            mode_grid: "hex",
            output_format: "CoLM",
            global: false,
            default_nxp: 96,
            refine: true,
            help_key: "tpl.hydrology_land.help",
        },
        TargetTemplate {
            id: "urban_land",
            name_key: "tpl.urban_land",
            mesh_type: "landmesh",
            mode_grid: "hex",
            output_format: "CoLM",
            global: false,
            default_nxp: 96,
            refine: true,
            help_key: "tpl.urban_land.help",
        },
        TargetTemplate {
            id: "orographic_atmos",
            name_key: "tpl.orographic_atmos",
            mesh_type: "atmosmesh",
            mode_grid: "hex",
            output_format: "MPAS",
            global: false,
            default_nxp: 96,
            refine: true,
            help_key: "tpl.orographic_atmos.help",
        },
    ]
}

// ----------------------------- workflow steps -----------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkflowStep {
    NewProject,
    Target,
    Domain,
    Data,
    Strategy,
    Quality,
    RunResults,
}

pub fn workflow_steps() -> &'static [(WorkflowStep, &'static str)] {
    &[
        (WorkflowStep::NewProject, "step.new_project"),
        (WorkflowStep::Target, "step.target"),
        (WorkflowStep::Domain, "step.domain"),
        (WorkflowStep::Data, "step.data"),
        (WorkflowStep::Strategy, "step.strategy"),
        (WorkflowStep::Quality, "step.quality"),
        (WorkflowStep::RunResults, "step.run_results"),
    ]
}

// ----------------------------- tooltips -----------------------------

/// Help text for a UI concept (English; the GUI can localize via i18n keys too).
pub fn tooltip(key: &str) -> &'static str {
    match key {
        "nxp" => "NXP: number of grid points spanning one icosahedral triangle edge. Higher = finer base mesh (more cells).",
        "refinement_strategy" => "How cells are selected for refinement: specified regions, feature thresholds, or (future) composite score.",
        "threshold" => "A feature value above which a cell is refined (e.g. LAI std, slope). Units depend on the field.",
        "quality_status" => "Overall mesh quality verdict: PASS (gates met), WARN (suspicious), FAIL (catastrophic topology/geometry).",
        "merit_root" => "Folder holding MERIT-Hydro tiles (5°×5°, e.g. n10e100.nc). Set EARTHMESH_DATA or point here for hydro/coast meshes.",
        "mesh_target" => "What you are meshing: land, ocean, atmosphere, or land-ocean coupled (LOCmesh, CoLM output).",
        "score_based_refinement" => "Planned: a weighted composite score across criteria assigns target levels under a cell budget (skeleton in earthmesh_refine_planner).",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUALITY_JSON: &str = r#"{
  "kind": "earthmesh_mesh_quality",
  "verdict": "warn",
  "geometry": { "cell_count": 5120, "vertex_count": 2563, "edge_count": 7680, "min_angle_deg": 53.4 },
  "gates": [
    {"metric": "min_angle_deg", "value": 53.4, "level": "pass"},
    {"metric": "cell_area_cv", "value": 1.9, "level": "warn"}
  ],
  "topology_issues": [
    {"issue_type": "orphan_cell", "severity": "fail", "cell_id": 7}
  ]
}"#;

    const MANIFEST_JSON: &str = r#"{
  "kind": "earthmesh_run_manifest",
  "status": "completed",
  "warnings": ["merit_root not set; hydro masks skipped"]
}"#;

    #[test]
    fn json_field_extractors() {
        assert_eq!(
            json_string(QUALITY_JSON, "verdict").as_deref(),
            Some("warn")
        );
        assert_eq!(json_number(QUALITY_JSON, "cell_count"), Some(5120.0));
        assert_eq!(
            json_string(MANIFEST_JSON, "status").as_deref(),
            Some("completed")
        );
        let w = json_string_array(MANIFEST_JSON, "warnings");
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("merit_root"));
    }

    #[test]
    fn dashboard_parses_verdict_warnings_and_next_steps() {
        let d = QualityDashboard::parse(Some(QUALITY_JSON), Some(MANIFEST_JSON));
        assert_eq!(d.verdict, "warn");
        assert!(d.headline.iter().any(|(k, _)| k == "cell_count"));
        assert!(d.top_warnings.iter().any(|w| w.contains("cell_area_cv")));
        assert!(d.top_warnings.iter().any(|w| w.contains("orphan_cell")));
        assert_eq!(d.manifest_status.as_deref(), Some("completed"));
        assert!(!d.manifest_warnings.is_empty());
        assert!(!d.next_steps.is_empty());
    }

    #[test]
    fn dashboard_missing_quality_suggests_running_check() {
        let d = QualityDashboard::parse(None, None);
        assert_eq!(d.verdict, "unknown");
        assert!(!d.has_quality);
        assert!(d.next_steps.iter().any(|s| s.contains("mesh-quality")));
    }

    #[test]
    fn nine_target_templates_present() {
        let t = target_templates();
        assert_eq!(t.len(), 9);
        assert!(t
            .iter()
            .any(|t| t.id == "merit_hydro" && t.mesh_type == "LOCmesh"));
        assert!(t.iter().any(|t| t.id == "global_atmos" && t.global));
    }

    #[test]
    fn tooltips_present_for_required_keys() {
        for key in [
            "nxp",
            "refinement_strategy",
            "threshold",
            "quality_status",
            "merit_root",
            "mesh_target",
            "score_based_refinement",
        ] {
            assert!(!tooltip(key).is_empty(), "missing tooltip for {key}");
        }
        assert!(tooltip("unknown_key").is_empty());
    }

    #[test]
    fn seven_workflow_steps() {
        assert_eq!(workflow_steps().len(), 7);
    }

    #[test]
    fn coupling_quality_summary_parses_verdict_and_counts() {
        let json = r#"{
  "kind": "earthmesh_coupling_quality",
  "verdict": "warn",
  "total_land_cells": 6,
  "total_ocean_cells": 3,
  "mixed_coastline_cells": 3,
  "coast_overlap_cells": 3,
  "orphan_land_cells": 0,
  "orphan_ocean_cells": 0,
  "coastline_preservation_score": 1
}"#;
        let s = CouplingQualitySummary::parse(json);
        assert!(s.present);
        assert_eq!(s.verdict, "warn");
        assert!(s
            .fields
            .contains(&("total_land_cells".to_string(), "6".to_string())));
        assert!(s
            .fields
            .contains(&("mixed_coastline_cells".to_string(), "3".to_string())));
    }

    #[test]
    fn refinement_plan_summary_parses_counts() {
        let json = r#"{
  "kind": "earthmesh_refinement_plan",
  "total_cells": 3,
  "cells_refined": 2,
  "max_level": 3,
  "level_histogram": {"0": 1, "2": 1, "3": 1}
}"#;
        let s = RefinementPlanSummary::parse(json);
        assert!(s.present);
        assert!(s
            .fields
            .contains(&("cells_refined".to_string(), "2".to_string())));
        assert!(s
            .fields
            .contains(&("max_level".to_string(), "3".to_string())));
    }
}
