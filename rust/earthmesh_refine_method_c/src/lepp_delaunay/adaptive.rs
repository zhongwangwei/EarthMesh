use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use rayon::prelude::*;

use earthmesh_boundary::SegmentList;
use earthmesh_core::{
    DEFAULT_METHOD_C_LEPP_MAXIMUM_INSERTIONS_PER_CYCLE,
    DEFAULT_METHOD_C_LEPP_MAXIMUM_NEIGHBOR_SIZE_RATIO, DEFAULT_METHOD_C_LEPP_MAXIMUM_PATH_LENGTH,
    DEFAULT_METHOD_C_LEPP_MAXIMUM_VERTICES, DEFAULT_METHOD_C_LEPP_MAX_CYCLES,
    DEFAULT_METHOD_C_LEPP_MINIMUM_TRIANGLE_ANGLE_DEGREES,
    DEFAULT_METHOD_C_LEPP_STOP_AT_SOURCE_RESOLUTION, DEFAULT_METHOD_C_LEPP_TARGET_SIZE_TOLERANCE,
};
use earthmesh_mesh::{
    dot, lonlat_degrees_to_unit_xyz, magnitude, spherical_centroid_degrees, CartesianPoint,
    FaceId as StableFaceId, LonLatDegrees, MeshState, RefinementRegion,
};
use earthmesh_refine::RefinementCause;

use super::insertion::{
    insert_lepp_terminal_midpoint_constrained_with_postcondition,
    insert_lepp_terminal_midpoint_with_postcondition, LeppInsertionSplitReason,
};
use super::post_quality::{quality_snapshot, strictly_improves_quality_snapshot};
use super::{
    insert_lepp_terminal_midpoint_constrained, push_report_detail, spherical_edge_length, FaceId,
    LeppInsertionError, LeppInsertionGates, LeppInsertionReport, LeppPostQualityConfig,
    LeppSearchConfig, LEPP_REPORT_DETAIL_LIMIT,
};

#[derive(Clone, Debug, PartialEq)]
pub struct AdaptiveHybridConfig {
    pub max_cycles: usize,
    pub target_size_tolerance: f64,
    pub stop_at_source_resolution: bool,
    pub maximum_neighbor_size_ratio: f64,
    pub maximum_vertices: usize,
    pub maximum_insertions_per_cycle: usize,
    pub minimum_triangle_angle: f64,
    pub search: LeppSearchConfig,
    pub gates: LeppInsertionGates,
}

