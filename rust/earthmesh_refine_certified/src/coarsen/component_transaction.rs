//! Component-scoped, rollback-first coarsening transaction core.
//!
//! This module is deliberately small: topology chooses faces, elastic may move
//! only transition coordinates, then the normal geometry/final-cell/remap gates
//! decide whether the cloned state is committed.

use super::elastic_block::{solve_elastic_patch_scoped, GeometryFailureWitness, GeometryScope};
use super::transition_topology::{
    angle_between, hierarchy_parent_neighbours, parent_edge_angle, RetryFocus, RetryRequest,
};
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
    remap::{ConservativeRemap, RemapCertificate, VoronoiRemapSource},
    requirement::{
        certify_final_cell_requirements_with_remap, FinalCellRequirementError,
        FinalCellRequirementReport, SourceLevelField, TargetLevelField,
    },
};
use earthmesh_mesh::{CartesianPoint, MeshState};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    time::Instant,
};

fn log_component_phase(enabled: bool, component: u64, phase: &str, started: &mut Instant) {
    if enabled {
        eprintln!(
            "earthmesh_cli: cmrc_timing phase=component_{phase} component={component} elapsed_ms={}",
            started.elapsed().as_millis()
        );
        *started = Instant::now();
    }
}

/// The movable vertices of a failed elastic solve where it stopped, by
/// source slot.
fn final_movable_positions(
    witness: &super::elastic_block::GeometryFailureWitness,
) -> BTreeMap<usize, CartesianPoint> {
    witness
        .patch
        .movable_compact_vertices
        .iter()
        .filter_map(|&compact| {
            let source = witness
                .mesh
                .source_vertex_slots
                .get(compact)
                .copied()
                .flatten()?;
            Some((source, witness.mesh.mesh.vertices()[compact]))
        })
        .collect()
}

/// A failed solve is a near miss when no guard face's angles lie more than
/// this outside the certificate's window, in degrees: the 100 km 30 m trial
/// spent its last candidates at level 3 -> 2 between 0.02 and 0.08 out,
/// one place at a time (guide 11.137).
const NEAR_MISS_DEGREES: f64 = 0.1;
/// Places a stalled near miss is retried at in one candidate, its worst
/// included.
const NEAR_MISS_PLACES: usize = 16;
/// A near miss has stalled when its worst face is still more than this
/// share of the previous near miss's outside the window.
const NEAR_MISS_STALL: f64 = 0.9;

/// How far a near miss missed, and where: its worst excess in degrees and
/// the guard faces outside the window, worst first.
#[derive(Debug, Clone, PartialEq)]
struct NearMiss {
    worst: f64,
    faces: Vec<usize>,
}

/// The guard faces a failed solve left outside the certificate's window,
/// when it is a near miss (`NEAR_MISS_DEGREES`); `None` when a face lies
/// farther out or is not positive, or none lies out at all.
fn near_miss(witness: &GeometryFailureWitness, certificate: &Certificate) -> Option<NearMiss> {
    let mesh = &witness.mesh.mesh;
    let mut outside = Vec::new();
    for &face in &witness.patch.guard_faces {
        let points = mesh.triangles()[face].map(|site| mesh.vertices()[site]);
        if earthmesh_mesh::orientation_on_sphere(points[0], points[1], points[2])
            != Ok(earthmesh_mesh::Sign::Positive)
        {
            return None;
        }
        let angles = crate::certificate::spherical_triangle_angles(points)?;
        let smallest = angles.iter().copied().fold(f64::INFINITY, f64::min);
        let largest = angles.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let excess =
            (certificate.min_angle_degrees - smallest).max(largest - certificate.max_angle_degrees);
        if excess > NEAR_MISS_DEGREES {
            return None;
        }
        if excess > 0.0 {
            outside.push((excess, face));
        }
    }
    outside.sort_by(|left, right| right.0.total_cmp(&left.0).then(left.1.cmp(&right.1)));
    let worst = outside.first()?.0;
    Some(NearMiss {
        worst,
        faces: outside.into_iter().map(|(_, face)| face).collect(),
    })
}

/// Whether a near miss has stalled: the candidate before was a near miss
/// too, and the worst face came less than a tenth of the way in
/// (`NEAR_MISS_STALL`). One near miss alone is retried at its worst place,
/// as any failure is: at 40 km that passed, where promoting all sixteen
/// places changed the next level enough to cost it four failures.
fn near_miss_stalled(current: Option<&NearMiss>, previous: Option<f64>) -> bool {
    match (current, previous) {
        (Some(current), Some(previous)) => current.worst > NEAR_MISS_STALL * previous,
        _ => false,
    }
}

/// What an elastic solve did, for the timing log: how it ended, its
/// iterations and last phase, and the patch it moved.
fn log_elastic_outcome(component: u64, outcome: &ElasticBlockOutcome, patch: (usize, usize)) {
    let (kind, iterations, phase) = match outcome {
        ElasticBlockOutcome::Certified(trial) => {
            ("Certified", trial.report.elastic_iterations, None)
        }
        ElasticBlockOutcome::ElasticNoImprovement {
            elastic_iterations,
            final_phase,
            ..
        } => ("NoImprovement", *elastic_iterations, Some(*final_phase)),
        ElasticBlockOutcome::SearchBudgetExhausted {
            elastic_iterations,
            final_phase,
            ..
        } => ("BudgetExhausted", *elastic_iterations, Some(*final_phase)),
        ElasticBlockOutcome::RequiresDifferentTopology {
            elastic_iterations,
            final_phase,
            ..
        } => ("DifferentTopology", *elastic_iterations, Some(*final_phase)),
        ElasticBlockOutcome::InvalidPatch { .. } => ("InvalidPatch", 0, None),
    };
    eprintln!(
        "earthmesh_cli: cmrc_detail phase=elastic_outcome component={component} outcome={kind} \
         iterations={iterations} final_phase={phase:?} movable={} guard_faces={}",
        patch.0, patch.1
    );
}

