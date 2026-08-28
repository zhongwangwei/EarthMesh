use serde::Serialize;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use earthmesh_refine_harp_dv::{HarpDvRunReport, HarpTraceStage};

pub(crate) const ENV_VAR: &str = "EARTHMESH_HARP_TRACE_JSONL";
pub(crate) const SCHEMA_VERSION: u32 = 4;
pub(crate) const STAGE_COUNT: usize = 7;

static PARTIAL_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) fn from_env() -> io::Result<Option<HarpTraceSession>> {
    from_env_value(env::var_os(ENV_VAR))
}

fn from_env_value(value: Option<std::ffi::OsString>) -> io::Result<Option<HarpTraceSession>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let target = PathBuf::from(value);
    if !target.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{ENV_VAR} must be an absolute path"),
        ));
    }
    HarpTraceSession::create(target).map(Some)
}

pub(crate) struct HarpTraceSession {
    target: PathBuf,
    partial: PathBuf,
    writer: Option<TraceLineWriter<BufWriter<File>>>,
}

impl HarpTraceSession {
    fn create(target: PathBuf) -> io::Result<Self> {
        match fs::symlink_metadata(&target) {
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("HARP trace target already exists: {}", target.display()),
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let parent = target.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "HARP trace target has no parent directory: {}",
                    target.display()
                ),
            )
        })?;
        let file_name = target.file_name().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("HARP trace target has no file name: {}", target.display()),
            )
        })?;
        let file_name = file_name.to_string_lossy();
        let pid = std::process::id();
        for _ in 0..1024 {
            let nonce = PARTIAL_COUNTER.fetch_add(1, Ordering::Relaxed);
            let partial = parent.join(format!(".{file_name}.partial.{pid}.{nonce}"));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&partial)
            {
                Ok(file) => {
                    let mut session = Self {
                        target,
                        partial,
                        writer: Some(TraceLineWriter::new(BufWriter::new(file))),
                    };
                    session.write_header()?;
                    return Ok(session);
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate a unique HARP trace partial file",
        ))
    }

    fn write_header(&mut self) -> io::Result<()> {
        self.writer_mut()?.write_record(&RunHeader {
            record_type: "run_header",
            schema_version: SCHEMA_VERSION,
            backend: "harp_dv",
            stage_count: STAGE_COUNT,
        })
    }

    pub(crate) fn write_stage_summary<T: Serialize>(
        &mut self,
        stage: HarpTraceStage,
        record: &T,
    ) -> io::Result<()> {
        self.writer_mut()?.write_stage_summary(stage, record)
    }

    pub(crate) fn write_event<T: Serialize>(&mut self, record: &T) -> io::Result<()> {
        self.writer_mut()?.write_event(record)
    }

    pub(crate) fn publish(mut self, report: &HarpDvRunReport) -> io::Result<()> {
        let mut writer = self.writer.take().ok_or_else(|| {
            io::Error::other("HARP trace session was already closed before publish")
        })?;
        if usize::from(writer.next_expected_stage) != STAGE_COUNT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "HARP trace completed {} ordered stage_summary records; expected {STAGE_COUNT}",
                    writer.next_expected_stage
                ),
            ));
        }
        writer.write_run_end(report)?;
        writer.inner.flush()?;
        writer.inner.get_ref().sync_all()?;
        drop(writer);
        fs::hard_link(&self.partial, &self.target).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "failed to publish HARP trace without overwriting {} from {}: {error}",
                    self.target.display(),
                    self.partial.display()
                ),
            )
        })?;
        if let Err(error) = fs::remove_file(&self.partial) {
            eprintln!(
                "harp_dv trace: published {}, but could not remove partial {}: {error}",
                self.target.display(),
                self.partial.display()
            );
        }
        Ok(())
    }

    fn writer_mut(&mut self) -> io::Result<&mut TraceLineWriter<BufWriter<File>>> {
        self.writer
            .as_mut()
            .ok_or_else(|| io::Error::other("HARP trace session is closed"))
    }
}

struct TraceLineWriter<W: Write> {
    inner: W,
    event_count: usize,
    next_expected_stage: u8,
}

impl<W: Write> TraceLineWriter<W> {
    fn new(inner: W) -> Self {
        Self {
            inner,
            event_count: 0,
            next_expected_stage: 0,
        }
    }

    fn write_record<T: Serialize>(&mut self, record: &T) -> io::Result<()> {
        serde_json::to_writer(&mut self.inner, record).map_err(io::Error::other)?;
        self.inner.write_all(b"\n")
    }

    fn write_event<T: Serialize>(&mut self, record: &T) -> io::Result<()> {
        self.write_record(record)?;
        self.event_count += 1;
        Ok(())
    }

    fn write_stage_summary<T: Serialize>(
        &mut self,
        stage: HarpTraceStage,
        record: &T,
    ) -> io::Result<()> {
        if stage.index() != self.next_expected_stage {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "HARP stage_summary is {} ({}); expected stage index {}",
                    stage.name(),
                    stage.index(),
                    self.next_expected_stage
                ),
            ));
        }
        self.write_event(record)?;
        self.next_expected_stage += 1;
        Ok(())
    }

    fn write_run_end(&mut self, report: &HarpDvRunReport) -> io::Result<()> {
        self.write_record(&RunEnd {
            record_type: "harp_run_end",
            event_count: self.event_count,
            stage_summary_count: usize::from(self.next_expected_stage),
            stop_reason: report.stop_reason.as_str(),
            cycles_completed: report.cycles_completed,
            final_sites: report.final_sites,
            physical_demands_remaining: report.physical_demands_remaining,
            balance_demands_remaining: report.balance_demands_remaining,
            unbalanced_pairs_remaining: report.unbalanced_pairs_remaining,
            unresolved_cells: report.unresolved_count,
            d4_leaf_retirement_audit_evaluated: report.d4_leaf_retirement_audit_evaluated,
            d4_leaf_retirement_sites_audited: report.d4_leaf_retirement_candidates,
            d4_leaf_retirement_trials_evaluated: report.d4_leaf_retirement_trials_total,
            d4_leaf_retirement_sites_committed: report.d4_leaf_retirement_committed,
            d4_leaf_retirement_sites_fully_acceptable: report.d4_leaf_retirement_fully_acceptable,
        })
    }
}

pub(crate) fn write_core_event(
    session: &mut HarpTraceSession,
    event: &earthmesh_refine_harp_dv::HarpTraceEvent,
) -> io::Result<()> {
    match event {
        earthmesh_refine_harp_dv::HarpTraceEvent::StageSummary {
            stage,
            certification,
        } => session.write_stage_summary(
            *stage,
            &JsonStageSummary::from_certification(*stage, certification),
        ),
        earthmesh_refine_harp_dv::HarpTraceEvent::AngleViolation { stage, violation } => {
            session.write_event(&JsonAngleViolation::from_violation(*stage, violation)?)
        }
        earthmesh_refine_harp_dv::HarpTraceEvent::PhaseSkipped { stage, reason } => session
            .write_event(&JsonPhaseSkipped {
                record_type: "phase_skipped",
                stage_index: stage.index(),
                stage_name: stage.name(),
                reason,
            }),
        earthmesh_refine_harp_dv::HarpTraceEvent::DegreeFourRetirementSummary(summary) => {
            session.write_event(&JsonDegreeFourRetirementSummary::from_summary(summary))
        }
        earthmesh_refine_harp_dv::HarpTraceEvent::DegreeFourRetirementSite(site) => {
            session.write_event(&JsonDegreeFourRetirementSite::from_site(site))
        }
        earthmesh_refine_harp_dv::HarpTraceEvent::DegreeFourRetirementTrial(trial) => {
            session.write_event(&JsonDegreeFourRetirementTrial::from_trial(trial))
        }
        earthmesh_refine_harp_dv::HarpTraceEvent::WindowBudgetPassSummary(summary) => {
            session.write_event(&JsonWindowBudgetPassSummary::from_summary(summary))
        }
        earthmesh_refine_harp_dv::HarpTraceEvent::WindowBudgetArmSummary(summary) => {
            session.write_event(&JsonWindowBudgetArmSummary::from_summary(summary))
        }
    }
}

