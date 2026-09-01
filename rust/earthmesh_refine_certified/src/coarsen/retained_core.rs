//! Deterministic retained coarse-parent subset planning for Frozen N6 recovery.

use super::{
    annulus::{parent_by_source_face, parent_graph},
    build_face_band_problem_with_source_face_rings,
    direct_restore::mesh_angle_range,
    face_band::solve_exact_face_bands_with_filter,
    solve_full_polygon_merge_from_face_bands,
    solve_full_polygon_merge_from_face_bands_with_geometry_witness, AnchorBandPolicy,
    ElasticTargetMode, FaceBandLimits, FaceBandOutcomeKind, FaceBandPlan, FaceBandProblem,
    FaceBandSolveOutcome, FullPolygonCberLimits, FullPolygonMergeLimits, FullPolygonMergeOutcome,
    FullPolygonMergeTrial, GeometryDomainId, GeometryFailureWitness, GeometryStartId,
    HierarchyComponent,
};
use crate::{mother_grid::TriangleAddress, MotherGrid};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const MAX_EXACT_CORE_PARENTS: usize = 20;

#[derive(Debug, Clone, PartialEq)]
pub struct RetainedCoreCandidate {
    pub retained_parents: BTreeSet<TriangleAddress>,
    pub released_parents: BTreeSet<TriangleAddress>,
    pub retained_components: usize,
    pub retained_boundary_edges: usize,
    pub violation_influence_score: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetainedCoreSearchPlan {
    pub initial_coarse_parents: BTreeSet<TriangleAddress>,
    pub parent_adjacency: BTreeMap<TriangleAddress, BTreeSet<TriangleAddress>>,
    pub candidates: Vec<RetainedCoreCandidate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetainedCoreTopologyLimits {
    pub face_band_states: u64,
    pub topology_states: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RetainedCoreCorridorFamily {
    F0CurrentSourceFaceCorridor,
    F1PlusOneSourceFaceRing,
    F2PlusTwoSourceFaceRings,
    F3AnchorOnSingleInterface,
    F4FineCapRadiusOne,
    F5FineCapRadiusTwo,
}

impl RetainedCoreCorridorFamily {
    pub const ALL: [Self; 6] = [
        Self::F0CurrentSourceFaceCorridor,
        Self::F1PlusOneSourceFaceRing,
        Self::F2PlusTwoSourceFaceRings,
        Self::F3AnchorOnSingleInterface,
        Self::F4FineCapRadiusOne,
        Self::F5FineCapRadiusTwo,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::F0CurrentSourceFaceCorridor => "F0CurrentSourceFaceCorridor",
            Self::F1PlusOneSourceFaceRing => "F1PlusOneSourceFaceRing",
            Self::F2PlusTwoSourceFaceRings => "F2PlusTwoSourceFaceRings",
            Self::F3AnchorOnSingleInterface => "F3AnchorOnSingleInterface",
            Self::F4FineCapRadiusOne => "F4FineCapRadiusOne",
            Self::F5FineCapRadiusTwo => "F5FineCapRadiusTwo",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetainedCoreFamilyStatus {
    Closed,
    ExactNoSolution,
    SearchIncomplete,
    InvalidInput,
}

impl RetainedCoreFamilyStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Closed => "Closed",
            Self::ExactNoSolution => "ExactNoSolution",
            Self::SearchIncomplete => "SearchIncomplete",
            Self::InvalidInput => "InvalidInput",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetainedCoreLadderLimits {
    pub topology: RetainedCoreTopologyLimits,
    pub geometry_topology_states: usize,
    pub geometry_iterations: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetainedCoreGeometryStatus {
    Certified,
    ContinuousSearchIncomplete,
    InvalidInput,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetainedCoreGeometryEvidence {
    pub status: RetainedCoreGeometryStatus,
    pub candidates_attempted: usize,
    pub best_angle_range_deg: Option<(f64, f64)>,
    pub strict_certified: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetainedCoreFamilyAttempt {
    pub candidate: RetainedCoreCandidate,
    pub family: RetainedCoreCorridorFamily,
    pub status: RetainedCoreFamilyStatus,
    pub topology: RetainedCoreTopologyEvidence,
    pub geometry: Option<RetainedCoreGeometryEvidence>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetainedCoreLadderReport {
    pub triggered: bool,
    pub attempted_cardinalities: Vec<usize>,
    pub connected_candidates_attempted: usize,
    pub families_attempted: usize,
    pub attempts: Vec<RetainedCoreFamilyAttempt>,
    pub strict_certified: bool,
    pub selected_strict_attempt: Option<usize>,
    pub best_angle_range_deg: Option<(f64, f64)>,
    pub retain_one_tested: bool,
    pub continuous_search_incomplete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetainedCoreTopologyOutcomeKind {
    Closed,
    TopologyFamilyExhaustedNoSolution,
    SearchBudgetExhausted,
    InvalidInput,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetainedCoreTopologyEvidence {
    pub retained_parents: BTreeSet<TriangleAddress>,
    pub released_parents: BTreeSet<TriangleAddress>,
    pub face_band_outcome: FaceBandOutcomeKind,
    pub face_band_states: u64,
    pub topology_outcome: RetainedCoreTopologyOutcomeKind,
    pub topology_states: usize,
    pub selected_topologies: usize,
    pub vertices: Option<usize>,
    pub edges: Option<usize>,
    pub faces: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RetainedCoreTopologyOutcome {
    Closed {
        component: HierarchyComponent,
        face_band_plan: Box<FaceBandPlan>,
        trial: Box<FullPolygonMergeTrial>,
        evidence: RetainedCoreTopologyEvidence,
    },
    TopologyFamilyExhaustedNoSolution(RetainedCoreTopologyEvidence),
    SearchBudgetExhausted(RetainedCoreTopologyEvidence),
    InvalidInput {
        reason: String,
        evidence: RetainedCoreTopologyEvidence,
    },
}

impl RetainedCoreTopologyOutcome {
    pub fn evidence(&self) -> &RetainedCoreTopologyEvidence {
        match self {
            Self::Closed { evidence, .. }
            | Self::TopologyFamilyExhaustedNoSolution(evidence)
            | Self::SearchBudgetExhausted(evidence)
            | Self::InvalidInput { evidence, .. } => evidence,
        }
    }
}

impl RetainedCoreSearchPlan {
    pub fn connected_candidates(&self) -> impl Iterator<Item = &RetainedCoreCandidate> {
        self.candidates
            .iter()
            .filter(|candidate| candidate.retained_components == 1)
    }
}

pub fn plan_retained_core_subsets(
    source: &MotherGrid,
    initial_core: &BTreeSet<TriangleAddress>,
    violation_parents: &BTreeSet<TriangleAddress>,
) -> Result<RetainedCoreSearchPlan, String> {
    if initial_core.is_empty() {
        return Err("retained-core planning requires at least one coarse parent".into());
    }
    if initial_core.len() > MAX_EXACT_CORE_PARENTS {
        return Err(format!(
            "exact retained-core planning is limited to {MAX_EXACT_CORE_PARENTS} parents"
        ));
    }

    let parent_by_face = parent_by_source_face(source).map_err(|error| format!("{error:?}"))?;
    let graph = parent_graph(source, &parent_by_face).map_err(|error| format!("{error:?}"))?;
    for parent in initial_core.iter().chain(violation_parents) {
        if !graph.contains_key(parent) {
            return Err(format!(
                "retained-core parent {parent:?} is absent from the source hierarchy"
            ));
        }
    }

    let parent_adjacency = initial_core
        .iter()
        .copied()
        .map(|parent| {
            let neighbours = graph[&parent].intersection(initial_core).copied().collect();
            (parent, neighbours)
        })
        .collect::<BTreeMap<_, _>>();
    let distances = graph_distances(&graph, violation_parents);
    let parents = initial_core.iter().copied().collect::<Vec<_>>();
    let mut candidates = Vec::with_capacity(1usize << parents.len());
    for mask in 0..(1usize << parents.len()) {
        let retained_parents = parents
            .iter()
            .enumerate()
            .filter_map(|(index, &parent)| ((mask & (1usize << index)) != 0).then_some(parent))
            .collect::<BTreeSet<_>>();
        let released_parents = initial_core
            .difference(&retained_parents)
            .copied()
            .collect::<BTreeSet<_>>();
        candidates.push(RetainedCoreCandidate {
            retained_components: component_count(&retained_parents, &parent_adjacency),
            retained_boundary_edges: retained_parents
                .iter()
                .map(|parent| {
                    graph[parent]
                        .iter()
                        .filter(|neighbour| !retained_parents.contains(neighbour))
                        .count()
                })
                .sum(),
            violation_influence_score: released_parents
                .iter()
                .filter_map(|parent| distances.get(parent))
                .map(|distance| 1.0 / (1.0 + *distance as f64))
                .sum(),
            retained_parents,
            released_parents,
        });
    }
    candidates.sort_by(|left, right| {
        right
            .retained_parents
            .len()
            .cmp(&left.retained_parents.len())
            .then(left.retained_components.cmp(&right.retained_components))
            .then_with(|| {
                right
                    .violation_influence_score
                    .total_cmp(&left.violation_influence_score)
            })
            .then(
                left.retained_boundary_edges
                    .cmp(&right.retained_boundary_edges),
            )
            .then(left.retained_parents.cmp(&right.retained_parents))
    });

    Ok(RetainedCoreSearchPlan {
        initial_coarse_parents: initial_core.clone(),
        parent_adjacency,
        candidates,
    })
}

pub fn solve_retained_core_topology(
    source: &MotherGrid,
    original: &HierarchyComponent,
    candidate: &RetainedCoreCandidate,
    limits: RetainedCoreTopologyLimits,
) -> RetainedCoreTopologyOutcome {
    solve_retained_core_topology_family(
        source,
        original,
        candidate,
        RetainedCoreCorridorFamily::F0CurrentSourceFaceCorridor,
        limits,
    )
}

pub fn solve_retained_core_topology_family(
    source: &MotherGrid,
    original: &HierarchyComponent,
    candidate: &RetainedCoreCandidate,
    family: RetainedCoreCorridorFamily,
    limits: RetainedCoreTopologyLimits,
) -> RetainedCoreTopologyOutcome {
    let component = match component_for_retained_core(original, candidate) {
        Ok(component) => component,
        Err(reason) => {
            return invalid_topology(candidate, reason, FaceBandOutcomeKind::InvalidInput, 0, 0)
        }
    };
    let problem = match retained_core_family_problem(source, &component, family) {
        Ok(problem) => problem,
        Err(FamilyProblemError::ExactNoSolution) => {
            return RetainedCoreTopologyOutcome::TopologyFamilyExhaustedNoSolution(evidence_for(
                candidate,
                FaceBandOutcomeKind::FamilyExhaustedNoSolution,
                0,
                RetainedCoreTopologyOutcomeKind::TopologyFamilyExhaustedNoSolution,
                0,
                None,
            ))
        }
        Err(FamilyProblemError::Invalid(reason)) => {
            return invalid_topology(candidate, reason, FaceBandOutcomeKind::InvalidInput, 0, 0)
        }
    };
    let mut closed_trial = None;
    let mut topology_states = 0usize;
    let mut topology_incomplete = false;
    let mut topology_invalid = None;
    let face_band = solve_exact_face_bands_with_filter(
        &problem,
        FaceBandLimits {
            maximum_states: limits.face_band_states,
        },
        |plan| match solve_full_polygon_merge_from_face_bands(
            source,
            &component,
            plan,
            FullPolygonMergeLimits {
                topology_states: limits.topology_states,
            },
        ) {
            FullPolygonMergeOutcome::Closed(trial) => {
                topology_states = topology_states.saturating_add(trial.evidence.states_examined);
                closed_trial = Some(trial);
                true
            }
            FullPolygonMergeOutcome::TopologyFamilyExhaustedNoSolution(evidence) => {
                topology_states = topology_states.saturating_add(evidence.states_examined);
                false
            }
            FullPolygonMergeOutcome::SearchBudgetExhausted(evidence) => {
                topology_states = topology_states.saturating_add(evidence.states_examined);
                topology_incomplete = true;
                false
            }
            FullPolygonMergeOutcome::InvalidInput { reason, evidence } => {
                topology_states = topology_states.saturating_add(evidence.states_examined);
                topology_invalid.get_or_insert(reason);
                false
            }
        },
    );
    match face_band {
        FaceBandSolveOutcome::Closed(face_band_plan, evidence) => {
            let trial = closed_trial.expect("accepted face-band plan must have a topology trial");
            let global = &trial.global_trial.evidence;
            let topology = evidence_for(
                candidate,
                evidence.outcome,
                evidence.states_examined,
                RetainedCoreTopologyOutcomeKind::Closed,
                topology_states,
                Some((
                    trial.evidence.selected_topology_keys.len(),
                    global.vertices,
                    global.edges,
                    global.faces,
                )),
            );
            RetainedCoreTopologyOutcome::Closed {
                component,
                face_band_plan,
                trial,
                evidence: topology,
            }
        }
        FaceBandSolveOutcome::FamilyExhaustedNoSolution { evidence, .. } => {
            if let Some(reason) = topology_invalid {
                invalid_topology(
                    candidate,
                    reason,
                    evidence.outcome,
                    evidence.states_examined,
                    topology_states,
                )
            } else if topology_incomplete {
                RetainedCoreTopologyOutcome::SearchBudgetExhausted(evidence_for(
                    candidate,
                    evidence.outcome,
                    evidence.states_examined,
                    RetainedCoreTopologyOutcomeKind::SearchBudgetExhausted,
                    topology_states,
                    None,
                ))
            } else {
                RetainedCoreTopologyOutcome::TopologyFamilyExhaustedNoSolution(evidence_for(
                    candidate,
                    evidence.outcome,
                    evidence.states_examined,
                    RetainedCoreTopologyOutcomeKind::TopologyFamilyExhaustedNoSolution,
                    topology_states,
                    None,
                ))
            }
        }
        FaceBandSolveOutcome::SearchBudgetExhausted { evidence, .. } => {
            RetainedCoreTopologyOutcome::SearchBudgetExhausted(evidence_for(
                candidate,
                evidence.outcome,
                evidence.states_examined,
                RetainedCoreTopologyOutcomeKind::SearchBudgetExhausted,
                topology_states,
                None,
            ))
        }
        FaceBandSolveOutcome::InvalidInput { reason } => {
            invalid_topology(candidate, reason, FaceBandOutcomeKind::InvalidInput, 0, 0)
        }
    }
}

pub fn retained_core_ladder_required(strict_witness_found: bool) -> bool {
    !strict_witness_found
}

pub fn remaining_connected_retained_core_candidates(
    plan: &RetainedCoreSearchPlan,
) -> Vec<&RetainedCoreCandidate> {
    plan.connected_candidates()
        .filter(|candidate| (1..=7).contains(&candidate.retained_parents.len()))
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub fn solve_complete_retained_core_ladder(
    source: &MotherGrid,
    original: &HierarchyComponent,
    plan: &RetainedCoreSearchPlan,
    inherited_witness: &GeometryFailureWitness,
    source_levels: &[Option<usize>],
    strict_witness_found: bool,
    limits: RetainedCoreLadderLimits,
) -> Result<RetainedCoreLadderReport, String> {
    if !retained_core_ladder_required(strict_witness_found) {
        return Ok(empty_ladder_report(false));
    }
    let original_core = original
        .core_parents
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if plan.initial_coarse_parents != original_core {
        return Err("retained-core ladder plan does not match the original core".into());
    }
    let starts = [
        GeometryStartId::MaterializedSource,
        GeometryStartId::HierarchySpringEquilibrium,
        GeometryStartId::RingScaleInterpolation,
        GeometryStartId::DegreeAngleEquilibrium,
        GeometryStartId::SignedNormalPlus,
        GeometryStartId::SignedNormalMinus,
    ];
    let mut report = empty_ladder_report(true);
    for cardinality in (1..=7).rev() {
        let candidates = plan
            .connected_candidates()
            .filter(|candidate| candidate.retained_parents.len() == cardinality)
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            continue;
        }
        report.attempted_cardinalities.push(cardinality);
        report.connected_candidates_attempted += candidates.len();
        let mut strict_in_cardinality = false;
        for candidate in candidates {
            for family in RetainedCoreCorridorFamily::ALL {
                let outcome = solve_retained_core_topology_family(
                    source,
                    original,
                    candidate,
                    family,
                    limits.topology,
                );
                let status = family_status(&outcome);
                let topology = outcome.evidence().clone();
                let geometry = match outcome {
                    RetainedCoreTopologyOutcome::Closed {
                        component,
                        face_band_plan,
                        ..
                    } => {
                        let geometry = retained_core_geometry(
                            source,
                            &component,
                            &face_band_plan,
                            inherited_witness,
                            source_levels,
                            &starts,
                            limits,
                        );
                        strict_in_cardinality |= geometry.strict_certified;
                        update_best_range(
                            &mut report.best_angle_range_deg,
                            geometry.best_angle_range_deg,
                        );
                        Some(geometry)
                    }
                    _ => None,
                };
                report.attempts.push(RetainedCoreFamilyAttempt {
                    candidate: candidate.clone(),
                    family,
                    status,
                    topology,
                    geometry,
                });
            }
        }
        if strict_in_cardinality {
            break;
        }
    }
    report.families_attempted = report.attempts.len();
    report.strict_certified = report.attempts.iter().any(|attempt| {
        attempt
            .geometry
            .as_ref()
            .is_some_and(|geometry| geometry.strict_certified)
    });
    report.selected_strict_attempt = report
        .attempts
        .iter()
        .enumerate()
        .filter(|(_, attempt)| {
            attempt
                .geometry
                .as_ref()
                .is_some_and(|geometry| geometry.strict_certified)
        })
        .min_by_key(|(_, attempt)| attempt.topology.faces.unwrap_or(usize::MAX))
        .map(|(index, _)| index);
    report.retain_one_tested = report
        .attempts
        .iter()
        .any(|attempt| attempt.candidate.retained_parents.len() == 1);
    report.continuous_search_incomplete = !report.strict_certified
        && report.attempts.iter().any(|attempt| {
            attempt.status == RetainedCoreFamilyStatus::SearchIncomplete
                || attempt.geometry.as_ref().is_some_and(|geometry| {
                    geometry.status != RetainedCoreGeometryStatus::Certified
                })
        });
    Ok(report)
}

pub fn retained_core_ladder_report_json(report: &RetainedCoreLadderReport) -> String {
    let attempts = report
        .attempts
        .iter()
        .map(|attempt| {
            let geometry = attempt.geometry.as_ref().map_or_else(
                || "null".into(),
                |geometry| {
                    format!(
                        "{{\"status\":\"{:?}\",\"candidates_attempted\":{},\"best_angle_range_deg\":{},\"strict_certified\":{}}}",
                        geometry.status,
                        geometry.candidates_attempted,
                        angle_range_json(geometry.best_angle_range_deg),
                        geometry.strict_certified,
                    )
                },
            );
            format!(
                "{{\"retained_cardinality\":{},\"family\":\"{}\",\"status\":\"{}\",\"topology\":{},\"geometry\":{geometry}}}",
                attempt.candidate.retained_parents.len(),
                attempt.family.as_str(),
                attempt.status.as_str(),
                retained_core_topology_evidence_json(&attempt.topology),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"triggered\":{},\"attempted_cardinalities\":{:?},\"connected_candidates_attempted\":{},\"families_attempted\":{},\"strict_certified\":{},\"selected_strict_attempt\":{},\"best_angle_range_deg\":{},\"retain_one_tested\":{},\"continuous_search_incomplete\":{},\"attempts\":[{attempts}]}}",
        report.triggered,
        report.attempted_cardinalities,
        report.connected_candidates_attempted,
        report.families_attempted,
        report.strict_certified,
        report
            .selected_strict_attempt
            .map_or_else(|| "null".into(), |index| index.to_string()),
        angle_range_json(report.best_angle_range_deg),
        report.retain_one_tested,
        report.continuous_search_incomplete,
    )
}

pub(crate) enum FamilyProblemError {
    ExactNoSolution,
    Invalid(String),
}

pub(crate) fn retained_core_family_problem(
    source: &MotherGrid,
    component: &HierarchyComponent,
    family: RetainedCoreCorridorFamily,
) -> Result<FaceBandProblem, FamilyProblemError> {
    let source_face_rings = match family {
        RetainedCoreCorridorFamily::F0CurrentSourceFaceCorridor => 0,
        RetainedCoreCorridorFamily::F1PlusOneSourceFaceRing => 1,
        _ => 2,
    };
    let mut problem =
        build_face_band_problem_with_source_face_rings(source, component, 2, source_face_rings)
            .map_err(FamilyProblemError::Invalid)?;
    match family {
        RetainedCoreCorridorFamily::F3AnchorOnSingleInterface => {
            for policy in problem.anchor_policies.values_mut() {
                *policy = AnchorBandPolicy::OnSingleInterface;
            }
        }
        RetainedCoreCorridorFamily::F4FineCapRadiusOne => {
            force_fine_anchor_cap(source, &mut problem, 1)?;
        }
        RetainedCoreCorridorFamily::F5FineCapRadiusTwo => {
            force_fine_anchor_cap(source, &mut problem, 2)?;
        }
        _ => {}
    }
    Ok(problem)
}

fn force_fine_anchor_cap(
    source: &MotherGrid,
    problem: &mut FaceBandProblem,
    radius: usize,
) -> Result<(), FamilyProblemError> {
    let anchors = problem.anchor_policies.keys().copied().collect::<Vec<_>>();
    let transition = problem
        .transition_faces
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut cap = anchors
        .iter()
        .flat_map(|anchor| {
            problem
                .vertex_incident_faces
                .get(anchor)
                .into_iter()
                .flatten()
                .copied()
        })
        .collect::<BTreeSet<_>>();
    let mut frontier = cap.clone();
    for _ in 1..radius {
        let next = frontier
            .iter()
            .flat_map(|face| problem.face_adjacency.get(face).into_iter().flatten())
            .copied()
            .filter(|face| transition.contains(face) && !cap.contains(face))
            .collect::<BTreeSet<_>>();
        cap.extend(next.iter().copied());
        frontier = next;
    }
    if !cap.is_disjoint(&problem.coarse_boundary_faces) {
        return Err(FamilyProblemError::ExactNoSolution);
    }
    problem.fine_boundary_faces.extend(cap.iter().copied());
    problem
        .fine_boundary_vertices
        .extend(cap.iter().flat_map(|&face| source.mesh.triangles()[face]));
    for policy in problem.anchor_policies.values_mut() {
        *policy = AnchorBandPolicy::FineCapConnectedToExterior;
    }
    Ok(())
}

fn family_status(outcome: &RetainedCoreTopologyOutcome) -> RetainedCoreFamilyStatus {
    match outcome {
        RetainedCoreTopologyOutcome::Closed { .. } => RetainedCoreFamilyStatus::Closed,
        RetainedCoreTopologyOutcome::TopologyFamilyExhaustedNoSolution(_) => {
            RetainedCoreFamilyStatus::ExactNoSolution
        }
        RetainedCoreTopologyOutcome::SearchBudgetExhausted(_) => {
            RetainedCoreFamilyStatus::SearchIncomplete
        }
        RetainedCoreTopologyOutcome::InvalidInput { .. } => RetainedCoreFamilyStatus::InvalidInput,
    }
}

fn retained_core_geometry(
    source: &MotherGrid,
    component: &HierarchyComponent,
    plan: &FaceBandPlan,
    inherited_witness: &GeometryFailureWitness,
    source_levels: &[Option<usize>],
    starts: &[GeometryStartId],
    limits: RetainedCoreLadderLimits,
) -> RetainedCoreGeometryEvidence {
    let outcome = solve_full_polygon_merge_from_face_bands_with_geometry_witness(
        source,
        component,
        plan,
        inherited_witness,
        &BTreeSet::new(),
        FullPolygonCberLimits {
            topology_states: limits.geometry_topology_states,
            elastic_iterations: limits.geometry_iterations,
        },
        ElasticTargetMode::HierarchyEdgeAreaDegree,
        Some(source_levels),
        starts,
        GeometryDomainId::PlusTwoOrdinaryRings,
    );
    match outcome {
        FullPolygonMergeOutcome::Closed(trial) => RetainedCoreGeometryEvidence {
            status: RetainedCoreGeometryStatus::Certified,
            candidates_attempted: trial.evidence.geometry_candidates_attempted,
            best_angle_range_deg: mesh_angle_range(&trial.global_trial.mesh),
            strict_certified: true,
        },
        FullPolygonMergeOutcome::TopologyFamilyExhaustedNoSolution(evidence)
        | FullPolygonMergeOutcome::SearchBudgetExhausted(evidence) => {
            RetainedCoreGeometryEvidence {
                status: RetainedCoreGeometryStatus::ContinuousSearchIncomplete,
                candidates_attempted: evidence.geometry_candidates_attempted,
                best_angle_range_deg: evidence
                    .best_geometry_failure
                    .and_then(|failure| failure.global_angle_degrees),
                strict_certified: false,
            }
        }
        FullPolygonMergeOutcome::InvalidInput { evidence, .. } => RetainedCoreGeometryEvidence {
            status: RetainedCoreGeometryStatus::InvalidInput,
            candidates_attempted: evidence.geometry_candidates_attempted,
            best_angle_range_deg: evidence
                .best_geometry_failure
                .and_then(|failure| failure.global_angle_degrees),
            strict_certified: false,
        },
    }
}

fn empty_ladder_report(triggered: bool) -> RetainedCoreLadderReport {
    RetainedCoreLadderReport {
        triggered,
        attempted_cardinalities: Vec::new(),
        connected_candidates_attempted: 0,
        families_attempted: 0,
        attempts: Vec::new(),
        strict_certified: false,
        selected_strict_attempt: None,
        best_angle_range_deg: None,
        retain_one_tested: false,
        continuous_search_incomplete: false,
    }
}

fn update_best_range(best: &mut Option<(f64, f64)>, candidate: Option<(f64, f64)>) {
    let Some(candidate) = candidate else {
        return;
    };
    if best.is_none_or(|current| range_margin(candidate) > range_margin(current)) {
        *best = Some(candidate);
    }
}

fn range_margin((minimum, maximum): (f64, f64)) -> f64 {
    (minimum - 40.2).min(79.8 - maximum)
}

fn angle_range_json(range: Option<(f64, f64)>) -> String {
    range.map_or_else(
        || "null".into(),
        |(minimum, maximum)| format!("[{minimum:.12},{maximum:.12}]"),
    )
}

pub(crate) fn component_for_retained_core(
    original: &HierarchyComponent,
    candidate: &RetainedCoreCandidate,
) -> Result<HierarchyComponent, String> {
    if candidate.retained_parents.is_empty() {
        return Err("retained-core topology requires a non-empty retained set".into());
    }
    let initial = original
        .core_parents
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if candidate
        .retained_parents
        .union(&candidate.released_parents)
        .copied()
        .collect::<BTreeSet<_>>()
        != initial
        || !candidate
            .retained_parents
            .is_disjoint(&candidate.released_parents)
    {
        return Err("retained-core candidate does not partition the original core".into());
    }
    let retained = &candidate.retained_parents;
    Ok(HierarchyComponent {
        id: original.id,
        parents: original.parents.clone(),
        boundary_edges: original.boundary_edges.clone(),
        core_parents: retained.iter().copied().collect(),
        transition_parents: original
            .parents
            .iter()
            .copied()
            .filter(|parent| !retained.contains(parent))
            .collect(),
    })
}

fn evidence_for(
    candidate: &RetainedCoreCandidate,
    face_band_outcome: FaceBandOutcomeKind,
    face_band_states: u64,
    topology_outcome: RetainedCoreTopologyOutcomeKind,
    topology_states: usize,
    closed: Option<(usize, usize, usize, usize)>,
) -> RetainedCoreTopologyEvidence {
    let (selected_topologies, vertices, edges, faces) = closed
        .map(|(selected, vertices, edges, faces)| {
            (selected, Some(vertices), Some(edges), Some(faces))
        })
        .unwrap_or((0, None, None, None));
    RetainedCoreTopologyEvidence {
        retained_parents: candidate.retained_parents.clone(),
        released_parents: candidate.released_parents.clone(),
        face_band_outcome,
        face_band_states,
        topology_outcome,
        topology_states,
        selected_topologies,
        vertices,
        edges,
        faces,
    }
}

fn invalid_topology(
    candidate: &RetainedCoreCandidate,
    reason: String,
    face_band_outcome: FaceBandOutcomeKind,
    face_band_states: u64,
    topology_states: usize,
) -> RetainedCoreTopologyOutcome {
    RetainedCoreTopologyOutcome::InvalidInput {
        reason,
        evidence: evidence_for(
            candidate,
            face_band_outcome,
            face_band_states,
            RetainedCoreTopologyOutcomeKind::InvalidInput,
            topology_states,
            None,
        ),
    }
}

fn graph_distances(
    graph: &BTreeMap<TriangleAddress, BTreeSet<TriangleAddress>>,
    seeds: &BTreeSet<TriangleAddress>,
) -> BTreeMap<TriangleAddress, usize> {
    let mut distances = BTreeMap::new();
    let mut queue = VecDeque::new();
    for &seed in seeds {
        distances.insert(seed, 0);
        queue.push_back(seed);
    }
    while let Some(parent) = queue.pop_front() {
        let next_distance = distances[&parent] + 1;
        for &neighbour in &graph[&parent] {
            if let std::collections::btree_map::Entry::Vacant(entry) = distances.entry(neighbour) {
                entry.insert(next_distance);
                queue.push_back(neighbour);
            }
        }
    }
    distances
}

fn component_count(
    retained: &BTreeSet<TriangleAddress>,
    adjacency: &BTreeMap<TriangleAddress, BTreeSet<TriangleAddress>>,
) -> usize {
    let mut unseen = retained.clone();
    let mut components = 0;
    while let Some(seed) = unseen.pop_first() {
        components += 1;
        let mut queue = VecDeque::from([seed]);
        while let Some(parent) = queue.pop_front() {
            for &neighbour in &adjacency[&parent] {
                if unseen.remove(&neighbour) {
                    queue.push_back(neighbour);
                }
            }
        }
    }
    components
}

pub fn retained_core_search_plan_json(plan: &RetainedCoreSearchPlan) -> String {
    let candidates = plan
        .candidates
        .iter()
        .map(|candidate| {
            format!(
                "{{\"retained_parents\":{},\"released_parents\":{},\"retained_components\":{},\"retained_boundary_edges\":{},\"violation_influence_score\":{:.12}}}",
                address_set_json(&candidate.retained_parents),
                address_set_json(&candidate.released_parents),
                candidate.retained_components,
                candidate.retained_boundary_edges,
                candidate.violation_influence_score,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"initial_coarse_parents\":{},\"candidate_count\":{},\"connected_candidate_count\":{},\"candidates\":[{}]}}",
        address_set_json(&plan.initial_coarse_parents),
        plan.candidates.len(),
        plan.connected_candidates().count(),
        candidates,
    )
}

pub fn retained_core_topology_evidence_json(evidence: &RetainedCoreTopologyEvidence) -> String {
    format!(
        "{{\"retained_parents\":{},\"released_parents\":{},\"face_band_outcome\":\"{:?}\",\"face_band_states\":{},\"topology_outcome\":\"{:?}\",\"topology_states\":{},\"selected_topologies\":{},\"vertices\":{},\"edges\":{},\"faces\":{}}}",
        address_set_json(&evidence.retained_parents),
        address_set_json(&evidence.released_parents),
        evidence.face_band_outcome,
        evidence.face_band_states,
        evidence.topology_outcome,
        evidence.topology_states,
        evidence.selected_topologies,
        option_usize(evidence.vertices),
        option_usize(evidence.edges),
        option_usize(evidence.faces),
    )
}

fn option_usize(value: Option<usize>) -> String {
    value.map_or_else(|| "null".into(), |value| value.to_string())
}

fn address_set_json(values: &BTreeSet<TriangleAddress>) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|address| format!(
                "{{\"base_face\":{},\"i\":{},\"j\":{},\"n\":{},\"orientation\":\"{:?}\"}}",
                address.base_face, address.i, address.j, address.n, address.orientation,
            ))
            .collect::<Vec<_>>()
            .join(",")
    )
}