fn log_failed_candidate_tail(
    enabled: bool,
    component: u64,
    stage: &ComponentTransactionStage,
    started: &mut Instant,
) {
    if enabled {
        eprintln!(
            "earthmesh_cli: cmrc_timing phase=component_failed_candidate_tail component={component} stage={stage:?} elapsed_ms={}",
            started.elapsed().as_millis()
        );
        *started = Instant::now();
    }
}

/// What a transaction on a built region checks differently from one on the
/// whole sphere (design B1e): the region's mesh is open along its edge, only
/// the cells of R are certified cell by cell, and the remap's tolerances scale
/// with the whole fine mother.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionScope {
    /// By fine source slot: whether the site is a corner of a face in R.
    certified_sources: Vec<bool>,
    /// Cells of the whole fine mother.
    whole_cells: usize,
    /// The built region's Euler characteristic, which re-triangulation keeps.
    euler: isize,
}

impl RegionScope {
    /// The scope of `source`, a built region, whose cells under the base
    /// faces `certified` (level `base_subdivision`) are certified one by one.
    pub fn new(
        source: &MotherGrid,
        certified: &BTreeSet<TriangleAddress>,
        base_subdivision: usize,
    ) -> Result<Self, String> {
        if source.region.is_none() {
            return Err("a region scope needs a built region".into());
        }
        let mut certified_sources = vec![false; source.mesh.vertices().len()];
        for face in source.mesh.active_triangle_slots() {
            let mut base = source.triangle_addresses[face]
                .ok_or_else(|| format!("region face {face} has no address"))?;
            while base.n > base_subdivision {
                base = base
                    .parent_2_to_1()
                    .ok_or_else(|| format!("region face {face} has no base face"))?;
            }
            if base.n == base_subdivision && certified.contains(&base) {
                for site in source.mesh.triangles()[face] {
                    certified_sources[site] = true;
                }
            }
        }
        let faces = source.mesh.triangle_count();
        let edges = (3 * faces + source.mesh.open_edge_count()) / 2;
        let n = source.subdivision;
        Ok(Self {
            certified_sources,
            whole_cells: 10usize
                .checked_mul(n)
                .and_then(|cells| cells.checked_mul(n))
                .and_then(|cells| cells.checked_add(2))
                .ok_or_else(|| "whole fine mother cell count overflows".to_string())?,
            euler: source.mesh.vertex_count() as isize - edges as isize + faces as isize,
        })
    }

    pub(super) fn whole_cells(&self) -> usize {
        self.whole_cells
    }

    /// The edge sites of `mesh` (those of the region's edge) and the
    /// region's Euler characteristic.
    pub(super) fn geometry_scope(
        &self,
        source: &MotherGrid,
        mesh: &HierarchyLeafMesh,
    ) -> GeometryScope {
        let outer = source
            .region
            .as_ref()
            .map(|region| region.outer_boundary())
            .expect("a region scope belongs to a built region");
        GeometryScope {
            edge_sites: mesh
                .source_vertex_slots
                .iter()
                .enumerate()
                .filter(|(_, slot)| slot.is_some_and(|slot| outer.contains(&slot)))
                .map(|(compact, _)| compact)
                .collect(),
            euler: self.euler,
        }
    }