#[derive(Serialize)]
struct JsonStageSummary<'a> {
    record_type: &'static str,
    stage_index: u8,
    stage_name: &'static str,
    vertex_count: usize,
    edge_count: usize,
    triangle_count: usize,
    open_edge_count: usize,
    topology_error_count: usize,
    euler_characteristic: isize,
    degree_sum: usize,
    twice_edge_count: usize,
    euler_degree_charge: isize,
    degree_histogram: &'a std::collections::BTreeMap<usize, usize>,
    measurable_angle_count: usize,
    min_angle_deg: Option<f64>,
    min_angle_deg_measurable: bool,
    p1_angle_deg: Option<f64>,
    p1_angle_deg_measurable: bool,
    p99_angle_deg: Option<f64>,
    p99_angle_deg_measurable: bool,
    max_angle_deg: Option<f64>,
    max_angle_deg_measurable: bool,
    below_40_count: usize,
    above_80_count: usize,
    unmeasurable_triangle_count: usize,
    unmeasurable_angle_count: usize,
    violating_angles_at_degree_le_4: usize,
    violating_angles_at_degree_ge_5: usize,
    lineage_angle_exposure: Vec<JsonLineageAngleExposure>,
    triangle_context_angle_exposure: Vec<JsonTriangleContextAngleExposure>,
    violation_count: usize,
}

impl<'a> JsonStageSummary<'a> {
    fn from_certification(
        stage: earthmesh_refine_harp_dv::HarpTraceStage,
        certification: &'a earthmesh_refine_harp_dv::MeshCertification,
    ) -> Self {
        let (min_angle_deg, min_angle_deg_measurable) =
            optional_finite(certification.min_angle_deg);
        let (p1_angle_deg, p1_angle_deg_measurable) = optional_finite(certification.p1_angle_deg);
        let (p99_angle_deg, p99_angle_deg_measurable) =
            optional_finite(certification.p99_angle_deg);
        let (max_angle_deg, max_angle_deg_measurable) =
            optional_finite(certification.max_angle_deg);
        Self {
            record_type: "stage_summary",
            stage_index: stage.index(),
            stage_name: stage.name(),
            vertex_count: certification.vertex_count,
            edge_count: certification.edge_count,
            triangle_count: certification.triangle_count,
            open_edge_count: certification.open_edge_count,
            topology_error_count: certification.topology_error_count,
            euler_characteristic: certification.euler_characteristic,
            degree_sum: certification.degree_sum,
            twice_edge_count: certification.twice_edge_count,
            euler_degree_charge: certification.euler_degree_charge,
            degree_histogram: &certification.degree_histogram,
            measurable_angle_count: certification.measurable_angle_count,
            min_angle_deg,
            min_angle_deg_measurable,
            p1_angle_deg,
            p1_angle_deg_measurable,
            p99_angle_deg,
            p99_angle_deg_measurable,
            max_angle_deg,
            max_angle_deg_measurable,
            below_40_count: certification.below_40_count,
            above_80_count: certification.above_80_count,
            unmeasurable_triangle_count: certification.unmeasurable_triangle_count,
            unmeasurable_angle_count: certification.unmeasurable_angle_count,
            violating_angles_at_degree_le_4: certification.violating_angles_at_degree_le_4,
            violating_angles_at_degree_ge_5: certification.violating_angles_at_degree_ge_5,
            lineage_angle_exposure: certification
                .lineage_angle_exposure
                .iter()
                .map(|(key, row)| JsonLineageAngleExposure::from_row(key, row))
                .collect(),
            triangle_context_angle_exposure: certification
                .triangle_context_angle_exposure
                .iter()
                .map(|(key, row)| JsonTriangleContextAngleExposure::from_row(key, row))
                .collect(),
            violation_count: certification.violations.len(),
        }
    }
}

#[derive(Serialize)]
struct JsonLineageAngleExposure {
    birth_source_class: &'static str,
    refinement_depth: u16,
    birth_cycle: u32,
    active_site_count: usize,
    sites_with_violation_count: usize,
    measurable_angle_count: usize,
    below_40_count: usize,
    above_80_count: usize,
}

impl JsonLineageAngleExposure {
    fn from_row(
        key: &earthmesh_refine_harp_dv::certifier::LineageCohortKey,
        row: &earthmesh_refine_harp_dv::certifier::LineageAngleExposure,
    ) -> Self {
        Self {
            birth_source_class: birth_source_class(key.birth_source_class),
            refinement_depth: key.refinement_depth,
            birth_cycle: key.birth_cycle,
            active_site_count: row.active_site_count,
            sites_with_violation_count: row.sites_with_violation_count,
            measurable_angle_count: row.measurable_angle_count,
            below_40_count: row.below_40_count,
            above_80_count: row.above_80_count,
        }
    }
}

#[derive(Serialize)]
struct JsonTriangleContextAngleExposure {
    refinement_boundary_class: &'static str,
    raw_criterion_target_gradient_bin: &'static str,
    frozen_gradated_target_gradient_bin: &'static str,
    measurable_angle_count: usize,
    below_40_count: usize,
    above_80_count: usize,
}

impl JsonTriangleContextAngleExposure {
    fn from_row(
        key: &earthmesh_refine_harp_dv::certifier::TriangleContextKey,
        row: &earthmesh_refine_harp_dv::certifier::TriangleContextAngleExposure,
    ) -> Self {
        Self {
            refinement_boundary_class: refinement_boundary_class(key.refinement_boundary_class),
            raw_criterion_target_gradient_bin: target_gradient_bin(
                key.raw_criterion_target_gradient_bin,
            ),
            frozen_gradated_target_gradient_bin: target_gradient_bin(
                key.frozen_gradated_target_gradient_bin,
            ),
            measurable_angle_count: row.measurable_angle_count,
            below_40_count: row.below_40_count,
            above_80_count: row.above_80_count,
        }
    }
}

#[derive(Serialize)]
struct JsonAngleViolation {
    record_type: &'static str,
    stage_index: u8,
    stage_name: &'static str,
    triangle_sites: [u64; 3],
    corner_site: u64,
    kind: &'static str,
    angle_deg: Option<f64>,
    angle_deg_measurable: bool,
    corner_degree: usize,
    triangle_degree_triplet: [usize; 3],
    refinement_depth: Option<u16>,
    birth_cycle: Option<u32>,
    birth_candidate_source: Option<&'static str>,
    lineage_depth_span: Option<u16>,
    raw_target_coverage_count: u8,
    refinement_boundary_class: &'static str,
    raw_criterion_target_gradient_to_limit_ratio: Option<f64>,
    raw_criterion_target_gradient_to_limit_ratio_measurable: bool,
    frozen_gradated_target_gradient_to_limit_ratio: Option<f64>,
    frozen_gradated_target_gradient_to_limit_ratio_measurable: bool,
    realized_to_raw_criterion_target_scale_ratio: Option<f64>,
    realized_to_raw_criterion_target_scale_ratio_measurable: bool,
}

impl JsonAngleViolation {
    fn from_violation(
        stage: earthmesh_refine_harp_dv::HarpTraceStage,
        violation: &earthmesh_refine_harp_dv::AngleViolation,
    ) -> io::Result<Self> {
        let key = violation.key.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "HARP angle violation is missing its stable AngleKey",
            )
        })?;
        let (angle_deg, angle_deg_measurable) = finite(violation.angle_deg);
        let (
            realized_to_raw_criterion_target_scale_ratio,
            realized_to_raw_criterion_target_scale_ratio_measurable,
        ) = optional_finite(violation.realized_to_raw_criterion_target_scale_ratio);
        let (
            raw_criterion_target_gradient_to_limit_ratio,
            raw_criterion_target_gradient_to_limit_ratio_measurable,
        ) = optional_finite(violation.raw_criterion_target_gradient_to_limit_ratio);
        let (
            frozen_gradated_target_gradient_to_limit_ratio,
            frozen_gradated_target_gradient_to_limit_ratio_measurable,
        ) = optional_finite(violation.frozen_gradated_target_gradient_to_limit_ratio);
        Ok(Self {
            record_type: "angle_violation",
            stage_index: stage.index(),
            stage_name: stage.name(),
            triangle_sites: key.triangle_sites.map(|site| site.0),
            corner_site: key.corner_site.0,
            kind: angle_violation_kind(violation.kind),
            angle_deg,
            angle_deg_measurable,
            corner_degree: violation.corner_degree,
            triangle_degree_triplet: violation.triangle_degree_triplet,
            refinement_depth: violation.refinement_depth,
            birth_cycle: violation.birth_cycle,
            birth_candidate_source: violation.birth_candidate_source.map(candidate_source),
            lineage_depth_span: violation.lineage_depth_span,
            raw_target_coverage_count: violation.raw_target_coverage_count,
            refinement_boundary_class: refinement_boundary_class(
                violation.refinement_boundary_class,
            ),
            raw_criterion_target_gradient_to_limit_ratio,
            raw_criterion_target_gradient_to_limit_ratio_measurable,
            frozen_gradated_target_gradient_to_limit_ratio,
            frozen_gradated_target_gradient_to_limit_ratio_measurable,
            realized_to_raw_criterion_target_scale_ratio,
            realized_to_raw_criterion_target_scale_ratio_measurable,
        })
    }
}