impl Default for AdaptiveHybridConfig {
    fn default() -> Self {
        Self {
            max_cycles: DEFAULT_METHOD_C_LEPP_MAX_CYCLES,
            target_size_tolerance: DEFAULT_METHOD_C_LEPP_TARGET_SIZE_TOLERANCE,
            stop_at_source_resolution: DEFAULT_METHOD_C_LEPP_STOP_AT_SOURCE_RESOLUTION,
            maximum_neighbor_size_ratio: DEFAULT_METHOD_C_LEPP_MAXIMUM_NEIGHBOR_SIZE_RATIO,
            maximum_vertices: DEFAULT_METHOD_C_LEPP_MAXIMUM_VERTICES,
            maximum_insertions_per_cycle: DEFAULT_METHOD_C_LEPP_MAXIMUM_INSERTIONS_PER_CYCLE,
            minimum_triangle_angle: DEFAULT_METHOD_C_LEPP_MINIMUM_TRIANGLE_ANGLE_DEGREES,
            search: LeppSearchConfig {
                maximum_path_length: DEFAULT_METHOD_C_LEPP_MAXIMUM_PATH_LENGTH,
                ..LeppSearchConfig::default()
            },
            gates: LeppInsertionGates::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AdaptiveHybridDemand {
    pub criterion_id: String,
    pub region: RefinementRegion,
    pub cause: RefinementCause,
    pub hard: bool,
    pub source_resolution_m: Option<f64>,
    pub target_edge_m: Option<f64>,
}

impl AdaptiveHybridDemand {
    pub fn user_region(criterion_id: impl Into<String>, region: RefinementRegion) -> Self {
        Self {
            criterion_id: criterion_id.into(),
            region,
            cause: RefinementCause::UserSpecified,
            hard: true,
            source_resolution_m: None,
            target_edge_m: None,
        }
    }

    pub fn physical_region(criterion_id: impl Into<String>, region: RefinementRegion) -> Self {
        let criterion_id = criterion_id.into();
        Self {
            cause: RefinementCause::PhysicalCriterion {
                criterion_id: criterion_id.clone(),
            },
            criterion_id,
            region,
            hard: true,
            source_resolution_m: None,
            target_edge_m: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AdaptiveHybridInsertionCounts {
    pub physical: usize,
    pub balance: usize,
    pub quality: usize,
    pub boundary: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AdaptiveHybridPathStats {
    pub attempted: usize,
    pub committed: usize,
    pub rejected: usize,
    pub total_path_faces: usize,
    pub max_path_faces: usize,
    pub mean_path_faces: f64,
    pub p95_path_faces: f64,
    /// First sampled path lengths only; aggregate fields above stay exact.
    pub path_lengths: Vec<usize>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AdaptiveHybridTargetSatisfaction {
    pub target_faces: usize,
    pub satisfied_faces: usize,
    pub unsatisfied_faces: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdaptiveHybridStopReason {
    Satisfied,
    MaxCycles,
    MaxVertices,
    NoCommittableInsertion,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdaptiveHybridUnresolvedReason {
    SourceResolution,
    Limit,
    Rejection,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AdaptiveHybridUnresolvedDemand {
    pub criterion_id: String,
    pub face: Option<StableFaceId>,
    pub hard: bool,
    pub reason: AdaptiveHybridUnresolvedReason,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AdaptiveHybridRejection {
    pub criterion_id: String,
    pub face: StableFaceId,
    pub hard: bool,
    pub error: LeppInsertionError,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AdaptiveHybridError {
    InvalidConfig { message: String },
    InvalidMesh { message: String },
}

impl std::fmt::Display for AdaptiveHybridError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidConfig { message } => {
                write!(formatter, "invalid adaptive hybrid config: {message}")
            }
            Self::InvalidMesh { message } => {
                write!(formatter, "invalid adaptive hybrid mesh: {message}")
            }
        }
    }
}

impl std::error::Error for AdaptiveHybridError {}

#[derive(Clone, Debug, PartialEq)]
pub struct AdaptiveHybridReport {
    pub cycles: usize,
    /// First sampled insertion reports only; counts and path stats stay exact.
    pub insertions: Vec<LeppInsertionReport>,
    pub insertion_counts: AdaptiveHybridInsertionCounts,
    pub path_stats: AdaptiveHybridPathStats,
    pub initial_vertices: usize,
    pub final_vertices: usize,
    pub initial_faces: usize,
    pub final_faces: usize,
    pub target_satisfaction: AdaptiveHybridTargetSatisfaction,
    /// Exact number of unresolved demand details encountered.
    pub unresolved_demand_count: usize,
    /// First sampled unresolved details only; `unresolved_demand_count` stays exact.
    pub unresolved_demands: Vec<AdaptiveHybridUnresolvedDemand>,
    /// First sampled rejection details only; `path_stats.rejected` stays exact.
    pub rejections: Vec<AdaptiveHybridRejection>,
    pub stop_reason: AdaptiveHybridStopReason,
}

impl AdaptiveHybridReport {
    pub fn add_unresolved_demand(&mut self, demand: AdaptiveHybridUnresolvedDemand) {
        self.unresolved_demand_count += 1;
        push_unresolved_detail(&mut self.unresolved_demands, demand);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CandidateKind {
    Physical,
    Balance,
    Quality,
    Boundary,
}

#[derive(Clone, Debug)]
struct Candidate {
    stable: StableFaceId,
    criterion_id: String,
    hard: bool,
    violation: f64,
    kind: CandidateKind,
}

#[derive(Clone, Debug)]
struct TargetSpec {
    demand: AdaptiveHybridDemand,
    target_edge: f64,
}

#[derive(Clone, Debug)]
struct Evaluation {
    candidates: Vec<Candidate>,
    source_unresolved: Vec<AdaptiveHybridUnresolvedDemand>,
    source_unresolved_count: usize,
    target_satisfaction: AdaptiveHybridTargetSatisfaction,
}

#[derive(Default)]
struct EvaluationAccumulator {
    candidates: Vec<Candidate>,
    source_unresolved: Vec<AdaptiveHybridUnresolvedDemand>,
    source_unresolved_count: usize,
    target_satisfaction: AdaptiveHybridTargetSatisfaction,
}

struct FaceEvaluation {
    candidate: Option<Candidate>,
    source_unresolved: Vec<AdaptiveHybridUnresolvedDemand>,
    target_satisfaction: AdaptiveHybridTargetSatisfaction,
}

impl EvaluationAccumulator {
    fn push(&mut self, item: FaceEvaluation) {
        if let Some(candidate) = item.candidate {
            self.candidates.push(candidate);
        }
        self.source_unresolved_count += item.source_unresolved.len();
        for unresolved in item.source_unresolved {
            push_unresolved_detail(&mut self.source_unresolved, unresolved);
        }
        self.add_satisfaction(item.target_satisfaction);
    }

    fn extend(&mut self, mut other: Self) {
        self.candidates.append(&mut other.candidates);
        self.source_unresolved_count += other.source_unresolved_count;
        for unresolved in other.source_unresolved {
            push_unresolved_detail(&mut self.source_unresolved, unresolved);
        }
        self.add_satisfaction(other.target_satisfaction);
    }

    fn add_satisfaction(&mut self, satisfaction: AdaptiveHybridTargetSatisfaction) {
        self.target_satisfaction.target_faces += satisfaction.target_faces;
        self.target_satisfaction.satisfied_faces += satisfaction.satisfied_faces;
        self.target_satisfaction.unsatisfied_faces += satisfaction.unsatisfied_faces;
    }
}

fn push_unresolved_detail(
    items: &mut Vec<AdaptiveHybridUnresolvedDemand>,
    item: AdaptiveHybridUnresolvedDemand,
) {
    items.push(item);
    items.sort_by(|left, right| {
        left.criterion_id
            .cmp(&right.criterion_id)
            .then_with(|| left.face.cmp(&right.face))
            .then_with(|| {
                unresolved_reason_rank(&left.reason).cmp(&unresolved_reason_rank(&right.reason))
            })
            .then_with(|| right.hard.cmp(&left.hard))
            .then_with(|| left.message.cmp(&right.message))
    });
    items.truncate(LEPP_REPORT_DETAIL_LIMIT);
}

fn unresolved_reason_rank(reason: &AdaptiveHybridUnresolvedReason) -> u8 {
    match reason {
        AdaptiveHybridUnresolvedReason::SourceResolution => 0,
        AdaptiveHybridUnresolvedReason::Limit => 1,
        AdaptiveHybridUnresolvedReason::Rejection => 2,
    }
}

fn keep_better_candidate(best: &mut Option<Candidate>, candidate: Candidate) {
    if best
        .as_ref()
        .is_none_or(|current| better_candidate_order(&candidate, current) == Ordering::Less)
    {
        *best = Some(candidate);
    }
}

pub fn adaptive_hybrid_target_edge_from_level(
    initial_mesh: &MeshState,
    region: &RefinementRegion,
) -> Result<f64, AdaptiveHybridError> {
    let radius = mesh_radius(initial_mesh)?;
    let mut lengths = longest_edges_in_region(initial_mesh, region, radius)?;
    if lengths.is_empty() {
        let face = locate_region_representative_face(initial_mesh, region, radius)?;
        lengths.push(face_longest_edge(initial_mesh, face, radius)?);
    }
    lengths.sort_by(|left, right| left.partial_cmp(right).unwrap_or(Ordering::Equal));
    let h0 = lengths[lengths.len() / 2];
    let level = i32::try_from(region.level()).map_err(|_| AdaptiveHybridError::InvalidConfig {
        message: format!("refinement level {} is too large", region.level()),
    })?;
    let target = h0 / 2f64.powi(level);
    if !target.is_finite() || target <= 0.0 {
        return Err(AdaptiveHybridError::InvalidConfig {
            message: format!(
                "refinement level {} has no finite positive target edge",
                region.level()
            ),
        });
    }
    Ok(target)
}

pub fn refine_adaptive_hybrid(
    mesh: &mut MeshState,
    demands: &[AdaptiveHybridDemand],
    config: &AdaptiveHybridConfig,
) -> Result<AdaptiveHybridReport, AdaptiveHybridError> {
    refine_adaptive_hybrid_impl(mesh, demands, config, None)
}

pub fn refine_adaptive_hybrid_constrained(
    mesh: &mut MeshState,
    segments: &mut SegmentList,
    demands: &[AdaptiveHybridDemand],
    config: &AdaptiveHybridConfig,
) -> Result<AdaptiveHybridReport, AdaptiveHybridError> {
    refine_adaptive_hybrid_impl(mesh, demands, config, Some(segments))
}

fn refine_adaptive_hybrid_impl(
    mesh: &mut MeshState,
    demands: &[AdaptiveHybridDemand],
    config: &AdaptiveHybridConfig,
    mut segments: Option<&mut SegmentList>,
) -> Result<AdaptiveHybridReport, AdaptiveHybridError> {
    validate_config(config)?;
    if let Err(errors) = mesh.validate() {
        return Err(AdaptiveHybridError::InvalidMesh {
            message: format!("topology validation failed: {errors:?}"),
        });
    }
    if mesh.open_edge_count() != 0 && segments.is_none() {
        return Err(AdaptiveHybridError::InvalidMesh {
            message: format!(
                "adaptive hybrid currently requires a closed mesh; found {} open edges",
                mesh.open_edge_count()
            ),
        });
    }
    if let Some(segments) = segments.as_deref() {
        if let Some(edge) = first_unprotected_open_edge(mesh, segments) {
            return Err(AdaptiveHybridError::InvalidMesh {
                message: format!(
                    "open boundary edge [{}, {}] is not present in the protected segment list",
                    edge[0], edge[1]
                ),
            });
        }
        if let Some(edge) = first_non_mesh_segment(mesh, segments) {
            return Err(AdaptiveHybridError::InvalidMesh {
                message: format!(
                    "protected segment [{}, {}] is not a current mesh edge",
                    edge[0], edge[1]
                ),
            });
        }
    }

    let initial = mesh.clone();
    let targets = demands
        .iter()
        .cloned()
        .map(|demand| {
            demand
                .region
                .validate()
                .map_err(|error| AdaptiveHybridError::InvalidConfig {
                    message: format!(
                        "demand '{}' region is invalid: {error}",
                        demand.criterion_id
                    ),
                })?;
            if demand
                .source_resolution_m
                .is_some_and(|source| !source.is_finite() || source <= 0.0)
            {
                return Err(AdaptiveHybridError::InvalidConfig {
                    message: format!(
                        "demand '{}' source resolution must be finite and positive",
                        demand.criterion_id
                    ),
                });
            }
            if demand
                .target_edge_m
                .is_some_and(|target| !target.is_finite() || target <= 0.0)
            {
                return Err(AdaptiveHybridError::InvalidConfig {
                    message: format!(
                        "demand '{}' target edge must be finite and positive",
                        demand.criterion_id
                    ),
                });
            }
            let target_edge = match demand.target_edge_m {
                Some(target) => target,
                None => adaptive_hybrid_target_edge_from_level(&initial, &demand.region)?,
            };
            Ok(TargetSpec {
                target_edge,
                demand,
            })
        })
        .collect::<Result<Vec<_>, AdaptiveHybridError>>()?;

    let mut report = AdaptiveHybridReport {
        cycles: 0,
        insertions: Vec::new(),
        insertion_counts: AdaptiveHybridInsertionCounts::default(),
        path_stats: AdaptiveHybridPathStats::default(),
        initial_vertices: mesh.vertex_count(),
        final_vertices: mesh.vertex_count(),
        initial_faces: mesh.triangle_count(),
        final_faces: mesh.triangle_count(),
        target_satisfaction: AdaptiveHybridTargetSatisfaction::default(),
        unresolved_demand_count: 0,
        unresolved_demands: Vec::new(),
        rejections: Vec::new(),
        stop_reason: AdaptiveHybridStopReason::Satisfied,
    };
    let mut rejected_this_mesh = BTreeSet::<(String, StableFaceId)>::new();
    let mut path_histogram = BTreeMap::<usize, usize>::new();

    loop {
        let evaluation = evaluate(mesh, &targets, config, &rejected_this_mesh)?;
        report.target_satisfaction = evaluation.target_satisfaction;
        report.unresolved_demand_count = evaluation.source_unresolved_count;
        report.unresolved_demands = evaluation.source_unresolved;
        if evaluation.candidates.is_empty() {
            report.stop_reason = if report.unresolved_demand_count == 0 {
                AdaptiveHybridStopReason::Satisfied
            } else {
                AdaptiveHybridStopReason::NoCommittableInsertion
            };
            append_final_unresolved(mesh, &targets, config, &mut report)?;
            break;
        }
        if report.cycles >= config.max_cycles {
            report.stop_reason = AdaptiveHybridStopReason::MaxCycles;
            append_final_unresolved(mesh, &targets, config, &mut report)?;
            break;
        }
        if mesh.vertex_count() >= config.maximum_vertices {
            report.stop_reason = AdaptiveHybridStopReason::MaxVertices;
            append_final_unresolved(mesh, &targets, config, &mut report)?;
            break;
        }

        report.cycles += 1;
        let mut committed_this_cycle = 0usize;
        for candidate in evaluation.candidates {
            if committed_this_cycle >= config.maximum_insertions_per_cycle
                || mesh.vertex_count() >= config.maximum_vertices
            {
                break;
            }
            if !mesh.contains_face_id(candidate.stable) {
                continue;
            }
            if !candidate_is_still_actionable(
                mesh,
                &candidate,
                &targets,
                config,
                &rejected_this_mesh,
            )? {
                continue;
            }
            report.path_stats.attempted += 1;
            let insertion = if candidate.kind == CandidateKind::Quality {
                let quality_config = LeppPostQualityConfig {
                    minimum_spherical_triangle_angle_degrees: Some(config.minimum_triangle_angle),
                    ..LeppPostQualityConfig::default()
                };
                let baseline = quality_snapshot(mesh, &quality_config).map_err(|error| {
                    AdaptiveHybridError::InvalidMesh {
                        message: error.to_string(),
                    }
                })?;
                if let Some(segments) = segments.as_deref_mut() {
                    insert_lepp_terminal_midpoint_constrained_with_postcondition(
                        mesh,
                        segments,
                        candidate.stable.slot,
                        &config.search,
                        &config.gates,
                        |state, _| {
                            quality_snapshot(state, &quality_config).is_ok_and(|after| {
                                strictly_improves_quality_snapshot(after, baseline)
                            })
                        },
                    )
                } else {
                    insert_lepp_terminal_midpoint_with_postcondition(
                        mesh,
                        candidate.stable.slot,
                        &config.search,
                        &config.gates,
                        |state, _| {
                            quality_snapshot(state, &quality_config).is_ok_and(|after| {
                                strictly_improves_quality_snapshot(after, baseline)
                            })
                        },
                    )
                }
            } else if let Some(segments) = segments.as_deref_mut() {
                insert_lepp_terminal_midpoint_constrained(
                    mesh,
                    segments,
                    candidate.stable.slot,
                    &config.search,
                    &config.gates,
                )
            } else {
                insert_lepp_terminal_midpoint_with_postcondition(
                    mesh,
                    candidate.stable.slot,
                    &config.search,
                    &config.gates,
                    |_, _| true,
                )
            };
            match insertion {
                Ok(insertion) => {
                    committed_this_cycle += 1;
                    report.path_stats.committed += 1;
                    let path_len = insertion.path.faces.len();
                    report.path_stats.total_path_faces += path_len;
                    report.path_stats.max_path_faces =
                        report.path_stats.max_path_faces.max(path_len);
                    *path_histogram.entry(path_len).or_default() += 1;
                    push_report_detail(&mut report.path_stats.path_lengths, path_len);
                    match committed_kind(candidate.kind, insertion.split_reason) {
                        CandidateKind::Physical => report.insertion_counts.physical += 1,
                        CandidateKind::Balance => report.insertion_counts.balance += 1,
                        CandidateKind::Quality => report.insertion_counts.quality += 1,
                        CandidateKind::Boundary => report.insertion_counts.boundary += 1,
                    }
                    push_report_detail(&mut report.insertions, insertion);
                    rejected_this_mesh.clear();
                }
                Err(error) => {
                    report.path_stats.rejected += 1;
                    rejected_this_mesh.insert((candidate.criterion_id.clone(), candidate.stable));
                    push_report_detail(
                        &mut report.rejections,
                        AdaptiveHybridRejection {
                            criterion_id: candidate.criterion_id,
                            face: candidate.stable,
                            hard: candidate.hard,
                            error,
                        },
                    );
                }
            }
        }
        if committed_this_cycle == 0 {
            report.stop_reason = AdaptiveHybridStopReason::NoCommittableInsertion;
            append_final_unresolved(mesh, &targets, config, &mut report)?;
            break;
        }
    }

    report.final_vertices = mesh.vertex_count();
    report.final_faces = mesh.triangle_count();
    update_path_distribution(&mut report.path_stats, &path_histogram);
    Ok(report)
}

fn first_unprotected_open_edge(mesh: &MeshState, segments: &SegmentList) -> Option<[usize; 2]> {
    let mut counts = BTreeMap::<[usize; 2], usize>::new();
    for &triangle in &mesh.triangles()[earthmesh_mesh::MESH_STATE_FIRST_ID..] {
        for (a, b) in [
            (triangle[0], triangle[1]),
            (triangle[1], triangle[2]),
            (triangle[2], triangle[0]),
        ] {
            let edge = if a <= b { [a, b] } else { [b, a] };
            *counts.entry(edge).or_default() += 1;
        }
    }
    counts.into_iter().find_map(|(edge, count)| {
        (count == 1 && !segments.contains(edge[0], edge[1])).then_some(edge)
    })
}

fn first_non_mesh_segment(mesh: &MeshState, segments: &SegmentList) -> Option<[usize; 2]> {
    let mut edges = BTreeSet::<[usize; 2]>::new();
    for &triangle in &mesh.triangles()[earthmesh_mesh::MESH_STATE_FIRST_ID..] {
        for (a, b) in [
            (triangle[0], triangle[1]),
            (triangle[1], triangle[2]),
            (triangle[2], triangle[0]),
        ] {
            edges.insert(if a <= b { [a, b] } else { [b, a] });
        }
    }
    segments
        .iter()
        .map(|(a, b)| if a <= b { [a, b] } else { [b, a] })
        .find(|edge| !edges.contains(edge))
}

pub fn refine_adaptive_hybrid_regions(
    mesh: &mut MeshState,
    regions: &[RefinementRegion],
    config: &AdaptiveHybridConfig,
) -> Result<AdaptiveHybridReport, AdaptiveHybridError> {
    let demands = regions
        .iter()
        .enumerate()
        .map(|(index, region)| {
            AdaptiveHybridDemand::user_region(format!("region-{index}"), region.clone())
        })
        .collect::<Vec<_>>();
    refine_adaptive_hybrid(mesh, &demands, config)
}

fn validate_config(config: &AdaptiveHybridConfig) -> Result<(), AdaptiveHybridError> {
    let invalid = |message: &str| AdaptiveHybridError::InvalidConfig {
        message: message.to_string(),
    };
    if config.max_cycles == 0 {
        return Err(invalid("max_cycles must be greater than zero"));
    }
    if !config.target_size_tolerance.is_finite() || config.target_size_tolerance < 1.0 {
        return Err(invalid(
            "target_size_tolerance must be finite and at least one",
        ));
    }
    if !config.maximum_neighbor_size_ratio.is_finite() || config.maximum_neighbor_size_ratio <= 1.0
    {
        return Err(invalid(
            "maximum_neighbor_size_ratio must be finite and greater than one",
        ));
    }
    if config.maximum_vertices == 0 || config.maximum_insertions_per_cycle == 0 {
        return Err(invalid(
            "maximum_vertices and maximum_insertions_per_cycle must be positive",
        ));
    }
    if !config.minimum_triangle_angle.is_finite()
        || config.minimum_triangle_angle < 0.0
        || config.minimum_triangle_angle >= 60.0
    {
        return Err(invalid(
            "minimum_triangle_angle must be finite and in [0, 60)",
        ));
    }
    if config.gates.maximum_vertex_degree < 3 {
        return Err(invalid("maximum_vertex_degree must be at least three"));
    }
    Ok(())
}

fn evaluate(
    mesh: &MeshState,
    targets: &[TargetSpec],
    config: &AdaptiveHybridConfig,
    rejected: &BTreeSet<(String, StableFaceId)>,
) -> Result<Evaluation, AdaptiveHybridError> {
    let radius = mesh_radius(mesh)?;
    let representative_faces = representative_faces(mesh, targets, radius);
    // ponytail: O(faces * demands); add a spherical region index only when
    // profiles show many simultaneous demands dominate this parallel scan.
    let mut acc = (earthmesh_mesh::MESH_STATE_FIRST_ID..mesh.triangles().len())
        .into_par_iter()
        .map(|face| {
            evaluate_face(
                mesh,
                face,
                radius,
                targets,
                &representative_faces,
                config,
                rejected,
            )
        })
        .try_fold(EvaluationAccumulator::default, |mut acc, item| {
            acc.push(item?);
            Ok::<_, AdaptiveHybridError>(acc)
        })
        .try_reduce(EvaluationAccumulator::default, |mut left, right| {
            left.extend(right);
            Ok::<_, AdaptiveHybridError>(left)
        })?;
    acc.candidates.sort_by(better_candidate_order);
    Ok(Evaluation {
        candidates: acc.candidates,
        source_unresolved: acc.source_unresolved,
        source_unresolved_count: acc.source_unresolved_count,
        target_satisfaction: acc.target_satisfaction,
    })
}

fn evaluate_face(
    mesh: &MeshState,
    face: FaceId,
    radius: f64,
    targets: &[TargetSpec],
    representative_faces: &[Option<FaceId>],
    config: &AdaptiveHybridConfig,
    rejected: &BTreeSet<(String, StableFaceId)>,
) -> Result<FaceEvaluation, AdaptiveHybridError> {
    let stable = mesh
        .face_id(face)
        .ok_or_else(|| AdaptiveHybridError::InvalidMesh {
            message: format!("face {face} has no stable id"),
        })?;
    let center = face_center(mesh, face)?;
    let longest = face_longest_edge(mesh, face, radius)?;
    let mut best_candidate = None;
    let mut unresolved = Vec::new();
    let mut satisfaction = AdaptiveHybridTargetSatisfaction::default();

    for (target_index, spec) in targets.iter().enumerate() {
        if !spec.demand.region.contains_cartesian(center, radius)
            && representative_faces.get(target_index).copied().flatten() != Some(face)
        {
            continue;
        }
        satisfaction.target_faces += 1;
        let violation = longest / (config.target_size_tolerance * spec.target_edge) - 1.0;
        if violation > 0.0 {
            satisfaction.unsatisfied_faces += 1;
            if let Some(source) = spec.demand.source_resolution_m {
                if config.stop_at_source_resolution && spec.target_edge < source {
                    unresolved.push(AdaptiveHybridUnresolvedDemand {
                        criterion_id: spec.demand.criterion_id.clone(),
                        face: Some(stable),
                        hard: spec.demand.hard,
                        reason: AdaptiveHybridUnresolvedReason::SourceResolution,
                        message: format!(
                            "target edge {} is below source resolution {source}",
                            spec.target_edge
                        ),
                    });
                    continue;
                }
            }
            if !rejected.contains(&(spec.demand.criterion_id.clone(), stable)) {
                keep_better_candidate(
                    &mut best_candidate,
                    Candidate {
                        stable,
                        criterion_id: spec.demand.criterion_id.clone(),
                        hard: spec.demand.hard,
                        violation,
                        kind: kind_from_cause(&spec.demand.cause),
                    },
                );
            }
        } else {
            satisfaction.satisfied_faces += 1;
        }
    }

    if let Some(balance) = balance_violation(mesh, face, radius, config)? {
        keep_better_candidate(
            &mut best_candidate,
            Candidate {
                stable,
                criterion_id: "scale-balance".to_string(),
                hard: false,
                violation: balance,
                kind: CandidateKind::Balance,
            },
        );
    }
    if config.minimum_triangle_angle > 0.0 {
        let angle = minimum_face_angle_degrees(mesh, face)?;
        if angle < config.minimum_triangle_angle {
            keep_better_candidate(
                &mut best_candidate,
                Candidate {
                    stable,
                    criterion_id: "mesh-quality".to_string(),
                    hard: false,
                    violation: (config.minimum_triangle_angle - angle)
                        / config.minimum_triangle_angle,
                    kind: CandidateKind::Quality,
                },
            );
        }
    }

    Ok(FaceEvaluation {
        candidate: best_candidate,
        source_unresolved: unresolved,
        target_satisfaction: satisfaction,
    })
}

fn kind_from_cause(cause: &RefinementCause) -> CandidateKind {
    match cause {
        RefinementCause::BoundaryResolution => CandidateKind::Boundary,
        RefinementCause::ScaleBalance { .. } => CandidateKind::Balance,
        RefinementCause::QualityRepair => CandidateKind::Quality,
        RefinementCause::PhysicalCriterion { .. } | RefinementCause::UserSpecified => {
            CandidateKind::Physical
        }
    }
}

fn committed_kind(
    candidate: CandidateKind,
    split_reason: LeppInsertionSplitReason,
) -> CandidateKind {
    match split_reason {
        LeppInsertionSplitReason::EncroachedSegment => CandidateKind::Boundary,
        LeppInsertionSplitReason::TerminalEdge => candidate,
    }
}

fn candidate_is_still_actionable(
    mesh: &MeshState,
    candidate: &Candidate,
    targets: &[TargetSpec],
    config: &AdaptiveHybridConfig,
    rejected: &BTreeSet<(String, StableFaceId)>,
) -> Result<bool, AdaptiveHybridError> {
    let radius = mesh_radius(mesh)?;
    let representatives = representative_faces(mesh, targets, radius);
    let current = evaluate_face(
        mesh,
        candidate.stable.slot,
        radius,
        targets,
        &representatives,
        config,
        rejected,
    )?
    .candidate;
    Ok(current.is_some_and(|current| {
        current.kind == candidate.kind && current.criterion_id == candidate.criterion_id
    }))
}

fn update_path_distribution(
    stats: &mut AdaptiveHybridPathStats,
    histogram: &BTreeMap<usize, usize>,
) {
    if stats.committed == 0 {
        stats.mean_path_faces = 0.0;
        stats.p95_path_faces = 0.0;
        return;
    }
    stats.mean_path_faces = stats.total_path_faces as f64 / stats.committed as f64;
    let target_rank = ((stats.committed as f64) * 0.95).ceil() as usize;
    let mut seen = 0usize;
    for (&length, &count) in histogram {
        seen += count;
        if seen >= target_rank {
            stats.p95_path_faces = length as f64;
            return;
        }
    }
    stats.p95_path_faces = stats.max_path_faces as f64;
}

fn better_candidate_order(left: &Candidate, right: &Candidate) -> Ordering {
    right
        .hard
        .cmp(&left.hard)
        .then_with(|| {
            right
                .violation
                .partial_cmp(&left.violation)
                .unwrap_or(Ordering::Equal)
        })
        .then_with(|| left.criterion_id.cmp(&right.criterion_id))
        .then_with(|| left.stable.slot.cmp(&right.stable.slot))
        .then_with(|| left.stable.generation.cmp(&right.stable.generation))
}

fn append_final_unresolved(
    mesh: &MeshState,
    targets: &[TargetSpec],
    config: &AdaptiveHybridConfig,
    report: &mut AdaptiveHybridReport,
) -> Result<(), AdaptiveHybridError> {
    let radius = mesh_radius(mesh)?;
    let representative_faces = representative_faces(mesh, targets, radius);
    for face in earthmesh_mesh::MESH_STATE_FIRST_ID..mesh.triangles().len() {
        let stable = mesh
            .face_id(face)
            .ok_or_else(|| AdaptiveHybridError::InvalidMesh {
                message: format!("face {face} has no stable id"),
            })?;
        let center = face_center(mesh, face)?;
        let longest = face_longest_edge(mesh, face, radius)?;
        let reason = match report.stop_reason {
            AdaptiveHybridStopReason::NoCommittableInsertion => {
                AdaptiveHybridUnresolvedReason::Rejection
            }
            _ => AdaptiveHybridUnresolvedReason::Limit,
        };
        for (target_index, spec) in targets.iter().enumerate() {
            if !spec.demand.region.contains_cartesian(center, radius)
                && representative_faces.get(target_index).copied().flatten() != Some(face)
            {
                continue;
            }
            let violation = longest / (config.target_size_tolerance * spec.target_edge) - 1.0;
            let already_source_limited = config.stop_at_source_resolution
                && spec
                    .demand
                    .source_resolution_m
                    .is_some_and(|source| spec.target_edge < source);
            if violation > 0.0 && !already_source_limited {
                report.add_unresolved_demand(AdaptiveHybridUnresolvedDemand {
                    criterion_id: spec.demand.criterion_id.clone(),
                    face: Some(stable),
                    hard: spec.demand.hard,
                    reason: reason.clone(),
                    message: format!(
                        "face longest edge {longest} still exceeds target {}",
                        spec.target_edge
                    ),
                });
            }
        }
        if let Some(violation) = balance_violation(mesh, face, radius, config)? {
            report.add_unresolved_demand(AdaptiveHybridUnresolvedDemand {
                criterion_id: "scale-balance".to_string(),
                face: Some(stable),
                hard: false,
                reason: reason.clone(),
                message: format!("neighbour size ratio still exceeds limit by {violation}"),
            });
        }
        if config.minimum_triangle_angle > 0.0 {
            let angle = minimum_face_angle_degrees(mesh, face)?;
            if angle < config.minimum_triangle_angle {
                report.add_unresolved_demand(AdaptiveHybridUnresolvedDemand {
                    criterion_id: "mesh-quality".to_string(),
                    face: Some(stable),
                    hard: false,
                    reason: reason.clone(),
                    message: format!(
                        "minimum triangle angle {angle} is below {}",
                        config.minimum_triangle_angle
                    ),
                });
            }
        }
    }
    Ok(())
}

fn representative_faces(
    mesh: &MeshState,
    targets: &[TargetSpec],
    radius: f64,
) -> Vec<Option<FaceId>> {
    targets
        .iter()
        .map(|target| locate_region_representative_face(mesh, &target.demand.region, radius).ok())
        .collect()
}

fn locate_region_representative_face(
    mesh: &MeshState,
    region: &RefinementRegion,
    radius: f64,
) -> Result<FaceId, AdaptiveHybridError> {
    let unit = lonlat_degrees_to_unit_xyz(region_representative(region));
    let point = CartesianPoint::new(unit.x * radius, unit.y * radius, unit.z * radius);
    if region.contains_cartesian(point, radius) {
        return mesh.locate_triangle(point, None).map_err(|error| {
            AdaptiveHybridError::InvalidMesh {
                message: format!("could not locate representative point: {error}"),
            }
        });
    }
    for face in earthmesh_mesh::MESH_STATE_FIRST_ID..mesh.triangles().len() {
        if region.contains_cartesian(face_center(mesh, face)?, radius) {
            return Ok(face);
        }
    }
    Err(AdaptiveHybridError::InvalidConfig {
        message: "region representative is outside the refinement region".to_string(),
    })
}

fn region_representative(region: &RefinementRegion) -> LonLatDegrees {
    match region {
        RefinementRegion::Circle { center, .. } => *center,
        RefinementRegion::Bbox {
            west_degrees,
            east_degrees,
            south_degrees,
            north_degrees,
            ..
        } => LonLatDegrees::new(
            midpoint_longitude(*west_degrees, *east_degrees),
            (south_degrees + north_degrees) / 2.0,
        ),
        RefinementRegion::Corridor { points, .. } => points[0],
        RefinementRegion::Polygon { points, .. } => spherical_centroid_degrees(points)
            .or_else(|| points.first().copied())
            .unwrap_or_else(|| LonLatDegrees::new(0.0, 0.0)),
    }
}

fn midpoint_longitude(west: f64, east: f64) -> f64 {
    let mut east = east;
    if east < west {
        east += 360.0;
    }
    let mut mid = (west + east) / 2.0;
    if mid > 180.0 {
        mid -= 360.0;
    }
    mid
}

fn longest_edges_in_region(
    mesh: &MeshState,
    region: &RefinementRegion,
    radius: f64,
) -> Result<Vec<f64>, AdaptiveHybridError> {
    let mut lengths = Vec::new();
    for face in earthmesh_mesh::MESH_STATE_FIRST_ID..mesh.triangles().len() {
        if region.contains_cartesian(face_center(mesh, face)?, radius) {
            lengths.push(face_longest_edge(mesh, face, radius)?);
        }
    }
    Ok(lengths)
}

fn balance_violation(
    mesh: &MeshState,
    face: FaceId,
    radius: f64,
    config: &AdaptiveHybridConfig,
) -> Result<Option<f64>, AdaptiveHybridError> {
    let own = face_longest_edge(mesh, face, radius)?;
    let mut worst = 0.0;
    for &neighbour in &mesh.neighbours()[face] {
        if neighbour == 0 {
            continue;
        }
        let other = face_longest_edge(mesh, neighbour, radius)?;
        let ratio = own / other;
        if ratio > config.maximum_neighbor_size_ratio {
            worst = f64::max(worst, ratio / config.maximum_neighbor_size_ratio - 1.0);
        }
    }
    Ok((worst > 0.0).then_some(worst))
}

fn face_longest_edge(
    mesh: &MeshState,
    face: FaceId,
    radius: f64,
) -> Result<f64, AdaptiveHybridError> {
    let corners = *mesh
        .triangles()
        .get(face)
        .ok_or_else(|| AdaptiveHybridError::InvalidMesh {
            message: format!("face {face} is out of range"),
        })?;
    let points = [
        point(mesh, face, corners[0])?,
        point(mesh, face, corners[1])?,
        point(mesh, face, corners[2])?,
    ];
    let mut longest = 0.0f64;
    for (a, b) in [
        (points[0], points[1]),
        (points[1], points[2]),
        (points[2], points[0]),
    ] {
        let length = spherical_edge_length(radius, a, b);
        if !length.is_finite() || length <= 0.0 {
            return Err(AdaptiveHybridError::InvalidMesh {
                message: format!("face {face} has invalid edge length"),
            });
        }
        longest = longest.max(length);
    }
    Ok(longest)
}

fn face_center(mesh: &MeshState, face: FaceId) -> Result<CartesianPoint, AdaptiveHybridError> {
    let corners = *mesh
        .triangles()
        .get(face)
        .ok_or_else(|| AdaptiveHybridError::InvalidMesh {
            message: format!("face {face} is out of range"),
        })?;
    let p = CartesianPoint::new(
        mesh.vertices()[corners[0]].x
            + mesh.vertices()[corners[1]].x
            + mesh.vertices()[corners[2]].x,
        mesh.vertices()[corners[0]].y
            + mesh.vertices()[corners[1]].y
            + mesh.vertices()[corners[2]].y,
        mesh.vertices()[corners[0]].z
            + mesh.vertices()[corners[1]].z
            + mesh.vertices()[corners[2]].z,
    );
    let norm = magnitude(p);
    let radius = (magnitude(mesh.vertices()[corners[0]])
        + magnitude(mesh.vertices()[corners[1]])
        + magnitude(mesh.vertices()[corners[2]]))
        / 3.0;
    if !norm.is_finite() || norm <= 0.0 || !radius.is_finite() || radius <= 0.0 {
        return Err(AdaptiveHybridError::InvalidMesh {
            message: format!("face {face} has invalid center"),
        });
    }
    Ok(CartesianPoint::new(
        p.x / norm * radius,
        p.y / norm * radius,
        p.z / norm * radius,
    ))
}

fn point(
    mesh: &MeshState,
    face: FaceId,
    vertex: usize,
) -> Result<CartesianPoint, AdaptiveHybridError> {
    mesh.vertices()
        .get(vertex)
        .copied()
        .ok_or_else(|| AdaptiveHybridError::InvalidMesh {
            message: format!("face {face} names invalid vertex {vertex}"),
        })
}

fn mesh_radius(mesh: &MeshState) -> Result<f64, AdaptiveHybridError> {
    let mut total = 0.0;
    let mut count = 0usize;
    for point in &mesh.vertices()[earthmesh_mesh::MESH_STATE_FIRST_ID..] {
        let radius = magnitude(*point);
        if !radius.is_finite() || radius <= 0.0 {
            return Err(AdaptiveHybridError::InvalidMesh {
                message: "mesh has invalid vertex radius".to_string(),
            });
        }
        total += radius;
        count += 1;
    }
    let radius = total / count as f64;
    if !radius.is_finite() || radius <= 0.0 {
        return Err(AdaptiveHybridError::InvalidMesh {
            message: "mesh radius is invalid".to_string(),
        });
    }
    Ok(radius)
}

fn minimum_face_angle_degrees(mesh: &MeshState, face: FaceId) -> Result<f64, AdaptiveHybridError> {
    let corners = mesh.triangles()[face];
    let points = [
        point(mesh, face, corners[0])?,
        point(mesh, face, corners[1])?,
        point(mesh, face, corners[2])?,
    ];
    let mut minimum = f64::MAX;
    for i in 0..3 {
        let current = unit(points[i])?;
        let previous = unit(points[(i + 2) % 3])?;
        let next = unit(points[(i + 1) % 3])?;
        let angle = dot(tangent(current, previous)?, tangent(current, next)?)
            .clamp(-1.0, 1.0)
            .acos()
            .to_degrees();
        if !angle.is_finite() || angle <= 0.0 {
            return Err(AdaptiveHybridError::InvalidMesh {
                message: format!("face {face} has invalid angle"),
            });
        }
        minimum = minimum.min(angle);
    }
    Ok(minimum)
}

fn tangent(
    from: CartesianPoint,
    to: CartesianPoint,
) -> Result<CartesianPoint, AdaptiveHybridError> {
    unit(CartesianPoint::new(
        to.x - from.x * dot(from, to),
        to.y - from.y * dot(from, to),
        to.z - from.z * dot(from, to),
    ))
}

fn unit(point: CartesianPoint) -> Result<CartesianPoint, AdaptiveHybridError> {
    let norm = magnitude(point);
    if !norm.is_finite() || norm <= 0.0 {
        return Err(AdaptiveHybridError::InvalidMesh {
            message: "invalid vector".to_string(),
        });
    }
    Ok(CartesianPoint::new(
        point.x / norm,
        point.y / norm,
        point.z / norm,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encroached_segment_commit_counts_as_boundary_work() {
        assert_eq!(
            committed_kind(
                CandidateKind::Physical,
                LeppInsertionSplitReason::EncroachedSegment,
            ),
            CandidateKind::Boundary
        );
    }
}
