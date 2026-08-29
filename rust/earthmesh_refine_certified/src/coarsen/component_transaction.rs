//! Component-scoped, rollback-first coarsening transaction core.
//!
//! This module is deliberately small: topology chooses faces, elastic may move
//! only transition coordinates, then the normal geometry/final-cell/remap gates
//! decide whether the cloned state is committed.

use super::{
    core_condensation::rebuild_from_leaf_set_with_custom_triangles,
    core_condensation::source_face_slot, solve_elastic_patch, solve_transition_topology,
    ElasticBlockLimits, ElasticBlockOutcome, ElasticBlockReport, ElasticBlockTrial, ElasticPatch,
    HierarchyComponent, HierarchyLeafMesh, HierarchyLeafSet, TransitionTopologyCandidate,
    TransitionTopologyLimits, TransitionTopologyOutcome,
};
use crate::{
    certificate::{
        Certificate, FinalCertificateReport, GeometryCertificateReport,
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
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentTransactionLimits {
    pub topology_states: usize,
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
    pub core_search_states: usize,
    pub topology_states: usize,
    pub elastic_iterations: usize,
    pub interval_boxes: usize,
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
    let snapshot = state.clone();
    let before_fingerprint = snapshot.fingerprint();
    let pre_vertices = snapshot.mesh.mesh.vertex_count();
    let pre_faces = snapshot.mesh.mesh.triangle_count();
    let mut counters = Counters::default();

    macro_rules! fail {
        ($variant:ident, $stage:expr, $reason:expr) => {{
            *state = snapshot;
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
            })
        }};
    }

    if let Err(reason) = validate_preflight(source, source_levels, state, component) {
        return fail!(InvalidInput, ComponentTransactionStage::Preflight, reason);
    }
    if let Err(reason) =
        validate_physical_eligibility(source, source_levels, component, coarse_level)
    {
        return fail!(NotCertifiable, ComponentTransactionStage::Physical, reason);
    }

    let transition = match solve_transition_topology(
        source,
        component,
        TransitionTopologyLimits {
            topology_states: limits.topology_states,
            maximum_halo_expansions: limits.halo_expansions,
        },
    ) {
        TransitionTopologyOutcome::Closed(trial) => {
            counters.topology_states = trial.report.topology_states;
            trial
        }
        TransitionTopologyOutcome::RequiresWiderHalo {
            states_examined, ..
        } => {
            counters.topology_states = states_examined;
            return fail!(
                RequiresWiderHalo,
                ComponentTransactionStage::Topology,
                "component needs a wider transition halo".to_string()
            );
        }
        TransitionTopologyOutcome::SearchBudgetExhausted {
            states_examined, ..
        } => {
            counters.topology_states = states_examined;
            return fail!(
                SearchBudgetExhausted,
                ComponentTransactionStage::Topology,
                "transition topology budget exhausted".to_string()
            );
        }
        TransitionTopologyOutcome::ProvenInfeasible {
            states_examined,
            reason,
            ..
        } => {
            counters.topology_states = states_examined;
            return fail!(NoTopology, ComponentTransactionStage::Topology, reason);
        }
        TransitionTopologyOutcome::InvalidBoundary {
            states_examined,
            reason,
            ..
        } => {
            counters.topology_states = states_examined;
            return fail!(InvalidInput, ComponentTransactionStage::Topology, reason);
        }
    };

    let candidate = transition.candidate.clone();
    if let Err(reason) = install_delta(source, state, &candidate) {
        return fail!(
            InvalidInput,
            ComponentTransactionStage::InstallDelta,
            reason
        );
    }
    apply_source_positions(&mut state.mesh, &state.source_positions);

    let mut elastic_report = None;
    if Certificate::internal()
        .verify_geometry(&state.mesh.mesh)
        .is_err()
    {
        if transition.candidate.custom_transition_triangles.is_empty() {
            return fail!(
                NotCertifiable,
                ComponentTransactionStage::GlobalGeometry,
                "exact coarse core failed internal geometry certification".to_string()
            );
        }
        let patch = match elastic_patch_for_state(&transition, &state.mesh) {
            Ok(patch) => patch,
            Err(reason) => {
                return fail!(
                    ElasticNoImprovement,
                    ComponentTransactionStage::Elastic,
                    reason
                )
            }
        };
        let elastic = match solve_elastic_patch(
            &state.mesh,
            patch,
            ElasticBlockLimits {
                elastic_iterations: limits.elastic_iterations,
            },
        ) {
            ElasticBlockOutcome::Certified(trial) => trial,
            ElasticBlockOutcome::ElasticNoImprovement {
                elastic_iterations,
                reason,
                ..
            } => {
                counters.elastic_iterations = elastic_iterations;
                return fail!(
                    ElasticNoImprovement,
                    ComponentTransactionStage::Elastic,
                    reason
                );
            }
            ElasticBlockOutcome::SearchBudgetExhausted {
                elastic_iterations, ..
            } => {
                counters.elastic_iterations = elastic_iterations;
                return fail!(
                    SearchBudgetExhausted,
                    ComponentTransactionStage::Elastic,
                    "elastic iteration budget exhausted".to_string()
                );
            }
            ElasticBlockOutcome::InvalidPatch { reason } => {
                return fail!(InvalidInput, ComponentTransactionStage::Elastic, reason);
            }
        };
        counters.elastic_iterations = elastic.report.elastic_iterations;
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
    let guard_faces = affected_faces(source, &state.mesh, &candidate);
    counters.interval_boxes = guard_faces.len().saturating_mul(3);
    if counters.interval_boxes > limits.interval_boxes {
        return fail!(
            SearchBudgetExhausted,
            ComponentTransactionStage::LocalGeometry,
            "local geometry interval-box budget exhausted".to_string()
        );
    }
    let local_geometry =
        match Certificate::internal().verify_geometry_region(&state.mesh.mesh, &guard_faces) {
            Ok(report) => report,
            Err(error) => {
                return fail!(
                    NotCertifiable,
                    ComponentTransactionStage::LocalGeometry,
                    format!("{error:?}")
                )
            }
        };
    debug_assert_eq!(counters.interval_boxes, local_geometry.interval_boxes);

    let global_geometry = match Certificate::internal().verify_geometry(&state.mesh.mesh) {
        Ok(report) => report,
        Err(error) => {
            return fail!(
                NotCertifiable,
                ComponentTransactionStage::GlobalGeometry,
                format!("{error:?}")
            )
        }
    };
    let final_geometry = match Certificate::final_delivery().verify_geometry(&state.mesh.mesh) {
        Ok(report) => report,
        Err(error) => {
            return fail!(
                NotCertifiable,
                ComponentTransactionStage::FinalGeometry,
                format!("{error:?}")
            )
        }
    };

    let target_levels = match state.target_levels() {
        Ok(levels) => levels,
        Err(reason) => return fail!(InvalidInput, ComponentTransactionStage::FinalCells, reason),
    };
    let remap = match ConservativeRemap::between_voronoi_meshes(&source.mesh, &state.mesh.mesh) {
        Ok(remap) => remap,
        Err(reason) => return fail!(NotCertifiable, ComponentTransactionStage::Remap, reason),
    };
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
            return fail!(InvalidInput, ComponentTransactionStage::FinalCells, reason);
        }
        Err(FinalCellRequirementError::Residuals(report)) => {
            return fail!(
                NotCertifiable,
                ComponentTransactionStage::FinalCells,
                format!(
                    "{} physical and {} balance residual(s)",
                    report.physical_residuals(),
                    report.balance_residuals()
                )
            );
        }
    };
    let final_evidence =
        match FinalCertificationEvidence::from_final_cells(&final_cells, remap_certificate.clone())
        {
            Ok(evidence) => evidence,
            Err(reason) => return fail!(NotCertifiable, ComponentTransactionStage::Remap, reason),
        };

    let final_mesh = match crate::finalize_geometry_certified_mother(
        GeometryCertifiedMotherGrid::new(state.mesh.mesh.clone(), final_geometry),
        final_evidence,
    ) {
        Ok(mesh) => mesh,
        Err(error) => {
            return fail!(
                NotCertifiable,
                ComponentTransactionStage::FinalGeometry,
                format!("{error:?}")
            )
        }
    };
    let final_certificate = final_mesh.certificate().clone();

    let post_vertices = state.mesh.mesh.vertex_count();
    let post_faces = state.mesh.mesh.triangle_count();
    if post_vertices >= pre_vertices || post_faces >= pre_faces {
        return fail!(
            NotCertifiable,
            ComponentTransactionStage::Postcondition,
            "component transaction did not reduce both vertices and faces".to_string()
        );
    }
    state
        .claimed_parents
        .extend(component.parents.iter().copied());

    ComponentTransactionOutcome::Certified(Box::new(ComponentCommitReport {
        component_id: component.id,
        before_fingerprint,
        after_fingerprint: state.fingerprint(),
        pre_vertices,
        pre_faces,
        post_vertices,
        post_faces,
        removed_vertices: pre_vertices - post_vertices,
        removed_faces: pre_faces - post_faces,
        core_search_states: 0,
        topology_states: counters.topology_states,
        elastic_iterations: counters.elastic_iterations,
        interval_boxes: counters.interval_boxes,
        local_geometry,
        global_geometry,
        final_certificate,
        final_cells,
        remap: remap_certificate,
        elastic: elastic_report,
    }))
}

