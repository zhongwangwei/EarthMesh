//! Mesh quality metrics + reports for EarthMesh v3 (MVP).
//!
//! Computes geometry / topology / refinement-transition metrics from a light,
//! engine-agnostic [`QualityMeshInput`] (so this crate stays free of the heavy
//! `netcdf`-linked crates and is unit-testable). Callers (CLI/GUI/mesh) build the
//! input from their own mesh representation. Output writers live in [`io`].
//!
//! Cell areas and edge lengths are spherical km²/km metrics via
//! `earthmesh_geometry`; overlay/fraction internals still carry their own planar
//! caveats. The public API is kept self-contained so it can graduate into its own
//! `earthmesh_quality` deliverable without churn (it already is one crate).

use earthmesh_geometry::{
    haversine_km, try_spherical_polygon_area, Point, SphericalArea, SphericalPolygonError,
    SphericalWinding, EARTH_RADIUS_KM,
};

pub mod coupling;
pub mod io;
pub mod topology;

/// Pass / warn / fail level for one gate or the whole report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum QualityLevel {
    #[default]
    Pass,
    Warn,
    Fail,
}

impl QualityLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            QualityLevel::Pass => "pass",
            QualityLevel::Warn => "warn",
            QualityLevel::Fail => "fail",
        }
    }
    fn worse(self, other: QualityLevel) -> QualityLevel {
        use QualityLevel::*;
        match (self, other) {
            (Fail, _) | (_, Fail) => Fail,
            (Warn, _) | (_, Warn) => Warn,
            _ => Pass,
        }
    }

    pub fn is_worse_than(self, other: QualityLevel) -> bool {
        quality_level_rank(self) > quality_level_rank(other)
    }
}

fn quality_level_rank(level: QualityLevel) -> u8 {
    match level {
        QualityLevel::Pass => 0,
        QualityLevel::Warn => 1,
        QualityLevel::Fail => 2,
    }
}

/// Direction in which a guarded quality metric improves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QualityMetricPreference {
    Lower,
    Higher,
}

impl QualityMetricPreference {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lower => "lower",
            Self::Higher => "higher",
        }
    }
}

/// One guarded metric that makes an AutoRefine candidate unsafe to accept.
#[derive(Clone, Debug, PartialEq)]
pub struct QualityMetricRegression {
    pub metric: String,
    pub preference: QualityMetricPreference,
    pub baseline: f64,
    pub candidate: f64,
}

impl QualityMetricRegression {
    pub fn delta(&self) -> f64 {
        self.candidate - self.baseline
    }
}

/// min/max/mean/std/CV of a sample.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stat5 {
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub std: f64,
    pub cv: f64,
}

impl Stat5 {
    pub fn from_slice(values: &[f64]) -> Stat5 {
        if values.is_empty() {
            return Stat5::default();
        }
        let n = values.len() as f64;
        let mean = values.iter().sum::<f64>() / n;
        let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
        let std = var.sqrt();
        Stat5 {
            min: values.iter().cloned().fold(f64::INFINITY, f64::min),
            max: values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            mean,
            std,
            cv: if mean.abs() > 0.0 {
                std / mean.abs()
            } else {
                0.0
            },
        }
    }

    fn from_slice_or_nan(values: &[f64]) -> Stat5 {
        if values.is_empty() {
            return Stat5 {
                min: f64::NAN,
                max: f64::NAN,
                mean: f64::NAN,
                std: f64::NAN,
                cv: f64::NAN,
            };
        }
        Self::from_slice(values)
    }
}

/// One cell of the quality input mesh.
#[derive(Clone, Debug, Default)]
pub struct QualityCell {
    /// Indices into [`QualityMeshInput::vertices`], an open ring (no repeated closing vertex).
    pub vertices: Vec<usize>,
    /// Refinement level (base = lowest); enables refinement/transition metrics.
    pub refine_level: Option<u32>,
    /// Adjacent cell indices; enables neighbor-reciprocity / orphan / transition metrics.
    pub neighbors: Vec<usize>,
}

/// Engine-agnostic mesh for quality analysis. `vertices` are (lon, lat) degrees.
#[derive(Clone, Debug, Default)]
pub struct QualityMeshInput {
    pub vertices: Vec<Point>,
    pub cells: Vec<QualityCell>,
}

/// Geometry metrics (spherical areas, great-circle edge lengths).
#[derive(Clone, Debug, Default)]
pub struct GeometryMetrics {
    pub cell_count: usize,
    pub vertex_count: usize,
    pub edge_count: usize,
    pub cell_area: Stat5,
    pub cell_area_ratio: f64,
    pub edge_length_km: Stat5,
    /// Per-cell edge-length CV; catches skew that a global edge statistic hides.
    pub cell_edge_length_cv: Stat5,
    pub min_angle_deg: f64,
    pub max_angle_deg: f64,
    /// Per-cell max deviation from the equal-angle spherical n-gon with the
    /// same spherical excess.
    pub angle_deviation_deg: Stat5,
    /// Local Euclidean compatibility metric; omitted for triangles with an edge
    /// longer than 15 degrees.
    pub triangle_eta: Stat5,
    /// Local Euclidean compatibility metric; omitted for triangles with an edge
    /// longer than 15 degrees.
    pub triangle_nsr: Stat5,
    pub aspect_ratio: Stat5,
    /// Exact spherical isoperimetric quotient `a(4π-a)/l²`.
    pub compactness: Stat5,
    /// Cells eligible for the local triangle eta/NSR compatibility metrics.
    pub local_shape_metric_sample_count: usize,
    /// Valid cells omitted from local eta/NSR because at least one great-circle
    /// edge spans more than 15 degrees. Exact spherical angles, compactness,
    /// area, and edge statistics remain available.
    pub local_shape_metric_excluded_cell_count: usize,
    pub zero_area_cell_count: usize,
    pub negative_area_cell_count: usize,
    /// Cells with a NaN/Inf vertex coordinate; excluded from all geometry stats.
    pub non_finite_cell_count: usize,
    pub self_intersection_count: usize,
    pub invalid_polygon_count: usize,
}

/// Topology + refinement-transition metrics.
#[derive(Clone, Debug, Default)]
pub struct TopologyMetrics {
    /// `used vertices - unique valid edges + valid cells`; informational because
    /// the expected value depends on global/regional boundaries and holes.
    pub euler_characteristic: isize,
    /// Caller-provided expectation, when the mesh domain topology is known.
    pub expected_euler_characteristic: Option<isize>,
    /// `1` when an explicit expectation differs from the measured value, else `0`.
    pub euler_characteristic_mismatch_count: usize,
    /// Number of cell components connected through shared edges.
    pub connected_component_count: usize,
    /// Vertices whose incident cells form more than one disconnected fan.
    pub non_manifold_vertex_fan_count: usize,
    pub invalid_vertex_index_count: usize,
    pub invalid_cell_index_count: usize,
    pub duplicate_edge_count: usize,
    pub dangling_edge_count: usize,
    /// Edges with exactly one incident cell; informational for regional meshes.
    pub boundary_edge_count: usize,
    /// Closed components in the graph formed by single-incidence edges.
    ///
    /// A closed global mesh has none. More than one means the domain has
    /// several separate rims -- an interior hole, or a piece the carve detached
    /// -- which the edge count alone cannot distinguish from one long coastline.
    pub boundary_loop_count: usize,
    /// Boundary vertices whose single-incidence edge degree is not two.
    ///
    /// A rim passes through each of its vertices once, so degree two is the
    /// only well-formed case. Anything else is a rim that branches or pinches,
    /// which downstream tools read as a broken boundary.
    pub boundary_vertex_degree_violation_count: usize,
    /// Shared edges traversed in the same direction by both incident cells.
    pub misoriented_shared_edge_count: usize,
    /// Closed cells whose declared neighbors do not match edge-derived neighbors.
    pub neighbor_degree_mismatch_count: usize,
    pub orphan_cell_count: usize,
    pub neighbor_reciprocity_failure_count: usize,
    pub abnormal_polygon_edge_count: usize,
    pub triangle_cell_count: usize,
    pub quadrilateral_cell_count: usize,
    pub pentagon_cell_count: usize,
    pub hexagon_cell_count: usize,
    pub heptagon_cell_count: usize,
    pub other_polygon_cell_count: usize,
    pub isolated_refined_cell_count: usize,
    pub max_adjacent_resolution_ratio: f64,
    pub transition_continuity_warning_count: usize,
}

/// One gate evaluation.
#[derive(Clone, Debug)]
pub struct GateResult {
    pub metric: String,
    pub value: f64,
    pub level: QualityLevel,
    pub detail: String,
}

/// Quality summary for cells sharing the same refinement level.
#[derive(Clone, Debug, Default)]
pub struct RefineLevelQualitySummary {
    pub refine_level: Option<u32>,
    pub cell_count: usize,
    pub cell_area: Stat5,
    pub cell_edge_length_cv: Stat5,
    pub angle_deviation_deg: Stat5,
    pub triangle_eta: Stat5,
    pub triangle_nsr: Stat5,
}

/// Count of cells assigned to one h-field/refinement level.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LevelCount {
    pub level: u32,
    pub count: usize,
}

/// Effective h-field controls recorded with a quality report.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HfieldConfigDiagnostics {
    pub enabled: bool,
    pub g: Option<f64>,
    pub max_level: Option<u32>,
    pub base_m: Option<f64>,
}

/// Optional diagnostics for h-field driven refinement.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HfieldDiagnostics {
    pub config: HfieldConfigDiagnostics,
    pub cell_count: usize,
    pub target_level_distribution: Vec<LevelCount>,
    pub actual_refine_level_distribution: Vec<LevelCount>,
    pub missing_target_level_count: usize,
    pub extra_target_level_count: usize,
    pub missing_actual_refine_level_count: usize,
    pub target_actual_mismatch_count: usize,
    pub target_above_actual_count: usize,
    pub actual_above_target_count: usize,
    pub max_target_actual_delta: u32,
    pub max_adjacent_target_level_jump: u32,
    pub target_level_jump_gt_one_count: usize,
    pub max_adjacent_actual_level_jump: u32,
    pub actual_level_jump_gt_one_count: usize,
}

/// What the point+radius route asked for and what the mesh delivered.
///
/// The reconciliation is the question the h-field's diagnostics answer — did
/// every cell reach the level something asked it to — so the counts carry the
/// same names. `pass_count` and `circle_count` are what only this route has: it
/// refines one level at a time and can say how much each pass emitted.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AdaptiveDiagnostics {
    pub enabled: bool,
    pub max_level: Option<u32>,
    pub base_m: Option<f64>,
    pub coastline: bool,
    /// Levels actually refined. Fewer than `max_level` means the loop stopped
    /// because nothing asked for more, which is a normal outcome.
    pub pass_count: usize,
    pub circle_count: usize,
    pub cell_count: usize,
    pub target_level_distribution: Vec<LevelCount>,
    pub actual_refine_level_distribution: Vec<LevelCount>,
    pub missing_target_level_count: usize,
    pub extra_target_level_count: usize,
    pub missing_actual_refine_level_count: usize,
    pub target_actual_mismatch_count: usize,
    pub target_above_actual_count: usize,
    pub actual_above_target_count: usize,
    pub max_target_actual_delta: u32,
    pub max_adjacent_target_level_jump: u32,
    pub target_level_jump_gt_one_count: usize,
    pub max_adjacent_actual_level_jump: u32,
    pub actual_level_jump_gt_one_count: usize,
}