#[derive(Serialize)]
struct JsonDegreeFourRetirementSummary {
    record_type: &'static str,
    evaluated: bool,
    sites_total: usize,
    sites_not_leaf: usize,
    sites_eligible: usize,
    sites_without_window_violation: usize,
    sites_audited: usize,
    sites_ranked_beyond_64: usize,
    sites_with_any_valid_trial: usize,
    sites_with_any_fully_acceptable_trial: usize,
    sites_committed: usize,
    trials_total: usize,
    checks: JsonDegreeFourRetirementCheckCounts,
    trials_quality_improving: usize,
    trials_fully_acceptable: usize,
}

#[derive(Serialize)]
struct JsonDegreeFourCheckCounts {
    pass: usize,
    fail: usize,
    not_evaluated: usize,
}

impl JsonDegreeFourCheckCounts {
    fn from_counts(counts: &earthmesh_refine_harp_dv::DegreeFourCheckCounts) -> Self {
        Self {
            pass: counts.pass,
            fail: counts.fail,
            not_evaluated: counts.not_evaluated,
        }
    }
}

#[derive(Serialize)]
struct JsonDegreeFourRetirementCheckCounts {
    geometry: JsonDegreeFourCheckCounts,
    hard_gate: JsonDegreeFourCheckCounts,
    physical_demand: JsonDegreeFourCheckCounts,
    scale_balance: JsonDegreeFourCheckCounts,
    no_new_low_degree: JsonDegreeFourCheckCounts,
    angle_count: JsonDegreeFourCheckCounts,
    worst_deviation: JsonDegreeFourCheckCounts,
    penalty: JsonDegreeFourCheckCounts,
    eta: JsonDegreeFourCheckCounts,
    margin: JsonDegreeFourCheckCounts,
    conservative_remap: JsonDegreeFourCheckCounts,
}

impl JsonDegreeFourRetirementCheckCounts {
    fn from_counts(counts: &earthmesh_refine_harp_dv::DegreeFourRetirementCheckCounts) -> Self {
        Self {
            geometry: JsonDegreeFourCheckCounts::from_counts(&counts.geometry),
            hard_gate: JsonDegreeFourCheckCounts::from_counts(&counts.hard_gate),
            physical_demand: JsonDegreeFourCheckCounts::from_counts(&counts.physical_demand),
            scale_balance: JsonDegreeFourCheckCounts::from_counts(&counts.scale_balance),
            no_new_low_degree: JsonDegreeFourCheckCounts::from_counts(&counts.no_new_low_degree),
            angle_count: JsonDegreeFourCheckCounts::from_counts(&counts.angle_count),
            worst_deviation: JsonDegreeFourCheckCounts::from_counts(&counts.worst_deviation),
            penalty: JsonDegreeFourCheckCounts::from_counts(&counts.penalty),
            eta: JsonDegreeFourCheckCounts::from_counts(&counts.eta),
            margin: JsonDegreeFourCheckCounts::from_counts(&counts.margin),
            conservative_remap: JsonDegreeFourCheckCounts::from_counts(&counts.conservative_remap),
        }
    }
}

impl JsonDegreeFourRetirementSummary {
    fn from_summary(summary: &earthmesh_refine_harp_dv::DegreeFourRetirementSummary) -> Self {
        Self {
            record_type: "degree_four_retirement_summary",
            evaluated: summary.evaluated,
            sites_total: summary.sites_total,
            sites_not_leaf: summary.sites_not_leaf,
            sites_eligible: summary.sites_eligible,
            sites_without_window_violation: summary.sites_without_window_violation,
            sites_audited: summary.sites_audited,
            sites_ranked_beyond_64: summary.sites_ranked_beyond_64,
            sites_with_any_valid_trial: summary.sites_with_any_valid_trial,
            sites_with_any_fully_acceptable_trial: summary.sites_with_any_fully_acceptable_trial,
            sites_committed: summary.sites_committed,
            trials_total: summary.trials_total,
            checks: JsonDegreeFourRetirementCheckCounts::from_counts(&summary.checks),
            trials_quality_improving: summary.trials_quality_improving,
            trials_fully_acceptable: summary.trials_fully_acceptable,
        }
    }
}

#[derive(Serialize)]
struct JsonDegreeFourRetirementSite {
    record_type: &'static str,
    site_id: u64,
    vertex: usize,
    interior_leaf: bool,
    window_violation: bool,
    candidate_rank: Option<usize>,
    ranked_beyond_64: bool,
    trial_count: usize,
    any_valid_trial: bool,
    any_fully_acceptable_trial: bool,
    committed: bool,
}

impl JsonDegreeFourRetirementSite {
    fn from_site(site: &earthmesh_refine_harp_dv::DegreeFourRetirementSite) -> Self {
        Self {
            record_type: "degree_four_retirement_site",
            site_id: site.site_id.0,
            vertex: site.vertex,
            interior_leaf: site.interior_leaf,
            window_violation: site.window_violation,
            candidate_rank: site.candidate_rank,
            ranked_beyond_64: site.ranked_beyond_64,
            trial_count: site.trial_count,
            any_valid_trial: site.any_valid_trial,
            any_fully_acceptable_trial: site.any_fully_acceptable_trial,
            committed: site.committed,
        }
    }
}

#[derive(Serialize)]
struct JsonDegreeFourRetirementTrial {
    record_type: &'static str,
    site_id: u64,
    vertex: usize,
    trial_index: u8,
    ring_site_ids: Option<[u64; 4]>,
    diagonal_site_ids: Option<[u64; 2]>,
    geometry: &'static str,
    hard_gate: &'static str,
    physical_demand: &'static str,
    scale_balance: &'static str,
    no_new_low_degree: &'static str,
    angle_count: &'static str,
    worst_deviation: &'static str,
    penalty: &'static str,
    eta: &'static str,
    margin: &'static str,
    conservative_remap: &'static str,
    fully_acceptable: bool,
}

impl JsonDegreeFourRetirementTrial {
    fn from_trial(trial: &earthmesh_refine_harp_dv::DegreeFourRetirementTrial) -> Self {
        Self {
            record_type: "degree_four_retirement_trial",
            site_id: trial.site_id.0,
            vertex: trial.vertex,
            trial_index: trial.trial_index,
            ring_site_ids: trial.ring_site_ids.map(|sites| sites.map(|site| site.0)),
            diagonal_site_ids: trial
                .diagonal_site_ids
                .map(|sites| sites.map(|site| site.0)),
            geometry: check_status(trial.geometry),
            hard_gate: check_status(trial.hard_gate),
            physical_demand: check_status(trial.physical_demand),
            scale_balance: check_status(trial.scale_balance),
            no_new_low_degree: check_status(trial.no_new_low_degree),
            angle_count: check_status(trial.angle_count),
            worst_deviation: check_status(trial.worst_deviation),
            penalty: check_status(trial.penalty),
            eta: check_status(trial.eta),
            margin: check_status(trial.margin),
            conservative_remap: check_status(trial.conservative_remap),
            fully_acceptable: trial.fully_acceptable,
        }
    }
}

fn check_status(status: earthmesh_refine_harp_dv::DegreeFourCheckStatus) -> &'static str {
    match status {
        earthmesh_refine_harp_dv::DegreeFourCheckStatus::Pass => "pass",
        earthmesh_refine_harp_dv::DegreeFourCheckStatus::Fail => "fail",
        earthmesh_refine_harp_dv::DegreeFourCheckStatus::NotEvaluated => "not_evaluated",
    }
}

