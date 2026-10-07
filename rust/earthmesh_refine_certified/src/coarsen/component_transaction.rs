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
        AngleContractId, Certificate, CertificateError, FinalCertificateReport,
        GeometryCertificateReport, GeometryRegionCertificateReport,
    },
    fingerprint::mesh_fingerprint,
    mother_grid::{MotherGrid, TriangleAddress},
    outcome::{FinalCertificationEvidence, GeometryCertifiedMotherGrid},
    remap::{RemapCertificate, VoronoiRemapSource},
    requirement::{
        certify_final_cell_requirements_with_remap, row_required_level, FinalCellRequirementError,
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
/// A failure that missed at more places than this -- core parents nearest
/// its faces outside the window -- is retried at every one of them at once
/// (guide 11.139). The Heihe trial's first solve at level 3 -> 2 left 9,067
/// faces outside, the worst 64 alone at 60 places, and each retry at its
/// worst place brought about 60 faces in. One place can count dozens: the
/// 10 km 30 m trial's 60 faces outside lay at 22, and one promotion
/// brought them all in.
const WIDESPREAD_PLACES: usize = 64;

/// Where a failed solve left guard faces outside the certificate's window:
/// its worst excess in degrees and those faces, worst first.
#[derive(Debug, Clone, PartialEq)]
struct Missed {
    worst: f64,
    faces: Vec<usize>,
}

impl Missed {
    /// Whether no face lies more than `NEAR_MISS_DEGREES` out.
    fn is_near(&self) -> bool {
        self.worst <= NEAR_MISS_DEGREES
    }
}

/// The guard faces a failed solve left outside the certificate's window;
/// `None` when a face is not positive, or none lies out at all.
fn missed_faces(witness: &GeometryFailureWitness, certificate: &Certificate) -> Option<Missed> {
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
        if excess > 0.0 {
            outside.push((excess, face));
        }
    }
    outside.sort_by(|left, right| right.0.total_cmp(&left.0).then(left.1.cmp(&right.1)));
    let worst = outside.first()?.0;
    Some(Missed {
        worst,
        faces: outside.into_iter().map(|(_, face)| face).collect(),
    })
}

/// Whether a near miss has stalled: the candidate before was a near miss
/// too, and the worst face came less than a tenth of the way in
/// (`NEAR_MISS_STALL`). One near miss alone is retried at its worst place,
/// as any failure is: at 40 km that passed, where promoting all sixteen
/// places changed the next level enough to cost it four failures.
fn near_miss_stalled(current: Option<&Missed>, previous: Option<f64>) -> bool {
    match (current.filter(|current| current.is_near()), previous) {
        (Some(current), Some(previous)) => current.worst > NEAR_MISS_STALL * previous,
        _ => false,
    }
}

/// The core parents a failure's next candidate promotes besides the one
/// nearest its worst face (`worst`): those nearest its other faces outside
/// the window, in the faces' order, each once. A failure that missed at
/// more than `WIDESPREAD_PLACES` places gives every one (guide 11.139); a
/// stalled near miss, up to `NEAR_MISS_PLACES` in all (guide 11.137); any
/// other failure, none.
fn retry_places(
    missed: &Missed,
    stalled: bool,
    worst: Option<TriangleAddress>,
    mut nearest: impl FnMut(usize) -> Option<TriangleAddress>,
) -> Vec<TriangleAddress> {
    let mut seen = worst.into_iter().collect::<BTreeSet<_>>();
    let mut places = Vec::new();
    for &face in &missed.faces {
        if let Some(parent) = nearest(face) {
            if seen.insert(parent) {
                places.push(parent);
            }
        }
    }
    if seen.len() > WIDESPREAD_PLACES {
        return places;
    }
    if !stalled {
        return Vec::new();
    }
    places.truncate(NEAR_MISS_PLACES - 1);
    places
}

/// What an elastic solve did, for the timing log: how it ended, its
/// iterations and last phase, and the patch it moved.
fn log_elastic_outcome<G>(component: u64, outcome: &ElasticBlockOutcome<G>, patch: (usize, usize)) {
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
    /// Whether `mesh` lags the leaves, transition triangles and positions:
    /// a commit built in a window changes those alone, and the whole mesh
    /// is rebuilt once, before it is next read (`refresh`, guide 11.146).
    stale: bool,
    /// The whole mesh's active vertices and faces, kept as commits change
    /// them.
    vertex_count: usize,
    face_count: usize,
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
            stale: false,
            vertex_count: mesh.mesh.vertex_count(),
            face_count: mesh.mesh.triangle_count(),
            mesh,
            source_fingerprint: mesh_fingerprint(&source.mesh),
            source_subdivision: source.subdivision,
            claimed_parent_subdivision: None,
            claimed_parents: BTreeSet::new(),
        })
    }

    /// The whole mesh. Only a state `refresh`ed since its last windowed
    /// commit has one.
    pub fn mesh(&self) -> &HierarchyLeafMesh {
        assert!(
            !self.stale,
            "a transaction state's mesh is read before it is rebuilt (refresh)"
        );
        &self.mesh
    }

    /// Whether the whole mesh must be rebuilt (`refresh`) before it is read.
    pub fn is_stale(&self) -> bool {
        self.stale
    }

    /// Rebuilds the whole mesh from the leaves, transition triangles and
    /// positions when commits built in windows left it behind (guide
    /// 11.146): the mesh a commit built whole would have left.
    pub fn refresh(&mut self, source: &MotherGrid) -> Result<(), String> {
        if !self.stale {
            return Ok(());
        }
        let (custom_parents, custom_triangles) = custom_parts(&self.custom_transition_triangles);
        let mut mesh = rebuild_from_leaf_set_with_custom_triangles(
            source,
            &self.leaf_set,
            &custom_parents,
            &custom_triangles,
        )?;
        apply_source_positions(&mut mesh, &self.source_positions);
        if (mesh.mesh.vertex_count(), mesh.mesh.triangle_count())
            != (self.vertex_count, self.face_count)
        {
            return Err(format!(
                "the rebuilt mesh has {} vertices and {} faces, the commits left {} and {}",
                mesh.mesh.vertex_count(),
                mesh.mesh.triangle_count(),
                self.vertex_count,
                self.face_count
            ));
        }
        self.mesh = mesh;
        self.stale = false;
        Ok(())
    }

    pub fn target_levels(&self) -> Result<TargetLevelField, String> {
        target_levels_for(
            &self.mesh().mesh,
            &self.mesh().source_vertex_slots,
            &self.source_delivered_levels,
        )
    }

    pub fn source_delivered_levels(&self) -> &[Option<usize>] {
        &self.source_delivered_levels
    }

    pub fn fingerprint(&self) -> u64 {
        mesh_fingerprint(&self.mesh().mesh)
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
            .mesh()
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
    /// The whole mesh's fingerprints, when the transaction certifies it
    /// whole (`CommitCertification::Whole`).
    pub before_fingerprint: Option<u64>,
    pub restored_fingerprint: Option<u64>,
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
    /// The whole mesh's fingerprints, when the transaction certifies it
    /// whole (`CommitCertification::Whole`).
    pub before_fingerprint: Option<u64>,
    pub after_fingerprint: Option<u64>,
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
    /// What certified the committed geometry (`CommitCertification`).
    pub geometry: CommitGeometry,
    /// What certified the committed cells: their requirements and remap.
    pub cells: CommitCells,
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

/// How a component transaction certifies what it commits (guide 11.143).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitCertification {
    /// The whole mesh, against the internal and the final-delivery
    /// certificates, and the final certificate built from them: what a
    /// transaction on its own owes.
    Whole,
    /// The faces the candidate changed or whose corners it moved, against
    /// both certificates. The rest of the mesh is as the last commit left
    /// it, and the caller certifies the whole once its components are done
    /// -- the scheduler at the end of each level. Every check the whole
    /// mesh's certificate makes is local to a face, an edge's two faces or a
    /// site's fan, so those faces decide it alone; the counts it adds up are
    /// left to the level's certificate. A candidate whose faces reach a
    /// region's open edge, where the whole mesh's checks spare the sites,
    /// is certified whole.
    Changed,
}

/// The geometry evidence a commit carries (`CommitCertification`).
#[derive(Debug, Clone, PartialEq)]
pub enum CommitGeometry {
    Whole {
        internal: GeometryCertificateReport,
        final_certificate: FinalCertificateReport,
    },
    Changed {
        internal: GeometryRegionCertificateReport,
        final_delivery: GeometryRegionCertificateReport,
    },
    /// A window's, when the faces that changed reach a region's edge: the
    /// window certified as a region is, open along its edges (guide
    /// 11.146).
    Window {
        internal: GeometryCertificateReport,
        final_delivery: GeometryCertificateReport,
    },
}

