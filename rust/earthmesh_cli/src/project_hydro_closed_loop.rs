//! Project hydro closed loop: measure the coarse mesh, apply the hydro target
//! field through Method-C, then recompute delivery/coupling on the final mesh.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use earthmesh_project::ProjectConfig;

use crate::hydro_refinement_adapter::{
    run_hydro_refinement_adapter, run_hydro_refinement_adapter_with_gradation_cap,
    run_quality_refinement_adapter, HydroRefinementAdapterReport,
};
use crate::project_hydro::{run_project_hydro_postprocess, ProjectHydroReport};

#[derive(Debug)]
pub struct ProjectHydroClosedLoopReport {
    pub coarse: ProjectHydroReport,
    pub refinement: Option<HydroRefinementAdapterReport>,
    pub final_analysis: Option<ProjectHydroReport>,
    pub final_gridfile: PathBuf,
    pub final_quality_verdict: earthmesh_quality::QualityLevel,
    pub final_coupling_quality_verdict: Option<String>,
    pub final_quality_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub quality_retry_applied: bool,
    pub quality_retry_rejected: bool,
}

// A level-3 HField transition at g=0.2 can be topologically valid yet leave a
// badly stretched level-2/3 interface cell. A single retry at g=0.1 widens the
// graded skirt instead of weakening the quality threshold or over-iterating
// the spring solver.
const HYDRO_DEEP_RETRY_HFIELD_G: f64 = 0.1;

fn deep_hfield_retry_needed(
    max_level: u8,
    edge_cv_level: Option<earthmesh_quality::QualityLevel>,
) -> bool {
    max_level >= 3
        && edge_cv_level.is_some_and(|level| level != earthmesh_quality::QualityLevel::Pass)
}

fn needs_deep_hfield_retry(max_level: u8, report: &earthmesh_quality::MeshQualityReport) -> bool {
    deep_hfield_retry_needed(
        max_level,
        report
            .gates
            .iter()
            .find(|gate| gate.metric == "cell_edge_length_cv_max")
            .map(|gate| gate.level),
    )
}

fn snapshot_quality_summary(source_dir: &Path, snapshot_dir: &Path) -> io::Result<PathBuf> {
    let source = source_dir.join("quality_summary.json");
    fs::create_dir_all(snapshot_dir)?;
    let snapshot = snapshot_dir.join("quality_summary.json");
    fs::copy(&source, &snapshot).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "snapshot baseline quality report {} -> {}: {error}",
                source.display(),
                snapshot.display()
            ),
        )
    })?;
    Ok(snapshot)
}