/// How a run configured the point+radius route.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AdaptiveConfigDiagnostics {
    pub enabled: bool,
    pub max_level: Option<u32>,
    pub base_m: Option<f64>,
    pub coastline: bool,
    pub pass_count: usize,
    pub circle_count: usize,
}

/// A worst-offending cell for the GeoJSON layer.
#[derive(Clone, Debug)]
pub struct WorstCell {
    pub cell_index: usize,
    /// Zero-based Method-C refinement level carried by the measured cell.
    pub refine_level: Option<u32>,
    pub centroid: Point,
    pub ring: Vec<Point>,
    pub metric: String,
    pub value: f64,
    pub level: QualityLevel,
}

impl WorstCell {
    pub fn is_refinement_repairable(&self) -> bool {
        matches!(
            self.metric.as_str(),
            "min_angle_deg" | "aspect_ratio" | "cell_edge_length_cv" | "angle_deviation_deg"
        )
    }
}

/// Conservative default thresholds; future config can override fields.
#[derive(Clone, Copy, Debug)]
pub struct QualityThresholds {
    pub min_angle_warn_deg: f64,
    pub min_angle_fail_deg: f64,
    pub angle_deviation_warn_deg: f64,
    pub aspect_ratio_warn: f64,
    pub aspect_ratio_fail: f64,
    pub cell_edge_cv_warn: f64,
    pub area_cv_warn: f64,
    pub max_adjacent_resolution_ratio_warn: f64,
    pub worst_cells_limit: usize,
    /// Maximum number of mutually connected cells in one local repair pass.
    /// This is deliberately independent of `worst_cells_limit`, which controls
    /// diagnostics rather than how much mesh is changed at once.
    pub repair_batch_limit: usize,
    /// Exclusive refinement-level ceiling for repair candidates. `None` keeps
    /// standalone quality analysis independent of a Project resolution cap.
    pub repair_level_cap: Option<u32>,
}

impl Default for QualityThresholds {
    fn default() -> Self {
        // Conservative: catastrophic topology = fail; suspicious geometry = warn.
        Self {
            min_angle_warn_deg: earthmesh_core::DEFAULT_MIN_ANGLE_WARN_DEG,
            min_angle_fail_deg: 5.0,
            angle_deviation_warn_deg: 35.0,
            aspect_ratio_warn: 4.0,
            aspect_ratio_fail: 10.0,
            cell_edge_cv_warn: 0.35,
            area_cv_warn: 1.5,
            max_adjacent_resolution_ratio_warn: 2.0,
            worst_cells_limit: 50,
            repair_batch_limit: 1,
            repair_level_cap: None,
        }
    }
}

/// Optional topology context for quality computation.
///
/// The legacy [`compute`] entry point uses `None`, because an Euler expectation
/// cannot be inferred safely from connectivity alone (regional holes and
/// multipart domains change it).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QualityComputationOptions {
    pub expected_euler_characteristic: Option<isize>,
}

/// Full quality report. `to_*` serializers are in [`io`].
#[derive(Clone, Debug)]
pub struct MeshQualityReport {
    pub mesh_name: String,
    /// Which cell view was measured (`tri`, `hex`, or empty when the caller does
    /// not provide that context).
    pub cell_view: String,
    pub tool_version: String,
    pub geometry: GeometryMetrics,
    pub topology: TopologyMetrics,
    /// Per-refinement-level quality rollup; `None` means the input carried no level.
    pub refine_level_groups: Vec<RefineLevelQualitySummary>,
    /// Optional h-field target-vs-actual diagnostics, attached by callers that
    /// have sampled target levels for the measured cell view.
    pub hfield: Option<HfieldDiagnostics>,
    /// Optional point+radius target-vs-actual diagnostics. Only one of this and
    /// `hfield` is ever set: a run refines one way or the other.
    pub adaptive: Option<AdaptiveDiagnostics>,
    pub gates: Vec<GateResult>,
    pub worst_cells: Vec<WorstCell>,
    /// Geometry defects that can be repaired by one bounded local refinement
    /// pass. Kept separately so invalid cells cannot consume the display limit.
    pub repair_cells: Vec<WorstCell>,
    /// Structured topology problems from [`topology::MeshTopologyValidator`].
    pub topology_issues: Vec<topology::TopologyIssue>,
    pub verdict: QualityLevel,
}

impl MeshQualityReport {
    /// A failed report that local refinement cannot safely repair.
    pub fn has_unrepairable_failure(&self) -> bool {
        self.verdict == QualityLevel::Fail
            && (self.repair_cells.is_empty()
                || self.worst_cells.iter().any(|cell| {
                    cell.level == QualityLevel::Fail && !cell.is_refinement_repairable()
                })
                || topology::worst_severity(&self.topology_issues)
                    == Some(topology::Severity::Fail))
    }

    /// Structured reasons why this candidate may not replace `baseline`.
    ///
    /// Level-valued metrics use ranks `pass=0`, `warn=1`, `fail=2`; a missing
    /// candidate gate is rank `3`; a candidate-only non-pass gate is compared
    /// with an implicit pass baseline. Count and shape metrics retain their
    /// native numeric values. This is the same guarded set used by
    /// [`Self::is_strict_improvement_over`].
    pub fn guarded_metric_regressions(&self, baseline: &Self) -> Vec<QualityMetricRegression> {
        fn push(
            regressions: &mut Vec<QualityMetricRegression>,
            metric: impl Into<String>,
            preference: QualityMetricPreference,
            baseline: f64,
            candidate: f64,
        ) {
            regressions.push(QualityMetricRegression {
                metric: metric.into(),
                preference,
                baseline,
                candidate,
            });
        }

        let mut regressions = Vec::new();
        if self.has_unrepairable_failure() {
            push(
                &mut regressions,
                "unrepairable_failure",
                QualityMetricPreference::Lower,
                f64::from(u8::from(baseline.has_unrepairable_failure())),
                1.0,
            );
        }
        if self.verdict.is_worse_than(baseline.verdict) {
            push(
                &mut regressions,
                "verdict.level",
                QualityMetricPreference::Lower,
                f64::from(quality_level_rank(baseline.verdict)),
                f64::from(quality_level_rank(self.verdict)),
            );
        }
        for prior in &baseline.gates {
            let candidate = self
                .gates
                .iter()
                .find(|candidate| candidate.metric == prior.metric);
            let candidate_rank = candidate
                .map(|gate| quality_level_rank(gate.level))
                .unwrap_or(3);
            if candidate.is_none_or(|gate| gate.level.is_worse_than(prior.level)) {
                push(
                    &mut regressions,
                    format!("gate.{}.level", prior.metric),
                    QualityMetricPreference::Lower,
                    f64::from(quality_level_rank(prior.level)),
                    f64::from(candidate_rank),
                );
            }
        }
        for candidate in &self.gates {
            if candidate.level != QualityLevel::Pass
                && !baseline
                    .gates
                    .iter()
                    .any(|prior| prior.metric == candidate.metric)
            {
                push(
                    &mut regressions,
                    format!("gate.{}.level", candidate.metric),
                    QualityMetricPreference::Lower,
                    f64::from(quality_level_rank(QualityLevel::Pass)),
                    f64::from(quality_level_rank(candidate.level)),
                );
            }
        }

        let fail_gates = |report: &Self| {
            report
                .gates
                .iter()
                .filter(|gate| gate.level == QualityLevel::Fail)
                .count()
        };
        let fail_issues = |report: &Self| {
            report
                .topology_issues
                .iter()
                .filter(|issue| issue.severity == topology::Severity::Fail)
                .count()
        };
        let warn_issues = |report: &Self| {
            report
                .topology_issues
                .iter()
                .filter(|issue| issue.severity == topology::Severity::Warn)
                .count()
        };
        let discrete = [
            ("fail_gate_count", fail_gates(baseline), fail_gates(self)),
            (
                "fail_topology_issue_count",
                fail_issues(baseline),
                fail_issues(self),
            ),
            (
                "warn_topology_issue_count",
                warn_issues(baseline),
                warn_issues(self),
            ),
            (
                "zero_area_cell_count",
                baseline.geometry.zero_area_cell_count,
                self.geometry.zero_area_cell_count,
            ),
            (
                "negative_area_cell_count",
                baseline.geometry.negative_area_cell_count,
                self.geometry.negative_area_cell_count,
            ),
            (
                "non_finite_cell_count",
                baseline.geometry.non_finite_cell_count,
                self.geometry.non_finite_cell_count,
            ),
            (
                "self_intersection_count",
                baseline.geometry.self_intersection_count,
                self.geometry.self_intersection_count,
            ),
            (
                "invalid_polygon_count",
                baseline.geometry.invalid_polygon_count,
                self.geometry.invalid_polygon_count,
            ),
            (
                "transition_continuity_warning_count",
                baseline.topology.transition_continuity_warning_count,
                self.topology.transition_continuity_warning_count,
            ),
            (
                "isolated_refined_cell_count",
                baseline.topology.isolated_refined_cell_count,
                self.topology.isolated_refined_cell_count,
            ),
        ];
        for (metric, prior, candidate) in discrete {
            if candidate > prior {
                push(
                    &mut regressions,
                    metric,
                    QualityMetricPreference::Lower,
                    prior as f64,
                    candidate as f64,
                );
            }
        }

        let lower_is_better = [
            (
                "aspect_ratio.max",
                baseline.geometry.aspect_ratio.max,
                self.geometry.aspect_ratio.max,
            ),
            (
                "cell_edge_length_cv.max",
                baseline.geometry.cell_edge_length_cv.max,
                self.geometry.cell_edge_length_cv.max,
            ),
            (
                "angle_deviation_deg.max",
                baseline.geometry.angle_deviation_deg.max,
                self.geometry.angle_deviation_deg.max,
            ),
        ];
        for (metric, prior, candidate) in lower_is_better {
            if geometry_metric_change(candidate, prior) > 0 {
                push(
                    &mut regressions,
                    metric,
                    QualityMetricPreference::Lower,
                    prior,
                    candidate,
                );
            }
        }
        // An exact ratio, not a continuous extremum: any increase is a real
        // coarsening of the transition, so it keeps the strict comparison.
        if metric_change(
            self.topology.max_adjacent_resolution_ratio,
            baseline.topology.max_adjacent_resolution_ratio,
        ) > 0
        {
            push(
                &mut regressions,
                "max_adjacent_resolution_ratio",
                QualityMetricPreference::Lower,
                baseline.topology.max_adjacent_resolution_ratio,
                self.topology.max_adjacent_resolution_ratio,
            );
        }
        if geometry_metric_change(self.geometry.min_angle_deg, baseline.geometry.min_angle_deg) < 0
        {
            push(
                &mut regressions,
                "min_angle_deg",
                QualityMetricPreference::Higher,
                baseline.geometry.min_angle_deg,
                self.geometry.min_angle_deg,
            );
        }
        regressions
    }