#[derive(Serialize)]
struct JsonWindowBudgetPassSummary {
    record_type: &'static str,
    arm: &'static str,
    pass_index: usize,
    window_pass_limit: usize,
    per_pass_site_budget: usize,
    processed_sites: usize,
    eligible_sites: usize,
    found_sites: usize,
    unique_sites_seen: usize,
    candidate_count: usize,
    line_search_attempt_count: usize,
    retained_move_count: usize,
    completed_breadth_sweep: bool,
    below_40_count: usize,
    above_80_count: usize,
    total_violation_count: usize,
    resolved_s3_cohort_key_count: usize,
    persisted_s3_cohort_key_count: usize,
    kind_changed_s3_cohort_key_count: usize,
    new_global_angle_key_count: usize,
    worst_window_deviation_deg: Option<f64>,
    worst_window_deviation_deg_measurable: bool,
    window_penalty: Option<f64>,
    window_penalty_measurable: bool,
    eta_min: Option<f64>,
    eta_min_measurable: bool,
    eta_p1: Option<f64>,
    eta_p1_measurable: bool,
    physical_demands_remaining: usize,
    balance_demands_remaining: usize,
    unbalanced_pairs_remaining: usize,
    wall_time_ms: u64,
    stop_reason_if_terminal: Option<&'static str>,
}

impl JsonWindowBudgetPassSummary {
    fn from_summary(summary: &earthmesh_refine_harp_dv::trace::WindowBudgetPassSummary) -> Self {
        let (worst_window_deviation_deg, worst_window_deviation_deg_measurable) =
            finite(summary.worst_window_deviation_deg);
        let (window_penalty, window_penalty_measurable) = finite(summary.window_penalty);
        let (eta_min, eta_min_measurable) = finite(summary.eta_min);
        let (eta_p1, eta_p1_measurable) = finite(summary.eta_p1);
        Self {
            record_type: "window_budget_pass_summary",
            arm: summary.arm.name(),
            pass_index: summary.pass_index,
            window_pass_limit: summary.window_pass_limit,
            per_pass_site_budget: summary.per_pass_site_budget,
            processed_sites: summary.processed_sites,
            eligible_sites: summary.eligible_sites,
            found_sites: summary.found_sites,
            unique_sites_seen: summary.unique_sites_seen,
            candidate_count: summary.candidate_count,
            line_search_attempt_count: summary.line_search_attempt_count,
            retained_move_count: summary.retained_move_count,
            completed_breadth_sweep: summary.completed_breadth_sweep,
            below_40_count: summary.below_40_count,
            above_80_count: summary.above_80_count,
            total_violation_count: summary.total_violation_count,
            resolved_s3_cohort_key_count: summary.resolved_s3_cohort_key_count,
            persisted_s3_cohort_key_count: summary.persisted_s3_cohort_key_count,
            kind_changed_s3_cohort_key_count: summary.kind_changed_s3_cohort_key_count,
            new_global_angle_key_count: summary.new_global_angle_key_count,
            worst_window_deviation_deg,
            worst_window_deviation_deg_measurable,
            window_penalty,
            window_penalty_measurable,
            eta_min,
            eta_min_measurable,
            eta_p1,
            eta_p1_measurable,
            physical_demands_remaining: summary.physical_demands_remaining,
            balance_demands_remaining: summary.balance_demands_remaining,
            unbalanced_pairs_remaining: summary.unbalanced_pairs_remaining,
            wall_time_ms: summary.wall_time_ms,
            stop_reason_if_terminal: summary
                .stop_reason_if_terminal
                .map(window_budget_stop_reason),
        }
    }
}

#[derive(Serialize)]
struct JsonWindowBudgetArmSummary {
    record_type: &'static str,
    arm: &'static str,
    window_pass_limit: usize,
    pass_count: usize,
    s3_violation_key_count: usize,
    s4_below_40_count: usize,
    s4_above_80_count: usize,
    s4_total_violation_count: usize,
    s4_worst_window_deviation_deg: Option<f64>,
    s4_worst_window_deviation_deg_measurable: bool,
    s4_window_penalty: Option<f64>,
    s4_window_penalty_measurable: bool,
    s4_eta_min: Option<f64>,
    s4_eta_min_measurable: bool,
    s4_eta_p1: Option<f64>,
    s4_eta_p1_measurable: bool,
    s4_physical_demands_remaining: usize,
    s4_balance_demands_remaining: usize,
    s4_unbalanced_pairs_remaining: usize,
    s4_resolved_s3_cohort_key_count: usize,
    s4_persisted_s3_cohort_key_count: usize,
    s4_kind_changed_s3_cohort_key_count: usize,
    s4_new_global_angle_key_count: usize,
    s6_below_40_count: usize,
    s6_above_80_count: usize,
    s6_total_violation_count: usize,
    s6_worst_window_deviation_deg: Option<f64>,
    s6_worst_window_deviation_deg_measurable: bool,
    s6_window_penalty: Option<f64>,
    s6_window_penalty_measurable: bool,
    s6_eta_min: Option<f64>,
    s6_eta_min_measurable: bool,
    s6_eta_p1: Option<f64>,
    s6_eta_p1_measurable: bool,
    s6_physical_demands_remaining: usize,
    s6_balance_demands_remaining: usize,
    s6_unbalanced_pairs_remaining: usize,
    s6_resolved_s3_cohort_key_count: usize,
    s6_persisted_s3_cohort_key_count: usize,
    s6_kind_changed_s3_cohort_key_count: usize,
    s6_new_global_angle_key_count: usize,
    final_low_degree_moves: usize,
    default_leaf_retirements: usize,
    wall_time_ms: u64,
    stop_reason: &'static str,
}

impl JsonWindowBudgetArmSummary {
    fn from_summary(summary: &earthmesh_refine_harp_dv::trace::WindowBudgetArmSummary) -> Self {
        let (s4_worst_window_deviation_deg, s4_worst_window_deviation_deg_measurable) =
            finite(summary.s4_worst_window_deviation_deg);
        let (s4_window_penalty, s4_window_penalty_measurable) = finite(summary.s4_window_penalty);
        let (s4_eta_min, s4_eta_min_measurable) = finite(summary.s4_eta_min);
        let (s4_eta_p1, s4_eta_p1_measurable) = finite(summary.s4_eta_p1);
        let (s6_worst_window_deviation_deg, s6_worst_window_deviation_deg_measurable) =
            finite(summary.s6_worst_window_deviation_deg);
        let (s6_window_penalty, s6_window_penalty_measurable) = finite(summary.s6_window_penalty);
        let (s6_eta_min, s6_eta_min_measurable) = finite(summary.s6_eta_min);
        let (s6_eta_p1, s6_eta_p1_measurable) = finite(summary.s6_eta_p1);
        Self {
            record_type: "window_budget_arm_summary",
            arm: summary.arm.name(),
            window_pass_limit: summary.window_pass_limit,
            pass_count: summary.pass_count,
            s3_violation_key_count: summary.s3_violation_key_count,
            s4_below_40_count: summary.s4_below_40_count,
            s4_above_80_count: summary.s4_above_80_count,
            s4_total_violation_count: summary.s4_total_violation_count,
            s4_worst_window_deviation_deg,
            s4_worst_window_deviation_deg_measurable,
            s4_window_penalty,
            s4_window_penalty_measurable,
            s4_eta_min,
            s4_eta_min_measurable,
            s4_eta_p1,
            s4_eta_p1_measurable,
            s4_physical_demands_remaining: summary.s4_physical_demands_remaining,
            s4_balance_demands_remaining: summary.s4_balance_demands_remaining,
            s4_unbalanced_pairs_remaining: summary.s4_unbalanced_pairs_remaining,
            s4_resolved_s3_cohort_key_count: summary.s4_resolved_s3_cohort_key_count,
            s4_persisted_s3_cohort_key_count: summary.s4_persisted_s3_cohort_key_count,
            s4_kind_changed_s3_cohort_key_count: summary.s4_kind_changed_s3_cohort_key_count,
            s4_new_global_angle_key_count: summary.s4_new_global_angle_key_count,
            s6_below_40_count: summary.s6_below_40_count,
            s6_above_80_count: summary.s6_above_80_count,
            s6_total_violation_count: summary.s6_total_violation_count,
            s6_worst_window_deviation_deg,
            s6_worst_window_deviation_deg_measurable,
            s6_window_penalty,
            s6_window_penalty_measurable,
            s6_eta_min,
            s6_eta_min_measurable,
            s6_eta_p1,
            s6_eta_p1_measurable,
            s6_physical_demands_remaining: summary.s6_physical_demands_remaining,
            s6_balance_demands_remaining: summary.s6_balance_demands_remaining,
            s6_unbalanced_pairs_remaining: summary.s6_unbalanced_pairs_remaining,
            s6_resolved_s3_cohort_key_count: summary.s6_resolved_s3_cohort_key_count,
            s6_persisted_s3_cohort_key_count: summary.s6_persisted_s3_cohort_key_count,
            s6_kind_changed_s3_cohort_key_count: summary.s6_kind_changed_s3_cohort_key_count,
            s6_new_global_angle_key_count: summary.s6_new_global_angle_key_count,
            final_low_degree_moves: summary.final_low_degree_moves,
            default_leaf_retirements: summary.default_leaf_retirements,
            wall_time_ms: summary.wall_time_ms,
            stop_reason: window_budget_stop_reason(summary.stop_reason),
        }
    }
}

