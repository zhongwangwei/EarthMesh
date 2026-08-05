use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    io,
};

use serde::{Deserialize, Serialize};

use super::*;

const METHOD_C_LOCAL_LEGALIZATION_MAX_STEPS: usize = 64;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MethodCHfieldPassDiagnostics {
    pub pass: usize,
    pub preserve_all_demands: bool,
    pub parent_interior_m_points: usize,
    pub hard_demand_m_points: usize,
    pub hard_demand_anchors: usize,
    pub phase_support_m_points: usize,
    pub component_count: usize,
    pub component_phases: Vec<MethodCHfieldComponentPhaseDiagnostics>,
    pub legal_rad3_seeds: usize,
    pub initial_selected_seeds: usize,
    pub initial_seed_footprint_faces: usize,
    pub demand_tail_seeds: usize,
    pub demand_tail_faces: usize,
    pub connectivity_bridge_seeds: usize,
    pub connectivity_bridge_faces: usize,
    pub face_reason_mask_counts: [usize; 8],
    pub alignable_faces: usize,
    pub final_selected_faces: usize,
    pub unexplained_selected_faces: usize,
    pub selected_seed_ids: Vec<usize>,
    pub(crate) legal_seed_ids: Vec<usize>,
    pub seed_union_vertex_only_contacts: usize,
    pub seed_union_first_contact_m_point: Option<usize>,
    pub seed_reconstruction_matches: bool,
    pub seed_reconstruction_error: Option<String>,
    pub candidate_validation: Option<MethodCHfieldCandidateValidation>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodCHfieldComponentPhaseDiagnostics {
    pub component_index: usize,
    pub component_m_points: Vec<usize>,
    pub demand_start: usize,
    pub phase_class_count: usize,
    pub phase_starts: Vec<usize>,
    pub selected_phase_ordinal: usize,
    pub selected_start: usize,
    #[serde(default)]
    pub legal_seed_ids: Vec<usize>,
    #[serde(default)]
    pub selected_seed_ids: Vec<usize>,
}

/// Deterministic, topology-only inputs and selection produced immediately
/// before one spherical HField Method-C pass is materialized.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodCHfieldSelectionCheckpoint {
    pub pass: usize,
    pub preserve_all_demands: bool,
    pub m_target_levels: Vec<usize>,
    pub u_target_levels: Vec<usize>,
    pub face_demand: Vec<bool>,
    pub demand_anchors: Vec<MethodCHfieldDemandAnchorCheckpoint>,
    pub selected_faces: Vec<bool>,
    pub legal_seed_ids: Vec<usize>,
    pub selected_seed_ids: Vec<usize>,
    #[serde(default)]
    pub component_phases: Vec<MethodCHfieldComponentPhaseDiagnostics>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodCHfieldDemandAnchorCheckpoint {
    pub parent_m_point: usize,
    pub parent_m_lineage: usize,
    pub candidate_faces: Vec<usize>,
    pub candidate_face_lineages: Vec<usize>,
}

#[derive(Debug)]
struct MethodCHfieldDemandCoverageError {
    parent_m_point: usize,
}

impl std::fmt::Display for MethodCHfieldDemandCoverageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Method-C h-field aligned demand anchor at M point {} is not covered by the refinement mask",
            self.parent_m_point
        )
    }
}

impl std::error::Error for MethodCHfieldDemandCoverageError {}

#[derive(Debug)]
struct MethodCHfieldPerimeterTopologyError(String);

impl std::fmt::Display for MethodCHfieldPerimeterTopologyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for MethodCHfieldPerimeterTopologyError {}

fn hfield_demand_coverage_error(parent_m_point: usize) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        MethodCHfieldDemandCoverageError { parent_m_point },
    )
}

#[derive(Clone, Copy)]
struct M0HfieldPhaseAnchor {
    component: CartesianPoint,
    phase: CartesianPoint,
}

fn m0_hfield_phase_anchor(pass: usize) -> io::Result<Option<M0HfieldPhaseAnchor>> {
    let Ok(value) = std::env::var("EARTHMESH_M0_HFIELD_PHASE_VARIANT") else {
        return Ok(None);
    };
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() != 7 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "EARTHMESH_M0_HFIELD_PHASE_VARIANT must be PASS:COMPONENT_X:COMPONENT_Y:COMPONENT_Z:PHASE_X:PHASE_Y:PHASE_Z",
        ));
    }
    let requested_pass = parts[0].parse::<usize>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "EARTHMESH_M0_HFIELD_PHASE_VARIANT pass must be an unsigned integer",
        )
    })?;
    let coordinates = parts[1..]
        .iter()
        .map(|value| value.parse::<f64>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "EARTHMESH_M0_HFIELD_PHASE_VARIANT coordinates must be finite numbers",
            )
        })?;
    if coordinates.iter().any(|value| !value.is_finite()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "EARTHMESH_M0_HFIELD_PHASE_VARIANT coordinates must be finite numbers",
        ));
    }
    Ok((requested_pass == pass).then_some(M0HfieldPhaseAnchor {
        component: CartesianPoint::new(coordinates[0], coordinates[1], coordinates[2]),
        phase: CartesianPoint::new(coordinates[3], coordinates[4], coordinates[5]),
    }))
}

impl MethodCHfieldSelectionCheckpoint {
    pub fn validate_demand_coverage(&self, selected_faces: &[bool]) -> io::Result<()> {
        for anchor in &self.demand_anchors {
            if !anchor
                .candidate_faces
                .iter()
                .any(|&iw| selected_faces.get(iw).copied().unwrap_or(false))
            {
                return Err(hfield_demand_coverage_error(anchor.parent_m_point));
            }
        }
        Ok(())
    }
}