/// Execute one bounded hydro-driven refinement pass.
///
/// The planner is measured on `initial_gridfile`. If it requests refinement,
/// the HField/Method-C pipeline continues from the unmasked
/// `refinement_parent_gridfile` in an isolated output tree, then recomputes the
/// complete hydro analysis against the clipped final gridfile.
#[allow(clippy::too_many_arguments)]
pub fn run_project_hydro_closed_loop(
    project: &ProjectConfig,
    project_path: impl AsRef<Path>,
    source_namelist: impl AsRef<Path>,
    initial_gridfile: impl AsRef<Path>,
    refinement_parent_gridfile: impl AsRef<Path>,
    out_dir: impl AsRef<Path>,
    engine_workdir: impl AsRef<Path>,
    max_tris: usize,
    source_gridnum_perdegree: Option<usize>,
) -> io::Result<Option<ProjectHydroClosedLoopReport>> {
    let project_path = project_path.as_ref();
    let source_namelist = source_namelist.as_ref();
    let initial_gridfile = initial_gridfile.as_ref();
    let refinement_parent_gridfile = refinement_parent_gridfile.as_ref();
    let out_dir = out_dir.as_ref();
    let engine_workdir = engine_workdir.as_ref();
    prepare_closed_loop_output_dir(out_dir)?;
    let coarse_dir = out_dir.join("coarse");
    let Some(coarse) =
        run_project_hydro_postprocess(project, project_path, initial_gridfile, &coarse_dir)?
    else {
        return Ok(None);
    };

    let (mut refinement, mut final_gridfile) = if coarse.hydro.refinement_max_level > 0 {
        let adapter_namelist = out_dir.join("hydro_refinement_adapter.nml");
        let adapter = run_hydro_refinement_adapter(
            source_namelist,
            refinement_parent_gridfile,
            &coarse.hydro.intersections_path,
            &coarse.hydro.refinement_plan_path,
            &adapter_namelist,
            engine_workdir,
            max_tris,
            source_gridnum_perdegree,
        )?;
        let final_gridfile = adapter.final_gridfile().to_path_buf();
        (Some(adapter), final_gridfile)
    } else {
        (None, initial_gridfile.to_path_buf())
    };

    let final_quality_dir = out_dir.join("final_quality");
    let mut final_quality = crate::project_quality::write_project_quality_report_with_namelist(
        project,
        &final_gridfile,
        &final_quality_dir,
        refinement
            .as_ref()
            .map(|adapter| adapter.adapter_namelist.as_path()),
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let mut quality_retry_applied = false;
    let mut quality_retry_rejected = false;
    if refinement
        .as_ref()
        .is_some_and(|adapter| needs_deep_hfield_retry(adapter.target.max_level, &final_quality))
    {
        let baseline_gridfile = final_gridfile.clone();
        let baseline_verdict = final_quality.verdict;
        let retry_dir = out_dir.join("deep_quality_retry");
        let adapter_namelist = retry_dir.join("adapter.nml");
        let adapter = run_hydro_refinement_adapter_with_gradation_cap(
            source_namelist,
            refinement_parent_gridfile,
            &coarse.hydro.intersections_path,
            &coarse.hydro.refinement_plan_path,
            &adapter_namelist,
            engine_workdir,
            max_tris,
            source_gridnum_perdegree,
            Some(HYDRO_DEEP_RETRY_HFIELD_G),
        )?;
        let candidate_gridfile = adapter.final_gridfile().to_path_buf();
        let baseline_quality_report =
            snapshot_quality_summary(&final_quality_dir, &retry_dir.join("baseline_quality"))?;
        let candidate_quality_dir = retry_dir.join("candidate_quality");
        let candidate_quality_report = candidate_quality_dir.join("quality_summary.json");
        let candidate_quality = crate::project_quality::write_project_quality_report_with_namelist(
            project,
            &candidate_gridfile,
            &candidate_quality_dir,
            Some(adapter.adapter_namelist.as_path()),
        )
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let regressions = candidate_quality.guarded_metric_regressions(&final_quality);
        if crate::project_quality::select_auto_refine_candidate(&final_quality, &candidate_quality)
            == crate::project_quality::AutoRefineCandidateSelection::Candidate
        {
            crate::project_quality::write_auto_refine_decision(
                &candidate_quality_dir,
                &crate::project_quality::AutoRefineDecision {
                    pass: project.refinement.max_passes,
                    decision: "accepted",
                    reason: "tighter hydro gradation strictly improved quality without guarded regressions",
                    regressions: &regressions,
                    baseline_gridfile: Some(&baseline_gridfile),
                    candidate_gridfile: &candidate_gridfile,
                    selected_gridfile: &candidate_gridfile,
                    baseline_quality_report: Some(&baseline_quality_report),
                    candidate_quality_report: &candidate_quality_report,
                    selected_quality_report: &candidate_quality_report,
                    baseline_verdict: Some(baseline_verdict),
                    candidate_verdict: candidate_quality.verdict,
                    selected_verdict: candidate_quality.verdict,
                },
            )
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            quality_retry_applied = true;
            final_gridfile = candidate_gridfile;
            refinement = Some(adapter);
            final_quality = crate::project_quality::write_project_quality_report_with_namelist(
                project,
                &final_gridfile,
                &final_quality_dir,
                refinement
                    .as_ref()
                    .map(|adapter| adapter.adapter_namelist.as_path()),
            )
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        } else {
            crate::project_quality::write_auto_refine_decision(
                &candidate_quality_dir,
                &crate::project_quality::AutoRefineDecision {
                    pass: project.refinement.max_passes,
                    decision: "rejected",
                    reason: "tighter hydro gradation did not strictly improve all guarded quality metrics",
                    regressions: &regressions,
                    baseline_gridfile: Some(&baseline_gridfile),
                    candidate_gridfile: &candidate_gridfile,
                    selected_gridfile: &baseline_gridfile,
                    baseline_quality_report: Some(&baseline_quality_report),
                    candidate_quality_report: &candidate_quality_report,
                    selected_quality_report: &baseline_quality_report,
                    baseline_verdict: Some(baseline_verdict),
                    candidate_verdict: candidate_quality.verdict,
                    selected_verdict: baseline_verdict,
                },
            )
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            quality_retry_rejected = true;
            eprintln!(
                "earthmesh_cli: warning: rejected tighter hydro gradation because quality did not strictly improve ({} -> {}); keeping the previous mesh",
                baseline_verdict.as_str(),
                candidate_quality.verdict.as_str()
            );
        }
    }

    if project.quality.on_violation == earthmesh_project::ViolationPolicy::AutoRefine {
        let target_nxp = project
            .try_lower()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?
            .mkgrd
            .nxp;
        let mut state =
            earthmesh_project::AutoRefineState::new(project.refinement.max_passes, target_nxp);
        loop {
            if final_quality.verdict == earthmesh_quality::QualityLevel::Pass {
                break;
            }
            if final_quality.has_unrepairable_failure() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "hydro final mesh has an unrepairable quality failure; report={}",
                        final_quality_dir.join("quality_summary.json").display()
                    ),
                ));
            }
            match state.transition(earthmesh_project::AutoRefineEvent::QualityViolation) {
                earthmesh_project::AutoRefineAction::Retry { next_pass } => {
                    if final_quality.repair_cells.is_empty() {
                        eprintln!(
                            "earthmesh_cli: warning: hydro final mesh has no locally repairable quality cells"
                        );
                        break;
                    }
                    let repair_dir = out_dir
                        .join("quality_auto_refine")
                        .join(format!("pass_{next_pass}"));
                    let repair_source = refinement
                        .as_ref()
                        .map(|adapter| adapter.adapter_namelist.as_path())
                        .unwrap_or(source_namelist);
                    let adapter = run_quality_refinement_adapter(
                        repair_source,
                        refinement_parent_gridfile,
                        final_quality_dir.join("quality_repair_cells.geojson"),
                        final_quality_dir.join("quality_repair_plan.json"),
                        repair_dir.join("adapter.nml"),
                        engine_workdir,
                        max_tris,
                        source_gridnum_perdegree,
                    )?;
                    let candidate_gridfile = adapter.final_gridfile().to_path_buf();
                    let baseline_quality_report = snapshot_quality_summary(
                        &final_quality_dir,
                        &repair_dir.join("baseline_quality"),
                    )?;
                    let candidate_quality_dir = repair_dir.join("candidate_quality");
                    let candidate_quality_report =
                        candidate_quality_dir.join("quality_summary.json");
                    let candidate_quality =
                        crate::project_quality::write_project_quality_report_with_namelist(
                            project,
                            &candidate_gridfile,
                            &candidate_quality_dir,
                            Some(adapter.adapter_namelist.as_path()),
                        )
                        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
                    let regressions = candidate_quality.guarded_metric_regressions(&final_quality);
                    if crate::project_quality::select_auto_refine_candidate(
                        &final_quality,
                        &candidate_quality,
                    ) == crate::project_quality::AutoRefineCandidateSelection::Baseline
                    {
                        crate::project_quality::write_auto_refine_decision(
                            &candidate_quality_dir,
                            &crate::project_quality::AutoRefineDecision {
                                pass: next_pass,
                                decision: "rejected",
                                reason: "candidate did not strictly improve all guarded quality metrics",
                                regressions: &regressions,
                                baseline_gridfile: Some(&final_gridfile),
                                candidate_gridfile: &candidate_gridfile,
                                selected_gridfile: &final_gridfile,
                                baseline_quality_report: Some(&baseline_quality_report),
                                candidate_quality_report: &candidate_quality_report,
                                selected_quality_report: &baseline_quality_report,
                                baseline_verdict: Some(final_quality.verdict),
                                candidate_verdict: candidate_quality.verdict,
                                selected_verdict: final_quality.verdict,
                            },
                        )
                        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
                        if final_quality.verdict == earthmesh_quality::QualityLevel::Fail {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                format!(
                                    "hydro final auto_refine produced an unrepairable candidate; report={}",
                                    final_quality_dir.join("quality_summary.json").display()
                                ),
                            ));
                        }
                        eprintln!(
                            "earthmesh_cli: warning: hydro final auto_refine rejected pass {next_pass} because the candidate did not strictly improve quality ({} -> {}); keeping the previous valid mesh",
                            final_quality.verdict.as_str(),
                            candidate_quality.verdict.as_str()
                        );
                        quality_retry_rejected = true;
                        break;
                    }
                    crate::project_quality::write_auto_refine_decision(
                        &candidate_quality_dir,
                        &crate::project_quality::AutoRefineDecision {
                            pass: next_pass,
                            decision: "accepted",
                            reason:
                                "candidate strictly improved quality without guarded regressions",
                            regressions: &regressions,
                            baseline_gridfile: Some(&final_gridfile),
                            candidate_gridfile: &candidate_gridfile,
                            selected_gridfile: &candidate_gridfile,
                            baseline_quality_report: Some(&baseline_quality_report),
                            candidate_quality_report: &candidate_quality_report,
                            selected_quality_report: &candidate_quality_report,
                            baseline_verdict: Some(final_quality.verdict),
                            candidate_verdict: candidate_quality.verdict,
                            selected_verdict: candidate_quality.verdict,
                        },
                    )
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
                    quality_retry_applied = true;
                    eprintln!(
                        "earthmesh_cli: hydro final auto_refine applying {} local quality targets at pass {next_pass}",
                        final_quality.repair_cells.len()
                    );
                    final_gridfile = candidate_gridfile;
                    refinement = Some(adapter);
                    final_quality =
                        crate::project_quality::write_project_quality_report_with_namelist(
                            project,
                            &final_gridfile,
                            &final_quality_dir,
                            refinement
                                .as_ref()
                                .map(|adapter| adapter.adapter_namelist.as_path()),
                        )
                        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
                }
                earthmesh_project::AutoRefineAction::CapReached { cap, .. } => {
                    if final_quality.verdict == earthmesh_quality::QualityLevel::Fail {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!(
                                "hydro final auto_refine reached level cap {cap} with verdict=fail; report={}",
                                final_quality_dir.join("quality_summary.json").display()
                            ),
                        ));
                    }
                    eprintln!(
                        "earthmesh_cli: warning: hydro final auto_refine reached level cap {cap}; keeping the final mesh"
                    );
                    break;
                }
                earthmesh_project::AutoRefineAction::Complete { .. } => break,
                earthmesh_project::AutoRefineAction::AbortEngine { .. } => {
                    return Err(io::Error::other(
                        "hydro final auto_refine produced an engine-failure transition",
                    ));
                }
            }
        }
    }

    let final_analysis = if refinement.is_some() {
        Some(
            run_project_hydro_postprocess(
                project,
                project_path,
                &final_gridfile,
                out_dir.join("final"),
            )?
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "configured Project hydro disappeared during final recomputation",
                )
            })?,
        )
    } else {
        None
    };
    let manifest_path = out_dir.join("closed_loop_manifest.json");
    let final_manifest = final_analysis
        .as_ref()
        .map(|report| report.manifest_path.as_path())
        .unwrap_or(coarse.manifest_path.as_path());
    let adapter_namelist = refinement
        .as_ref()
        .map(|report| report.adapter_namelist.display().to_string());
    let final_coupling_quality_verdict = final_analysis
        .as_ref()
        .unwrap_or(&coarse)
        .hydro
        .coupling_quality_verdict
        .clone();
    let coupling_verdict_json = final_coupling_quality_verdict
        .as_deref()
        .map(|value| format!("\"{}\"", crate::json_escape_string(value)))
        .unwrap_or_else(|| "null".to_string());
    fs::write(
        &manifest_path,
        format!(
            "{{\n  \"kind\": \"earthmesh_project_hydro_closed_loop\",\n  \"plan_applied\": {},\n  \"quality_retry_applied\": {},\n  \"quality_retry_rejected\": {},\n  \"initial_gridfile\": \"{}\",\n  \"refinement_parent_gridfile\": \"{}\",\n  \"final_gridfile\": \"{}\",\n  \"final_quality_verdict\": \"{}\",\n  \"final_coupling_quality_verdict\": {},\n  \"final_quality_dir\": \"{}\",\n  \"coarse_manifest\": \"{}\",\n  \"adapter_namelist\": {},\n  \"final_manifest\": \"{}\"\n}}\n",
            refinement.is_some(),
            quality_retry_applied,
            quality_retry_rejected,
            crate::json_escape_string(&initial_gridfile.display().to_string()),
            crate::json_escape_string(&refinement_parent_gridfile.display().to_string()),
            crate::json_escape_string(&final_gridfile.display().to_string()),
            final_quality.verdict.as_str(),
            coupling_verdict_json,
            crate::json_escape_string(&final_quality_dir.display().to_string()),
            crate::json_escape_string(&coarse.manifest_path.display().to_string()),
            adapter_namelist
                .as_deref()
                .map(|path| format!("\"{}\"", crate::json_escape_string(path)))
                .unwrap_or_else(|| "null".to_string()),
            crate::json_escape_string(&final_manifest.display().to_string()),
        ),
    )?;

    Ok(Some(ProjectHydroClosedLoopReport {
        coarse,
        refinement,
        final_analysis,
        final_gridfile,
        final_quality_verdict: final_quality.verdict,
        final_coupling_quality_verdict,
        final_quality_dir,
        manifest_path,
        quality_retry_applied,
        quality_retry_rejected,
    }))
}