fn window_budget_stop_reason(
    reason: earthmesh_refine_harp_dv::trace::WindowBudgetStopReason,
) -> &'static str {
    match reason {
        earthmesh_refine_harp_dv::trace::WindowBudgetStopReason::PassLimit => "pass_limit",
        earthmesh_refine_harp_dv::trace::WindowBudgetStopReason::NoRetainedMoves => {
            "no_retained_moves"
        }
        earthmesh_refine_harp_dv::trace::WindowBudgetStopReason::CompletedNoImprovementSweep => {
            "completed_no_improvement_sweep"
        }
    }
}

#[derive(Serialize)]
struct JsonPhaseSkipped<'a> {
    record_type: &'static str,
    stage_index: u8,
    stage_name: &'static str,
    reason: &'a str,
}

fn finite(value: f64) -> (Option<f64>, bool) {
    if value.is_finite() {
        (Some(value), true)
    } else {
        (None, false)
    }
}

fn optional_finite(value: Option<f64>) -> (Option<f64>, bool) {
    match value {
        Some(value) => finite(value),
        None => (None, false),
    }
}

fn angle_violation_kind(kind: earthmesh_refine_harp_dv::AngleViolationKind) -> &'static str {
    match kind {
        earthmesh_refine_harp_dv::AngleViolationKind::Below40 => "below_40",
        earthmesh_refine_harp_dv::AngleViolationKind::Above80 => "above_80",
    }
}

fn candidate_source(source: earthmesh_refine_harp_dv::CandidateSource) -> &'static str {
    match source {
        earthmesh_refine_harp_dv::CandidateSource::Witness => "witness",
        earthmesh_refine_harp_dv::CandidateSource::FarthestPoint => "farthest_point",
        earthmesh_refine_harp_dv::CandidateSource::OffCentre => "off_centre",
        earthmesh_refine_harp_dv::CandidateSource::LongestEdgeMidpoint => "longest_edge_midpoint",
        earthmesh_refine_harp_dv::CandidateSource::AdaptiveOffCentre => "adaptive_off_centre",
        earthmesh_refine_harp_dv::CandidateSource::IncidentEdgeMidpoint => "incident_edge_midpoint",
    }
}

fn birth_source_class(
    source: earthmesh_refine_harp_dv::certifier::BirthSourceClass,
) -> &'static str {
    match source {
        earthmesh_refine_harp_dv::certifier::BirthSourceClass::Inherited => "inherited",
        earthmesh_refine_harp_dv::certifier::BirthSourceClass::Candidate(source) => {
            candidate_source(source)
        }
        earthmesh_refine_harp_dv::certifier::BirthSourceClass::Unknown => "unknown",
    }
}

fn refinement_boundary_class(
    class: earthmesh_refine_harp_dv::certifier::RefinementBoundaryClass,
) -> &'static str {
    match class {
        earthmesh_refine_harp_dv::certifier::RefinementBoundaryClass::Neither => "neither",
        earthmesh_refine_harp_dv::certifier::RefinementBoundaryClass::LineageOnly => "lineage_only",
        earthmesh_refine_harp_dv::certifier::RefinementBoundaryClass::RawCriterionOnly => {
            "raw_criterion_only"
        }
        earthmesh_refine_harp_dv::certifier::RefinementBoundaryClass::Both => "both",
        earthmesh_refine_harp_dv::certifier::RefinementBoundaryClass::Unknown => "unknown",
    }
}

fn target_gradient_bin(
    bin: earthmesh_refine_harp_dv::certifier::TargetGradientBin,
) -> &'static str {
    match bin {
        earthmesh_refine_harp_dv::certifier::TargetGradientBin::Unavailable => "unavailable",
        earthmesh_refine_harp_dv::certifier::TargetGradientBin::Le0_25 => "le_0_25",
        earthmesh_refine_harp_dv::certifier::TargetGradientBin::Gt0_25Le0_5 => "gt_0_25_le_0_5",
        earthmesh_refine_harp_dv::certifier::TargetGradientBin::Gt0_5Le1 => "gt_0_5_le_1",
        earthmesh_refine_harp_dv::certifier::TargetGradientBin::Gt1Le2 => "gt_1_le_2",
        earthmesh_refine_harp_dv::certifier::TargetGradientBin::Gt2 => "gt_2",
    }
}

#[derive(Serialize)]
struct RunHeader {
    record_type: &'static str,
    schema_version: u32,
    backend: &'static str,
    stage_count: usize,
}

#[derive(Serialize)]
struct RunEnd {
    record_type: &'static str,
    event_count: usize,
    stage_summary_count: usize,
    stop_reason: &'static str,
    cycles_completed: u32,
    final_sites: usize,
    physical_demands_remaining: usize,
    balance_demands_remaining: usize,
    unbalanced_pairs_remaining: usize,
    unresolved_cells: usize,
    d4_leaf_retirement_audit_evaluated: bool,
    d4_leaf_retirement_sites_audited: usize,
    d4_leaf_retirement_trials_evaluated: usize,
    d4_leaf_retirement_sites_committed: usize,
    d4_leaf_retirement_sites_fully_acceptable: usize,
}

#[cfg(test)]
fn finite_json(value: f64) -> JsonMeasuredF64 {
    if value.is_finite() {
        JsonMeasuredF64 {
            measurable: true,
            value: Some(value),
        }
    } else {
        JsonMeasuredF64 {
            measurable: false,
            value: None,
        }
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, Serialize)]
struct JsonMeasuredF64 {
    measurable: bool,
    value: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;
    use serde_json::Value;
    use std::path::Path;