    /// Whether this candidate is a strict, non-regressing improvement over a
    /// previously accepted mesh.
    ///
    /// Auto-refinement is allowed to trade cell count and within-tier global
    /// area variability for local shape quality, but it may not worsen any
    /// individual gate tier, topology defect, local shape metric, or transition
    /// metric. Equal reports are not improvements.
    pub fn is_strict_improvement_over(&self, baseline: &Self) -> bool {
        if !self.guarded_metric_regressions(baseline).is_empty() {
            return false;
        }

        let self_fail_gates = self
            .gates
            .iter()
            .filter(|gate| gate.level == QualityLevel::Fail)
            .count();
        let baseline_fail_gates = baseline
            .gates
            .iter()
            .filter(|gate| gate.level == QualityLevel::Fail)
            .count();
        let self_warn_gates = self
            .gates
            .iter()
            .filter(|gate| gate.level == QualityLevel::Warn)
            .count();
        let baseline_warn_gates = baseline
            .gates
            .iter()
            .filter(|gate| gate.level == QualityLevel::Warn)
            .count();
        let self_fail_issues = self
            .topology_issues
            .iter()
            .filter(|issue| issue.severity == topology::Severity::Fail)
            .count();
        let baseline_fail_issues = baseline
            .topology_issues
            .iter()
            .filter(|issue| issue.severity == topology::Severity::Fail)
            .count();
        let self_warn_issues = self
            .topology_issues
            .iter()
            .filter(|issue| issue.severity == topology::Severity::Warn)
            .count();
        let baseline_warn_issues = baseline
            .topology_issues
            .iter()
            .filter(|issue| issue.severity == topology::Severity::Warn)
            .count();

        let discrete = [
            (self_fail_gates, baseline_fail_gates),
            (self_warn_gates, baseline_warn_gates),
            (self_fail_issues, baseline_fail_issues),
            (self_warn_issues, baseline_warn_issues),
            (
                self.geometry.zero_area_cell_count,
                baseline.geometry.zero_area_cell_count,
            ),
            (
                self.geometry.negative_area_cell_count,
                baseline.geometry.negative_area_cell_count,
            ),
            (
                self.geometry.non_finite_cell_count,
                baseline.geometry.non_finite_cell_count,
            ),
            (
                self.geometry.self_intersection_count,
                baseline.geometry.self_intersection_count,
            ),
            (
                self.geometry.invalid_polygon_count,
                baseline.geometry.invalid_polygon_count,
            ),
            (
                self.topology.transition_continuity_warning_count,
                baseline.topology.transition_continuity_warning_count,
            ),
            (
                self.topology.isolated_refined_cell_count,
                baseline.topology.isolated_refined_cell_count,
            ),
        ];
        let lower_is_better = [
            (
                self.geometry.aspect_ratio.max,
                baseline.geometry.aspect_ratio.max,
            ),
            (
                self.geometry.cell_edge_length_cv.max,
                baseline.geometry.cell_edge_length_cv.max,
            ),
            (
                self.geometry.angle_deviation_deg.max,
                baseline.geometry.angle_deviation_deg.max,
            ),
        ];
        // Mirrors `guarded_metric_regressions`: the same tolerance decides both
        // "is this a regression" and "is this an improvement", so a candidate
        // can never fall into a dead zone that counts as neither.
        baseline.verdict.is_worse_than(self.verdict)
            || discrete.iter().any(|(candidate, prior)| candidate < prior)
            || lower_is_better
                .iter()
                .any(|&(candidate, prior)| geometry_metric_change(candidate, prior) < 0)
            || metric_change(
                self.topology.max_adjacent_resolution_ratio,
                baseline.topology.max_adjacent_resolution_ratio,
            ) < 0
            || geometry_metric_change(self.geometry.min_angle_deg, baseline.geometry.min_angle_deg)
                > 0
    }
}

/// Float-noise guard: anything above this is a real difference in value, though
/// not necessarily a meaningful one. Used for metrics whose values are exact
/// (counts, resolution ratios).
const METRIC_NOISE_TOLERANCE: f64 = 1.0e-9;

/// Meaningfulness threshold for continuous geometry metrics.
///
/// `aspect_ratio.max`, `angle_deviation_deg.max`, `cell_edge_length_cv.max` and
/// `min_angle_deg` are extrema over the whole mesh, so touching any single cell
/// perturbs them, in a direction that is effectively random. On a 10^5-cell mesh
/// a one-cell AutoRefine repair moves `aspect_ratio.max` by ~1e-6 — three orders
/// of magnitude above the noise guard, which made
/// [`MeshQualityReport::is_strict_improvement_over`] reject otherwise fine passes
/// as regressions. A mesh-quality-relevant degradation is percent-level, so 1e-4
/// relative separates the two without masking anything real.
const GEOMETRY_METRIC_TOLERANCE: f64 = 1.0e-4;

/// Compare finite metrics with a scale-aware relative tolerance. Non-finite
/// candidate values are worse than finite baselines; two non-finite values are
/// treated as unchanged.
fn metric_change_within(candidate: f64, baseline: f64, relative_tolerance: f64) -> i8 {
    match (candidate.is_finite(), baseline.is_finite()) {
        (false, false) => 0,
        (false, true) => 1,
        (true, false) => -1,
        (true, true) => {
            let tolerance = relative_tolerance * candidate.abs().max(baseline.abs()).max(1.0);
            if candidate > baseline + tolerance {
                1
            } else if candidate < baseline - tolerance {
                -1
            } else {
                0
            }
        }
    }
}

/// Exact-valued metrics: any difference beyond float noise counts.
fn metric_change(candidate: f64, baseline: f64) -> i8 {
    metric_change_within(candidate, baseline, METRIC_NOISE_TOLERANCE)
}

/// Continuous whole-mesh extrema: only mesh-quality-relevant shifts count.
fn geometry_metric_change(candidate: f64, baseline: f64) -> i8 {
    metric_change_within(candidate, baseline, GEOMETRY_METRIC_TOLERANCE)
}

fn cell_ring(input: &QualityMeshInput, cell: &QualityCell) -> Option<Vec<Point>> {
    let mut ring = Vec::with_capacity(cell.vertices.len());
    for &i in &cell.vertices {
        ring.push(*input.vertices.get(i)?);
    }
    Some(ring)
}

/// Copy of `ring` with longitudes unwrapped to within ±180° of the first
/// vertex, so dateline-crossing cells are measured as compact polygons rather
/// than world-spanning slivers (raw lon averaging/shoelace flips sign there).
fn unwrap_ring_lon(ring: &[Point]) -> Vec<Point> {
    let Some(first) = ring.first() else {
        return Vec::new();
    };
    let lon0 = first.x;
    ring.iter()
        .map(|p| {
            let mut lon = p.x;
            while lon - lon0 > 180.0 {
                lon -= 360.0;
            }
            while lon - lon0 < -180.0 {
                lon += 360.0;
            }
            Point::new(lon, p.y)
        })
        .collect()
}

fn centroid(ring: &[Point]) -> Point {
    if ring.is_empty() {
        return Point::new(0.0, 0.0);
    }
    // Average in unwrapped-longitude space so dateline-crossing cells do not
    // land on the wrong side of the globe, then wrap back to [-180, 180].
    let unwrapped = unwrap_ring_lon(ring);
    let n = unwrapped.len() as f64;
    let mut lon = unwrapped.iter().map(|p| p.x).sum::<f64>() / n;
    while lon > 180.0 {
        lon -= 360.0;
    }
    while lon < -180.0 {
        lon += 360.0;
    }
    Point::new(lon, unwrapped.iter().map(|p| p.y).sum::<f64>() / n)
}