fn prepare_closed_loop_output_dir(out_dir: &Path) -> io::Result<()> {
    const SENTINEL: &str = ".earthmesh_hydro_closed_loop";
    fs::create_dir_all(out_dir)?;
    let sentinel = out_dir.join(SENTINEL);
    let prior_manifest = out_dir.join("closed_loop_manifest.json");
    let owned = sentinel.is_file()
        || fs::read_to_string(&prior_manifest)
            .ok()
            .is_some_and(|text| text.contains("\"kind\": \"earthmesh_project_hydro_closed_loop\""));
    let mut entries = fs::read_dir(out_dir)?;
    let nonempty = entries.next().transpose()?.is_some();
    if nonempty && !owned {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "refusing to reuse non-empty, unowned hydro output directory {}",
                out_dir.display()
            ),
        ));
    }
    if owned {
        for name in [
            "coarse",
            "final",
            "final_quality",
            "engine",
            "deep_quality_retry",
            "quality_auto_refine",
        ] {
            let path = out_dir.join(name);
            if path.exists() {
                fs::remove_dir_all(path)?;
            }
        }
        for name in ["hydro_refinement_adapter.nml", "closed_loop_manifest.json"] {
            let path = out_dir.join(name);
            if path.exists() {
                fs::remove_file(path)?;
            }
        }
    }
    fs::write(sentinel, b"earthmesh_project_hydro_closed_loop\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_quality_snapshot_survives_final_report_rewrite() {
        let root =
            std::env::temp_dir().join(format!("earthmesh_quality_snapshot_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let final_quality = root.join("final_quality");
        fs::create_dir_all(&final_quality).unwrap();
        fs::write(final_quality.join("quality_summary.json"), "baseline").unwrap();

        let snapshot =
            snapshot_quality_summary(&final_quality, &root.join("retry/baseline_quality")).unwrap();
        fs::write(final_quality.join("quality_summary.json"), "candidate").unwrap();

        assert_eq!(fs::read_to_string(snapshot).unwrap(), "baseline");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn deep_hfield_retry_is_bounded_to_failed_level_three_transitions() {
        use earthmesh_quality::QualityLevel::{Pass, Warn};

        assert!(!deep_hfield_retry_needed(2, Some(Warn)));
        assert!(!deep_hfield_retry_needed(3, Some(Pass)));
        assert!(!deep_hfield_retry_needed(3, None));
        assert!(deep_hfield_retry_needed(3, Some(Warn)));
    }

    #[test]
    fn output_cleanup_preserves_unrelated_files() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_closed_loop_cleanup_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("coarse")).unwrap();
        fs::create_dir_all(root.join("deep_quality_retry")).unwrap();
        fs::create_dir_all(root.join("quality_auto_refine")).unwrap();
        fs::write(root.join(".earthmesh_hydro_closed_loop"), "owned").unwrap();
        fs::write(root.join("coarse/old"), "old").unwrap();
        fs::write(root.join("unrelated.txt"), "keep").unwrap();
        prepare_closed_loop_output_dir(&root).unwrap();
        assert!(!root.join("coarse").exists());
        assert!(!root.join("deep_quality_retry").exists());
        assert!(!root.join("quality_auto_refine").exists());
        assert_eq!(
            fs::read_to_string(root.join("unrelated.txt")).unwrap(),
            "keep"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn output_cleanup_rejects_nonempty_unowned_directory() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_closed_loop_unowned_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("user.txt"), "keep").unwrap();
        let error = prepare_closed_loop_output_dir(&root).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert!(root.join("user.txt").is_file());
        let _ = fs::remove_dir_all(root);
    }
}
