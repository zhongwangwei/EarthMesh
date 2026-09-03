//! Component-scoped, rollback-first coarsening transaction core.
//!
//! This module is deliberately small: topology chooses faces, elastic may move
//! only transition coordinates, then the normal geometry/final-cell/remap gates
//! decide whether the cloned state is committed.

use super::elastic_block::solve_elastic_patch_with_contract;
use super::transition_topology::hierarchy_parent_neighbours;
use super::{
    core_condensation::rebuild_from_leaf_set_with_custom_triangles,
    core_condensation::source_face_slot, ElasticBlockLimits, ElasticBlockOutcome,
    ElasticBlockReport, ElasticBlockTrial, ElasticPatch, ElasticTargetField, ElasticTargetMode,
    GeometryDomainId, HierarchyComponent, HierarchyLeafMesh, HierarchyLeafSet,
    TransitionTopologyCandidate, TransitionTopologyLimits, TransitionTopologyOutcome,
};
use crate::{
    certificate::{
        AngleContractId, Certificate, FinalCertificateReport, GeometryCertificateReport,
        GeometryRegionCertificateReport,
    },
    fingerprint::mesh_fingerprint,
    mother_grid::{MotherGrid, TriangleAddress},
    outcome::{FinalCertificationEvidence, GeometryCertifiedMotherGrid},
    remap::{ConservativeRemap, RemapCertificate},
    requirement::{
        certify_final_cell_requirements_with_remap, FinalCellRequirementError,
        FinalCellRequirementReport, SourceLevelField, TargetLevelField,
    },
};
use earthmesh_mesh::{CartesianPoint, MeshState};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentTransactionLimits {
    pub topology_states: usize,
    /// Maximum CBER iterations for each hard-topology candidate.
    pub elastic_iterations: usize,
    pub interval_boxes: usize,
    pub halo_expansions: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComponentTransactionState {
    leaf_set: HierarchyLeafSet,
    custom_transition_triangles: BTreeMap<TriangleAddress, Vec<[usize; 3]>>,
    source_positions: Vec<CartesianPoint>,
    source_delivered_levels: Vec<Option<usize>>,
    mesh: HierarchyLeafMesh,
    source_fingerprint: u64,
    source_subdivision: usize,
    claimed_parent_subdivision: Option<usize>,
    claimed_parents: BTreeSet<TriangleAddress>,
}

impl ComponentTransactionState {
    pub fn new(source: &MotherGrid, initial_level: usize) -> Result<Self, String> {
        let leaf_set = HierarchyLeafSet::from_mother_grid(source)?;
        let mesh = super::core_condensation::rebuild_from_leaf_set(source, &leaf_set)?;
        Ok(Self {
            leaf_set,
            custom_transition_triangles: BTreeMap::new(),
            source_positions: source.mesh.vertices().to_vec(),
            source_delivered_levels: source
                .mesh
                .vertices()
                .iter()
                .enumerate()
                .map(|(slot, _)| source.mesh.is_vertex_live(slot).then_some(initial_level))
                .collect(),
            mesh,
            source_fingerprint: mesh_fingerprint(&source.mesh),
            source_subdivision: source.subdivision,
            claimed_parent_subdivision: None,
            claimed_parents: BTreeSet::new(),
        })
    }

    pub fn mesh(&self) -> &HierarchyLeafMesh {
        &self.mesh
    }

    pub fn target_levels(&self) -> Result<TargetLevelField, String> {
        target_levels_for(
            &self.mesh.mesh,
            &self.mesh.source_vertex_slots,
            &self.source_delivered_levels,
        )
    }

    pub fn source_delivered_levels(&self) -> &[Option<usize>] {
        &self.source_delivered_levels
    }

    pub fn fingerprint(&self) -> u64 {
        mesh_fingerprint(&self.mesh.mesh)
    }

    pub(super) fn leaf_set(&self) -> &HierarchyLeafSet {
        &self.leaf_set
    }

    pub(super) fn level_source_slots(
        &self,
        source: &MotherGrid,
        level_grid: &MotherGrid,
    ) -> Result<Vec<Option<usize>>, String> {
        if level_grid.subdivision > self.source_subdivision
            || !self
                .source_subdivision
                .is_multiple_of(level_grid.subdivision)
            || !(self.source_subdivision / level_grid.subdivision).is_power_of_two()
        {
            return Err(format!(
                "level subdivision {} is not in source hierarchy {}",
                level_grid.subdivision, self.source_subdivision
            ));
        }
        let mut slots = vec![None; level_grid.mesh.vertices().len()];
        let live_sources = self
            .mesh
            .source_vertex_slots
            .iter()
            .flatten()
            .copied()
            .collect::<BTreeSet<_>>();
        for level_face in level_grid.mesh.active_triangle_slots() {
            let address = level_grid.triangle_addresses[level_face]
                .ok_or_else(|| format!("level face {level_face} has no hierarchy address"))?;
            for (corner, level_site) in level_grid.mesh.triangles()[level_face]
                .into_iter()
                .enumerate()
            {
                let source_site =
                    super::core_condensation::source_corner_site(source, address, corner)?;
                if !live_sources.contains(&source_site) {
                    continue;
                }
                match slots[level_site] {
                    Some(existing) if existing != source_site => {
                        return Err(format!(
                            "level site {level_site} maps to source sites {existing} and {source_site}"
                        ));
                    }
                    _ => slots[level_site] = Some(source_site),
                }
            }
        }
        Ok(slots)
    }

    fn prepare_parent_level(&mut self, parent_subdivision: usize) {
        if self.claimed_parent_subdivision != Some(parent_subdivision) {
            self.claimed_parent_subdivision = Some(parent_subdivision);
            self.claimed_parents.clear();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentTransactionStage {
    Preflight,
    Physical,
    Topology,
    InstallDelta,
    Elastic,
    LocalGeometry,
    GlobalGeometry,
    FinalGeometry,
    FinalCells,
    Remap,
    Postcondition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentRollbackReport {
    pub component_id: u64,
    pub stage: ComponentTransactionStage,
    pub reason: String,
    pub before_fingerprint: u64,
    pub restored_fingerprint: u64,
    pub pre_vertices: usize,
    pub pre_faces: usize,
    pub topology_states: usize,
    pub elastic_iterations: usize,
    pub interval_boxes: usize,
    pub halo_expansions: usize,
}

#[derive(Debug, Clone)]
pub struct ComponentCommitReport {
    pub component_id: u64,
    pub before_fingerprint: u64,
    pub after_fingerprint: u64,
    pub pre_vertices: usize,
    pub pre_faces: usize,
    pub post_vertices: usize,
    pub post_faces: usize,
    pub removed_vertices: usize,
    pub removed_faces: usize,
    pub core_vertices_removed: usize,
    pub core_search_states: usize,
    pub topology_states: usize,
    pub elastic_iterations: usize,
    pub interval_boxes: usize,
    pub halo_expansions: usize,
    pub local_geometry: GeometryRegionCertificateReport,
    pub global_geometry: GeometryCertificateReport,
    pub final_certificate: FinalCertificateReport,
    pub final_cells: FinalCellRequirementReport,
    pub remap: RemapCertificate,
    pub elastic: Option<ElasticBlockReport>,
}

#[derive(Debug, Clone)]
pub enum ComponentTransactionOutcome {
    Certified(Box<ComponentCommitReport>),
    NoTopology(ComponentRollbackReport),
    ElasticNoImprovement(ComponentRollbackReport),
    SearchBudgetExhausted(ComponentRollbackReport),
    RequiresWiderHalo(ComponentRollbackReport),
    NotCertifiable(ComponentRollbackReport),
    InvalidInput(ComponentRollbackReport),
}

#[allow(clippy::too_many_arguments)]
pub fn solve_component_transaction(
    source: &MotherGrid,
    source_levels: &SourceLevelField,
    state: &mut ComponentTransactionState,
    component: &HierarchyComponent,
    coarse_level: usize,
    max_adjacent_level_delta: usize,
    limits: ComponentTransactionLimits,
) -> ComponentTransactionOutcome {
    solve_component_transaction_with_contract(
        source,
        source_levels,
        state,
        component,
        coarse_level,
        max_adjacent_level_delta,
        limits,
        AngleContractId::default(),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn solve_component_transaction_with_contract(
    source: &MotherGrid,
    source_levels: &SourceLevelField,
    state: &mut ComponentTransactionState,
    component: &HierarchyComponent,
    coarse_level: usize,
    max_adjacent_level_delta: usize,
    limits: ComponentTransactionLimits,
    angle_contract: AngleContractId,
) -> ComponentTransactionOutcome {
    let level_source_slots = source
        .mesh
        .vertices()
        .iter()
        .enumerate()
        .map(|(site, _)| source.mesh.is_vertex_live(site).then_some(site))
        .collect::<Vec<_>>();
    solve_component_transaction_at_level(
        source,
        source_levels,
        state,
        source,
        &level_source_slots,
        component,
        coarse_level,
        max_adjacent_level_delta,
        limits,
        angle_contract,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn solve_component_transaction_at_level(
    source: &MotherGrid,
    source_levels: &SourceLevelField,
    state: &mut ComponentTransactionState,
    level_grid: &MotherGrid,
    level_source_slots: &[Option<usize>],
    component: &HierarchyComponent,
    coarse_level: usize,
    max_adjacent_level_delta: usize,
    limits: ComponentTransactionLimits,
    angle_contract: AngleContractId,
) -> ComponentTransactionOutcome {
    let before_fingerprint = state.fingerprint();
    let pre_vertices = state.mesh.mesh.vertex_count();
    let pre_faces = state.mesh.mesh.triangle_count();
    let mut counters = Counters::default();

    macro_rules! fail {
        ($variant:ident, $stage:expr, $reason:expr) => {{
            ComponentTransactionOutcome::$variant(ComponentRollbackReport {
                component_id: component.id,
                stage: $stage,
                reason: $reason,
                before_fingerprint,
                restored_fingerprint: state.fingerprint(),
                pre_vertices,
                pre_faces,
                topology_states: counters.topology_states,
                elastic_iterations: counters.elastic_iterations,
                interval_boxes: counters.interval_boxes,
                halo_expansions: counters.halo_expansions,
            })
        }};
    }

    let Some(parent_subdivision) = component.parents.first().map(|parent| parent.n) else {
        return fail!(
            InvalidInput,
            ComponentTransactionStage::Preflight,
            "component has no parents".to_string()
        );
    };

    if let Err(reason) = validate_preflight(source, source_levels, state, component) {
        return fail!(InvalidInput, ComponentTransactionStage::Preflight, reason);
    }
    if let Err(reason) = validate_level_mapping(source, level_grid, level_source_slots, component) {
        return fail!(InvalidInput, ComponentTransactionStage::Preflight, reason);
    }
    if let Err(reason) =
        validate_physical_eligibility(source, source_levels, component, coarse_level)
    {
        return fail!(NotCertifiable, ComponentTransactionStage::Physical, reason);
    }

    let pre_sources = active_source_mask(&state.mesh, source.mesh.vertices().len());
    let mut topology_cursor = 0usize;
    let mut saw_candidate = false;
    let mut last_retry: Option<(ComponentTransactionStage, String)> = None;
    let mut last_elastic_budget_failure: Option<String> = None;
    let mut preferred_core_promotion = None;
    let mut topology_state_offset = 0usize;
    let mut halo_expansion_offset = 0usize;
    let mut search_component = component.clone();
    let promotion_depths = match core_promotion_depths(level_grid, component) {
        Ok(depths) => depths,
        Err(reason) => return fail!(InvalidInput, ComponentTransactionStage::Preflight, reason),
    };

    loop {
        let preferred_promotion_with_cost = preferred_core_promotion.and_then(|parent| {
            let depth = promotion_depths.get(&parent).copied()?;
            (depth <= limits.halo_expansions)
                .then_some((parent, depth.saturating_sub(halo_expansion_offset)))
        });
        let transition = match (TransitionTopologyLimits {
            topology_states: limits.topology_states.saturating_sub(topology_state_offset),
            maximum_halo_expansions: limits.halo_expansions.saturating_sub(halo_expansion_offset),
        })
        .solve_from_cursor_with_promotion(
            level_grid,
            &search_component,
            topology_cursor,
            preferred_promotion_with_cost,
        ) {
            TransitionTopologyOutcome::Closed(trial) => {
                counters.topology_states =
                    topology_state_offset.saturating_add(trial.report.topology_states);
                counters.halo_expansions =
                    halo_expansion_offset.saturating_add(trial.report.halo_expansions);
                match remap_transition_trial(trial, level_source_slots) {
                    Ok(trial) => trial,
                    Err(reason) => {
                        return fail!(InvalidInput, ComponentTransactionStage::Topology, reason)
                    }
                }
            }
            TransitionTopologyOutcome::RequiresWiderHalo {
                states_examined,
                halo_expansions,
            } => {
                counters.topology_states = topology_state_offset.saturating_add(states_examined);
                counters.halo_expansions = halo_expansion_offset.saturating_add(halo_expansions);
                return fail!(
                    RequiresWiderHalo,
                    ComponentTransactionStage::Topology,
                    "component needs a wider transition halo".to_string()
                );
            }
            TransitionTopologyOutcome::SearchBudgetExhausted {
                states_examined,
                halo_expansions,
            } => {
                counters.topology_states = topology_state_offset.saturating_add(states_examined);
                counters.halo_expansions = halo_expansion_offset.saturating_add(halo_expansions);
                let reason = last_elastic_budget_failure
                    .as_deref()
                    .map(|elastic| {
                        format!("transition topology budget exhausted; earlier {elastic}")
                    })
                    .or_else(|| {
                        last_retry.as_ref().map(|(stage, retry)| {
                            format!(
                                "transition topology budget exhausted; last candidate failed at {stage:?}: {retry}"
                            )
                        })
                    })
                    .unwrap_or_else(|| "transition topology budget exhausted".to_string());
                return fail!(
                    SearchBudgetExhausted,
                    ComponentTransactionStage::Topology,
                    reason
                );
            }
            TransitionTopologyOutcome::ProvenInfeasible {
                states_examined,
                halo_expansions,
                reason,
            } => {
                counters.topology_states = topology_state_offset.saturating_add(states_examined);
                counters.halo_expansions = halo_expansion_offset.saturating_add(halo_expansions);
                if saw_candidate {
                    if let Some(reason) = last_elastic_budget_failure {
                        return fail!(
                            SearchBudgetExhausted,
                            ComponentTransactionStage::Elastic,
                            reason
                        );
                    }
                    let (stage, retry_reason) = last_retry.unwrap_or((
                        ComponentTransactionStage::Topology,
                        "candidate certification failed".to_string(),
                    ));
                    return fail!(
                        NotCertifiable,
                        stage,
                        format!("all topology candidates failed certification; last failure: {retry_reason}")
                    );
                }
                return fail!(NoTopology, ComponentTransactionStage::Topology, reason);
            }
            TransitionTopologyOutcome::InvalidBoundary {
                states_examined,
                halo_expansions,
                reason,
            } => {
                counters.topology_states = topology_state_offset.saturating_add(states_examined);
                counters.halo_expansions = halo_expansion_offset.saturating_add(halo_expansions);
                return fail!(InvalidInput, ComponentTransactionStage::Topology, reason);
            }
        };

        saw_candidate = true;
        let candidate_topology_states = transition.report.layout_topology_states;
        let layout_changed = transition.candidate.core_parents != search_component.core_parents
            || transition.boundary.halo_parents != search_component.transition_parents;
        let candidate_previous_cursor = if layout_changed { 0 } else { topology_cursor };
        if layout_changed {
            topology_state_offset = counters
                .topology_states
                .saturating_sub(candidate_topology_states);
            halo_expansion_offset = counters.halo_expansions;
            search_component.core_parents = transition.candidate.core_parents.clone();
            search_component.transition_parents = transition.boundary.halo_parents.clone();
        }
        let exact_core_candidate = transition.candidate.custom_transition_triangles.is_empty();
        let mut candidate_state = state.clone();
        candidate_state.prepare_parent_level(parent_subdivision);
        match certify_candidate(
            source,
            source_levels,
            &mut candidate_state,
            component,
            coarse_level,
            max_adjacent_level_delta,
            &transition,
            limits.elastic_iterations,
            limits
                .interval_boxes
                .saturating_sub(counters.interval_boxes),
            before_fingerprint,
            pre_vertices,
            pre_faces,
            &pre_sources,
            angle_contract,
        ) {
            Ok(mut report) => {
                counters.elastic_iterations += report.elastic_iterations;
                counters.interval_boxes += report.interval_boxes;
                report.topology_states = counters.topology_states;
                report.elastic_iterations = counters.elastic_iterations;
                report.interval_boxes = counters.interval_boxes;
                report.halo_expansions = counters.halo_expansions;
                *state = candidate_state;
                return ComponentTransactionOutcome::Certified(Box::new(report));
            }
            Err(failure) => {
                counters.elastic_iterations += failure.elastic_iterations;
                counters.interval_boxes += failure.interval_boxes;
                preferred_core_promotion = failure.failed_guard_face.and_then(|face| {
                    preferred_core_promotion_for_face(&candidate_state.mesh, &transition, face)
                });
                match failure.disposition {
                    CandidateFailureDisposition::InvalidInput => {
                        return fail!(InvalidInput, failure.stage, failure.reason)
                    }
                    CandidateFailureDisposition::BudgetExhausted => {
                        if failure.stage != ComponentTransactionStage::Elastic
                            || exact_core_candidate
                            || candidate_topology_states <= candidate_previous_cursor
                        {
                            return fail!(SearchBudgetExhausted, failure.stage, failure.reason);
                        }
                        last_elastic_budget_failure = Some(failure.reason);
                        topology_cursor = candidate_topology_states;
                    }
                    CandidateFailureDisposition::Retry => {
                        last_retry = Some((failure.stage, failure.reason));
                        if exact_core_candidate
                            || candidate_topology_states <= candidate_previous_cursor
                        {
                            let (stage, reason) = last_retry.expect("just recorded retry");
                            return fail!(NotCertifiable, stage, reason);
                        }
                        topology_cursor = candidate_topology_states;
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CandidateFailureDisposition {
    Retry,
    InvalidInput,
    BudgetExhausted,
}

#[derive(Debug, PartialEq, Eq)]
struct CandidateAttemptFailure {
    disposition: CandidateFailureDisposition,
    stage: ComponentTransactionStage,
    reason: String,
    elastic_iterations: usize,
    interval_boxes: usize,
    failed_guard_face: Option<usize>,
}

impl CandidateAttemptFailure {
    fn retry(stage: ComponentTransactionStage, reason: impl Into<String>) -> Self {
        Self {
            disposition: CandidateFailureDisposition::Retry,
            stage,
            reason: reason.into(),
            elastic_iterations: 0,
            interval_boxes: 0,
            failed_guard_face: None,
        }
    }

    fn invalid(stage: ComponentTransactionStage, reason: impl Into<String>) -> Self {
        Self {
            disposition: CandidateFailureDisposition::InvalidInput,
            stage,
            reason: reason.into(),
            elastic_iterations: 0,
            interval_boxes: 0,
            failed_guard_face: None,
        }
    }

    fn budget(stage: ComponentTransactionStage, reason: impl Into<String>) -> Self {
        Self {
            disposition: CandidateFailureDisposition::BudgetExhausted,
            stage,
            reason: reason.into(),
            elastic_iterations: 0,
            interval_boxes: 0,
            failed_guard_face: None,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn certify_candidate(
    source: &MotherGrid,
    source_levels: &SourceLevelField,
    state: &mut ComponentTransactionState,
    component: &HierarchyComponent,
    coarse_level: usize,
    max_adjacent_level_delta: usize,
    transition: &super::TransitionTopologyTrial,
    remaining_elastic_iterations: usize,
    remaining_interval_boxes: usize,
    before_fingerprint: u64,
    pre_vertices: usize,
    pre_faces: usize,
    pre_sources: &[bool],
    angle_contract: AngleContractId,
) -> Result<ComponentCommitReport, CandidateAttemptFailure> {
    let candidate = transition.candidate.clone();
    install_delta(source, state, &candidate).map_err(|reason| {
        CandidateAttemptFailure::invalid(ComponentTransactionStage::InstallDelta, reason)
    })?;
    apply_source_positions(&mut state.mesh, &state.source_positions);

    let mut elastic_iterations = 0usize;
    let mut elastic_report = None;
    let guard_faces = affected_faces(source, &state.mesh, &candidate);
    let interval_boxes = guard_faces.len().saturating_mul(3);
    if interval_boxes > remaining_interval_boxes {
        let mut failure = CandidateAttemptFailure::budget(
            ComponentTransactionStage::LocalGeometry,
            "local geometry interval-box budget exhausted".to_string(),
        );
        failure.interval_boxes = interval_boxes;
        return Err(failure);
    }
    if !Certificate::internal_for(angle_contract)
        .geometry_region_passes(&state.mesh.mesh, &guard_faces)
    {
        if transition.candidate.custom_transition_triangles.is_empty() {
            return Err(CandidateAttemptFailure::retry(
                ComponentTransactionStage::GlobalGeometry,
                "exact coarse core failed internal geometry certification".to_string(),
            ));
        }
        let patch =
            elastic_patch_for_state(transition, &state.mesh, angle_contract).map_err(|reason| {
                CandidateAttemptFailure::retry(ComponentTransactionStage::Elastic, reason)
            })?;
        let elastic = match solve_elastic_patch_with_contract(
            &state.mesh,
            patch,
            ElasticBlockLimits {
                elastic_iterations: remaining_elastic_iterations,
            },
            angle_contract,
        ) {
            ElasticBlockOutcome::Certified(trial) => trial,
            ElasticBlockOutcome::ElasticNoImprovement {
                elastic_iterations: iterations,
                initial_energy,
                final_energy,
                reason,
                failed_guard_face,
                global_angle_degrees,
                ..
            }
            | ElasticBlockOutcome::RequiresDifferentTopology {
                elastic_iterations: iterations,
                initial_energy,
                final_energy,
                reason,
                failed_guard_face,
                global_angle_degrees,
                ..
            } => {
                let mut failure = CandidateAttemptFailure::retry(
                    ComponentTransactionStage::Elastic,
                    format!(
                        "{reason}{}{} (energy {initial_energy:.6e} -> {final_energy:.6e})",
                        guard_face_suffix(failed_guard_face),
                        angle_range_suffix(global_angle_degrees)
                    ),
                );
                failure.elastic_iterations = iterations;
                failure.failed_guard_face = failed_guard_face;
                return Err(failure);
            }
            ElasticBlockOutcome::SearchBudgetExhausted {
                elastic_iterations: iterations,
                initial_energy,
                final_energy,
                reason,
                failed_guard_face,
                global_angle_degrees,
                ..
            } => {
                let mut failure = CandidateAttemptFailure::budget(
                    ComponentTransactionStage::Elastic,
                    format!(
                        "elastic iteration budget exhausted: {reason}{}{} (energy {initial_energy:.6e} -> {final_energy:.6e})",
                        guard_face_suffix(failed_guard_face),
                        angle_range_suffix(global_angle_degrees)
                    ),
                );
                failure.elastic_iterations = iterations;
                failure.failed_guard_face = failed_guard_face;
                return Err(failure);
            }
            ElasticBlockOutcome::InvalidPatch { reason } => {
                return Err(CandidateAttemptFailure::invalid(
                    ComponentTransactionStage::Elastic,
                    reason,
                ));
            }
        };
        elastic_iterations = elastic.report.elastic_iterations;
        apply_elastic(state, &elastic);
        elastic_report = Some(elastic.report.clone());
    }

    lower_covered_source_levels(
        source,
        state,
        &candidate,
        &transition.boundary,
        coarse_level,
    );
    let local_geometry = Certificate::internal_for(angle_contract)
        .verify_geometry_region(&state.mesh.mesh, &guard_faces)
        .map_err(|error| {
            let mut failure = CandidateAttemptFailure::retry(
                ComponentTransactionStage::LocalGeometry,
                format!("{error:?}"),
            );
            failure.interval_boxes = interval_boxes;
            failure
        })?;
    debug_assert_eq!(interval_boxes, local_geometry.interval_boxes);

    let global_geometry = Certificate::internal_for(angle_contract)
        .verify_geometry(&state.mesh.mesh)
        .map_err(|error| {
            let mut failure = CandidateAttemptFailure::retry(
                ComponentTransactionStage::GlobalGeometry,
                format!("{error:?}"),
            );
            failure.interval_boxes = interval_boxes;
            failure
        })?;
    let final_geometry = Certificate::final_delivery_for(angle_contract)
        .verify_geometry(&state.mesh.mesh)
        .map_err(|error| {
            let mut failure = CandidateAttemptFailure::retry(
                ComponentTransactionStage::FinalGeometry,
                format!("{error:?}"),
            );
            failure.interval_boxes = interval_boxes;
            failure
        })?;

    let target_levels = state.target_levels().map_err(|reason| {
        let mut failure =
            CandidateAttemptFailure::invalid(ComponentTransactionStage::FinalCells, reason);
        failure.interval_boxes = interval_boxes;
        failure
    })?;
    let remap = ConservativeRemap::between_voronoi_meshes(&source.mesh, &state.mesh.mesh).map_err(
        |reason| {
            let mut failure =
                CandidateAttemptFailure::retry(ComponentTransactionStage::Remap, reason);
            failure.interval_boxes = interval_boxes;
            failure
        },
    )?;
    let remap_certificate =
        remap.certify_spherical_overlap(source_levels.levels().len(), target_levels.levels().len());
    let final_cells = match certify_final_cell_requirements_with_remap(
        &source.mesh,
        source_levels,
        &state.mesh.mesh,
        &target_levels,
        max_adjacent_level_delta,
        &remap,
    ) {
        Ok(report) => report,
        Err(FinalCellRequirementError::InvalidInput(reason)) => {
            let mut failure =
                CandidateAttemptFailure::invalid(ComponentTransactionStage::FinalCells, reason);
            failure.interval_boxes = interval_boxes;
            return Err(failure);
        }
        Err(FinalCellRequirementError::Residuals(report)) => {
            let mut failure = CandidateAttemptFailure::retry(
                ComponentTransactionStage::FinalCells,
                format!(
                    "{} physical and {} balance residual(s)",
                    report.physical_residuals(),
                    report.balance_residuals()
                ),
            );
            failure.interval_boxes = interval_boxes;
            return Err(failure);
        }
    };
    let final_evidence =
        FinalCertificationEvidence::from_final_cells(&final_cells, remap_certificate.clone())
            .map_err(|reason| {
                let mut failure =
                    CandidateAttemptFailure::retry(ComponentTransactionStage::Remap, reason);
                failure.interval_boxes = interval_boxes;
                failure
            })?;

    let final_mesh = crate::finalize_geometry_certified_mother(
        GeometryCertifiedMotherGrid::new(state.mesh.mesh.clone(), final_geometry),
        final_evidence,
    )
    .map_err(|error| {
        let mut failure = CandidateAttemptFailure::retry(
            ComponentTransactionStage::FinalGeometry,
            format!("{error:?}"),
        );
        failure.interval_boxes = interval_boxes;
        failure
    })?;
    let final_certificate = final_mesh.certificate().clone();

    let post_vertices = state.mesh.mesh.vertex_count();
    let post_faces = state.mesh.mesh.triangle_count();
    if post_vertices >= pre_vertices || post_faces >= pre_faces {
        let mut failure = CandidateAttemptFailure::retry(
            ComponentTransactionStage::Postcondition,
            "component transaction did not reduce both vertices and faces".to_string(),
        );
        failure.interval_boxes = interval_boxes;
        return Err(failure);
    }
    state
        .claimed_parents
        .extend(candidate.core_parents.iter().copied());
    state
        .claimed_parents
        .extend(candidate.custom_transition_triangles.keys().copied());
    let post_sources = active_source_mask(&state.mesh, source.mesh.vertices().len());
    let core_sources = source_site_mask_for_parents(source, candidate.core_parents.iter().copied());
    let core_vertices_removed = pre_sources
        .iter()
        .zip(&post_sources)
        .zip(&core_sources)
        .filter(|&((&before, &after), &core)| before && !after && core)
        .count();

    Ok(ComponentCommitReport {
        component_id: component.id,
        before_fingerprint,
        after_fingerprint: state.fingerprint(),
        pre_vertices,
        pre_faces,
        post_vertices,
        post_faces,
        removed_vertices: pre_vertices - post_vertices,
        removed_faces: pre_faces - post_faces,
        core_vertices_removed,
        core_search_states: 0,
        topology_states: transition.report.topology_states,
        elastic_iterations,
        interval_boxes,
        halo_expansions: transition.report.halo_expansions,
        local_geometry,
        global_geometry,
        final_certificate,
        final_cells,
        remap: remap_certificate,
        elastic: elastic_report,
    })
}

#[derive(Clone, Copy, Default)]
struct Counters {
    topology_states: usize,
    elastic_iterations: usize,
    interval_boxes: usize,
    halo_expansions: usize,
}

fn validate_level_mapping(
    source: &MotherGrid,
    level_grid: &MotherGrid,
    level_source_slots: &[Option<usize>],
    component: &HierarchyComponent,
) -> Result<(), String> {
    if level_source_slots.len() != level_grid.mesh.vertices().len() {
        return Err("level source-slot map does not match level grid vertices".into());
    }
    let expected_parent_n = level_grid.subdivision / 2;
    for parent in &component.parents {
        if parent.n != expected_parent_n {
            return Err(format!(
                "component parent {parent:?} is not at level-grid parent subdivision {expected_parent_n}"
            ));
        }
        for child in parent
            .children_2_to_1()
            .ok_or_else(|| format!("invalid component parent {parent:?}"))?
        {
            let face = source_face_slot(level_grid, child)?;
            for level_site in level_grid.mesh.triangles()[face] {
                let source_site = mapped_source_site(level_source_slots, level_site)?;
                if !source.mesh.is_vertex_live(source_site) {
                    return Err(format!(
                        "level site {level_site} maps to inactive source site {source_site}"
                    ));
                }
            }
        }
    }
    Ok(())
}

fn remap_transition_trial(
    mut trial: Box<super::TransitionTopologyTrial>,
    level_source_slots: &[Option<usize>],
) -> Result<Box<super::TransitionTopologyTrial>, String> {
    for source in &mut trial.mesh.source_vertex_slots {
        if let Some(level_site) = *source {
            *source = Some(mapped_source_site(level_source_slots, level_site)?);
        }
    }
    for triangles in trial.candidate.custom_transition_triangles.values_mut() {
        remap_triangles(triangles, level_source_slots)?;
    }
    remap_triangles(&mut trial.candidate.source_triangles, level_source_slots)?;
    remap_sites(
        &mut trial.candidate.source_active_vertices,
        level_source_slots,
    )?;
    let mut degree_forecast = BTreeMap::new();
    for (level_site, degree) in std::mem::take(&mut trial.candidate.source_degree_forecast) {
        let source_site = mapped_source_site(level_source_slots, level_site)?;
        if degree_forecast.insert(source_site, degree).is_some() {
            return Err(format!(
                "multiple level sites map to transition source site {source_site}"
            ));
        }
    }
    trial.candidate.source_degree_forecast = degree_forecast;
    for cycle in trial
        .boundary
        .fine_outer_cycles
        .iter_mut()
        .chain(&mut trial.boundary.coarse_inner_cycles)
    {
        remap_sites(cycle, level_source_slots)?;
    }
    remap_sites(&mut trial.boundary.seam, level_source_slots)?;
    remap_sites(&mut trial.boundary.pentagon, level_source_slots)?;
    Ok(trial)
}

fn guard_face_suffix(failed_guard_face: Option<usize>) -> String {
    failed_guard_face
        .map(|face| format!("; failed guard face {face}"))
        .unwrap_or_default()
}

fn angle_range_suffix(range: Option<(f64, f64)>) -> String {
    range
        .map(|(minimum, maximum)| format!("; global angles {minimum:.12}..{maximum:.12} deg"))
        .unwrap_or_default()
}

fn core_promotion_depths(
    source: &MotherGrid,
    component: &HierarchyComponent,
) -> Result<BTreeMap<TriangleAddress, usize>, String> {
    let parents = component.parents.iter().copied().collect::<BTreeSet<_>>();
    let mut depths = BTreeMap::new();
    let mut queue = VecDeque::new();
    for parent in component.transition_parents.iter().copied() {
        depths.insert(parent, 0);
        queue.push_back(parent);
    }
    while let Some(parent) = queue.pop_front() {
        let next_depth = depths[&parent] + 1;
        for neighbour in hierarchy_parent_neighbours(source, parent)? {
            if parents.contains(&neighbour) && !depths.contains_key(&neighbour) {
                depths.insert(neighbour, next_depth);
                queue.push_back(neighbour);
            }
        }
    }
    if !component.transition_parents.is_empty() && depths.len() != parents.len() {
        return Err("component promotion depths are disconnected".into());
    }
    Ok(depths)
}

fn preferred_core_promotion_for_face(
    mesh: &HierarchyLeafMesh,
    transition: &super::TransitionTopologyTrial,
    failed_face: usize,
) -> Option<TriangleAddress> {
    if !mesh.mesh.is_triangle_live(failed_face) {
        return None;
    }
    let core = transition
        .candidate
        .core_parents
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut seen = vec![false; mesh.mesh.triangles().len()];
    seen[failed_face] = true;
    let mut queue = VecDeque::from([failed_face]);
    while !queue.is_empty() {
        let mut nearest = BTreeSet::new();
        for _ in 0..queue.len() {
            let face = queue.pop_front().expect("current breadth is non-empty");
            if let Some(parent) =
                mesh.triangle_addresses[face].filter(|parent| core.contains(parent))
            {
                nearest.insert(parent);
                continue;
            }
            for neighbour in mesh.mesh.neighbours()[face] {
                if neighbour != 0 && mesh.mesh.is_triangle_live(neighbour) && !seen[neighbour] {
                    seen[neighbour] = true;
                    queue.push_back(neighbour);
                }
            }
        }
        if let Some(parent) = nearest.into_iter().next() {
            return Some(parent);
        }
    }
    None
}

fn remap_triangles(
    triangles: &mut [[usize; 3]],
    level_source_slots: &[Option<usize>],
) -> Result<(), String> {
    for triangle in triangles {
        for site in triangle {
            *site = mapped_source_site(level_source_slots, *site)?;
        }
    }
    Ok(())
}

fn remap_sites(sites: &mut [usize], level_source_slots: &[Option<usize>]) -> Result<(), String> {
    for site in sites {
        *site = mapped_source_site(level_source_slots, *site)?;
    }
    Ok(())
}

fn mapped_source_site(
    level_source_slots: &[Option<usize>],
    level_site: usize,
) -> Result<usize, String> {
    level_source_slots
        .get(level_site)
        .and_then(|source| *source)
        .ok_or_else(|| format!("level site {level_site} has no live source mapping"))
}

fn validate_preflight(
    source: &MotherGrid,
    source_levels: &SourceLevelField,
    state: &ComponentTransactionState,
    component: &HierarchyComponent,
) -> Result<(), String> {
    if state.source_fingerprint != mesh_fingerprint(&source.mesh)
        || state.source_subdivision != source.subdivision
    {
        return Err("transaction state belongs to a different source mesh".into());
    }
    let active_source_sites = source.mesh.active_vertex_slots().collect::<Vec<_>>();
    if source_levels.active_sites() != active_source_sites.as_slice() {
        return Err("source level field active sites do not match source mesh".into());
    }
    let parents = component.parents.iter().copied().collect::<BTreeSet<_>>();
    if parents.len() != component.parents.len() {
        return Err("component contains duplicate parents".into());
    }
    if !parents.is_disjoint(&state.claimed_parents) {
        return Err("component overlaps an already claimed parent".into());
    }
    Ok(())
}

fn validate_physical_eligibility(
    source: &MotherGrid,
    source_levels: &SourceLevelField,
    component: &HierarchyComponent,
    coarse_level: usize,
) -> Result<(), String> {
    for parent in &component.parents {
        visit_source_descendant_faces(source, *parent, &mut |face| {
            for site in source.mesh.triangles()[face] {
                let required = source_level_at_site(source_levels, site)
                    .ok_or_else(|| format!("source site {site} has no physical requirement"))?;
                if required > coarse_level {
                    return Err(format!(
                        "component parent {parent:?} requires level {required}, above coarse level {coarse_level}"
                    ));
                }
            }
            Ok(())
        })?;
    }
    Ok(())
}

fn source_level_at_site(source_levels: &SourceLevelField, site: usize) -> Option<usize> {
    let active = source_levels.active_sites();
    let first = *active.first()?;
    let offset = site.checked_sub(first)?;
    if active.get(offset) == Some(&site) {
        return source_levels.levels().get(offset).copied();
    }
    active
        .binary_search(&site)
        .ok()
        .and_then(|index| source_levels.levels().get(index).copied())
}

fn visit_source_descendant_faces(
    source: &MotherGrid,
    address: TriangleAddress,
    visit: &mut impl FnMut(usize) -> Result<(), String>,
) -> Result<(), String> {
    if address.n == source.subdivision {
        return visit(source_face_slot(source, address)?);
    }
    if address.n == 0
        || address.n > source.subdivision
        || !source.subdivision.is_multiple_of(address.n)
        || !(source.subdivision / address.n).is_power_of_two()
    {
        return Err(format!(
            "hierarchy address {address:?} is not an ancestor of source subdivision {}",
            source.subdivision
        ));
    }
    for child in address
        .children_2_to_1()
        .ok_or_else(|| format!("invalid hierarchy address {address:?}"))?
    {
        visit_source_descendant_faces(source, child, visit)?;
    }
    Ok(())
}

fn install_delta(
    source: &MotherGrid,
    state: &mut ComponentTransactionState,
    candidate: &TransitionTopologyCandidate,
) -> Result<(), String> {
    state.leaf_set.condense_core(&candidate.core_parents)?;
    for (&parent, triangles) in &candidate.custom_transition_triangles {
        if state.custom_transition_triangles.contains_key(&parent) {
            return Err(format!(
                "custom transition parent {parent:?} is already installed"
            ));
        }
        for child in parent
            .children_2_to_1()
            .ok_or_else(|| format!("invalid custom transition parent {parent:?}"))?
        {
            state.leaf_set.leaves.remove(&child);
        }
        state
            .custom_transition_triangles
            .insert(parent, triangles.clone());
    }
    let custom_parents = state
        .custom_transition_triangles
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    let custom_triangles = state
        .custom_transition_triangles
        .values()
        .flat_map(|triangles| triangles.iter().copied())
        .collect::<Vec<_>>();
    state.mesh = rebuild_from_leaf_set_with_custom_triangles(
        source,
        &state.leaf_set,
        &custom_parents,
        &custom_triangles,
    )?;
    Ok(())
}

fn elastic_patch_for_state(
    transition: &super::TransitionTopologyTrial,
    mesh: &HierarchyLeafMesh,
    angle_contract: AngleContractId,
) -> Result<ElasticPatch, String> {
    let domain = match angle_contract {
        AngleContractId::LegacyStrict40To80 => GeometryDomainId::CurrentAnnulus,
        AngleContractId::DomainQuality38To82V1 => GeometryDomainId::PlusTwoOrdinaryRings,
    };
    let base = ElasticPatch::from_transition_with_domain(transition, domain)?;
    let source_to_compact = mesh
        .source_vertex_slots
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(compact, source)| source.map(|source| (source, compact)))
        .collect::<BTreeMap<_, _>>();
    let movable_sources = base
        .movable_compact_vertices
        .iter()
        .map(|&compact| {
            transition.mesh.source_vertex_slots[compact].ok_or_else(|| {
                format!("elastic movable compact vertex {compact} has no source slot")
            })
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let movable_compact_vertices = movable_sources
        .iter()
        .map(|source| {
            source_to_compact.get(source).copied().ok_or_else(|| {
                format!("elastic source vertex {source} is absent from transaction mesh")
            })
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let guard_faces = mesh
        .mesh
        .active_triangle_slots()
        .filter(|&face| {
            mesh.mesh.triangles()[face]
                .iter()
                .any(|site| movable_compact_vertices.contains(site))
        })
        .collect::<BTreeSet<_>>();
    let fixed_compact_vertices = guard_faces
        .iter()
        .flat_map(|&face| mesh.mesh.triangles()[face])
        .filter(|site| !movable_compact_vertices.contains(site))
        .collect::<BTreeSet<_>>();
    Ok(ElasticPatch {
        domain_id: domain,
        topology: base.topology,
        reference_positions: mesh.mesh.vertices().to_vec(),
        fixed_compact_vertices: fixed_compact_vertices.into_iter().collect(),
        movable_compact_vertices: movable_compact_vertices.into_iter().collect(),
        guard_faces: guard_faces.into_iter().collect(),
        target_mode: ElasticTargetMode::TrialReference,
        target_field: ElasticTargetField::default(),
    })
}

fn apply_source_positions(mesh: &mut HierarchyLeafMesh, positions: &[CartesianPoint]) {
    for (compact, source) in mesh.source_vertex_slots.iter().copied().enumerate() {
        if let Some(position) = source.and_then(|source| positions.get(source).copied()) {
            mesh.mesh.move_vertex(compact, position);
        }
    }
}

fn apply_elastic(state: &mut ComponentTransactionState, elastic: &ElasticBlockTrial) {
    state.mesh = elastic.mesh.clone();
    for (compact, source) in state.mesh.source_vertex_slots.iter().copied().enumerate() {
        if let Some(source_slot) = source {
            state.source_positions[source_slot] = state.mesh.mesh.vertices()[compact];
        }
    }
}

fn lower_covered_source_levels(
    source: &MotherGrid,
    state: &mut ComponentTransactionState,
    candidate: &TransitionTopologyCandidate,
    boundary: &super::TransitionBoundary,
    coarse_level: usize,
) {
    let fixed_fine = boundary
        .fine_outer_cycles
        .iter()
        .flat_map(|cycle| cycle.iter().copied())
        .collect::<BTreeSet<_>>();
    for parent in candidate
        .core_parents
        .iter()
        .copied()
        .chain(candidate.custom_transition_triangles.keys().copied())
    {
        let _ = visit_source_descendant_faces(source, parent, &mut |face| {
            for source_site in source.mesh.triangles()[face] {
                if fixed_fine.contains(&source_site) {
                    continue;
                }
                if let Some(level) = state
                    .source_delivered_levels
                    .get_mut(source_site)
                    .and_then(Option::as_mut)
                {
                    *level = (*level).min(coarse_level);
                }
            }
            Ok(())
        });
    }
}

fn active_source_mask(mesh: &HierarchyLeafMesh, source_slots: usize) -> Vec<bool> {
    let mut active = vec![false; source_slots];
    for source in mesh.source_vertex_slots.iter().flatten().copied() {
        if let Some(slot) = active.get_mut(source) {
            *slot = true;
        }
    }
    active
}

fn affected_faces(
    source: &MotherGrid,
    mesh: &HierarchyLeafMesh,
    candidate: &TransitionTopologyCandidate,
) -> BTreeSet<usize> {
    let sources = if candidate.source_active_vertices.is_empty() {
        candidate_source_sites(source, candidate)
    } else {
        candidate.source_active_vertices.iter().copied().collect()
    };
    mesh.mesh
        .active_triangle_slots()
        .filter(|&face| {
            mesh.mesh.triangles()[face].iter().any(|&compact| {
                mesh.source_vertex_slots[compact]
                    .is_some_and(|source_site| sources.contains(&source_site))
            })
        })
        .collect()
}

fn candidate_source_sites(
    source: &MotherGrid,
    candidate: &TransitionTopologyCandidate,
) -> BTreeSet<usize> {
    source_sites_for_parents(
        source,
        candidate
            .core_parents
            .iter()
            .copied()
            .chain(candidate.custom_transition_triangles.keys().copied()),
    )
}

fn source_sites_for_parents(
    source: &MotherGrid,
    parents: impl IntoIterator<Item = TriangleAddress>,
) -> BTreeSet<usize> {
    let mut sources = BTreeSet::new();
    for parent in parents {
        let _ = visit_source_descendant_faces(source, parent, &mut |face| {
            sources.extend(source.mesh.triangles()[face]);
            Ok(())
        });
    }
    sources
}

fn source_site_mask_for_parents(
    source: &MotherGrid,
    parents: impl IntoIterator<Item = TriangleAddress>,
) -> Vec<bool> {
    let mut sources = vec![false; source.mesh.vertices().len()];
    for parent in parents {
        let _ = visit_source_descendant_faces(source, parent, &mut |face| {
            for site in source.mesh.triangles()[face] {
                sources[site] = true;
            }
            Ok(())
        });
    }
    sources
}

fn target_levels_for(
    mesh: &MeshState,
    source_slots: &[Option<usize>],
    source_levels: &[Option<usize>],
) -> Result<TargetLevelField, String> {
    let levels = mesh
        .active_vertex_slots()
        .map(|site| {
            let source = source_slots
                .get(site)
                .and_then(|slot| *slot)
                .ok_or_else(|| format!("active target site {site} has no source slot"))?;
            source_levels
                .get(source)
                .and_then(|level| *level)
                .ok_or_else(|| format!("source site {source} has no delivered level"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    TargetLevelField::from_active_voronoi_cells(mesh, levels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_component_certifies_only_its_transition_neighbourhood() {
        let source = MotherGrid::generate(4).unwrap();
        let face = source.mesh.active_triangle_slots().next().unwrap();
        let transition_sources = source.mesh.triangles()[face].to_vec();
        let core_parents = source
            .triangle_addresses
            .iter()
            .flatten()
            .filter_map(|address| address.parent_2_to_1())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let mut mesh = HierarchyLeafMesh {
            mesh: source.mesh.clone(),
            triangle_addresses: source.triangle_addresses.clone(),
            source_vertex_slots: source
                .mesh
                .vertices()
                .iter()
                .enumerate()
                .map(|(site, _)| source.mesh.is_vertex_live(site).then_some(site))
                .collect(),
        };
        let remote = mesh
            .mesh
            .active_vertex_slots()
            .find(|site| !transition_sources.contains(site))
            .unwrap();
        let position = mesh.mesh.vertices()[remote];
        mesh.mesh.move_vertex(
            remote,
            CartesianPoint::new(position.x + f64::EPSILON, position.y, position.z),
        );
        let candidate = TransitionTopologyCandidate {
            component_id: 1,
            topology_id: 1,
            core_parents,
            custom_transition_triangles: BTreeMap::new(),
            source_triangles: vec![source.mesh.triangles()[face]],
            source_active_vertices: transition_sources.clone(),
            source_degree_forecast: BTreeMap::new(),
        };

        let affected = affected_faces(&source, &mesh, &candidate);
        let expected = source
            .mesh
            .active_triangle_slots()
            .filter(|&candidate_face| {
                source.mesh.triangles()[candidate_face]
                    .iter()
                    .any(|site| transition_sources.contains(site))
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(affected, expected);
        assert!(affected.len() < source.mesh.triangle_count());
    }

    #[test]
    fn level_mapping_uses_live_vertices_from_finer_faces() {
        let source = MotherGrid::generate(8).unwrap();
        let state = ComponentTransactionState::new(&source, 3).unwrap();
        let level = MotherGrid::generate(4).unwrap();
        let slots = state.level_source_slots(&source, &level).unwrap();
        assert!(level
            .mesh
            .active_vertex_slots()
            .all(|site| slots[site].is_some()));
    }

    #[test]
    fn failed_face_maps_to_the_nearest_core_face_across_the_transition() {
        let grid = MotherGrid::generate(2).unwrap();
        let core_face = grid.mesh.active_triangle_slots().next().unwrap();
        let core_parent = grid.triangle_addresses[core_face].unwrap();
        let failed_face = grid
            .mesh
            .active_triangle_slots()
            .find(|&face| {
                face != core_face
                    && !grid.mesh.neighbours()[face].contains(&core_face)
                    && grid.mesh.triangles()[face]
                        .iter()
                        .all(|site| !grid.mesh.triangles()[core_face].contains(site))
            })
            .unwrap();
        let mesh = HierarchyLeafMesh {
            mesh: grid.mesh.clone(),
            triangle_addresses: grid.triangle_addresses.clone(),
            source_vertex_slots: (0..grid.mesh.vertices().len())
                .map(|site| grid.mesh.is_vertex_live(site).then_some(site))
                .collect(),
        };
        let transition = super::super::TransitionTopologyTrial {
            mesh: mesh.clone(),
            boundary: super::super::TransitionBoundary::default(),
            candidate: TransitionTopologyCandidate {
                component_id: 1,
                topology_id: 0,
                core_parents: vec![core_parent],
                custom_transition_triangles: BTreeMap::new(),
                source_triangles: Vec::new(),
                source_active_vertices: Vec::new(),
                source_degree_forecast: BTreeMap::new(),
            },
            report: super::super::TransitionTopologyReport::default(),
        };

        assert_eq!(
            preferred_core_promotion_for_face(&mesh, &transition, failed_face),
            Some(core_parent)
        );
    }
}
