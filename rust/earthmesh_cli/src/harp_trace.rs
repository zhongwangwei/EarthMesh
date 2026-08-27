use serde::Serialize;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) const ENV_VAR: &str = "EARTHMESH_HARP_TRACE_JSONL";
pub(crate) const SCHEMA_VERSION: u32 = 1;
pub(crate) const STAGE_COUNT: usize = 7;

static PARTIAL_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) fn from_env() -> io::Result<Option<HarpTraceSession>> {
    let Some(value) = env::var_os(ENV_VAR) else {
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

    pub(crate) fn write_stage_summary<T: Serialize>(&mut self, record: &T) -> io::Result<()> {
        self.writer_mut()?.write_counted_record(record, true)
    }

    pub(crate) fn write_event<T: Serialize>(&mut self, record: &T) -> io::Result<()> {
        self.writer_mut()?.write_counted_record(record, false)
    }

    pub(crate) fn publish(mut self) -> io::Result<()> {
        let mut writer = self.writer.take().ok_or_else(|| {
            io::Error::other("HARP trace session was already closed before publish")
        })?;
        if writer.stage_summary_count != STAGE_COUNT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "HARP trace has {} stage_summary records; expected {STAGE_COUNT}",
                    writer.stage_summary_count
                ),
            ));
        }
        writer.write_run_end()?;
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
    stage_summary_count: usize,
}

impl<W: Write> TraceLineWriter<W> {
    fn new(inner: W) -> Self {
        Self {
            inner,
            event_count: 0,
            stage_summary_count: 0,
        }
    }

    fn write_record<T: Serialize>(&mut self, record: &T) -> io::Result<()> {
        serde_json::to_writer(&mut self.inner, record).map_err(io::Error::other)?;
        self.inner.write_all(b"\n")
    }

    fn write_counted_record<T: Serialize>(
        &mut self,
        record: &T,
        is_stage_summary: bool,
    ) -> io::Result<()> {
        self.write_record(record)?;
        self.event_count += 1;
        if is_stage_summary {
            self.stage_summary_count += 1;
        }
        Ok(())
    }

    fn write_run_end(&mut self) -> io::Result<()> {
        self.write_record(&RunEnd {
            record_type: "harp_run_end",
            event_count: self.event_count,
            stage_summary_count: self.stage_summary_count,
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
        } => session
            .write_stage_summary(&JsonStageSummary::from_certification(*stage, certification)),
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
            violation_count: certification.violations.len(),
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
    realized_to_target_scale_ratio: Option<f64>,
    realized_to_target_scale_ratio_measurable: bool,
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
        let (realized_to_target_scale_ratio, realized_to_target_scale_ratio_measurable) =
            optional_finite(violation.realized_to_target_scale_ratio);
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
            realized_to_target_scale_ratio,
            realized_to_target_scale_ratio_measurable,
        })
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
    use std::sync::{Mutex, OnceLock};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

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
        for stage_index in 0..STAGE_COUNT {
            session
                .write_stage_summary(&TestRecord {
                    record_type: "stage_summary",
                    stage_index,
                    stage_name: "stage",
                    angle_deg: None,
                })
                .unwrap();
        }
    }

    #[test]
    fn unset_env_disables_trace() {
        let _guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        env::remove_var(ENV_VAR);
        assert!(from_env().unwrap().is_none());
    }

    #[test]
    fn relative_env_path_is_rejected() {
        let _guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        env::set_var(ENV_VAR, "trace.jsonl");
        let error = match from_env() {
            Ok(_) => panic!("relative trace path unexpectedly accepted"),
            Err(error) => error,
        };
        env::remove_var(ENV_VAR);
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
        let error = session.publish().unwrap_err();
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
        for stage_index in 0..STAGE_COUNT {
            session
                .write_stage_summary(&TestRecord {
                    record_type: "stage_summary",
                    stage_index,
                    stage_name: "stage",
                    angle_deg: None,
                })
                .unwrap();
        }
        session.publish().unwrap();
        let rows = jsonl(&target);
        assert_eq!(rows.first().unwrap()["record_type"], "run_header");
        assert_eq!(rows.first().unwrap()["stage_count"], STAGE_COUNT);
        assert_eq!(rows.last().unwrap()["record_type"], "harp_run_end");
        assert_eq!(rows.last().unwrap()["stage_summary_count"], STAGE_COUNT);
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
            session.publish().unwrap();
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
        for stage_index in 0..(STAGE_COUNT - 1) {
            session
                .write_stage_summary(&TestRecord {
                    record_type: "stage_summary",
                    stage_index,
                    stage_name: "stage",
                    angle_deg: None,
                })
                .unwrap();
        }
        let error = session.publish().unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(!target.exists());
        assert!(partial.exists());
        assert!(!fs::read_to_string(partial)
            .unwrap()
            .contains("harp_run_end"));
    }

    #[test]
    fn core_events_are_stable_snake_case_json() {
        use earthmesh_refine_harp_dv::{
            AngleKey, AngleViolation, AngleViolationKind, CandidateSource, HarpTraceEvent,
            HarpTraceStage, MeshCertification, SiteId,
        };
        let target = temp_path("core.jsonl");
        let mut degree_histogram = std::collections::BTreeMap::new();
        degree_histogram.insert(6, 4);
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
            realized_to_target_scale_ratio: Some(f64::NAN),
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
                stage: HarpTraceStage::PostEta,
                certification,
            },
        )
        .unwrap();
        for stage_index in 1..STAGE_COUNT {
            session
                .write_stage_summary(&TestRecord {
                    record_type: "stage_summary",
                    stage_index,
                    stage_name: "stage",
                    angle_deg: None,
                })
                .unwrap();
        }
        session.publish().unwrap();
        let rows = jsonl(&target);
        assert_eq!(rows[1]["record_type"], "angle_violation");
        assert_eq!(rows[1]["stage_index"], 3);
        assert_eq!(rows[1]["kind"], "above_80");
        assert_eq!(rows[1]["triangle_sites"], serde_json::json!([3, 4, 5]));
        assert_eq!(rows[1]["corner_site"], 4);
        assert!(rows[1].get("triangle").is_none());
        assert!(rows[1].get("corner_vertex").is_none());
        assert_eq!(rows[1]["birth_candidate_source"], "off_centre");
        assert!(rows[1]["angle_deg"].is_null());
        assert_eq!(rows[1]["angle_deg_measurable"], false);
        assert!(rows[1]["realized_to_target_scale_ratio"].is_null());
        assert_eq!(rows[2]["record_type"], "stage_summary");
        assert_eq!(rows[2]["stage_summary_count"], Value::Null);
        assert_eq!(rows.last().unwrap()["stage_summary_count"], STAGE_COUNT);
    }

    #[test]
    fn angle_violation_without_stable_key_is_rejected() {
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
            realized_to_target_scale_ratio: None,
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
            .write_counted_record(
                &TestRecord {
                    record_type: "stage_summary",
                    stage_index: 0,
                    stage_name: "input",
                    angle_deg: None,
                },
                true,
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
            .write_counted_record(
                &TestRecord {
                    record_type: "stage_summary",
                    stage_index: 0,
                    stage_name: "input",
                    angle_deg: None,
                },
                true,
            )
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert_eq!(writer.event_count, 0);
    }
}
