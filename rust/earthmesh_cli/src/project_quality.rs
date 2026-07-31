use std::{
    fs,
    path::{Path, PathBuf},
};

use earthmesh_project::{DomainConfig, MeshCellKind, MeshDomainKind, ProjectConfig};

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

const MIN_AUTO_REFINE_HFIELD_G: f64 = 0.01;

fn primary_quality_output_is_masked(kind: MeshDomainKind, domain: &DomainConfig) -> bool {
    matches!(kind, MeshDomainKind::Land | MeshDomainKind::Ocean)
        || matches!(domain, DomainConfig::Regional { .. })
}

/// Return one bounded HField gradation retry for transition-shaped defects.
///
/// Tightening `g` smooths refinement transitions without changing target
/// sources or the maximum refinement level. The caller still measures the
/// candidate and applies the normal strict-improvement guard before accepting
/// it.
pub fn tighter_hfield_gradation_for_quality(
    report: &earthmesh_quality::MeshQualityReport,
    already_attempted: bool,
) -> Option<f64> {
    if already_attempted
        || report.verdict == earthmesh_quality::QualityLevel::Pass
        || report.has_unrepairable_failure()
    {
        return None;
    }
    let config = report.hfield.as_ref()?.config;
    let current = config
        .g
        .filter(|g| config.enabled && g.is_finite() && *g > 0.0)?;
    let has_gradation_defect = report.gates.iter().any(|gate| {
        gate.level != earthmesh_quality::QualityLevel::Pass
            && matches!(
                gate.metric.as_str(),
                "max_adjacent_resolution_ratio"
                    | "transition_continuity_warning_count"
                    | "isolated_refined_cell_count"
                    | "hfield_actual_level_jump_gt_one_count"
            )
    });
    if !has_gradation_defect {
        return None;
    }
    let candidate = (current * 0.5).max(MIN_AUTO_REFINE_HFIELD_G);
    (candidate < current).then_some(candidate)
}

/// Whether adding one local refinement level is a plausible repair.
///
/// A conforming HField whose warnings are limited to transition shape or the
/// expected multi-resolution area spread is not missing resolution. Refining
/// the worst cell creates a deeper transition instead of repairing those
/// metrics.
pub fn should_attempt_local_quality_refinement(
    report: &earthmesh_quality::MeshQualityReport,
) -> bool {
    let non_pass = report
        .gates
        .iter()
        .filter(|gate| gate.level != earthmesh_quality::QualityLevel::Pass)
        .collect::<Vec<_>>();
    if non_pass.is_empty() {
        return false;
    }
    if non_pass.iter().any(|gate| {
        !matches!(
            gate.metric.as_str(),
            "aspect_ratio_max"
                | "cell_edge_length_cv_max"
                | "cell_area_cv"
                | "cell_area_cv_normalized"
                | "cell_area_ratio"
                | "hfield_target_level_jump_gt_one_count"
        )
    }) {
        return true;
    }
    if !has_conforming_hfield(report) {
        return true;
    }
    report
        .repair_cells
        .iter()
        .any(|cell| cell.metric != "cell_edge_length_cv")
}

fn has_conforming_hfield(report: &earthmesh_quality::MeshQualityReport) -> bool {
    report.hfield.as_ref().is_some_and(|hfield| {
        hfield.config.enabled
            && hfield.missing_target_level_count == 0
            && hfield.extra_target_level_count == 0
            && hfield.missing_actual_refine_level_count == 0
            && hfield.target_above_actual_count == 0
            && hfield.actual_level_jump_gt_one_count == 0
    })
}