    fn temp_path(name: &str) -> PathBuf {
        let nonce = PARTIAL_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = env::temp_dir().join(format!(
            "earthmesh-harp-trace-test-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[derive(Serialize)]
    struct TestRecord<'a> {
        record_type: &'a str,
        stage_index: usize,
        stage_name: &'a str,
        angle_deg: Option<JsonMeasuredF64>,
    }

    fn jsonl(path: &Path) -> Vec<Value> {
        fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn write_dummy_stage_summaries(session: &mut HarpTraceSession) {
        for stage in HarpTraceStage::ALL {
            session
                .write_stage_summary(
                    stage,
                    &TestRecord {
                        record_type: "stage_summary",
                        stage_index: usize::from(stage.index()),
                        stage_name: stage.name(),
                        angle_deg: None,
                    },
                )
                .unwrap();
        }
    }

    fn test_report() -> HarpDvRunReport {
        HarpDvRunReport::empty(4, earthmesh_refine_harp_dv::StopReason::AllSatisfied)
    }

    #[test]
    fn missing_env_value_disables_trace() {
        assert!(from_env_value(None).unwrap().is_none());
    }

    #[test]
    fn relative_env_value_is_rejected() {
        let error = match from_env_value(Some("trace.jsonl".into())) {
            Ok(_) => panic!("relative trace path unexpectedly accepted"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn existing_target_is_not_overwritten() {
        let target = temp_path("trace.jsonl");
        fs::write(&target, "old\n").unwrap();
        let error = match HarpTraceSession::create(target.clone()) {
            Ok(_) => panic!("existing trace target unexpectedly accepted"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(target).unwrap(), "old\n");
    }

    #[test]
    fn missing_parent_is_not_created() {
        let target = temp_path("root")
            .with_file_name("missing-parent")
            .join("trace.jsonl");
        let error = match HarpTraceSession::create(target.clone()) {
            Ok(_) => panic!("trace session unexpectedly created a missing parent"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(!target.parent().unwrap().exists());
    }

    #[test]
    fn publish_race_does_not_overwrite_target_and_keeps_partial() {
        let target = temp_path("trace.jsonl");
        let mut session = HarpTraceSession::create(target.clone()).unwrap();
        let partial = session.partial.clone();
        write_dummy_stage_summaries(&mut session);
        fs::write(&target, "sentinel\n").unwrap();
        let error = session.publish(&test_report()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(&target).unwrap(), "sentinel\n");
        assert!(partial.exists());
        assert!(fs::read_to_string(partial)
            .unwrap()
            .contains("harp_run_end"));
    }

    #[test]
    fn publish_writes_header_seven_summaries_and_run_end() {
        let target = temp_path("trace.jsonl");
        let mut session = HarpTraceSession::create(target.clone()).unwrap();
        write_dummy_stage_summaries(&mut session);
        let mut report = test_report();
        report.stop_reason = earthmesh_refine_harp_dv::StopReason::MaximumCyclesReached;
        report.cycles_completed = 29;
        report.final_sites = 101_715;
        report.physical_demands_remaining = 2;
        report.balance_demands_remaining = 3;
        report.unbalanced_pairs_remaining = 4;
        report.unresolved_count = 5;
        session.publish(&report).unwrap();
        let rows = jsonl(&target);
        assert_eq!(rows.first().unwrap()["record_type"], "run_header");
        assert_eq!(rows.first().unwrap()["stage_count"], STAGE_COUNT);
        assert_eq!(rows.first().unwrap()["schema_version"], SCHEMA_VERSION);
        assert_eq!(rows.last().unwrap()["record_type"], "harp_run_end");
        assert_eq!(rows.last().unwrap()["stage_summary_count"], STAGE_COUNT);
        assert_eq!(
            rows.last().unwrap()["stop_reason"],
            "maximum_cycles_reached"
        );
        assert_eq!(rows.last().unwrap()["cycles_completed"], 29);
        assert_eq!(rows.last().unwrap()["final_sites"], 101_715);
        assert_eq!(rows.last().unwrap()["physical_demands_remaining"], 2);
        assert_eq!(rows.last().unwrap()["balance_demands_remaining"], 3);
        assert_eq!(rows.last().unwrap()["unbalanced_pairs_remaining"], 4);
        assert_eq!(rows.last().unwrap()["unresolved_cells"], 5);
        assert_eq!(rows.len(), STAGE_COUNT + 2);
    }

    #[test]
    fn json_output_is_deterministic() {
        let write = |target: PathBuf| {
            let mut session = HarpTraceSession::create(target.clone()).unwrap();
            session
                .write_event(&TestRecord {
                    record_type: "angle_violation",
                    stage_index: 3,
                    stage_name: "post_eta",
                    angle_deg: Some(finite_json(f64::NAN)),
                })
                .unwrap();
            write_dummy_stage_summaries(&mut session);
            session.publish(&test_report()).unwrap();
            fs::read_to_string(target).unwrap()
        };
        assert_eq!(write(temp_path("a.jsonl")), write(temp_path("b.jsonl")));
    }

    #[test]
    fn partial_without_run_end_is_not_published() {
        let target = temp_path("trace.jsonl");
        {
            let mut session = HarpTraceSession::create(target.clone()).unwrap();
            session
                .write_event(&TestRecord {
                    record_type: "phase_skipped",
                    stage_index: 4,
                    stage_name: "post_window",
                    angle_deg: None,
                })
                .unwrap();
        }
        assert!(!target.exists());
        let partials = fs::read_dir(target.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".partial."))
            .count();
        assert_eq!(partials, 1);
    }

    #[test]
    fn publish_requires_all_stage_summaries() {
        let target = temp_path("incomplete.jsonl");
        let mut session = HarpTraceSession::create(target.clone()).unwrap();
        let partial = session.partial.clone();
        for stage in HarpTraceStage::ALL.into_iter().take(STAGE_COUNT - 1) {
            session
                .write_stage_summary(
                    stage,
                    &TestRecord {
                        record_type: "stage_summary",
                        stage_index: usize::from(stage.index()),
                        stage_name: stage.name(),
                        angle_deg: None,
                    },
                )
                .unwrap();
        }
        let error = session.publish(&test_report()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(!target.exists());
        assert!(partial.exists());
        assert!(!fs::read_to_string(partial)
            .unwrap()
            .contains("harp_run_end"));
    }

    #[test]
    fn stage_summaries_must_be_unique_and_ordered() {
        for unexpected in [HarpTraceStage::Input, HarpTraceStage::PostInitialLowDegree] {
            let target = temp_path(unexpected.name());
            let mut session = HarpTraceSession::create(target.clone()).unwrap();
            let partial = session.partial.clone();
            session
                .write_stage_summary(
                    HarpTraceStage::Input,
                    &TestRecord {
                        record_type: "stage_summary",
                        stage_index: 0,
                        stage_name: "input",
                        angle_deg: None,
                    },
                )
                .unwrap();
            let error = session
                .write_stage_summary(
                    unexpected,
                    &TestRecord {
                        record_type: "stage_summary",
                        stage_index: usize::from(unexpected.index()),
                        stage_name: unexpected.name(),
                        angle_deg: None,
                    },
                )
                .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
            assert!(!target.exists());
            assert!(!fs::read_to_string(partial)
                .unwrap()
                .contains("harp_run_end"));
        }
    }

    #[test]
    fn core_events_are_stable_snake_case_json() {
        use earthmesh_refine_harp_dv::certifier::{
            BirthSourceClass, LineageAngleExposure, LineageCohortKey, RefinementBoundaryClass,
            TargetGradientBin, TriangleContextAngleExposure, TriangleContextKey,
        };
        use earthmesh_refine_harp_dv::{
            AngleKey, AngleViolation, AngleViolationKind, CandidateSource, HarpTraceEvent,
            HarpTraceStage, MeshCertification, SiteId,
        };
        let target = temp_path("core.jsonl");
        let mut degree_histogram = std::collections::BTreeMap::new();
        degree_histogram.insert(6, 4);
        let mut lineage_angle_exposure = std::collections::BTreeMap::new();
        lineage_angle_exposure.insert(
            LineageCohortKey {
                birth_source_class: BirthSourceClass::Inherited,
                refinement_depth: 0,
                birth_cycle: 0,
            },
            LineageAngleExposure {
                active_site_count: 1,
                sites_with_violation_count: 0,
                measurable_angle_count: 3,
                below_40_count: 0,
                above_80_count: 0,
            },
        );
        lineage_angle_exposure.insert(
            LineageCohortKey {
                birth_source_class: BirthSourceClass::Candidate(CandidateSource::OffCentre),
                refinement_depth: 2,
                birth_cycle: 3,
            },
            LineageAngleExposure {
                active_site_count: 3,
                sites_with_violation_count: 1,
                measurable_angle_count: 9,
                below_40_count: 0,
                above_80_count: 1,
            },
        );
        let mut triangle_context_angle_exposure = std::collections::BTreeMap::new();
        triangle_context_angle_exposure.insert(
            TriangleContextKey {
                refinement_boundary_class: RefinementBoundaryClass::Both,
                raw_criterion_target_gradient_bin: TargetGradientBin::Gt1Le2,
                frozen_gradated_target_gradient_bin: TargetGradientBin::Unavailable,
            },
            TriangleContextAngleExposure {
                measurable_angle_count: 12,
                below_40_count: 0,
                above_80_count: 1,
            },
        );
        let violation = AngleViolation {
            key: Some(AngleKey {
                triangle_sites: [SiteId(3), SiteId(4), SiteId(5)],
                corner_site: SiteId(4),
            }),
            kind: AngleViolationKind::Above80,
            triangle: 9,
            corner_vertex: 12,
            angle_deg: f64::INFINITY,
            corner_degree: 7,
            triangle_degree_triplet: [5, 6, 7],
            refinement_depth: Some(2),
            birth_cycle: Some(3),
            birth_candidate_source: Some(CandidateSource::OffCentre),
            lineage_depth_span: Some(2),
            raw_target_coverage_count: 3,
            refinement_boundary_class: RefinementBoundaryClass::Both,
            raw_criterion_target_gradient_to_limit_ratio: Some(1.25),
            frozen_gradated_target_gradient_to_limit_ratio: Some(f64::NAN),
            realized_to_raw_criterion_target_scale_ratio: Some(f64::NAN),
        };
        let certification = MeshCertification {
            vertex_count: 4,
            edge_count: 6,
            triangle_count: 4,
            open_edge_count: 0,
            topology_error_count: 0,
            euler_characteristic: 2,
            degree_sum: 12,
            twice_edge_count: 12,
            euler_degree_charge: 12,
            degree_histogram,
            measurable_angle_count: 12,
            min_angle_deg: Some(30.0),
            p1_angle_deg: None,
            p99_angle_deg: Some(100.0),
            max_angle_deg: Some(100.0),
            below_40_count: 0,
            above_80_count: 1,
            unmeasurable_triangle_count: 0,
            unmeasurable_angle_count: 0,
            violating_angles_at_degree_le_4: 0,
            violating_angles_at_degree_ge_5: 1,
            unmapped_identity_count: 0,
            attribution_closure_error_count: 0,
            lineage_angle_exposure,
            triangle_context_angle_exposure,
            violations: vec![violation.clone()],
        };
        let mut session = HarpTraceSession::create(target.clone()).unwrap();
        write_core_event(
            &mut session,
            &HarpTraceEvent::AngleViolation {
                stage: HarpTraceStage::PostEta,
                violation,
            },
        )
        .unwrap();
        write_core_event(
            &mut session,
            &HarpTraceEvent::StageSummary {
                stage: HarpTraceStage::Input,
                certification,
            },
        )
        .unwrap();
        for stage in HarpTraceStage::ALL.into_iter().skip(1) {
            session
                .write_stage_summary(
                    stage,
                    &TestRecord {
                        record_type: "stage_summary",
                        stage_index: usize::from(stage.index()),
                        stage_name: stage.name(),
                        angle_deg: None,
                    },
                )
                .unwrap();
        }
        session.publish(&test_report()).unwrap();
        let rows = jsonl(&target);
        assert_eq!(rows[1]["record_type"], "angle_violation");
        assert_eq!(rows[1]["stage_index"], 3);
        assert_eq!(rows[1]["kind"], "above_80");
        assert_eq!(rows[1]["triangle_sites"], serde_json::json!([3, 4, 5]));
        assert_eq!(rows[1]["corner_site"], 4);
        assert!(rows[1].get("triangle").is_none());
        assert!(rows[1].get("corner_vertex").is_none());
        assert_eq!(rows[1]["birth_candidate_source"], "off_centre");
        assert_eq!(rows[1]["lineage_depth_span"], 2);
        assert_eq!(rows[1]["raw_target_coverage_count"], 3);
        assert_eq!(rows[1]["refinement_boundary_class"], "both");
        assert_eq!(
            rows[1]["raw_criterion_target_gradient_to_limit_ratio"],
            1.25
        );
        assert_eq!(
            rows[1]["raw_criterion_target_gradient_to_limit_ratio_measurable"],
            true
        );
        assert!(rows[1]["frozen_gradated_target_gradient_to_limit_ratio"].is_null());
        assert_eq!(
            rows[1]["frozen_gradated_target_gradient_to_limit_ratio_measurable"],
            false
        );
        assert!(rows[1]["angle_deg"].is_null());
        assert_eq!(rows[1]["angle_deg_measurable"], false);
        assert!(rows[1]["realized_to_raw_criterion_target_scale_ratio"].is_null());
        assert!(rows[1].get("realized_to_target_scale_ratio").is_none());
        assert_eq!(rows[2]["record_type"], "stage_summary");
        assert_eq!(rows[2]["stage_summary_count"], Value::Null);
        assert_eq!(
            rows[2]["lineage_angle_exposure"][0]["birth_source_class"],
            "inherited"
        );
        assert_eq!(
            rows[2]["lineage_angle_exposure"][1]["birth_source_class"],
            "off_centre"
        );
        assert_eq!(rows[2]["lineage_angle_exposure"][1]["active_site_count"], 3);
        assert_eq!(
            rows[2]["triangle_context_angle_exposure"][0]["refinement_boundary_class"],
            "both"
        );
        assert_eq!(
            rows[2]["triangle_context_angle_exposure"][0]["raw_criterion_target_gradient_bin"],
            "gt_1_le_2"
        );
        assert_eq!(
            rows[2]["triangle_context_angle_exposure"][0]["frozen_gradated_target_gradient_bin"],
            "unavailable"
        );
        assert_eq!(rows.last().unwrap()["stage_summary_count"], STAGE_COUNT);
    }

    #[test]
    fn degree_four_audit_events_are_stable_snake_case_json() {
        use earthmesh_refine_harp_dv::{
            DegreeFourCheckCounts, DegreeFourCheckStatus, DegreeFourRetirementCheckCounts,
            DegreeFourRetirementSite, DegreeFourRetirementSummary, DegreeFourRetirementTrial,
            HarpTraceEvent, SiteId,
        };
        let target = temp_path("d4.jsonl");
        let mut session = HarpTraceSession::create(target.clone()).unwrap();
        write_core_event(
            &mut session,
            &HarpTraceEvent::DegreeFourRetirementSummary(DegreeFourRetirementSummary {
                evaluated: true,
                sites_total: 3,
                sites_not_leaf: 1,
                sites_eligible: 2,
                sites_without_window_violation: 1,
                sites_audited: 1,
                sites_ranked_beyond_64: 0,
                sites_with_any_valid_trial: 1,
                sites_with_any_fully_acceptable_trial: 0,
                sites_committed: 0,
                trials_total: 2,
                checks: DegreeFourRetirementCheckCounts {
                    geometry: DegreeFourCheckCounts {
                        pass: 1,
                        fail: 1,
                        not_evaluated: 0,
                    },
                    hard_gate: DegreeFourCheckCounts {
                        pass: 1,
                        fail: 0,
                        not_evaluated: 1,
                    },
                    physical_demand: DegreeFourCheckCounts {
                        pass: 0,
                        fail: 1,
                        not_evaluated: 1,
                    },
                    ..DegreeFourRetirementCheckCounts::default()
                },
                trials_quality_improving: 0,
                trials_fully_acceptable: 0,
            }),
        )
        .unwrap();
        write_core_event(
            &mut session,
            &HarpTraceEvent::DegreeFourRetirementSite(DegreeFourRetirementSite {
                site_id: SiteId(10),
                vertex: 12,
                interior_leaf: true,
                window_violation: true,
                candidate_rank: Some(4),
                ranked_beyond_64: false,
                trial_count: 2,
                any_valid_trial: true,
                any_fully_acceptable_trial: false,
                committed: false,
            }),
        )
        .unwrap();
        write_core_event(
            &mut session,
            &HarpTraceEvent::DegreeFourRetirementTrial(DegreeFourRetirementTrial {
                site_id: SiteId(10),
                vertex: 12,
                trial_index: 1,
                ring_site_ids: Some([SiteId(1), SiteId(2), SiteId(3), SiteId(4)]),
                diagonal_site_ids: Some([SiteId(2), SiteId(4)]),
                geometry: DegreeFourCheckStatus::Pass,
                hard_gate: DegreeFourCheckStatus::Fail,
                physical_demand: DegreeFourCheckStatus::NotEvaluated,
                scale_balance: DegreeFourCheckStatus::Pass,
                no_new_low_degree: DegreeFourCheckStatus::Pass,
                angle_count: DegreeFourCheckStatus::Fail,
                worst_deviation: DegreeFourCheckStatus::Pass,
                penalty: DegreeFourCheckStatus::Fail,
                eta: DegreeFourCheckStatus::Pass,
                margin: DegreeFourCheckStatus::Pass,
                conservative_remap: DegreeFourCheckStatus::NotEvaluated,
                fully_acceptable: false,
            }),
        )
        .unwrap();
        write_dummy_stage_summaries(&mut session);
        let mut report = test_report();
        report.d4_leaf_retirement_audit_evaluated = true;
        report.d4_leaf_retirement_candidates = 1;
        report.d4_leaf_retirement_trials_total = 2;
        report.d4_leaf_retirement_triangulations = 2;
        report.d4_leaf_retirement_fully_acceptable = 0;
        session.publish(&report).unwrap();
        let rows = jsonl(&target);
        assert_eq!(rows[1]["record_type"], "degree_four_retirement_summary");
        assert_eq!(rows[1]["evaluated"], true);
        assert_eq!(rows[1]["sites_total"], 3);
        assert_eq!(rows[1]["checks"]["geometry"]["pass"], 1);
        assert_eq!(rows[1]["checks"]["geometry"]["fail"], 1);
        assert_eq!(rows[1]["checks"]["hard_gate"]["not_evaluated"], 1);
        assert_eq!(rows[2]["record_type"], "degree_four_retirement_site");
        assert_eq!(rows[2]["site_id"], 10);
        assert_eq!(rows[2]["candidate_rank"], 4);
        assert_eq!(rows[2]["trial_count"], 2);
        assert_eq!(rows[3]["record_type"], "degree_four_retirement_trial");
        assert_eq!(rows[3]["ring_site_ids"], serde_json::json!([1, 2, 3, 4]));
        assert_eq!(rows[3]["diagonal_site_ids"], serde_json::json!([2, 4]));
        assert_eq!(rows[3]["geometry"], "pass");
        assert_eq!(rows[3]["hard_gate"], "fail");
        assert_eq!(rows[3]["physical_demand"], "not_evaluated");
        assert_eq!(
            rows.last().unwrap()["d4_leaf_retirement_audit_evaluated"],
            true
        );
        assert_eq!(
            rows.last().unwrap()["d4_leaf_retirement_trials_evaluated"],
            2
        );
    }

    #[test]
    fn angle_violation_without_stable_key_is_rejected() {
        use earthmesh_refine_harp_dv::certifier::RefinementBoundaryClass;
        use earthmesh_refine_harp_dv::{
            AngleViolation, AngleViolationKind, HarpTraceEvent, HarpTraceStage,
        };
        let target = temp_path("missing-key.jsonl");
        let mut session = HarpTraceSession::create(target).unwrap();
        let violation = AngleViolation {
            key: None,
            kind: AngleViolationKind::Below40,
            triangle: 1,
            corner_vertex: 2,
            angle_deg: 30.0,
            corner_degree: 4,
            triangle_degree_triplet: [4, 5, 6],
            refinement_depth: None,
            birth_cycle: None,
            birth_candidate_source: None,
            lineage_depth_span: Some(0),
            raw_target_coverage_count: 0,
            refinement_boundary_class: RefinementBoundaryClass::Neither,
            raw_criterion_target_gradient_to_limit_ratio: None,
            frozen_gradated_target_gradient_to_limit_ratio: None,
            realized_to_raw_criterion_target_scale_ratio: None,
        };
        let error = write_core_event(
            &mut session,
            &HarpTraceEvent::AngleViolation {
                stage: HarpTraceStage::Input,
                violation,
            },
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn window_budget_summaries_are_compact_stable_json() {
        use earthmesh_refine_harp_dv::trace::{
            WindowBudgetArm, WindowBudgetArmSummary, WindowBudgetPassSummary,
            WindowBudgetStopReason,
        };

        let pass = serde_json::to_value(JsonWindowBudgetPassSummary::from_summary(
            &WindowBudgetPassSummary {
                arm: WindowBudgetArm::W64,
                pass_index: 33,
                window_pass_limit: 64,
                per_pass_site_budget: 1_024,
                processed_sites: 800,
                eligible_sites: 800,
                found_sites: 1_600,
                unique_sites_seen: 8_000,
                candidate_count: 2_100,
                line_search_attempt_count: 3_200,
                retained_move_count: 700,
                completed_breadth_sweep: true,
                below_40_count: 10,
                above_80_count: 20,
                total_violation_count: 30,
                resolved_s3_cohort_key_count: 80,
                persisted_s3_cohort_key_count: 20,
                kind_changed_s3_cohort_key_count: 2,
                new_global_angle_key_count: 3,
                worst_window_deviation_deg: 4.5,
                window_penalty: 9.25,
                eta_min: 0.8,
                eta_p1: f64::NAN,
                physical_demands_remaining: 0,
                balance_demands_remaining: 0,
                unbalanced_pairs_remaining: 0,
                wall_time_ms: 123,
                stop_reason_if_terminal: Some(WindowBudgetStopReason::PassLimit),
            },
        ))
        .unwrap();
        assert_eq!(pass["record_type"], "window_budget_pass_summary");
        assert_eq!(pass["arm"], "W64");
        assert_eq!(pass["stop_reason_if_terminal"], "pass_limit");
        assert!(pass["eta_p1"].is_null());
        assert_eq!(pass["eta_p1_measurable"], false);
        assert!(pass.get("angle_violation").is_none());

        let arm = serde_json::to_value(JsonWindowBudgetArmSummary::from_summary(
            &WindowBudgetArmSummary {
                arm: WindowBudgetArm::W96,
                window_pass_limit: 96,
                pass_count: 72,
                s3_violation_key_count: 100,
                s4_below_40_count: 4,
                s4_above_80_count: 5,
                s4_total_violation_count: 9,
                s4_worst_window_deviation_deg: 3.0,
                s4_window_penalty: 8.0,
                s4_eta_min: 0.81,
                s4_eta_p1: 0.9,
                s4_physical_demands_remaining: 0,
                s4_balance_demands_remaining: 0,
                s4_unbalanced_pairs_remaining: 0,
                s4_resolved_s3_cohort_key_count: 91,
                s4_persisted_s3_cohort_key_count: 9,
                s4_kind_changed_s3_cohort_key_count: 1,
                s4_new_global_angle_key_count: 2,
                s6_below_40_count: 3,
                s6_above_80_count: 4,
                s6_total_violation_count: 7,
                s6_worst_window_deviation_deg: 2.5,
                s6_window_penalty: 6.0,
                s6_eta_min: 0.82,
                s6_eta_p1: 0.91,
                s6_physical_demands_remaining: 0,
                s6_balance_demands_remaining: 0,
                s6_unbalanced_pairs_remaining: 0,
                s6_resolved_s3_cohort_key_count: 93,
                s6_persisted_s3_cohort_key_count: 7,
                s6_kind_changed_s3_cohort_key_count: 0,
                s6_new_global_angle_key_count: 1,
                final_low_degree_moves: 2,
                default_leaf_retirements: 1,
                wall_time_ms: 456,
                stop_reason: WindowBudgetStopReason::CompletedNoImprovementSweep,
            },
        ))
        .unwrap();
        assert_eq!(arm["record_type"], "window_budget_arm_summary");
        assert_eq!(arm["arm"], "W96");
        assert_eq!(arm["stop_reason"], "completed_no_improvement_sweep");
        assert_eq!(arm["s3_violation_key_count"], 100);
        assert_eq!(arm["s6_total_violation_count"], 7);
    }

    #[test]
    fn flush_error_propagates() {
        struct FailingFlush(Vec<u8>);
        impl Write for FailingFlush {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                self.0.extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "flush boom"))
            }
        }
        let mut writer = TraceLineWriter::new(FailingFlush(Vec::new()));
        writer
            .write_stage_summary(
                HarpTraceStage::Input,
                &TestRecord {
                    record_type: "stage_summary",
                    stage_index: 0,
                    stage_name: "input",
                    angle_deg: None,
                },
            )
            .unwrap();
        let error = writer.inner.flush().unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    }

    #[test]
    fn write_error_propagates() {
        struct FailingWrite;
        impl Write for FailingWrite {
            fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "boom"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut writer = TraceLineWriter::new(FailingWrite);
        let error = writer
            .write_stage_summary(
                HarpTraceStage::Input,
                &TestRecord {
                    record_type: "stage_summary",
                    stage_index: 0,
                    stage_name: "input",
                    angle_deg: None,
                },
            )
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert_eq!(writer.event_count, 0);
    }
}