#[derive(Clone, Copy, Default)]
struct Counters {
    topology_states: usize,
    elastic_iterations: usize,
    interval_boxes: usize,
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
    let levels_by_site = source_levels
        .active_sites()
        .iter()
        .copied()
        .zip(source_levels.levels().iter().copied())
        .collect::<BTreeMap<_, _>>();
    for parent in &component.parents {
        let children = parent
            .children_2_to_1()
            .ok_or_else(|| format!("invalid component parent {parent:?}"))?;
        for child in children {
            let face = source_face_slot(source, child)?;
            for site in source.mesh.triangles()[face] {
                let required = levels_by_site
                    .get(&site)
                    .copied()
                    .ok_or_else(|| format!("source site {site} has no physical requirement"))?;
                if required > coarse_level {
                    return Err(format!(
                        "component parent {parent:?} requires level {required}, above coarse level {coarse_level}"
                    ));
                }
            }
        }
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
) -> Result<ElasticPatch, String> {
    let base = ElasticPatch::from_transition(transition)?;
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
        topology: base.topology,
        reference_positions: mesh.mesh.vertices().to_vec(),
        fixed_compact_vertices: fixed_compact_vertices.into_iter().collect(),
        movable_compact_vertices: movable_compact_vertices.into_iter().collect(),
        guard_faces: guard_faces.into_iter().collect(),
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
    for source_site in candidate_source_sites(source, candidate)
        .into_iter()
        .filter(|site| !fixed_fine.contains(site))
    {
        if let Some(level) = state
            .source_delivered_levels
            .get_mut(source_site)
            .and_then(Option::as_mut)
        {
            *level = (*level).min(coarse_level);
        }
    }
}

fn affected_faces(
    source: &MotherGrid,
    mesh: &HierarchyLeafMesh,
    candidate: &TransitionTopologyCandidate,
) -> BTreeSet<usize> {
    let sources = candidate_source_sites(source, candidate);
    mesh.mesh
        .active_triangle_slots()
        .filter(|&face| {
            mesh.mesh.triangles()[face].iter().any(|&compact| {
                mesh.source_vertex_slots[compact].is_some_and(|source| sources.contains(&source))
            })
        })
        .collect()
}

fn candidate_source_sites(
    source: &MotherGrid,
    candidate: &TransitionTopologyCandidate,
) -> BTreeSet<usize> {
    let mut sources = BTreeSet::new();
    for parent in candidate
        .core_parents
        .iter()
        .chain(candidate.custom_transition_triangles.keys())
    {
        if let Some(children) = parent.children_2_to_1() {
            for child in children {
                if let Ok(face) = source_face_slot(source, child) {
                    sources.extend(source.mesh.triangles()[face]);
                }
            }
        }
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