fn prefer_non_transition_repair(
    report: &mut earthmesh_quality::MeshQualityReport,
    input: &earthmesh_quality::QualityMeshInput,
    thresholds: &earthmesh_quality::QualityThresholds,
) {
    if !has_conforming_hfield(report)
        || report.repair_cells.is_empty()
        || report
            .repair_cells
            .iter()
            .any(|cell| cell.metric != "cell_edge_length_cv")
    {
        return;
    }
    let alternate =
        earthmesh_quality::repair_batch_without_metric(input, thresholds, "cell_edge_length_cv");
    if !alternate.is_empty() {
        report.repair_cells = alternate;
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

/// Project quality with the engine namelist describing the complete target
/// field for `gridfile`. Incremental adapters carry the union of original
/// Project demands and new absolute targets in that same namelist.
pub fn write_project_quality_report_with_namelist(
    project: &ProjectConfig,
    gridfile: &Path,
    out_dir: &Path,
    target_namelist: Option<&Path>,
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
            masked_subset: primary_quality_output_is_masked(project.target.kind, &project.domain),
        },
    );
    report.mesh_name = gridfile.display().to_string();
    let cell_view = match project.target.cell {
        MeshCellKind::Hex => "hex",
        MeshCellKind::Tri => "tri",
    };
    report.cell_view = cell_view.to_string();
    if let Some(path) = target_namelist {
        let namelist = fs::read_to_string(path)
            .map_err(|err| format!("project quality read {}: {err}", path.display()))?;
        crate::grid_quality_pipeline::attach_hfield_diagnostics_from_namelist_for_gridfile(
            &mut report,
            &input,
            &mesh,
            gridfile,
            cell_view,
            &namelist,
        )
        .map_err(|err| format!("project quality attach h-field diagnostics: {err}"))?;
        prefer_non_transition_repair(&mut report, &input, &thresholds);
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
    fn only_carved_primary_outputs_use_masked_quality_semantics() {
        let global = DomainConfig::Global;
        let regional = DomainConfig::Regional {
            shape: earthmesh_project::RegionShape::Bbox {
                w: 0.0,
                e: 1.0,
                n: 1.0,
                s: 0.0,
            },
            sea_ratio: None,
        };

        assert!(primary_quality_output_is_masked(
            MeshDomainKind::Land,
            &global
        ));
        assert!(primary_quality_output_is_masked(
            MeshDomainKind::Ocean,
            &global
        ));
        assert!(!primary_quality_output_is_masked(
            MeshDomainKind::Coupled,
            &global
        ));
        assert!(!primary_quality_output_is_masked(
            MeshDomainKind::Earth,
            &global
        ));
        assert!(!primary_quality_output_is_masked(
            MeshDomainKind::Atmosphere,
            &global
        ));
        for kind in [
            MeshDomainKind::Coupled,
            MeshDomainKind::Earth,
            MeshDomainKind::Atmosphere,
        ] {
            assert!(primary_quality_output_is_masked(kind, &regional));
        }
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

    fn quality_with_hfield_g(g: f64) -> earthmesh_quality::MeshQualityReport {
        let mut quality = baseline_quality();
        quality.hfield = Some(earthmesh_quality::HfieldDiagnostics {
            config: earthmesh_quality::HfieldConfigDiagnostics {
                enabled: true,
                g: Some(g),
                max_level: Some(2),
                base_m: Some(100_000.0),
            },
            ..earthmesh_quality::HfieldDiagnostics::default()
        });
        quality
    }

    fn set_repair_metric(quality: &mut earthmesh_quality::MeshQualityReport, metric: &str) {
        quality.repair_cells = vec![earthmesh_quality::WorstCell {
            cell_index: 0,
            refine_level: Some(0),
            centroid: Point::new(0.0, 0.0),
            ring: Vec::new(),
            metric: metric.to_string(),
            value: 1.0,
            level: earthmesh_quality::QualityLevel::Warn,
        }];
    }

    #[test]
    fn gradation_retry_is_relative_bounded_and_attempted_once() {
        let mut quality = quality_with_hfield_g(0.24);
        quality.verdict = earthmesh_quality::QualityLevel::Warn;
        quality
            .gates
            .iter_mut()
            .find(|gate| gate.metric == "max_adjacent_resolution_ratio")
            .unwrap()
            .level = earthmesh_quality::QualityLevel::Warn;

        assert_eq!(
            tighter_hfield_gradation_for_quality(&quality, false),
            Some(0.12)
        );
        assert_eq!(tighter_hfield_gradation_for_quality(&quality, true), None);

        quality.hfield.as_mut().unwrap().config.g = Some(0.015);
        assert_eq!(
            tighter_hfield_gradation_for_quality(&quality, false),
            Some(0.01)
        );
        quality.hfield.as_mut().unwrap().config.g = Some(0.01);
        assert_eq!(tighter_hfield_gradation_for_quality(&quality, false), None);
    }

    #[test]
    fn gradation_retry_only_targets_transition_shaped_quality_gates() {
        for metric in [
            "max_adjacent_resolution_ratio",
            "transition_continuity_warning_count",
            "isolated_refined_cell_count",
        ] {
            let mut quality = quality_with_hfield_g(0.2);
            quality.verdict = earthmesh_quality::QualityLevel::Warn;
            quality
                .gates
                .iter_mut()
                .find(|gate| gate.metric == metric)
                .unwrap()
                .level = earthmesh_quality::QualityLevel::Warn;
            assert_eq!(
                tighter_hfield_gradation_for_quality(&quality, false),
                Some(0.1),
                "metric={metric}"
            );
        }

        let mut angle_only = quality_with_hfield_g(0.2);
        angle_only.verdict = earthmesh_quality::QualityLevel::Warn;
        angle_only
            .gates
            .iter_mut()
            .find(|gate| gate.metric == "aspect_ratio_max")
            .unwrap()
            .level = earthmesh_quality::QualityLevel::Warn;
        assert_eq!(
            tighter_hfield_gradation_for_quality(&angle_only, false),
            None
        );

        let mut hfield_jump = quality_with_hfield_g(0.2);
        hfield_jump.verdict = earthmesh_quality::QualityLevel::Warn;
        hfield_jump.gates.push(earthmesh_quality::GateResult {
            metric: "hfield_target_level_jump_gt_one_count".to_string(),
            value: 1.0,
            level: earthmesh_quality::QualityLevel::Warn,
            detail: "adjacent target refinement level jump > 1".to_string(),
        });
        assert_eq!(
            tighter_hfield_gradation_for_quality(&hfield_jump, false),
            None
        );

        hfield_jump.gates.last_mut().unwrap().metric =
            "hfield_actual_level_jump_gt_one_count".to_string();
        assert_eq!(
            tighter_hfield_gradation_for_quality(&hfield_jump, false),
            Some(0.1)
        );

        let mut deep_edge_cv = quality_with_hfield_g(0.2);
        deep_edge_cv.hfield.as_mut().unwrap().config.max_level = Some(3);
        deep_edge_cv.verdict = earthmesh_quality::QualityLevel::Warn;
        deep_edge_cv
            .gates
            .iter_mut()
            .find(|gate| gate.metric == "cell_edge_length_cv_max")
            .unwrap()
            .level = earthmesh_quality::QualityLevel::Warn;
        assert_eq!(
            tighter_hfield_gradation_for_quality(&deep_edge_cv, false),
            None
        );
    }

    #[test]
    fn local_refinement_skips_conforming_hfield_transition_only_warnings() {
        let mut shallow_edge_cv = quality_with_hfield_g(0.2);
        shallow_edge_cv.verdict = earthmesh_quality::QualityLevel::Warn;
        shallow_edge_cv
            .gates
            .iter_mut()
            .find(|gate| gate.metric == "cell_edge_length_cv_max")
            .unwrap()
            .level = earthmesh_quality::QualityLevel::Warn;
        set_repair_metric(&mut shallow_edge_cv, "cell_edge_length_cv");
        assert!(!should_attempt_local_quality_refinement(&shallow_edge_cv));

        shallow_edge_cv.hfield.as_mut().unwrap().config.max_level = Some(3);
        shallow_edge_cv.gates.push(earthmesh_quality::GateResult {
            metric: "cell_area_cv".to_string(),
            value: 1.6,
            level: earthmesh_quality::QualityLevel::Warn,
            detail: "multi-resolution area spread".to_string(),
        });
        shallow_edge_cv.gates.push(earthmesh_quality::GateResult {
            metric: "hfield_target_level_jump_gt_one_count".to_string(),
            value: 19.0,
            level: earthmesh_quality::QualityLevel::Warn,
            detail: "target field is steeper than the realized mesh".to_string(),
        });
        assert!(!should_attempt_local_quality_refinement(&shallow_edge_cv));

        shallow_edge_cv
            .hfield
            .as_mut()
            .unwrap()
            .actual_level_jump_gt_one_count = 1;
        assert!(should_attempt_local_quality_refinement(&shallow_edge_cv));

        let mut shape_only = quality_with_hfield_g(0.2);
        shape_only.verdict = earthmesh_quality::QualityLevel::Warn;
        shape_only
            .gates
            .iter_mut()
            .find(|gate| gate.metric == "aspect_ratio_max")
            .unwrap()
            .level = earthmesh_quality::QualityLevel::Warn;
        shape_only
            .gates
            .iter_mut()
            .find(|gate| gate.metric == "cell_edge_length_cv_max")
            .unwrap()
            .level = earthmesh_quality::QualityLevel::Warn;
        set_repair_metric(&mut shape_only, "cell_edge_length_cv");
        assert!(!should_attempt_local_quality_refinement(&shape_only));

        set_repair_metric(&mut shape_only, "aspect_ratio");
        assert!(should_attempt_local_quality_refinement(&shape_only));

        set_repair_metric(&mut shape_only, "cell_edge_length_cv");
        shape_only
            .hfield
            .as_mut()
            .unwrap()
            .target_above_actual_count = 1;
        assert!(should_attempt_local_quality_refinement(&shape_only));
    }

    #[test]
    fn conforming_hfield_repair_plan_does_not_hide_aspect_behind_batch_one() {
        let input = QualityMeshInput {
            vertices: vec![
                Point::new(0.0, 0.0),
                Point::new(5.0, 0.0),
                Point::new(5.0, 1.0),
                Point::new(0.0, 1.0),
            ],
            cells: vec![QualityCell {
                vertices: vec![0, 1, 2, 3],
                refine_level: Some(0),
                neighbors: vec![],
            }],
        };
        let thresholds = QualityThresholds {
            min_angle_warn_deg: 0.0,
            angle_deviation_warn_deg: 180.0,
            ..QualityThresholds::default()
        };
        let mut quality = earthmesh_quality::compute(&input, &thresholds);
        quality.hfield = quality_with_hfield_g(0.2).hfield;
        assert_eq!(quality.repair_cells[0].metric, "cell_edge_length_cv");

        prefer_non_transition_repair(&mut quality, &input, &thresholds);

        assert_eq!(quality.repair_cells[0].metric, "aspect_ratio");
        assert!(should_attempt_local_quality_refinement(&quality));
    }

    #[test]
    fn gradation_retry_skips_unrepairable_failures() {
        let mut quality = quality_with_hfield_g(0.2);
        quality.verdict = earthmesh_quality::QualityLevel::Fail;
        quality
            .gates
            .iter_mut()
            .find(|gate| gate.metric == "cell_edge_length_cv_max")
            .unwrap()
            .level = earthmesh_quality::QualityLevel::Warn;
        assert!(quality.has_unrepairable_failure());
        assert_eq!(tighter_hfield_gradation_for_quality(&quality, false), None);
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
