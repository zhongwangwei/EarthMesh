use std::{
    fs,
    path::{Path, PathBuf},
};

use earthmesh_project::{MeshCellKind, ProjectConfig};

/// Which mesh survives comparison of an AutoRefine candidate with its baseline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoRefineCandidateSelection {
    Baseline,
    Candidate,
}

/// Central candidate classifier shared by the normal CLI and Hydro loops.
/// Keeping this decision in one place prevents either loop from accidentally
/// accepting an equal or regressing candidate.
pub fn select_auto_refine_candidate(
    baseline: &earthmesh_quality::MeshQualityReport,
    candidate: &earthmesh_quality::MeshQualityReport,
) -> AutoRefineCandidateSelection {
    if candidate.is_strict_improvement_over(baseline) {
        AutoRefineCandidateSelection::Candidate
    } else {
        AutoRefineCandidateSelection::Baseline
    }
}

/// Durable record of why an AutoRefine candidate was selected or rejected.
pub struct AutoRefineDecision<'a> {
    pub pass: u8,
    pub decision: &'a str,
    pub reason: &'a str,
    pub regressions: &'a [earthmesh_quality::QualityMetricRegression],
    pub baseline_gridfile: Option<&'a Path>,
    pub candidate_gridfile: &'a Path,
    pub selected_gridfile: &'a Path,
    pub baseline_quality_report: Option<&'a Path>,
    pub candidate_quality_report: &'a Path,
    pub selected_quality_report: &'a Path,
    pub baseline_verdict: Option<earthmesh_quality::QualityLevel>,
    pub candidate_verdict: earthmesh_quality::QualityLevel,
    pub selected_verdict: earthmesh_quality::QualityLevel,
}

/// Write the machine-readable selection decision next to its quality report.
pub fn write_auto_refine_decision(
    out_dir: &Path,
    decision: &AutoRefineDecision<'_>,
) -> Result<PathBuf, String> {
    fn optional_path(path: Option<&Path>) -> String {
        path.map(|path| {
            format!(
                "\"{}\"",
                crate::json_escape_string(&path.display().to_string())
            )
        })
        .unwrap_or_else(|| "null".to_string())
    }
    fn optional_verdict(verdict: Option<earthmesh_quality::QualityLevel>) -> String {
        verdict
            .map(|verdict| format!("\"{}\"", verdict.as_str()))
            .unwrap_or_else(|| "null".to_string())
    }
    fn regressions_json(items: &[earthmesh_quality::QualityMetricRegression]) -> String {
        if items.is_empty() {
            return "[]".to_string();
        }
        let rows = items
            .iter()
            .map(|item| {
                format!(
                    "    {{\"metric\": \"{}\", \"preferred\": \"{}\", \"baseline\": {}, \"candidate\": {}, \"delta\": {}}}",
                    crate::json_escape_string(&item.metric),
                    item.preference.as_str(),
                    crate::json_number(item.baseline),
                    crate::json_number(item.candidate),
                    crate::json_number(item.delta()),
                )
            })
            .collect::<Vec<_>>()
            .join(",\n");
        format!("[\n{rows}\n  ]")
    }

    fs::create_dir_all(out_dir).map_err(|error| {
        format!(
            "auto_refine create decision directory {}: {error}",
            out_dir.display()
        )
    })?;
    let path = out_dir.join("auto_refine_decision.json");
    fs::write(
        &path,
        format!(
            "{{\n  \"schema_version\": 1,\n  \"kind\": \"earthmesh_auto_refine_decision\",\n  \"pass\": {},\n  \"decision\": \"{}\",\n  \"reason\": \"{}\",\n  \"regressions\": {},\n  \"baseline_gridfile\": {},\n  \"candidate_gridfile\": \"{}\",\n  \"selected_gridfile\": \"{}\",\n  \"baseline_quality_report\": {},\n  \"candidate_quality_report\": \"{}\",\n  \"selected_quality_report\": \"{}\",\n  \"baseline_verdict\": {},\n  \"candidate_verdict\": \"{}\",\n  \"selected_verdict\": \"{}\"\n}}\n",
            decision.pass,
            crate::json_escape_string(decision.decision),
            crate::json_escape_string(decision.reason),
            regressions_json(decision.regressions),
            optional_path(decision.baseline_gridfile),
            crate::json_escape_string(&decision.candidate_gridfile.display().to_string()),
            crate::json_escape_string(&decision.selected_gridfile.display().to_string()),
            optional_path(decision.baseline_quality_report),
            crate::json_escape_string(&decision.candidate_quality_report.display().to_string()),
            crate::json_escape_string(&decision.selected_quality_report.display().to_string()),
            optional_verdict(decision.baseline_verdict),
            decision.candidate_verdict.as_str(),
            decision.selected_verdict.as_str(),
        ),
    )
    .map_err(|error| format!("auto_refine write {}: {error}", path.display()))?;
    Ok(path)
}