/// The cell evidence a commit carries (`CommitCertification`).
#[derive(Debug, Clone, PartialEq)]
pub enum CommitCells {
    /// Every cell's requirements, and the remap of every cell.
    Whole {
        final_cells: FinalCellRequirementReport,
        remap: RemapCertificate,
    },
    /// The requirements and the remap rows of the cells the candidate
    /// changed, and the balance of every edge at their sites (guide
    /// 11.145). The remap the delivery needs is the scheduler's, made at
    /// the end of the level.
    Changed {
        remap: RemapCertificate,
        cells: ChangedCellsReport,
    },
}

/// What a commit's changed cells were checked for (`CommitCells::Changed`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangedCellsReport {
    /// Cells whose remap rows were made and whose requirements were met.
    pub cells: usize,
    /// Edges at the changed sites whose balance was met.
    pub edges: usize,
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
        CommitCertification::Whole,
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
    certification: CommitCertification,
) -> ComponentTransactionOutcome {
    let timing_enabled = std::env::var("EARTHMESH_CMRC_TIMING").as_deref() == Ok("1");
    // A transaction certified whole reads, and fingerprints, the whole mesh;
    // one certified where it changed builds its candidates in the window
    // its search used, and leaves the whole mesh to be rebuilt once for the
    // level (guide 11.146).
    let whole = certification == CommitCertification::Whole;
    let refreshed = if whole { state.refresh(source) } else { Ok(()) };
    let before_fingerprint = (whole && !state.is_stale()).then(|| state.fingerprint());
    let pre_vertices = state.vertex_count;
    let pre_faces = state.face_count;
    let mut counters = Counters::default();

    // A component left as it was says why in the timing log: a search that
    // ends without a candidate leaves no other trace (guide 11.139).
    macro_rules! fail {
        ($variant:ident, $stage:expr, $reason:expr) => {{
            let stage = $stage;
            let reason: String = $reason;
            if timing_enabled {
                eprintln!(
                    "earthmesh_cli: cmrc_detail phase=component_rollback component={} \
                     outcome={} stage={stage:?} topology_states={} halo={} reason={reason}",
                    component.id,
                    stringify!($variant),
                    counters.topology_states,
                    counters.halo_expansions
                );
            }
            ComponentTransactionOutcome::$variant(ComponentRollbackReport {
                component_id: component.id,
                stage,
                reason,
                before_fingerprint,
                restored_fingerprint: before_fingerprint.map(|_| state.fingerprint()),
                pre_vertices,
                pre_faces,
                topology_states: counters.topology_states,
                elastic_iterations: counters.elastic_iterations,
                interval_boxes: counters.interval_boxes,
                halo_expansions: counters.halo_expansions,
            })
        }};
    }

    if let Err(reason) = refreshed {
        return fail!(InvalidInput, ComponentTransactionStage::Preflight, reason);
    }
    let Some(parent_subdivision) = component.parents.first().map(|parent| parent.n) else {
        return fail!(
            InvalidInput,
            ComponentTransactionStage::Preflight,
            "component has no parents".to_string()
        );
    };

    if let Err(reason) = validate_preflight(
        source,
        source_remap,
        source_levels,
        state,
        component,
        source_active_sites,
    ) {
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
                // A component's candidates are built and checked in a window
                // round it, not over the whole level (guide 11.144).
                windowed: true,
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
        // The candidate is built in the window its search used, or whole.
        let window_parents = transition.window_parents.as_ref().filter(|_| !whole);
        // Tests narrow it to the component's own parents.
        #[cfg(test)]
        let narrowed = tests::NARROW_WINDOWS.with(std::cell::Cell::get).then(|| {
            transition
                .candidate
                .core_parents
                .iter()
                .chain(&transition.boundary.halo_parents)
                .copied()
                .collect::<BTreeSet<_>>()
        });
        #[cfg(test)]
        let window_parents = window_parents.and(narrowed.as_ref()).or(window_parents);
        let work = match window_parents {
            Some(parents) => Work::window(source, state, parents),
            None => state.refresh(source).map(|()| Work::whole(state)),
        };
        let mut work = match work {
            Ok(work) => work,
            Err(reason) => {
                return fail!(
                    InvalidInput,
                    ComponentTransactionStage::InstallDelta,
                    reason
                )
            }
        };
        log_component_phase(
            timing_enabled,
            component.id,
            "state_clone",
            &mut phase_started,
        );
        macro_rules! certify {
            () => {
                certify_candidate(
                    source,
                    source_remap,
                    source_levels,
                    state,
                    &mut work,
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
                    angle_contract,
                    scope,
                    certification,
                    &mut phase_started,
                )
            };
        }
        let mut certified = certify!();
        // A candidate whose checks would read past its window's edge is
        // built again on the whole state, as it would have been without one.
        if let Some(failure) = certified.as_ref().err().filter(|failure| {
            work.within.is_some() && failure.disposition == CandidateFailureDisposition::Whole
        }) {
            if timing_enabled {
                eprintln!(
                    "earthmesh_cli: cmrc_detail phase=window_cut component={} stage={:?} reason={}",
                    component.id, failure.stage, failure.reason
                );
            }
            #[cfg(test)]
            tests::WINDOW_CUTS.with(|cuts| cuts.set(cuts.get() + 1));
            work = match state.refresh(source).map(|()| Work::whole(state)) {
                Ok(work) => work,
                Err(reason) => {
                    return fail!(
                        InvalidInput,
                        ComponentTransactionStage::InstallDelta,
                        reason
                    )
                }
            };
            certified = certify!();
        }
        match certified {
            Ok((mut report, counts)) => {
                counters.elastic_iterations += report.elastic_iterations;
                counters.interval_boxes += report.interval_boxes;
                report.topology_states = counters.topology_states;
                report.elastic_iterations = counters.elastic_iterations;
                report.interval_boxes = counters.interval_boxes;
                report.halo_expansions = counters.halo_expansions;
                if let Err(reason) =
                    work.commit(state, &transition.candidate, parent_subdivision, counts)
                {
                    return fail!(
                        InvalidInput,
                        ComponentTransactionStage::InstallDelta,
                        reason
                    );
                }
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
                        failed_face_place(&work.mesh, failure.failed_guard_face)
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
                    preferred_core_promotion_for_face(&work.mesh, &transition, face)
                });
                // A near miss is retried at every place it missed, not one
                // a candidate (guide 11.137).
                // ... and one that missed at many places, at all of them
                // (guide 11.139).
                let missed = failure.missed.as_deref();
                let stalled = near_miss_stalled(missed, previous_near_miss);
                previous_near_miss = missed
                    .filter(|missed| missed.is_near())
                    .map(|miss| miss.worst);
                other_core_promotions = missed.map_or_else(Vec::new, |missed| {
                    let core = transition
                        .candidate
                        .core_parents
                        .iter()
                        .copied()
                        .collect::<BTreeSet<_>>();
                    retry_places(missed, stalled, preferred_core_promotion, |face| {
                        nearest_core_parent(&work.mesh, &core, face)
                    })
                });
                // Retried at that many places, the next candidate differs
                // from this one at all of them: its solve starts from the
                // unmoved mesh. Started where this one stopped, the 100 km
                // trial's 215 places tangled and the untangling ran for an
                // hour without finishing (guide 11.139).
                if other_core_promotions.len() >= WIDESPREAD_PLACES {
                    warm_start = None;
                }
                if timing_enabled {
                    eprintln!(
                        "earthmesh_cli: cmrc_detail phase=retry_place component={} {} \
                         preferred={:?} depth={:?} halo_offset={} halo_budget={}",
                        component.id,
                        failed_face_neighbourhood(
                            &work.mesh,
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
                    .and_then(|face| face_centroid(&work.mesh.mesh, face));
                last_failure = Some((failure.stage.clone(), failure.reason.clone()));
                match failure.disposition {
                    CandidateFailureDisposition::InvalidInput
                    | CandidateFailureDisposition::Whole => {
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
    /// The candidate, or the faces its checks read round it, reached the
    /// edge of its window: it is built again on the whole state (guide
    /// 11.146).
    Whole,
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
    /// Where a failed solve left faces outside the window, and how far
    /// (`missed_faces`); boxed, as the failure is returned by value.
    missed: Option<Box<Missed>>,
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
            missed: None,
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
            missed: None,
        }
    }

    fn whole(stage: ComponentTransactionStage, reason: impl Into<String>) -> Self {
        Self {
            disposition: CandidateFailureDisposition::Whole,
            stage,
            reason: reason.into(),
            elastic_iterations: 0,
            interval_boxes: 0,
            failed_guard_face: None,
            warm_positions: None,
            missed: None,
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
            missed: None,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn certify_candidate(
    source: &MotherGrid,
    source_remap: &VoronoiRemapSource<'_>,
    source_levels: &SourceLevelField,
    state: &ComponentTransactionState,
    work: &mut Work,
    component: &HierarchyComponent,
    coarse_level: usize,
    max_adjacent_level_delta: usize,
    transition: &super::TransitionTopologyTrial,
    remaining_elastic_iterations: usize,
    warm_start: Option<&BTreeMap<usize, CartesianPoint>>,
    remaining_interval_boxes: usize,
    before_fingerprint: Option<u64>,
    pre_vertices: usize,
    pre_faces: usize,
    angle_contract: AngleContractId,
    scope: Option<&RegionScope>,
    certification: CommitCertification,
    phase_started: &mut Instant,
) -> Result<(ComponentCommitReport, (usize, usize)), CandidateAttemptFailure> {
    let timing_enabled = std::env::var("EARTHMESH_CMRC_TIMING").as_deref() == Ok("1");
    let candidate = transition.candidate.clone();
    work.install(source, &candidate).map_err(|reason| {
        CandidateAttemptFailure::invalid(ComponentTransactionStage::InstallDelta, reason)
    })?;
    work.apply_positions(state);
    // On a built region, the checks of the whole sphere become the region's
    // (`verify_geometry_within`), and in a window the window's, open along
    // its edge (guide 11.146); the numbering is fixed from here on.
    let geometry_scope = match work.edge_scope() {
        Some(edge) => Some(edge.map_err(|reason| {
            CandidateAttemptFailure::whole(ComponentTransactionStage::InstallDelta, reason)
        })?),
        None => scope.map(|scope| scope.geometry_scope(source, &work.mesh)),
    };
    let verify = |certificate: Certificate, mesh: &MeshState| match &geometry_scope {
        None => certificate.verify_geometry(mesh),
        Some(region) => certificate.verify_geometry_within(mesh, &region.edge_sites, region.euler),
    };
    log_component_phase(timing_enabled, component.id, "install_delta", phase_started);

    let mut elastic_iterations = 0usize;
    let mut elastic_report = None;
    // The faces round every vertex an elastic solve may have moved.
    let mut moved_faces = Vec::new();
    let guard_faces = affected_faces(source, &work.mesh, &candidate);
    if work.reaches_cut(guard_faces.iter().copied()) {
        return Err(CandidateAttemptFailure::whole(
            ComponentTransactionStage::InstallDelta,
            "the candidate's faces reach the edge of its window",
        ));
    }
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
        .geometry_region_passes(&work.mesh.mesh, &guard_faces);
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
            elastic_patch_for_state(transition, &work.mesh, angle_contract).map_err(|reason| {
                CandidateAttemptFailure::retry(ComponentTransactionStage::Elastic, reason)
            })?;
        log_component_phase(timing_enabled, component.id, "prepare_patch", phase_started);
        if work.reaches_cut(patch.guard_faces.iter().copied()) {
            return Err(CandidateAttemptFailure::whole(
                ComponentTransactionStage::Elastic,
                "the elastic patch reaches the edge of its window",
            ));
        }
        let patch_size = (
            patch.movable_compact_vertices.len(),
            patch.guard_faces.len(),
        );
        if let Some(positions) = warm_start {
            // The patch's targets come from the unmoved mesh; only the start
            // moves, and only for the vertices the failed solve moved too.
            for &compact in &patch.movable_compact_vertices {
                if let Some(&point) =
                    work.mesh.source_vertex_slots[compact].and_then(|source| positions.get(&source))
                {
                    work.mesh.mesh.move_vertex(compact, point);
                }
            }
        }
        let outcome = solve_elastic_patch_scoped(
            &work.mesh,
            patch,
            ElasticBlockLimits {
                elastic_iterations: remaining_elastic_iterations,
            },
            angle_contract,
            geometry_scope.as_ref(),
            (certification == CommitCertification::Changed).then_some(&guard_faces),
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
                failure.missed = missed_faces(&witness, &Certificate::internal_for(angle_contract))
                    .map(Box::new);
                if timing_enabled {
                    eprintln!(
                        "{}",
                        failure_places_log(
                            &witness,
                            transition,
                            &Certificate::internal_for(angle_contract),
                            failed_guard_face,
                            component.id,
                        )
                    );
                }
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
                failure.missed = missed_faces(&witness, &Certificate::internal_for(angle_contract))
                    .map(Box::new);
                if timing_enabled {
                    eprintln!(
                        "{}",
                        failure_places_log(
                            &witness,
                            transition,
                            &Certificate::internal_for(angle_contract),
                            failed_guard_face,
                            component.id,
                        )
                    );
                }
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
        moved_faces.clone_from(&elastic.patch.guard_faces);
        work.apply_elastic(state, &elastic);
        elastic_report = Some(elastic.report.clone());
        log_component_phase(timing_enabled, component.id, "elastic_apply", phase_started);
    }

    lower_covered_source_levels(
        source,
        work,
        state,
        &candidate,
        &transition.boundary,
        coarse_level,
    );
    let local_geometry = Certificate::internal_for(angle_contract)
        .verify_geometry_region(&work.mesh.mesh, &guard_faces)
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

    let geometry_failure = |stage: ComponentTransactionStage, error: CertificateError| {
        let mut failure = CandidateAttemptFailure::retry(stage, format!("{error:?}"));
        failure.interval_boxes = interval_boxes;
        failure
    };
    // Only the faces that changed, when the mesh round them was certified
    // before and is certified whole later (`CommitCertification::Changed`).
    // Faces that reach an edge -- a region's, where the whole mesh's checks
    // spare the sites -- are certified with the window round them, or whole.
    let changed_faces = (certification == CommitCertification::Changed).then(|| {
        let mut faces = guard_faces.clone();
        faces.extend(moved_faces.iter().copied());
        faces
    });
    let at_edge = changed_faces.as_ref().is_some_and(|faces| {
        geometry_scope.as_ref().is_some_and(|scope| {
            work.mesh
                .mesh
                .sites_touching(faces)
                .keys()
                .any(|site| scope.edge_sites.contains(site))
        })
    });
    // The whole mesh's counts after the candidate: a window's change is the
    // whole mesh's.
    let (after_vertices, after_faces) = work.counts();
    let post_vertices = (pre_vertices + after_vertices).saturating_sub(work.counts_before.0);
    let post_faces = (pre_faces + after_faces).saturating_sub(work.counts_before.1);
    let level = |source: usize| work.level(state, source);
    let (geometry, cells) = match &changed_faces {
        Some(faces) if !at_edge => {
            let internal = Certificate::internal_for(angle_contract)
                .verify_geometry_region(&work.mesh.mesh, faces)
                .map_err(|error| {
                    geometry_failure(ComponentTransactionStage::GlobalGeometry, error)
                })?;
            log_component_phase(
                timing_enabled,
                component.id,
                "internal_geometry",
                phase_started,
            );
            let final_delivery = Certificate::final_delivery_for(angle_contract)
                .verify_geometry_region(&work.mesh.mesh, faces)
                .map_err(|error| {
                    geometry_failure(ComponentTransactionStage::FinalGeometry, error)
                })?;
            log_component_phase(
                timing_enabled,
                component.id,
                "final_geometry",
                phase_started,
            );
            let (remap, cells) = certify_changed_cells(
                source_remap,
                source_levels,
                &work.mesh,
                &level,
                post_vertices,
                faces,
                scope,
                max_adjacent_level_delta,
                (timing_enabled, component.id),
                phase_started,
            )
            .map_err(|mut failure| {
                failure.interval_boxes = interval_boxes;
                failure
            })?;
            (
                CommitGeometry::Changed {
                    internal,
                    final_delivery,
                },
                CommitCells::Changed { remap, cells },
            )
        }
        Some(faces) if work.within.is_some() => {
            let internal = verify(Certificate::internal_for(angle_contract), &work.mesh.mesh)
                .map_err(|error| {
                    geometry_failure(ComponentTransactionStage::GlobalGeometry, error)
                })?;
            log_component_phase(
                timing_enabled,
                component.id,
                "internal_geometry",
                phase_started,
            );
            let final_delivery = verify(
                Certificate::final_delivery_for(angle_contract),
                &work.mesh.mesh,
            )
            .map_err(|error| geometry_failure(ComponentTransactionStage::FinalGeometry, error))?;
            log_component_phase(
                timing_enabled,
                component.id,
                "final_geometry",
                phase_started,
            );
            let (remap, cells) = certify_changed_cells(
                source_remap,
                source_levels,
                &work.mesh,
                &level,
                post_vertices,
                faces,
                scope,
                max_adjacent_level_delta,
                (timing_enabled, component.id),
                phase_started,
            )
            .map_err(|mut failure| {
                failure.interval_boxes = interval_boxes;
                failure
            })?;
            (
                CommitGeometry::Window {
                    internal,
                    final_delivery,
                },
                CommitCells::Changed { remap, cells },
            )
        }
        _ => {
            let internal = verify(Certificate::internal_for(angle_contract), &work.mesh.mesh)
                .map_err(|error| {
                    geometry_failure(ComponentTransactionStage::GlobalGeometry, error)
                })?;
            log_component_phase(
                timing_enabled,
                component.id,
                "internal_geometry",
                phase_started,
            );
            let final_delivery = verify(
                Certificate::final_delivery_for(angle_contract),
                &work.mesh.mesh,
            )
            .map_err(|error| geometry_failure(ComponentTransactionStage::FinalGeometry, error))?;
            log_component_phase(
                timing_enabled,
                component.id,
                "final_geometry",
                phase_started,
            );
            let target_levels = target_levels_for(
                &work.mesh.mesh,
                &work.mesh.source_vertex_slots,
                &work.delivered_levels(state),
            )
            .map_err(|reason| {
                let mut failure =
                    CandidateAttemptFailure::invalid(ComponentTransactionStage::FinalCells, reason);
                failure.interval_boxes = interval_boxes;
                failure
            })?;
            let remap = match scope {
                None => source_remap.remap_to(&work.mesh.mesh),
                Some(scope) => source_remap.remap_region_to(
                    &work.mesh.mesh,
                    |site| scope.certifies(&work.mesh, site),
                    scope.whole_cells,
                ),
            }
            .map_err(|reason| {
                let mut failure =
                    CandidateAttemptFailure::retry(ComponentTransactionStage::Remap, reason);
                failure.interval_boxes = interval_boxes;
                failure
            })?;
            let remap_certificate = remap.certify_spherical_overlap(
                source_levels.levels().len(),
                target_levels.levels().len(),
            );
            log_component_phase(timing_enabled, component.id, "remap", phase_started);
            let final_cells = match certify_final_cell_requirements_with_remap(
                &source.mesh,
                source_levels,
                &work.mesh.mesh,
                &target_levels,
                max_adjacent_level_delta,
                &remap,
            ) {
                Ok(report) => report,
                Err(FinalCellRequirementError::InvalidInput(reason)) => {
                    let mut failure = CandidateAttemptFailure::invalid(
                        ComponentTransactionStage::FinalCells,
                        reason,
                    );
                    failure.interval_boxes = interval_boxes;
                    return Err(failure);
                }
                Err(FinalCellRequirementError::Residuals(report)) => {
                    let mut failure = CandidateAttemptFailure::retry(
                        ComponentTransactionStage::FinalCells,
                        residuals_reason(report.physical_residuals(), report.balance_residuals()),
                    );
                    failure.interval_boxes = interval_boxes;
                    return Err(failure);
                }
            };
            log_component_phase(timing_enabled, component.id, "final_cells", phase_started);
            let final_evidence = FinalCertificationEvidence::from_final_cells(
                &final_cells,
                remap_certificate.clone(),
            )
            .map_err(|reason| {
                let mut failure =
                    CandidateAttemptFailure::retry(ComponentTransactionStage::Remap, reason);
                failure.interval_boxes = interval_boxes;
                failure
            })?;
            let geometry = GeometryCertifiedMotherGrid::new(work.mesh.mesh.clone(), final_delivery);
            let final_mesh = match remap.covered_targets() {
                None => crate::finalize_geometry_certified_mother(geometry, final_evidence),
                Some(covered) => {
                    crate::api::finalize_region_geometry(geometry, final_evidence, covered.len())
                }
            }
            .map_err(|error| geometry_failure(ComponentTransactionStage::FinalGeometry, error))?;
            (
                CommitGeometry::Whole {
                    internal,
                    final_certificate: final_mesh.certificate().clone(),
                },
                CommitCells::Whole {
                    final_cells,
                    remap: remap_certificate,
                },
            )
        }
    };
    log_component_phase(timing_enabled, component.id, "finalize", phase_started);

    if post_vertices >= pre_vertices || post_faces >= pre_faces {
        let mut failure = CandidateAttemptFailure::retry(
            ComponentTransactionStage::Postcondition,
            "component transaction did not reduce both vertices and faces".to_string(),
        );
        failure.interval_boxes = interval_boxes;
        return Err(failure);
    }
    // Core sites the candidate removed: those before and not after, both
    // in ascending order.
    let core_sources = source_sites_for_parents(source, candidate.core_parents.iter().copied());
    let mut after = work
        .mesh
        .source_vertex_slots
        .iter()
        .flatten()
        .copied()
        .peekable();
    let mut core_vertices_removed = 0usize;
    for &before in &work.sources_before {
        while after.next_if(|&site| site < before).is_some() {}
        if after.peek() != Some(&before) && core_sources.contains(&before) {
            core_vertices_removed += 1;
        }
    }

    Ok((
        ComponentCommitReport {
            component_id: component.id,
            before_fingerprint,
            after_fingerprint: (certification == CommitCertification::Whole)
                .then(|| mesh_fingerprint(&work.mesh.mesh)),
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
            geometry,
            cells,
            elastic: elastic_report,
        },
        (post_vertices, post_faces),
    ))
}

/// Why a candidate's cells failed their requirements.
fn residuals_reason(physical: usize, balance: usize) -> String {
    format!("{physical} physical and {balance} balance residual(s)")
}

/// The level the cell of `site` delivers, as
/// `ComponentTransactionState::target_levels` gives it.
fn delivered_level(
    mesh: &HierarchyLeafMesh,
    level: &dyn Fn(usize) -> Option<usize>,
    site: usize,
) -> Result<usize, String> {
    let source = mesh
        .source_vertex_slots
        .get(site)
        .and_then(|slot| *slot)
        .ok_or_else(|| format!("active target site {site} has no source slot"))?;
    level(source).ok_or_else(|| format!("source site {source} has no delivered level"))
}

/// The sites joined to `site` by an edge, found by walking the faces round
/// it from `face`, whether they close round it or not.
fn sites_round(mesh: &MeshState, site: usize, face: usize) -> BTreeSet<usize> {
    let mut seen = BTreeSet::from([face]);
    let mut stack = vec![face];
    let mut around = BTreeSet::new();
    while let Some(current) = stack.pop() {
        let corners = mesh.triangles()[current];
        let Some(corner) = corners.iter().position(|&other| other == site) else {
            continue;
        };
        around.insert(corners[(corner + 1) % 3]);
        around.insert(corners[(corner + 2) % 3]);
        // The two edges at `site` lie opposite its other corners.
        for across in [(corner + 1) % 3, (corner + 2) % 3] {
            let next = mesh.neighbours()[current][across];
            if next != 0 && mesh.is_triangle_live(next) && seen.insert(next) {
                stack.push(next);
            }
        }
    }
    around
}

/// The cells a candidate changed, certified as the whole mesh's are (guide
/// 11.145). A Voronoi cell, a delivered level or an edge the candidate
/// changed belongs to a site of `faces` -- every face it made or moved a
/// corner of -- so those sites' remap rows (the ones `scope` certifies),
/// made as the whole remap makes them and certified with its tolerances,
/// their requirements and the balance of every edge at them decide the
/// commit. Every other row, requirement and edge is as the commit that
/// last changed it, or the level's certificate, left it.
#[allow(clippy::too_many_arguments)]
fn certify_changed_cells(
    source_remap: &VoronoiRemapSource<'_>,
    source_levels: &SourceLevelField,
    transaction: &HierarchyLeafMesh,
    level: &dyn Fn(usize) -> Option<usize>,
    whole_cells: usize,
    faces: &BTreeSet<usize>,
    scope: Option<&RegionScope>,
    max_adjacent_level_delta: usize,
    (timing_enabled, component): (bool, u64),
    phase_started: &mut Instant,
) -> Result<(RemapCertificate, ChangedCellsReport), CandidateAttemptFailure> {
    let invalid = |reason: String| {
        CandidateAttemptFailure::invalid(ComponentTransactionStage::FinalCells, reason)
    };
    let mesh = &transaction.mesh;
    // Each changed site with a face round it, the edges at them, and the
    // levels at both ends, read first as the whole mesh's levels are.
    let mut sites = BTreeMap::new();
    for &face in faces {
        for site in mesh.triangles()[face] {
            sites.entry(site).or_insert(face);
        }
    }
    let mut edges = BTreeSet::new();
    for (&site, &face) in &sites {
        for other in sites_round(mesh, site, face) {
            edges.insert((site.min(other), site.max(other)));
        }
    }
    let mut levels = BTreeMap::new();
    for &(left, right) in &edges {
        for site in [left, right] {
            if let std::collections::btree_map::Entry::Vacant(entry) = levels.entry(site) {
                entry.insert(delivered_level(transaction, level, site).map_err(invalid)?);
            }
        }
    }
    // The rows, in the order of the mesh's cells, whose number among the
    // whole mesh's they bound; a window's are fewer.
    let mut cells = Vec::new();
    let mut ids = Vec::new();
    for (id, site) in mesh.active_vertex_slots().enumerate() {
        if let Some(&face) = sites.get(&site) {
            if scope.is_none_or(|scope| scope.certifies(transaction, site)) {
                cells.push((site, face));
                ids.push(id);
            }
        }
    }
    let remap = source_remap
        .remap_sites_to(
            mesh,
            &cells,
            ids,
            scope.map_or(whole_cells, |scope| scope.whole_cells),
        )
        .map_err(|reason| {
            CandidateAttemptFailure::retry(ComponentTransactionStage::Remap, reason)
        })?;
    let certificate = remap.certify_spherical_overlap(source_levels.levels().len(), whole_cells);
    log_component_phase(timing_enabled, component, "remap", phase_started);
    if certificate.negative_weights() + certificate.bad_row_sums() + certificate.bad_lineage_rows()
        != 0
        || certificate.constant_closure_error() > certificate.closure_tolerance()
        || certificate.global_area_closure_error() > certificate.closure_tolerance()
    {
        return Err(invalid(format!(
            "Voronoi overlap remap failed certification: negative={}, bad_rows={}, bad_lineage={}, constant_error={}, area_error={}, tolerance={}",
            certificate.negative_weights(),
            certificate.bad_row_sums(),
            certificate.bad_lineage_rows(),
            certificate.constant_closure_error(),
            certificate.global_area_closure_error(),
            certificate.closure_tolerance(),
        )));
    }
    let mut physical = 0usize;
    for (row, &(site, _)) in remap.rows().iter().zip(&cells) {
        let (required, _) = row_required_level(row, source_levels.levels())
            .map_err(|reason| invalid(reason.to_string()))?;
        if levels[&site] < required {
            physical += 1;
        }
    }
    let balance = edges
        .iter()
        .filter(|(left, right)| levels[left].abs_diff(levels[right]) > max_adjacent_level_delta)
        .count();
    log_component_phase(timing_enabled, component, "final_cells", phase_started);
    if physical + balance != 0 {
        return Err(CandidateAttemptFailure::retry(
            ComponentTransactionStage::FinalCells,
            residuals_reason(physical, balance),
        ));
    }
    Ok((
        certificate,
        ChangedCellsReport {
            cells: cells.len(),
            edges: edges.len(),
        },
    ))
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

/// What a failed solve left outside the certificate's window, for the retry
/// log: how many guard faces, the worst excess, how many lie within a tenth
/// of it, the count in each band of excess, and how many core parents the
/// worst 64 would promote; then the failed face's three rings, each face's
/// kind (`failed_face_neighbourhood`), angles and corners, a movable corner
/// starred and followed by its degree.
fn failure_places_log(
    witness: &GeometryFailureWitness,
    transition: &super::TransitionTopologyTrial,
    certificate: &Certificate,
    failed: Option<usize>,
    component: u64,
) -> String {
    let mesh = &witness.mesh;
    let corners = |face: usize| mesh.mesh.triangles()[face].map(|site| mesh.mesh.vertices()[site]);
    let mut outside = Vec::new();
    for &face in &witness.patch.guard_faces {
        let Some(angles) = crate::certificate::spherical_triangle_angles(corners(face)) else {
            continue;
        };
        let smallest = angles.iter().copied().fold(f64::INFINITY, f64::min);
        let largest = angles.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let excess =
            (certificate.min_angle_degrees - smallest).max(largest - certificate.max_angle_degrees);
        if excess > 0.0 {
            outside.push((excess, face));
        }
    }
    outside.sort_by(|left, right| right.0.total_cmp(&left.0).then(left.1.cmp(&right.1)));
    let worst = outside.first().map_or(0.0, |&(excess, _)| excess);
    let bounds = [0.05, 0.1, 0.2, 0.3, 0.5, f64::INFINITY];
    let mut bands = [0usize; 6];
    for &(excess, _) in &outside {
        bands[bounds
            .iter()
            .position(|&bound| excess <= bound)
            .unwrap_or(5)] += 1;
    }
    let near_worst = outside
        .iter()
        .filter(|&&(excess, _)| excess >= 0.9 * worst)
        .count();
    let core = transition
        .candidate
        .core_parents
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let parents = outside
        .iter()
        .take(64)
        .filter_map(|&(_, face)| nearest_core_parent(mesh, &core, face))
        .collect::<BTreeSet<_>>()
        .len();
    let mut log = format!(
        "earthmesh_cli: cmrc_detail phase=failure_places component={component} outside={} \
         worst={worst:.6} near_worst={near_worst} bands(0.05,0.1,0.2,0.3,0.5,more)={bands:?} \
         parents_of_worst64={parents}",
        outside.len()
    );
    let Some(face) = failed.filter(|&face| mesh.mesh.is_triangle_live(face)) else {
        return log;
    };
    let movable = witness
        .patch
        .movable_compact_vertices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let kind = |face: usize| match mesh.triangle_addresses[face] {
        None => 'x',
        Some(address) if core.contains(&address) => 'c',
        Some(_) => 'f',
    };
    let mut seen = BTreeSet::from([face]);
    let mut frontier = vec![face];
    for ring in 0..=3 {
        for &face in &frontier {
            let angles = crate::certificate::spherical_triangle_angles(corners(face))
                .unwrap_or([f64::NAN; 3]);
            let sites = mesh.mesh.triangles()[face];
            let text = sites
                .iter()
                .map(|&site| {
                    let point = mesh.mesh.vertices()[site];
                    let norm = (point.x * point.x + point.y * point.y + point.z * point.z).sqrt();
                    let degree = mesh
                        .mesh
                        .vertex_degree_from(site, face)
                        .map_or("?".to_string(), |degree| degree.to_string());
                    format!(
                        "{}{site}:{:.6},{:.6}:d{degree}",
                        if movable.contains(&site) { "*" } else { "" },
                        point.y.atan2(point.x).to_degrees(),
                        (point.z / norm).asin().to_degrees()
                    )
                })
                .collect::<Vec<_>>()
                .join(" ");
            log.push_str(&format!(
                "\nearthmesh_cli: cmrc_detail phase=failure_face component={component} ring={ring} \
                 face={face} kind={} angles={:.4}/{:.4}/{:.4} corners={text}",
                kind(face),
                angles[0],
                angles[1],
                angles[2]
            ));
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
    log
}

fn preferred_core_promotion_for_face(
    mesh: &HierarchyLeafMesh,
    transition: &super::TransitionTopologyTrial,
    failed_face: usize,
) -> Option<TriangleAddress> {
    let core = transition
        .candidate
        .core_parents
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    nearest_core_parent(mesh, &core, failed_face)
}

/// The core parent nearest a face in face steps, the lowest of equally near
/// ones. The faces passed are kept in a set rather than a mesh-sized table:
/// a failure that missed at thousands of places (guide 11.139) looks each
/// one up, and its nearest core parent lies a step or two away.
fn nearest_core_parent(
    mesh: &HierarchyLeafMesh,
    core: &BTreeSet<TriangleAddress>,
    face: usize,
) -> Option<TriangleAddress> {
    if !mesh.mesh.is_triangle_live(face) {
        return None;
    }
    let mut seen = BTreeSet::from([face]);
    let mut queue = VecDeque::from([face]);
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
                if neighbour != 0 && mesh.mesh.is_triangle_live(neighbour) && seen.insert(neighbour)
                {
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
    source_remap: &VoronoiRemapSource<'_>,
    source_levels: &SourceLevelField,
    state: &ComponentTransactionState,
    component: &HierarchyComponent,
    source_active_sites: &[usize],
) -> Result<(), String> {
    // The remap source hashes the source mesh once for every component of a
    // run; a whole fine mother hashed per component cost as much as a small
    // component's search.
    let source_fingerprint = if source_remap.is_source(&source.mesh) {
        source_remap.source_fingerprint()
    } else {
        mesh_fingerprint(&source.mesh)
    };
    if state.source_fingerprint != source_fingerprint
        || state.source_subdivision != source.subdivision
    {
        return Err("transaction state belongs to a different source mesh".into());
    }
    // A scheduler passes the level field's own list, matched with the source
    // once for the run: compared again, every component paid for a sweep of
    // the fine mother's sites.
    if !std::ptr::eq(source_levels.active_sites(), source_active_sites)
        && source_levels.active_sites() != source_active_sites
    {
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

/// What a candidate is built and certified on (guide 11.146): the whole
/// state's leaves, transition triangles and mesh, or those of a window of
/// parents round the component, rebuilt from the state with every site and
/// face in the order the whole mesh has them. The source positions and
/// delivered levels a candidate changes are kept apart until it commits.
struct Work {
    /// The fine source faces under the window's parents, when it is one.
    within: Option<BTreeSet<usize>>,
    leaf_set: HierarchyLeafSet,
    custom_transition_triangles: BTreeMap<TriangleAddress, Vec<[usize; 3]>>,
    mesh: HierarchyLeafMesh,
    /// Before the candidate: the mesh's source sites, ascending, and its
    /// active vertices and faces.
    sources_before: Vec<usize>,
    counts_before: (usize, usize),
    /// A window's edge, before the candidate: the source sites on it, open
    /// as a region's edge is, and the window's Euler characteristic.
    edge: Option<(BTreeSet<usize>, isize)>,
    /// The sites of that edge that are not the region's: faces with a
    /// corner there lack neighbours the whole mesh has.
    cut: BTreeSet<usize>,
    moved: BTreeMap<usize, CartesianPoint>,
    lowered: Lowered,
}

/// The delivered levels a candidate lowers (`lower_covered_source_levels`):
/// apart from the state's in a window; in a copy of them for the whole
/// state, whose candidates lower every site under a core that can hold
/// most of the mesh.
enum Lowered {
    Window(BTreeMap<usize, usize>),
    Whole(Vec<Option<usize>>),
}

impl Work {
    /// The whole state, cloned.
    fn whole(state: &ComponentTransactionState) -> Self {
        Self {
            within: None,
            leaf_set: state.leaf_set.clone(),
            custom_transition_triangles: state.custom_transition_triangles.clone(),
            mesh: state.mesh().clone(),
            sources_before: state
                .mesh()
                .source_vertex_slots
                .iter()
                .flatten()
                .copied()
                .collect(),
            counts_before: (state.vertex_count, state.face_count),
            edge: None,
            cut: BTreeSet::new(),
            moved: BTreeMap::new(),
            lowered: Lowered::Whole(state.source_delivered_levels.clone()),
        }
    }

    /// The state under `parents`: their leaves and transition triangles,
    /// and the mesh of those alone, open along the window's edge.
    fn window(
        source: &MotherGrid,
        state: &ComponentTransactionState,
        parents: &BTreeSet<TriangleAddress>,
    ) -> Result<Self, String> {
        let mut leaves = Vec::new();
        let mut custom = BTreeMap::new();
        let mut within = Vec::new();
        let mut stack = Vec::new();
        for &parent in parents {
            for face in super::core_condensation::source_faces_under(source, parent)? {
                within.push(source_face_slot(source, face)?);
            }
            stack.push(parent);
            while let Some(face) = stack.pop() {
                if state.leaf_set.leaves.contains(&face) {
                    leaves.push(face);
                } else if let Some(triangles) = state.custom_transition_triangles.get(&face) {
                    custom.insert(face, triangles.clone());
                } else if face.n >= source.subdivision {
                    return Err(format!(
                        "window face {face:?} is neither a leaf nor a transition parent"
                    ));
                } else {
                    stack.extend(
                        face.children_2_to_1()
                            .ok_or_else(|| format!("invalid hierarchy address {face:?}"))?,
                    );
                }
            }
        }
        leaves.sort_unstable();
        let leaf_set = HierarchyLeafSet {
            leaves: leaves.into_iter().collect(),
        };
        let within = within.into_iter().collect::<BTreeSet<_>>();
        let (custom_parents, custom_triangles) = custom_parts(&custom);
        let mut mesh = super::core_condensation::rebuild_custom_within(
            source,
            &leaf_set,
            &custom_parents,
            &custom_triangles,
            Some(&within),
        )?;
        apply_source_positions(&mut mesh, &state.source_positions);
        let (open, euler) = open_sites_and_euler(&mesh.mesh);
        let edge = open
            .into_iter()
            .map(|site| {
                mesh.source_vertex_slots[site]
                    .ok_or_else(|| format!("window site {site} has no source slot"))
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        let cut = match &source.region {
            Some(region) => edge.difference(region.outer_boundary()).copied().collect(),
            None => edge.clone(),
        };
        Ok(Self {
            within: Some(within),
            leaf_set,
            custom_transition_triangles: custom,
            sources_before: mesh.source_vertex_slots.iter().flatten().copied().collect(),
            counts_before: (mesh.mesh.vertex_count(), mesh.mesh.triangle_count()),
            mesh,
            edge: Some((edge, euler)),
            cut,
            moved: BTreeMap::new(),
            lowered: Lowered::Window(BTreeMap::new()),
        })
    }

    /// `candidate` installed: its core condensed, its transition parents'
    /// children replaced by its triangles, and the mesh rebuilt.
    fn install(
        &mut self,
        source: &MotherGrid,
        candidate: &TransitionTopologyCandidate,
    ) -> Result<(), String> {
        install_leaves(
            &mut self.leaf_set,
            &mut self.custom_transition_triangles,
            candidate,
        )?;
        let (custom_parents, custom_triangles) = custom_parts(&self.custom_transition_triangles);
        self.mesh = super::core_condensation::rebuild_custom_within(
            source,
            &self.leaf_set,
            &custom_parents,
            &custom_triangles,
            self.within.as_ref(),
        )?;
        Ok(())
    }

    /// Where `source` lies: as the candidate moved it, or as the state has it.
    fn position(&self, state: &ComponentTransactionState, source: usize) -> CartesianPoint {
        self.moved
            .get(&source)
            .copied()
            .unwrap_or(state.source_positions[source])
    }

    /// The level `source` delivers: as the candidate lowered it, or as the
    /// state has it.
    fn level(&self, state: &ComponentTransactionState, source: usize) -> Option<usize> {
        match &self.lowered {
            Lowered::Window(lowered) => lowered
                .get(&source)
                .copied()
                .or_else(|| state.source_delivered_levels.get(source).copied().flatten()),
            Lowered::Whole(levels) => levels.get(source).copied().flatten(),
        }
    }

    /// `source` delivers `level` once the candidate commits.
    fn lower(&mut self, source: usize, level: usize) {
        match &mut self.lowered {
            Lowered::Window(lowered) => {
                lowered.insert(source, level);
            }
            Lowered::Whole(levels) => levels[source] = Some(level),
        }
    }

    /// The mesh's vertices where the state, and the candidate, put them.
    fn apply_positions(&mut self, state: &ComponentTransactionState) {
        for compact in 0..self.mesh.source_vertex_slots.len() {
            if let Some(source) = self.mesh.source_vertex_slots[compact] {
                let position = self.position(state, source);
                self.mesh.mesh.move_vertex(compact, position);
            }
        }
    }

    /// The elastic solve's mesh, and the positions it moved.
    fn apply_elastic<G>(
        &mut self,
        state: &ComponentTransactionState,
        elastic: &ElasticBlockTrial<G>,
    ) {
        self.mesh = elastic.mesh.clone();
        for (compact, source) in self.mesh.source_vertex_slots.iter().copied().enumerate() {
            let Some(source) = source else {
                continue;
            };
            let position = self.mesh.mesh.vertices()[compact];
            let known = self.position(state, source);
            if [position.x, position.y, position.z].map(f64::to_bits)
                != [known.x, known.y, known.z].map(f64::to_bits)
            {
                self.moved.insert(source, position);
            }
        }
    }

    /// Every source site's delivered level, the candidate's changes made.
    fn delivered_levels<'a>(
        &'a self,
        state: &ComponentTransactionState,
    ) -> std::borrow::Cow<'a, [Option<usize>]> {
        match &self.lowered {
            Lowered::Window(lowered) => {
                let mut levels = state.source_delivered_levels.clone();
                for (&source, &level) in lowered {
                    levels[source] = Some(level);
                }
                std::borrow::Cow::Owned(levels)
            }
            Lowered::Whole(levels) => std::borrow::Cow::Borrowed(levels),
        }
    }

    /// Whether a corner of `faces`, whose fans the checks read, lies on the
    /// window's own edge.
    fn reaches_cut(&self, faces: impl IntoIterator<Item = usize>) -> bool {
        !self.cut.is_empty()
            && faces.into_iter().any(|face| {
                self.mesh.mesh.triangles()[face].iter().any(|&site| {
                    self.mesh.source_vertex_slots[site]
                        .is_some_and(|source| self.cut.contains(&source))
                })
            })
    }

    /// The mesh's active vertices and faces now.
    fn counts(&self) -> (usize, usize) {
        (
            self.mesh.mesh.vertex_count(),
            self.mesh.mesh.triangle_count(),
        )
    }

    /// The edge of a window as the mesh now numbers it: `None` for the
    /// whole state, or when the candidate removed a site on the edge -- it
    /// reached past the window.
    fn edge_scope(&self) -> Option<Result<GeometryScope, String>> {
        let (edge, euler) = self.edge.as_ref()?;
        let edge_sites = self
            .mesh
            .source_vertex_slots
            .iter()
            .enumerate()
            .filter(|(_, source)| source.is_some_and(|source| edge.contains(&source)))
            .map(|(compact, _)| compact)
            .collect::<BTreeSet<_>>();
        Some(if edge_sites.len() == edge.len() {
            Ok(GeometryScope {
                edge_sites,
                euler: *euler,
            })
        } else {
            Err("the candidate reached the edge of its window".to_string())
        })
    }

    /// Commits the candidate to `state`: a whole work replaces the state's
    /// leaves, triangles and mesh; a window's changes are made to the
    /// state's leaves and triangles, and the whole mesh is left to be
    /// rebuilt (`ComponentTransactionState::refresh`).
    fn commit(
        self,
        state: &mut ComponentTransactionState,
        candidate: &TransitionTopologyCandidate,
        parent_subdivision: usize,
        counts: (usize, usize),
    ) -> Result<(), String> {
        match self.within {
            None => {
                state.leaf_set = self.leaf_set;
                state.custom_transition_triangles = self.custom_transition_triangles;
                state.mesh = self.mesh;
            }
            Some(_) => {
                install_leaves(
                    &mut state.leaf_set,
                    &mut state.custom_transition_triangles,
                    candidate,
                )?;
                state.stale = true;
            }
        }
        for (source, position) in self.moved {
            state.source_positions[source] = position;
        }
        match self.lowered {
            Lowered::Window(lowered) => {
                for (source, level) in lowered {
                    state.source_delivered_levels[source] = Some(level);
                }
            }
            Lowered::Whole(levels) => state.source_delivered_levels = levels,
        }
        (state.vertex_count, state.face_count) = counts;
        state.prepare_parent_level(parent_subdivision);
        state
            .claimed_parents
            .extend(candidate.core_parents.iter().copied());
        state
            .claimed_parents
            .extend(candidate.custom_transition_triangles.keys().copied());
        Ok(())
    }
}

/// `candidate`'s core condensed in `leaf_set` and its transition parents'
/// children replaced by their triangles.
fn install_leaves(
    leaf_set: &mut HierarchyLeafSet,
    custom: &mut BTreeMap<TriangleAddress, Vec<[usize; 3]>>,
    candidate: &TransitionTopologyCandidate,
) -> Result<(), String> {
    leaf_set.condense_core(&candidate.core_parents)?;
    for (&parent, triangles) in &candidate.custom_transition_triangles {
        if custom.contains_key(&parent) {
            return Err(format!(
                "custom transition parent {parent:?} is already installed"
            ));
        }
        for child in parent
            .children_2_to_1()
            .ok_or_else(|| format!("invalid custom transition parent {parent:?}"))?
        {
            leaf_set.leaves.remove(&child);
        }
        custom.insert(parent, triangles.clone());
    }
    Ok(())
}

/// The sites on a mesh's open edges, and its Euler characteristic.
fn open_sites_and_euler(mesh: &MeshState) -> (BTreeSet<usize>, isize) {
    let mut open = BTreeSet::new();
    let (mut faces, mut open_edges) = (0usize, 0usize);
    for face in mesh.active_triangle_slots() {
        faces += 1;
        let corners = mesh.triangles()[face];
        // The edge across `neighbours()[face][k]` lies opposite corner k.
        for (corner, &neighbour) in mesh.neighbours()[face].iter().enumerate() {
            if neighbour == 0 || !mesh.is_triangle_live(neighbour) {
                open_edges += 1;
                open.insert(corners[(corner + 1) % 3]);
                open.insert(corners[(corner + 2) % 3]);
            }
        }
    }
    let edges = (3 * faces + open_edges) / 2;
    (
        open,
        mesh.vertex_count() as isize - edges as isize + faces as isize,
    )
}

/// The transition parents and their triangles in the order a rebuild
/// appends them.
fn custom_parts(
    custom: &BTreeMap<TriangleAddress, Vec<[usize; 3]>>,
) -> (BTreeSet<TriangleAddress>, Vec<[usize; 3]>) {
    (
        custom.keys().copied().collect(),
        custom
            .values()
            .flat_map(|triangles| triangles.iter().copied())
            .collect(),
    )
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

fn lower_covered_source_levels(
    source: &MotherGrid,
    work: &mut Work,
    state: &ComponentTransactionState,
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
                if let Some(level) = work.level(state, source_site) {
                    work.lower(source_site, level.min(coarse_level));
                }
            }
            Ok(())
        });
    }
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

    thread_local! {
        /// Candidates built again whole after reaching their window's edge.
        pub(super) static WINDOW_CUTS: std::cell::Cell<usize> =
            const { std::cell::Cell::new(0) };
        /// Whether a transaction's window is narrowed to the component's
        /// own parents.
        pub(super) static NARROW_WINDOWS: std::cell::Cell<bool> =
            const { std::cell::Cell::new(false) };
    }

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

    /// A failed solve lists the guard faces it left outside the window,
    /// worst first, and is a near miss when none lies more than
    /// `NEAR_MISS_DEGREES` out. None out at all lists nothing.
    #[test]
    fn a_failure_lists_the_faces_it_left_outside_the_window() {
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
        assert_eq!(missed_faces(&moved(0.0), &certificate), None);
        let near = moved(fraction_for(0.5 * NEAR_MISS_DEGREES));
        let miss = missed_faces(&near, &certificate).expect("faces outside");
        assert!(miss.is_near());
        assert!(miss.worst > 0.0 && miss.worst <= NEAR_MISS_DEGREES);
        assert!(!miss.faces.is_empty());
        assert!(miss
            .faces
            .iter()
            .all(|face| near.mesh.mesh.triangles()[*face].contains(&centre)));
        let far = moved(fraction_for(2.0 * NEAR_MISS_DEGREES));
        assert!(excess(&far) > NEAR_MISS_DEGREES);
        let miss = missed_faces(&far, &certificate).expect("faces outside");
        assert!(!miss.is_near());
        assert!((miss.worst - excess(&far)).abs() < 1.0e-12);
        assert!(miss
            .faces
            .iter()
            .all(|face| far.mesh.mesh.triangles()[*face].contains(&centre)));
    }

    /// A near miss has stalled only when the candidate before was one too
    /// and the worst face came less than a tenth of the way in.
    #[test]
    fn a_near_miss_stalls_only_after_another_that_came_no_nearer() {
        let miss = |worst| Missed {
            worst,
            faces: vec![1],
        };
        assert!(!near_miss_stalled(None, Some(0.05)));
        assert!(!near_miss_stalled(Some(&miss(0.05)), None));
        assert!(!near_miss_stalled(Some(&miss(0.04)), Some(0.08)));
        assert!(near_miss_stalled(Some(&miss(0.0192)), Some(0.0193)));
        assert!(near_miss_stalled(Some(&miss(0.06)), Some(0.05)));
        // A failure farther out is no near miss, stalled or not.
        assert!(!near_miss_stalled(Some(&miss(0.2)), Some(0.05)));
    }

    /// The places a failure's next candidate promotes besides its worst: a
    /// failure that missed at more than `WIDESPREAD_PLACES` places gives
    /// every one, each once and in its faces' order; at fewer, only a
    /// stalled near miss gives any, up to `NEAR_MISS_PLACES` in all.
    #[test]
    fn a_failure_that_missed_at_many_places_is_retried_at_all_of_them() {
        let parent = |i: usize| TriangleAddress {
            base_face: 0,
            i,
            j: 0,
            n: 1_000,
            orientation: crate::mother_grid::TriangleOrientation::Up,
        };
        // Two faces at every place, the worst place's first.
        let missed = |places: usize, worst: f64| Missed {
            worst,
            faces: (0..2 * places).collect(),
        };
        let nearest = |face: usize| Some(parent(face / 2));
        let worst = Some(parent(0));
        let many = retry_places(&missed(WIDESPREAD_PLACES + 1, 0.3), false, worst, nearest);
        assert_eq!(
            many,
            (1..=WIDESPREAD_PLACES).map(parent).collect::<Vec<_>>()
        );
        // As many places as the bound: no more than any other failure.
        let bound = missed(WIDESPREAD_PLACES, 0.3);
        assert!(retry_places(&bound, false, worst, nearest).is_empty());
        let near = missed(WIDESPREAD_PLACES, 0.05);
        assert!(retry_places(&near, false, worst, nearest).is_empty());
        assert_eq!(
            retry_places(&near, true, worst, nearest),
            (1..NEAR_MISS_PLACES).map(parent).collect::<Vec<_>>()
        );
        // A face no core parent is near gives no place.
        let few = Missed {
            worst: 0.05,
            faces: vec![0, 1, 2, 3, 4],
        };
        let nearest = |face: usize| (face != 3).then(|| parent(face));
        assert_eq!(
            retry_places(&few, true, worst, nearest),
            vec![parent(1), parent(2), parent(4)]
        );
    }

    /// A core parent and the two rings of parents round it as its
    /// transition: a candidate changes a small part of the sphere.
    fn ringed_component(source: &MotherGrid, coarse_n: usize) -> HierarchyComponent {
        let core = TriangleAddress {
            base_face: 0,
            i: 1,
            j: 1,
            n: coarse_n,
            orientation: crate::mother_grid::TriangleOrientation::Down,
        };
        let mut transition = BTreeSet::new();
        let mut frontier = vec![core];
        for _ in 0..2 {
            let mut next = Vec::new();
            for parent in frontier {
                for neighbour in hierarchy_parent_neighbours(source, parent).unwrap() {
                    if neighbour != core && transition.insert(neighbour) {
                        next.push(neighbour);
                    }
                }
            }
            frontier = next;
        }
        let mut parents = transition.iter().copied().collect::<Vec<_>>();
        parents.push(core);
        parents.sort_unstable();
        HierarchyComponent {
            id: 7,
            parents,
            boundary_edges: Vec::new(),
            core_parents: vec![core],
            transition_parents: transition.into_iter().collect(),
        }
    }

    fn solve_certifying(
        source: &MotherGrid,
        state: &mut ComponentTransactionState,
        component: &HierarchyComponent,
        certification: CommitCertification,
    ) -> ComponentTransactionOutcome {
        let levels = SourceLevelField::from_active_voronoi_cells(
            &source.mesh,
            vec![2; source.mesh.active_vertex_slots().count()],
        )
        .unwrap();
        let active_sites = source.mesh.active_vertex_slots().collect::<Vec<_>>();
        let level_source_slots = source
            .mesh
            .vertices()
            .iter()
            .enumerate()
            .map(|(site, _)| source.mesh.is_vertex_live(site).then_some(site))
            .collect::<Vec<_>>();
        solve_component_transaction_at_level(
            source,
            &VoronoiRemapSource::new(&source.mesh),
            &levels,
            state,
            source,
            &active_sites,
            &level_source_slots,
            component,
            2,
            1,
            ComponentTransactionLimits {
                topology_states: 10_000,
                elastic_iterations: 1_024,
                interval_boxes: 1_000_000,
                halo_expansions: 0,
                retry_at_failure: true,
            },
            AngleContractId::DomainQuality38To82V1,
            None,
            certification,
        )
    }

    /// Certifying only what a commit changed (guide 11.143, 11.145), in the
    /// window its search used (11.146), commits what certifying the whole
    /// mesh commits: the same mesh, with its geometry and its cells checked
    /// where the candidate changed them -- the remap rows with the whole
    /// remap's tolerances.
    #[test]
    fn a_commit_certified_where_it_changed_is_the_commit_certified_whole() {
        let source = MotherGrid::generate(16).unwrap();
        let component = ringed_component(&source, 8);
        let initial = ComponentTransactionState::new(&source, 3).unwrap();
        let mut whole_state = initial.clone();
        let whole = solve_certifying(
            &source,
            &mut whole_state,
            &component,
            CommitCertification::Whole,
        );
        let mut changed_state = initial.clone();
        let changed = solve_certifying(
            &source,
            &mut changed_state,
            &component,
            CommitCertification::Changed,
        );
        let (
            ComponentTransactionOutcome::Certified(whole),
            ComponentTransactionOutcome::Certified(changed),
        ) = (whole, changed)
        else {
            panic!("the ringed component certifies either way")
        };
        // Built in the window its search used, the commit left the whole
        // mesh to be rebuilt (guide 11.146): rebuilt, it is the whole commit's.
        assert!(changed_state.is_stale());
        changed_state.refresh(&source).unwrap();
        assert_eq!(whole_state, changed_state);
        assert!(matches!(changed.geometry, CommitGeometry::Changed { .. }));
        let (
            CommitCells::Whole {
                remap: whole_remap, ..
            },
            CommitCells::Changed { remap, cells },
        ) = (&whole.cells, &changed.cells)
        else {
            panic!("each commit carries its own cell evidence")
        };
        let all_cells = changed_state.mesh.mesh.vertex_count();
        assert!(cells.cells > 0 && cells.cells < all_cells / 2);
        assert!(cells.edges > cells.cells);
        assert_eq!(remap.rows(), cells.cells);
        assert_eq!(whole_remap.rows(), all_cells);
        assert_eq!(remap.closure_tolerance(), whole_remap.closure_tolerance());
        assert!(remap.constant_closure_error() <= whole_remap.constant_closure_error());
    }

    /// A window too narrow for its candidate -- here the component's own
    /// parents, no rings round them -- has the candidate's faces or elastic
    /// patch reach its edge, where the window's mesh lacks the neighbours
    /// the checks read: the candidate is built again on the whole state
    /// (guide 11.146), and the commit is the whole commit.
    #[test]
    fn a_candidate_that_reaches_its_windows_edge_is_built_whole() {
        let source = MotherGrid::generate(16).unwrap();
        let component = ringed_component(&source, 8);
        let initial = ComponentTransactionState::new(&source, 3).unwrap();
        let mut whole_state = initial.clone();
        let whole = solve_certifying(
            &source,
            &mut whole_state,
            &component,
            CommitCertification::Whole,
        );
        NARROW_WINDOWS.with(|narrow| narrow.set(true));
        WINDOW_CUTS.with(|cuts| cuts.set(0));
        let mut changed_state = initial.clone();
        let changed = solve_certifying(
            &source,
            &mut changed_state,
            &component,
            CommitCertification::Changed,
        );
        NARROW_WINDOWS.with(|narrow| narrow.set(false));
        assert!(WINDOW_CUTS.with(std::cell::Cell::get) > 0);
        assert!(matches!(whole, ComponentTransactionOutcome::Certified(_)));
        assert!(matches!(changed, ComponentTransactionOutcome::Certified(_)));
        changed_state.refresh(&source).unwrap();
        assert_eq!(whole_state, changed_state);
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
            window_parents: None,
        };

        assert_eq!(
            preferred_core_promotion_for_face(&mesh, &transition, failed_face),
            Some(core_parent)
        );
    }
}