    /// Whether the cell of compact site `site` of `mesh` is certified one
    /// by one.
    pub(super) fn certifies(&self, mesh: &HierarchyLeafMesh, site: usize) -> bool {
        mesh.source_vertex_slots
            .get(site)
            .copied()
            .flatten()
            .is_some_and(|slot| self.certified_sources.get(slot).copied().unwrap_or(false))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentTransactionLimits {
    pub topology_states: usize,
    /// Maximum CBER iterations for each hard-topology candidate.
    pub elastic_iterations: usize,
    pub interval_boxes: usize,
    pub halo_expansions: usize,
    /// `ElasticCmrcConfig::retry_at_failure`.
    pub retry_at_failure: bool,
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

    pub(super) fn custom_transition_triangles(
        &self,
    ) -> &BTreeMap<TriangleAddress, Vec<[usize; 3]>> {
        &self.custom_transition_triangles
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
    /// The certified remap itself, from the source mother to the committed
    /// mesh: the scheduler hands the last one on rather than have it computed
    /// again (`ElasticCmrcResult::final_remap`).
    pub remap_matrix: Option<ConservativeRemap>,
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
    let source_active_sites = source.mesh.active_vertex_slots().collect::<Vec<_>>();
    let level_source_slots = source
        .mesh
        .vertices()
        .iter()
        .enumerate()
        .map(|(site, _)| source.mesh.is_vertex_live(site).then_some(site))
        .collect::<Vec<_>>();
    let source_remap = VoronoiRemapSource::new(&source.mesh);
    solve_component_transaction_at_level(
        source,
        &source_remap,
        source_levels,
        state,
        source,
        &source_active_sites,
        &level_source_slots,
        component,
        coarse_level,
        max_adjacent_level_delta,
        limits,
        angle_contract,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn solve_component_transaction_at_level(
    source: &MotherGrid,
    source_remap: &VoronoiRemapSource<'_>,
    source_levels: &SourceLevelField,
    state: &mut ComponentTransactionState,
    level_grid: &MotherGrid,
    source_active_sites: &[usize],
    level_source_slots: &[Option<usize>],
    component: &HierarchyComponent,
    coarse_level: usize,
    max_adjacent_level_delta: usize,
    limits: ComponentTransactionLimits,
    angle_contract: AngleContractId,
    scope: Option<&RegionScope>,
) -> ComponentTransactionOutcome {
    let timing_enabled = std::env::var("EARTHMESH_CMRC_TIMING").as_deref() == Ok("1");
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

    if let Err(reason) =
        validate_preflight(source, source_levels, state, component, source_active_sites)
    {
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
    // A near miss's other places (guide 11.137), promoted with the worst.
    let mut other_core_promotions = Vec::<TriangleAddress>::new();
    let mut previous_near_miss = None::<f64>;
    // A retried candidate differs from the failed one only near the failure
    // (guide 11.126): its solve starts where the failed one stopped.
    let mut warm_start = None::<BTreeMap<usize, CartesianPoint>>;
    let mut topology_state_offset = 0usize;
    let mut halo_expansion_offset = 0usize;
    let mut search_component = component.clone();
    let promotion_depths = match core_promotion_depths(level_grid, component, scope.is_some()) {
        Ok(depths) => depths,
        Err(reason) => return fail!(InvalidInput, ComponentTransactionStage::Preflight, reason),
    };
    // A failure no promotion reaches is retried around its face (guide
    // 11.130): `focus` keeps the failed candidate and the place, and the
    // search offers transitions that change it there and keep it as it was
    // beyond the focus's radius. A focus whose candidates fail at its place
    // `FOCUS_CANDIDATES` times, or that has nothing left to offer even
    // widened, ends the transaction: the layout's own enumeration would
    // change the transition far from the failure, which leaves it where it
    // is (the 40 km 30 m trial failed at one face 40 times that way).
    let mut focus = None::<RetryFocus>;
    let mut last_failure = None::<(ComponentTransactionStage, String)>;
    let mut focus_candidates = 0usize;
    let mut focused_candidates = 0usize;
    let mut focused_states = 0usize;
    let parent_edge = match component
        .parents
        .first()
        .map(|&parent| parent_edge_angle(level_grid, parent))
        .transpose()
    {
        Ok(edge) => edge.unwrap_or(0.0),
        Err(reason) => return fail!(InvalidInput, ComponentTransactionStage::Preflight, reason),
    };

    loop {
        let with_cost = |parent: TriangleAddress| {
            let depth = promotion_depths.get(&parent).copied()?;
            (depth <= limits.halo_expansions)
                .then_some((parent, depth.saturating_sub(halo_expansion_offset)))
        };
        let preferred_promotion_with_cost = preferred_core_promotion.and_then(with_cost);
        let other_promotions_with_cost = other_core_promotions
            .iter()
            .copied()
            .filter_map(with_cost)
            .collect::<Vec<_>>();
        // Start topology timing exactly at the solver call; failed-candidate
        // bookkeeping from the previous iteration is deliberately uncharged.
        let mut phase_started = Instant::now();
        let outcome = (TransitionTopologyLimits {
            topology_states: limits
                .topology_states
                .saturating_sub(topology_state_offset + focused_states),
            maximum_halo_expansions: limits.halo_expansions.saturating_sub(halo_expansion_offset),
        })
        .solve_from_cursor_with_promotion(
            level_grid,
            &search_component,
            topology_cursor,
            RetryRequest {
                promotion: preferred_promotion_with_cost,
                also: &other_promotions_with_cost,
                reach: Some((&promotion_depths, limits.halo_expansions)),
                focus: focus.as_ref().filter(|_| limits.retry_at_failure),
            },
            limits.retry_at_failure,
        );
        log_component_phase(
            timing_enabled,
            component.id,
            "topology_search",
            &mut phase_started,
        );
        let (transition, level_custom_triangles) = match outcome {
            TransitionTopologyOutcome::Closed(trial) => {
                if let (Some(states), Some(current)) =
                    (trial.report.focus_topology_states, focus.as_mut())
                {
                    focused_states += states.saturating_sub(current.cursor);
                    current.cursor = states;
                }
                counters.topology_states = (topology_state_offset + focused_states)
                    .saturating_add(trial.report.topology_states);
                counters.halo_expansions =
                    halo_expansion_offset.saturating_add(trial.report.halo_expansions);
                // A focus on this candidate keeps its triangles as the
                // search numbers them.
                let level_custom_triangles = trial.candidate.custom_transition_triangles.clone();
                match remap_transition_trial(trial, level_source_slots) {
                    Ok(trial) => (trial, level_custom_triangles),
                    Err(reason) => {
                        return fail!(InvalidInput, ComponentTransactionStage::Topology, reason)
                    }
                }
            }
            TransitionTopologyOutcome::FocusExhausted {
                states_examined, ..
            } => {
                let Some(current) = focus.as_mut() else {
                    return fail!(
                        InvalidInput,
                        ComponentTransactionStage::Topology,
                        "a focused search ended without a focus".to_string()
                    );
                };
                focused_states += states_examined.saturating_sub(current.cursor);
                counters.topology_states =
                    (topology_state_offset + focused_states).saturating_add(topology_cursor);
                if current.previous_radius_edges == 0.0 {
                    current.previous_radius_edges = current.radius_edges;
                    current.change_radius_edges = WIDENED_FOCUS_CHANGE_RADIUS_EDGES;
                    current.radius_edges = WIDENED_FOCUS_RADIUS_EDGES;
                    current.cursor = 0;
                    log_retry_focus(
                        timing_enabled,
                        component.id,
                        "widen",
                        current,
                        focus_candidates,
                    );
                    continue;
                }
                log_retry_focus(
                    timing_enabled,
                    component.id,
                    "exhausted",
                    current,
                    focus_candidates,
                );
                let reason = focus_end_reason(current, focus_candidates, &last_failure);
                let stage = last_failure
                    .as_ref()
                    .map_or(ComponentTransactionStage::Topology, |(stage, _)| {
                        stage.clone()
                    });
                // As when the layout runs out: an elastic budget that ran
                // out on the way leaves the search unfinished, not proven.
                if last_elastic_budget_failure.is_some() {
                    return fail!(SearchBudgetExhausted, stage, reason);
                }
                return fail!(NotCertifiable, stage, reason);
            }
            TransitionTopologyOutcome::RequiresWiderHalo {
                states_examined,
                halo_expansions,
            } => {
                counters.topology_states =
                    (topology_state_offset + focused_states).saturating_add(states_examined);
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
                counters.topology_states =
                    (topology_state_offset + focused_states).saturating_add(states_examined);
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
                counters.topology_states =
                    (topology_state_offset + focused_states).saturating_add(states_examined);
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
                counters.topology_states =
                    (topology_state_offset + focused_states).saturating_add(states_examined);
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
            sync_search_component_partition(
                &mut search_component,
                transition.candidate.core_parents.clone(),
                transition.boundary.halo_parents.clone(),
            );
        }
        let exact_core_candidate = transition.candidate.custom_transition_triangles.is_empty();
        let mut candidate_state = state.clone();
        log_component_phase(
            timing_enabled,
            component.id,
            "state_clone",
            &mut phase_started,
        );
        candidate_state.prepare_parent_level(parent_subdivision);
        match certify_candidate(
            source,
            source_remap,
            source_levels,
            &mut candidate_state,
            component,
            coarse_level,
            max_adjacent_level_delta,
            &transition,
            limits.elastic_iterations,
            warm_start.as_ref().filter(|_| limits.retry_at_failure),
            limits
                .interval_boxes
                .saturating_sub(counters.interval_boxes),
            before_fingerprint,
            pre_vertices,
            pre_faces,
            &pre_sources,
            angle_contract,
            scope,
            &mut phase_started,
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
            Err(mut failure) => {
                if let Some(positions) = failure.warm_positions.take() {
                    warm_start = Some(positions);
                }
                // Why a candidate failed -- what tells a hard search from a
                // broken one (guide 11.110).
                if timing_enabled {
                    eprintln!(
                        "earthmesh_cli: cmrc_detail phase=candidate_failure component={} stage={:?} reason={}{}",
                        component.id,
                        failure.stage,
                        failure.reason,
                        failed_face_place(&candidate_state.mesh, failure.failed_guard_face)
                    );
                }
                // certify_candidate uses this same timer for completed phases;
                // only the unlogged tail since its last phase boundary is charged here.
                log_failed_candidate_tail(
                    timing_enabled,
                    component.id,
                    &failure.stage,
                    &mut phase_started,
                );
                counters.elastic_iterations += failure.elastic_iterations;
                counters.interval_boxes += failure.interval_boxes;
                preferred_core_promotion = failure.failed_guard_face.and_then(|face| {
                    preferred_core_promotion_for_face(&candidate_state.mesh, &transition, face)
                });
                // A near miss is retried at every place it missed, not one
                // a candidate (guide 11.137).
                other_core_promotions.clear();
                let stalled = near_miss_stalled(failure.near_miss.as_deref(), previous_near_miss);
                previous_near_miss = failure.near_miss.as_ref().map(|miss| miss.worst);
                let places = failure
                    .near_miss
                    .as_ref()
                    .filter(|_| stalled)
                    .map_or(&[][..], |miss| &miss.faces[..]);
                for &face in places {
                    if other_core_promotions.len() + 1 >= NEAR_MISS_PLACES {
                        break;
                    }
                    if let Some(parent) =
                        preferred_core_promotion_for_face(&candidate_state.mesh, &transition, face)
                    {
                        if Some(parent) != preferred_core_promotion
                            && !other_core_promotions.contains(&parent)
                        {
                            other_core_promotions.push(parent);
                        }
                    }
                }
                if timing_enabled {
                    eprintln!(
                        "earthmesh_cli: cmrc_detail phase=retry_place component={} {} \
                         preferred={:?} depth={:?} halo_offset={} halo_budget={}",
                        component.id,
                        failed_face_neighbourhood(
                            &candidate_state.mesh,
                            &transition,
                            failure.failed_guard_face
                        ),
                        preferred_core_promotion,
                        preferred_core_promotion
                            .and_then(|parent| promotion_depths.get(&parent).copied()),
                        halo_expansion_offset,
                        limits.halo_expansions,
                    );
                }
                // A focused candidate leaves the layout's cursor where it
                // was: the focus keeps its own.
                let focused = transition.report.focus_topology_states.is_some();
                let failed_place = failure
                    .failed_guard_face
                    .and_then(|face| face_centroid(&candidate_state.mesh.mesh, face));
                last_failure = Some((failure.stage.clone(), failure.reason.clone()));
                match failure.disposition {
                    CandidateFailureDisposition::InvalidInput => {
                        return fail!(InvalidInput, failure.stage, failure.reason)
                    }
                    CandidateFailureDisposition::BudgetExhausted => {
                        if failure.stage != ComponentTransactionStage::Elastic
                            || exact_core_candidate
                            || (!focused && candidate_topology_states <= candidate_previous_cursor)
                        {
                            return fail!(SearchBudgetExhausted, failure.stage, failure.reason);
                        }
                        last_elastic_budget_failure = Some(failure.reason);
                        if !focused {
                            topology_cursor = candidate_topology_states;
                        }
                    }
                    CandidateFailureDisposition::Retry => {
                        last_retry = Some((failure.stage, failure.reason));
                        if exact_core_candidate
                            || (!focused && candidate_topology_states <= candidate_previous_cursor)
                        {
                            let (stage, reason) = last_retry.expect("just recorded retry");
                            return fail!(NotCertifiable, stage, reason);
                        }
                        if !focused {
                            topology_cursor = candidate_topology_states;
                        }
                    }
                }
                if !limits.retry_at_failure {
                    continue;
                }
                let Some(point) = failed_place.filter(|_| !transition.report.retired_vertex) else {
                    focus = None;
                    continue;
                };
                focused_candidates += usize::from(focused);
                let same_place = focused
                    && focus.as_ref().is_some_and(|current| {
                        angle_between(current.point, point)
                            <= current.change_radius_edges * parent_edge
                    });
                if same_place {
                    focus_candidates += 1;
                } else {
                    focus = Some(RetryFocus {
                        point,
                        change_radius_edges: FOCUS_CHANGE_RADIUS_EDGES,
                        radius_edges: FOCUS_RADIUS_EDGES,
                        previous_radius_edges: 0.0,
                        chosen: level_custom_triangles,
                        cursor: 0,
                    });
                    focus_candidates = 0;
                }
                let current = focus.as_ref().expect("a focus was just kept or made");
                log_retry_focus(
                    timing_enabled,
                    component.id,
                    if same_place { "again" } else { "new" },
                    current,
                    focus_candidates,
                );
                if focus_candidates >= FOCUS_CANDIDATES || focused_candidates >= FOCUSED_CANDIDATES
                {
                    let reason = focus_end_reason(current, focus_candidates, &last_failure);
                    let stage = last_failure
                        .as_ref()
                        .map_or(ComponentTransactionStage::Elastic, |(stage, _)| {
                            stage.clone()
                        });
                    if last_elastic_budget_failure.is_some() {
                        return fail!(SearchBudgetExhausted, stage, reason);
                    }
                    return fail!(NotCertifiable, stage, reason);
                }
            }
        }
    }
}

/// A focus's radii in parent edge lengths (guide 11.130): a candidate must
/// change the transition within the first, may change it within the
/// second; then both once widened.
const FOCUS_CHANGE_RADIUS_EDGES: f64 = 2.0;
const FOCUS_RADIUS_EDGES: f64 = 8.0;
const WIDENED_FOCUS_CHANGE_RADIUS_EDGES: f64 = 4.0;
const WIDENED_FOCUS_RADIUS_EDGES: f64 = 16.0;
/// Candidates of one focus that may fail at its place again, and focused
/// candidates one transaction may try in all, before it ends.
const FOCUS_CANDIDATES: usize = 12;
const FOCUSED_CANDIDATES: usize = 64;

fn face_centroid(mesh: &MeshState, face: usize) -> Option<CartesianPoint> {
    if !mesh.is_triangle_live(face) {
        return None;
    }
    let corners = mesh.triangles()[face].map(|site| mesh.vertices()[site]);
    Some(CartesianPoint::new(
        corners.iter().map(|point| point.x).sum(),
        corners.iter().map(|point| point.y).sum(),
        corners.iter().map(|point| point.z).sum(),
    ))
}

fn longitude_latitude(point: CartesianPoint) -> (f64, f64) {
    let norm = (point.x * point.x + point.y * point.y + point.z * point.z).sqrt();
    (
        point.y.atan2(point.x).to_degrees(),
        (point.z / norm).asin().to_degrees(),
    )
}

fn log_retry_focus(
    enabled: bool,
    component: u64,
    action: &str,
    focus: &RetryFocus,
    candidates: usize,
) {
    if enabled {
        let (longitude, latitude) = longitude_latitude(focus.point);
        eprintln!(
            "earthmesh_cli: cmrc_detail phase=retry_focus component={component} action={action} \
             at lon {longitude:.5} lat {latitude:.5} radius_edges={} candidates={candidates} \
             cursor={}",
            focus.radius_edges, focus.cursor
        );
    }
}

fn focus_end_reason(
    focus: &RetryFocus,
    candidates: usize,
    last_failure: &Option<(ComponentTransactionStage, String)>,
) -> String {
    let (longitude, latitude) = longitude_latitude(focus.point);
    format!(
        "the failure near lon {longitude:.5} lat {latitude:.5} repeats where no promotion \
         reaches: {} more transitions tried within {} parent edges of it did not certify; \
         last failure: {}",
        candidates,
        focus.radius_edges,
        last_failure
            .as_ref()
            .map_or("none", |(_, reason)| reason.as_str())
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CandidateFailureDisposition {
    Retry,
    InvalidInput,
    BudgetExhausted,
}

#[derive(Debug, PartialEq)]
struct CandidateAttemptFailure {
    disposition: CandidateFailureDisposition,
    stage: ComponentTransactionStage,
    reason: String,
    elastic_iterations: usize,
    interval_boxes: usize,
    failed_guard_face: Option<usize>,
    /// Where a failed elastic solve left its movable vertices, by source
    /// slot: the next candidate's solve starts there.
    warm_positions: Option<BTreeMap<usize, CartesianPoint>>,
    /// How far a near miss missed, and where (`near_miss`); boxed, as the
    /// failure is returned by value.
    near_miss: Option<Box<NearMiss>>,
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
            warm_positions: None,
            near_miss: None,
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
            warm_positions: None,
            near_miss: None,
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
            warm_positions: None,
            near_miss: None,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn certify_candidate(
    source: &MotherGrid,
    source_remap: &VoronoiRemapSource<'_>,
    source_levels: &SourceLevelField,
    state: &mut ComponentTransactionState,
    component: &HierarchyComponent,
    coarse_level: usize,
    max_adjacent_level_delta: usize,
    transition: &super::TransitionTopologyTrial,
    remaining_elastic_iterations: usize,
    warm_start: Option<&BTreeMap<usize, CartesianPoint>>,
    remaining_interval_boxes: usize,
    before_fingerprint: u64,
    pre_vertices: usize,
    pre_faces: usize,
    pre_sources: &[bool],
    angle_contract: AngleContractId,
    scope: Option<&RegionScope>,
    phase_started: &mut Instant,
) -> Result<ComponentCommitReport, CandidateAttemptFailure> {
    let timing_enabled = std::env::var("EARTHMESH_CMRC_TIMING").as_deref() == Ok("1");
    let candidate = transition.candidate.clone();
    install_delta(source, state, &candidate).map_err(|reason| {
        CandidateAttemptFailure::invalid(ComponentTransactionStage::InstallDelta, reason)
    })?;
    apply_source_positions(&mut state.mesh, &state.source_positions);
    // On a built region, the checks of the whole sphere become the region's
    // (`verify_geometry_within`); its numbering is fixed from here on.
    let geometry_scope = scope.map(|scope| scope.geometry_scope(source, &state.mesh));
    let verify = |certificate: Certificate, mesh: &MeshState| match &geometry_scope {
        None => certificate.verify_geometry(mesh),
        Some(region) => certificate.verify_geometry_within(mesh, &region.edge_sites, region.euler),
    };
    log_component_phase(timing_enabled, component.id, "install_delta", phase_started);

    let mut elastic_iterations = 0usize;
    let mut elastic_report = None;
    let guard_faces = affected_faces(source, &state.mesh, &candidate);
    let interval_boxes = guard_faces.len().saturating_mul(3);
    log_component_phase(timing_enabled, component.id, "prepare_guard", phase_started);
    if interval_boxes > remaining_interval_boxes {
        let mut failure = CandidateAttemptFailure::budget(
            ComponentTransactionStage::LocalGeometry,
            "local geometry interval-box budget exhausted".to_string(),
        );
        failure.interval_boxes = interval_boxes;
        return Err(failure);
    }
    let geometry_passes = Certificate::internal_for(angle_contract)
        .geometry_region_passes(&state.mesh.mesh, &guard_faces);
    log_component_phase(
        timing_enabled,
        component.id,
        "geometry_screen",
        phase_started,
    );
    if !geometry_passes {
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
        log_component_phase(timing_enabled, component.id, "prepare_patch", phase_started);
        let patch_size = (
            patch.movable_compact_vertices.len(),
            patch.guard_faces.len(),
        );
        if let Some(positions) = warm_start {
            // The patch's targets come from the unmoved mesh; only the start
            // moves, and only for the vertices the failed solve moved too.
            for &compact in &patch.movable_compact_vertices {
                if let Some(&point) = state.mesh.source_vertex_slots[compact]
                    .and_then(|source| positions.get(&source))
                {
                    state.mesh.mesh.move_vertex(compact, point);
                }
            }
        }
        let outcome = solve_elastic_patch_scoped(
            &state.mesh,
            patch,
            ElasticBlockLimits {
                elastic_iterations: remaining_elastic_iterations,
            },
            angle_contract,
            geometry_scope.as_ref(),
        );
        // Include rejected candidates in solve timing, not in a generic failure tail.
        log_component_phase(timing_enabled, component.id, "elastic_solve", phase_started);
        if timing_enabled {
            log_elastic_outcome(component.id, &outcome, patch_size);
        }
        let elastic = match outcome {
            ElasticBlockOutcome::Certified(trial) => trial,
            ElasticBlockOutcome::ElasticNoImprovement {
                elastic_iterations: iterations,
                initial_energy,
                final_energy,
                reason,
                failed_guard_face,
                global_angle_degrees,
                witness,
                ..
            }
            | ElasticBlockOutcome::RequiresDifferentTopology {
                elastic_iterations: iterations,
                initial_energy,
                final_energy,
                reason,
                failed_guard_face,
                global_angle_degrees,
                witness,
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
                failure.warm_positions = Some(final_movable_positions(&witness));
                failure.near_miss =
                    near_miss(&witness, &Certificate::internal_for(angle_contract)).map(Box::new);
                return Err(failure);
            }
            ElasticBlockOutcome::SearchBudgetExhausted {
                elastic_iterations: iterations,
                initial_energy,
                final_energy,
                reason,
                failed_guard_face,
                global_angle_degrees,
                witness,
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
                failure.warm_positions = Some(final_movable_positions(&witness));
                failure.near_miss =
                    near_miss(&witness, &Certificate::internal_for(angle_contract)).map(Box::new);
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
        log_component_phase(timing_enabled, component.id, "elastic_apply", phase_started);
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
    log_component_phase(
        timing_enabled,
        component.id,
        "local_geometry",
        phase_started,
    );

    let global_geometry = verify(Certificate::internal_for(angle_contract), &state.mesh.mesh)
        .map_err(|error| {
            let mut failure = CandidateAttemptFailure::retry(
                ComponentTransactionStage::GlobalGeometry,
                format!("{error:?}"),
            );
            failure.interval_boxes = interval_boxes;
            failure
        })?;
    log_component_phase(
        timing_enabled,
        component.id,
        "internal_geometry",
        phase_started,
    );
    let final_geometry = verify(
        Certificate::final_delivery_for(angle_contract),
        &state.mesh.mesh,
    )
    .map_err(|error| {
        let mut failure = CandidateAttemptFailure::retry(
            ComponentTransactionStage::FinalGeometry,
            format!("{error:?}"),
        );
        failure.interval_boxes = interval_boxes;
        failure
    })?;
    log_component_phase(
        timing_enabled,
        component.id,
        "final_geometry",
        phase_started,
    );

    let target_levels = state.target_levels().map_err(|reason| {
        let mut failure =
            CandidateAttemptFailure::invalid(ComponentTransactionStage::FinalCells, reason);
        failure.interval_boxes = interval_boxes;
        failure
    })?;
    let remap = match scope {
        None => source_remap.remap_to(&state.mesh.mesh),
        Some(scope) => source_remap.remap_region_to(
            &state.mesh.mesh,
            |site| scope.certifies(&state.mesh, site),
            scope.whole_cells,
        ),
    }
    .map_err(|reason| {
        let mut failure = CandidateAttemptFailure::retry(ComponentTransactionStage::Remap, reason);
        failure.interval_boxes = interval_boxes;
        failure
    })?;
    let remap_certificate =
        remap.certify_spherical_overlap(source_levels.levels().len(), target_levels.levels().len());
    log_component_phase(timing_enabled, component.id, "remap", phase_started);
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
    log_component_phase(timing_enabled, component.id, "final_cells", phase_started);
    let final_evidence =
        FinalCertificationEvidence::from_final_cells(&final_cells, remap_certificate.clone())
            .map_err(|reason| {
                let mut failure =
                    CandidateAttemptFailure::retry(ComponentTransactionStage::Remap, reason);
                failure.interval_boxes = interval_boxes;
                failure
            })?;

    let geometry = GeometryCertifiedMotherGrid::new(state.mesh.mesh.clone(), final_geometry);
    let final_mesh = match remap.covered_targets() {
        None => crate::finalize_geometry_certified_mother(geometry, final_evidence),
        Some(covered) => {
            crate::api::finalize_region_geometry(geometry, final_evidence, covered.len())
        }
    }
    .map_err(|error| {
        let mut failure = CandidateAttemptFailure::retry(
            ComponentTransactionStage::FinalGeometry,
            format!("{error:?}"),
        );
        failure.interval_boxes = interval_boxes;
        failure
    })?;
    let final_certificate = final_mesh.certificate().clone();
    log_component_phase(timing_enabled, component.id, "finalize", phase_started);

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
        remap_matrix: Some(remap),
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

fn sync_search_component_partition(
    component: &mut HierarchyComponent,
    core_parents: Vec<TriangleAddress>,
    transition_parents: Vec<TriangleAddress>,
) {
    component.parents = core_parents
        .iter()
        .chain(&transition_parents)
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    component.core_parents = core_parents;
    component.transition_parents = transition_parents;
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

/// Each parent's distance from the transition ring. On a built region,
/// parents joined to the component only through the settled region keep no
/// depth: they are farther than any promotion reaches.
fn core_promotion_depths(
    source: &MotherGrid,
    component: &HierarchyComponent,
    region: bool,
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
    if !region && !component.transition_parents.is_empty() && depths.len() != parents.len() {
        return Err("component promotion depths are disconnected".into());
    }
    Ok(depths)
}

/// Where a failed guard face lies, for the failure log: its centroid and
/// how many of its edges are open (on the region's outer boundary).
fn failed_face_place(mesh: &HierarchyLeafMesh, face: Option<usize>) -> String {
    let Some(face) = face.filter(|&face| mesh.mesh.is_triangle_live(face)) else {
        return String::new();
    };
    let corners = mesh.mesh.triangles()[face].map(|site| mesh.mesh.vertices()[site]);
    let [x, y, z] = [
        corners.iter().map(|point| point.x).sum::<f64>(),
        corners.iter().map(|point| point.y).sum::<f64>(),
        corners.iter().map(|point| point.z).sum::<f64>(),
    ];
    let open = mesh.mesh.neighbours()[face]
        .iter()
        .filter(|&&neighbour| neighbour == 0 || !mesh.mesh.is_triangle_live(neighbour))
        .count();
    format!(
        "; failed face at lon {:.5} lat {:.5}, {open} open edges",
        y.atan2(x).to_degrees(),
        (z / (x * x + y * y + z * z).sqrt()).asin().to_degrees()
    )
}

/// What a failed face is -- a core parent (`c`), a custom transition
/// triangle (`x`) or a fine leaf (`f`) -- and how many face steps away the
/// nearest of each kind lies, for the retry log.
fn failed_face_neighbourhood(
    mesh: &HierarchyLeafMesh,
    transition: &super::TransitionTopologyTrial,
    face: Option<usize>,
) -> String {
    let Some(face) = face.filter(|&face| mesh.mesh.is_triangle_live(face)) else {
        return String::new();
    };
    let core = transition
        .candidate
        .core_parents
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let kind = |face: usize| match mesh.triangle_addresses[face] {
        None => 'x',
        Some(address) if core.contains(&address) => 'c',
        Some(_) => 'f',
    };
    let mut first = BTreeMap::<char, usize>::new();
    let mut custom_within_six = 0usize;
    let mut seen = BTreeSet::from([face]);
    let mut frontier = vec![face];
    for distance in 0..=12 {
        for &face in &frontier {
            let kind = kind(face);
            first.entry(kind).or_insert(distance);
            if kind == 'x' && distance <= 6 {
                custom_within_six += 1;
            }
        }
        let mut next = Vec::new();
        for &face in &frontier {
            for neighbour in mesh.mesh.neighbours()[face] {
                if neighbour != 0 && mesh.mesh.is_triangle_live(neighbour) && seen.insert(neighbour)
                {
                    next.push(neighbour);
                }
            }
        }
        frontier = next;
    }
    format!(
        "face_kind={} first_custom={:?} first_core={:?} first_fine={:?} \
         custom_within_six={custom_within_six}",
        kind(face),
        first.get(&'x'),
        first.get(&'c'),
        first.get(&'f')
    )
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
    source_active_sites: &[usize],
) -> Result<(), String> {
    if state.source_fingerprint != mesh_fingerprint(&source.mesh)
        || state.source_subdivision != source.subdivision
    {
        return Err("transaction state belongs to a different source mesh".into());
    }
    if source_levels.active_sites() != source_active_sites {
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
    if address.n == source.subdivision || super::core_condensation::source_has_face(source, address)
    {
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

    /// A disk of movable vertices, eight edges across from its centre, on a
    /// near-equilateral grid: every face well inside the window.
    fn near_miss_disk() -> (HierarchyLeafMesh, ElasticPatch, usize) {
        let grid = MotherGrid::generate(32).unwrap();
        let face = grid.mesh.active_triangle_slots().next().unwrap();
        let centre = grid.mesh.triangles()[face][0];
        let edge = angle_between(
            grid.mesh.vertices()[centre],
            grid.mesh.vertices()[grid.mesh.triangles()[face][1]],
        );
        let movable = grid
            .mesh
            .active_vertex_slots()
            .filter(|&site| {
                angle_between(grid.mesh.vertices()[site], grid.mesh.vertices()[centre])
                    <= 8.0 * edge
            })
            .collect::<BTreeSet<_>>();
        let guard_faces = grid
            .mesh
            .active_triangle_slots()
            .filter(|&face| {
                grid.mesh.triangles()[face]
                    .iter()
                    .any(|site| movable.contains(site))
            })
            .collect::<Vec<_>>();
        let fixed = guard_faces
            .iter()
            .flat_map(|&face| grid.mesh.triangles()[face])
            .filter(|site| !movable.contains(site))
            .collect::<BTreeSet<_>>();
        let mesh = HierarchyLeafMesh {
            mesh: grid.mesh.clone(),
            triangle_addresses: grid.triangle_addresses.clone(),
            source_vertex_slots: (0..grid.mesh.vertices().len())
                .map(|site| grid.mesh.is_vertex_live(site).then_some(site))
                .collect(),
        };
        let patch = ElasticPatch {
            domain_id: GeometryDomainId::CurrentAnnulus,
            topology: TransitionTopologyCandidate {
                component_id: 1,
                topology_id: 1,
                core_parents: Vec::new(),
                custom_transition_triangles: BTreeMap::new(),
                source_triangles: Vec::new(),
                source_active_vertices: movable.iter().chain(&fixed).copied().collect(),
                source_degree_forecast: BTreeMap::new(),
            },
            reference_positions: grid.mesh.vertices().to_vec(),
            fixed_compact_vertices: fixed.into_iter().collect(),
            movable_compact_vertices: movable.into_iter().collect(),
            guard_faces,
            target_mode: ElasticTargetMode::TrialReference,
            target_field: ElasticTargetField::default(),
        };
        (mesh, patch, centre)
    }

    /// A failed solve is a near miss when every guard face lies within
    /// `NEAR_MISS_DEGREES` of the window: its faces outside it are listed,
    /// worst first. A face farther out, or none out at all, lists nothing.
    #[test]
    fn a_near_miss_lists_the_faces_it_left_outside_the_window() {
        let (mesh, patch, centre) = near_miss_disk();
        let certificate = Certificate::internal_for(AngleContractId::DomainQuality38To82V1);
        let neighbour = mesh
            .mesh
            .active_triangle_slots()
            .find_map(|face| {
                let triangle = mesh.mesh.triangles()[face];
                triangle
                    .contains(&centre)
                    .then(|| *triangle.iter().find(|&&site| site != centre).unwrap())
            })
            .unwrap();
        let moved = |fraction: f64| {
            let mut moved = mesh.clone();
            let [a, b] = [
                mesh.mesh.vertices()[centre],
                mesh.mesh.vertices()[neighbour],
            ];
            let point = CartesianPoint::new(
                a.x + fraction * (b.x - a.x),
                a.y + fraction * (b.y - a.y),
                a.z + fraction * (b.z - a.z),
            );
            let norm = (point.x * point.x + point.y * point.y + point.z * point.z).sqrt();
            moved.mesh.move_vertex(
                centre,
                CartesianPoint::new(point.x / norm, point.y / norm, point.z / norm),
            );
            GeometryFailureWitness {
                mesh: moved,
                patch: patch.clone(),
            }
        };
        let excess = |witness: &GeometryFailureWitness| {
            witness
                .patch
                .guard_faces
                .iter()
                .map(|&face| {
                    let angles = crate::certificate::spherical_triangle_angles(
                        witness.mesh.mesh.triangles()[face]
                            .map(|site| witness.mesh.mesh.vertices()[site]),
                    )
                    .unwrap();
                    let smallest = angles.iter().copied().fold(f64::INFINITY, f64::min);
                    let largest = angles.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                    (certificate.min_angle_degrees - smallest)
                        .max(largest - certificate.max_angle_degrees)
                })
                .fold(f64::NEG_INFINITY, f64::max)
        };
        // The fraction at which the worst face lies `target` outside.
        let fraction_for = |target: f64| {
            let (mut low, mut high) = (0.0, 0.45);
            for _ in 0..60 {
                let middle = 0.5 * (low + high);
                if excess(&moved(middle)) < target {
                    low = middle;
                } else {
                    high = middle;
                }
            }
            0.5 * (low + high)
        };
        assert_eq!(near_miss(&moved(0.0), &certificate), None);
        let near = moved(fraction_for(0.5 * NEAR_MISS_DEGREES));
        let miss = near_miss(&near, &certificate).expect("a near miss");
        assert!(miss.worst > 0.0 && miss.worst <= NEAR_MISS_DEGREES);
        let faces = miss.faces;
        assert!(!faces.is_empty());
        assert!(faces
            .iter()
            .all(|face| near.mesh.mesh.triangles()[*face].contains(&centre)));
        let far = moved(fraction_for(2.0 * NEAR_MISS_DEGREES));
        assert!(excess(&far) > NEAR_MISS_DEGREES);
        assert_eq!(near_miss(&far, &certificate), None);
    }

    /// A near miss has stalled only when the candidate before was one too
    /// and the worst face came less than a tenth of the way in.
    #[test]
    fn a_near_miss_stalls_only_after_another_that_came_no_nearer() {
        let miss = |worst| NearMiss {
            worst,
            faces: vec![1],
        };
        assert!(!near_miss_stalled(None, Some(0.05)));
        assert!(!near_miss_stalled(Some(&miss(0.05)), None));
        assert!(!near_miss_stalled(Some(&miss(0.04)), Some(0.08)));
        assert!(near_miss_stalled(Some(&miss(0.0192)), Some(0.0193)));
        assert!(near_miss_stalled(Some(&miss(0.06)), Some(0.05)));
    }

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
    fn cached_component_partition_tracks_layout_changed_retry() {
        let source = MotherGrid::generate(4).unwrap();
        let (core, transition) = source
            .triangle_addresses
            .iter()
            .flatten()
            .filter_map(|address| address.parent_2_to_1())
            .find_map(|parent| {
                hierarchy_parent_neighbours(&source, parent)
                    .ok()
                    .and_then(|neighbours| {
                        neighbours
                            .into_iter()
                            .next()
                            .map(|neighbour| (parent, neighbour))
                    })
            })
            .unwrap();
        let limits = TransitionTopologyLimits {
            topology_states: 8,
            maximum_halo_expansions: 0,
        };
        let mut search_component = HierarchyComponent {
            id: 7,
            parents: vec![core, transition],
            boundary_edges: Vec::new(),
            core_parents: vec![core],
            transition_parents: vec![transition],
        };
        assert_eq!(
            search_component
                .parents
                .iter()
                .copied()
                .collect::<BTreeSet<_>>(),
            search_component
                .core_parents
                .iter()
                .chain(&search_component.transition_parents)
                .copied()
                .collect::<BTreeSet<_>>()
        );

        let mut stale_after_layout_shrink = search_component.clone();
        stale_after_layout_shrink.transition_parents.clear();
        assert!(matches!(
            limits.solve_from_cursor_with_promotion(
                &source,
                &stale_after_layout_shrink,
                0,
                RetryRequest::default(),
                true
            ),
            TransitionTopologyOutcome::InvalidBoundary { reason, .. }
                if reason == "component parents must equal core union transition parents"
        ));

        sync_search_component_partition(&mut search_component, vec![core], Vec::new());
        assert_eq!(search_component.parents, vec![core]);
        assert!(matches!(
            limits.solve_from_cursor_with_promotion(
                &source,
                &search_component,
                0,
                RetryRequest::default(),
                true
            ),
            TransitionTopologyOutcome::RequiresWiderHalo {
                states_examined: 0,
                halo_expansions: 0,
            }
        ));
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