/// (lon, lat) degrees -> unit sphere (x, y, z). Dateline/pole-safe corner geometry.
fn lonlat_to_unit(p: Point) -> [f64; 3] {
    let lon = p.x.to_radians();
    let lat = p.y.to_radians();
    [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
}

fn arc_length_unit_sphere(a: [f64; 3], b: [f64; 3]) -> f64 {
    let chord = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
    2.0 * (0.5 * chord).clamp(0.0, 1.0).asin()
}

fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn tangent_toward(origin: [f64; 3], target: [f64; 3]) -> Option<[f64; 3]> {
    let projection = dot3(origin, target);
    let tangent = [
        target[0] - projection * origin[0],
        target[1] - projection * origin[1],
        target[2] - projection * origin[2],
    ];
    let norm = dot3(tangent, tangent).sqrt();
    (norm > 64.0 * f64::EPSILON).then_some([
        tangent[0] / norm,
        tangent[1] / norm,
        tangent[2] / norm,
    ])
}

/// Exact geodesic corner angles from unit-vector tangent directions.
fn interior_angles_deg(ring: &[Point], winding: SphericalWinding) -> Vec<f64> {
    let n = ring.len();
    if n < 3 {
        return Vec::new();
    }
    let xyz: Vec<[f64; 3]> = ring.iter().map(|p| lonlat_to_unit(*p)).collect();
    let mut angles = Vec::with_capacity(n);
    let orientation = match winding {
        SphericalWinding::CounterClockwise => 1.0,
        SphericalWinding::Clockwise => -1.0,
        SphericalWinding::Indeterminate => return Vec::new(),
    };
    for i in 0..n {
        let previous = xyz[(i + n - 1) % n];
        let current = xyz[i];
        let next = xyz[(i + 1) % n];
        let (Some(to_previous), Some(to_next)) = (
            tangent_toward(current, previous),
            tangent_toward(current, next),
        ) else {
            return Vec::new();
        };
        let minor = dot3(to_previous, to_next)
            .clamp(-1.0, 1.0)
            .acos()
            .to_degrees();
        let signed_turn = dot3(current, cross3(to_previous, to_next));
        let reflex = signed_turn * orientation > 0.0;
        angles.push(if reflex { 360.0 - minor } else { minor });
    }
    angles
}

fn supports_local_shape_metrics(ring: &[Point]) -> bool {
    const MAX_LOCAL_EDGE_RAD: f64 = 15.0_f64.to_radians();
    ring.len() >= 3
        && (0..ring.len()).all(|i| {
            arc_length_unit_sphere(
                lonlat_to_unit(ring[i]),
                lonlat_to_unit(ring[(i + 1) % ring.len()]),
            ) <= MAX_LOCAL_EDGE_RAD
        })
}

/// Sorted, deduplicated edge key for a vertex pair.
fn edge_key(a: usize, b: usize) -> (usize, usize) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

fn triangle_quality(lens: &[f64]) -> Option<(f64, f64)> {
    if lens.len() != 3 || lens.iter().any(|v| *v <= 0.0) {
        return None;
    }
    let (a, b, c) = (lens[0], lens[1], lens[2]);
    let semiperimeter = 0.5 * (a + b + c);
    let area = (semiperimeter * (semiperimeter - a) * (semiperimeter - b) * (semiperimeter - c))
        .max(0.0)
        .sqrt();
    if area <= 0.0 {
        return None;
    }
    let eta = 4.0 * 3.0_f64.sqrt() * area / lens.iter().map(|v| v * v).sum::<f64>();
    let inradius = 2.0 * area / (a + b + c);
    let circumradius = 0.25 * a * b * c / area;
    (circumradius > 0.0).then_some((eta, 2.0 * inradius / circumradius))
}

#[derive(Default)]
struct RefineLevelAccumulator {
    cell_count: usize,
    areas: Vec<f64>,
    edge_cvs: Vec<f64>,
    angle_deviations: Vec<f64>,
    triangle_etas: Vec<f64>,
    triangle_nsrs: Vec<f64>,
}

impl RefineLevelAccumulator {
    fn finish(self, refine_level: Option<u32>) -> RefineLevelQualitySummary {
        RefineLevelQualitySummary {
            refine_level,
            cell_count: self.cell_count,
            cell_area: Stat5::from_slice(&self.areas),
            cell_edge_length_cv: Stat5::from_slice(&self.edge_cvs),
            angle_deviation_deg: Stat5::from_slice_or_nan(&self.angle_deviations),
            triangle_eta: Stat5::from_slice_or_nan(&self.triangle_etas),
            triangle_nsr: Stat5::from_slice_or_nan(&self.triangle_nsrs),
        }
    }
}

/// Compute a quality report without assuming a target Euler characteristic.
pub fn compute(input: &QualityMeshInput, thresholds: &QualityThresholds) -> MeshQualityReport {
    compute_with_options(input, thresholds, QualityComputationOptions::default())
}

/// Compute the full quality report with explicit domain topology context.
pub fn compute_with_options(
    input: &QualityMeshInput,
    thresholds: &QualityThresholds,
    options: QualityComputationOptions,
) -> MeshQualityReport {
    let mut geom = GeometryMetrics {
        cell_count: input.cells.len(),
        vertex_count: input.vertices.len(),
        ..Default::default()
    };
    let mut topo = TopologyMetrics::default();

    let mut areas = Vec::new();
    let mut edge_lengths = Vec::new();
    let mut cell_edge_cvs = Vec::new();
    let mut angle_deviations = Vec::new();
    let mut triangle_etas = Vec::new();
    let mut triangle_nsrs = Vec::new();
    let mut aspects = Vec::new();
    let mut compactnesses = Vec::new();
    let mut cell_scales = vec![None; input.cells.len()];
    let mut min_angle = f64::INFINITY;
    let mut max_angle = f64::NEG_INFINITY;

    // edge -> count of incident cells; track degenerate / invalid edges.
    use std::collections::BTreeMap;
    type EdgeKey = (usize, usize);
    type DirectedEdgeUse = (usize, usize, usize);
    let mut edge_cells: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
    let mut edge_orientations: BTreeMap<EdgeKey, Vec<DirectedEdgeUse>> = BTreeMap::new();
    let mut refine_groups: BTreeMap<Option<u32>, RefineLevelAccumulator> = BTreeMap::new();
    let nv = input.vertices.len();

    for (ci, cell) in input.cells.iter().enumerate() {
        refine_groups
            .entry(cell.refine_level)
            .or_default()
            .cell_count += 1;
        let unique_idx: Vec<usize> = {
            let mut seen = Vec::new();
            for &i in &cell.vertices {
                if !seen.contains(&i) {
                    seen.push(i);
                }
            }
            seen
        };
        if unique_idx.len() < 3 {
            topo.abnormal_polygon_edge_count += 1;
        } else {
            match unique_idx.len() {
                3 => topo.triangle_cell_count += 1,
                4 => topo.quadrilateral_cell_count += 1,
                5 => topo.pentagon_cell_count += 1,
                6 => topo.hexagon_cell_count += 1,
                7 => topo.heptagon_cell_count += 1,
                _ => topo.other_polygon_cell_count += 1,
            }
        }
        for &i in &cell.vertices {
            if i >= nv {
                topo.invalid_vertex_index_count += 1;
            }
        }
        // edges
        let m = cell.vertices.len();
        for k in 0..m {
            let a = cell.vertices[k];
            let b = cell.vertices[(k + 1) % m];
            if a >= nv || b >= nv {
                topo.dangling_edge_count += 1;
                continue;
            }
            if a == b {
                topo.dangling_edge_count += 1;
                continue;
            }
            let key = edge_key(a, b);
            edge_cells.entry(key).or_default().push(ci);
            edge_orientations.entry(key).or_default().push((ci, a, b));
            // great-circle length; skip non-finite endpoints so one bad vertex
            // cannot poison the edge-length mean/std with NaN.
            let (pa, pb) = (input.vertices[a], input.vertices[b]);
            if pa.x.is_finite() && pa.y.is_finite() && pb.x.is_finite() && pb.y.is_finite() {
                edge_lengths.push(haversine_km(pa, pb));
            }
        }

        // neighbor index validity (kept before the geometry block: that block
        // may `continue` on non-finite cells and must not skip this check)
        for &n in &cell.neighbors {
            if n >= input.cells.len() || n == ci {
                topo.invalid_cell_index_count += 1;
            }
        }

        // per-cell geometry (only when ring is resolvable)
        if let Some(ring) = cell_ring(input, cell) {
            let spherical = match try_spherical_polygon_area(&ring) {
                Ok(area) => area,
                Err(SphericalPolygonError::NonFiniteCoordinate { .. }) => {
                    geom.non_finite_cell_count += 1;
                    continue;
                }
                Err(SphericalPolygonError::SelfIntersection { .. }) => {
                    geom.self_intersection_count += 1;
                    geom.invalid_polygon_count += 1;
                    continue;
                }
                Err(SphericalPolygonError::DegenerateArea) => {
                    geom.zero_area_cell_count += 1;
                    continue;
                }
                Err(_) => {
                    geom.invalid_polygon_count += 1;
                    continue;
                }
            };
            if spherical.winding == SphericalWinding::Clockwise {
                geom.negative_area_cell_count += 1;
            }
            let area = spherical.minor_sr * EARTH_RADIUS_KM * EARTH_RADIUS_KM;
            if !area.is_finite() || area <= 1.0e-12 {
                geom.zero_area_cell_count += 1;
            } else {
                cell_scales[ci] = Some(area.sqrt());
                areas.push(area);
                let group = refine_groups.entry(cell.refine_level).or_default();
                group.areas.push(area);
                // aspect ratio = longest/shortest edge in great-circle km (the ratio is
                // unit-free and, unlike planar lon/lat lengths, dateline/pole-safe).
                let mut km_lens = Vec::new();
                for k in 0..ring.len() {
                    let p = ring[k];
                    let q = ring[(k + 1) % ring.len()];
                    km_lens.push(haversine_km(p, q));
                }
                let emin = km_lens.iter().cloned().fold(f64::INFINITY, f64::min);
                let emax = km_lens.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                if emin > 0.0 {
                    aspects.push(emax / emin);
                }
                let edge_cv = Stat5::from_slice(&km_lens).cv;
                cell_edge_cvs.push(edge_cv);
                group.edge_cvs.push(edge_cv);
                let area_sr = area / (EARTH_RADIUS_KM * EARTH_RADIUS_KM);
                let perimeter_rad = km_lens.iter().sum::<f64>() / EARTH_RADIUS_KM;
                if perimeter_rad > 0.0 {
                    compactnesses.push(
                        (area_sr * (4.0 * std::f64::consts::PI - area_sr) / perimeter_rad.powi(2))
                            .clamp(0.0, 1.0),
                    );
                }

                let angles = interior_angles_deg(&ring, spherical.winding);
                if !angles.is_empty() {
                    let ideal = (((ring.len() as f64 - 2.0) * std::f64::consts::PI + area_sr)
                        / ring.len() as f64)
                        .to_degrees();
                    let angle_deviation = angles
                        .iter()
                        .map(|ang| (ang - ideal).abs())
                        .fold(0.0, f64::max);
                    angle_deviations.push(angle_deviation);
                    group.angle_deviations.push(angle_deviation);
                    for angle in angles {
                        min_angle = min_angle.min(angle);
                        max_angle = max_angle.max(angle);
                    }
                }

                if supports_local_shape_metrics(&ring) {
                    geom.local_shape_metric_sample_count += 1;
                    // Eta/NSR use Euclidean Heron/inradius/circumradius formulas.
                    // Keep those two compatibility metrics local and explicitly
                    // exclude coarse cells rather than claiming a unique spherical
                    // extension that the literature does not define.
                    if let Some((eta, nsr)) = triangle_quality(&km_lens) {
                        triangle_etas.push(eta);
                        triangle_nsrs.push(nsr);
                        group.triangle_etas.push(eta);
                        group.triangle_nsrs.push(nsr);
                    }
                } else {
                    geom.local_shape_metric_excluded_cell_count += 1;
                }
            }
        }
    }

    geom.edge_count = edge_cells.len();
    geom.cell_area = Stat5::from_slice(&areas);
    geom.cell_area_ratio = if geom.cell_area.min > 0.0 {
        geom.cell_area.max / geom.cell_area.min
    } else {
        0.0
    };
    geom.edge_length_km = Stat5::from_slice(&edge_lengths);
    geom.cell_edge_length_cv = Stat5::from_slice(&cell_edge_cvs);
    geom.angle_deviation_deg = Stat5::from_slice_or_nan(&angle_deviations);
    geom.triangle_eta = Stat5::from_slice_or_nan(&triangle_etas);
    geom.triangle_nsr = Stat5::from_slice_or_nan(&triangle_nsrs);
    geom.aspect_ratio = Stat5::from_slice(&aspects);
    geom.compactness = Stat5::from_slice_or_nan(&compactnesses);
    geom.min_angle_deg = if min_angle.is_finite() {
        min_angle
    } else {
        f64::NAN
    };
    geom.max_angle_deg = if max_angle.is_finite() {
        max_angle
    } else {
        f64::NAN
    };

    // non-manifold edges (shared by > 2 cells)
    topo.duplicate_edge_count = edge_cells.values().filter(|c| c.len() > 2).count();
    topo.boundary_edge_count = edge_cells.values().filter(|c| c.len() == 1).count();
    let boundary = topology::boundary_topology(input);
    topo.boundary_loop_count = boundary.loops.len();
    topo.boundary_vertex_degree_violation_count = boundary.invalid_vertex_degrees.len();
    topo.misoriented_shared_edge_count = edge_orientations
        .values()
        .filter(|occ| occ.len() == 2 && occ[0].1 == occ[1].1 && occ[0].2 == occ[1].2)
        .count();
    topo.euler_characteristic = topology::euler_characteristic(input);
    topo.expected_euler_characteristic = options.expected_euler_characteristic;
    topo.euler_characteristic_mismatch_count = usize::from(
        options
            .expected_euler_characteristic
            .is_some_and(|expected| expected != topo.euler_characteristic),
    );
    topo.connected_component_count = topology::connected_component_count(input);
    topo.non_manifold_vertex_fan_count = topology::non_manifold_vertex_fan_count(input);

    // orphan cells: share no edge with any other cell
    for (ci, cell) in input.cells.iter().enumerate() {
        let m = cell.vertices.len();
        let mut shares = false;
        for k in 0..m {
            let a = cell.vertices[k];
            let b = cell.vertices[(k + 1) % m];
            if a >= nv || b >= nv || a == b {
                continue;
            }
            if let Some(cells) = edge_cells.get(&edge_key(a, b)) {
                if cells.iter().any(|&other| other != ci) {
                    shares = true;
                    break;
                }
            }
        }
        if !shares && m >= 3 {
            topo.orphan_cell_count += 1;
        }
    }

    // Closed-cell adjacency sanity: if every valid edge has exactly one opposite
    // cell, the declared neighbor set should match the edge-derived one. Boundary
    // cells are intentionally skipped because regional/filter meshes are valid.
    for (ci, cell) in input.cells.iter().enumerate() {
        let m = cell.vertices.len();
        if m < 3 {
            continue;
        }
        let mut derived_neighbors = Vec::new();
        let mut closed = true;
        for k in 0..m {
            let a = cell.vertices[k];
            let b = cell.vertices[(k + 1) % m];
            if a >= nv || b >= nv || a == b {
                closed = false;
                break;
            }
            let Some(cells) = edge_cells.get(&edge_key(a, b)) else {
                closed = false;
                break;
            };
            if cells.len() != 2 {
                closed = false;
                break;
            }
            if let Some(&other) = cells.iter().find(|&&other| other != ci) {
                if !derived_neighbors.contains(&other) {
                    derived_neighbors.push(other);
                }
            } else {
                closed = false;
                break;
            }
        }
        if !closed {
            continue;
        }
        derived_neighbors.sort_unstable();
        let mut declared_neighbors: Vec<usize> = cell
            .neighbors
            .iter()
            .copied()
            .filter(|&nb| nb < input.cells.len() && nb != ci)
            .collect();
        declared_neighbors.sort_unstable();
        declared_neighbors.dedup();
        if declared_neighbors != derived_neighbors {
            topo.neighbor_degree_mismatch_count += 1;
        }
    }

    // Neighbor reciprocity plus the physical resolution jump across each
    // shared edge. Refinement generations are provenance, not a length scale:
    // arbitrary Delaunay insertion can advance generations without halving a
    // cell, so 2^level_diff overstates HARP-DV transitions.
    for (ci, cell) in input.cells.iter().enumerate() {
        for &nb in &cell.neighbors {
            if nb >= input.cells.len() {
                continue;
            }
            if !input.cells[nb].neighbors.contains(&ci) {
                topo.neighbor_reciprocity_failure_count += 1;
            }
            if nb > ci {
                let (Some(here), Some(there)) = (cell_scales[ci], cell_scales[nb]) else {
                    continue;
                };
                let ratio = here.max(there) / here.min(there);
                topo.max_adjacent_resolution_ratio = topo.max_adjacent_resolution_ratio.max(ratio);
                if ratio > thresholds.max_adjacent_resolution_ratio_warn {
                    topo.transition_continuity_warning_count += 1;
                }
            }
        }
        // isolated refined: refined cell whose every neighbor is strictly coarser
        if let Some(la) = cell.refine_level {
            if la > 0
                && !cell.neighbors.is_empty()
                && cell.neighbors.iter().all(|&nb| {
                    input
                        .cells
                        .get(nb)
                        .and_then(|c| c.refine_level)
                        .map(|lb| lb < la)
                        .unwrap_or(false)
                })
            {
                topo.isolated_refined_cell_count += 1;
            }
        }
    }

    let (gates, worst_cells, repair_cells, gate_verdict) =
        evaluate(input, &geom, &topo, thresholds);
    let refine_level_groups = refine_groups
        .into_iter()
        .map(|(level, acc)| acc.finish(level))
        .collect();

    // Run the detailed topology validator and fold its worst severity into the
    // verdict (catastrophic connectivity = Fail; transition degradation = Warn).
    let topology_issues = topology::MeshTopologyValidator::new(input).validate_all();
    let validator_level = match topology::worst_severity(&topology_issues) {
        Some(topology::Severity::Fail) => QualityLevel::Fail,
        Some(topology::Severity::Warn) => QualityLevel::Warn,
        None => QualityLevel::Pass,
    };
    let verdict = gate_verdict.worse(validator_level);

    MeshQualityReport {
        mesh_name: String::new(),
        cell_view: String::new(),
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        geometry: geom,
        topology: topo,
        refine_level_groups,
        hfield: None,
        adaptive: None,
        gates,
        worst_cells,
        repair_cells,
        topology_issues,
        verdict,
    }
}

fn level_counts_from_map(map: std::collections::BTreeMap<u32, usize>) -> Vec<LevelCount> {
    map.into_iter()
        .map(|(level, count)| LevelCount { level, count })
        .collect()
}

/// Compute h-field diagnostics from per-cell target levels plus actual
/// refinement levels carried by [`QualityCell::refine_level`].
///
/// `target_levels[i]` is interpreted as the h-field target for `input.cells[i]`.
/// Extra targets are reported and ignored; missing target/actual values are
/// counted separately and included in the mismatch total when only one side is
/// present.
pub fn compute_hfield_diagnostics(
    input: &QualityMeshInput,
    target_levels: &[u32],
    config: HfieldConfigDiagnostics,
) -> HfieldDiagnostics {
    use std::collections::{BTreeMap, BTreeSet};

    let cell_count = input.cells.len();
    let mut target_hist = BTreeMap::<u32, usize>::new();
    let mut actual_hist = BTreeMap::<u32, usize>::new();

    for &level in target_levels.iter().take(cell_count) {
        *target_hist.entry(level).or_default() += 1;
    }
    for cell in &input.cells {
        if let Some(level) = cell.refine_level {
            *actual_hist.entry(level).or_default() += 1;
        }
    }

    let missing_target_level_count = cell_count.saturating_sub(target_levels.len());
    let extra_target_level_count = target_levels.len().saturating_sub(cell_count);
    let missing_actual_refine_level_count = input
        .cells
        .iter()
        .filter(|cell| cell.refine_level.is_none())
        .count();

    let mut target_actual_mismatch_count = 0usize;
    let mut target_above_actual_count = 0usize;
    let mut actual_above_target_count = 0usize;
    let mut max_target_actual_delta = 0u32;

    for (ci, cell) in input.cells.iter().enumerate() {
        match (target_levels.get(ci).copied(), cell.refine_level) {
            (Some(target), Some(actual)) => {
                if target != actual {
                    target_actual_mismatch_count += 1;
                    if target > actual {
                        target_above_actual_count += 1;
                    } else {
                        actual_above_target_count += 1;
                    }
                    max_target_actual_delta = max_target_actual_delta.max(target.abs_diff(actual));
                }
            }
            (Some(_), None) | (None, Some(_)) => {
                target_actual_mismatch_count += 1;
            }
            (None, None) => {}
        }
    }

    let mut pairs = BTreeSet::<(usize, usize)>::new();
    for (ci, cell) in input.cells.iter().enumerate() {
        for &nb in &cell.neighbors {
            if nb < cell_count && nb != ci {
                pairs.insert((ci.min(nb), ci.max(nb)));
            }
        }
    }

    let mut max_adjacent_target_level_jump = 0u32;
    let mut target_level_jump_gt_one_count = 0usize;
    let mut max_adjacent_actual_level_jump = 0u32;
    let mut actual_level_jump_gt_one_count = 0usize;
    for (a, b) in pairs {
        if let (Some(la), Some(lb)) = (target_levels.get(a), target_levels.get(b)) {
            let diff = la.abs_diff(*lb);
            max_adjacent_target_level_jump = max_adjacent_target_level_jump.max(diff);
            if diff > 1 {
                target_level_jump_gt_one_count += 1;
            }
        }
        if let (Some(la), Some(lb)) = (input.cells[a].refine_level, input.cells[b].refine_level) {
            let diff = la.abs_diff(lb);
            max_adjacent_actual_level_jump = max_adjacent_actual_level_jump.max(diff);
            if diff > 1 {
                actual_level_jump_gt_one_count += 1;
            }
        }
    }

    HfieldDiagnostics {
        config,
        cell_count,
        target_level_distribution: level_counts_from_map(target_hist),
        actual_refine_level_distribution: level_counts_from_map(actual_hist),
        missing_target_level_count,
        extra_target_level_count,
        missing_actual_refine_level_count,
        target_actual_mismatch_count,
        target_above_actual_count,
        actual_above_target_count,
        max_target_actual_delta,
        max_adjacent_target_level_jump,
        target_level_jump_gt_one_count,
        max_adjacent_actual_level_jump,
        actual_level_jump_gt_one_count,
    }
}

/// Attach h-field diagnostics and warn only for unmet demand, missing mappings,
/// or discontinuous levels. Conforming refinement may legitimately place a
/// cell one level above its sampled target; that safe over-refinement remains
/// observable in `actual_above_target_count` but is not a quality violation.
/// Reconcile what the point+radius route asked for against what it delivered.
///
/// Computed through the h-field routine because the question is the same one;
/// only the configuration and the per-pass counts differ. Keeping one
/// implementation is what makes the two backends answerable in the same terms.
pub fn compute_adaptive_diagnostics(
    input: &QualityMeshInput,
    target_levels: &[u32],
    config: AdaptiveConfigDiagnostics,
) -> AdaptiveDiagnostics {
    let shared = compute_hfield_diagnostics(
        input,
        target_levels,
        HfieldConfigDiagnostics {
            enabled: config.enabled,
            g: None,
            max_level: config.max_level,
            base_m: config.base_m,
        },
    );
    AdaptiveDiagnostics {
        enabled: config.enabled,
        max_level: config.max_level,
        base_m: config.base_m,
        coastline: config.coastline,
        pass_count: config.pass_count,
        circle_count: config.circle_count,
        cell_count: shared.cell_count,
        target_level_distribution: shared.target_level_distribution,
        actual_refine_level_distribution: shared.actual_refine_level_distribution,
        missing_target_level_count: shared.missing_target_level_count,
        extra_target_level_count: shared.extra_target_level_count,
        missing_actual_refine_level_count: shared.missing_actual_refine_level_count,
        target_actual_mismatch_count: shared.target_actual_mismatch_count,
        target_above_actual_count: shared.target_above_actual_count,
        actual_above_target_count: shared.actual_above_target_count,
        max_target_actual_delta: shared.max_target_actual_delta,
        max_adjacent_target_level_jump: shared.max_adjacent_target_level_jump,
        target_level_jump_gt_one_count: shared.target_level_jump_gt_one_count,
        max_adjacent_actual_level_jump: shared.max_adjacent_actual_level_jump,
        actual_level_jump_gt_one_count: shared.actual_level_jump_gt_one_count,
    }
}

/// Attach the point+radius diagnostics and the gate that reads them.
///
/// Only one thing is gated, and the reason is measured. A circle has a hard
/// edge, so cells whose centre sits just inside one and which Method-C cannot
/// legally refine are a fixed feature of the geometry, not a defect: on a real
/// two-level coastal run, 97 of 1017 cells asked for the deeper level and came
/// back one short, and `max_target_actual_delta` was 1 — every shortfall was
/// exactly one level, which is what a boundary produces. Warning on any
/// shortfall would warn on every run, and a gate that always fires is a gate
/// nobody reads.
///
/// Falling short by *more* than one level cannot be explained that way, so that
/// is what fires. The counts are reported either way, for anyone comparing runs
/// or comparing this route against the h-field.
pub fn attach_adaptive_diagnostics(
    report: &mut MeshQualityReport,
    input: &QualityMeshInput,
    target_levels: &[u32],
    config: AdaptiveConfigDiagnostics,
) {
    let diagnostics = compute_adaptive_diagnostics(input, target_levels, config);
    let mut add_gate = |metric: &str, value: usize, detail: &str| {
        let level = if value > 0 {
            QualityLevel::Warn
        } else {
            QualityLevel::Pass
        };
        report.gates.push(GateResult {
            metric: metric.to_string(),
            value: value as f64,
            level,
            detail: detail.to_string(),
        });
        report.verdict = report.verdict.worse(level);
    };
    add_gate(
        "adaptive_target_short_by_more_than_one_level",
        usize::from(diagnostics.max_target_actual_delta > 1),
        "a circle asked for a level the mesh missed by more than one, which a \
         hard circle edge cannot explain",
    );
    add_gate(
        "adaptive_missing_level_count",
        diagnostics.missing_target_level_count
            + diagnostics.extra_target_level_count
            + diagnostics.missing_actual_refine_level_count,
        "point+radius target/actual level mapping is incomplete",
    );
    add_gate(
        "adaptive_actual_level_jump_gt_one_count",
        diagnostics.actual_level_jump_gt_one_count,
        "adjacent actual refinement level jump > 1",
    );
    report.adaptive = Some(diagnostics);
}

pub fn attach_hfield_diagnostics(
    report: &mut MeshQualityReport,
    input: &QualityMeshInput,
    target_levels: &[u32],
    config: HfieldConfigDiagnostics,
) {
    let diagnostics = compute_hfield_diagnostics(input, target_levels, config);
    let mut add_gate = |metric: &str, value: usize, detail: &str| {
        let level = if value > 0 {
            QualityLevel::Warn
        } else {
            QualityLevel::Pass
        };
        report.gates.push(GateResult {
            metric: metric.to_string(),
            value: value as f64,
            level,
            detail: detail.to_string(),
        });
        report.verdict = report.verdict.worse(level);
    };
    add_gate(
        "hfield_target_above_actual_count",
        diagnostics.target_above_actual_count,
        "h-field target level exceeds actual refinement level",
    );
    add_gate(
        "hfield_missing_level_count",
        diagnostics.missing_target_level_count
            + diagnostics.extra_target_level_count
            + diagnostics.missing_actual_refine_level_count,
        "h-field target/actual level mapping is incomplete",
    );
    add_gate(
        "hfield_target_level_jump_gt_one_count",
        diagnostics.target_level_jump_gt_one_count,
        "adjacent h-field target level jump > 1",
    );
    add_gate(
        "hfield_actual_level_jump_gt_one_count",
        diagnostics.actual_level_jump_gt_one_count,
        "adjacent actual refinement level jump > 1",
    );
    report.hfield = Some(diagnostics);
}

fn evaluate(
    input: &QualityMeshInput,
    geom: &GeometryMetrics,
    topo: &TopologyMetrics,
    th: &QualityThresholds,
) -> (
    Vec<GateResult>,
    Vec<WorstCell>,
    Vec<WorstCell>,
    QualityLevel,
) {
    let mut gates = Vec::new();
    let mut push = |metric: &str, value: f64, level: QualityLevel, detail: &str| {
        gates.push(GateResult {
            metric: metric.to_string(),
            value,
            level,
            detail: detail.to_string(),
        });
    };

    // Catastrophic topology -> Fail.
    for (name, count) in [
        (
            "invalid_vertex_index_count",
            topo.invalid_vertex_index_count,
        ),
        ("invalid_cell_index_count", topo.invalid_cell_index_count),
        ("duplicate_edge_count", topo.duplicate_edge_count),
        ("dangling_edge_count", topo.dangling_edge_count),
        (
            "misoriented_shared_edge_count",
            topo.misoriented_shared_edge_count,
        ),
        (
            "neighbor_degree_mismatch_count",
            topo.neighbor_degree_mismatch_count,
        ),
        ("orphan_cell_count", topo.orphan_cell_count),
        (
            "neighbor_reciprocity_failure_count",
            topo.neighbor_reciprocity_failure_count,
        ),
        (
            "abnormal_polygon_edge_count",
            topo.abnormal_polygon_edge_count,
        ),
        // A rim passes through each of its vertices once, so any other degree
        // is a boundary that branches or pinches -- malformed in the same way
        // the other entries here are, not a matter of degree.
        (
            "boundary_vertex_degree_violation_count",
            topo.boundary_vertex_degree_violation_count,
        ),
        ("self_intersection_count", geom.self_intersection_count),
        ("invalid_polygon_count", geom.invalid_polygon_count),
        ("zero_area_cell_count", geom.zero_area_cell_count),
        ("negative_area_cell_count", geom.negative_area_cell_count),
        ("non_finite_cell_count", geom.non_finite_cell_count),
    ] {
        push(
            name,
            count as f64,
            if count > 0 {
                QualityLevel::Fail
            } else {
                QualityLevel::Pass
            },
            if count > 0 {
                "catastrophic topology/geometry error"
            } else {
                ""
            },
        );
    }

    if let Some(expected) = topo.expected_euler_characteristic {
        let mismatch = topo.euler_characteristic != expected;
        push(
            "euler_characteristic",
            topo.euler_characteristic as f64,
            if mismatch {
                QualityLevel::Fail
            } else {
                QualityLevel::Pass
            },
            if mismatch {
                "measured Euler characteristic differs from the explicit domain expectation"
            } else {
                "matches the explicit domain expectation"
            },
        );
    }

    // Suspicious geometry degradation -> Warn (fail only when extreme).
    let min_angle_level = if !geom.min_angle_deg.is_finite() {
        QualityLevel::Pass
    } else if geom.min_angle_deg < th.min_angle_fail_deg {
        QualityLevel::Fail
    } else if geom.min_angle_deg < th.min_angle_warn_deg {
        QualityLevel::Warn
    } else {
        QualityLevel::Pass
    };
    push(
        "min_angle_deg",
        geom.min_angle_deg,
        min_angle_level,
        if !geom.min_angle_deg.is_finite() {
            "N/A: no valid spherical corner-angle sample"
        } else {
            "smallest interior angle"
        },
    );

    // Strict comparisons on both graded gates so a value landing exactly on a
    // threshold stays in the less severe tier, matching the min_angle gate.
    let aspect_level = if geom.aspect_ratio.max > th.aspect_ratio_fail {
        QualityLevel::Fail
    } else if geom.aspect_ratio.max > th.aspect_ratio_warn {
        QualityLevel::Warn
    } else {
        QualityLevel::Pass
    };
    push(
        "aspect_ratio_max",
        geom.aspect_ratio.max,
        aspect_level,
        "max cell aspect ratio",
    );

    let edge_cv_level = if geom.cell_edge_length_cv.max > th.cell_edge_cv_warn {
        QualityLevel::Warn
    } else {
        QualityLevel::Pass
    };
    push(
        "cell_edge_length_cv_max",
        geom.cell_edge_length_cv.max,
        edge_cv_level,
        "max per-cell edge-length coefficient of variation",
    );

    let angle_dev_level = if geom.angle_deviation_deg.max.is_finite()
        && geom.angle_deviation_deg.max > th.angle_deviation_warn_deg
    {
        QualityLevel::Warn
    } else {
        QualityLevel::Pass
    };
    push(
        "angle_deviation_deg_max",
        geom.angle_deviation_deg.max,
        angle_dev_level,
        "max deviation from regular n-gon angle",
    );

    let cv_level = if geom.cell_area.cv >= th.area_cv_warn {
        QualityLevel::Warn
    } else {
        QualityLevel::Pass
    };
    push(
        "cell_area_cv",
        geom.cell_area.cv,
        cv_level,
        "cell area coefficient of variation",
    );
    push(
        "cell_area_ratio",
        geom.cell_area_ratio,
        QualityLevel::Pass,
        "max/min positive cell area",
    );

    let res_level = if topo.max_adjacent_resolution_ratio > th.max_adjacent_resolution_ratio_warn {
        QualityLevel::Warn
    } else {
        QualityLevel::Pass
    };
    push(
        "max_adjacent_resolution_ratio",
        topo.max_adjacent_resolution_ratio,
        res_level,
        "abrupt refinement transition",
    );

    let trans_level = if topo.transition_continuity_warning_count > 0 {
        QualityLevel::Warn
    } else {
        QualityLevel::Pass
    };
    push(
        "transition_continuity_warning_count",
        topo.transition_continuity_warning_count as f64,
        trans_level,
        "adjacent level jump > 1",
    );

    let isolated_level = if topo.isolated_refined_cell_count > 0 {
        QualityLevel::Warn
    } else {
        QualityLevel::Pass
    };
    push(
        "isolated_refined_cell_count",
        topo.isolated_refined_cell_count as f64,
        isolated_level,
        "refined cell with only coarser neighbors",
    );

    let verdict = gates
        .iter()
        .fold(QualityLevel::Pass, |acc, g| acc.worse(g.level));
    let (worst_cells, repair_cells) = collect_worst_cells(input, th);
    (gates, worst_cells, repair_cells, verdict)
}

/// Worst cells: invalid rings first, then locally repairable shape defects.
fn collect_worst_cells(
    input: &QualityMeshInput,
    th: &QualityThresholds,
) -> (Vec<WorstCell>, Vec<WorstCell>) {
    let mut scored: Vec<WorstCell> = Vec::new();
    for (ci, cell) in input.cells.iter().enumerate() {
        let Some(ring) = cell_ring(input, cell) else {
            continue;
        };
        let (metric, value, level) = match try_spherical_polygon_area(&ring) {
            Err(SphericalPolygonError::NonFiniteCoordinate { .. }) => continue,
            Err(SphericalPolygonError::SelfIntersection { .. }) => {
                ("self_intersection".to_string(), 1.0, QualityLevel::Fail)
            }
            Err(SphericalPolygonError::DegenerateArea) => {
                ("zero_area".to_string(), 0.0, QualityLevel::Fail)
            }
            Err(_) => ("invalid_polygon".to_string(), 0.0, QualityLevel::Fail),
            Ok(area) if area.minor_sr * EARTH_RADIUS_KM * EARTH_RADIUS_KM <= 1.0e-12 => {
                ("zero_area".to_string(), 0.0, QualityLevel::Fail)
            }
            Ok(area) => match worst_local_shape_defect(&ring, area, th) {
                Some(defect) => defect,
                None => continue,
            },
        };
        scored.push(WorstCell {
            cell_index: ci,
            refine_level: cell.refine_level,
            centroid: centroid(&ring),
            ring,
            metric,
            value,
            level,
        });
    }
    // Fail before Warn; within a level, compare threshold-normalized severity
    // so values with different units remain attributable.
    scored.sort_by(|a, b| {
        quality_level_rank(b.level)
            .cmp(&quality_level_rank(a.level))
            .then_with(|| {
                repair_defect_badness(b, th)
                    .partial_cmp(&repair_defect_badness(a, th))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| a.cell_index.cmp(&b.cell_index))
    });
    let repair_cells =
        connected_repair_batch(input, &scored, th.repair_batch_limit, th.repair_level_cap);
    scored.truncate(th.worst_cells_limit);
    (scored, repair_cells)
}

fn worst_local_shape_defect(
    ring: &[Point],
    area: SphericalArea,
    th: &QualityThresholds,
) -> Option<(String, f64, QualityLevel)> {
    let angles = interior_angles_deg(ring, area.winding);
    let edge_lengths = (0..ring.len())
        .map(|index| haversine_km(ring[index], ring[(index + 1) % ring.len()]))
        .collect::<Vec<_>>();
    let edge_min = edge_lengths.iter().copied().fold(f64::INFINITY, f64::min);
    let edge_max = edge_lengths
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let mut defects = Vec::with_capacity(4);

    let min_angle = angles.iter().copied().fold(f64::INFINITY, f64::min);
    if min_angle.is_finite() && min_angle < th.min_angle_warn_deg {
        defects.push((
            "min_angle_deg".to_string(),
            min_angle,
            if min_angle < th.min_angle_fail_deg {
                QualityLevel::Fail
            } else {
                QualityLevel::Warn
            },
        ));
    }
    if edge_min.is_finite() && edge_min > 0.0 && edge_max.is_finite() {
        let aspect_ratio = edge_max / edge_min;
        if aspect_ratio > th.aspect_ratio_warn {
            defects.push((
                "aspect_ratio".to_string(),
                aspect_ratio,
                if aspect_ratio > th.aspect_ratio_fail {
                    QualityLevel::Fail
                } else {
                    QualityLevel::Warn
                },
            ));
        }
    }
    let edge_cv = Stat5::from_slice(&edge_lengths).cv;
    if edge_cv > th.cell_edge_cv_warn {
        defects.push((
            "cell_edge_length_cv".to_string(),
            edge_cv,
            QualityLevel::Warn,
        ));
    }
    if !angles.is_empty() {
        let ideal = (((ring.len() as f64 - 2.0) * std::f64::consts::PI + area.minor_sr)
            / ring.len() as f64)
            .to_degrees();
        let angle_deviation = angles
            .iter()
            .map(|angle| (angle - ideal).abs())
            .fold(0.0, f64::max);
        if angle_deviation > th.angle_deviation_warn_deg {
            defects.push((
                "angle_deviation_deg".to_string(),
                angle_deviation,
                QualityLevel::Warn,
            ));
        }
    }

    defects.into_iter().max_by(|left, right| {
        quality_level_rank(left.2)
            .cmp(&quality_level_rank(right.2))
            .then_with(|| {
                local_defect_badness(&left.0, left.1, th)
                    .partial_cmp(&local_defect_badness(&right.0, right.1, th))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    })
}

fn repair_defect_badness(cell: &WorstCell, th: &QualityThresholds) -> f64 {
    local_defect_badness(&cell.metric, cell.value, th)
}

fn local_defect_badness(metric: &str, value: f64, th: &QualityThresholds) -> f64 {
    let threshold = match metric {
        "min_angle_deg" => return th.min_angle_warn_deg / value.max(f64::EPSILON),
        "aspect_ratio" => th.aspect_ratio_warn,
        "cell_edge_length_cv" => th.cell_edge_cv_warn,
        "angle_deviation_deg" => th.angle_deviation_warn_deg,
        _ => return f64::INFINITY,
    };
    if threshold > 0.0 {
        value / threshold
    } else {
        value
    }
}

/// Select one connected defect component, seeded by the worst repairable cell.
/// Refining several unrelated locations in one Method-C pass made it impossible
/// to attribute regressions and could create interacting transition skirts.
fn connected_repair_batch(
    input: &QualityMeshInput,
    scored: &[WorstCell],
    limit: usize,
    level_cap: Option<u32>,
) -> Vec<WorstCell> {
    let Some(seed) = scored
        .iter()
        .find(|cell| repair_candidate_below_cap(cell, level_cap))
    else {
        return Vec::new();
    };
    if limit == 0 || seed.cell_index >= input.cells.len() {
        return Vec::new();
    }

    let mut repairable = vec![None; input.cells.len()];
    for cell in scored
        .iter()
        .filter(|cell| repair_candidate_below_cap(cell, level_cap))
    {
        if cell.cell_index < repairable.len() {
            repairable[cell.cell_index] = Some(cell);
        }
    }

    let mut selected = Vec::with_capacity(limit.min(scored.len()));
    let mut visited = vec![false; input.cells.len()];
    let mut pending = std::collections::VecDeque::from([seed.cell_index]);
    visited[seed.cell_index] = true;
    while let Some(cell_index) = pending.pop_front() {
        if let Some(cell) = repairable[cell_index] {
            selected.push(cell.clone());
            if selected.len() == limit {
                break;
            }
        }
        let mut neighbors = input.cells[cell_index].neighbors.clone();
        neighbors.sort_unstable();
        neighbors.dedup();
        for neighbor in neighbors {
            if neighbor < visited.len() && !visited[neighbor] && repairable[neighbor].is_some() {
                visited[neighbor] = true;
                pending.push_back(neighbor);
            }
        }
    }
    selected
}

fn repair_candidate_below_cap(cell: &WorstCell, level_cap: Option<u32>) -> bool {
    cell.is_refinement_repairable()
        && level_cap.is_none_or(|cap| cell.refine_level.unwrap_or(0) < cap)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square_cell(a: usize, b: usize, c: usize, d: usize) -> QualityCell {
        QualityCell {
            vertices: vec![a, b, c, d],
            refine_level: Some(0),
            neighbors: Vec::new(),
        }
    }

    /// 2 unit squares sharing an edge.
    fn two_square_mesh() -> QualityMeshInput {
        QualityMeshInput {
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
        }
    }

    #[test]
    fn tiny_valid_mesh_passes() {
        let r = compute(&two_square_mesh(), &QualityThresholds::default());
        assert_eq!(r.verdict, QualityLevel::Pass);
        assert!(!r.has_unrepairable_failure());
        assert_eq!(r.geometry.cell_count, 2);
        assert_eq!(r.geometry.vertex_count, 6);
        assert_eq!(r.geometry.edge_count, 7); // 4+4 minus 1 shared
        assert_eq!(r.topology.duplicate_edge_count, 0);
        assert_eq!(r.topology.orphan_cell_count, 0);
        assert_eq!(r.topology.neighbor_reciprocity_failure_count, 0);
        assert_eq!(r.topology.quadrilateral_cell_count, 2);
        // 3D chord corner angle of a 1°×1° equatorial square is ~90° (not exactly,
        // since the chord vectors live on the sphere) — sane, not a planar artifact.
        assert!((r.geometry.min_angle_deg - 90.0).abs() < 1.0);
    }

    #[test]
    fn small_positive_area_uses_one_zero_area_threshold() {
        let delta = 5.0e-5;
        let mesh = QualityMeshInput {
            vertices: vec![
                Point::new(0.0, 0.0),
                Point::new(delta, 0.0),
                Point::new(delta, delta),
                Point::new(0.0, delta),
            ],
            cells: vec![square_cell(0, 1, 2, 3)],
        };
        let report = compute(&mesh, &QualityThresholds::default());
        assert_eq!(report.geometry.zero_area_cell_count, 0);
        assert!(!report
            .worst_cells
            .iter()
            .any(|cell| cell.metric == "zero_area"));
    }

    #[test]
    fn invalid_vertex_index_is_fail() {
        let mut m = two_square_mesh();
        m.cells[0].vertices = vec![0, 1, 2, 99];
        let r = compute(&m, &QualityThresholds::default());
        assert!(r.topology.invalid_vertex_index_count >= 1);
        assert_eq!(r.verdict, QualityLevel::Fail);
        assert!(r.has_unrepairable_failure());
    }

    #[test]
    fn repair_plan_respects_the_project_level_cap() {
        let mut mesh = two_square_mesh();
        for cell in &mut mesh.cells {
            cell.refine_level = Some(2);
        }
        let thresholds = QualityThresholds {
            min_angle_warn_deg: 179.0,
            min_angle_fail_deg: 178.0,
            ..QualityThresholds::default()
        };
        let report = compute(&mesh, &thresholds);
        assert!(!report.repair_cells.is_empty());
        let plan = crate::io::to_quality_repair_plan_json_capped(&report, 2);
        assert!(plan.contains("\"target_level\": 2"));
        assert!(!plan.contains("\"target_level\": 3"));
    }

    #[test]
    fn repair_batch_stays_inside_one_connected_defect_component() {
        let mut vertices = Vec::new();
        for x in [0.0, 5.0, 20.0, 25.0] {
            vertices.extend([
                Point::new(x, 0.0),
                Point::new(x + 4.0, 0.0),
                Point::new(x + 0.1, 0.2),
            ]);
        }
        let cells = (0..4)
            .map(|cell_index| QualityCell {
                vertices: vec![cell_index * 3, cell_index * 3 + 1, cell_index * 3 + 2],
                refine_level: Some(0),
                neighbors: vec![cell_index ^ 1],
            })
            .collect();
        let report = compute(
            &QualityMeshInput { vertices, cells },
            &QualityThresholds {
                min_angle_warn_deg: 179.0,
                min_angle_fail_deg: 0.0,
                repair_batch_limit: 2,
                ..QualityThresholds::default()
            },
        );

        assert_eq!(report.repair_cells.len(), 2);
        let mut selected = report
            .repair_cells
            .iter()
            .map(|cell| cell.cell_index)
            .collect::<Vec<_>>();
        selected.sort_unstable();
        assert!(selected == [0, 1] || selected == [2, 3], "{selected:?}");
    }

    #[test]
    fn repair_batch_skips_capped_seed_and_selects_next_eligible_defect() {
        let report = compute(
            &QualityMeshInput {
                vertices: vec![
                    Point::new(0.0, 0.0),
                    Point::new(4.0, 0.0),
                    Point::new(0.01, 0.01),
                    Point::new(10.0, 0.0),
                    Point::new(14.0, 0.0),
                    Point::new(10.5, 0.5),
                ],
                cells: vec![
                    QualityCell {
                        vertices: vec![0, 1, 2],
                        refine_level: Some(2),
                        neighbors: vec![],
                    },
                    QualityCell {
                        vertices: vec![3, 4, 5],
                        refine_level: Some(1),
                        neighbors: vec![],
                    },
                ],
            },
            &QualityThresholds {
                min_angle_warn_deg: 179.0,
                min_angle_fail_deg: 0.0,
                repair_batch_limit: 1,
                repair_level_cap: Some(2),
                ..QualityThresholds::default()
            },
        );

        assert_eq!(report.repair_cells.len(), 1);
        assert_eq!(report.repair_cells[0].cell_index, 1);
        assert_eq!(report.repair_cells[0].refine_level, Some(1));
    }

    #[test]
    fn repair_plan_can_target_aspect_ratio_without_an_edge_cv_gate() {
        let report = compute(
            &QualityMeshInput {
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
            },
            &QualityThresholds {
                min_angle_warn_deg: 0.0,
                cell_edge_cv_warn: 10.0,
                angle_deviation_warn_deg: 180.0,
                ..QualityThresholds::default()
            },
        );

        assert_eq!(report.repair_cells.len(), 1);
        assert_eq!(report.repair_cells[0].metric, "aspect_ratio");
    }

    #[test]
    fn repair_plan_can_target_angle_deviation_on_an_equal_edge_rhombus() {
        let report = compute(
            &QualityMeshInput {
                vertices: vec![
                    Point::new(-2.0, 0.0),
                    Point::new(0.0, -0.7),
                    Point::new(2.0, 0.0),
                    Point::new(0.0, 0.7),
                ],
                cells: vec![QualityCell {
                    vertices: vec![0, 1, 2, 3],
                    refine_level: Some(0),
                    neighbors: vec![],
                }],
            },
            &QualityThresholds::default(),
        );

        assert_eq!(report.repair_cells.len(), 1);
        assert_eq!(report.repair_cells[0].metric, "angle_deviation_deg");
    }

    #[test]
    fn negligible_extremum_drift_is_not_a_regression() {
        // Reproduces a real AutoRefine rollback: a one-cell repair on a global
        // 100 km mesh improved the target metric but nudged aspect_ratio.max by
        // 3.2e-6 (2.040302169 -> 2.040305343), which the old 1e-9 guard scored
        // as a regression and rolled the whole pass back.
        let mut baseline = compute(&two_square_mesh(), &QualityThresholds::default());
        baseline.geometry.aspect_ratio.max = 2.040302169394414;
        baseline.geometry.angle_deviation_deg.max = 40.0;

        let mut candidate = baseline.clone();
        candidate.geometry.aspect_ratio.max += 3.1736e-6;
        candidate.geometry.angle_deviation_deg.max = 27.0;

        assert!(
            candidate.guarded_metric_regressions(&baseline).is_empty(),
            "drift far below mesh-quality relevance must not read as a regression"
        );
        assert!(candidate.is_strict_improvement_over(&baseline));
    }

    #[test]
    fn extremum_drift_past_the_tolerance_still_regresses() {
        let mut baseline = compute(&two_square_mesh(), &QualityThresholds::default());
        baseline.geometry.aspect_ratio.max = 2.0;
        baseline.geometry.angle_deviation_deg.max = 40.0;

        let mut candidate = baseline.clone();
        // 1e-3 relative is an order of magnitude past the 1e-4 threshold.
        candidate.geometry.aspect_ratio.max += 2.0e-3;
        candidate.geometry.angle_deviation_deg.max = 27.0;

        let regressions = candidate.guarded_metric_regressions(&baseline);
        assert_eq!(regressions.len(), 1);
        assert_eq!(regressions[0].metric, "aspect_ratio.max");
        assert!(!candidate.is_strict_improvement_over(&baseline));
    }

    #[test]
    fn adjacent_resolution_ratio_keeps_the_strict_comparison() {
        // An exact ratio, so it must not inherit the geometry tolerance: a 2 -> 3
        // transition coarsening is a real regression however small the delta.
        let mut baseline = compute(&two_square_mesh(), &QualityThresholds::default());
        baseline.topology.max_adjacent_resolution_ratio = 2.0;
        baseline.geometry.angle_deviation_deg.max = 40.0;

        let mut candidate = baseline.clone();
        candidate.topology.max_adjacent_resolution_ratio = 2.0 + 1.0e-6;
        candidate.geometry.angle_deviation_deg.max = 27.0;

        let regressions = candidate.guarded_metric_regressions(&baseline);
        assert_eq!(regressions.len(), 1);
        assert_eq!(regressions[0].metric, "max_adjacent_resolution_ratio");
        assert!(!candidate.is_strict_improvement_over(&baseline));
    }

    #[test]
    fn strict_improvement_rejects_equal_or_regressing_candidates() {
        let baseline = compute(&two_square_mesh(), &QualityThresholds::default());
        let mut improved = baseline.clone();
        improved.geometry.aspect_ratio.max *= 0.9;
        assert!(improved.is_strict_improvement_over(&baseline));
        assert!(!baseline.is_strict_improvement_over(&baseline));

        let mut regressed = baseline.clone();
        regressed.geometry.cell_edge_length_cv.max += 0.01;
        assert!(!regressed.is_strict_improvement_over(&baseline));
        let regressions = regressed.guarded_metric_regressions(&baseline);
        assert_eq!(regressions.len(), 1);
        assert_eq!(regressions[0].metric, "cell_edge_length_cv.max");
        assert_eq!(regressions[0].preference, QualityMetricPreference::Lower);
        assert!((regressions[0].delta() - 0.01).abs() < 1.0e-12);

        let mut within_tier_area_change = improved;
        within_tier_area_change.geometry.cell_area.cv += 0.01;
        assert!(within_tier_area_change.is_strict_improvement_over(&baseline));

        let mut warned_baseline = baseline.clone();
        warned_baseline.verdict = QualityLevel::Warn;
        warned_baseline
            .gates
            .iter_mut()
            .find(|gate| gate.metric == "aspect_ratio_max")
            .unwrap()
            .level = QualityLevel::Warn;
        let mut swapped_warning = warned_baseline.clone();
        swapped_warning.geometry.aspect_ratio.max *= 0.9;
        swapped_warning
            .gates
            .iter_mut()
            .find(|gate| gate.metric == "aspect_ratio_max")
            .unwrap()
            .level = QualityLevel::Pass;
        swapped_warning
            .gates
            .iter_mut()
            .find(|gate| gate.metric == "cell_area_cv")
            .unwrap()
            .level = QualityLevel::Warn;
        assert!(!swapped_warning.is_strict_improvement_over(&warned_baseline));
        let regressions = swapped_warning.guarded_metric_regressions(&warned_baseline);
        assert!(regressions
            .iter()
            .any(|item| item.metric == "gate.cell_area_cv.level"));

        let mut failed_baseline = baseline.clone();
        failed_baseline.verdict = QualityLevel::Fail;
        failed_baseline
            .gates
            .iter_mut()
            .find(|gate| gate.metric == "aspect_ratio_max")
            .unwrap()
            .level = QualityLevel::Fail;
        let mut warned_candidate = failed_baseline.clone();
        warned_candidate.verdict = QualityLevel::Warn;
        warned_candidate
            .gates
            .iter_mut()
            .find(|gate| gate.metric == "aspect_ratio_max")
            .unwrap()
            .level = QualityLevel::Warn;
        assert!(warned_candidate
            .guarded_metric_regressions(&failed_baseline)
            .is_empty());
        assert!(warned_candidate.is_strict_improvement_over(&failed_baseline));

        let mut added_warning = warned_candidate.clone();
        added_warning.gates.push(GateResult {
            metric: "candidate_only_warning".to_string(),
            value: 1.0,
            level: QualityLevel::Warn,
            detail: "new warning".to_string(),
        });
        assert!(!added_warning.is_strict_improvement_over(&failed_baseline));

        let mut topology_regressed = warned_candidate;
        topology_regressed.topology.isolated_refined_cell_count += 1;
        assert!(!topology_regressed.is_strict_improvement_over(&failed_baseline));
    }

    #[test]
    fn duplicate_edge_non_manifold_is_fail() {
        // three cells sharing the same edge (0,1)
        let m = QualityMeshInput {
            vertices: vec![
                Point::new(0.0, 0.0),
                Point::new(1.0, 0.0),
                Point::new(0.5, 1.0),
                Point::new(0.5, -1.0),
                Point::new(0.5, 2.0),
            ],
            cells: vec![
                square_cell(0, 1, 2, 2),
                QualityCell {
                    vertices: vec![0, 1, 3],
                    refine_level: Some(0),
                    neighbors: vec![],
                },
                QualityCell {
                    vertices: vec![0, 1, 4],
                    refine_level: Some(0),
                    neighbors: vec![],
                },
            ],
        };
        let r = compute(&m, &QualityThresholds::default());
        assert!(r.topology.duplicate_edge_count >= 1);
        assert_eq!(r.verdict, QualityLevel::Fail);
    }

    #[test]
    fn zero_area_cell_is_fail() {
        let m = QualityMeshInput {
            vertices: vec![
                Point::new(0.0, 0.0),
                Point::new(1.0, 0.0),
                Point::new(2.0, 0.0),
            ],
            cells: vec![QualityCell {
                vertices: vec![0, 1, 2],
                refine_level: Some(0),
                neighbors: vec![],
            }],
        };
        let r = compute(&m, &QualityThresholds::default());
        assert_eq!(r.geometry.zero_area_cell_count, 1);
        assert_eq!(r.verdict, QualityLevel::Fail);
    }

    #[test]
    fn bad_neighbor_reciprocity_is_fail() {
        let mut m = two_square_mesh();
        m.cells[1].neighbors = vec![]; // cell0 claims 1 as neighbor; 1 doesn't reciprocate
        let r = compute(&m, &QualityThresholds::default());
        assert!(r.topology.neighbor_reciprocity_failure_count >= 1);
        assert_eq!(r.verdict, QualityLevel::Fail);
    }

    #[test]
    fn clockwise_cell_counts_negative_area_and_fails() {
        let mut m = two_square_mesh();
        // Reverse cell 0's ring: CCW -> CW winding.
        m.cells[0].vertices = vec![3, 2, 1, 0];
        let r = compute(&m, &QualityThresholds::default());
        assert_eq!(r.geometry.negative_area_cell_count, 1);
        assert_eq!(r.verdict, QualityLevel::Fail);
    }

    #[test]
    fn ccw_dateline_cell_is_not_negative_area() {
        // CCW quad straddling the antimeridian; raw-longitude shoelace would
        // wrongly report it as clockwise/negative without lon unwrapping.
        let m = QualityMeshInput {
            vertices: vec![
                Point::new(179.0, 0.0),
                Point::new(-179.0, 0.0),
                Point::new(-179.0, 1.0),
                Point::new(179.0, 1.0),
            ],
            cells: vec![QualityCell {
                vertices: vec![0, 1, 2, 3],
                refine_level: Some(0),
                neighbors: vec![],
            }],
        };
        let r = compute(&m, &QualityThresholds::default());
        assert_eq!(r.geometry.negative_area_cell_count, 0);
        // Centroid of the worst-cells layer must sit near the dateline, not lon~0.
        let c = centroid(&[
            Point::new(179.0, 0.0),
            Point::new(-179.0, 0.0),
            Point::new(-179.0, 1.0),
            Point::new(179.0, 1.0),
        ]);
        assert!(c.x.abs() > 179.0, "centroid lon {} should hug ±180", c.x);
    }

    #[test]
    fn non_finite_vertex_is_counted_and_does_not_poison_stats() {
        let mut m = two_square_mesh();
        m.vertices[0] = Point::new(f64::NAN, 0.0);
        let r = compute(&m, &QualityThresholds::default());
        assert_eq!(r.geometry.non_finite_cell_count, 1);
        assert_eq!(r.verdict, QualityLevel::Fail);
        // Cell 1 is untouched; its stats must stay finite.
        assert!(r.geometry.cell_area.mean.is_finite());
        assert!(r.geometry.cell_area.std.is_finite());
        assert!(r.geometry.edge_length_km.mean.is_finite());
    }

    #[test]
    fn abrupt_transition_warns() {
        let mut m = two_square_mesh();
        m.vertices[4].x = 10.0;
        m.vertices[5].x = 10.0;
        let r = compute(&m, &QualityThresholds::default());
        assert!(r.topology.max_adjacent_resolution_ratio > 2.0);
        assert!(r.topology.transition_continuity_warning_count >= 1);
        assert_ne!(r.verdict, QualityLevel::Pass); // at least Warn
    }
}