/// Compute and persist the quality report for a Project-produced gridfile.
///
/// Unlike standalone `--mesh-quality`, this carries the Project topology
/// context. CLI and GUI orchestration must use this entry point so Block and
/// AutoRefine observe the same verdict.
pub fn write_project_quality_report(
    project: &ProjectConfig,
    gridfile: &Path,
    out_dir: &Path,
) -> Result<earthmesh_quality::MeshQualityReport, String> {
    write_project_quality_report_with_namelist(project, gridfile, out_dir, None)
}

/// Project quality with the exact engine namelist used to build `gridfile`.
/// Supplying it attaches target-vs-actual HField diagnostics to the same report
/// that controls the closed-loop verdict.
pub fn write_project_quality_report_with_namelist(
    project: &ProjectConfig,
    gridfile: &Path,
    out_dir: &Path,
    engine_namelist: Option<&Path>,
) -> Result<earthmesh_quality::MeshQualityReport, String> {
    let mesh = crate::grid_quality_pipeline::read_gridfile_mesh_points(gridfile)
        .map_err(|err| format!("project quality read {}: {err}", gridfile.display()))?;
    let input = match project.target.cell {
        MeshCellKind::Hex => crate::grid_quality_pipeline::quality_input_from_gridfile_hex(&mesh),
        MeshCellKind::Tri => crate::grid_quality_pipeline::quality_input_from_gridfile(&mesh),
    }
    .map_err(|err| format!("project quality validate {}: {err}", gridfile.display()))?;
    let target_nxp = project.try_lower()?.mkgrd.nxp;
    let repair_level_cap = earthmesh_project::auto_refine_level_cap(target_nxp);
    let thresholds = earthmesh_quality::QualityThresholds {
        min_angle_warn_deg: project.quality.min_angle_deg,
        repair_batch_limit: project.quality.auto_refine_batch_cells,
        repair_level_cap: Some(u32::from(repair_level_cap)),
        ..earthmesh_quality::QualityThresholds::default()
    };
    let mut report = earthmesh_quality::compute_with_options(
        &input,
        &thresholds,
        earthmesh_quality::QualityComputationOptions {
            expected_euler_characteristic: project.expected_euler_characteristic(),
        },
    );
    report.mesh_name = gridfile.display().to_string();
    let cell_view = match project.target.cell {
        MeshCellKind::Hex => "hex",
        MeshCellKind::Tri => "tri",
    };
    report.cell_view = cell_view.to_string();
    if let Some(path) = engine_namelist {
        let namelist = fs::read_to_string(path)
            .map_err(|err| format!("project quality read {}: {err}", path.display()))?;
        crate::grid_quality_pipeline::attach_hfield_diagnostics_from_namelist(
            &mut report,
            &input,
            &mesh,
            cell_view,
            &namelist,
        )
        .map_err(|err| format!("project quality attach h-field diagnostics: {err}"))?;
    }
    earthmesh_quality::io::write_all(&report, out_dir)
        .map_err(|err| format!("project quality write report: {err}"))?;
    fs::write(
        out_dir.join("quality_repair_plan.json"),
        earthmesh_quality::io::to_quality_repair_plan_json_capped(&report, repair_level_cap),
    )
    .map_err(|err| format!("project quality write capped repair plan: {err}"))?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use earthmesh_geometry::Point;
    use earthmesh_quality::{QualityCell, QualityMeshInput, QualityThresholds};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn baseline_quality() -> earthmesh_quality::MeshQualityReport {
        earthmesh_quality::compute(
            &QualityMeshInput {
                vertices: vec![
                    Point::new(0.0, 0.0),
                    Point::new(1.0, 0.0),
                    Point::new(1.0, 1.0),
                    Point::new(0.0, 1.0),
                    Point::new(2.0, 0.0),
                    Point::new(2.0, 1.0),
                ],
                cells: vec![
                    QualityCell {
                        vertices: vec![0, 1, 2, 3],
                        refine_level: Some(0),
                        neighbors: vec![1],
                    },
                    QualityCell {
                        vertices: vec![1, 4, 5, 2],
                        refine_level: Some(0),
                        neighbors: vec![0],
                    },
                ],
            },
            &QualityThresholds::default(),
        )
    }

    #[test]
    fn candidate_selection_keeps_baseline_for_equal_or_regressing_meshes() {
        let baseline = baseline_quality();
        assert_eq!(
            select_auto_refine_candidate(&baseline, &baseline),
            AutoRefineCandidateSelection::Baseline
        );

        let mut regressing = baseline.clone();
        regressing.geometry.cell_edge_length_cv.max += 0.01;
        assert_eq!(
            select_auto_refine_candidate(&baseline, &regressing),
            AutoRefineCandidateSelection::Baseline
        );
    }

    #[test]
    fn candidate_selection_accepts_only_strict_improvement() {
        let baseline = baseline_quality();
        let mut improved = baseline.clone();
        improved.geometry.aspect_ratio.max *= 0.9;
        assert_eq!(
            select_auto_refine_candidate(&baseline, &improved),
            AutoRefineCandidateSelection::Candidate
        );

        let mut failed = baseline;
        failed.verdict = earthmesh_quality::QualityLevel::Fail;
        failed
            .gates
            .iter_mut()
            .find(|gate| gate.metric == "aspect_ratio_max")
            .unwrap()
            .level = earthmesh_quality::QualityLevel::Fail;
        let mut warned = failed.clone();
        warned.verdict = earthmesh_quality::QualityLevel::Warn;
        warned
            .gates
            .iter_mut()
            .find(|gate| gate.metric == "aspect_ratio_max")
            .unwrap()
            .level = earthmesh_quality::QualityLevel::Warn;
        assert_eq!(
            select_auto_refine_candidate(&failed, &warned),
            AutoRefineCandidateSelection::Candidate
        );
    }

    #[test]
    fn decision_manifest_records_candidate_and_selected_meshes() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "earthmesh_auto_refine_decision_{}_{}",
            std::process::id(),
            nonce
        ));
        let baseline = root.join("baseline/gridfile.grid");
        let candidate = root.join("candidate/gridfile.grid");
        let baseline_quality = root.join("reports/baseline/quality_summary.json");
        let candidate_quality = root.join("reports/candidate/quality_summary.json");
        let regressions = [earthmesh_quality::QualityMetricRegression {
            metric: "aspect_ratio.max".to_string(),
            preference: earthmesh_quality::QualityMetricPreference::Lower,
            baseline: 3.0,
            candidate: 3.5,
        }];
        let path = write_auto_refine_decision(
            &root,
            &AutoRefineDecision {
                pass: 2,
                decision: "rejected",
                reason: "candidate regressed",
                regressions: &regressions,
                baseline_gridfile: Some(&baseline),
                candidate_gridfile: &candidate,
                selected_gridfile: &baseline,
                baseline_quality_report: Some(&baseline_quality),
                candidate_quality_report: &candidate_quality,
                selected_quality_report: &baseline_quality,
                baseline_verdict: Some(earthmesh_quality::QualityLevel::Warn),
                candidate_verdict: earthmesh_quality::QualityLevel::Fail,
                selected_verdict: earthmesh_quality::QualityLevel::Warn,
            },
        )
        .expect("write decision manifest");

        let json = fs::read_to_string(path).expect("read decision manifest");
        assert!(json.contains("\"schema_version\": 1"));
        assert!(json.contains("\"decision\": \"rejected\""));
        assert!(json.contains(&format!(
            "\"candidate_gridfile\": \"{}\"",
            candidate.display()
        )));
        assert!(json.contains(&format!(
            "\"selected_gridfile\": \"{}\"",
            baseline.display()
        )));
        assert!(json.contains(&format!(
            "\"candidate_quality_report\": \"{}\"",
            candidate_quality.display()
        )));
        assert!(json.contains(&format!(
            "\"selected_quality_report\": \"{}\"",
            baseline_quality.display()
        )));
        assert!(json.contains("\"candidate_verdict\": \"fail\""));
        assert!(json.contains(
            "{\"metric\": \"aspect_ratio.max\", \"preferred\": \"lower\", \"baseline\": 3, \"candidate\": 3.5, \"delta\": 0.5}"
        ));
        let _ = fs::remove_dir_all(root);
    }
}
