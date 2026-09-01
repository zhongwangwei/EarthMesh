//! Exact identity and bounded profiling for legacy face-band problems.

use super::{
    build_face_band_problem_with_source_face_rings, n6_legacy_mixed_fixture,
    plan_retained_core_subsets, remaining_connected_retained_core_candidates,
    retained_core::{
        component_for_retained_core, retained_core_family_problem, FamilyProblemError,
    },
    solve_exact_face_bands, AnchorBandPolicy, FaceBandLimits, FaceBandOutcomeKind, FaceBandProblem,
    FaceBandSolveOutcome, RetainedCoreCandidate, RetainedCoreCorridorFamily,
};
use crate::{MotherGrid, TriangleAddress, VertexAddress};
use std::collections::{BTreeMap, BTreeSet};

pub const ESSENTIAL_CYCLE_CONTRACT_VERSION: u32 = 1;
pub const DOWNSTREAM_FULL_POLYGON_CONTRACT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CanonicalFaceId {
    pub address: TriangleAddress,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum CanonicalVertexId {
    Address(VertexAddress),
    FrozenSourceSlot { source_n: usize, slot: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CanonicalEdgeId {
    pub vertices: [CanonicalVertexId; 2],
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EssentialCycleProblemKey {
    pub contract_version: u32,
    pub downstream_contract_version: u32,
    pub source_n: usize,
    pub band_count: u8,
    pub source_face_rings: usize,
    pub transition_faces: Vec<TriangleAddress>,
    pub coarse_boundary_faces: Vec<TriangleAddress>,
    pub fine_boundary_faces: Vec<TriangleAddress>,
    pub face_adjacency_edges: Vec<(CanonicalFaceId, CanonicalFaceId)>,
    pub candidate_edges: Vec<CanonicalEdgeId>,
    pub candidate_edge_incident_faces: Vec<(CanonicalEdgeId, [CanonicalFaceId; 2])>,
    pub anchor_policies: Vec<(CanonicalVertexId, AnchorBandPolicy)>,
    pub retained_parents: Vec<TriangleAddress>,
    pub corridor_family: RetainedCoreCorridorFamily,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FaceBandProblemIdentityReport {
    pub attempts: usize,
    pub canonicalized_attempts: usize,
    pub unique_exact_problem_keys: usize,
    pub unique_address_graphs: usize,
    pub repeated_address_graph_attempts: usize,
    pub legacy_fingerprint_buckets: usize,
    pub legacy_fingerprint_ambiguous_buckets: usize,
    pub transition_face_count_histogram: BTreeMap<usize, usize>,
    pub frozen_pr84_unknown_transition_face_count_histogram: BTreeMap<usize, usize>,
    pub raw_states_by_transition_face_count: BTreeMap<usize, u64>,
    pub corridor_attempts: BTreeMap<RetainedCoreCorridorFamily, usize>,
    pub corridor_unique_address_graphs: BTreeMap<RetainedCoreCorridorFamily, usize>,
    pub outcome_histogram: BTreeMap<String, usize>,
    pub frozen_pr84_unknown_attempts: usize,
    pub total_raw_states: u64,
    pub total_propagation_rounds: u64,
    pub total_pruned_domains: u64,
    pub total_clone_checkpoints: u64,
    pub total_clone_payload_bytes: u64,
    pub clone_payload_bytes_per_checkpoint: f64,
    pub total_leaf_validations: u64,
    pub total_leaf_rejections: u64,
    pub leaf_validation_fraction: f64,
    pub total_propagation_rejections: u64,
    pub maximum_states_per_problem: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct AddressGraphKey {
    source_n: usize,
    band_count: u8,
    source_face_rings: usize,
    transition_faces: Vec<TriangleAddress>,
    coarse_boundary_faces: Vec<TriangleAddress>,
    fine_boundary_faces: Vec<TriangleAddress>,
    face_adjacency_edges: Vec<(CanonicalFaceId, CanonicalFaceId)>,
    candidate_edge_incident_faces: Vec<(CanonicalEdgeId, [CanonicalFaceId; 2])>,
}

impl From<&EssentialCycleProblemKey> for AddressGraphKey {
    fn from(key: &EssentialCycleProblemKey) -> Self {
        Self {
            source_n: key.source_n,
            band_count: key.band_count,
            source_face_rings: key.source_face_rings,
            transition_faces: key.transition_faces.clone(),
            coarse_boundary_faces: key.coarse_boundary_faces.clone(),
            fine_boundary_faces: key.fine_boundary_faces.clone(),
            face_adjacency_edges: key.face_adjacency_edges.clone(),
            candidate_edge_incident_faces: key.candidate_edge_incident_faces.clone(),
        }
    }
}

pub fn essential_cycle_problem_key(
    source: &MotherGrid,
    problem: &FaceBandProblem,
    retained_parents: impl IntoIterator<Item = TriangleAddress>,
    corridor_family: RetainedCoreCorridorFamily,
) -> Result<EssentialCycleProblemKey, String> {
    let face = |slot: usize| -> Result<CanonicalFaceId, String> {
        problem
            .face_addresses
            .get(&slot)
            .copied()
            .map(|address| CanonicalFaceId { address })
            .ok_or_else(|| format!("face-band slot {slot} has no canonical address"))
    };
    let mut transition_faces = problem
        .transition_faces
        .iter()
        .map(|slot| face(*slot).map(|id| id.address))
        .collect::<Result<Vec<_>, _>>()?;
    transition_faces.sort_unstable();
    transition_faces.dedup();
    if transition_faces.len() != problem.transition_faces.len() {
        return Err("canonical transition face addresses are not unique".into());
    }
    let sorted_faces = |slots: &BTreeSet<usize>| -> Result<Vec<TriangleAddress>, String> {
        let mut values = slots
            .iter()
            .map(|slot| face(*slot).map(|id| id.address))
            .collect::<Result<Vec<_>, _>>()?;
        values.sort_unstable();
        values.dedup();
        Ok(values)
    };

    let mut face_adjacency_edges = BTreeSet::new();
    for (&left, neighbours) in &problem.face_adjacency {
        for &right in neighbours {
            face_adjacency_edges.insert(canonical_face_pair(face(left)?, face(right)?));
        }
    }

    let blocked_anchor_vertices = problem
        .anchor_policies
        .iter()
        .filter_map(|(&slot, policy)| {
            (*policy != AnchorBandPolicy::OnSingleInterface).then_some(slot)
        })
        .collect::<BTreeSet<_>>();
    let mut candidate_edge_incident_faces = BTreeSet::new();
    for (&(left, right), &(a, b)) in &problem.face_shared_edges {
        if problem.coarse_boundary_vertices.contains(&a)
            || problem.coarse_boundary_vertices.contains(&b)
            || problem.fine_boundary_vertices.contains(&a)
            || problem.fine_boundary_vertices.contains(&b)
            || blocked_anchor_vertices.contains(&a)
            || blocked_anchor_vertices.contains(&b)
        {
            continue;
        }
        let edge = canonical_edge(canonical_vertex(source, a), canonical_vertex(source, b));
        let mut incident = [face(left)?, face(right)?];
        incident.sort_unstable();
        candidate_edge_incident_faces.insert((edge, incident));
    }
    let candidate_edge_incident_faces = candidate_edge_incident_faces
        .into_iter()
        .collect::<Vec<_>>();
    let candidate_edges = candidate_edge_incident_faces
        .iter()
        .map(|(edge, _)| edge.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut anchor_policies = problem
        .anchor_policies
        .iter()
        .map(|(&slot, &policy)| (canonical_vertex(source, slot), policy))
        .collect::<Vec<_>>();
    anchor_policies.sort_unstable();
    let mut retained_parents = retained_parents.into_iter().collect::<Vec<_>>();
    retained_parents.sort_unstable();
    retained_parents.dedup();

    Ok(EssentialCycleProblemKey {
        contract_version: ESSENTIAL_CYCLE_CONTRACT_VERSION,
        downstream_contract_version: DOWNSTREAM_FULL_POLYGON_CONTRACT_VERSION,
        source_n: source.subdivision,
        band_count: problem.band_count as u8,
        source_face_rings: problem.source_face_rings,
        transition_faces,
        coarse_boundary_faces: sorted_faces(&problem.coarse_boundary_faces)?,
        fine_boundary_faces: sorted_faces(&problem.fine_boundary_faces)?,
        face_adjacency_edges: face_adjacency_edges.into_iter().collect(),
        candidate_edges,
        candidate_edge_incident_faces,
        anchor_policies,
        retained_parents,
        corridor_family,
    })
}

pub fn profile_frozen_n6_face_band_problems(
    maximum_states_per_problem: u64,
) -> Result<FaceBandProblemIdentityReport, String> {
    let (source, component) = n6_legacy_mixed_fixture()?;
    let initial_core = component
        .core_parents
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let plan = plan_retained_core_subsets(&source, &initial_core, &initial_core)?;
    let candidates = remaining_connected_retained_core_candidates(&plan);
    let attempts = candidates.len() * RetainedCoreCorridorFamily::ALL.len();
    let mut exact_keys = BTreeSet::new();
    let mut graph_keys = BTreeSet::new();
    let mut corridor_graphs =
        BTreeMap::<RetainedCoreCorridorFamily, BTreeSet<AddressGraphKey>>::new();
    let mut fingerprints = BTreeMap::<u64, BTreeSet<EssentialCycleProblemKey>>::new();
    let mut report = FaceBandProblemIdentityReport {
        attempts,
        canonicalized_attempts: 0,
        unique_exact_problem_keys: 0,
        unique_address_graphs: 0,
        repeated_address_graph_attempts: 0,
        legacy_fingerprint_buckets: 0,
        legacy_fingerprint_ambiguous_buckets: 0,
        transition_face_count_histogram: BTreeMap::new(),
        frozen_pr84_unknown_transition_face_count_histogram: BTreeMap::new(),
        raw_states_by_transition_face_count: BTreeMap::new(),
        corridor_attempts: BTreeMap::new(),
        corridor_unique_address_graphs: BTreeMap::new(),
        outcome_histogram: BTreeMap::new(),
        frozen_pr84_unknown_attempts: 0,
        total_raw_states: 0,
        total_propagation_rounds: 0,
        total_pruned_domains: 0,
        total_clone_checkpoints: 0,
        total_clone_payload_bytes: 0,
        clone_payload_bytes_per_checkpoint: 0.0,
        total_leaf_validations: 0,
        total_leaf_rejections: 0,
        leaf_validation_fraction: 0.0,
        total_propagation_rejections: 0,
        maximum_states_per_problem,
    };

    for candidate in candidates {
        for family in RetainedCoreCorridorFamily::ALL {
            *report.corridor_attempts.entry(family).or_default() += 1;
            let (problem, precheck_exact_no_solution) =
                profile_problem(&source, &component, candidate, family)?;
            let key = essential_cycle_problem_key(
                &source,
                &problem,
                candidate.retained_parents.iter().copied(),
                family,
            )?;
            exact_keys.insert(key.clone());
            let graph_key = AddressGraphKey::from(&key);
            graph_keys.insert(graph_key.clone());
            corridor_graphs.entry(family).or_default().insert(graph_key);
            report.canonicalized_attempts += 1;
            *report
                .transition_face_count_histogram
                .entry(problem.transition_faces.len())
                .or_default() += 1;

            if precheck_exact_no_solution {
                *report
                    .outcome_histogram
                    .entry("PrecheckExactNoSolution".into())
                    .or_default() += 1;
                continue;
            }
            let evidence = match solve_exact_face_bands(
                &problem,
                FaceBandLimits {
                    maximum_states: maximum_states_per_problem,
                },
            ) {
                FaceBandSolveOutcome::Closed(_, evidence)
                | FaceBandSolveOutcome::FamilyExhaustedNoSolution { evidence, .. }
                | FaceBandSolveOutcome::SearchBudgetExhausted { evidence, .. } => evidence,
                FaceBandSolveOutcome::InvalidInput { reason } => {
                    return Err(format!("canonicalized legacy problem is invalid: {reason}"));
                }
            };
            let stage = rejection_stage(&evidence);
            *report.outcome_histogram.entry(stage.into()).or_default() += 1;
            *report
                .raw_states_by_transition_face_count
                .entry(problem.transition_faces.len())
                .or_default() += evidence.states_examined;
            if matches!(
                evidence.outcome,
                FaceBandOutcomeKind::Closed | FaceBandOutcomeKind::SearchBudgetExhausted
            ) {
                report.frozen_pr84_unknown_attempts += 1;
                *report
                    .frozen_pr84_unknown_transition_face_count_histogram
                    .entry(problem.transition_faces.len())
                    .or_default() += 1;
            }
            report.total_raw_states += evidence.states_examined;
            report.total_propagation_rounds += evidence.propagation_rounds as u64;
            report.total_pruned_domains += evidence.pruned_domains as u64;
            report.total_clone_checkpoints += evidence.full_domain_clone_checkpoints;
            report.total_clone_payload_bytes += evidence.domain_clone_payload_bytes;
            report.total_leaf_validations += evidence.leaf_validations;
            report.total_leaf_rejections += evidence.leaf_rejections;
            report.total_propagation_rejections += evidence.propagation_rejections;
            fingerprints
                .entry(evidence.face_complex_fingerprint)
                .or_default()
                .insert(key);
        }
    }
    report.unique_exact_problem_keys = exact_keys.len();
    report.unique_address_graphs = graph_keys.len();
    report.repeated_address_graph_attempts = attempts.saturating_sub(graph_keys.len());
    report.corridor_unique_address_graphs = corridor_graphs
        .into_iter()
        .map(|(family, keys)| (family, keys.len()))
        .collect();
    report.legacy_fingerprint_buckets = fingerprints.len();
    report.legacy_fingerprint_ambiguous_buckets =
        fingerprints.values().filter(|keys| keys.len() > 1).count();
    report.clone_payload_bytes_per_checkpoint = ratio(
        report.total_clone_payload_bytes,
        report.total_clone_checkpoints,
    );
    report.leaf_validation_fraction = ratio(report.total_leaf_validations, report.total_raw_states);
    Ok(report)
}

pub fn face_band_problem_identity_report_json(report: &FaceBandProblemIdentityReport) -> String {
    format!(
        "{{\"schema_version\":1,\"attempts\":{},\"canonicalized_attempts\":{},\"unique_exact_problem_keys\":{},\"unique_address_graphs\":{},\"repeated_address_graph_attempts\":{},\"legacy_fingerprint_buckets\":{},\"legacy_fingerprint_ambiguous_buckets\":{},\"transition_face_count_histogram\":{},\"frozen_pr84_unknown_transition_face_count_histogram\":{},\"raw_states_by_transition_face_count\":{},\"corridor_attempts\":{},\"corridor_unique_address_graphs\":{},\"outcome_histogram\":{},\"frozen_pr84_unknown_attempts\":{},\"total_raw_states\":{},\"total_propagation_rounds\":{},\"total_pruned_domains\":{},\"total_clone_checkpoints\":{},\"total_clone_payload_bytes\":{},\"clone_payload_bytes_per_checkpoint\":{:.12},\"total_leaf_validations\":{},\"total_leaf_rejections\":{},\"leaf_validation_fraction\":{:.12},\"total_propagation_rejections\":{},\"maximum_states_per_problem\":{}}}",
        report.attempts,
        report.canonicalized_attempts,
        report.unique_exact_problem_keys,
        report.unique_address_graphs,
        report.repeated_address_graph_attempts,
        report.legacy_fingerprint_buckets,
        report.legacy_fingerprint_ambiguous_buckets,
        usize_map_json(&report.transition_face_count_histogram),
        usize_map_json(&report.frozen_pr84_unknown_transition_face_count_histogram),
        usize_u64_map_json(&report.raw_states_by_transition_face_count),
        family_map_json(&report.corridor_attempts),
        family_map_json(&report.corridor_unique_address_graphs),
        string_map_json(&report.outcome_histogram),
        report.frozen_pr84_unknown_attempts,
        report.total_raw_states,
        report.total_propagation_rounds,
        report.total_pruned_domains,
        report.total_clone_checkpoints,
        report.total_clone_payload_bytes,
        report.clone_payload_bytes_per_checkpoint,
        report.total_leaf_validations,
        report.total_leaf_rejections,
        report.leaf_validation_fraction,
        report.total_propagation_rejections,
        report.maximum_states_per_problem,
    )
}

fn profile_problem(
    source: &MotherGrid,
    original: &super::HierarchyComponent,
    candidate: &RetainedCoreCandidate,
    family: RetainedCoreCorridorFamily,
) -> Result<(FaceBandProblem, bool), String> {
    let component = component_for_retained_core(original, candidate)?;
    match retained_core_family_problem(source, &component, family) {
        Ok(problem) => Ok((problem, false)),
        Err(FamilyProblemError::Invalid(reason)) => Err(reason),
        Err(FamilyProblemError::ExactNoSolution) => {
            let source_face_rings = match family {
                RetainedCoreCorridorFamily::F0CurrentSourceFaceCorridor => 0,
                RetainedCoreCorridorFamily::F1PlusOneSourceFaceRing => 1,
                _ => 2,
            };
            let mut problem = build_face_band_problem_with_source_face_rings(
                source,
                &component,
                2,
                source_face_rings,
            )?;
            if matches!(
                family,
                RetainedCoreCorridorFamily::F4FineCapRadiusOne
                    | RetainedCoreCorridorFamily::F5FineCapRadiusTwo
            ) {
                for policy in problem.anchor_policies.values_mut() {
                    *policy = AnchorBandPolicy::FineCapConnectedToExterior;
                }
            }
            Ok((problem, true))
        }
    }
}

pub(crate) fn canonical_vertex(source: &MotherGrid, slot: usize) -> CanonicalVertexId {
    source
        .addresses
        .get(slot)
        .and_then(Clone::clone)
        .map(CanonicalVertexId::Address)
        .unwrap_or(CanonicalVertexId::FrozenSourceSlot {
            source_n: source.subdivision,
            slot,
        })
}

pub(crate) fn canonical_edge(left: CanonicalVertexId, right: CanonicalVertexId) -> CanonicalEdgeId {
    CanonicalEdgeId {
        vertices: if left <= right {
            [left, right]
        } else {
            [right, left]
        },
    }
}

fn canonical_face_pair(
    left: CanonicalFaceId,
    right: CanonicalFaceId,
) -> (CanonicalFaceId, CanonicalFaceId) {
    if left <= right {
        (left, right)
    } else {
        (right, left)
    }
}

fn rejection_stage(evidence: &super::FaceBandEvidence) -> &'static str {
    match evidence.outcome {
        FaceBandOutcomeKind::Closed => "Closed",
        FaceBandOutcomeKind::SearchBudgetExhausted => "SearchBudgetExhausted",
        FaceBandOutcomeKind::InvalidInput => "InvalidInput",
        FaceBandOutcomeKind::FamilyExhaustedNoSolution if evidence.leaf_validations > 0 => {
            "LeafValidationRejected"
        }
        FaceBandOutcomeKind::FamilyExhaustedNoSolution => "PropagationRejected",
    }
}

fn ratio(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn usize_map_json(values: &BTreeMap<usize, usize>) -> String {
    format!(
        "{{{}}}",
        values
            .iter()
            .map(|(key, value)| format!("\"{key}\":{value}"))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn family_map_json(values: &BTreeMap<RetainedCoreCorridorFamily, usize>) -> String {
    format!(
        "{{{}}}",
        values
            .iter()
            .map(|(key, value)| format!("\"{}\":{value}", key.as_str()))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn usize_u64_map_json(values: &BTreeMap<usize, u64>) -> String {
    format!(
        "{{{}}}",
        values
            .iter()
            .map(|(key, value)| format!("\"{key}\":{value}"))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn string_map_json(values: &BTreeMap<String, usize>) -> String {
    format!(
        "{{{}}}",
        values
            .iter()
            .map(|(key, value)| format!("\"{key}\":{value}"))
            .collect::<Vec<_>>()
            .join(",")
    )
}
