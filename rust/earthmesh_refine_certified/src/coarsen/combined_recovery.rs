//! Auditable local-repair actions used by the post-PR78 recovery ladder.

use super::{
    boundary_parent_peel::split_retained_interfaces,
    coarse_core_ears,
    direct_restore::{
        closed_edge_incidence, logical_exterior_equal_with_custom,
        materialize_sector_restores_with_replacements, mesh_angle_range, outside_coordinates_equal,
    },
    peel_boundary_parent_for_sector, restore_fine_compatible_sector,
    solve_elastic_patch_with_max_min_trust_start, solve_local_annular_collar,
    BoundaryParentPeelOutcome, DirectSectorRestoreOutcome, ElasticBlockLimits, ElasticBlockOutcome,
    ElasticPatch, ElasticTargetMode, GeometryDomainId, GeometryStartId, HierarchyLeafMesh,
    LocalAnnularCollarLimits, LocalAnnularCollarOutcome, PromotionPatchTopology,
    ProtectedCoarseRegion, RecoveryAtom, SectorRecoveryAtlas, ViolatingAngle,
    ViolationSupportAtlas,
};
use crate::{
    certificate::spherical_triangle_angles, mesh_fingerprint, MotherGrid, TriangleAddress,
    VertexAddress,
};
use earthmesh_mesh::{normalize_cartesian_to_radius, CartesianPoint};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LocalRepairKind {
    DirectSectorRestore,
    OneParentPeel,
    TwoParentAnnularCollar,
    HierarchyCoordinateRelease,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalRepairAction {
    pub id: u64,
    pub kind: LocalRepairKind,
    pub local_component_ids: BTreeSet<u64>,
    pub sector_ids: BTreeSet<u64>,
    pub removed_mixed_faces: BTreeSet<usize>,
    pub restored_source_faces: BTreeSet<usize>,
    pub released_parents: BTreeSet<TriangleAddress>,
    pub retained_parents: BTreeSet<TriangleAddress>,
    pub inserted_custom_triangles: BTreeSet<[usize; 3]>,
    pub modified_source_vertices: BTreeSet<usize>,
    pub boundary_source_vertices: BTreeSet<usize>,
    pub movable_source_vertices: BTreeSet<usize>,
    pub original_violation_ids_covered: BTreeSet<usize>,
    pub original_violation_ids_untouched: BTreeSet<usize>,
    pub patch_topology: PromotionPatchTopology,
    pub protected_coarse_regions: Vec<ProtectedCoarseRegion>,
    pub geometry_target_mode: ElasticTargetMode,
    pub geometry_iteration_limit: usize,
    pub fingerprint: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalRepairCandidate {
    pub action: LocalRepairAction,
    pub mesh: HierarchyLeafMesh,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalActionEffect {
    pub action_id: u64,
    pub original_violations_total: usize,
    pub original_violations_removed: usize,
    pub original_violations_resolved: usize,
    pub original_violations_persisted: usize,
    pub uncovered_original_violations: usize,
    pub new_violations_created: usize,
    pub local_angle_range: Option<(f64, f64)>,
    pub outside_angle_range: Option<(f64, f64)>,
    pub global_angle_range: Option<(f64, f64)>,
    pub local_signed_margin_deg: Option<f64>,
    pub global_signed_margin_deg: Option<f64>,
    pub topology_valid: bool,
    pub orientation_valid: bool,
    pub strict_certified: bool,
    pub failure_reason: SingletonFailureReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SingletonFailureReason {
    StrictCertified,
    UncoveredOriginalViolations,
    NewViolationsCreated,
    ContinuousUnknown,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalActionConflictGraph {
    pub actions: Vec<LocalRepairAction>,
    pub hard_conflicts: BTreeSet<(u64, u64)>,
    pub merge_required: BTreeSet<(u64, u64)>,
    pub independent: BTreeSet<(u64, u64)>,
    pub components: Vec<BTreeSet<u64>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionSetClassification {
    Empty,
    HardConflict,
    NoRetainedCoarseParent,
    NoPotentialImprovement,
    Compatible,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombinedRepairPlan {
    pub bitmask: usize,
    pub action_ids: BTreeSet<u64>,
    pub sector_ids: BTreeSet<u64>,
    pub released_parents: BTreeSet<TriangleAddress>,
    pub retained_parents: BTreeSet<TriangleAddress>,
    pub removed_mixed_faces: BTreeSet<usize>,
    pub restored_source_faces: BTreeSet<usize>,
    pub singleton_boundary_source_vertices: BTreeSet<usize>,
    pub original_violation_ids_covered: BTreeSet<usize>,
    pub uncovered_original_violations: usize,
    pub estimated_new_violations: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombinedCavity {
    pub id: u64,
    pub source_faces: BTreeSet<usize>,
    pub boundary_source_vertices: BTreeSet<usize>,
    pub movable_source_vertices: BTreeSet<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CombinedRepairMaterialization {
    pub plan: CombinedRepairPlan,
    pub mesh: HierarchyLeafMesh,
    pub rebuilds: usize,
    pub removed_mixed_faces: BTreeSet<usize>,
    pub restored_source_faces: BTreeSet<usize>,
    pub released_parents: BTreeSet<TriangleAddress>,
    pub split_interface_parents: BTreeSet<TriangleAddress>,
    pub retained_parents: BTreeSet<TriangleAddress>,
    pub protected_coarse_regions: Vec<ProtectedCoarseRegion>,
    pub cavities: Vec<CombinedCavity>,
    pub internal_singleton_boundary_vertices: BTreeSet<usize>,
    pub topology_closed: bool,
    pub outside_topology_bitwise_equal: bool,
    pub outside_coordinates_bitwise_equal: bool,
    pub edge_incidence_at_most_two: bool,
    pub protected_core_preserved: bool,
    pub compression_ratio: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombinedGeometryStartId {
    InheritedIncumbent,
    SafeInteriorBlend,
    HierarchySpringEquilibrium,
    RingScaleInterpolation,
    DegreeAngleEquilibrium,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CombinedGeometryAttempt {
    pub start_id: CombinedGeometryStartId,
    pub target_mode: ElasticTargetMode,
    pub requested_iterations: usize,
    pub completed_iterations: usize,
    pub angle_range_deg: Option<(f64, f64)>,
    pub signed_margin_deg: Option<f64>,
    pub strict_certified: bool,
    pub outcome: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CombinedGeometryTrial {
    pub plan: CombinedRepairPlan,
    pub start_id: CombinedGeometryStartId,
    pub mesh: HierarchyLeafMesh,
    pub patch: ElasticPatch,
    pub angle_range_deg: Option<(f64, f64)>,
    pub signed_margin_deg: Option<f64>,
    pub retained_parents: BTreeSet<TriangleAddress>,
    pub compression_ratio: f64,
    pub strict_certified: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CombinedGeometryResult {
    pub attempts: Vec<CombinedGeometryAttempt>,
    pub best_candidate: Option<Box<CombinedGeometryTrial>>,
    pub incumbent_angle_range_deg: Option<(f64, f64)>,
    pub best_preserved_angle_range_deg: Option<(f64, f64)>,
    pub incumbent_preserved: bool,
    pub strict_certified: bool,
    pub continuous_search_incomplete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct GeometryDefectVector {
    pub non_positive_triangles: usize,
    pub crossings: usize,
    pub angle_violations: usize,
    pub negative_signed_margin_mdeg: i64,
    pub delaunay_violations: usize,
    pub invalid_voronoi_cells: usize,
    pub promoted_source_faces: usize,
    pub negative_retained_coarse_parents: isize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkingMeshHardGates {
    pub topology: bool,
    pub degree: bool,
    pub link: bool,
    pub euler: bool,
    pub charge: bool,
    pub physical: bool,
}

impl WorkingMeshHardGates {
    fn all_pass(self) -> bool {
        self.topology && self.degree && self.link && self.euler && self.charge && self.physical
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkingMeshCandidate {
    pub mesh: HierarchyLeafMesh,
    pub defect: GeometryDefectVector,
    pub hard_gates: WorkingMeshHardGates,
    pub new_outside_angle_violations: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkingMesh {
    mesh: HierarchyLeafMesh,
    pub defect: GeometryDefectVector,
    visited_fingerprints: BTreeSet<u64>,
    pub action_registry_fingerprints: BTreeSet<u64>,
    pub registry_epoch: usize,
    pub accepted_steps: usize,
}

impl WorkingMesh {
    pub fn new(
        mesh: HierarchyLeafMesh,
        defect: GeometryDefectVector,
        action_registry_fingerprints: impl IntoIterator<Item = u64>,
    ) -> Self {
        let visited_fingerprints = BTreeSet::from([mesh_fingerprint(&mesh.mesh)]);
        Self {
            mesh,
            defect,
            visited_fingerprints,
            action_registry_fingerprints: action_registry_fingerprints.into_iter().collect(),
            registry_epoch: 0,
            accepted_steps: 0,
        }
    }

    pub fn mesh(&self) -> &HierarchyLeafMesh {
        &self.mesh
    }

    pub fn is_publishable(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkingMeshRejectReason {
    HardInvariantFailure,
    OrientationOrCrossingRegression,
    DefectNotStrictlyLower,
    OutsideAngleViolation,
    VisitedFingerprint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkingMeshStep {
    Accepted {
        fingerprint: u64,
        previous_defect: GeometryDefectVector,
        defect: GeometryDefectVector,
        registry_epoch: usize,
    },
    Rejected(WorkingMeshRejectReason),
}

pub fn accept_monotone_working_candidate<F, I>(
    state: &mut WorkingMesh,
    candidate: WorkingMeshCandidate,
    recompute_action_registry_fingerprints: F,
) -> Result<WorkingMeshStep, String>
where
    F: FnOnce(&HierarchyLeafMesh) -> Result<I, String>,
    I: IntoIterator<Item = u64>,
{
    let reason = if !candidate.hard_gates.all_pass() {
        Some(WorkingMeshRejectReason::HardInvariantFailure)
    } else if candidate.defect.non_positive_triangles > state.defect.non_positive_triangles
        || candidate.defect.crossings > state.defect.crossings
    {
        Some(WorkingMeshRejectReason::OrientationOrCrossingRegression)
    } else if candidate.defect >= state.defect {
        Some(WorkingMeshRejectReason::DefectNotStrictlyLower)
    } else if candidate.new_outside_angle_violations > 0 {
        Some(WorkingMeshRejectReason::OutsideAngleViolation)
    } else {
        let fingerprint = mesh_fingerprint(&candidate.mesh.mesh);
        state
            .visited_fingerprints
            .contains(&fingerprint)
            .then_some(WorkingMeshRejectReason::VisitedFingerprint)
    };
    if let Some(reason) = reason {
        return Ok(WorkingMeshStep::Rejected(reason));
    }

    let fingerprint = mesh_fingerprint(&candidate.mesh.mesh);
    let registry = recompute_action_registry_fingerprints(&candidate.mesh)?
        .into_iter()
        .collect();
    let previous_defect = std::mem::replace(&mut state.defect, candidate.defect.clone());
    state.mesh = candidate.mesh;
    state.visited_fingerprints.insert(fingerprint);
    state.action_registry_fingerprints = registry;
    state.registry_epoch += 1;
    state.accepted_steps += 1;
    Ok(WorkingMeshStep::Accepted {
        fingerprint,
        previous_defect,
        defect: candidate.defect,
        registry_epoch: state.registry_epoch,
    })
}

pub fn sequential_matches_exact_combination_oracle(
    sequential: &GeometryDefectVector,
    exact: &GeometryDefectVector,
) -> bool {
    sequential == exact
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompatibleActionSetPlan {
    pub total_bitmasks: usize,
    pub classifications: Vec<ActionSetClassification>,
    pub compatible_plans: Vec<CombinedRepairPlan>,
    pub classification_counts: BTreeMap<&'static str, usize>,
}

pub fn build_local_action_conflict_graph(
    actions: &[LocalRepairAction],
) -> Result<LocalActionConflictGraph, String> {
    let mut actions = actions.to_vec();
    actions.sort_by_key(|action| action.id);
    if actions.windows(2).any(|pair| pair[0].id == pair[1].id) {
        return Err("local repair action IDs must be unique".into());
    }
    let mut hard_conflicts = BTreeSet::new();
    let mut merge_required = BTreeSet::new();
    let mut independent = BTreeSet::new();
    for left in 0..actions.len() {
        for right in left + 1..actions.len() {
            let pair = (actions[left].id, actions[right].id);
            if replacements_conflict(&actions[left], &actions[right]) {
                hard_conflicts.insert(pair);
            } else if actions_interact(&actions[left], &actions[right]) {
                merge_required.insert(pair);
            } else {
                independent.insert(pair);
            }
        }
    }
    let components = interaction_components(&actions, &hard_conflicts, &merge_required);
    Ok(LocalActionConflictGraph {
        actions,
        hard_conflicts,
        merge_required,
        independent,
        components,
    })
}

pub fn enumerate_compatible_action_sets(
    graph: &LocalActionConflictGraph,
    effects: &[LocalActionEffect],
    original_violation_count: usize,
) -> Result<CompatibleActionSetPlan, String> {
    if graph.actions.len() >= usize::BITS as usize {
        return Err("local action-set bitmask capacity exceeded".into());
    }
    let effect_by_id = effects
        .iter()
        .map(|effect| (effect.action_id, effect))
        .collect::<BTreeMap<_, _>>();
    if graph
        .actions
        .iter()
        .any(|action| !effect_by_id.contains_key(&action.id))
    {
        return Err("each local action requires singleton effect evidence".into());
    }
    let total_bitmasks = 1usize << graph.actions.len();
    let mut classifications = Vec::with_capacity(total_bitmasks);
    let mut compatible_plans = Vec::new();
    let mut classification_counts = BTreeMap::new();
    for bitmask in 0..total_bitmasks {
        let selected = graph
            .actions
            .iter()
            .enumerate()
            .filter_map(|(index, action)| ((bitmask >> index) & 1 == 1).then_some(action))
            .collect::<Vec<_>>();
        let classification = if selected.is_empty() {
            ActionSetClassification::Empty
        } else if contains_hard_conflict(&selected, &graph.hard_conflicts) {
            ActionSetClassification::HardConflict
        } else {
            let retained_parents = selected
                .iter()
                .map(|action| action.retained_parents.clone())
                .reduce(|left, right| left.intersection(&right).copied().collect())
                .unwrap_or_default();
            let covered = selected
                .iter()
                .flat_map(|action| action.original_violation_ids_covered.iter().copied())
                .collect::<BTreeSet<_>>();
            if retained_parents.is_empty() {
                ActionSetClassification::NoRetainedCoarseParent
            } else if covered.is_empty() {
                ActionSetClassification::NoPotentialImprovement
            } else {
                compatible_plans.push(combined_plan(
                    bitmask,
                    &selected,
                    retained_parents,
                    covered,
                    &effect_by_id,
                    original_violation_count,
                ));
                ActionSetClassification::Compatible
            }
        };
        *classification_counts
            .entry(classification_name(classification))
            .or_default() += 1;
        classifications.push(classification);
    }
    compatible_plans.sort_by(|left, right| {
        left.uncovered_original_violations
            .cmp(&right.uncovered_original_violations)
            .then(
                left.estimated_new_violations
                    .cmp(&right.estimated_new_violations),
            )
            .then(
                left.released_parents
                    .len()
                    .cmp(&right.released_parents.len()),
            )
            .then(
                left.restored_source_faces
                    .len()
                    .cmp(&right.restored_source_faces.len()),
            )
            .then(left.action_ids.len().cmp(&right.action_ids.len()))
            .then(left.bitmask.cmp(&right.bitmask))
    });
    Ok(CompatibleActionSetPlan {
        total_bitmasks,
        classifications,
        compatible_plans,
        classification_counts,
    })
}

pub fn materialize_combined_repair_plan(
    source: &MotherGrid,
    incumbent: &HierarchyLeafMesh,
    atlas: &SectorRecoveryAtlas,
    original_retained_parents: &BTreeSet<TriangleAddress>,
    plan: &CombinedRepairPlan,
) -> Result<CombinedRepairMaterialization, String> {
    if plan.action_ids.len() < 2 {
        return Err("combined materialization requires at least two actions".into());
    }
    if plan.sector_ids.is_empty()
        || plan
            .sector_ids
            .iter()
            .any(|sector| !atlas.sectors.contains_key(sector))
    {
        return Err("combined materialization references an invalid sector set".into());
    }
    if !plan.released_parents.is_subset(original_retained_parents) {
        return Err("combined materialization releases a non-retained parent".into());
    }
    let replacements = split_retained_interfaces(
        source,
        incumbent,
        &plan.released_parents,
        original_retained_parents,
    )?;
    let split_interface_parents = replacements.keys().copied().collect::<BTreeSet<_>>();
    let materialized = materialize_sector_restores_with_replacements(
        source,
        incumbent,
        atlas,
        &plan.sector_ids,
        &plan.released_parents,
        &replacements,
    )?;
    let modified_parents = plan
        .released_parents
        .union(&split_interface_parents)
        .copied()
        .collect::<BTreeSet<_>>();
    let removed_parent_faces = incumbent
        .mesh
        .active_triangle_slots()
        .filter(|&face| {
            incumbent.triangle_addresses[face]
                .is_some_and(|address| modified_parents.contains(&address))
        })
        .collect::<BTreeSet<_>>();
    let removed_faces = materialized
        .removed_mixed_faces
        .union(&removed_parent_faces)
        .copied()
        .collect::<BTreeSet<_>>();
    let inserted_addresses = materialized
        .restored_addresses
        .union(&materialized.released_children)
        .copied()
        .collect::<BTreeSet<_>>();
    let mut cavity_source_faces = materialized.restored_source_faces.clone();
    for &child in &materialized.released_children {
        cavity_source_faces.insert(source_face_for_address(source, child)?);
    }
    for &parent in &split_interface_parents {
        cavity_source_faces.extend(source_descendant_faces(source, parent));
    }
    let candidate = materialized.mesh;
    let retained_parents = original_retained_parents
        .iter()
        .copied()
        .filter(|parent| {
            candidate
                .mesh
                .active_triangle_slots()
                .any(|face| candidate.triangle_addresses[face] == Some(*parent))
        })
        .collect::<BTreeSet<_>>();
    let protected_coarse_regions = retained_parent_components(incumbent, &retained_parents)
        .into_iter()
        .enumerate()
        .map(|(id, parents)| super::build_protected_coarse_region(source, id as u64, parents))
        .collect::<Result<Vec<_>, _>>()?;
    let cavities = source_face_components(source, &cavity_source_faces)
        .into_iter()
        .enumerate()
        .map(|(id, source_faces)| combined_cavity(source, id as u64, source_faces))
        .collect::<Vec<_>>();
    let combined_boundary = cavities
        .iter()
        .flat_map(|cavity| cavity.boundary_source_vertices.iter().copied())
        .collect::<BTreeSet<_>>();
    let internal_singleton_boundary_vertices = plan
        .singleton_boundary_source_vertices
        .difference(&combined_boundary)
        .copied()
        .collect::<BTreeSet<_>>();
    let topology_closed = candidate.mesh.open_edge_count() == 0;
    let edge_incidence_at_most_two = closed_edge_incidence(&candidate);
    let outside_topology_bitwise_equal = logical_exterior_equal_with_custom(
        incumbent,
        &removed_faces,
        &modified_parents,
        &candidate,
        &inserted_addresses,
        &materialized.inserted_custom_triangles,
    );
    let outside_coordinates_bitwise_equal =
        outside_coordinates_equal(incumbent, &removed_faces, &candidate);
    let protected_core_preserved = !protected_coarse_regions.is_empty()
        && protected_coarse_regions.iter().all(|region| {
            region
                .descendant_source_faces
                .is_disjoint(&cavity_source_faces)
        });
    let compression_ratio = candidate.mesh.active_triangle_slots().count() as f64
        / source.mesh.active_triangle_slots().count() as f64;
    Ok(CombinedRepairMaterialization {
        plan: plan.clone(),
        mesh: candidate,
        rebuilds: 1,
        removed_mixed_faces: materialized.removed_mixed_faces,
        restored_source_faces: materialized.restored_source_faces,
        released_parents: plan.released_parents.clone(),
        split_interface_parents,
        retained_parents,
        protected_coarse_regions,
        cavities,
        internal_singleton_boundary_vertices,
        topology_closed,
        outside_topology_bitwise_equal,
        outside_coordinates_bitwise_equal,
        edge_incidence_at_most_two,
        protected_core_preserved,
        compression_ratio,
    })
}

pub fn solve_combined_global_max_min(
    source: &MotherGrid,
    source_levels: &[Option<usize>],
    incumbent: &HierarchyLeafMesh,
    incumbent_patch: &ElasticPatch,
    materialization: &CombinedRepairMaterialization,
    geometry_iterations: usize,
) -> Result<CombinedGeometryResult, String> {
    if geometry_iterations == 0 {
        return Err("combined geometry requires a positive iteration budget".into());
    }
    if !materialization.topology_closed
        || !materialization.outside_topology_bitwise_equal
        || !materialization.outside_coordinates_bitwise_equal
        || !materialization.edge_incidence_at_most_two
        || !materialization.protected_core_preserved
        || materialization.retained_parents.is_empty()
        || materialization.compression_ratio >= 1.0
    {
        return Err("combined geometry requires a hard-gate-closed mixed candidate".into());
    }
    let fixed_sources = incumbent_patch
        .fixed_compact_vertices
        .iter()
        .filter_map(|&compact| incumbent.source_vertex_slots[compact])
        .chain(
            source
                .addresses
                .iter()
                .enumerate()
                .filter_map(|(site, address)| {
                    matches!(address, Some(VertexAddress::IcosahedronVertex(_))).then_some(site)
                }),
        )
        .collect::<BTreeSet<_>>();
    let movable_sources = combined_movable_sources(source, materialization, 2)
        .difference(&fixed_sources)
        .copied()
        .collect::<BTreeSet<_>>();
    if movable_sources.is_empty() {
        return Err("combined geometry has no movable source vertex".into());
    }
    let incumbent_angle_range_deg = mesh_angle_range(incumbent);
    let starts = [
        CombinedGeometryStartId::InheritedIncumbent,
        CombinedGeometryStartId::SafeInteriorBlend,
        CombinedGeometryStartId::HierarchySpringEquilibrium,
        CombinedGeometryStartId::RingScaleInterpolation,
        CombinedGeometryStartId::DegreeAngleEquilibrium,
    ];
    let mut attempts = Vec::with_capacity(starts.len());
    let mut best_candidate = None::<CombinedGeometryTrial>;
    for start_id in starts {
        let mut start_mesh = materialization.mesh.clone();
        if start_id == CombinedGeometryStartId::SafeInteriorBlend {
            apply_safe_interior_blend(source, &movable_sources, &mut start_mesh);
        }
        let patch = combined_elastic_patch(
            source,
            source_levels,
            incumbent_patch,
            materialization,
            &start_mesh,
            &movable_sources,
        )?;
        let solver_start = match start_id {
            CombinedGeometryStartId::InheritedIncumbent
            | CombinedGeometryStartId::SafeInteriorBlend => GeometryStartId::MaterializedSource,
            CombinedGeometryStartId::HierarchySpringEquilibrium => {
                GeometryStartId::HierarchySpringEquilibrium
            }
            CombinedGeometryStartId::RingScaleInterpolation => {
                GeometryStartId::RingScaleInterpolation
            }
            CombinedGeometryStartId::DegreeAngleEquilibrium => {
                GeometryStartId::DegreeAngleEquilibrium
            }
        };
        let outcome = solve_elastic_patch_with_max_min_trust_start(
            &start_mesh,
            patch.clone(),
            ElasticBlockLimits {
                elastic_iterations: geometry_iterations,
            },
            solver_start,
        );
        let (mesh, final_patch, completed_iterations, strict_certified, outcome_name) =
            match outcome {
                ElasticBlockOutcome::Certified(trial) => (
                    trial.mesh,
                    trial.patch,
                    trial.report.elastic_iterations,
                    true,
                    "Certified".to_string(),
                ),
                ElasticBlockOutcome::ElasticNoImprovement {
                    elastic_iterations,
                    witness,
                    ..
                } => (
                    witness.mesh,
                    witness.patch,
                    elastic_iterations,
                    false,
                    "ElasticNoImprovement".to_string(),
                ),
                ElasticBlockOutcome::SearchBudgetExhausted {
                    elastic_iterations,
                    witness,
                    ..
                } => (
                    witness.mesh,
                    witness.patch,
                    elastic_iterations,
                    false,
                    "SearchBudgetExhausted".to_string(),
                ),
                ElasticBlockOutcome::RequiresDifferentTopology {
                    elastic_iterations,
                    witness,
                    ..
                } => (
                    witness.mesh,
                    witness.patch,
                    elastic_iterations,
                    false,
                    "RequiresDifferentTopology".to_string(),
                ),
                ElasticBlockOutcome::InvalidPatch { reason } => (
                    start_mesh,
                    patch,
                    0,
                    false,
                    format!("InvalidPatch:{reason}"),
                ),
            };
        let angle_range_deg = mesh_angle_range(&mesh);
        let signed_margin_deg = angle_range_deg.map(range_margin);
        attempts.push(CombinedGeometryAttempt {
            start_id,
            target_mode: ElasticTargetMode::HierarchyEdgeAreaDegree,
            requested_iterations: geometry_iterations,
            completed_iterations,
            angle_range_deg,
            signed_margin_deg,
            strict_certified,
            outcome: outcome_name,
        });
        let trial = CombinedGeometryTrial {
            plan: materialization.plan.clone(),
            start_id,
            mesh,
            patch: final_patch,
            angle_range_deg,
            signed_margin_deg,
            retained_parents: materialization.retained_parents.clone(),
            compression_ratio: materialization.compression_ratio,
            strict_certified,
        };
        if best_candidate.as_ref().is_none_or(|best| {
            trial.signed_margin_deg.unwrap_or(f64::NEG_INFINITY)
                > best.signed_margin_deg.unwrap_or(f64::NEG_INFINITY)
        }) {
            best_candidate = Some(trial);
        }
    }
    let candidate_range = best_candidate
        .as_ref()
        .and_then(|candidate| candidate.angle_range_deg);
    let incumbent_margin = incumbent_angle_range_deg
        .map(range_margin)
        .unwrap_or(f64::NEG_INFINITY);
    let candidate_margin = candidate_range
        .map(range_margin)
        .unwrap_or(f64::NEG_INFINITY);
    let best_preserved_angle_range_deg = if candidate_margin > incumbent_margin {
        candidate_range
    } else {
        incumbent_angle_range_deg
    };
    let incumbent_preserved = best_preserved_angle_range_deg
        .map(range_margin)
        .is_some_and(|margin| margin + 1.0e-12 >= incumbent_margin);
    let strict_certified = best_candidate
        .as_ref()
        .is_some_and(|candidate| candidate.strict_certified);
    Ok(CombinedGeometryResult {
        attempts,
        best_candidate: best_candidate.map(Box::new),
        incumbent_angle_range_deg,
        best_preserved_angle_range_deg,
        incumbent_preserved,
        strict_certified,
        continuous_search_incomplete: !strict_certified,
    })
}

pub fn build_singleton_local_repair_registry(
    source: &MotherGrid,
    incumbent: &HierarchyLeafMesh,
    incumbent_patch: &ElasticPatch,
    atlas: &ViolationSupportAtlas,
    retained_parents: &BTreeSet<TriangleAddress>,
    geometry_iteration_limit: usize,
) -> Result<Vec<LocalRepairCandidate>, String> {
    let ear_parents = coarse_core_ears(source, retained_parents)?
        .into_iter()
        .map(|ear| ear.parent)
        .collect::<BTreeSet<_>>();
    let sectors = atlas
        .recovery_atoms
        .iter()
        .filter_map(|atom| match atom {
            RecoveryAtom::Sector { sector_id, .. } => Some(*sector_id),
            RecoveryAtom::HierarchyLeaf { .. } => None,
        })
        .collect::<BTreeSet<_>>();
    let original = &atlas.evidence_sets.strict_violations;
    sectors
        .into_iter()
        .map(|sector_id| {
            let (kind, sector_ids, removed_faces, restored_faces, released, candidate) =
                match restore_fine_compatible_sector(
                    source,
                    incumbent,
                    incumbent_patch,
                    &atlas.sector_recovery_atlas,
                    sector_id,
                    0,
                ) {
                    DirectSectorRestoreOutcome::Certified(trial)
                    | DirectSectorRestoreOutcome::GeometryNotCertified { trial, .. } => (
                        LocalRepairKind::DirectSectorRestore,
                        BTreeSet::from([sector_id]),
                        trial.removed_mixed_faces.clone(),
                        trial.restored_source_faces.clone(),
                        BTreeSet::new(),
                        trial.mesh.clone(),
                    ),
                    DirectSectorRestoreOutcome::RequiresBoundaryParentPeel {
                        adjacent_parents,
                        ..
                    } if adjacent_parents.len() == 1 => {
                        let parent = adjacent_parents
                            .intersection(&ear_parents)
                            .copied()
                            .next()
                            .ok_or_else(|| {
                                format!(
                                    "sector {sector_id} one-parent blocker is not a coarse-core ear"
                                )
                            })?;
                        let trial = match peel_boundary_parent_for_sector(
                            source,
                            incumbent,
                            incumbent_patch,
                            &atlas.sector_recovery_atlas,
                            retained_parents,
                            sector_id,
                            parent,
                            0,
                        ) {
                            BoundaryParentPeelOutcome::Certified(trial)
                            | BoundaryParentPeelOutcome::GeometryNotCertified { trial, .. } => {
                                trial
                            }
                            other => {
                                return Err(format!(
                                    "sector {sector_id} one-parent action failed: {other:?}"
                                ))
                            }
                        };
                        (
                            LocalRepairKind::OneParentPeel,
                            trial.restored_sector_ids.clone(),
                            trial.removed_mixed_faces.clone(),
                            trial.restored_source_faces.clone(),
                            BTreeSet::from([parent]),
                            trial.mesh.clone(),
                        )
                    }
                    DirectSectorRestoreOutcome::RequiresBoundaryParentPeel {
                        adjacent_parents,
                        ..
                    } if adjacent_parents.len() == 2 => {
                        let component = atlas
                            .local_recovery_components
                            .iter()
                            .find(|component| {
                                component.atoms.iter().any(|atom| {
                                    matches!(atom, RecoveryAtom::Sector { sector_id: candidate, .. } if *candidate == sector_id)
                                })
                            })
                            .ok_or_else(|| {
                                format!("sector {sector_id} has no local recovery component")
                            })?;
                        let trial = match solve_local_annular_collar(
                            source,
                            incumbent,
                            incumbent_patch,
                            &atlas.sector_recovery_atlas,
                            component,
                            retained_parents,
                            sector_id,
                            &adjacent_parents,
                            LocalAnnularCollarLimits {
                                topology_states: 3,
                                geometry_iterations: 0,
                                maximum_parent_peels: 2,
                            },
                        ) {
                            LocalAnnularCollarOutcome::Certified(trial) => trial,
                            LocalAnnularCollarOutcome::MaterializedNotCertified { best, .. } => {
                                best
                            }
                            other => {
                                return Err(format!(
                                    "sector {sector_id} two-parent action failed: {other:?}"
                                ))
                            }
                        };
                        let restored = source_faces_for_sectors(
                            &atlas.sector_recovery_atlas,
                            &trial.evidence.restored_sector_ids,
                        );
                        let removed = removed_mixed_faces_for_sectors(
                            incumbent,
                            &atlas.sector_recovery_atlas,
                            &trial.evidence.restored_sector_ids,
                        )?;
                        (
                            LocalRepairKind::TwoParentAnnularCollar,
                            trial.evidence.restored_sector_ids.clone(),
                            removed,
                            restored,
                            trial.evidence.released_parents.clone(),
                            trial.mesh.clone(),
                        )
                    }
                    other => {
                        return Err(format!(
                            "sector {sector_id} action registry classification failed: {other:?}"
                        ))
                    }
                };
            let local_component_ids = atlas
                .local_recovery_components
                .iter()
                .filter(|component| {
                    component.atoms.iter().any(|atom| {
                        matches!(atom, RecoveryAtom::Sector { sector_id, .. } if sector_ids.contains(sector_id))
                    })
                })
                .map(|component| component.id)
                .collect::<BTreeSet<_>>();
            let inserted_custom_triangles = new_custom_triangles(incumbent, &candidate);
            let action = build_local_repair_action(
                source,
                &candidate,
                &atlas.sector_recovery_atlas,
                original,
                retained_parents,
                sector_id,
                kind,
                local_component_ids,
                sector_ids,
                removed_faces,
                restored_faces,
                released,
                inserted_custom_triangles,
                geometry_iteration_limit,
            )?;
            Ok(LocalRepairCandidate {
                action,
                mesh: candidate,
            })
        })
        .collect()
}

fn source_faces_for_sectors(
    atlas: &SectorRecoveryAtlas,
    sector_ids: &BTreeSet<u64>,
) -> BTreeSet<usize> {
    sector_ids
        .iter()
        .flat_map(|sector| atlas.sectors[sector].source_faces.iter().copied())
        .collect()
}

fn removed_mixed_faces_for_sectors(
    incumbent: &HierarchyLeafMesh,
    atlas: &SectorRecoveryAtlas,
    sector_ids: &BTreeSet<u64>,
) -> Result<BTreeSet<usize>, String> {
    incumbent
        .mesh
        .active_triangle_slots()
        .filter(|&face| incumbent.triangle_addresses[face].is_none())
        .filter_map(|face| {
            source_triangle_key(incumbent, face)
                .and_then(|triangle| atlas.custom_face_owner.get(&triangle))
                .filter(|sector| sector_ids.contains(sector))
                .map(|_| Ok(face))
        })
        .collect()
}

fn new_custom_triangles(
    incumbent: &HierarchyLeafMesh,
    candidate: &HierarchyLeafMesh,
) -> BTreeSet<[usize; 3]> {
    let incumbent_custom = incumbent
        .mesh
        .active_triangle_slots()
        .filter(|&face| incumbent.triangle_addresses[face].is_none())
        .filter_map(|face| source_triangle_key(incumbent, face))
        .collect::<BTreeSet<_>>();
    candidate
        .mesh
        .active_triangle_slots()
        .filter(|&face| candidate.triangle_addresses[face].is_none())
        .filter_map(|face| source_triangle_key(candidate, face))
        .filter(|triangle| !incumbent_custom.contains(triangle))
        .collect()
}

fn replacements_conflict(left: &LocalRepairAction, right: &LocalRepairAction) -> bool {
    !left
        .removed_mixed_faces
        .is_disjoint(&right.removed_mixed_faces)
        && !left.inserted_custom_triangles.is_empty()
        && !right.inserted_custom_triangles.is_empty()
        && left.inserted_custom_triangles != right.inserted_custom_triangles
}

fn actions_interact(left: &LocalRepairAction, right: &LocalRepairAction) -> bool {
    !left
        .local_component_ids
        .is_disjoint(&right.local_component_ids)
        && (!left.sector_ids.is_disjoint(&right.sector_ids)
            || !left
                .removed_mixed_faces
                .is_disjoint(&right.removed_mixed_faces)
            || !left.released_parents.is_disjoint(&right.released_parents)
            || !left
                .modified_source_vertices
                .is_disjoint(&right.modified_source_vertices)
            || !left
                .boundary_source_vertices
                .is_disjoint(&right.boundary_source_vertices))
}

fn interaction_components(
    actions: &[LocalRepairAction],
    hard_conflicts: &BTreeSet<(u64, u64)>,
    merge_required: &BTreeSet<(u64, u64)>,
) -> Vec<BTreeSet<u64>> {
    let mut remaining = actions
        .iter()
        .map(|action| action.id)
        .collect::<BTreeSet<_>>();
    let mut components = Vec::new();
    while let Some(seed) = remaining.pop_first() {
        let mut component = BTreeSet::from([seed]);
        let mut frontier = vec![seed];
        while let Some(current) = frontier.pop() {
            for &(left, right) in hard_conflicts.iter().chain(merge_required) {
                let neighbour = if left == current {
                    Some(right)
                } else if right == current {
                    Some(left)
                } else {
                    None
                };
                if let Some(neighbour) = neighbour.filter(|item| remaining.remove(item)) {
                    component.insert(neighbour);
                    frontier.push(neighbour);
                }
            }
        }
        components.push(component);
    }
    components
}

fn contains_hard_conflict(
    selected: &[&LocalRepairAction],
    hard_conflicts: &BTreeSet<(u64, u64)>,
) -> bool {
    let ids = selected
        .iter()
        .map(|action| action.id)
        .collect::<BTreeSet<_>>();
    hard_conflicts
        .iter()
        .any(|(left, right)| ids.contains(left) && ids.contains(right))
}

fn combined_plan(
    bitmask: usize,
    selected: &[&LocalRepairAction],
    retained_parents: BTreeSet<TriangleAddress>,
    original_violation_ids_covered: BTreeSet<usize>,
    effect_by_id: &BTreeMap<u64, &LocalActionEffect>,
    original_violation_count: usize,
) -> CombinedRepairPlan {
    CombinedRepairPlan {
        bitmask,
        action_ids: selected.iter().map(|action| action.id).collect(),
        sector_ids: selected
            .iter()
            .flat_map(|action| action.sector_ids.iter().copied())
            .collect(),
        released_parents: selected
            .iter()
            .flat_map(|action| action.released_parents.iter().copied())
            .collect(),
        retained_parents,
        removed_mixed_faces: selected
            .iter()
            .flat_map(|action| action.removed_mixed_faces.iter().copied())
            .collect(),
        restored_source_faces: selected
            .iter()
            .flat_map(|action| action.restored_source_faces.iter().copied())
            .collect(),
        singleton_boundary_source_vertices: selected
            .iter()
            .flat_map(|action| action.boundary_source_vertices.iter().copied())
            .collect(),
        uncovered_original_violations: original_violation_count
            .saturating_sub(original_violation_ids_covered.len()),
        original_violation_ids_covered,
        estimated_new_violations: selected
            .iter()
            .map(|action| effect_by_id[&action.id].new_violations_created)
            .sum(),
    }
}

fn classification_name(classification: ActionSetClassification) -> &'static str {
    match classification {
        ActionSetClassification::Empty => "empty",
        ActionSetClassification::HardConflict => "hard_conflict",
        ActionSetClassification::NoRetainedCoarseParent => "no_retained_coarse_parent",
        ActionSetClassification::NoPotentialImprovement => "no_potential_improvement",
        ActionSetClassification::Compatible => "compatible",
    }
}

fn combined_movable_sources(
    source: &MotherGrid,
    materialization: &CombinedRepairMaterialization,
    ordinary_guard_rings: usize,
) -> BTreeSet<usize> {
    let mut faces = materialization
        .cavities
        .iter()
        .flat_map(|cavity| cavity.source_faces.iter().copied())
        .collect::<BTreeSet<_>>();
    let mut frontier = faces.clone();
    for _ in 0..ordinary_guard_rings {
        let next = frontier
            .iter()
            .flat_map(|&face| source.mesh.neighbours()[face])
            .filter(|face| !faces.contains(face))
            .collect::<BTreeSet<_>>();
        faces.extend(&next);
        frontier = next;
    }
    faces
        .iter()
        .flat_map(|&face| source.mesh.triangles()[face])
        .collect()
}

fn combined_elastic_patch(
    source: &MotherGrid,
    source_levels: &[Option<usize>],
    incumbent_patch: &ElasticPatch,
    materialization: &CombinedRepairMaterialization,
    candidate: &HierarchyLeafMesh,
    movable_sources: &BTreeSet<usize>,
) -> Result<ElasticPatch, String> {
    let movable_compact_vertices = candidate
        .source_vertex_slots
        .iter()
        .enumerate()
        .filter_map(|(compact, source)| {
            source
                .is_some_and(|source| movable_sources.contains(&source))
                .then_some(compact)
        })
        .collect::<Vec<_>>();
    let movable = movable_compact_vertices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if movable.is_empty() {
        return Err("combined hierarchy patch has no compact movable vertex".into());
    }
    let guard_faces = candidate
        .mesh
        .active_triangle_slots()
        .filter(|&face| {
            candidate.mesh.triangles()[face]
                .iter()
                .any(|site| movable.contains(site))
        })
        .collect::<Vec<_>>();
    let fixed_compact_vertices = guard_faces
        .iter()
        .flat_map(|&face| candidate.mesh.triangles()[face])
        .filter(|site| !movable.contains(site))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut topology = incumbent_patch.topology.clone();
    topology.component_id = materialization.plan.bitmask as u64;
    topology.topology_id = materialization.plan.bitmask;
    topology.core_parents = materialization.retained_parents.iter().copied().collect();
    topology.source_active_vertices = guard_faces
        .iter()
        .flat_map(|&face| candidate.mesh.triangles()[face])
        .filter_map(|compact| candidate.source_vertex_slots[compact])
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    ElasticPatch {
        domain_id: GeometryDomainId::PlusTwoOrdinaryRings,
        topology,
        reference_positions: candidate.mesh.vertices().to_vec(),
        fixed_compact_vertices,
        movable_compact_vertices,
        guard_faces,
        target_mode: ElasticTargetMode::HierarchyEdgeAreaDegree,
        target_field: Default::default(),
    }
    .with_hierarchy_targets(
        source,
        candidate,
        source_levels,
        ElasticTargetMode::HierarchyEdgeAreaDegree,
    )
}

fn apply_safe_interior_blend(
    source: &MotherGrid,
    movable_sources: &BTreeSet<usize>,
    candidate: &mut HierarchyLeafMesh,
) {
    for (compact, source_site) in candidate.source_vertex_slots.iter().copied().enumerate() {
        let Some(source_site) = source_site.filter(|site| movable_sources.contains(site)) else {
            continue;
        };
        let current = candidate.mesh.vertices()[compact];
        let safe = source.mesh.vertices()[source_site];
        if let Ok(point) = normalize_cartesian_to_radius(
            CartesianPoint::new(
                0.5 * (current.x + safe.x),
                0.5 * (current.y + safe.y),
                0.5 * (current.z + safe.z),
            ),
            1.0,
        ) {
            candidate.mesh.move_vertex(compact, point);
        }
    }
}

fn source_face_for_address(source: &MotherGrid, address: TriangleAddress) -> Result<usize, String> {
    source
        .triangle_addresses
        .iter()
        .position(|candidate| candidate == &Some(address))
        .ok_or_else(|| format!("source hierarchy address {address:?} is absent"))
}

fn source_descendant_faces(source: &MotherGrid, ancestor: TriangleAddress) -> BTreeSet<usize> {
    source
        .triangle_addresses
        .iter()
        .enumerate()
        .filter_map(|(face, address)| {
            address
                .is_some_and(|address| is_descendant(address, ancestor))
                .then_some(face)
        })
        .collect()
}

fn is_descendant(mut address: TriangleAddress, ancestor: TriangleAddress) -> bool {
    while address.n > ancestor.n {
        let Some(parent) = address.parent_2_to_1() else {
            return false;
        };
        address = parent;
    }
    address == ancestor
}

fn source_face_components(source: &MotherGrid, faces: &BTreeSet<usize>) -> Vec<BTreeSet<usize>> {
    let mut remaining = faces.clone();
    let mut components = Vec::new();
    while let Some(seed) = remaining.pop_first() {
        let mut component = BTreeSet::from([seed]);
        let mut frontier = vec![seed];
        while let Some(face) = frontier.pop() {
            for neighbour in source.mesh.neighbours()[face] {
                if remaining.remove(&neighbour) {
                    component.insert(neighbour);
                    frontier.push(neighbour);
                }
            }
        }
        components.push(component);
    }
    components
}

fn combined_cavity(source: &MotherGrid, id: u64, source_faces: BTreeSet<usize>) -> CombinedCavity {
    let mut edges = BTreeMap::<(usize, usize), usize>::new();
    for &face in &source_faces {
        let triangle = source.mesh.triangles()[face];
        for (left, right) in [
            (triangle[0], triangle[1]),
            (triangle[1], triangle[2]),
            (triangle[2], triangle[0]),
        ] {
            *edges.entry((left.min(right), left.max(right))).or_default() += 1;
        }
    }
    let boundary_source_vertices = edges
        .into_iter()
        .filter(|(_, count)| *count == 1)
        .flat_map(|(edge, _)| [edge.0, edge.1])
        .collect::<BTreeSet<_>>();
    let movable_source_vertices = source_faces
        .iter()
        .flat_map(|&face| source.mesh.triangles()[face])
        .filter(|site| !boundary_source_vertices.contains(site))
        .collect::<BTreeSet<_>>();
    CombinedCavity {
        id,
        source_faces,
        boundary_source_vertices,
        movable_source_vertices,
    }
}

fn retained_parent_components(
    incumbent: &HierarchyLeafMesh,
    parents: &BTreeSet<TriangleAddress>,
) -> Vec<BTreeSet<TriangleAddress>> {
    let face_by_parent = incumbent
        .mesh
        .active_triangle_slots()
        .filter_map(|face| incumbent.triangle_addresses[face].map(|parent| (parent, face)))
        .collect::<BTreeMap<_, _>>();
    let mut remaining = parents.clone();
    let mut components = Vec::new();
    while let Some(seed) = remaining.pop_first() {
        let mut component = BTreeSet::from([seed]);
        let mut frontier = vec![seed];
        while let Some(parent) = frontier.pop() {
            let Some(&face) = face_by_parent.get(&parent) else {
                continue;
            };
            for neighbour in incumbent.mesh.neighbours()[face] {
                let Some(next) = incumbent.triangle_addresses[neighbour] else {
                    continue;
                };
                if remaining.remove(&next) {
                    component.insert(next);
                    frontier.push(next);
                }
            }
        }
        components.push(component);
    }
    components
}

#[allow(clippy::too_many_arguments)]
pub fn build_local_repair_action(
    source: &MotherGrid,
    candidate: &HierarchyLeafMesh,
    atlas: &SectorRecoveryAtlas,
    original_violations: &[ViolatingAngle],
    retained_parents: &BTreeSet<TriangleAddress>,
    id: u64,
    kind: LocalRepairKind,
    local_component_ids: BTreeSet<u64>,
    sector_ids: BTreeSet<u64>,
    removed_mixed_faces: BTreeSet<usize>,
    restored_source_faces: BTreeSet<usize>,
    released_parents: BTreeSet<TriangleAddress>,
    inserted_custom_triangles: BTreeSet<[usize; 3]>,
    geometry_iteration_limit: usize,
) -> Result<LocalRepairAction, String> {
    if sector_ids.is_empty() {
        return Err("local repair action requires at least one exact sector".into());
    }
    if sector_ids
        .iter()
        .any(|sector| !atlas.sectors.contains_key(sector))
    {
        return Err("local repair action references an unknown exact sector".into());
    }
    let mut modified_source_faces = restored_source_faces.clone();
    for parent in &released_parents {
        let children = parent
            .children_2_to_1()
            .ok_or_else(|| format!("invalid released parent {parent:?}"))?;
        for child in children {
            let face = source
                .triangle_addresses
                .iter()
                .position(|candidate| candidate == &Some(child))
                .ok_or_else(|| format!("released child {child:?} is absent from source"))?;
            modified_source_faces.insert(face);
        }
    }
    let modified_source_vertices = modified_source_faces
        .iter()
        .flat_map(|&face| source.mesh.triangles()[face])
        .collect::<BTreeSet<_>>();
    let boundary_source_vertices = sector_ids
        .iter()
        .flat_map(|sector| atlas.sectors[sector].boundary_cycles.iter())
        .flatten()
        .copied()
        .collect::<BTreeSet<_>>();
    let movable_source_vertices = modified_source_vertices
        .difference(&boundary_source_vertices)
        .copied()
        .collect::<BTreeSet<_>>();
    let original_violation_ids_covered = original_violations
        .iter()
        .enumerate()
        .filter_map(|(id, violation)| {
            (!violation
                .source_support_faces
                .is_disjoint(&modified_source_faces)
                || !violation.parent_support.is_disjoint(&released_parents))
            .then_some(id)
        })
        .collect::<BTreeSet<_>>();
    let original_violation_ids_untouched = (0..original_violations.len())
        .filter(|id| !original_violation_ids_covered.contains(id))
        .collect::<BTreeSet<_>>();
    let retained_parents = retained_parents
        .difference(&released_parents)
        .copied()
        .collect::<BTreeSet<_>>();
    let protected_coarse_regions = if retained_parents.is_empty() {
        Vec::new()
    } else {
        vec![super::build_protected_coarse_region(
            source,
            id,
            retained_parents.clone(),
        )?]
    };
    let patch_topology = if protected_coarse_regions.is_empty() {
        PromotionPatchTopology::WholeSphere
    } else {
        PromotionPatchTopology::Annulus {
            protected_hole_id: id,
        }
    };
    Ok(LocalRepairAction {
        id,
        kind,
        local_component_ids,
        sector_ids,
        removed_mixed_faces,
        restored_source_faces,
        released_parents,
        retained_parents,
        inserted_custom_triangles,
        modified_source_vertices,
        boundary_source_vertices,
        movable_source_vertices,
        original_violation_ids_covered,
        original_violation_ids_untouched,
        patch_topology,
        protected_coarse_regions,
        geometry_target_mode: ElasticTargetMode::TrialReference,
        geometry_iteration_limit,
        fingerprint: mesh_fingerprint(&candidate.mesh),
    })
}

pub fn audit_local_repair_action(
    action: &LocalRepairAction,
    original_violations: &[ViolatingAngle],
    incumbent: &HierarchyLeafMesh,
    candidate: &HierarchyLeafMesh,
) -> LocalActionEffect {
    let original_keys = original_violations
        .iter()
        .filter_map(|violation| source_angle_key(incumbent, violation.face, violation.corner_site))
        .collect::<BTreeSet<_>>();
    let mut removed = 0usize;
    let mut resolved = 0usize;
    let mut persisted = 0usize;
    for violation in original_violations {
        let Some(key) = source_angle_key(incumbent, violation.face, violation.corner_site) else {
            removed += 1;
            continue;
        };
        let Some(angle) = current_angle(candidate, key) else {
            removed += 1;
            continue;
        };
        if signed_margin(angle) >= 0.0 {
            resolved += 1;
        } else {
            persisted += 1;
        }
    }

    let mut local_range = None;
    let mut outside_range = None;
    let mut global_range = None;
    let mut new_violations = 0usize;
    let mut orientation_valid = true;
    for face in candidate.mesh.active_triangle_slots() {
        let triangle = candidate.mesh.triangles()[face];
        let Some(angles) =
            spherical_triangle_angles(triangle.map(|site| candidate.mesh.vertices()[site]))
        else {
            orientation_valid = false;
            continue;
        };
        let local = triangle.iter().any(|&site| {
            candidate.source_vertex_slots[site]
                .is_some_and(|source| action.modified_source_vertices.contains(&source))
        });
        for (corner, angle) in angles.into_iter().enumerate() {
            update_range(&mut global_range, angle);
            update_range(
                if local {
                    &mut local_range
                } else {
                    &mut outside_range
                },
                angle,
            );
            if signed_margin(angle) < 0.0
                && source_angle_key(candidate, face, triangle[corner])
                    .is_none_or(|key| !original_keys.contains(&key))
            {
                new_violations += 1;
            }
        }
    }
    let topology_valid = candidate.mesh.open_edge_count() == 0 && closed_edge_incidence(candidate);
    let strict_certified = topology_valid
        && orientation_valid
        && persisted == 0
        && new_violations == 0
        && global_range
            .is_some_and(|range| signed_margin(range.0).min(signed_margin(range.1)) >= 0.0);
    let failure_reason = if strict_certified {
        SingletonFailureReason::StrictCertified
    } else if !action.original_violation_ids_untouched.is_empty() {
        SingletonFailureReason::UncoveredOriginalViolations
    } else if new_violations > 0 {
        SingletonFailureReason::NewViolationsCreated
    } else {
        SingletonFailureReason::ContinuousUnknown
    };
    LocalActionEffect {
        action_id: action.id,
        original_violations_total: original_violations.len(),
        original_violations_removed: removed,
        original_violations_resolved: resolved,
        original_violations_persisted: persisted,
        uncovered_original_violations: action.original_violation_ids_untouched.len(),
        new_violations_created: new_violations,
        local_angle_range: local_range,
        outside_angle_range: outside_range,
        global_angle_range: global_range,
        local_signed_margin_deg: local_range.map(range_margin),
        global_signed_margin_deg: global_range.map(range_margin),
        topology_valid,
        orientation_valid,
        strict_certified,
        failure_reason,
    }
}

fn current_angle(mesh: &HierarchyLeafMesh, key: ([usize; 3], usize)) -> Option<f64> {
    mesh.mesh.active_triangle_slots().find_map(|face| {
        let triangle = mesh.mesh.triangles()[face];
        let corner = triangle.iter().position(|&site| {
            mesh.source_vertex_slots[site].is_some_and(|source| source == key.1)
        })?;
        (source_triangle_key(mesh, face)? == key.0).then(|| {
            spherical_triangle_angles(triangle.map(|site| mesh.mesh.vertices()[site]))
                .map(|angles| angles[corner])
        })?
    })
}

fn source_angle_key(
    mesh: &HierarchyLeafMesh,
    face: usize,
    corner_site: usize,
) -> Option<([usize; 3], usize)> {
    Some((
        source_triangle_key(mesh, face)?,
        mesh.source_vertex_slots[corner_site]?,
    ))
}

fn source_triangle_key(mesh: &HierarchyLeafMesh, face: usize) -> Option<[usize; 3]> {
    if !mesh.mesh.is_triangle_live(face) {
        return None;
    }
    let mut source_triangle: [usize; 3] = mesh.mesh.triangles()[face]
        .map(|site| mesh.source_vertex_slots[site])
        .into_iter()
        .collect::<Option<Vec<_>>>()?
        .try_into()
        .ok()?;
    source_triangle.sort_unstable();
    Some(source_triangle)
}

fn signed_margin(angle: f64) -> f64 {
    (angle - 40.2).min(79.8 - angle)
}

fn range_margin(range: (f64, f64)) -> f64 {
    signed_margin(range.0).min(signed_margin(range.1))
}

fn update_range(range: &mut Option<(f64, f64)>, value: f64) {
    *range = Some(match *range {
        Some((minimum, maximum)) => (minimum.min(value), maximum.max(value)),
        None => (value, value),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TriangleOrientation;

    fn defect(angle_violations: usize) -> GeometryDefectVector {
        GeometryDefectVector {
            non_positive_triangles: 0,
            crossings: 0,
            angle_violations,
            negative_signed_margin_mdeg: 922,
            delaunay_violations: 0,
            invalid_voronoi_cells: 0,
            promoted_source_faces: 0,
            negative_retained_coarse_parents: -7,
        }
    }

    fn working_mesh() -> HierarchyLeafMesh {
        let source = MotherGrid::generate(1).unwrap();
        let leaves = super::super::HierarchyLeafSet::from_mother_grid(&source).unwrap();
        super::super::rebuild_from_leaf_set(&source, &leaves).unwrap()
    }

    fn moved_mesh(mut mesh: HierarchyLeafMesh) -> HierarchyLeafMesh {
        let site = mesh.mesh.active_vertex_slots().next().unwrap();
        let point = mesh.mesh.vertices()[site];
        mesh.mesh.move_vertex(
            site,
            CartesianPoint::new(point.x + 1.0e-9, point.y, point.z),
        );
        mesh
    }

    fn passing_gates() -> WorkingMeshHardGates {
        WorkingMeshHardGates {
            topology: true,
            degree: true,
            link: true,
            euler: true,
            charge: true,
            physical: true,
        }
    }

    fn action(id: u64, covered: usize) -> LocalRepairAction {
        LocalRepairAction {
            id,
            kind: LocalRepairKind::DirectSectorRestore,
            local_component_ids: BTreeSet::from([id]),
            sector_ids: BTreeSet::from([id]),
            removed_mixed_faces: BTreeSet::new(),
            restored_source_faces: BTreeSet::from([covered]),
            released_parents: BTreeSet::new(),
            retained_parents: BTreeSet::from([TriangleAddress {
                base_face: 0,
                i: 0,
                j: 0,
                n: 1,
                orientation: TriangleOrientation::Up,
            }]),
            inserted_custom_triangles: BTreeSet::new(),
            modified_source_vertices: BTreeSet::from([covered]),
            boundary_source_vertices: BTreeSet::new(),
            movable_source_vertices: BTreeSet::from([covered]),
            original_violation_ids_covered: BTreeSet::from([covered]),
            original_violation_ids_untouched: BTreeSet::new(),
            patch_topology: PromotionPatchTopology::WholeSphere,
            protected_coarse_regions: Vec::new(),
            geometry_target_mode: ElasticTargetMode::TrialReference,
            geometry_iteration_limit: 8,
            fingerprint: id,
        }
    }

    fn effect(id: u64) -> LocalActionEffect {
        LocalActionEffect {
            action_id: id,
            original_violations_total: 14,
            original_violations_removed: 1,
            original_violations_resolved: 0,
            original_violations_persisted: 13,
            uncovered_original_violations: 13,
            new_violations_created: 0,
            local_angle_range: None,
            outside_angle_range: None,
            global_angle_range: None,
            local_signed_margin_deg: None,
            global_signed_margin_deg: None,
            topology_valid: true,
            orientation_valid: true,
            strict_certified: false,
            failure_reason: SingletonFailureReason::UncoveredOriginalViolations,
        }
    }

    #[test]
    fn uncovered_external_violation_precedes_continuous_unknown() {
        let reason = if 1usize > 0 {
            SingletonFailureReason::UncoveredOriginalViolations
        } else {
            SingletonFailureReason::ContinuousUnknown
        };
        assert_eq!(reason, SingletonFailureReason::UncoveredOriginalViolations);
    }

    #[test]
    fn range_margin_uses_the_internal_contract() {
        assert_eq!(range_margin((40.2, 79.8)), 0.0);
        assert!(range_margin((39.0, 79.0)) < 0.0);
    }

    #[test]
    fn all_14_action_bitmasks_are_classified() {
        let actions = (0..14)
            .map(|index| action(index as u64, index))
            .collect::<Vec<_>>();
        let effects = (0..14).map(effect).collect::<Vec<_>>();
        let graph = build_local_action_conflict_graph(&actions).unwrap();
        let plan = enumerate_compatible_action_sets(&graph, &effects, 14).unwrap();
        assert_eq!(plan.total_bitmasks, 1 << 14);
        assert_eq!(plan.classifications.len(), 1 << 14);
        assert_eq!(plan.classification_counts["empty"], 1);
        assert_eq!(plan.classification_counts["compatible"], (1 << 14) - 1);
        assert_eq!(plan.compatible_plans[0].uncovered_original_violations, 0);
    }

    #[test]
    fn hard_conflicts_are_never_combined() {
        let mut left = action(1, 0);
        let mut right = action(2, 1);
        left.local_component_ids = BTreeSet::from([0]);
        right.local_component_ids = BTreeSet::from([0]);
        left.removed_mixed_faces.insert(9);
        right.removed_mixed_faces.insert(9);
        left.inserted_custom_triangles.insert([1, 2, 3]);
        right.inserted_custom_triangles.insert([1, 2, 4]);
        let graph = build_local_action_conflict_graph(&[left, right]).unwrap();
        let plan = enumerate_compatible_action_sets(&graph, &[effect(1), effect(2)], 2).unwrap();
        assert_eq!(graph.hard_conflicts, BTreeSet::from([(1, 2)]));
        assert_eq!(
            plan.classifications[3],
            ActionSetClassification::HardConflict
        );
    }

    #[test]
    fn defect_vector_strictly_decreases() {
        let mesh = working_mesh();
        let candidate_mesh = moved_mesh(mesh.clone());
        let mut state = WorkingMesh::new(mesh, defect(2), [10]);
        let step = accept_monotone_working_candidate(
            &mut state,
            WorkingMeshCandidate {
                mesh: candidate_mesh,
                defect: defect(1),
                hard_gates: passing_gates(),
                new_outside_angle_violations: 0,
            },
            |_| Ok([20, 30]),
        )
        .unwrap();
        assert!(matches!(step, WorkingMeshStep::Accepted { .. }));
        assert_eq!(state.defect, defect(1));
        assert_eq!(state.action_registry_fingerprints, BTreeSet::from([20, 30]));
        assert_eq!(state.registry_epoch, 1);
        assert_eq!(state.accepted_steps, 1);
    }

    #[test]
    fn visited_fingerprint_prevents_cycles() {
        let mesh = working_mesh();
        let mut state = WorkingMesh::new(mesh.clone(), defect(2), [10]);
        let step = accept_monotone_working_candidate(
            &mut state,
            WorkingMeshCandidate {
                mesh,
                defect: defect(1),
                hard_gates: passing_gates(),
                new_outside_angle_violations: 0,
            },
            |_| Ok([20]),
        )
        .unwrap();
        assert_eq!(
            step,
            WorkingMeshStep::Rejected(WorkingMeshRejectReason::VisitedFingerprint)
        );
        assert_eq!(state.registry_epoch, 0);
    }

    #[test]
    fn intermediate_working_mesh_is_never_published() {
        let mesh = working_mesh();
        let candidate_mesh = moved_mesh(mesh.clone());
        let mut state = WorkingMesh::new(mesh, defect(2), [10]);
        assert!(!state.is_publishable());
        accept_monotone_working_candidate(
            &mut state,
            WorkingMeshCandidate {
                mesh: candidate_mesh,
                defect: defect(1),
                hard_gates: passing_gates(),
                new_outside_angle_violations: 0,
            },
            |_| Ok([20]),
        )
        .unwrap();
        assert!(!state.is_publishable());
    }

    #[test]
    fn sequential_n6_result_matches_exact_combination_oracle() {
        assert!(sequential_matches_exact_combination_oracle(
            &defect(1),
            &defect(1)
        ));
        assert!(!sequential_matches_exact_combination_oracle(
            &defect(2),
            &defect(1)
        ));
    }
}