impl MethodCHfieldLegalizationPreflight {
    /// Return every canonical seed currently known to affect the requested
    /// complete perimeter components.
    ///
    /// Older checkpoints predate this census and return `None`; callers must
    /// not infer a closed component scope from missing data.
    pub fn current_perimeter_candidate_scope(
        &self,
        perimeter_components: &[usize],
    ) -> io::Result<Option<Vec<usize>>> {
        if perimeter_components.is_empty() {
            return Ok(None);
        }
        if perimeter_components
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C perimeter component scope must be strictly increasing",
            ));
        }
        if self.perimeter_candidate_seed_ids.is_empty() && !self.perimeter_lengths.is_empty() {
            return Ok(None);
        }
        if self.perimeter_candidate_seed_ids.len() != self.perimeter_lengths.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Method-C perimeter candidate census does not match perimeter component count",
            ));
        }
        let mut candidates = BTreeSet::new();
        for &component in perimeter_components {
            let component_candidates = self
                .perimeter_candidate_seed_ids
                .get(component)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Method-C perimeter component is outside the current preflight",
                    )
                })?;
            candidates.extend(component_candidates.iter().copied());
        }
        let candidates = candidates.into_iter().collect::<Vec<_>>();
        Ok((!candidates.is_empty()).then_some(candidates))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodCTransitionSelfLoopCheckpointWitness {
    pub triple_index: usize,
    pub perimeter_component: usize,
    pub component_triple_index: usize,
    pub parent_u_edge: usize,
    pub parent_u_m_points: [usize; 2],
    pub parent_u_m_lineages: [usize; 2],
    pub dependency_faces: Vec<usize>,
    pub dependency_face_lineages: Vec<usize>,
    #[serde(default)]
    pub candidate_seed_ids: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodCHfieldLegalizationPreflight {
    pub prepared_selected_faces: Vec<bool>,
    pub perimeter_lengths: Vec<usize>,
    pub perimeter_remainders: Vec<usize>,
    #[serde(default)]
    pub perimeter_candidate_seed_ids: Vec<Vec<usize>>,
    pub self_loop_witnesses: Vec<MethodCTransitionSelfLoopCheckpointWitness>,
    pub witness_dependency_clusters: Vec<Vec<usize>>,
    pub patches: Vec<MethodCHfieldLegalizationPatch>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodCHfieldLegalizationPatch {
    pub cluster_index: usize,
    pub witness_indices: Vec<usize>,
    pub witness_perimeter_components: Vec<usize>,
    pub perimeter_components: Vec<usize>,
    pub perimeter_interfaces: Vec<MethodCHfieldPerimeterComponentCheckpoint>,
    pub dependency_faces: Vec<usize>,
    pub dependency_face_lineages: Vec<usize>,
    pub candidate_seed_ids: Vec<usize>,
    pub candidate_seed_lineages: Vec<usize>,
    pub selected_candidate_seed_ids: Vec<usize>,
    pub mutable_faces: Vec<usize>,
    pub mutable_face_lineages: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MethodCHfieldPerimeterComponentCheckpoint {
    pub component_index: usize,
    pub points: Vec<MethodCHfieldPerimeterPointCheckpoint>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MethodCHfieldPerimeterPointCheckpoint {
    pub parent_m_point: usize,
    pub parent_m_lineage: usize,
    pub parent_u_edge: usize,
    pub parent_u_m_lineages: [usize; 2],
    pub npoly: usize,
    pub nwdiv: usize,
    pub near_pentagon: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MethodCHfieldLegalizationPatchBoundaryCheck {
    pub outside_changed_faces: Vec<usize>,
    pub outside_perimeter_interface_changed: bool,
    pub selected_face_ids: Vec<usize>,
    pub ordered_perimeter_components: Vec<MethodCHfieldPerimeterComponentCheckpoint>,
    pub perimeter_lengths: Vec<usize>,
    pub vertex_only_contact_count: usize,
    pub predicted_transition_self_loop_count: usize,
    pub exact_materializable: bool,
    pub exact_failure_kind: Option<MethodCHfieldFailureKind>,
    pub exact_failure_message: Option<String>,
    pub exact_failure_dependency_faces: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodCHfieldLegalizationSymbolicCheck {
    pub selected_face_count: usize,
    pub perimeter_lengths: Vec<usize>,
    pub perimeter_remainders: Vec<usize>,
    pub vertex_only_contact_count: usize,
    pub predicted_transition_self_loop_count: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MethodCHfieldExactPatchTableStatus {
    Sat,
    PatchUnsat,
    Incomplete,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MethodCHfieldExactPatchTableCompilation {
    pub status: MethodCHfieldExactPatchTableStatus,
    pub candidate_seed_ids: Vec<usize>,
    pub demand_anchor_count: usize,
    pub fixed_direct_covered_demand_anchors: usize,
    pub fixed_closed_covered_demand_anchors: usize,
    pub maximal_direct_covered_demand_anchors: usize,
    pub maximal_closed_covered_demand_anchors: usize,
    pub fixed_uncovered_demand_anchors: usize,
    pub direct_unsupported_demand_anchors: usize,
    pub distinct_direct_candidate_support_scope_count: usize,
    pub min_direct_candidate_support_count: usize,
    pub max_direct_candidate_support_count: usize,
    pub direct_coverage_clause_satisfying_assignments: Option<usize>,
    pub total_assignments: Option<usize>,
    pub evaluated_assignments: usize,
    pub sat_assignments: usize,
    pub boundary_incomplete_assignments: usize,
    pub hard_rejected_assignments: BTreeMap<String, usize>,
    pub exact_failure_assignments: BTreeMap<String, usize>,
    pub unclassified_error_assignments: usize,
    pub first_unclassified_error: Option<String>,
    pub triplet_assignment_count: usize,
    pub distinct_exact_state_count: usize,
    pub max_exact_state_multiplicity: usize,
    pub mixed_exact_outcome_state_count: usize,
    pub current_perimeter_scope_candidate_seed_ids: Option<Vec<usize>>,
    pub covers_current_perimeter_scope: bool,
    pub ordered_perimeter_scope_analyses: Vec<MethodCHfieldOrderedPerimeterScopeAnalysis>,
    pub table: Option<MethodCBinaryTableConstraint>,
    pub propagation: Option<MethodCBinaryTablePropagation>,
    pub system_analysis: Option<MethodCBinaryTableSystemAnalysis>,
    /// Per-assignment outcomes, recorded only when
    /// `EARTHMESH_M0_LEGALIZATION_ASSIGNMENT_DUMP` is set. Empty otherwise, so
    /// the default path keeps its previous allocation and output behaviour.
    pub assignment_outcome_records: Vec<MethodCHfieldAssignmentOutcomeRecord>,
}

/// One enumerated assignment and how the exact oracle classified it.
///
/// This exists so a Myhill-Nerode style state minimisation can be run offline:
/// the aggregate `distinct_exact_state_count` reports how many full state keys
/// occur, but not which assignments share behaviour under every completion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MethodCHfieldAssignmentOutcomeRecord {
    /// Bit `i` selects `patch.candidate_seed_ids[i]`.
    pub value_mask: usize,
    /// `sat`, `boundary_incomplete`, `exact:<kind>`, `hard:<kind>` or
    /// `unclassified`.
    pub outcome: String,
    /// Deterministic ordinal of this assignment's full
    /// `(selected faces, ordered perimeter components)` key, or `None` when the
    /// assignment was rejected before any state key could be formed.
    pub exact_state_ordinal: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MethodCHfieldOrderedPerimeterScopeAnalysis {
    pub component_index: usize,
    pub perimeter_point_count: usize,
    pub candidate_seed_count: usize,
    pub point_seed_incidences: usize,
    pub max_point_candidate_seed_count: usize,
    pub distinct_incidence_signature_count: usize,
    pub max_incidence_signature_multiplicity: usize,
    pub max_local_ring_face_count: usize,
    pub max_distinct_local_footprint_mask_count: usize,
    pub max_local_union_state_count: usize,
    pub projected_interface_face_count: usize,
    pub projected_direct_union_state_cap: usize,
    pub projected_direct_union_state_count: Option<usize>,
    pub projected_direct_union_state_cap_exceeded_after_variables: Option<usize>,
    pub candidate_footprint_face_count: usize,
    pub candidate_footprint_union_state_count: Option<usize>,
    pub candidate_footprint_union_state_cap_exceeded_after_variables: Option<usize>,
    pub closure_prefix_variable_count: usize,
    pub closure_prefix_assignment_count: usize,
    pub closure_prefix_distinct_direct_mask_count: usize,
    pub closure_prefix_distinct_closed_mask_count: usize,
    pub closure_prefix_max_closed_mask_multiplicity: usize,
    pub closure_incremental_prefix_parity: bool,
    pub best_cut_point: usize,
    pub min_linearized_frontier_width: usize,
}

impl MethodCHfieldLegalizationPatchBoundaryCheck {
    pub fn is_closed(&self) -> bool {
        self.outside_changed_faces.is_empty() && !self.outside_perimeter_interface_changed
    }
}

fn sorted_usize_slices_intersect(left: &[usize], right: &[usize]) -> bool {
    let (mut left_index, mut right_index) = (0usize, 0usize);
    while left_index < left.len() && right_index < right.len() {
        match left[left_index].cmp(&right[right_index]) {
            std::cmp::Ordering::Less => left_index += 1,
            std::cmp::Ordering::Greater => right_index += 1,
            std::cmp::Ordering::Equal => return true,
        }
    }
    false
}

fn canonical_external_perimeter_interface(
    components: impl IntoIterator<Item = Vec<MethodCHfieldPerimeterPointCheckpoint>>,
    mutable_m_points: &BTreeSet<usize>,
    mutable_u_edges: &BTreeSet<usize>,
) -> Vec<Vec<Vec<MethodCHfieldPerimeterPointCheckpoint>>> {
    let mut result = Vec::new();
    for points in components {
        let Some(first_mutable) = points.iter().position(|point| {
            mutable_m_points.contains(&point.parent_m_point)
                || mutable_u_edges.contains(&point.parent_u_edge)
        }) else {
            continue;
        };
        let mut runs = Vec::new();
        let mut current = Vec::new();
        for step in 1..=points.len() {
            let point = &points[(first_mutable + step) % points.len()];
            if mutable_m_points.contains(&point.parent_m_point)
                || mutable_u_edges.contains(&point.parent_u_edge)
            {
                if !current.is_empty() {
                    runs.push(std::mem::take(&mut current));
                }
            } else {
                current.push(point.clone());
            }
        }
        if runs.is_empty() {
            continue;
        }
        let canonical_start = (0..runs.len())
            .min_by_key(|&start| {
                (0..runs.len())
                    .map(|offset| &runs[(start + offset) % runs.len()])
                    .collect::<Vec<_>>()
            })
            .expect("non-empty perimeter interface runs");
        runs.rotate_left(canonical_start);
        result.push(runs);
    }
    result.sort();
    result
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MethodCHfieldCandidateValidation {
    pub selected_faces_after_concavity: usize,
    pub coverage_valid: bool,
    pub parent_level_histogram: Vec<(usize, usize)>,
    pub parent_level_valid: bool,
    pub perimeter_lengths: Vec<usize>,
    pub perimeters_triplets: bool,
    pub predicted_transition_self_loops: usize,
    pub predicted_transition_first_parent_u_edge: Option<usize>,
    pub local_seed_candidate_pool: usize,
    pub local_seed_edit_sets_tested: usize,
    pub local_seed_edit_coverage_valid: usize,
    pub local_seed_edit_parent_level_valid: usize,
    pub local_seed_edit_triplet_valid: usize,
    pub local_seed_edit_predictor_clear: usize,
    pub local_seed_edit_first_predictor_clear_seeds: Vec<usize>,
    pub local_seed_edit_first_predictor_clear_removes_seed: Vec<bool>,
    pub local_seed_edit_materializable: usize,
    pub local_seed_edit_first_seeds: Vec<usize>,
    pub local_seed_edit_first_removes_seed: Vec<bool>,
    pub local_seed_edit_first_failure_kind: Option<MethodCHfieldFailureKind>,
    pub local_seed_edit_first_failure_parent_m_point: Option<usize>,
    pub local_seed_edit_first_failure_parent_u_edge: Option<usize>,
    pub local_seed_edit_first_failure_parent_m_valence_witnesses: Vec<usize>,
    pub local_seed_edit_first_failure_message: Option<String>,
    pub transition_materializable: bool,
    pub materialized_m_valence_census_available: bool,
    pub materialized_m_valence_violation_count: usize,
    pub failure_kind: Option<MethodCHfieldFailureKind>,
    pub failure_message: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MethodCHfieldFailureKind {
    HardCoverage,
    PerimeterTopology,
    NonTripletPerimeter,
    TransitionPatch,
    Valence,
    ParentLevelMismatch,
    Other,
}

impl MethodCHfieldFailureKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HardCoverage => "hard_coverage",
            Self::PerimeterTopology => "perimeter_topology",
            Self::NonTripletPerimeter => "non_triplet_perimeter",
            Self::TransitionPatch => "transition_patch",
            Self::Valence => "valence",
            Self::ParentLevelMismatch => "parent_level_mismatch",
            Self::Other => "other",
        }
    }
}

#[derive(Debug)]
pub struct MethodCHfieldSpawnFailure {
    pub pass: usize,
    pub kind: MethodCHfieldFailureKind,
    pub pass_diagnostics: Vec<MethodCHfieldPassDiagnostics>,
    pub perimeter_lengths: Vec<usize>,
    pub repair_attempts: usize,
    pub m_point: Option<usize>,
    pub parent_m_point: Option<usize>,
    pub parent_u_edge: Option<usize>,
    pub parent_m_valence_witnesses: Vec<usize>,
    pub w_face: Option<usize>,
    pub actual_mrlw: Option<usize>,
    pub expected_mrlw: Option<usize>,
    message: String,
}

impl std::fmt::Display for MethodCHfieldSpawnFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for MethodCHfieldSpawnFailure {}

pub fn method_c_hfield_spawn_failure(error: &io::Error) -> Option<&MethodCHfieldSpawnFailure> {
    error.get_ref()?.downcast_ref::<MethodCHfieldSpawnFailure>()
}

fn wrap_hfield_spawn_failure(
    pass: usize,
    pass_diagnostics: Vec<MethodCHfieldPassDiagnostics>,
    error: io::Error,
) -> io::Error {
    let (
        perimeter_lengths,
        repair_attempts,
        m_point,
        parent_m_point,
        parent_u_edge,
        parent_m_valence_witnesses,
    ) = if let Some(payload) = method_c_repairable_payload(&error) {
        (
            payload.perimeter_lengths.clone(),
            payload.repair_attempts,
            payload.m_point,
            payload.parent_m_point,
            payload.parent_u_edge,
            payload.parent_m_valence_witnesses.clone(),
        )
    } else {
        (Vec::new(), 0, None, None, None, Vec::new())
    };
    let parent = method_c_parent_level_mismatch_payload(&error);
    let kind = method_c_hfield_failure_kind(&error);
    let message = format!("Method-C h-field spawn_nest pass {pass} failed: {error}");
    io::Error::new(
        error.kind(),
        MethodCHfieldSpawnFailure {
            pass,
            kind,
            pass_diagnostics,
            perimeter_lengths,
            repair_attempts,
            m_point,
            parent_m_point,
            parent_u_edge,
            parent_m_valence_witnesses,
            w_face: parent.map(|payload| payload.w_face),
            actual_mrlw: parent.map(|payload| payload.actual_mrlw),
            expected_mrlw: parent.map(|payload| payload.expected_mrlw),
            message,
        },
    )
}

pub fn method_c_hfield_failure_kind(error: &io::Error) -> MethodCHfieldFailureKind {
    if error
        .get_ref()
        .and_then(|source| source.downcast_ref::<MethodCHfieldDemandCoverageError>())
        .is_some()
    {
        return MethodCHfieldFailureKind::HardCoverage;
    }
    if error
        .get_ref()
        .and_then(|source| source.downcast_ref::<MethodCHfieldPerimeterTopologyError>())
        .is_some()
    {
        return MethodCHfieldFailureKind::PerimeterTopology;
    }
    if method_c_parent_level_mismatch_payload(error).is_some() {
        return MethodCHfieldFailureKind::ParentLevelMismatch;
    }
    if let Some(payload) = method_c_repairable_payload(error) {
        return match payload.kind {
            MethodCRepairableKind::NonTripletPerimeter => {
                MethodCHfieldFailureKind::NonTripletPerimeter
            }
            MethodCRepairableKind::TransitionPatch => MethodCHfieldFailureKind::TransitionPatch,
            MethodCRepairableKind::Valence => MethodCHfieldFailureKind::Valence,
        };
    }
    MethodCHfieldFailureKind::Other
}

fn record_hfield_candidate_failure(
    validation: &mut MethodCHfieldCandidateValidation,
    error: &io::Error,
) {
    validation.failure_kind = Some(method_c_hfield_failure_kind(error));
    validation.failure_message = Some(error.to_string());
}

#[derive(Clone, Debug)]
pub(crate) struct MethodCHfieldDemandCoverage {
    anchors: Vec<(usize, Vec<usize>)>,
}

enum MethodCHfieldRad3Footprint {
    Materializable(Vec<usize>),
    PeriodicSeam,
}

impl MethodCHfieldDemandCoverage {
    #[cfg(test)]
    pub(crate) fn from_anchors(anchors: Vec<(usize, Vec<usize>)>) -> Self {
        Self { anchors }
    }

    pub(crate) fn validate(&self, selected: &[bool]) -> io::Result<()> {
        if coverage_relaxation_enabled() {
            // Diagnostic mode: let legalization proceed past anchors it cannot
            // cover so the run reports which demands Method-C had to concede
            // instead of aborting the pass. The concession list is what a
            // finer-granularity stage would have to satisfy afterwards, so the
            // hard-coverage contract is deferred here, never lowered.
            return Ok(());
        }
        for (im, faces) in &self.anchors {
            if !faces
                .iter()
                .any(|&iw| selected.get(iw).copied().unwrap_or(false))
            {
                return Err(hfield_demand_coverage_error(*im));
            }
        }
        Ok(())
    }

    /// Parent M points whose demand no face in `selected` covers.
    ///
    /// Always exact regardless of the relaxation switch, so a relaxed run can
    /// report precisely what it conceded.
    pub(crate) fn uncovered_anchors(&self, selected: &[bool]) -> Vec<usize> {
        self.anchors
            .iter()
            .filter(|(_, faces)| {
                !faces
                    .iter()
                    .any(|&iw| selected.get(iw).copied().unwrap_or(false))
            })
            .map(|(im, _)| *im)
            .collect()
    }

    pub(crate) fn anchor_count(&self) -> usize {
        self.anchors.len()
    }
}

/// Whether the parent-support oracle may answer from an unrepaired perimeter.
///
/// Off by default: a pass that cannot repair its perimeter still fails. When
/// set, the oracle reports the support that perimeter would need anyway, so the
/// outer loop can supply it and retry instead of stalling with no request.
pub(crate) fn support_oracle_best_effort_enabled() -> bool {
    std::env::var_os("EARTHMESH_M0_SUPPORT_ORACLE_BEST_EFFORT").is_some()
}

/// Whether hard-demand coverage may be conceded during legalization.
///
/// Off by default: production still fails a pass that cannot cover every
/// anchor. `EARTHMESH_M0_COVERAGE_RELAXATION` turns the failure into a recorded
/// concession so the size of the residue can be measured.
pub(crate) fn coverage_relaxation_enabled() -> bool {
    std::env::var_os("EARTHMESH_M0_COVERAGE_RELAXATION").is_some()
}

/// Method-C spawning driven by a quantized target-level field instead of
/// geometric regions ("M4" of the h-field integration).
///
/// The selection seam is the same `Vec<bool>` over Canonical-indexed W faces
/// that `selected_regions_faces` produces; everything downstream — the
/// Method-C pass, perimeter mrow construction, and the optional per-pass nest
/// spring — is reused verbatim. Invariants mirrored
/// from the region path: only faces of the current generation
/// (`mrlw == pass`) are selectable, and passes run shallow-to-deep so a
/// gradient-limited field (whose level sets are nested rings with bounded
/// shrink) always presents legal nesting to the discrete machinery.
///
/// Differences from the region path, by design: an empty pass-1 selection
/// returns the mesh unchanged (a field that demands nothing is a no-op, not
/// an error), an empty deeper pass simply stops descending, and the
/// region-specific parent-erosion retry is not applicable.
impl MethodCDelaunayMesh {
    fn hfield_component_phase_starts(
        &self,
        component: &[usize],
        demand_start: usize,
        m_neighbors: &[IcosahedronMPointNeighbors],
        phase_anchor: Option<CartesianPoint>,
    ) -> io::Result<(Vec<usize>, Option<usize>)> {
        let mut members = vec![false; self.nmd + 1];
        for &im in component {
            require_method_c_id("Method-C h-field phase component M point", im, self.nmd)?;
            members[im] = true;
        }
        let mut phase_of = vec![usize::MAX; self.nmd + 1];
        let mut jdone = vec![[false; 6]; self.nmd + 1];
        let mut starts = Vec::new();
        let anchor = self.m_points[demand_start];
        let mut candidates = component.to_vec();
        candidates.sort_unstable();
        for candidate in candidates {
            if phase_of[candidate] != usize::MAX {
                continue;
            }
            let phase = starts.len();
            let mut stack = vec![candidate];
            phase_of[candidate] = phase;
            let mut best = candidate;
            let mut best_distance = f64::INFINITY;
            while let Some(im) = stack.pop() {
                let point = self.m_points[im];
                let distance = (point.x - anchor.x).powi(2)
                    + (point.y - anchor.y).powi(2)
                    + (point.z - anchor.z).powi(2);
                if distance < best_distance || (distance == best_distance && im < best) {
                    best = im;
                    best_distance = distance;
                }
                for next in self.method_c_thirdm_neighbors_canonical_with_neighbors(
                    im,
                    &mut jdone,
                    m_neighbors,
                )? {
                    if members[next] && phase_of[next] == usize::MAX {
                        phase_of[next] = phase;
                        stack.push(next);
                    }
                }
            }
            starts.push(best);
        }
        let baseline_phase = phase_of[demand_start];
        if baseline_phase == usize::MAX {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Method-C h-field demand start is outside its phase component",
            ));
        }
        let requested_phase = phase_anchor.map(|anchor| {
            let im = component
                .iter()
                .copied()
                .min_by(|&a, &b| {
                    let distance = |im: usize| {
                        let point = self.m_points[im];
                        (point.x - anchor.x).powi(2)
                            + (point.y - anchor.y).powi(2)
                            + (point.z - anchor.z).powi(2)
                    };
                    distance(a).total_cmp(&distance(b)).then_with(|| a.cmp(&b))
                })
                .expect("demand component is non-empty");
            phase_of[im]
        });
        starts.swap(0, baseline_phase);
        starts[0] = demand_start;
        let requested_phase_ordinal = requested_phase.map(|phase| {
            if phase == baseline_phase {
                0
            } else if phase == 0 {
                baseline_phase
            } else {
                phase
            }
        });
        Ok((starts, requested_phase_ordinal))
    }

    fn hfield_perimeter_point_checkpoint(
        &self,
        point: MethodCPerimeterPoint,
    ) -> MethodCHfieldPerimeterPointCheckpoint {
        let mut parent_u_m_lineages = [
            self.m_lineage[self.u_edges[point.iu].im[0]],
            self.m_lineage[self.u_edges[point.iu].im[1]],
        ];
        parent_u_m_lineages.sort_unstable();
        MethodCHfieldPerimeterPointCheckpoint {
            parent_m_point: point.im,
            parent_m_lineage: self.m_lineage[point.im],
            parent_u_edge: point.iu,
            parent_u_m_lineages,
            npoly: point.npoly,
            nwdiv: point.nwdiv,
            near_pentagon: point.near_pentagon,
        }
    }

    fn selected_faces_from_method_c_seed_ids_with_neighbors(
        &self,
        seed_ids: &[usize],
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<Vec<bool>> {
        let mut selected = vec![false; self.nwd + 1];
        for &im in seed_ids {
            require_method_c_id("Method-C HField seed assignment", im, self.nmd)?;
            let level = self.m_metadata[im].mrlm;
            for iw in self.method_c_rad3_faces_with_neighbors(im, m_neighbors)? {
                if (2..=self.nwd).contains(&iw) && self.w_faces[iw].mrlw == level {
                    selected[iw] = true;
                }
            }
        }
        Ok(selected)
    }

    /// Map a finite Method-C seed assignment to the exact rad3 face mask
    /// consumed by the existing materializer.
    pub fn selected_faces_from_method_c_seed_ids(
        &self,
        seed_ids: &[usize],
    ) -> io::Result<Vec<bool>> {
        let m_neighbors = self.method_c_m_neighbors()?;
        self.selected_faces_from_method_c_seed_ids_with_neighbors(seed_ids, &m_neighbors)
    }

    fn sample_target_level<F: Fn(f64, f64) -> u8>(
        &self,
        point: CartesianPoint,
        target_level: &F,
        use_cartesian_xy: bool,
    ) -> usize {
        if use_cartesian_xy {
            usize::from(target_level(point.x, point.y))
        } else {
            let lonlat = xyz_to_lonlat_degrees(point);
            usize::from(target_level(lonlat.lon_degrees, lonlat.lat_degrees))
        }
    }

    fn m_point_target_level<F: Fn(f64, f64) -> u8>(
        &self,
        im: usize,
        target_level: &F,
        use_cartesian_xy: bool,
    ) -> usize {
        self.sample_target_level(self.m_points[im], target_level, use_cartesian_xy)
    }

    fn m_point_or_edge_target_level<F: Fn(f64, f64) -> u8>(
        &self,
        im: usize,
        neighbors: &IcosahedronMPointNeighbors,
        target_level: &F,
        use_cartesian_xy: bool,
    ) -> usize {
        let mut level = self.m_point_target_level(im, target_level, use_cartesian_xy);
        for &iu in neighbors.iu.iter().take(neighbors.npoly) {
            level =
                level.max(self.u_edge_midpoint_target_level(iu, target_level, use_cartesian_xy));
        }
        level
    }

    fn cartesian_hfield_rad3_failure_is_periodic_seam(
        &self,
        im: usize,
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<bool> {
        if self.impent.iter().any(|&pentagon| pentagon != 1) {
            return Ok(false);
        }
        require_method_c_id("Method-C Cartesian h-field seed M point", im, self.nmd)?;
        require_method_c_len(
            "Method-C Cartesian h-field M-neighbors",
            m_neighbors.len(),
            self.nmd + 1,
        )?;
        require_method_c_len(
            "Method-C Cartesian h-field M prognostic map",
            self.m_prognostic.len(),
            self.nmd + 1,
        )?;
        require_method_c_len(
            "Method-C Cartesian h-field W prognostic map",
            self.w_prognostic.len(),
            self.nwd + 1,
        )?;
        require_method_c_len(
            "Method-C Cartesian h-field W faces",
            self.w_faces.len(),
            self.nwd + 1,
        )?;
        require_method_c_len(
            "Method-C Cartesian h-field U edges",
            self.u_edges.len(),
            self.nud + 1,
        )?;

        let m_is_periodic_copy = |point: usize| -> io::Result<bool> {
            require_method_c_id("Method-C Cartesian h-field seam M point", point, self.nmd)?;
            let owner = self.m_prognostic[point];
            require_method_c_id("Method-C Cartesian h-field seam M owner", owner, self.nmd)?;
            if self.m_prognostic[owner] != owner {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Method-C Cartesian h-field M prognostic owner {owner} for point {point} is not canonical"
                    ),
                ));
            }
            Ok(owner != point)
        };
        let w_is_periodic_copy = |face: usize| -> io::Result<bool> {
            require_method_c_id("Method-C Cartesian h-field seam W face", face, self.nwd)?;
            let owner = self.w_prognostic[face];
            require_method_c_id("Method-C Cartesian h-field seam W owner", owner, self.nwd)?;
            if self.w_prognostic[owner] != owner {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Method-C Cartesian h-field W prognostic owner {owner} for face {face} is not canonical"
                    ),
                ));
            }
            Ok(owner != face)
        };
        let reciprocal_w_neighbors = |face_iw: usize| -> io::Result<[usize; 3]> {
            require_method_c_id(
                "Method-C Cartesian h-field reciprocal W face",
                face_iw,
                self.nwd,
            )?;
            let face = self.w_faces[face_iw];
            let mut result = [1usize; 3];
            for (slot, result_iw) in result.iter_mut().enumerate() {
                let iu = face.iu[slot];
                require_method_c_id("Method-C Cartesian h-field reciprocal U edge", iu, self.nud)?;
                let edge = self.u_edges[iu];
                let other_iw = if edge.iw[0] == face_iw {
                    edge.iw[1]
                } else if edge.iw[1] == face_iw {
                    edge.iw[0]
                } else {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "Method-C Cartesian h-field W face {face_iw} edge slot {slot} points at U edge {iu}, but the edge does not point back"
                        ),
                    ));
                };
                require_method_c_id(
                    "Method-C Cartesian h-field reciprocal W neighbor",
                    other_iw,
                    self.nwd,
                )?;
                *result_iw = other_iw;
            }
            Ok(result)
        };

        let neighbors = m_neighbors[im];
        for &iw in neighbors.iw.iter().take(neighbors.npoly) {
            require_method_c_id("Method-C Cartesian h-field sector W face", iw, self.nwd)?;
            let sector = self.w_faces[iw];
            let (imx, iwx, iwy, inner_slot) = if im == sector.im[0] {
                (sector.im[1], sector.iw[3], sector.iw[4], 0)
            } else if im == sector.im[1] {
                (sector.im[2], sector.iw[5], sector.iw[6], 1)
            } else if im == sector.im[2] {
                (sector.im[0], sector.iw[7], sector.iw[8], 2)
            } else {
                return Ok(false);
            };
            require_method_c_id("Method-C Cartesian h-field sector M point", imx, self.nmd)?;
            require_method_c_id("Method-C Cartesian h-field outer W face", iwx, self.nwd)?;
            require_method_c_id("Method-C Cartesian h-field outer W face", iwy, self.nwd)?;

            let (im1, im2) = match face_following_two_vertices(self.w_faces[iwx], imx, iwx) {
                Ok(points) => points,
                Err(_) => {
                    // `iw[3..9]` is not covered by the general topology
                    // validator. Re-derive this exact pair from the validated
                    // first-ring adjacency before accepting the known cart_hex
                    // periodic representation gap. An arbitrary ghost pointer
                    // therefore remains a fatal rad3 error.
                    let sector_neighbors = reciprocal_w_neighbors(iw)?;
                    let inner_iw = sector_neighbors[inner_slot];
                    let inner_neighbors = reciprocal_w_neighbors(inner_iw)?;
                    let canonical_pair = tri_neighbors_outer_w_pair(iw, inner_neighbors);
                    for &outer_iw in &canonical_pair {
                        require_method_c_id(
                            "Method-C Cartesian h-field canonical outer W face",
                            outer_iw,
                            self.nwd,
                        )?;
                    }
                    if [iwx, iwy] != canonical_pair
                        || self.w_faces[canonical_pair[0]].im.contains(&imx)
                        || self.w_faces[canonical_pair[1]].im.contains(&imx)
                    {
                        return Ok(false);
                    }
                    let mut touches_periodic_copy = m_is_periodic_copy(imx)?;
                    for &outer_iw in &canonical_pair {
                        touches_periodic_copy |= w_is_periodic_copy(outer_iw)?;
                        for &face_im in &self.w_faces[outer_iw].im {
                            touches_periodic_copy |= m_is_periodic_copy(face_im)?;
                        }
                    }
                    return Ok(touches_periodic_copy);
                }
            };
            require_method_c_id("Method-C Cartesian h-field distant M point", im1, self.nmd)?;
            require_method_c_id("Method-C Cartesian h-field distant M point", im2, self.nmd)?;
            let im3 = match face_following_vertex(self.w_faces[iwy], im2, iwy) {
                Ok(point) => point,
                Err(_) => return Ok(false),
            };
            require_method_c_id("Method-C Cartesian h-field distant M point", im3, self.nmd)?;
            for far_im in [im1, im2, im3] {
                for &far_iw in m_neighbors[far_im].iw.iter().take(6) {
                    if far_iw > self.nwd {
                        return Ok(false);
                    }
                }
            }
        }
        Ok(false)
    }

    fn hfield_rad3_footprint(
        &self,
        im: usize,
        m_neighbors: &[IcosahedronMPointNeighbors],
        use_cartesian_xy: bool,
    ) -> io::Result<MethodCHfieldRad3Footprint> {
        match self.method_c_rad3_faces_with_neighbors(im, m_neighbors) {
            Ok(faces) => Ok(MethodCHfieldRad3Footprint::Materializable(faces)),
            Err(_error)
                if use_cartesian_xy
                    && self.cartesian_hfield_rad3_failure_is_periodic_seam(im, m_neighbors)? =>
            {
                Ok(MethodCHfieldRad3Footprint::PeriodicSeam)
            }
            Err(error) => Err(error),
        }
    }

    #[cfg(test)]
    pub(crate) fn hfield_rad3_faces_for_test(
        &self,
        im: usize,
        m_neighbors: &[IcosahedronMPointNeighbors],
        use_cartesian_xy: bool,
    ) -> io::Result<Option<Vec<usize>>> {
        self.hfield_rad3_footprint(im, m_neighbors, use_cartesian_xy)
            .map(|footprint| match footprint {
                MethodCHfieldRad3Footprint::Materializable(faces) => Some(faces),
                MethodCHfieldRad3Footprint::PeriodicSeam => None,
            })
    }

    fn u_edge_midpoint_target_level<F: Fn(f64, f64) -> u8>(
        &self,
        iu: usize,
        target_level: &F,
        use_cartesian_xy: bool,
    ) -> usize {
        let [im1, im2] = self.u_edges[iu].im;
        let p1 = self.m_points[im1];
        let p2 = self.m_points[im2];
        let midpoint = CartesianPoint::new(
            0.5 * (p1.x + p2.x),
            0.5 * (p1.y + p2.y),
            0.5 * (p1.z + p2.z),
        );
        self.sample_target_level(midpoint, target_level, use_cartesian_xy)
    }

    /// Mirror of `selected_regions_faces` with the geometric containment test
    /// replaced by the target-level closure: grow thirdm-stride seed M points
    /// from a deterministic anchor (the deepest-demand point, lowest id on
    /// ties), then mark each seed's rad3 face footprint filtered to the seed
    /// generation (`mrlw == mrlo`). Reusing the seed/rad3 machinery — rather
    /// rather than selecting sampled faces directly — is what keeps the mask
    /// boundary smooth and multiple-of-3 aligned, which the Method-C perimeter
    /// walker requires.
    #[cfg(test)]
    pub(crate) fn selected_faces_from_target_levels<F: Fn(f64, f64) -> u8>(
        &self,
        target_level: &F,
        pass: usize,
        use_cartesian_xy: bool,
    ) -> io::Result<Vec<bool>> {
        self.selected_faces_and_coverage_from_target_levels_with_policy(
            target_level,
            None,
            pass,
            use_cartesian_xy,
            true,
            false,
            false,
        )
        .map(|(selected, _, _)| selected)
    }

    #[cfg(test)]
    pub(crate) fn selected_faces_from_target_levels_with_policy_for_test<F: Fn(f64, f64) -> u8>(
        &self,
        target_level: &F,
        pass: usize,
        preserve_all_demands: bool,
    ) -> io::Result<(Vec<bool>, MethodCHfieldPassDiagnostics)> {
        self.selected_faces_and_coverage_from_target_levels_with_policy(
            target_level,
            None,
            pass,
            false,
            preserve_all_demands,
            true,
            false,
        )
        .map(|(selected, _, diagnostics)| (selected, diagnostics))
    }

    pub(crate) fn close_hfield_seed_vertex_contacts(
        &self,
        selected: &mut [bool],
        selected_seeds: &mut [usize],
        legal_seed: &[usize],
        face_owner_seed: &[usize],
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<Vec<usize>> {
        let mut added = Vec::new();
        loop {
            let contacts = self.method_c_vertex_only_perimeter_contacts(selected, m_neighbors)?;
            if contacts.is_empty() {
                return Ok(added);
            }
            let fill_faces = self.method_c_vertex_only_perimeter_contact_fill_faces(
                selected,
                m_neighbors,
                &contacts,
            )?;
            let mut seeds = fill_faces
                .into_iter()
                .filter_map(|iw| face_owner_seed.get(iw).copied())
                .filter(|&im| {
                    im > 1
                        && im < legal_seed.len()
                        && legal_seed[im] != 0
                        && selected_seeds[im] != legal_seed[im]
                })
                .collect::<Vec<_>>();
            seeds.sort_unstable();
            seeds.dedup();
            if seeds.is_empty() {
                return Ok(added);
            }
            for im in seeds {
                let level = self.m_metadata[im].mrlm;
                selected_seeds[im] = legal_seed[im];
                for iw in self.method_c_rad3_faces_with_neighbors(im, m_neighbors)? {
                    if iw >= 2 && iw <= self.nwd && self.w_faces[iw].mrlw == level {
                        selected[iw] = true;
                    }
                }
                added.push(im);
            }
        }
    }

    fn close_hfield_seed_concavities(
        &self,
        selected: &mut [bool],
        selected_seeds: &mut [usize],
        legal_seed: &[usize],
        face_owner_seed: &[usize],
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<Vec<usize>> {
        let mut added = Vec::new();
        loop {
            let mut seeds = std::collections::BTreeSet::new();
            for neighbors in m_neighbors.iter().take(self.nmd + 1).skip(2) {
                let mut selected_count = 0usize;
                let mut missing = 0usize;
                for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                    if selected[iw] {
                        selected_count += 1;
                    } else {
                        missing = iw;
                    }
                }
                if selected_count != neighbors.npoly.saturating_sub(1) {
                    continue;
                }
                let im = face_owner_seed[missing];
                if im > 1
                    && im < legal_seed.len()
                    && legal_seed[im] != 0
                    && selected_seeds[im] != legal_seed[im]
                {
                    seeds.insert(im);
                }
            }
            if seeds.is_empty() {
                return Ok(added);
            }
            for im in seeds {
                let level = self.m_metadata[im].mrlm;
                selected_seeds[im] = legal_seed[im];
                for iw in self.method_c_rad3_faces_with_neighbors(im, m_neighbors)? {
                    if iw >= 2 && iw <= self.nwd && self.w_faces[iw].mrlw == level {
                        selected[iw] = true;
                    }
                }
                added.push(im);
            }
        }
    }

    pub(crate) fn close_hfield_seed_non_triplet_perimeters(
        &self,
        selected: &mut Vec<bool>,
        selected_seeds: &mut [usize],
        legal_seed: &[usize],
        face_owner_seed: &[usize],
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<Vec<usize>> {
        let mut added = Vec::new();
        loop {
            let Ok(perimeters) =
                self.method_c_perimeters_from_selected_faces(selected, m_neighbors)
            else {
                return Ok(added);
            };
            if Self::method_c_perimeters_are_triplets(&perimeters) {
                return Ok(added);
            }
            let current_remainder = Self::method_c_perimeter_remainder_score(&perimeters);
            let mut candidates = std::collections::BTreeSet::new();
            for point in perimeters.iter().flatten() {
                let neighbors = m_neighbors[point.im];
                for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                    if selected[iw] {
                        continue;
                    }
                    let im = face_owner_seed[iw];
                    if im > 1
                        && im < legal_seed.len()
                        && legal_seed[im] != 0
                        && selected_seeds[im] != legal_seed[im]
                    {
                        candidates.insert(im);
                    }
                }
            }

            let mut best = None;
            for im in candidates {
                let mut trial = selected.clone();
                let level = self.m_metadata[im].mrlm;
                for iw in self.method_c_rad3_faces_with_neighbors(im, m_neighbors)? {
                    if iw >= 2 && iw <= self.nwd && self.w_faces[iw].mrlw == level {
                        trial[iw] = true;
                    }
                }
                let Ok(trial_perimeters) =
                    self.method_c_perimeters_from_selected_faces(&trial, m_neighbors)
                else {
                    continue;
                };
                let remainder = Self::method_c_perimeter_remainder_score(&trial_perimeters);
                if remainder >= current_remainder {
                    continue;
                }
                let score = (
                    remainder,
                    trial_perimeters.iter().map(Vec::len).sum::<usize>(),
                    im,
                );
                if best
                    .as_ref()
                    .is_none_or(|(best_score, _)| score < *best_score)
                {
                    best = Some((score, trial));
                }
            }
            let Some(((.., im), trial)) = best else {
                return Ok(added);
            };
            selected_seeds[im] = legal_seed[im];
            *selected = trial;
            added.push(im);
        }
    }

    fn selected_faces_and_coverage_from_target_levels_with_policy<F: Fn(f64, f64) -> u8>(
        &self,
        target_level: &F,
        face_demand: Option<&[bool]>,
        pass: usize,
        use_cartesian_xy: bool,
        preserve_all_demands: bool,
        collect_diagnostics: bool,
        allow_m0_phase_variant: bool,
    ) -> io::Result<(
        Vec<bool>,
        MethodCHfieldDemandCoverage,
        MethodCHfieldPassDiagnostics,
    )> {
        require_method_c_len("m_points", self.m_points.len(), self.nmd + 1)?;
        require_method_c_len("w_faces", self.w_faces.len(), self.nwd + 1)?;
        if use_cartesian_xy {
            require_method_c_len(
                "Method-C Cartesian h-field M prognostic map",
                self.m_prognostic.len(),
                self.nmd + 1,
            )?;
        }
        let method_c_m_neighbors = self.method_c_m_neighbors()?;
        let mut selected = vec![false; self.nwd + 1];
        let mut anchors = Vec::new();
        let mut alignable_faces = vec![false; self.nwd + 1];
        let mut diagnostics = MethodCHfieldPassDiagnostics {
            pass,
            preserve_all_demands,
            ..Default::default()
        };

        // A deeper H-field level may touch the transition apron produced by
        // the previous pass. Only current-parent interior M points can seed a
        // legal Method-C perimeter; clipping that boundary row preserves the
        // valid demand instead of aborting the whole refinement.
        let mut parent_interior = vec![false; self.nmd + 1];
        for im in 2..=self.nmd {
            if use_cartesian_xy {
                let owner = self.m_prognostic[im];
                require_method_c_id("Method-C Cartesian h-field M owner", owner, self.nmd)?;
                if self.m_prognostic[owner] != owner {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "Method-C Cartesian h-field M prognostic owner {owner} for point {im} is not canonical"
                        ),
                    ));
                }
                if owner != im {
                    continue;
                }
            }
            let mrlo = self.m_metadata[im].mrlm;
            if mrlo != pass {
                continue;
            }
            let neighbors = method_c_m_neighbors[im];
            let mut is_parent_interior = true;
            for &iu in neighbors.iu.iter().take(neighbors.npoly) {
                require_method_c_id("Method-C h-field eligibility U edge", iu, self.nud)?;
                if self.u_edges[iu].mrlu != mrlo {
                    is_parent_interior = false;
                    break;
                }
            }
            parent_interior[im] = is_parent_interior;
        }
        if collect_diagnostics {
            diagnostics.parent_interior_m_points =
                parent_interior.iter().filter(|&&inside| inside).count();
        }

        // Record every sampled point/edge demand separately. This prevents a
        // repair from preserving one face of a large component while silently
        // eroding the rest of the requested threshold footprint.
        let mut demand_at_m = vec![false; self.nmd + 1];
        let mut point_demand_at_m = vec![false; self.nmd + 1];
        for im in 2..=self.nmd {
            let level = self.m_point_target_level(im, target_level, use_cartesian_xy);
            if !parent_interior[im] || level < pass {
                continue;
            }
            let neighbors = method_c_m_neighbors[im];
            let mut faces = Vec::new();
            for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                require_method_c_id("Method-C h-field point-demand W face", iw, self.nwd)?;
                if self.w_faces[iw].mrlw == pass {
                    faces.push(iw);
                }
            }
            if !faces.is_empty() {
                demand_at_m[im] = true;
                point_demand_at_m[im] = true;
                anchors.push((im, faces));
            }
        }
        for iu in 2..=self.nud {
            let edge = self.u_edges[iu];
            let level = self.u_edge_midpoint_target_level(iu, target_level, use_cartesian_xy);
            if edge.mrlu != pass || level < pass {
                continue;
            }
            let Some(anchor_im) = edge
                .im
                .iter()
                .copied()
                .find(|&im| im <= self.nmd && parent_interior[im])
            else {
                continue;
            };
            let mut faces = Vec::new();
            for &iw in edge.iw.iter().take(2) {
                require_method_c_id("Method-C h-field edge-demand W face", iw, self.nwd)?;
                if self.w_faces[iw].mrlw == pass {
                    faces.push(iw);
                }
            }
            if !faces.is_empty() {
                for &im in &edge.im {
                    if im <= self.nmd && parent_interior[im] {
                        demand_at_m[im] = true;
                    }
                }
                anchors.push((anchor_im, faces));
            }
        }
        if let Some(face_demand) = face_demand {
            require_method_c_len("Method-C face hard demand", face_demand.len(), self.nwd + 1)?;
            for (iw, &demanded) in face_demand.iter().enumerate().take(self.nwd + 1).skip(2) {
                if !demanded || self.w_faces[iw].mrlw != pass {
                    continue;
                }
                let face = self.w_faces[iw];
                let Some(anchor_im) = face
                    .im
                    .iter()
                    .copied()
                    .find(|&im| im <= self.nmd && parent_interior[im])
                else {
                    continue;
                };
                for &im in &face.im {
                    if im <= self.nmd && parent_interior[im] {
                        demand_at_m[im] = true;
                    }
                }
                anchors.push((anchor_im, vec![iw]));
            }
        }
        if collect_diagnostics {
            diagnostics.hard_demand_m_points =
                demand_at_m.iter().filter(|&&demanded| demanded).count();
            diagnostics.hard_demand_anchors = anchors.len();
        }

        // Nearby islands must share one Canonical phase because their rad3
        // footprints and transition aprons can meet. A six-edge support halo
        // joins only potentially interacting islands; keeping the traversal
        // local avoids carrying one phase around a pentagon or parent seam.
        let mut phase_support = demand_at_m.clone();
        // Every demand fragment needs a bounded owner search. Pass 1 still
        // uses the base icosahedron's global canonical phase; expanding only
        // its support does not change that phase, and lets a sub-lattice hard
        // face reach the nearest canonical stride-3 seed.
        for _ in 0..6 {
            let mut expanded = phase_support.clone();
            for im in 2..=self.nmd {
                if !phase_support[im] {
                    continue;
                }
                let neighbors = method_c_m_neighbors[im];
                for &iu in neighbors.iu.iter().take(neighbors.npoly) {
                    require_method_c_id("Method-C h-field phase-support U edge", iu, self.nud)?;
                    for &next in &self.u_edges[iu].im {
                        if next > 1 && next <= self.nmd && parent_interior[next] {
                            expanded[next] = true;
                        }
                    }
                }
            }
            phase_support = expanded;
        }
        if collect_diagnostics {
            diagnostics.phase_support_m_points =
                phase_support.iter().filter(|&&supported| supported).count();
        }

        // Method-C has one globally valid thirdm congruence class on the base
        // icosahedron. Anchoring separate pass-1 demand islands to arbitrary
        // local M points can shift that phase and create an invalid transition
        // even when every individual rad3 footprint is legal. Build the phase
        // membership once; components still select only their local demand.
        // cart_hex has no spherical pentagon; like the geometric Cartesian
        // path, its local stride phase is anchored directly in the demand.
        let use_global_canonical_phase = pass == 1 && !use_cartesian_xy;
        let mut canonical_phase = vec![false; self.nmd + 1];
        if use_global_canonical_phase {
            if let Some(global_start) = self.impent.iter().copied().find(|&im| im > 1) {
                let mut phase_done = vec![[false; 6]; self.nmd + 1];
                let mut stack = vec![global_start];
                canonical_phase[global_start] = true;
                while let Some(im) = stack.pop() {
                    for next in self.method_c_thirdm_neighbors_canonical_with_neighbors(
                        im,
                        &mut phase_done,
                        &method_c_m_neighbors,
                    )? {
                        if !canonical_phase[next] {
                            canonical_phase[next] = true;
                            stack.push(next);
                        }
                    }
                }
            }
        }

        // Reuse pass-wide indexed scratch. Fragmented fields can contain many
        // components; allocating/scanning nmd/nwd-sized buffers per island
        // made selection O(components * mesh size) before any mesh work.
        // The component root is a unique non-zero stamp, so touched entries
        // need neither clearing nor a second membership bitmap.
        let mut component_stamp = vec![0usize; self.nmd + 1];
        let mut seed_seen = vec![0usize; self.nmd + 1];
        let mut legal_seed = vec![0usize; self.nmd + 1];
        let mut lattice_neighbors = vec![Vec::new(); self.nmd + 1];
        let mut jdone = vec![[false; 6]; self.nmd + 1];
        let mut jdone_touched = Vec::new();
        let mut seed_demand_reachable = vec![0usize; self.nmd + 1];
        let mut selected_seeds = vec![0usize; self.nmd + 1];
        let mut seed_reason = collect_diagnostics.then(|| vec![0u8; self.nmd + 1]);
        let mut face_reason = collect_diagnostics.then(|| vec![0u8; self.nwd + 1]);
        let mut footprint_index = vec![usize::MAX; self.nmd + 1];
        let mut face_owner_seed = vec![0usize; self.nwd + 1];
        let mut owner = HashMap::new();
        let mut anchor_indices_by_m = vec![Vec::new(); self.nmd + 1];
        for (index, (im, _)) in anchors.iter().enumerate() {
            anchor_indices_by_m[*im].push(index);
        }

        for root in 2..=self.nmd {
            if component_stamp[root] != 0 || !phase_support[root] {
                continue;
            }
            let component_mrl = self.m_metadata[root].mrlm;
            let mut component = Vec::new();
            let mut queue = std::collections::VecDeque::from([root]);
            component_stamp[root] = root;
            while let Some(im) = queue.pop_front() {
                component.push(im);
                let neighbors = method_c_m_neighbors[im];
                for &iu in neighbors.iu.iter().take(neighbors.npoly) {
                    require_method_c_id("Method-C h-field component U edge", iu, self.nud)?;
                    let edge = self.u_edges[iu];
                    let next = if edge.im[0] == im {
                        edge.im[1]
                    } else {
                        edge.im[0]
                    };
                    if next > 1
                        && next <= self.nmd
                        && component_stamp[next] == 0
                        && phase_support[next]
                        && self.m_metadata[next].mrlm == component_mrl
                    {
                        component_stamp[next] = root;
                        queue.push_back(next);
                    }
                }
            }
            if !component.iter().any(|&im| demand_at_m[im]) {
                continue;
            }
            let component_index = diagnostics.component_count;
            diagnostics.component_count += 1;
            let has_point_demand = component.iter().any(|&im| point_demand_at_m[im]);
            // Anchor the Canonical phase inside the demand, as the geometric
            // region path does. All demand islands in this parent then share
            // one phase without forcing a globe-spanning pentagon phase.
            let demand_start = component
                .iter()
                .copied()
                .filter(|&im| demand_at_m[im])
                .find(|im| self.impent.contains(im))
                .or_else(|| {
                    let demanded = component.iter().copied().filter(|&im| demand_at_m[im]);
                    if preserve_all_demands {
                        demanded.min()
                    } else {
                        demanded.max_by(|a, b| {
                            let a_level = self.m_point_or_edge_target_level(
                                *a,
                                &method_c_m_neighbors[*a],
                                target_level,
                                use_cartesian_xy,
                            );
                            let b_level = self.m_point_or_edge_target_level(
                                *b,
                                &method_c_m_neighbors[*b],
                                target_level,
                                use_cartesian_xy,
                            );
                            a_level.cmp(&b_level).then_with(|| b.cmp(a))
                        })
                    }
                })
                .expect("demanded parent component has an anchor");
            let mut start = if use_global_canonical_phase {
                let anchor = self.m_points[demand_start];
                component
                    .iter()
                    .copied()
                    .filter(|&im| canonical_phase[im])
                    .min_by(|&a, &b| {
                        let distance = |im: usize| {
                            let point = self.m_points[im];
                            (point.x - anchor.x).powi(2)
                                + (point.y - anchor.y).powi(2)
                                + (point.z - anchor.z).powi(2)
                        };
                        distance(a).total_cmp(&distance(b)).then_with(|| a.cmp(&b))
                    })
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!(
                                "Method-C h-field pass {pass} demand component has no canonical stride-3 seed"
                            ),
                        )
                    })?
            } else if use_cartesian_xy {
                let (sum_x, sum_y, count) = component
                    .iter()
                    .copied()
                    .filter(|&im| demand_at_m[im])
                    .fold((0.0, 0.0, 0usize), |(sum_x, sum_y, count), im| {
                        (
                            sum_x + self.m_points[im].x,
                            sum_y + self.m_points[im].y,
                            count + 1,
                        )
                    });
                let centroid = CartesianPoint::new(sum_x / count as f64, sum_y / count as f64, 0.0);
                let mut candidates = component.clone();
                candidates.sort_by(|&a, &b| {
                    let distance = |im: usize| {
                        let point = self.m_points[im];
                        (point.x - centroid.x).powi(2) + (point.y - centroid.y).powi(2)
                    };
                    distance(a).total_cmp(&distance(b)).then_with(|| a.cmp(&b))
                });
                let mut legal_start = None;
                for im in candidates {
                    let MethodCHfieldRad3Footprint::Materializable(footprint) =
                        self.hfield_rad3_footprint(im, &method_c_m_neighbors, use_cartesian_xy)?
                    else {
                        continue;
                    };
                    if footprint.iter().any(|&iw| iw >= 2)
                        && footprint
                            .iter()
                            .copied()
                            .filter(|&iw| iw >= 2)
                            .all(|iw| iw <= self.nwd && self.w_faces[iw].mrlw == component_mrl)
                    {
                        legal_start = Some(im);
                        break;
                    }
                }
                legal_start.ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "Method-C Cartesian h-field pass {pass} demand component has no legal local stride-3 seed"
                        ),
                    )
                })?
            } else {
                demand_start
            };
            if pass > 1 && !use_cartesian_xy {
                let requested = if allow_m0_phase_variant {
                    m0_hfield_phase_anchor(pass)?
                } else {
                    None
                };
                let phase_anchor = requested
                    .filter(|requested| {
                        component.iter().any(|&im| {
                            let point = self.m_points[im];
                            point.x == requested.component.x
                                && point.y == requested.component.y
                                && point.z == requested.component.z
                        })
                    })
                    .map(|requested| requested.phase);
                if collect_diagnostics || phase_anchor.is_some() {
                    let (phase_starts, requested_phase_ordinal) = self
                        .hfield_component_phase_starts(
                            &component,
                            demand_start,
                            &method_c_m_neighbors,
                            phase_anchor,
                        )?;
                    let selected_phase_ordinal = requested_phase_ordinal.unwrap_or_default();
                    if phase_anchor.is_some() {
                        eprintln!(
                            "earthmesh_mesh: M0 HField phase override pass={pass} component={component_index} phase_count={} ordinal={selected_phase_ordinal}",
                            phase_starts.len()
                            );
                    }
                    start = phase_starts[selected_phase_ordinal];
                    if collect_diagnostics {
                        let mut component_m_points = component.clone();
                        component_m_points.sort_unstable();
                        diagnostics
                            .component_phases
                            .push(MethodCHfieldComponentPhaseDiagnostics {
                                component_index,
                                component_m_points,
                                demand_start,
                                phase_class_count: phase_starts.len(),
                                phase_starts,
                                selected_phase_ordinal,
                                selected_start: start,
                                legal_seed_ids: Vec::new(),
                                selected_seed_ids: Vec::new(),
                            });
                    }
                }
            }
            let mut lattice_seeds = Vec::new();
            let mut lista = vec![start];
            seed_seen[start] = root;
            while let Some(im) = lista.pop() {
                // cart_hex's outer seam contains valid traversal points whose
                // periodic face representation cannot materialize a complete
                // rad3 footprint. Skip only that explicitly classified case;
                // malformed topology and every other rad3 error remain fatal.
                let footprint = match self.hfield_rad3_footprint(
                    im,
                    &method_c_m_neighbors,
                    use_cartesian_xy,
                )? {
                    MethodCHfieldRad3Footprint::Materializable(footprint) => footprint,
                    MethodCHfieldRad3Footprint::PeriodicSeam => Vec::new(),
                };
                let footprint_is_legal = footprint.iter().any(|&iw| iw >= 2)
                    && !footprint
                        .iter()
                        .copied()
                        .filter(|&iw| iw >= 2)
                        .any(|iw| iw > self.nwd || self.w_faces[iw].mrlw != component_mrl);
                if footprint_is_legal {
                    for &iw in &footprint {
                        if iw >= 2 && iw <= self.nwd && self.w_faces[iw].mrlw == component_mrl {
                            alignable_faces[iw] = true;
                        }
                    }
                    legal_seed[im] = root;
                    lattice_seeds.push((im, footprint));
                }
                // An atomic footprint can straddle the parent transition
                // boundary even though legal seeds exist one stride farther
                // inside. Keep traversing the Canonical lattice through that
                // non-materializable seed; otherwise the boundary seed cuts
                // off the entire legal interior and valid deeper demand is
                // reported as uncovered.
                jdone_touched.push(im);
                for neighbor in self.method_c_thirdm_neighbors_canonical_with_neighbors(
                    im,
                    &mut jdone,
                    &method_c_m_neighbors,
                )? {
                    jdone_touched.push(neighbor);
                    let traversed_count = jdone[neighbor].iter().filter(|&&done| done).count();
                    if component_stamp[neighbor] != root
                        || self.m_metadata[neighbor].mrlm != component_mrl
                    {
                        continue;
                    }
                    lattice_neighbors[im].push(neighbor);
                    lattice_neighbors[neighbor].push(im);
                    // The component is already bounded to the six-hop phase
                    // support. Do not gate this traversal on demand: an
                    // isolated hard fragment can contain no canonical seed.
                    if traversed_count < 2 && seed_seen[neighbor] != root {
                        seed_seen[neighbor] = root;
                        lista.push(neighbor);
                    }
                }
            }
            for im in jdone_touched.drain(..) {
                jdone[im] = [false; 6];
            }
            if collect_diagnostics {
                diagnostics.legal_rad3_seeds += lattice_seeds.len();
            }
            for (im, _) in &lattice_seeds {
                let neighbors = &mut lattice_neighbors[*im];
                neighbors.retain(|&neighbor| legal_seed[neighbor] == root);
                neighbors.sort_unstable();
                neighbors.dedup();
            }
            if legal_seed[start] == root {
                let mut queue = std::collections::VecDeque::from([start]);
                seed_demand_reachable[start] = root;
                while let Some(im) = queue.pop_front() {
                    for &next in &lattice_neighbors[im] {
                        let follows_demand = if has_point_demand {
                            point_demand_at_m[next]
                        } else {
                            demand_at_m[next]
                        };
                        if follows_demand && seed_demand_reachable[next] != root {
                            seed_demand_reachable[next] = root;
                            queue.push_back(next);
                        }
                    }
                }
            }

            // Assign each parent face to its nearest seed. Selecting the owner
            // of each demand anchor applies one aligned rad3 footprint instead
            // of dilating the demand once while finding seeds and again while
            // materializing their footprints.
            owner.clear();
            for (im, footprint) in &lattice_seeds {
                let seed = self.m_points[*im];
                for &iw in footprint {
                    if iw < 2 || iw > self.nwd || self.w_faces[iw].mrlw != component_mrl {
                        continue;
                    }
                    let face = self.w_faces[iw];
                    let center = CartesianPoint::new(
                        (self.m_points[face.im[0]].x
                            + self.m_points[face.im[1]].x
                            + self.m_points[face.im[2]].x)
                            / 3.0,
                        (self.m_points[face.im[0]].y
                            + self.m_points[face.im[1]].y
                            + self.m_points[face.im[2]].y)
                            / 3.0,
                        (self.m_points[face.im[0]].z
                            + self.m_points[face.im[1]].z
                            + self.m_points[face.im[2]].z)
                            / 3.0,
                    );
                    let distance = (seed.x - center.x).powi(2)
                        + (seed.y - center.y).powi(2)
                        + (seed.z - center.z).powi(2);
                    let (current_owner, current_distance) =
                        owner.get(&iw).copied().unwrap_or((0, f64::INFINITY));
                    if distance < current_distance
                        || (distance == current_distance && *im < current_owner)
                    {
                        owner.insert(iw, (*im, distance));
                    }
                }
            }
            for (&iw, &(im, _)) in &owner {
                face_owner_seed[iw] = im;
            }

            for (index, (im, _)) in lattice_seeds.iter().enumerate() {
                footprint_index[*im] = index;
            }
            for (im, _) in &lattice_seeds {
                if seed_demand_reachable[*im] == root {
                    selected_seeds[*im] = root;
                }
            }
            selected_seeds[start] = root;
            for (im, footprint) in &lattice_seeds {
                if selected_seeds[*im] == root {
                    if let Some(reasons) = seed_reason.as_mut() {
                        if reasons[*im] == 0 {
                            reasons[*im] = 1;
                            diagnostics.initial_selected_seeds += 1;
                        }
                    }
                    for &iw in footprint {
                        if iw >= 2 && iw <= self.nwd && self.w_faces[iw].mrlw == component_mrl {
                            selected[iw] = true;
                            if let Some(reasons) = face_reason.as_mut() {
                                reasons[iw] |= 1;
                            }
                        }
                    }
                }
            }
            // Vertex sampling can miss a thin edge-only tail. Add exactly one
            // nearest aligned owner only when an individual demand anchor is
            // still uncovered by the center-selected footprints.
            let mut component_anchor_indices = component
                .iter()
                .flat_map(|&im| anchor_indices_by_m[im].iter().copied())
                .collect::<Vec<_>>();
            component_anchor_indices.sort_unstable();
            for anchor_index in component_anchor_indices {
                let (anchor_im, faces) = &anchors[anchor_index];
                if faces.iter().any(|&iw| selected[iw]) {
                    continue;
                }
                let anchor = self.m_points[*anchor_im];
                let mut best = None;
                for &iw in faces {
                    let im = owner.get(&iw).map(|&(im, _)| im).unwrap_or(0);
                    if im <= 1 {
                        continue;
                    }
                    let seed = self.m_points[im];
                    let distance = (seed.x - anchor.x).powi(2)
                        + (seed.y - anchor.y).powi(2)
                        + (seed.z - anchor.z).powi(2);
                    if best.is_none_or(|(best_distance, best_im)| {
                        distance < best_distance || (distance == best_distance && im < best_im)
                    }) {
                        best = Some((distance, im));
                    }
                }
                if let Some((_, im)) = best {
                    if selected_seeds[im] != root {
                        selected_seeds[im] = root;
                        if let Some(reasons) = seed_reason.as_mut() {
                            reasons[im] |= 2;
                            diagnostics.demand_tail_seeds += 1;
                        }
                        let index = footprint_index[im];
                        if index != usize::MAX {
                            for &iw in &lattice_seeds[index].1 {
                                if iw >= 2
                                    && iw <= self.nwd
                                    && self.w_faces[iw].mrlw == component_mrl
                                {
                                    selected[iw] = true;
                                    if let Some(reasons) = face_reason.as_mut() {
                                        reasons[iw] |= 2;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if preserve_all_demands {
                loop {
                    let mut bridges = Vec::new();
                    for (mid, _) in &lattice_seeds {
                        if selected_seeds[*mid] == root {
                            continue;
                        }
                        let selected_neighbors = lattice_neighbors[*mid]
                            .iter()
                            .copied()
                            .filter(|&im| selected_seeds[im] == root)
                            .collect::<Vec<_>>();
                        'pairs: for a_index in 0..selected_neighbors.len() {
                            for b_index in (a_index + 1)..selected_neighbors.len() {
                                let a = selected_neighbors[a_index];
                                let b = selected_neighbors[b_index];
                                if lattice_neighbors[a].binary_search(&b).is_ok() {
                                    continue;
                                }
                                let common = lattice_neighbors[a]
                                    .iter()
                                    .copied()
                                    .filter(|candidate| {
                                        lattice_neighbors[b].binary_search(candidate).is_ok()
                                    })
                                    .collect::<Vec<_>>();
                                if common.as_slice() == [*mid] {
                                    bridges.push(*mid);
                                    break 'pairs;
                                }
                            }
                        }
                    }
                    if bridges.is_empty() {
                        break;
                    }
                    for im in bridges {
                        selected_seeds[im] = root;
                        if let Some(reasons) = seed_reason.as_mut() {
                            reasons[im] |= 4;
                            diagnostics.connectivity_bridge_seeds += 1;
                        }
                    }
                }
            }
            for (im, footprint) in lattice_seeds {
                if selected_seeds[im] == root {
                    for iw in footprint {
                        if iw >= 2 && iw <= self.nwd && self.w_faces[iw].mrlw == component_mrl {
                            selected[iw] = true;
                            if let (Some(seed_reasons), Some(face_reasons)) =
                                (seed_reason.as_ref(), face_reason.as_mut())
                            {
                                face_reasons[iw] |= seed_reasons[im];
                            }
                        }
                    }
                }
            }
        }
        let mut closure_seeds = Vec::new();
        loop {
            let before = closure_seeds.len();
            closure_seeds.extend(self.close_hfield_seed_concavities(
                &mut selected,
                &mut selected_seeds,
                &legal_seed,
                &face_owner_seed,
                &method_c_m_neighbors,
            )?);
            closure_seeds.extend(self.close_hfield_seed_vertex_contacts(
                &mut selected,
                &mut selected_seeds,
                &legal_seed,
                &face_owner_seed,
                &method_c_m_neighbors,
            )?);
            closure_seeds.extend(self.close_hfield_seed_non_triplet_perimeters(
                &mut selected,
                &mut selected_seeds,
                &legal_seed,
                &face_owner_seed,
                &method_c_m_neighbors,
            )?);
            if closure_seeds.len() == before {
                break;
            }
        }
        if std::env::var_os("EARTHMESH_M0_REPAIR_TRACE").is_some() {
            let perimeter_lengths = self
                .method_c_perimeters_from_selected_faces(&selected, &method_c_m_neighbors)
                .map(|perimeters| perimeters.into_iter().map(|p| p.len()).collect::<Vec<_>>());
            eprintln!(
                "earthmesh_mesh: method_c hfield seed closure added_seeds={} selected_faces={} perimeter_lengths={perimeter_lengths:?}",
                closure_seeds.len(),
                selected.iter().filter(|&&item| item).count(),
            );
        }
        if let Some(reasons) = seed_reason.as_mut() {
            for im in closure_seeds {
                reasons[im] |= 4;
                diagnostics.connectivity_bridge_seeds += 1;
                let level = self.m_metadata[im].mrlm;
                for iw in self.method_c_rad3_faces_with_neighbors(im, &method_c_m_neighbors)? {
                    if iw >= 2 && iw <= self.nwd && self.w_faces[iw].mrlw == level {
                        if let Some(face_reasons) = face_reason.as_mut() {
                            face_reasons[iw] |= 4;
                        }
                    }
                }
            }
        }
        // The previous pass's transition apron can contain current-generation
        // M points while still being too close to the parent seam for any
        // complete rad3 footprint. Those samples are not legal Method-C
        // anchors: clip them based on the existence of an atomic aligned
        // footprint, rather than letting a partial footprint cross the seam or
        // failing an otherwise valid deeper interior pass.
        anchors.retain(|(_, faces)| {
            faces
                .iter()
                .any(|&iw| alignable_faces.get(iw).copied().unwrap_or(false))
        });
        let coverage = MethodCHfieldDemandCoverage { anchors };
        coverage.validate(&selected)?;
        if collect_diagnostics {
            let component_by_root = diagnostics
                .component_phases
                .iter()
                .enumerate()
                .filter_map(|(index, component)| {
                    component
                        .component_m_points
                        .first()
                        .copied()
                        .map(|root| (root, index))
                })
                .collect::<HashMap<_, _>>();
            for im in 2..=self.nmd {
                let root = legal_seed[im];
                let Some(&index) = component_by_root.get(&root) else {
                    continue;
                };
                diagnostics.component_phases[index].legal_seed_ids.push(im);
                if selected_seeds[im] == root {
                    diagnostics.component_phases[index]
                        .selected_seed_ids
                        .push(im);
                }
            }
            diagnostics.legal_seed_ids = (2..=self.nmd).filter(|&im| legal_seed[im] != 0).collect();
            diagnostics.selected_seed_ids = (2..=self.nmd)
                .filter(|&im| selected_seeds[im] != 0 && selected_seeds[im] == legal_seed[im])
                .collect();
            let contacts =
                self.method_c_vertex_only_perimeter_contacts(&selected, &method_c_m_neighbors)?;
            diagnostics.seed_union_vertex_only_contacts = contacts.len();
            diagnostics.seed_union_first_contact_m_point = contacts.first().copied();
            match self.selected_faces_from_method_c_seed_ids_with_neighbors(
                &diagnostics.selected_seed_ids,
                &method_c_m_neighbors,
            ) {
                Ok(reconstructed) => {
                    diagnostics.seed_reconstruction_matches = reconstructed == selected;
                }
                Err(error) => diagnostics.seed_reconstruction_error = Some(error.to_string()),
            }
            diagnostics.alignable_faces = alignable_faces
                .iter()
                .filter(|&&alignable| alignable)
                .count();
            diagnostics.final_selected_faces = selected
                .iter()
                .skip(2)
                .filter(|&&selected| selected)
                .count();
            if let Some(reasons) = face_reason.as_ref() {
                for (&selected, &reason) in selected.iter().zip(reasons).skip(2) {
                    if selected {
                        diagnostics.face_reason_mask_counts[usize::from(reason & 7)] += 1;
                    }
                }
                let counts = diagnostics.face_reason_mask_counts;
                diagnostics.initial_seed_footprint_faces =
                    counts[1] + counts[3] + counts[5] + counts[7];
                diagnostics.demand_tail_faces = counts[2] + counts[3] + counts[6] + counts[7];
                diagnostics.connectivity_bridge_faces =
                    counts[4] + counts[5] + counts[6] + counts[7];
                diagnostics.unexplained_selected_faces = counts[0];
            }
        }
        Ok((selected, coverage, diagnostics))
    }

    #[allow(clippy::too_many_arguments)]
    fn diagnose_hfield_local_seed_edits(
        &self,
        validation: &mut MethodCHfieldCandidateValidation,
        coverage: &MethodCHfieldDemandCoverage,
        materialization: (usize, usize),
        m_neighbors: &[IcosahedronMPointNeighbors],
        selected_seed_ids: &[usize],
        legal_seed_ids: &[usize],
        witnesses: &[(usize, usize)],
    ) {
        let mut local_faces = vec![false; self.nwd + 1];
        for &(_, parent_u) in witnesses {
            let Some(edge) = self.u_edges.get(parent_u) else {
                continue;
            };
            for &im in &edge.im {
                let Some(neighbors) = m_neighbors.get(im) else {
                    continue;
                };
                for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                    if iw <= self.nwd {
                        local_faces[iw] = true;
                    }
                }
            }
        }

        let selected_seed = selected_seed_ids
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        let mut face_owners = vec![0usize; self.nwd + 1];
        for &im in selected_seed_ids {
            let Ok(faces) = self.method_c_rad3_faces_with_neighbors(im, m_neighbors) else {
                return;
            };
            let level = self.m_metadata[im].mrlm;
            for iw in faces {
                if iw >= 2 && iw <= self.nwd && self.w_faces[iw].mrlw == level {
                    face_owners[iw] += 1;
                }
            }
        }

        let mut local_candidates = Vec::new();
        for &im in legal_seed_ids {
            let Ok(faces) = self.method_c_rad3_faces_with_neighbors(im, m_neighbors) else {
                continue;
            };
            let level = self.m_metadata[im].mrlm;
            if faces.iter().any(|&iw| {
                iw >= 2 && iw <= self.nwd && self.w_faces[iw].mrlw == level && local_faces[iw]
            }) {
                local_candidates.push((im, selected_seed.contains(&im)));
            }
        }
        validation.local_seed_candidate_pool = local_candidates.len();
        let mut edit_sets = local_candidates
            .iter()
            .copied()
            .map(|edit| vec![edit])
            .collect::<Vec<_>>();
        for left in 0..local_candidates.len() {
            for right in (left + 1)..local_candidates.len() {
                edit_sets.push(vec![local_candidates[left], local_candidates[right]]);
            }
        }
        validation.local_seed_edit_sets_tested = edit_sets.len();

        let build_trial = |edits: &[(usize, bool)]| -> io::Result<Vec<bool>> {
            let mut owners = face_owners.clone();
            for &(seed, removes_seed) in edits {
                let level = self.m_metadata[seed].mrlm;
                for iw in self.method_c_rad3_faces_with_neighbors(seed, m_neighbors)? {
                    if iw < 2 || iw > self.nwd || self.w_faces[iw].mrlw != level {
                        continue;
                    }
                    if removes_seed {
                        owners[iw] = owners[iw].saturating_sub(1);
                    } else {
                        owners[iw] += 1;
                    }
                }
            }
            Ok(owners.into_iter().map(|owners| owners > 0).collect())
        };

        let mut predictor_clear = Vec::new();
        for edits in edit_sets {
            let Ok(mut trial) = build_trial(&edits) else {
                continue;
            };
            if self
                .close_method_c_concavities_for_level_with_neighbors(&mut trial, m_neighbors)
                .is_err()
                || coverage.validate(&trial).is_err()
            {
                continue;
            }
            validation.local_seed_edit_coverage_valid += 1;
            if self
                .ensure_method_c_selected_faces_share_parent_mrlw(&trial, materialization.0)
                .is_err()
            {
                continue;
            }
            validation.local_seed_edit_parent_level_valid += 1;
            let Ok(perimeters) = self.method_c_perimeters_from_selected_faces(&trial, m_neighbors)
            else {
                continue;
            };
            if !Self::method_c_perimeters_are_triplets(&perimeters) {
                continue;
            }
            validation.local_seed_edit_triplet_valid += 1;
            let perimeter = perimeters.iter().flatten().copied().collect::<Vec<_>>();
            let Ok(nest_wd) = self.method_c_nest_wd_from_selected_and_perimeter(&trial, &perimeter)
            else {
                continue;
            };
            let Ok(predicted) = self.method_c_transition_self_loop_witnesses(&perimeter, &nest_wd)
            else {
                continue;
            };
            if predicted.is_empty() {
                validation.local_seed_edit_predictor_clear += 1;
                if validation
                    .local_seed_edit_first_predictor_clear_seeds
                    .is_empty()
                {
                    validation.local_seed_edit_first_predictor_clear_seeds =
                        edits.iter().map(|&(seed, _)| seed).collect();
                    validation.local_seed_edit_first_predictor_clear_removes_seed = edits
                        .iter()
                        .map(|&(_, removes_seed)| removes_seed)
                        .collect();
                }
                predictor_clear.push(edits);
            }
        }

        for edits in predictor_clear {
            let Ok(trial) = build_trial(&edits) else {
                continue;
            };
            match self.spawn_nest_pass_method_c_without_mask_repair(
                &trial,
                materialization.0,
                materialization.1,
                true,
            ) {
                Ok(_) => {
                    validation.local_seed_edit_materializable += 1;
                    if validation.local_seed_edit_first_seeds.is_empty() {
                        validation.local_seed_edit_first_seeds =
                            edits.iter().map(|&(seed, _)| seed).collect();
                        validation.local_seed_edit_first_removes_seed = edits
                            .iter()
                            .map(|&(_, removes_seed)| removes_seed)
                            .collect();
                    }
                }
                Err(error) => {
                    if let Some(payload) = method_c_repairable_payload(&error) {
                        if std::env::var_os("EARTHMESH_M0_REPAIR_TRACE").is_some() {
                            eprintln!(
                                "earthmesh_mesh: local seed edits={edits:?} failed kind={:?} parent_m={:?} parent_u={:?} parent_m_valence_witnesses={:?}",
                                payload.kind,
                                payload.parent_m_point,
                                payload.parent_u_edge,
                                payload.parent_m_valence_witnesses,
                            );
                        }
                        if validation.local_seed_edit_first_failure_kind.is_none() {
                            validation.local_seed_edit_first_failure_parent_m_point =
                                payload.parent_m_point;
                            validation.local_seed_edit_first_failure_parent_u_edge =
                                payload.parent_u_edge;
                            validation.local_seed_edit_first_failure_parent_m_valence_witnesses =
                                payload.parent_m_valence_witnesses.clone();
                        }
                        if std::env::var_os("EARTHMESH_M0_REPAIR_TRACE").is_some() {
                            let mut closed = trial.clone();
                            if self
                                .close_method_c_concavities_for_level_with_neighbors(
                                    &mut closed,
                                    m_neighbors,
                                )
                                .is_ok()
                            {
                                if let Ok(perimeters) = self
                                    .method_c_perimeters_from_selected_faces(&closed, m_neighbors)
                                {
                                    if let Some(parent_u) = payload.parent_u_edge {
                                        let triples = perimeters
                                            .iter()
                                            .flat_map(|perimeter| perimeter.chunks_exact(3))
                                            .filter(|triple| {
                                                triple.iter().any(|point| point.iu == parent_u)
                                            })
                                            .collect::<Vec<_>>();
                                        eprintln!(
                                            "earthmesh_mesh: local seed edits={edits:?} moved valence witness to parent_u={parent_u} triples={triples:?}"
                                        );
                                    }
                                    if !payload.parent_m_valence_witnesses.is_empty() {
                                        let perimeter = perimeters
                                            .iter()
                                            .flatten()
                                            .copied()
                                            .collect::<Vec<_>>();
                                        if let Ok(nest_wd) = self
                                            .method_c_nest_wd_from_selected_and_perimeter(
                                                &closed, &perimeter,
                                            )
                                        {
                                            for &parent_m in &payload.parent_m_valence_witnesses {
                                                let Some(neighbors) =
                                                    m_neighbors.get(parent_m).copied()
                                                else {
                                                    continue;
                                                };
                                                let incident_u = neighbors
                                                    .iu
                                                    .iter()
                                                    .take(neighbors.npoly)
                                                    .copied()
                                                    .collect::<Vec<_>>();
                                                let mut expanded_u = incident_u.clone();
                                                for &iu in &incident_u {
                                                    expanded_u.extend(
                                                        self.u_edges[iu]
                                                            .iu
                                                            .iter()
                                                            .copied()
                                                            .filter(|&neighbor_u| neighbor_u > 1),
                                                    );
                                                }
                                                expanded_u.sort_unstable();
                                                expanded_u.dedup();
                                                let ring = neighbors
                                                    .iw
                                                    .iter()
                                                    .take(neighbors.npoly)
                                                    .map(|&iw| {
                                                        (
                                                            iw,
                                                            closed[iw],
                                                            nest_wd[iw].is_suppressed(),
                                                        )
                                                    })
                                                    .collect::<Vec<_>>();
                                                let direct_triples = perimeters
                                                    .iter()
                                                    .flat_map(|perimeter| perimeter.chunks_exact(3))
                                                    .enumerate()
                                                    .filter(|(_, triple)| {
                                                        triple.iter().any(|point| {
                                                            point.im == parent_m
                                                                || incident_u.contains(&point.iu)
                                                        })
                                                    })
                                                    .collect::<Vec<_>>();
                                                let expanded_triples = perimeters
                                                    .iter()
                                                    .flat_map(|perimeter| perimeter.chunks_exact(3))
                                                    .enumerate()
                                                    .filter(|(_, triple)| {
                                                        triple.iter().any(|point| {
                                                            expanded_u.contains(&point.iu)
                                                        })
                                                    })
                                                    .collect::<Vec<_>>();
                                                eprintln!(
                                                    "earthmesh_mesh: local seed edits={edits:?} parent_m_valence={parent_m} ring={ring:?} incident_u={incident_u:?} direct_triples={direct_triples:?} expanded_triples={expanded_triples:?}"
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if validation.local_seed_edit_first_failure_kind.is_none() {
                        validation.local_seed_edit_first_failure_kind =
                            Some(method_c_hfield_failure_kind(&error));
                        validation.local_seed_edit_first_failure_message = Some(error.to_string());
                    }
                }
            }
        }
    }

    fn hfield_has_demand_at_or_above<F: Fn(f64, f64) -> u8>(
        &self,
        target_level: &F,
        level: usize,
        use_cartesian_xy: bool,
    ) -> io::Result<bool> {
        let m_neighbors = self.method_c_m_neighbors()?;
        Ok((2..=self.nmd).any(|im| {
            self.m_point_or_edge_target_level(im, &m_neighbors[im], target_level, use_cartesian_xy)
                >= level
        }))
    }

    fn hfield_has_current_parent_demand<F: Fn(f64, f64) -> u8>(
        &self,
        target_level: &F,
        pass: usize,
        use_cartesian_xy: bool,
    ) -> io::Result<bool> {
        let m_neighbors = self.method_c_m_neighbors()?;
        Ok((2..=self.nmd).any(|im| {
            self.m_metadata[im].mrlm == pass
                && self.m_point_or_edge_target_level(
                    im,
                    &m_neighbors[im],
                    target_level,
                    use_cartesian_xy,
                ) >= pass
        }))
    }

    fn validate_hfield_candidate(
        &self,
        selected_faces: &[bool],
        coverage: &MethodCHfieldDemandCoverage,
        child_level: usize,
        max_mrows: usize,
        selected_seed_ids: &[usize],
        legal_seed_ids: &[usize],
    ) -> MethodCHfieldCandidateValidation {
        let mut validation = MethodCHfieldCandidateValidation::default();
        let mut selected = selected_faces.to_vec();
        let m_neighbors = match self.method_c_m_neighbors() {
            Ok(neighbors) => neighbors,
            Err(error) => {
                record_hfield_candidate_failure(&mut validation, &error);
                return validation;
            }
        };
        if let Err(error) =
            self.close_method_c_concavities_for_level_with_neighbors(&mut selected, &m_neighbors)
        {
            record_hfield_candidate_failure(&mut validation, &error);
            return validation;
        }
        validation.selected_faces_after_concavity = selected
            .iter()
            .skip(2)
            .filter(|&&selected| selected)
            .count();
        let mut parent_levels = BTreeMap::new();
        for iw in 2..=self.nwd {
            if selected[iw] {
                *parent_levels.entry(self.w_faces[iw].mrlw).or_insert(0) += 1;
            }
        }
        validation.parent_level_histogram = parent_levels.into_iter().collect();

        if let Err(error) = coverage.validate(&selected) {
            record_hfield_candidate_failure(&mut validation, &error);
            return validation;
        }
        validation.coverage_valid = true;
        if let Err(error) =
            self.ensure_method_c_selected_faces_share_parent_mrlw(&selected, child_level)
        {
            record_hfield_candidate_failure(&mut validation, &error);
            return validation;
        }
        validation.parent_level_valid = true;
        let perimeters = match self.method_c_perimeters_from_selected_faces(&selected, &m_neighbors)
        {
            Ok(perimeters) => perimeters,
            Err(error) => {
                record_hfield_candidate_failure(&mut validation, &error);
                return validation;
            }
        };
        validation.perimeter_lengths = perimeters.iter().map(Vec::len).collect();
        validation.perimeters_triplets = Self::method_c_perimeters_are_triplets(&perimeters);
        if !validation.perimeters_triplets {
            validation.failure_kind = Some(MethodCHfieldFailureKind::NonTripletPerimeter);
            validation.failure_message = Some(format!(
                "Method-C candidate perimeter lengths {:?} are not divisible by three",
                validation.perimeter_lengths
            ));
            return validation;
        }
        let perimeter = perimeters.iter().flatten().copied().collect::<Vec<_>>();
        let nest_wd = match self.method_c_nest_wd_from_selected_and_perimeter(&selected, &perimeter)
        {
            Ok(nest_wd) => nest_wd,
            Err(error) => {
                record_hfield_candidate_failure(&mut validation, &error);
                return validation;
            }
        };
        let witnesses = match self.method_c_transition_self_loop_witnesses(&perimeter, &nest_wd) {
            Ok(witnesses) => witnesses,
            Err(error) => {
                record_hfield_candidate_failure(&mut validation, &error);
                return validation;
            }
        };
        validation.predicted_transition_self_loops = witnesses.len();
        validation.predicted_transition_first_parent_u_edge =
            witnesses.first().map(|&(_, parent_u)| parent_u);

        match self.spawn_nest_pass_method_c_without_mask_repair(
            &selected,
            child_level,
            max_mrows,
            true,
        ) {
            Ok(mesh) => {
                validation.transition_materializable = true;
                match collect_icosahedron_m_valence_witnesses_canonical(
                    mesh.nmd,
                    &mesh.u_edges,
                    &mesh.w_faces,
                    Some(&mesh.m_prognostic),
                ) {
                    Ok(witnesses) => {
                        validation.materialized_m_valence_census_available = true;
                        validation.materialized_m_valence_violation_count = witnesses.len();
                    }
                    Err(error) => {
                        if std::env::var_os("EARTHMESH_M0_REPAIR_TRACE").is_some() {
                            eprintln!(
                                "earthmesh_mesh: materialized M-valence census unavailable: {error}"
                            );
                        }
                    }
                }
            }
            Err(error) => record_hfield_candidate_failure(&mut validation, &error),
        }
        if !witnesses.is_empty() {
            self.diagnose_hfield_local_seed_edits(
                &mut validation,
                coverage,
                (child_level, max_mrows),
                &m_neighbors,
                selected_seed_ids,
                legal_seed_ids,
                &witnesses,
            );
        }
        validation
    }

    /// Materialize one Method-C generation from hard demand attached directly
    /// to current Delaunay W faces. The existing aligned seed/rad3 closure and
    /// coverage checks remain the sole topology-selection implementation.
    pub fn spawn_nest_pass_from_face_demands(
        &self,
        face_demand: &[bool],
        pass: usize,
        child_grid_number: usize,
        max_mrows: usize,
    ) -> io::Result<Option<Self>> {
        self.spawn_nest_pass_from_target_levels_and_face_demands(
            |_, _| 0,
            face_demand,
            pass,
            child_grid_number,
            max_mrows,
            true,
        )
    }

    pub(crate) fn required_parent_support_lineages_from_selected_and_perimeter(
        &self,
        selected: &[bool],
        perimeter: &[MethodCPerimeterPoint],
    ) -> io::Result<Vec<i64>> {
        let nest_wd = self.method_c_nest_wd_from_selected_and_perimeter(selected, perimeter)?;
        let parent_level = selected
            .iter()
            .enumerate()
            .skip(2)
            .find_map(|(iw, &is_selected)| is_selected.then_some(self.w_faces[iw].mrlw))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Method-C HField support preflight has no selected parent level",
                )
            })?;
        // A conceded region can never be refined, so asking for parent support
        // there would loop forever. Those witnesses are dropped here and the
        // selection has to keep clear of them instead.
        let conceded = crate::method_c_perimeter_repair::conceded_lineage_snapshot();
        let mut lineages = self
            .method_c_transition_parent_boundary_witnesses(perimeter, &nest_wd, parent_level)?
            .into_iter()
            .flat_map(|(_, faces)| faces)
            .filter(|&iw| self.w_faces[iw].mrlw < parent_level && !nest_wd[iw].is_subdivided())
            .filter(|&iw| !conceded.contains(&self.w_lineage[iw]))
            .map(|iw| {
                i64::try_from(self.w_lineage[iw]).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "Method-C W-face lineage {} at face {iw} exceeds i64",
                            self.w_lineage[iw]
                        ),
                    )
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        lineages.sort_unstable();
        lineages.dedup();
        Ok(lineages)
    }

    /// Stable parent-face identities that must be refined one pass earlier
    /// before this HField candidate's canonical transition can materialize.
    pub fn required_parent_support_lineages_from_target_levels_and_face_demands<
        F: Fn(f64, f64) -> u8,
    >(
        &self,
        target_level: F,
        face_demand: &[bool],
        pass: usize,
        preserve_all_demands: bool,
    ) -> io::Result<Vec<i64>> {
        if pass <= 1 {
            return Ok(Vec::new());
        }
        let (mut selected_faces, coverage, _) = self
            .selected_faces_and_coverage_from_target_levels_with_policy(
                &target_level,
                Some(face_demand),
                pass,
                false,
                preserve_all_demands,
                false,
                true,
            )?;
        let m_neighbors = self.method_c_m_neighbors()?;
        self.clear_method_c_conceded_margin(&mut selected_faces, &m_neighbors, 2)?;
        coverage.validate(&selected_faces)?;
        if selected_faces.iter().skip(2).all(|selected| !*selected) {
            return Ok(Vec::new());
        }

        let mut selected = selected_faces;
        self.close_method_c_concavities_for_level_with_neighbors(&mut selected, &m_neighbors)?;
        coverage.validate(&selected)?;
        self.ensure_method_c_selected_faces_share_parent_mrlw(&selected, pass + 1)?;
        match self.repair_method_c_non_triplet_perimeter(&mut selected, &m_neighbors, pass + 1) {
            Ok(perimeter) => {
                coverage.validate(&selected)?;
                self.required_parent_support_lineages_from_selected_and_perimeter(
                    &selected, &perimeter,
                )
            }
            Err(error) => {
                if let Some(request) = crate::method_c_parent_support_request(&error) {
                    return Ok(request
                        .lineages
                        .iter()
                        .filter_map(|&lineage| i64::try_from(lineage).ok())
                        .collect());
                }
                if support_oracle_best_effort_enabled() {
                    // Which parent faces `perim_fill3` would consume is answerable
                    // from the perimeter as it stands: consumption reads the faces
                    // just outside the selection and does not depend on the
                    // perimeter being decomposable. Repairing first is a
                    // convenience, not a precondition, and letting its failure
                    // propagate leaves the pass with no support request at all --
                    // support cannot be computed because repair failed, and repair
                    // failed for want of support.
                    eprintln!(
                        "earthmesh_mesh: method_c support oracle best-effort pass={pass} \
                         repair_error={error}"
                    );
                    let perimeter = self
                        .method_c_perimeters_from_selected_faces(&selected, &m_neighbors)?
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>();
                    self.required_parent_support_lineages_from_selected_and_perimeter(
                        &selected, &perimeter,
                    )
                } else {
                    Err(error)
                }
            }
        }
    }

    /// Stable parent-face support required by one complete canonical seed
    /// assignment before exact child materialization.
    pub fn required_parent_support_lineages_from_seed_assignment(
        &self,
        checkpoint: &MethodCHfieldSelectionCheckpoint,
        selected_seed_ids: &[usize],
        child_level: usize,
    ) -> io::Result<Vec<i64>> {
        if child_level <= 1 {
            return Ok(Vec::new());
        }
        let m_neighbors = self.method_c_m_neighbors()?;
        let (selected, perimeters, symbolic) = self.legalization_symbolic_state_with_neighbors(
            checkpoint,
            selected_seed_ids,
            child_level,
            &m_neighbors,
        )?;
        if symbolic.predicted_transition_self_loop_count.is_none() {
            return Err(method_c_repairable_perimeter_error(
                MethodCRepairableKind::NonTripletPerimeter,
                symbolic.perimeter_lengths,
                0,
                "Method-C legalization assignment cannot request parent support because a perimeter is not triplet-aligned",
            ));
        }
        let perimeter = perimeters.into_iter().flatten().collect::<Vec<_>>();
        self.required_parent_support_lineages_from_selected_and_perimeter(&selected, &perimeter)
    }

    /// Freeze the exact discrete inputs and selected mask used by a spherical
    /// HField pass before any child topology is materialized.
    pub fn selection_checkpoint_from_target_levels_and_face_demands<F: Fn(f64, f64) -> u8>(
        &self,
        target_level: F,
        face_demand: &[bool],
        pass: usize,
        preserve_all_demands: bool,
    ) -> io::Result<MethodCHfieldSelectionCheckpoint> {
        if pass == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C face-demand checkpoint pass must be positive",
            ));
        }
        let (selected_faces, coverage, diagnostics) = self
            .selected_faces_and_coverage_from_target_levels_with_policy(
                &target_level,
                Some(face_demand),
                pass,
                false,
                preserve_all_demands,
                true,
                true,
            )?;
        if !diagnostics.seed_reconstruction_matches {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                diagnostics.seed_reconstruction_error.unwrap_or_else(|| {
                    "Method-C HField selected seeds do not reconstruct the selected face mask"
                        .to_string()
                }),
            ));
        }
        let mut m_target_levels = vec![0usize; self.nmd + 1];
        for (im, level) in m_target_levels
            .iter_mut()
            .enumerate()
            .take(self.nmd + 1)
            .skip(2)
        {
            *level = self.m_point_target_level(im, &target_level, false);
        }
        let mut u_target_levels = vec![0usize; self.nud + 1];
        for (iu, level) in u_target_levels
            .iter_mut()
            .enumerate()
            .take(self.nud + 1)
            .skip(2)
        {
            *level = self.u_edge_midpoint_target_level(iu, &target_level, false);
        }
        let demand_anchors = coverage
            .anchors
            .into_iter()
            .map(
                |(parent_m_point, candidate_faces)| MethodCHfieldDemandAnchorCheckpoint {
                    parent_m_point,
                    parent_m_lineage: self.m_lineage[parent_m_point],
                    candidate_face_lineages: candidate_faces
                        .iter()
                        .map(|&iw| self.w_lineage[iw])
                        .collect(),
                    candidate_faces,
                },
            )
            .collect();
        Ok(MethodCHfieldSelectionCheckpoint {
            pass,
            preserve_all_demands,
            m_target_levels,
            u_target_levels,
            face_demand: face_demand.to_vec(),
            demand_anchors,
            selected_faces,
            legal_seed_ids: diagnostics.legal_seed_ids,
            selected_seed_ids: diagnostics.selected_seed_ids,
            component_phases: diagnostics.component_phases,
        })
    }

    /// Prepare one selected mask exactly as the materializer does, then
    /// describe every predicted transition self-loop in stable parent space.
    pub fn legalization_preflight_from_selected_faces(
        &self,
        selected_faces: &[bool],
        legal_seed_ids: &[usize],
        selected_seed_ids: &[usize],
        child_level: usize,
    ) -> io::Result<MethodCHfieldLegalizationPreflight> {
        if child_level <= 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C legalization preflight child level must be greater than one",
            ));
        }
        require_method_c_len("selected_faces", selected_faces.len(), self.nwd + 1)?;
        let method_c_m_neighbors = self.method_c_m_neighbors()?;
        let mut selected = selected_faces.to_vec();
        self.close_method_c_concavities_for_level_with_neighbors(
            &mut selected,
            &method_c_m_neighbors,
        )?;
        self.ensure_method_c_selected_faces_share_parent_mrlw(&selected, child_level)?;
        self.repair_method_c_non_triplet_perimeter(
            &mut selected,
            &method_c_m_neighbors,
            child_level,
        )?;
        let perimeters =
            self.method_c_perimeters_from_selected_faces(&selected, &method_c_m_neighbors)?;
        let perimeter_lengths = perimeters.iter().map(Vec::len).collect::<Vec<_>>();
        let perimeter_remainders = perimeter_lengths
            .iter()
            .map(|length| length % 3)
            .collect::<Vec<_>>();
        let perimeter = perimeters.iter().flatten().copied().collect::<Vec<_>>();
        let nest_wd = self.method_c_nest_wd_from_selected_and_perimeter(&selected, &perimeter)?;
        let predicted = self.method_c_transition_self_loop_witnesses(&perimeter, &nest_wd)?;

        let mut triple_components = Vec::with_capacity(perimeter.len() / 3);
        for (component, points) in perimeters.iter().enumerate() {
            for component_triple in 0..points.len() / 3 {
                triple_components.push((component, component_triple));
            }
        }
        let mut witnesses = Vec::with_capacity(predicted.len());
        for (triple_index, parent_u_edge) in predicted {
            let &(perimeter_component, component_triple_index) =
                triple_components.get(triple_index).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "Method-C self-loop triple {triple_index} exceeds {} perimeter triples",
                            triple_components.len()
                        ),
                    )
                })?;
            require_method_c_id(
                "Method-C legalization preflight parent U edge",
                parent_u_edge,
                self.nud,
            )?;
            let parent_u_m_points = self.u_edges[parent_u_edge].im;
            let mut parent_u_m_lineages = [
                self.m_lineage[parent_u_m_points[0]],
                self.m_lineage[parent_u_m_points[1]],
            ];
            parent_u_m_lineages.sort_unstable();
            let dependency_faces =
                self.method_c_parent_u_dependency_faces(parent_u_edge, &method_c_m_neighbors);
            let mut dependency_face_lineages = dependency_faces
                .iter()
                .map(|&iw| self.w_lineage[iw])
                .collect::<Vec<_>>();
            dependency_face_lineages.sort_unstable();
            dependency_face_lineages.dedup();
            witnesses.push(MethodCTransitionSelfLoopCheckpointWitness {
                triple_index,
                perimeter_component,
                component_triple_index,
                parent_u_edge,
                parent_u_m_points,
                parent_u_m_lineages,
                dependency_faces,
                dependency_face_lineages,
                candidate_seed_ids: Vec::new(),
            });
        }

        let selected_seed_ids = selected_seed_ids.iter().copied().collect::<BTreeSet<_>>();
        let mut seed_footprints = Vec::with_capacity(legal_seed_ids.len());
        for seed in legal_seed_ids.iter().copied().collect::<BTreeSet<_>>() {
            require_method_c_id("Method-C legalization preflight seed", seed, self.nmd)?;
            let seed_level = self.m_metadata[seed].mrlm;
            let mut footprint = self
                .method_c_rad3_faces_with_neighbors(seed, &method_c_m_neighbors)?
                .into_iter()
                .filter(|&iw| (2..=self.nwd).contains(&iw) && self.w_faces[iw].mrlw == seed_level)
                .collect::<Vec<_>>();
            footprint.sort_unstable();
            footprint.dedup();
            seed_footprints.push((seed, footprint));
        }
        let witness_candidate_seeds = witnesses
            .iter()
            .map(|witness| {
                seed_footprints
                    .iter()
                    .filter(|(_, footprint)| {
                        sorted_usize_slices_intersect(footprint, &witness.dependency_faces)
                    })
                    .map(|(seed, _)| *seed)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let witness_mutable_faces = witness_candidate_seeds
            .iter()
            .map(|candidates| {
                seed_footprints
                    .iter()
                    .filter(|(seed, _)| candidates.binary_search(seed).is_ok())
                    .flat_map(|(_, footprint)| footprint.iter().copied())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        for (witness, candidates) in witnesses.iter_mut().zip(&witness_candidate_seeds) {
            witness.candidate_seed_ids.clone_from(candidates);
        }
        let affected_perimeter_components = |faces: &[usize]| {
            let mut m_points = BTreeSet::new();
            let mut u_edges = BTreeSet::new();
            for &iw in faces {
                m_points.extend(self.w_faces[iw].im);
                u_edges.extend(self.w_faces[iw].iu);
            }
            perimeters
                .iter()
                .enumerate()
                .filter(|(_, perimeter)| {
                    perimeter
                        .iter()
                        .any(|point| m_points.contains(&point.im) || u_edges.contains(&point.iu))
                })
                .map(|(component, _)| component)
                .collect::<Vec<_>>()
        };
        let mut perimeter_candidate_seed_ids = vec![Vec::new(); perimeters.len()];
        for (seed, footprint) in &seed_footprints {
            for component in affected_perimeter_components(footprint) {
                perimeter_candidate_seed_ids[component].push(*seed);
            }
        }
        let witness_affected_perimeter_components = witness_mutable_faces
            .iter()
            .map(|faces| affected_perimeter_components(faces))
            .collect::<Vec<_>>();

        let mut unassigned = (0..witnesses.len()).collect::<BTreeSet<_>>();
        let mut witness_dependency_clusters = Vec::new();
        while let Some(start) = unassigned.pop_first() {
            let mut cluster = vec![start];
            let mut cursor = 0usize;
            while cursor < cluster.len() {
                let witness = &witnesses[cluster[cursor]];
                let connected = unassigned
                    .iter()
                    .copied()
                    .filter(|&candidate| {
                        let other = &witnesses[candidate];
                        witness.perimeter_component == other.perimeter_component
                            || sorted_usize_slices_intersect(
                                &witness.dependency_face_lineages,
                                &other.dependency_face_lineages,
                            )
                            || sorted_usize_slices_intersect(
                                &witness_candidate_seeds[cluster[cursor]],
                                &witness_candidate_seeds[candidate],
                            )
                            || sorted_usize_slices_intersect(
                                &witness_mutable_faces[cluster[cursor]],
                                &witness_mutable_faces[candidate],
                            )
                            || witness_affected_perimeter_components[cluster[cursor]]
                                .binary_search(&other.perimeter_component)
                                .is_ok()
                            || witness_affected_perimeter_components[candidate]
                                .binary_search(&witness.perimeter_component)
                                .is_ok()
                    })
                    .collect::<Vec<_>>();
                for candidate in connected {
                    unassigned.remove(&candidate);
                    cluster.push(candidate);
                }
                cursor += 1;
            }
            cluster.sort_unstable();
            witness_dependency_clusters.push(cluster);
        }

        let patches = witness_dependency_clusters
            .iter()
            .enumerate()
            .map(|(cluster_index, witness_indices)| {
                let witness_perimeter_components = witness_indices
                    .iter()
                    .map(|&index| witnesses[index].perimeter_component)
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();
                let dependency_faces = witness_indices
                    .iter()
                    .flat_map(|&index| witnesses[index].dependency_faces.iter().copied())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();
                let dependency_face_lineages = dependency_faces
                    .iter()
                    .map(|&iw| self.w_lineage[iw])
                    .collect::<Vec<_>>();
                let candidate_seed_ids = witness_indices
                    .iter()
                    .flat_map(|&index| witness_candidate_seeds[index].iter().copied())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();
                let candidate_seed_lineages = candidate_seed_ids
                    .iter()
                    .map(|&im| self.m_lineage[im])
                    .collect::<Vec<_>>();
                let selected_candidate_seed_ids = candidate_seed_ids
                    .iter()
                    .copied()
                    .filter(|seed| selected_seed_ids.contains(seed))
                    .collect::<Vec<_>>();
                let mut mutable_faces = BTreeSet::new();
                for seed in &candidate_seed_ids {
                    if let Ok(index) =
                        seed_footprints.binary_search_by_key(seed, |(candidate, _)| *candidate)
                    {
                        mutable_faces.extend(seed_footprints[index].1.iter().copied());
                    }
                }
                let mutable_faces = mutable_faces.into_iter().collect::<Vec<_>>();
                let mutable_face_lineages = mutable_faces
                    .iter()
                    .map(|&iw| self.w_lineage[iw])
                    .collect::<Vec<_>>();
                let perimeter_components = affected_perimeter_components(&mutable_faces);
                let perimeter_interfaces = perimeter_components
                    .iter()
                    .map(
                        |&component_index| MethodCHfieldPerimeterComponentCheckpoint {
                            component_index,
                            points: perimeters[component_index]
                                .iter()
                                .map(|&point| self.hfield_perimeter_point_checkpoint(point))
                                .collect(),
                        },
                    )
                    .collect();
                MethodCHfieldLegalizationPatch {
                    cluster_index,
                    witness_indices: witness_indices.clone(),
                    witness_perimeter_components,
                    perimeter_components,
                    perimeter_interfaces,
                    dependency_faces,
                    dependency_face_lineages,
                    candidate_seed_ids,
                    candidate_seed_lineages,
                    selected_candidate_seed_ids,
                    mutable_faces,
                    mutable_face_lineages,
                }
            })
            .collect();

        Ok(MethodCHfieldLegalizationPreflight {
            prepared_selected_faces: selected,
            perimeter_lengths,
            perimeter_remainders,
            perimeter_candidate_seed_ids,
            self_loop_witnesses: witnesses,
            witness_dependency_clusters,
            patches,
        })
    }

    /// Expand one legalization patch by one canonical stride-3 seed ring.
    pub fn expand_legalization_patch_one_ring(
        &self,
        checkpoint: &MethodCHfieldSelectionCheckpoint,
        preflight: &MethodCHfieldLegalizationPreflight,
        patch: &MethodCHfieldLegalizationPatch,
    ) -> io::Result<MethodCHfieldLegalizationPatch> {
        require_method_c_len(
            "Method-C legalization preflight selected faces",
            preflight.prepared_selected_faces.len(),
            self.nwd + 1,
        )?;
        let legal_seeds = checkpoint
            .legal_seed_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut candidate_seeds = patch
            .candidate_seed_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if !candidate_seeds.is_subset(&legal_seeds) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Method-C legalization patch contains a non-legal seed",
            ));
        }

        let m_neighbors = self.method_c_m_neighbors()?;
        let mut visited_directions = vec![[false; 6]; self.nmd + 1];
        for seed in candidate_seeds.iter().copied().collect::<Vec<_>>() {
            for neighbor in self.method_c_thirdm_neighbors_canonical_with_neighbors(
                seed,
                &mut visited_directions,
                &m_neighbors,
            )? {
                if legal_seeds.contains(&neighbor) {
                    candidate_seeds.insert(neighbor);
                }
            }
        }
        self.legalization_patch_with_candidate_seeds(checkpoint, preflight, patch, candidate_seeds)
    }

    /// Add every locally overlapping canonical rad3 placement, including
    /// phases not selected by the production component anchor.
    pub fn expand_legalization_patch_local_phases(
        &self,
        checkpoint: &MethodCHfieldSelectionCheckpoint,
        preflight: &MethodCHfieldLegalizationPreflight,
        patch: &MethodCHfieldLegalizationPatch,
    ) -> io::Result<MethodCHfieldLegalizationPatch> {
        let ring = self.expand_legalization_patch_one_ring(checkpoint, preflight, patch)?;
        let mutable_faces = ring.mutable_faces.iter().copied().collect::<BTreeSet<_>>();
        let parent_levels = ring
            .mutable_faces
            .iter()
            .copied()
            .filter(|&iw| preflight.prepared_selected_faces[iw])
            .map(|iw| self.w_faces[iw].mrlw)
            .collect::<BTreeSet<_>>();
        let Some(&parent_level) = parent_levels.iter().next() else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Method-C legalization patch has no selected parent face",
            ));
        };
        if parent_levels.len() != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Method-C legalization patch crosses parent refinement levels",
            ));
        }

        let m_neighbors = self.method_c_m_neighbors()?;
        let mut candidate_seeds = ring
            .candidate_seed_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        for im in 2..=self.nmd {
            let Ok(faces) = self.method_c_rad3_faces_with_neighbors(im, &m_neighbors) else {
                continue;
            };
            if faces.iter().any(|iw| mutable_faces.contains(iw))
                && faces
                    .iter()
                    .all(|&iw| iw > 1 && self.w_faces[iw].mrlw == parent_level)
            {
                candidate_seeds.insert(im);
            }
        }
        self.legalization_patch_with_candidate_seeds(checkpoint, preflight, patch, candidate_seeds)
    }

    fn legalization_patch_with_candidate_seeds(
        &self,
        checkpoint: &MethodCHfieldSelectionCheckpoint,
        preflight: &MethodCHfieldLegalizationPreflight,
        patch: &MethodCHfieldLegalizationPatch,
        candidate_seeds: BTreeSet<usize>,
    ) -> io::Result<MethodCHfieldLegalizationPatch> {
        let candidate_seed_ids = candidate_seeds.into_iter().collect::<Vec<_>>();
        let m_neighbors = self.method_c_m_neighbors()?;
        let mutable_mask = self.selected_faces_from_method_c_seed_ids_with_neighbors(
            &candidate_seed_ids,
            &m_neighbors,
        )?;
        let mutable_faces = mutable_mask
            .iter()
            .enumerate()
            .skip(2)
            .filter_map(|(iw, &selected)| selected.then_some(iw))
            .collect::<Vec<_>>();
        let mutable_m_points = mutable_faces
            .iter()
            .flat_map(|&iw| self.w_faces[iw].im)
            .collect::<BTreeSet<_>>();
        let mutable_u_edges = mutable_faces
            .iter()
            .flat_map(|&iw| self.w_faces[iw].iu)
            .collect::<BTreeSet<_>>();
        let perimeters = self.method_c_perimeters_from_selected_faces(
            &preflight.prepared_selected_faces,
            &m_neighbors,
        )?;
        let perimeter_components = perimeters
            .iter()
            .enumerate()
            .filter(|(_, perimeter)| {
                perimeter.iter().any(|point| {
                    mutable_m_points.contains(&point.im) || mutable_u_edges.contains(&point.iu)
                })
            })
            .map(|(component, _)| component)
            .collect::<Vec<_>>();
        let perimeter_interfaces = perimeter_components
            .iter()
            .map(
                |&component_index| MethodCHfieldPerimeterComponentCheckpoint {
                    component_index,
                    points: perimeters[component_index]
                        .iter()
                        .map(|&point| self.hfield_perimeter_point_checkpoint(point))
                        .collect(),
                },
            )
            .collect();
        let selected_seed_ids = checkpoint
            .selected_seed_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();

        Ok(MethodCHfieldLegalizationPatch {
            cluster_index: patch.cluster_index,
            witness_indices: patch.witness_indices.clone(),
            witness_perimeter_components: patch.witness_perimeter_components.clone(),
            perimeter_components,
            perimeter_interfaces,
            dependency_faces: patch.dependency_faces.clone(),
            dependency_face_lineages: patch.dependency_face_lineages.clone(),
            candidate_seed_lineages: candidate_seed_ids
                .iter()
                .map(|&im| self.m_lineage[im])
                .collect(),
            selected_candidate_seed_ids: candidate_seed_ids
                .iter()
                .copied()
                .filter(|seed| selected_seed_ids.contains(seed))
                .collect(),
            mutable_face_lineages: mutable_faces.iter().map(|&iw| self.w_lineage[iw]).collect(),
            candidate_seed_ids,
            mutable_faces,
        })
    }

    /// Check that exact shared mask preparation neither changes a face outside
    /// the patch nor changes the fixed portion of an affected perimeter.
    fn legalization_symbolic_state_with_neighbors(
        &self,
        checkpoint: &MethodCHfieldSelectionCheckpoint,
        selected_seed_ids: &[usize],
        child_level: usize,
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<(
        Vec<bool>,
        Vec<Vec<MethodCPerimeterPoint>>,
        MethodCHfieldLegalizationSymbolicCheck,
    )> {
        if child_level <= 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C legalization child level must be greater than one",
            ));
        }
        let selected_seeds = selected_seed_ids.iter().copied().collect::<BTreeSet<_>>();
        if selected_seeds.len() != selected_seed_ids.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C legalization seed assignment must contain unique seed IDs",
            ));
        }
        for &seed in &selected_seeds {
            require_method_c_id("Method-C legalization seed", seed, self.nmd)?;
            self.method_c_rad3_faces_with_neighbors(seed, m_neighbors)?;
        }

        let mut selected = self
            .selected_faces_from_method_c_seed_ids_with_neighbors(selected_seed_ids, m_neighbors)?;
        self.close_method_c_concavities_for_level_with_neighbors(&mut selected, m_neighbors)?;
        self.ensure_method_c_selected_faces_share_parent_mrlw(&selected, child_level)?;
        checkpoint.validate_demand_coverage(&selected)?;
        let perimeters = self
            .method_c_perimeters_from_selected_faces(&selected, m_neighbors)
            .map_err(|error| {
                io::Error::new(
                    error.kind(),
                    MethodCHfieldPerimeterTopologyError(error.to_string()),
                )
            })?;
        let perimeter_lengths = perimeters.iter().map(Vec::len).collect::<Vec<_>>();
        let perimeter_remainders = perimeter_lengths
            .iter()
            .map(|length| length % 3)
            .collect::<Vec<_>>();
        let predicted_transition_self_loop_count =
            if perimeter_remainders.iter().all(|&remainder| remainder == 0) {
                let perimeter = perimeters.iter().flatten().copied().collect::<Vec<_>>();
                let nest_wd =
                    self.method_c_nest_wd_from_selected_and_perimeter(&selected, &perimeter)?;
                Some(
                    self.method_c_transition_self_loop_witnesses(&perimeter, &nest_wd)?
                        .len(),
                )
            } else {
                None
            };
        let report = MethodCHfieldLegalizationSymbolicCheck {
            selected_face_count: selected.iter().skip(2).filter(|&&face| face).count(),
            perimeter_lengths,
            perimeter_remainders,
            vertex_only_contact_count: self
                .method_c_vertex_only_perimeter_contacts(&selected, m_neighbors)?
                .len(),
            predicted_transition_self_loop_count,
        };
        Ok((selected, perimeters, report))
    }

    /// Evaluate the exact pre-materialization topology predicates for one
    /// complete canonical seed assignment without invoking the emitter.
    pub fn legalization_symbolic_check(
        &self,
        checkpoint: &MethodCHfieldSelectionCheckpoint,
        selected_seed_ids: &[usize],
        child_level: usize,
    ) -> io::Result<MethodCHfieldLegalizationSymbolicCheck> {
        let m_neighbors = self.method_c_m_neighbors()?;
        self.legalization_symbolic_state_with_neighbors(
            checkpoint,
            selected_seed_ids,
            child_level,
            &m_neighbors,
        )
        .map(|(_, _, report)| report)
    }

    /// Materialize one complete canonical seed assignment after the shared
    /// symbolic hard checks.
    pub fn materialize_legalization_seed_assignment(
        &self,
        checkpoint: &MethodCHfieldSelectionCheckpoint,
        selected_seed_ids: &[usize],
        child_level: usize,
        max_mrows: usize,
    ) -> io::Result<Self> {
        let m_neighbors = self.method_c_m_neighbors()?;
        let (selected, _, symbolic) = self.legalization_symbolic_state_with_neighbors(
            checkpoint,
            selected_seed_ids,
            child_level,
            &m_neighbors,
        )?;
        if symbolic.predicted_transition_self_loop_count.is_none() {
            return Err(method_c_repairable_perimeter_error(
                MethodCRepairableKind::NonTripletPerimeter,
                symbolic.perimeter_lengths,
                0,
                "Method-C legalization assignment cannot be materialized because a perimeter is not triplet-aligned",
            ));
        }
        self.spawn_nest_pass_method_c_without_mask_repair(&selected, child_level, max_mrows, true)
    }

    /// Check one complete canonical seed assignment with the production
    /// materializer without retaining the child mesh.
    pub fn legalization_exact_materialization_check(
        &self,
        checkpoint: &MethodCHfieldSelectionCheckpoint,
        selected_seed_ids: &[usize],
        child_level: usize,
        max_mrows: usize,
    ) -> io::Result<()> {
        self.materialize_legalization_seed_assignment(
            checkpoint,
            selected_seed_ids,
            child_level,
            max_mrows,
        )
        .map(|_| ())
    }

    /// Compile a complete, bounded patch truth table from the existing exact
    /// boundary and materialization checks.
    ///
    /// This is an offline/prototype path. It deliberately refuses wider
    /// patches instead of turning exponential enumeration into a production
    /// fallback.
    #[doc(hidden)]
    pub fn compile_bounded_exact_legalization_patch_table_for_diagnostics(
        &self,
        checkpoint: &MethodCHfieldSelectionCheckpoint,
        preflight: &MethodCHfieldLegalizationPreflight,
        patch: &MethodCHfieldLegalizationPatch,
        child_level: usize,
        max_mrows: usize,
        max_variables: usize,
    ) -> io::Result<MethodCHfieldExactPatchTableCompilation> {
        const HARD_VARIABLE_LIMIT: usize = 20;

        let candidate_count = patch.candidate_seed_ids.len();
        if patch
            .candidate_seed_ids
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C exact patch-table candidate seeds must be strictly increasing",
            ));
        }
        let current_perimeter_scope_candidate_seed_ids =
            preflight.current_perimeter_candidate_scope(&patch.perimeter_components)?;
        let covers_current_perimeter_scope = current_perimeter_scope_candidate_seed_ids
            .as_ref()
            .is_some_and(|scope| {
                scope
                    .iter()
                    .all(|seed| patch.candidate_seed_ids.binary_search(seed).is_ok())
            });
        let ordered_perimeter_scope_analyses = self
            .analyze_ordered_perimeter_scope_for_diagnostics(
                checkpoint,
                preflight,
                patch,
                child_level,
            )?;
        let candidate_seeds = patch
            .candidate_seed_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let fixed_seed_ids = checkpoint
            .selected_seed_ids
            .iter()
            .copied()
            .filter(|seed| !candidate_seeds.contains(seed))
            .collect::<Vec<_>>();
        let maximal_seed_ids = fixed_seed_ids
            .iter()
            .copied()
            .chain(patch.candidate_seed_ids.iter().copied())
            .collect::<Vec<_>>();
        let m_neighbors = self.method_c_m_neighbors()?;
        let covered_anchor_count = |selected_faces: &[bool]| {
            checkpoint
                .demand_anchors
                .iter()
                .filter(|anchor| {
                    anchor
                        .candidate_faces
                        .iter()
                        .any(|&iw| selected_faces.get(iw).copied().unwrap_or(false))
                })
                .count()
        };
        let fixed_direct = self
            .selected_faces_from_method_c_seed_ids_with_neighbors(&fixed_seed_ids, &m_neighbors)?;
        let mut fixed_closed = fixed_direct.clone();
        self.close_method_c_concavities_for_level_with_neighbors(&mut fixed_closed, &m_neighbors)?;
        let maximal_direct = self.selected_faces_from_method_c_seed_ids_with_neighbors(
            &maximal_seed_ids,
            &m_neighbors,
        )?;
        let mut maximal_closed = maximal_direct.clone();
        self.close_method_c_concavities_for_level_with_neighbors(
            &mut maximal_closed,
            &m_neighbors,
        )?;
        let demand_anchor_count = checkpoint.demand_anchors.len();
        let fixed_direct_covered_demand_anchors = covered_anchor_count(&fixed_direct);
        let fixed_closed_covered_demand_anchors = covered_anchor_count(&fixed_closed);
        let maximal_direct_covered_demand_anchors = covered_anchor_count(&maximal_direct);
        let maximal_closed_covered_demand_anchors = covered_anchor_count(&maximal_closed);
        let mut direct_candidate_support_scopes =
            vec![BTreeSet::<usize>::new(); demand_anchor_count];
        for (variable, &seed) in patch.candidate_seed_ids.iter().enumerate() {
            let seed_level = self.m_metadata[seed].mrlm;
            let footprint = self
                .method_c_rad3_faces_with_neighbors(seed, &m_neighbors)?
                .into_iter()
                .filter(|&iw| (2..=self.nwd).contains(&iw) && self.w_faces[iw].mrlw == seed_level)
                .collect::<BTreeSet<_>>();
            for (anchor, support_scope) in checkpoint
                .demand_anchors
                .iter()
                .zip(&mut direct_candidate_support_scopes)
            {
                if anchor
                    .candidate_faces
                    .iter()
                    .any(|iw| footprint.contains(iw))
                {
                    support_scope.insert(variable);
                }
            }
        }
        let fixed_uncovered_support_scopes = checkpoint
            .demand_anchors
            .iter()
            .zip(&direct_candidate_support_scopes)
            .filter_map(|(anchor, support_scope)| {
                (!anchor
                    .candidate_faces
                    .iter()
                    .any(|&iw| fixed_direct.get(iw).copied().unwrap_or(false)))
                .then_some(support_scope.clone())
            })
            .collect::<Vec<_>>();
        let distinct_direct_candidate_support_scope_count = checkpoint
            .demand_anchors
            .iter()
            .zip(direct_candidate_support_scopes)
            .filter_map(|(anchor, support_scope)| {
                (!anchor
                    .candidate_faces
                    .iter()
                    .any(|&iw| fixed_direct.get(iw).copied().unwrap_or(false)))
                .then_some(support_scope)
            })
            .collect::<BTreeSet<_>>()
            .len();
        let fixed_uncovered_support_counts = fixed_uncovered_support_scopes
            .iter()
            .map(BTreeSet::len)
            .collect::<Vec<_>>();
        let fixed_uncovered_demand_anchors = fixed_uncovered_support_counts.len();
        let direct_unsupported_demand_anchors = fixed_uncovered_support_counts
            .iter()
            .filter(|&&count| count == 0)
            .count();
        let min_direct_candidate_support_count = fixed_uncovered_support_counts
            .iter()
            .copied()
            .min()
            .unwrap_or(0);
        let max_direct_candidate_support_count = fixed_uncovered_support_counts
            .iter()
            .copied()
            .max()
            .unwrap_or(0);
        let direct_coverage_clause_satisfying_assignments =
            (candidate_count <= HARD_VARIABLE_LIMIT && candidate_count < usize::BITS as usize)
                .then(|| {
                    (0..(1usize << candidate_count))
                        .filter(|assignment| {
                            fixed_uncovered_support_scopes.iter().all(|scope| {
                                scope
                                    .iter()
                                    .any(|variable| assignment & (1usize << variable) != 0)
                            })
                        })
                        .count()
                });
        let bounded = candidate_count <= max_variables
            && candidate_count <= HARD_VARIABLE_LIMIT
            && candidate_count < usize::BITS as usize;
        if !bounded {
            return Ok(MethodCHfieldExactPatchTableCompilation {
                status: MethodCHfieldExactPatchTableStatus::Incomplete,
                candidate_seed_ids: patch.candidate_seed_ids.clone(),
                demand_anchor_count,
                fixed_direct_covered_demand_anchors,
                fixed_closed_covered_demand_anchors,
                maximal_direct_covered_demand_anchors,
                maximal_closed_covered_demand_anchors,
                fixed_uncovered_demand_anchors,
                direct_unsupported_demand_anchors,
                distinct_direct_candidate_support_scope_count,
                min_direct_candidate_support_count,
                max_direct_candidate_support_count,
                direct_coverage_clause_satisfying_assignments,
                total_assignments: None,
                evaluated_assignments: 0,
                sat_assignments: 0,
                boundary_incomplete_assignments: 0,
                hard_rejected_assignments: BTreeMap::new(),
                exact_failure_assignments: BTreeMap::new(),
                unclassified_error_assignments: 0,
                first_unclassified_error: None,
                triplet_assignment_count: 0,
                distinct_exact_state_count: 0,
                max_exact_state_multiplicity: 0,
                mixed_exact_outcome_state_count: 0,
                current_perimeter_scope_candidate_seed_ids,
                covers_current_perimeter_scope,
                ordered_perimeter_scope_analyses,
                table: None,
                propagation: None,
                system_analysis: None,
                assignment_outcome_records: Vec::new(),
            });
        }

        let total_assignments = 1usize << candidate_count;
        let mut sat_rows = Vec::new();
        let mut boundary_incomplete_assignments = 0usize;
        let mut hard_rejected_assignments = BTreeMap::new();
        let mut exact_failure_assignments = BTreeMap::new();
        let mut unclassified_error_assignments = 0usize;
        let mut first_unclassified_error = None;
        let mut exact_states = BTreeMap::<
            (Vec<usize>, Vec<MethodCHfieldPerimeterComponentCheckpoint>),
            BTreeMap<String, usize>,
        >::new();
        type AssignmentDumpRow = (
            usize,
            String,
            Option<(Vec<usize>, Vec<MethodCHfieldPerimeterComponentCheckpoint>)>,
        );
        let mut assignment_dump: Option<Vec<AssignmentDumpRow>> =
            std::env::var_os("EARTHMESH_M0_LEGALIZATION_ASSIGNMENT_DUMP")
                .is_some()
                .then(Vec::new);

        for value_mask in 0..total_assignments {
            let row = (0..candidate_count)
                .map(|bit| value_mask & (1usize << bit) != 0)
                .collect::<Vec<_>>();
            let assignment = patch
                .candidate_seed_ids
                .iter()
                .zip(&row)
                .filter_map(|(&seed, &selected)| selected.then_some(seed))
                .collect::<Vec<_>>();
            let result = self.legalization_patch_boundary_check(
                checkpoint,
                preflight,
                patch,
                &assignment,
                child_level,
                max_mrows,
            );
            if let Ok(check) = &result {
                let outcome = if !check.is_closed() {
                    "boundary_incomplete".to_string()
                } else if check.exact_materializable {
                    "sat".to_string()
                } else {
                    check.exact_failure_kind.map_or_else(
                        || "exact:unknown".to_string(),
                        |kind| format!("exact:{}", kind.as_str()),
                    )
                };
                *exact_states
                    .entry((
                        check.selected_face_ids.clone(),
                        check.ordered_perimeter_components.clone(),
                    ))
                    .or_default()
                    .entry(outcome)
                    .or_default() += 1;
            }
            if let Some(dump) = assignment_dump.as_mut() {
                let row = match &result {
                    Ok(check) => {
                        let outcome = if !check.is_closed() {
                            "boundary_incomplete".to_string()
                        } else if check.exact_materializable {
                            "sat".to_string()
                        } else {
                            check.exact_failure_kind.map_or_else(
                                || "exact:unknown".to_string(),
                                |kind| format!("exact:{}", kind.as_str()),
                            )
                        };
                        (
                            value_mask,
                            outcome,
                            Some((
                                check.selected_face_ids.clone(),
                                check.ordered_perimeter_components.clone(),
                            )),
                        )
                    }
                    Err(error) => {
                        let kind = method_c_hfield_failure_kind(error);
                        let outcome = if kind == MethodCHfieldFailureKind::Other {
                            "unclassified".to_string()
                        } else {
                            format!("hard:{}", kind.as_str())
                        };
                        (value_mask, outcome, None)
                    }
                };
                dump.push(row);
            }
            match result {
                Ok(check) if !check.is_closed() => boundary_incomplete_assignments += 1,
                Ok(check) if check.exact_materializable => sat_rows.push(row),
                Ok(check) => {
                    let kind = check
                        .exact_failure_kind
                        .map_or_else(|| "unknown".to_string(), |kind| kind.as_str().to_string());
                    *exact_failure_assignments.entry(kind).or_default() += 1;
                }
                Err(error) => {
                    let kind = method_c_hfield_failure_kind(&error);
                    if kind == MethodCHfieldFailureKind::Other {
                        unclassified_error_assignments += 1;
                        first_unclassified_error.get_or_insert_with(|| error.to_string());
                    } else {
                        *hard_rejected_assignments
                            .entry(kind.as_str().to_string())
                            .or_default() += 1;
                    }
                }
            }
        }
        let triplet_assignment_count = exact_states
            .values()
            .flat_map(BTreeMap::values)
            .copied()
            .sum();
        let distinct_exact_state_count = exact_states.len();
        let max_exact_state_multiplicity = exact_states
            .values()
            .map(|outcomes| outcomes.values().copied().sum())
            .max()
            .unwrap_or(0);
        let mixed_exact_outcome_state_count = exact_states
            .values()
            .filter(|outcomes| outcomes.len() > 1)
            .count();
        // `exact_states` is a BTreeMap, so its iteration order — and therefore
        // every recorded ordinal — is deterministic across runs.
        let assignment_outcome_records = assignment_dump
            .map(|dump| {
                let ordinals = exact_states
                    .keys()
                    .enumerate()
                    .map(|(ordinal, key)| (key.clone(), ordinal))
                    .collect::<BTreeMap<_, _>>();
                dump.into_iter()
                    .map(
                        |(value_mask, outcome, key)| MethodCHfieldAssignmentOutcomeRecord {
                            value_mask,
                            outcome,
                            exact_state_ordinal: key.and_then(|key| ordinals.get(&key).copied()),
                        },
                    )
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let (status, table, propagation, system_analysis) = if sat_rows.is_empty() {
            let status = if boundary_incomplete_assignments == 0
                && hard_rejected_assignments.is_empty()
                && unclassified_error_assignments == 0
                && covers_current_perimeter_scope
            {
                MethodCHfieldExactPatchTableStatus::PatchUnsat
            } else {
                MethodCHfieldExactPatchTableStatus::Incomplete
            };
            (status, None, None, None)
        } else {
            let table =
                MethodCBinaryTableConstraint::from_rows((0..candidate_count).collect(), &sat_rows)?;
            let system_analysis = analyze_method_c_binary_table_system(
                candidate_count,
                std::slice::from_ref(&table),
                None,
            )?;
            (
                MethodCHfieldExactPatchTableStatus::Sat,
                Some(table),
                Some(system_analysis.propagation.clone()),
                Some(system_analysis),
            )
        };

        Ok(MethodCHfieldExactPatchTableCompilation {
            status,
            candidate_seed_ids: patch.candidate_seed_ids.clone(),
            demand_anchor_count,
            fixed_direct_covered_demand_anchors,
            fixed_closed_covered_demand_anchors,
            maximal_direct_covered_demand_anchors,
            maximal_closed_covered_demand_anchors,
            fixed_uncovered_demand_anchors,
            direct_unsupported_demand_anchors,
            distinct_direct_candidate_support_scope_count,
            min_direct_candidate_support_count,
            max_direct_candidate_support_count,
            direct_coverage_clause_satisfying_assignments,
            total_assignments: Some(total_assignments),
            evaluated_assignments: total_assignments,
            sat_assignments: sat_rows.len(),
            boundary_incomplete_assignments,
            hard_rejected_assignments,
            exact_failure_assignments,
            unclassified_error_assignments,
            first_unclassified_error,
            triplet_assignment_count,
            distinct_exact_state_count,
            max_exact_state_multiplicity,
            mixed_exact_outcome_state_count,
            current_perimeter_scope_candidate_seed_ids,
            covers_current_perimeter_scope,
            ordered_perimeter_scope_analyses,
            table,
            propagation,
            system_analysis,
            assignment_outcome_records,
        })
    }

    fn analyze_ordered_perimeter_scope_for_diagnostics(
        &self,
        checkpoint: &MethodCHfieldSelectionCheckpoint,
        preflight: &MethodCHfieldLegalizationPreflight,
        patch: &MethodCHfieldLegalizationPatch,
        child_level: usize,
    ) -> io::Result<Vec<MethodCHfieldOrderedPerimeterScopeAnalysis>> {
        if preflight
            .current_perimeter_candidate_scope(&patch.perimeter_components)?
            .is_none()
        {
            return Ok(Vec::new());
        }
        if patch.perimeter_interfaces.len() != patch.perimeter_components.len()
            || patch
                .perimeter_interfaces
                .iter()
                .zip(&patch.perimeter_components)
                .any(|(interface, component)| interface.component_index != *component)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Method-C ordered perimeter interfaces do not match the requested components",
            ));
        }

        let m_neighbors = self.method_c_m_neighbors()?;
        let mut analyses = Vec::with_capacity(patch.perimeter_interfaces.len());
        for interface in &patch.perimeter_interfaces {
            const PROJECTED_UNION_STATE_CAP: usize = 100_000;
            const CLOSURE_PREFIX_VARIABLE_LIMIT: usize = 12;

            let candidates = &preflight.perimeter_candidate_seed_ids[interface.component_index];
            let candidate_set = candidates.iter().copied().collect::<BTreeSet<_>>();
            let fixed_seed_ids = checkpoint
                .selected_seed_ids
                .iter()
                .copied()
                .filter(|seed| !candidate_set.contains(seed))
                .collect::<Vec<_>>();
            let mut point_variables = vec![Vec::new(); interface.points.len()];
            let local_face_ids = interface
                .points
                .iter()
                .map(|point| {
                    let neighbors = m_neighbors[point.parent_m_point];
                    neighbors.iw[..neighbors.npoly].to_vec()
                })
                .collect::<Vec<_>>();
            let projected_interface_faces = local_face_ids
                .iter()
                .flatten()
                .copied()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let projected_face_slots = projected_interface_faces
                .iter()
                .enumerate()
                .map(|(slot, &iw)| (iw, slot))
                .collect::<BTreeMap<_, _>>();
            let projected_word_count = projected_interface_faces.len().div_ceil(u64::BITS as usize);
            let mut point_footprint_masks = vec![Vec::new(); interface.points.len()];
            let mut projected_candidate_masks = Vec::with_capacity(candidates.len());
            let mut candidate_footprints = Vec::with_capacity(candidates.len());
            for (variable, &seed) in candidates.iter().enumerate() {
                let seed_level = self.m_metadata[seed].mrlm;
                let footprint = self
                    .method_c_rad3_faces_with_neighbors(seed, &m_neighbors)?
                    .into_iter()
                    .filter(|&iw| {
                        (2..=self.nwd).contains(&iw) && self.w_faces[iw].mrlw == seed_level
                    })
                    .collect::<BTreeSet<_>>();
                candidate_footprints.push(footprint.clone());
                let mut projected_mask = vec![0u64; projected_word_count];
                for iw in &footprint {
                    let Some(&slot) = projected_face_slots.get(iw) else {
                        continue;
                    };
                    projected_mask[slot / u64::BITS as usize] |=
                        1u64 << (slot % u64::BITS as usize);
                }
                projected_candidate_masks.push(projected_mask);
                let mut touched = false;
                for ((local_faces, variables), masks) in local_face_ids
                    .iter()
                    .zip(&mut point_variables)
                    .zip(&mut point_footprint_masks)
                {
                    let mask = local_faces
                        .iter()
                        .enumerate()
                        .fold(0u8, |mask, (slot, iw)| {
                            mask | (u8::from(footprint.contains(iw)) << slot)
                        });
                    if mask != 0 {
                        variables.push(variable);
                        masks.push(mask);
                        touched = true;
                    }
                }
                if !touched {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Method-C perimeter candidate census contains a seed with no ordered-interface incidence",
                    ));
                }
            }
            let count_union_states = |candidate_masks: &[Vec<u64>]| {
                let mut states =
                    BTreeSet::from([vec![0u64; candidate_masks.first().map_or(0, Vec::len)]]);
                let mut cap_exceeded_after_variables = None;
                for (variable, candidate_mask) in candidate_masks.iter().enumerate() {
                    let states_before = states.iter().cloned().collect::<Vec<_>>();
                    for mut state in states_before {
                        for (word, candidate) in state.iter_mut().zip(candidate_mask) {
                            *word |= candidate;
                        }
                        states.insert(state);
                        if states.len() > PROJECTED_UNION_STATE_CAP {
                            cap_exceeded_after_variables = Some(variable + 1);
                            break;
                        }
                    }
                    if cap_exceeded_after_variables.is_some() {
                        break;
                    }
                }
                (
                    cap_exceeded_after_variables
                        .is_none()
                        .then_some(states.len()),
                    cap_exceeded_after_variables,
                )
            };
            let (
                projected_direct_union_state_count,
                projected_direct_union_state_cap_exceeded_after_variables,
            ) = count_union_states(&projected_candidate_masks);
            let candidate_footprint_faces = candidate_footprints
                .iter()
                .flatten()
                .copied()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let candidate_footprint_face_slots = candidate_footprint_faces
                .iter()
                .enumerate()
                .map(|(slot, &iw)| (iw, slot))
                .collect::<BTreeMap<_, _>>();
            let candidate_footprint_word_count =
                candidate_footprint_faces.len().div_ceil(u64::BITS as usize);
            let candidate_footprint_masks = candidate_footprints
                .iter()
                .map(|footprint| {
                    let mut mask = vec![0u64; candidate_footprint_word_count];
                    for iw in footprint {
                        let slot = candidate_footprint_face_slots[iw];
                        mask[slot / u64::BITS as usize] |= 1u64 << (slot % u64::BITS as usize);
                    }
                    mask
                })
                .collect::<Vec<_>>();
            let (
                candidate_footprint_union_state_count,
                candidate_footprint_union_state_cap_exceeded_after_variables,
            ) = count_union_states(&candidate_footprint_masks);
            let closure_prefix_variable_count = candidates.len().min(CLOSURE_PREFIX_VARIABLE_LIMIT);
            let closure_prefix_assignment_count = 1usize << closure_prefix_variable_count;
            let mut base_closed = self.selected_faces_from_method_c_seed_ids_with_neighbors(
                &fixed_seed_ids,
                &m_neighbors,
            )?;
            self.ensure_method_c_selected_faces_share_parent_mrlw(&base_closed, child_level)?;
            self.close_method_c_concavities_for_level_with_neighbors(
                &mut base_closed,
                &m_neighbors,
            )?;
            let mut closure_prefix_direct_masks = BTreeSet::new();
            let mut closure_prefix_closed_masks = BTreeMap::<Vec<usize>, usize>::new();
            for assignment in 0..closure_prefix_assignment_count {
                let mut seed_ids = fixed_seed_ids.clone();
                seed_ids.extend(
                    candidates
                        .iter()
                        .take(closure_prefix_variable_count)
                        .enumerate()
                        .filter_map(|(variable, &seed)| {
                            (assignment & (1usize << variable) != 0).then_some(seed)
                        }),
                );
                seed_ids.sort_unstable();
                seed_ids.dedup();
                let direct = self.selected_faces_from_method_c_seed_ids_with_neighbors(
                    &seed_ids,
                    &m_neighbors,
                )?;
                self.ensure_method_c_selected_faces_share_parent_mrlw(&direct, child_level)?;
                closure_prefix_direct_masks.insert(
                    direct
                        .iter()
                        .enumerate()
                        .skip(2)
                        .filter_map(|(iw, &selected)| selected.then_some(iw))
                        .collect::<Vec<_>>(),
                );
                let mut closed = direct;
                self.close_method_c_concavities_for_level_with_neighbors(
                    &mut closed,
                    &m_neighbors,
                )?;
                *closure_prefix_closed_masks
                    .entry(
                        closed
                            .iter()
                            .enumerate()
                            .skip(2)
                            .filter_map(|(iw, &selected)| {
                                (selected && !base_closed[iw]).then_some(iw)
                            })
                            .collect::<Vec<_>>(),
                    )
                    .or_default() += 1;
            }
            let closure_prefix_distinct_direct_mask_count = closure_prefix_direct_masks.len();
            let closure_prefix_distinct_closed_mask_count = closure_prefix_closed_masks.len();
            let closure_prefix_max_closed_mask_multiplicity = closure_prefix_closed_masks
                .values()
                .copied()
                .max()
                .unwrap_or(0);
            let mut closure_normalized_states = BTreeSet::from([Vec::<usize>::new()]);
            for footprint in candidate_footprints
                .iter()
                .take(closure_prefix_variable_count)
            {
                let states_before = closure_normalized_states
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>();
                for state in states_before {
                    let mut selected = base_closed.clone();
                    for iw in state {
                        selected[iw] = true;
                    }
                    for &iw in footprint {
                        selected[iw] = true;
                    }
                    self.ensure_method_c_selected_faces_share_parent_mrlw(&selected, child_level)?;
                    self.close_method_c_concavities_for_level_with_neighbors(
                        &mut selected,
                        &m_neighbors,
                    )?;
                    closure_normalized_states.insert(
                        selected
                            .iter()
                            .enumerate()
                            .skip(2)
                            .filter_map(|(iw, &selected)| {
                                (selected && !base_closed[iw]).then_some(iw)
                            })
                            .collect(),
                    );
                }
            }
            let closure_incremental_prefix_parity =
                closure_normalized_states == closure_prefix_closed_masks.keys().cloned().collect();
            let analysis = analyze_method_c_cyclic_binary_scope(&point_variables)?;
            analyses.push(MethodCHfieldOrderedPerimeterScopeAnalysis {
                component_index: interface.component_index,
                perimeter_point_count: analysis.point_count,
                candidate_seed_count: analysis.variable_count,
                point_seed_incidences: analysis.point_variable_incidences,
                max_point_candidate_seed_count: analysis.max_point_variable_count,
                distinct_incidence_signature_count: analysis.distinct_incidence_signature_count,
                max_incidence_signature_multiplicity: analysis.max_incidence_signature_multiplicity,
                max_local_ring_face_count: local_face_ids.iter().map(Vec::len).max().unwrap_or(0),
                max_distinct_local_footprint_mask_count: point_footprint_masks
                    .iter()
                    .map(|masks| masks.iter().copied().collect::<BTreeSet<_>>().len())
                    .max()
                    .unwrap_or(0),
                max_local_union_state_count: point_footprint_masks
                    .iter()
                    .map(|masks| count_method_c_u8_union_states(masks))
                    .max()
                    .unwrap_or(0),
                projected_interface_face_count: projected_interface_faces.len(),
                projected_direct_union_state_cap: PROJECTED_UNION_STATE_CAP,
                projected_direct_union_state_count,
                projected_direct_union_state_cap_exceeded_after_variables,
                candidate_footprint_face_count: candidate_footprint_faces.len(),
                candidate_footprint_union_state_count,
                candidate_footprint_union_state_cap_exceeded_after_variables,
                closure_prefix_variable_count,
                closure_prefix_assignment_count,
                closure_prefix_distinct_direct_mask_count,
                closure_prefix_distinct_closed_mask_count,
                closure_prefix_max_closed_mask_multiplicity,
                closure_incremental_prefix_parity,
                best_cut_point: analysis.best_cut_point,
                min_linearized_frontier_width: analysis.min_linearized_frontier_width,
            });
        }
        Ok(analyses)
    }

    /// Check that exact shared mask preparation neither changes a face outside
    /// the patch nor changes the fixed portion of an affected perimeter.
    pub fn legalization_patch_boundary_check(
        &self,
        checkpoint: &MethodCHfieldSelectionCheckpoint,
        preflight: &MethodCHfieldLegalizationPreflight,
        patch: &MethodCHfieldLegalizationPatch,
        selected_candidate_seed_ids: &[usize],
        child_level: usize,
        max_mrows: usize,
    ) -> io::Result<MethodCHfieldLegalizationPatchBoundaryCheck> {
        if child_level <= 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C legalization patch child level must be greater than one",
            ));
        }
        require_method_c_len(
            "Method-C legalization checkpoint selected faces",
            checkpoint.selected_faces.len(),
            self.nwd + 1,
        )?;
        require_method_c_len(
            "Method-C legalization preflight selected faces",
            preflight.prepared_selected_faces.len(),
            self.nwd + 1,
        )?;

        let candidate_seeds = patch
            .candidate_seed_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let selected_candidates = selected_candidate_seed_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if selected_candidates.len() != selected_candidate_seed_ids.len()
            || !selected_candidates.is_subset(&candidate_seeds)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C legalization assignment must be a unique subset of patch candidate seeds",
            ));
        }
        let legal_seeds = checkpoint
            .legal_seed_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let m_neighbors = self.method_c_m_neighbors()?;
        for &seed in &candidate_seeds {
            require_method_c_id("Method-C legalization candidate seed", seed, self.nmd)?;
            if !legal_seeds.contains(&seed) {
                self.method_c_rad3_faces_with_neighbors(seed, &m_neighbors)?;
            }
        }

        let mut trial_seeds = checkpoint
            .selected_seed_ids
            .iter()
            .copied()
            .filter(|seed| !candidate_seeds.contains(seed))
            .chain(selected_candidates)
            .collect::<Vec<_>>();
        trial_seeds.sort_unstable();
        trial_seeds.dedup();

        let (trial, trial_perimeters, symbolic) = self.legalization_symbolic_state_with_neighbors(
            checkpoint,
            &trial_seeds,
            child_level,
            &m_neighbors,
        )?;

        let mutable_faces = patch.mutable_faces.iter().copied().collect::<BTreeSet<_>>();
        let mut mutable_m_points = BTreeSet::new();
        let mut mutable_u_edges = BTreeSet::new();
        for &iw in &mutable_faces {
            require_method_c_id("Method-C legalization mutable W face", iw, self.nwd)?;
            mutable_m_points.extend(self.w_faces[iw].im);
            mutable_u_edges.extend(self.w_faces[iw].iu);
        }
        let outside_changed_faces = trial
            .iter()
            .zip(&preflight.prepared_selected_faces)
            .enumerate()
            .skip(2)
            .filter_map(|(iw, (trial, baseline))| {
                (trial != baseline && !mutable_faces.contains(&iw)).then_some(iw)
            })
            .collect();

        let baseline_outside_perimeter = canonical_external_perimeter_interface(
            patch
                .perimeter_interfaces
                .iter()
                .map(|component| component.points.clone()),
            &mutable_m_points,
            &mutable_u_edges,
        );
        if symbolic.predicted_transition_self_loop_count.is_none() {
            return Err(method_c_repairable_perimeter_error(
                MethodCRepairableKind::NonTripletPerimeter,
                symbolic.perimeter_lengths.clone(),
                0,
                format!(
                    "Method-C legalization assignment perimeter lengths {:?} cannot be grouped into transition triples",
                    symbolic.perimeter_lengths
                ),
            ));
        }
        let ordered_perimeter_components = trial_perimeters
            .iter()
            .enumerate()
            .map(
                |(component_index, component)| MethodCHfieldPerimeterComponentCheckpoint {
                    component_index,
                    points: component
                        .iter()
                        .copied()
                        .map(|point| self.hfield_perimeter_point_checkpoint(point))
                        .collect(),
                },
            )
            .collect::<Vec<_>>();
        let trial_outside_perimeter = canonical_external_perimeter_interface(
            ordered_perimeter_components
                .iter()
                .map(|component| component.points.clone())
                .collect::<Vec<_>>(),
            &mutable_m_points,
            &mutable_u_edges,
        );
        let (
            exact_materializable,
            exact_failure_kind,
            exact_failure_message,
            exact_failure_dependency_faces,
        ) = match self.spawn_nest_pass_method_c_without_mask_repair(
            &trial,
            child_level,
            max_mrows,
            true,
        ) {
            Ok(_) => (true, None, None, Vec::new()),
            Err(error) => {
                let dependency_faces =
                    self.method_c_repair_witness_dependency_faces(&error, &m_neighbors);
                (
                    false,
                    Some(method_c_hfield_failure_kind(&error)),
                    Some(error.to_string()),
                    dependency_faces,
                )
            }
        };

        Ok(MethodCHfieldLegalizationPatchBoundaryCheck {
            outside_changed_faces,
            outside_perimeter_interface_changed: baseline_outside_perimeter
                != trial_outside_perimeter,
            selected_face_ids: trial
                .iter()
                .enumerate()
                .skip(2)
                .filter_map(|(iw, &selected)| selected.then_some(iw))
                .collect(),
            ordered_perimeter_components,
            perimeter_lengths: symbolic.perimeter_lengths,
            vertex_only_contact_count: symbolic.vertex_only_contact_count,
            predicted_transition_self_loop_count: symbolic
                .predicted_transition_self_loop_count
                .expect("triplet perimeter checked above"),
            exact_materializable,
            exact_failure_kind,
            exact_failure_message,
            exact_failure_dependency_faces,
        })
    }

    fn try_legalize_hfield_selection_locally(
        &self,
        checkpoint: &MethodCHfieldSelectionCheckpoint,
        preflight: &MethodCHfieldLegalizationPreflight,
        child_level: usize,
        max_mrows: usize,
    ) -> io::Result<Option<Vec<bool>>> {
        if preflight.patches.is_empty() {
            return Ok(None);
        }
        let baseline_self_loops = preflight.self_loop_witnesses.len();
        let mut assignments = Vec::with_capacity(preflight.patches.len());
        for patch in &preflight.patches {
            let baseline = patch
                .selected_candidate_seed_ids
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            let mut found = None;
            for &seed in &patch.candidate_seed_ids {
                let mut assignment = baseline.clone();
                if !assignment.insert(seed) {
                    assignment.remove(&seed);
                }
                let assignment = assignment.into_iter().collect::<Vec<_>>();
                let Ok(check) = self.legalization_patch_boundary_check(
                    checkpoint,
                    preflight,
                    patch,
                    &assignment,
                    child_level,
                    max_mrows,
                ) else {
                    continue;
                };
                if check.is_closed()
                    && check.vertex_only_contact_count == 0
                    && check.perimeter_lengths.iter().all(|length| length % 3 == 0)
                    && check.predicted_transition_self_loop_count < baseline_self_loops
                {
                    found = Some(assignment);
                    break;
                }
            }
            let Some(assignment) = found else {
                return Ok(None);
            };
            assignments.push((patch, assignment));
        }

        let mut combined = MethodCHfieldLegalizationPatch {
            cluster_index: usize::MAX,
            witness_indices: Vec::new(),
            witness_perimeter_components: Vec::new(),
            perimeter_components: Vec::new(),
            perimeter_interfaces: Vec::new(),
            dependency_faces: Vec::new(),
            dependency_face_lineages: Vec::new(),
            candidate_seed_ids: Vec::new(),
            candidate_seed_lineages: Vec::new(),
            selected_candidate_seed_ids: Vec::new(),
            mutable_faces: Vec::new(),
            mutable_face_lineages: Vec::new(),
        };
        for (patch, assignment) in assignments {
            combined
                .candidate_seed_ids
                .extend(&patch.candidate_seed_ids);
            combined.selected_candidate_seed_ids.extend(assignment);
            combined.mutable_faces.extend(&patch.mutable_faces);
            combined
                .perimeter_interfaces
                .extend(patch.perimeter_interfaces.clone());
        }
        combined.candidate_seed_ids.sort_unstable();
        combined.candidate_seed_ids.dedup();
        combined.selected_candidate_seed_ids.sort_unstable();
        combined.selected_candidate_seed_ids.dedup();
        combined.mutable_faces.sort_unstable();
        combined.mutable_faces.dedup();
        let fixed_candidates = combined
            .candidate_seed_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();

        let mut assignment = combined
            .selected_candidate_seed_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut seen_failure_dependencies = BTreeSet::new();
        let mut last_progress = None;
        let mut legalization_steps = 0usize;
        loop {
            if legalization_steps == METHOD_C_LOCAL_LEGALIZATION_MAX_STEPS {
                eprintln!(
                    "earthmesh_mesh: Method-C local legalization exhausted its {METHOD_C_LOCAL_LEGALIZATION_MAX_STEPS}-step budget"
                );
                break;
            }
            legalization_steps += 1;
            let Ok(check) = self.legalization_patch_boundary_check(
                checkpoint,
                preflight,
                &combined,
                &assignment.iter().copied().collect::<Vec<_>>(),
                child_level,
                max_mrows,
            ) else {
                break;
            };
            if std::env::var_os("EARTHMESH_M0_LOCAL_LEGALIZATION_TRACE").is_some() {
                eprintln!(
                    "earthmesh_mesh: local legalization step {legalization_steps} failure={:?} dependencies={:?} perimeters={:?}",
                    check.exact_failure_kind,
                    check.exact_failure_dependency_faces,
                    check.perimeter_lengths
                );
            }
            let mut selected = vec![false; self.nwd + 1];
            for &iw in &check.selected_face_ids {
                selected[iw] = true;
            }
            if check.exact_materializable {
                eprintln!(
                    "earthmesh_mesh: Method-C local legalization exact SAT after {legalization_steps} steps"
                );
                return Ok(Some(selected));
            }
            if check.exact_failure_kind == Some(MethodCHfieldFailureKind::TransitionPatch) {
                let perimeter = check
                    .ordered_perimeter_components
                    .iter()
                    .flat_map(|component| component.points.iter())
                    .map(|point| MethodCPerimeterPoint {
                        im: point.parent_m_point,
                        iu: point.parent_u_edge,
                        npoly: point.npoly,
                        nwdiv: point.nwdiv,
                        near_pentagon: point.near_pentagon,
                    })
                    .collect::<Vec<_>>();
                let support = self
                    .required_parent_support_lineages_from_selected_and_perimeter(
                        &selected, &perimeter,
                    )?
                    .into_iter()
                    .filter_map(|lineage| usize::try_from(lineage).ok())
                    .collect::<BTreeSet<_>>();
                if !support.is_empty() {
                    eprintln!(
                        "earthmesh_mesh: Method-C local legalization requested {} parent supports after {legalization_steps} steps",
                        support.len()
                    );
                    return Err(
                        crate::method_c_perimeter_repair::method_c_parent_support_error(support),
                    );
                }
            }
            if check.predicted_transition_self_loop_count < baseline_self_loops {
                last_progress = Some(selected);
            }
            let current_dependencies = check.exact_failure_dependency_faces.clone();
            if !matches!(
                check.exact_failure_kind,
                Some(MethodCHfieldFailureKind::Valence | MethodCHfieldFailureKind::TransitionPatch)
            ) || check.exact_failure_dependency_faces.is_empty()
                || !seen_failure_dependencies.insert(current_dependencies.clone())
            {
                break;
            }

            let dependencies = check
                .exact_failure_dependency_faces
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            let mut direct = Vec::new();
            for &seed in &checkpoint.legal_seed_ids {
                let footprint = self.selected_faces_from_method_c_seed_ids(&[seed])?;
                if footprint
                    .iter()
                    .enumerate()
                    .any(|(iw, selected)| *selected && dependencies.contains(&iw))
                {
                    direct.push(seed);
                }
            }
            let failure_patch = MethodCHfieldLegalizationPatch {
                candidate_seed_ids: direct,
                perimeter_interfaces: check.ordered_perimeter_components.clone(),
                dependency_faces: check.exact_failure_dependency_faces,
                ..combined.clone()
            };
            let expanded =
                self.expand_legalization_patch_one_ring(checkpoint, preflight, &failure_patch)?;
            let existing_candidates = combined
                .candidate_seed_ids
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            let local = expanded
                .candidate_seed_ids
                .iter()
                .copied()
                .filter(|seed| !fixed_candidates.contains(seed))
                .collect::<BTreeSet<_>>();
            let baseline_selected = checkpoint
                .selected_seed_ids
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            assignment.extend(local.iter().copied().filter(|seed| {
                !existing_candidates.contains(seed) && baseline_selected.contains(seed)
            }));
            combined
                .candidate_seed_ids
                .extend(&expanded.candidate_seed_ids);
            combined.mutable_faces.extend(&expanded.mutable_faces);
            combined.candidate_seed_ids.sort_unstable();
            combined.candidate_seed_ids.dedup();
            combined.mutable_faces.sort_unstable();
            combined.mutable_faces.dedup();
            combined.perimeter_interfaces = check.ordered_perimeter_components;

            let mut trials = Vec::with_capacity(local.len() + 1);
            let mut all = assignment.clone();
            all.extend(&local);
            trials.push(all);
            for seed in local {
                let mut trial = assignment.clone();
                if !trial.insert(seed) {
                    trial.remove(&seed);
                }
                trials.push(trial);
            }
            let mut next = None;
            let mut evaluated = 0usize;
            for trial in trials {
                evaluated += 1;
                let Ok(candidate) = self.legalization_patch_boundary_check(
                    checkpoint,
                    preflight,
                    &combined,
                    &trial.iter().copied().collect::<Vec<_>>(),
                    child_level,
                    max_mrows,
                ) else {
                    continue;
                };
                if candidate.exact_materializable
                    || (matches!(
                        candidate.exact_failure_kind,
                        Some(
                            MethodCHfieldFailureKind::Valence
                                | MethodCHfieldFailureKind::TransitionPatch
                        )
                    ) && !candidate.exact_failure_dependency_faces.is_empty()
                        && candidate.exact_failure_dependency_faces != current_dependencies
                        && !seen_failure_dependencies
                            .contains(&candidate.exact_failure_dependency_faces))
                {
                    next = Some(trial);
                    break;
                }
            }
            let Some(next) = next else {
                if std::env::var_os("EARTHMESH_M0_LOCAL_LEGALIZATION_TRACE").is_some() {
                    eprintln!(
                        "earthmesh_mesh: local legalization exhausted {evaluated} new-domain assignments"
                    );
                }
                break;
            };
            assignment = next;
        }
        eprintln!(
            "earthmesh_mesh: Method-C local legalization stopped after {legalization_steps} steps with exact progress={} ",
            last_progress.is_some()
        );
        Ok(last_progress)
    }

    pub fn spawn_nest_pass_from_target_levels_and_face_demands<F: Fn(f64, f64) -> u8>(
        &self,
        target_level: F,
        face_demand: &[bool],
        pass: usize,
        child_grid_number: usize,
        max_mrows: usize,
        preserve_all_demands: bool,
    ) -> io::Result<Option<Self>> {
        if pass == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C face-demand pass must be positive",
            ));
        }
        let (mut selected_faces, coverage, _) = self
            .selected_faces_and_coverage_from_target_levels_with_policy(
                &target_level,
                Some(face_demand),
                pass,
                false,
                preserve_all_demands,
                false,
                true,
            )?;
        let m_neighbors = self.method_c_m_neighbors()?;
        self.clear_method_c_conceded_margin(&mut selected_faces, &m_neighbors, 2)?;
        coverage.validate(&selected_faces)?;
        if selected_faces.iter().skip(2).all(|selected| !*selected) {
            return Ok(None);
        }
        if std::env::var_os("EARTHMESH_M0_CROSS_LEVEL_SUPPORT").is_some() {
            let mut checkpoint = self.selection_checkpoint_from_target_levels_and_face_demands(
                &target_level,
                face_demand,
                pass,
                preserve_all_demands,
            )?;
            checkpoint.selected_faces.clone_from(&selected_faces);
            let preflight = self.legalization_preflight_from_selected_faces(
                &selected_faces,
                &checkpoint.legal_seed_ids,
                &checkpoint.selected_seed_ids,
                child_grid_number,
            )?;
            if let Some(legalized) = self.try_legalize_hfield_selection_locally(
                &checkpoint,
                &preflight,
                child_grid_number,
                max_mrows,
            )? {
                coverage.validate(&legalized)?;
                selected_faces = legalized;
            }
        }
        self.spawn_nest_pass_method_c_preserving_demands(
            &selected_faces,
            child_grid_number,
            max_mrows,
            true,
            &coverage,
        )
        .map(Some)
    }

    pub(crate) fn spawn_nest_from_target_levels_internal<F: Fn(f64, f64) -> u8>(
        &self,
        target_level: &F,
        max_level: usize,
        max_mrows: usize,
        spring: Option<(usize, usize, Option<f64>)>,
        use_cartesian_xy: bool,
        diagnostic_lineages: Option<bool>,
        collect_hfield_diagnostics: bool,
    ) -> io::Result<(
        Self,
        usize,
        Vec<MethodCNestSpringDiagnostics>,
        Vec<MethodCHfieldPassDiagnostics>,
    )> {
        self.validate_topology()?;
        if max_level == 0 {
            return Ok((self.clone(), 0, Vec::new(), Vec::new()));
        }
        if max_mrows == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C spawn_nest max_mrows must be greater than zero",
            ));
        }
        if max_level > 5 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Method-C refinement max_level {max_level} must be in 1..=5"),
            ));
        }

        let mut mesh = self.clone();
        let mut spring_passes = 0usize;
        let mut spring_diagnostics = Vec::new();
        let mut hfield_pass_diagnostics = Vec::new();
        let first_grid_number = self
            .w_faces
            .iter()
            .skip(2)
            .map(|face| face.ngr)
            .chain(self.m_metadata.iter().skip(2).map(|metadata| metadata.ngr))
            .max()
            .unwrap_or(1)
            .max(1)
            + 1;

        let mut grid_number = first_grid_number;
        let report_stage = |phase, pass| {
            if earthmesh_core::progress::report(phase, pass, max_level) {
                Ok(())
            } else {
                Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    format!("Method-C h-field {phase} cancelled"),
                ))
            }
        };
        for pass in 1..=max_level {
            let has_deeper_demand = pass < max_level
                && mesh.hfield_has_demand_at_or_above(target_level, pass + 1, use_cartesian_xy)?;
            report_stage("method_c-hfield-selection-start", pass)?;
            let (selected_faces, coverage, mut pass_diagnostics) = mesh
                .selected_faces_and_coverage_from_target_levels_with_policy(
                    target_level,
                    None,
                    pass,
                    use_cartesian_xy,
                    !has_deeper_demand,
                    collect_hfield_diagnostics,
                    true,
                )?;
            report_stage("method_c-hfield-selection-end", pass)?;
            if collect_hfield_diagnostics && selected_faces.iter().skip(2).any(|selected| *selected)
            {
                report_stage("method_c-hfield-candidate-validation-start", pass)?;
                pass_diagnostics.candidate_validation = Some(mesh.validate_hfield_candidate(
                    &selected_faces,
                    &coverage,
                    grid_number,
                    max_mrows,
                    &pass_diagnostics.selected_seed_ids,
                    &pass_diagnostics.legal_seed_ids,
                ));
                report_stage("method_c-hfield-candidate-validation-end", pass)?;
            }
            if selected_faces.iter().skip(2).all(|selected| !*selected) {
                if collect_hfield_diagnostics {
                    hfield_pass_diagnostics.push(pass_diagnostics);
                }
                if mesh.hfield_has_current_parent_demand(target_level, pass, use_cartesian_xy)? {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "Method-C h-field pass {pass} demand is entirely on the parent transition boundary"
                        ),
                    ));
                }
                if has_deeper_demand {
                    continue;
                }
                break;
            }

            report_stage("method_c-hfield-spawn-start", pass)?;
            let spawned = mesh.spawn_nest_pass_method_c_preserving_demands(
                &selected_faces,
                grid_number,
                max_mrows,
                true,
                &coverage,
            );
            report_stage("method_c-hfield-spawn-end", pass)?;
            mesh = match spawned {
                Ok(spawned) => {
                    if collect_hfield_diagnostics {
                        hfield_pass_diagnostics.push(pass_diagnostics);
                    }
                    spawned
                }
                Err(error) if collect_hfield_diagnostics => {
                    hfield_pass_diagnostics.push(pass_diagnostics);
                    return Err(wrap_hfield_spawn_failure(
                        pass,
                        hfield_pass_diagnostics,
                        error,
                    ));
                }
                Err(error) => {
                    return Err(io::Error::new(
                        error.kind(),
                        format!("Method-C h-field spawn_nest pass {pass} failed: {error}"),
                    ));
                }
            };

            if let Some((nxp, niter, cartesian_dist00)) = spring {
                if let Some(include_lineages) = diagnostic_lineages {
                    spring_diagnostics.push(mesh.nest_spring_diagnostics(
                        grid_number,
                        false,
                        include_lineages,
                    )?);
                }
                if niter > 0 {
                    mesh = mesh.spring_nest_with_radius_projection(
                        nxp,
                        niter,
                        grid_number,
                        false,
                        !use_cartesian_xy,
                        cartesian_dist00,
                    )?;
                    spring_passes += 1;
                }
            }
            grid_number += 1;
        }

        Ok((
            mesh,
            spring_passes,
            spring_diagnostics,
            hfield_pass_diagnostics,
        ))
    }

    /// Spawn Method-C nests from a quantized target-level closure, typically
    /// `|lon, lat| hfield.level_at(lon, lat, h_base, max_level)` built from a
    /// composed and gradient-limited `earthmesh_hfield` cell-width field.
    /// Spherical lon/lat meshes only (the Cartesian-XY native path keeps the
    /// geometric region API).
    pub fn spawn_nest_from_target_levels<F: Fn(f64, f64) -> u8>(
        &self,
        target_level: F,
        max_level: usize,
        max_mrows: usize,
    ) -> io::Result<Self> {
        self.spawn_nest_from_target_levels_internal(
            &target_level,
            max_level,
            max_mrows,
            None,
            false,
            None,
            false,
        )
        .map(|(mesh, _, _, _)| mesh)
    }

    /// Same as [`Self::spawn_nest_from_target_levels`], with the compatibility
    /// per-pass nest spring applied after each refinement pass. Returns the
    /// refined mesh together with the number of spring passes executed
    /// (matching the region-path driver's reporting shape).
    pub fn spawn_nest_from_target_levels_with_spring<F: Fn(f64, f64) -> u8>(
        &self,
        target_level: F,
        max_level: usize,
        max_mrows: usize,
        nxp: usize,
        niter: usize,
    ) -> io::Result<(Self, usize)> {
        self.spawn_nest_from_target_levels_internal(
            &target_level,
            max_level,
            max_mrows,
            Some((nxp, niter, None)),
            false,
            None,
            false,
        )
        .map(|(mesh, passes, _, _)| (mesh, passes))
    }

    /// Measurement variant of
    /// [`Self::spawn_nest_from_target_levels_with_spring`]. It preserves the
    /// production algorithm and additionally captures each pass's movable set
    /// before the spring runs.
    pub fn spawn_nest_from_target_levels_with_spring_diagnostics<F: Fn(f64, f64) -> u8>(
        &self,
        target_level: F,
        max_level: usize,
        max_mrows: usize,
        nxp: usize,
        niter: usize,
        include_lineages: bool,
    ) -> io::Result<(Self, usize, Vec<MethodCNestSpringDiagnostics>)> {
        self.spawn_nest_from_target_levels_internal(
            &target_level,
            max_level,
            max_mrows,
            Some((nxp, niter, None)),
            false,
            Some(include_lineages),
            false,
        )
        .map(|(mesh, passes, spring, _)| (mesh, passes, spring))
    }

    pub fn spawn_nest_from_target_levels_with_m0_diagnostics<F: Fn(f64, f64) -> u8>(
        &self,
        target_level: F,
        max_level: usize,
        max_mrows: usize,
        nxp: usize,
        niter: usize,
        include_lineages: bool,
    ) -> io::Result<(
        Self,
        usize,
        Vec<MethodCNestSpringDiagnostics>,
        Vec<MethodCHfieldPassDiagnostics>,
    )> {
        self.spawn_nest_from_target_levels_internal(
            &target_level,
            max_level,
            max_mrows,
            Some((nxp, niter, None)),
            false,
            Some(include_lineages),
            true,
        )
    }

    /// Cartesian-XY counterpart of
    /// [`Self::spawn_nest_from_target_levels_with_spring`]. The closure is
    /// sampled with native `(x, y)` meters and nest spring uses the same
    /// `deltax` target spacing as the geometric Cartesian Method-C path.
    pub fn spawn_nest_from_cartesian_xy_target_levels_with_spring_deltax<F: Fn(f64, f64) -> u8>(
        &self,
        target_level: F,
        max_level: usize,
        max_mrows: usize,
        nxp: usize,
        niter: usize,
        deltax_meters: f64,
    ) -> io::Result<(Self, usize)> {
        if !deltax_meters.is_finite() || deltax_meters < 0.001 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C Cartesian h-field nest spring deltax must be at least 0.001",
            ));
        }
        let cartesian_dist00 = deltax_meters * (2.0 / 3.0_f64.sqrt()).sqrt();
        self.spawn_nest_from_target_levels_internal(
            &target_level,
            max_level,
            max_mrows,
            Some((nxp, niter, Some(cartesian_dist00))),
            true,
            None,
            false,
        )
        .map(|(mesh, passes, _, _)| (mesh, passes))
    }
}
