//! Deterministic coordinate-only repair of a closed transition topology.

use super::{
    build_stratified_annulus, full_polygon::minor_arc_crossing_strength, relocation_step_window,
    FullPolygonMergeTrial, HierarchyComponent, HierarchyLeafMesh, RingAnchorKind,
    TransitionTopologyCandidate,
};
use crate::{
    certificate::{
        spherical_triangle_angles, voronoi_cell_is_convex_and_contains_site, Certificate,
        CertificateError, GeometryCertificateReport, GEOMETRY_INTERIOR_MARGIN_DEGREES,
    },
    coarsen::TransitionTopologyTrial,
    mother_grid::{MotherGrid, VertexAddress},
};
use earthmesh_mesh::{
    arc_length_unit_sphere, cross, in_circle_on_sphere, magnitude, orientation_on_sphere,
    CartesianPoint, MeshState, Sign,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElasticBlockLimits {
    pub elastic_iterations: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElasticTargetMode {
    TrialReference,
    HierarchyEdge,
    HierarchyEdgeAreaDegree,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GeometryStartId {
    MaterializedSource,
    HierarchySpringEquilibrium,
    RingScaleInterpolation,
    DegreeAngleEquilibrium,
    SignedNormalPlus,
    SignedNormalMinus,
}

impl GeometryStartId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MaterializedSource => "MaterializedSource",
            Self::HierarchySpringEquilibrium => "HierarchySpringEquilibrium",
            Self::RingScaleInterpolation => "RingScaleInterpolation",
            Self::DegreeAngleEquilibrium => "DegreeAngleEquilibrium",
            Self::SignedNormalPlus => "SignedNormalPlus",
            Self::SignedNormalMinus => "SignedNormalMinus",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GeometryDomainId {
    CurrentAnnulus,
    PlusOneOrdinaryRing,
    PlusTwoOrdinaryRings,
}

impl GeometryDomainId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CurrentAnnulus => "CurrentAnnulus",
            Self::PlusOneOrdinaryRing => "PlusOneOrdinaryRing",
            Self::PlusTwoOrdinaryRings => "PlusTwoOrdinaryRings",
        }
    }

    fn expansion_rings(self) -> usize {
        match self {
            Self::CurrentAnnulus => 0,
            Self::PlusOneOrdinaryRing => 1,
            Self::PlusTwoOrdinaryRings => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AngleConstraintKey {
    pub face: usize,
    pub corner: usize,
    pub angle_deg: f64,
    pub signed_margin_deg: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AngleMarginObjective {
    pub signed_margin_deg: f64,
    pub worst_constraints: Vec<AngleConstraintKey>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ElasticSolverMode {
    FiniteDifferenceElastic,
    MarginFiniteDifferenceLexicographic,
    ActiveTangentTrust,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocalGradientStatus {
    UndefinedNearDegenerate,
}

#[derive(Debug, Clone, PartialEq)]
struct LocalAngleGradient {
    angle_deg: f64,
    derivative: [CartesianPoint; 3],
}

#[derive(Debug, Clone, PartialEq)]
struct ActiveTrustStep {
    updates: Vec<(usize, CartesianPoint, CartesianPoint)>,
    predicted_margin_delta: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TrustUpdate {
    accepted: bool,
    ratio: f64,
    next_radius: f64,
}

impl ElasticTargetMode {
    fn uses_hierarchy_edges(self) -> bool {
        !matches!(self, Self::TrialReference)
    }

    fn uses_hierarchy_area_degree(self) -> bool {
        matches!(self, Self::HierarchyEdgeAreaDegree)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::TrialReference => "TrialReference",
            Self::HierarchyEdge => "HierarchyEdge",
            Self::HierarchyEdgeAreaDegree => "HierarchyEdgeAreaDegree",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotherLevelMetric {
    pub level: usize,
    pub median_edge_length: f64,
    pub median_voronoi_area: f64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ElasticTargetField {
    pub target_edge_lengths: BTreeMap<(usize, usize), f64>,
    pub target_cell_areas: BTreeMap<usize, f64>,
    pub target_vertex_scales: BTreeMap<usize, f64>,
    pub target_angles: BTreeMap<usize, f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElasticPatch {
    pub topology: TransitionTopologyCandidate,
    pub reference_positions: Vec<CartesianPoint>,
    pub fixed_compact_vertices: Vec<usize>,
    pub movable_compact_vertices: Vec<usize>,
    pub guard_faces: Vec<usize>,
    pub target_mode: ElasticTargetMode,
    pub target_field: ElasticTargetField,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElasticBlockReport {
    pub component_id: u64,
    pub topology_id: usize,
    pub elastic_iterations: usize,
    pub initial_energy: f64,
    pub final_energy: f64,
    pub moved_compact_vertices: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MovementDistribution {
    pub count: usize,
    pub min: f64,
    pub p50: f64,
    pub p90: f64,
    pub max: f64,
    pub sum: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActiveBoundaryConstraintRatio {
    pub numerator: usize,
    pub denominator: usize,
    pub ratio: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeometryFailureDiagnostics {
    pub movement_distribution: MovementDistribution,
    pub worst_triangle_guard_distance: Option<usize>,
    pub active_boundary_constraint_ratio: Option<ActiveBoundaryConstraintRatio>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElasticBlockTrial {
    pub mesh: HierarchyLeafMesh,
    pub patch: ElasticPatch,
    pub geometry: GeometryCertificateReport,
    pub report: ElasticBlockReport,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ElasticBlockOutcome {
    Certified(Box<ElasticBlockTrial>),
    ElasticNoImprovement {
        elastic_iterations: usize,
        initial_energy: f64,
        final_energy: f64,
        final_phase: ElasticBlockPhase,
        reason: String,
        failed_guard_face: Option<usize>,
        global_angle_degrees: Option<(f64, f64)>,
        guard_angle_degrees: Option<(f64, f64)>,
        diagnostics: Option<GeometryFailureDiagnostics>,
    },
    SearchBudgetExhausted {
        elastic_iterations: usize,
        initial_energy: f64,
        final_energy: f64,
        final_phase: ElasticBlockPhase,
        reason: String,
        failed_guard_face: Option<usize>,
        global_angle_degrees: Option<(f64, f64)>,
        guard_angle_degrees: Option<(f64, f64)>,
        diagnostics: Option<GeometryFailureDiagnostics>,
    },
    RequiresDifferentTopology {
        elastic_iterations: usize,
        initial_energy: f64,
        final_energy: f64,
        final_phase: ElasticBlockPhase,
        reason: String,
        failed_guard_face: Option<usize>,
        global_angle_degrees: Option<(f64, f64)>,
        guard_angle_degrees: Option<(f64, f64)>,
        diagnostics: Option<GeometryFailureDiagnostics>,
    },
    InvalidPatch {
        reason: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ElasticBlockPhase {
    Untangle,
    AngleFeasibility,
    DelaunayVoronoiFeasibility,
    Interior,
}

struct EnergyContext {
    degrees: Vec<usize>,
    guard_edges: Vec<(usize, usize)>,
    guard_faces: Vec<usize>,
    guard_seeds: Vec<(usize, usize)>,
    dual_pairs: Vec<DualPair>,
    derivatives: BTreeMap<usize, DerivativeContext>,
    reference_dual_areas: BTreeMap<usize, Option<f64>>,
}

#[derive(Clone, Copy)]
struct DualPair {
    face: usize,
    opposite: usize,
}

struct DerivativeContext {
    guard_edges: Vec<(usize, usize)>,
    guard_faces: Vec<usize>,
    guard_seeds: Vec<(usize, usize)>,
    dual_pairs: Vec<DualPair>,
}

struct DualEnergy {
    hard_feasible: bool,
    violation: f64,
    center: f64,
    area: f64,
}

impl ElasticPatch {
    pub fn from_transition(trial: &TransitionTopologyTrial) -> Result<Self, String> {
        let mesh = &trial.mesh.mesh;
        if trial.mesh.source_vertex_slots.len() != mesh.vertices().len() {
            return Err("transition source-slot map does not match compact vertices".into());
        }

        let source_to_compact = trial
            .mesh
            .source_vertex_slots
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(compact, source)| source.map(|source| (source, compact)))
            .collect::<BTreeMap<_, _>>();
        let fixed_sources = trial
            .boundary
            .fine_outer_cycles
            .iter()
            .chain(&trial.boundary.coarse_inner_cycles)
            .flat_map(|cycle| cycle.iter().copied())
            .chain(trial.boundary.seam.iter().copied())
            .chain(trial.boundary.pentagon.iter().copied())
            .collect::<BTreeSet<_>>();

        let transition_parents = trial
            .boundary
            .halo_parents
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut transition_compact_vertices = trial
            .candidate
            .source_active_vertices
            .iter()
            .copied()
            .map(|source| {
                source_to_compact.get(&source).copied().ok_or_else(|| {
                    format!("transition source vertex {source} is absent from the compact mesh")
                })
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        for face in mesh.active_triangle_slots() {
            let belongs_to_transition = trial.mesh.triangle_addresses[face]
                .and_then(|address| address.parent_2_to_1())
                .is_some_and(|parent| transition_parents.contains(&parent));
            if belongs_to_transition {
                transition_compact_vertices.extend(mesh.triangles()[face]);
            }
        }
        let movable_compact_vertices = transition_compact_vertices
            .into_iter()
            .filter(|&compact| {
                trial.mesh.source_vertex_slots[compact]
                    .is_some_and(|source| !fixed_sources.contains(&source))
            })
            .collect::<BTreeSet<_>>();
        if movable_compact_vertices.is_empty() {
            return Err("closed transition topology has no movable interior vertex".into());
        }

        let guard_faces = mesh
            .active_triangle_slots()
            .filter(|&face| {
                mesh.triangles()[face]
                    .iter()
                    .any(|site| movable_compact_vertices.contains(site))
            })
            .collect::<BTreeSet<_>>();
        if guard_faces.is_empty() {
            return Err("movable transition vertices have no incident guard faces".into());
        }
        let fixed_compact_vertices = guard_faces
            .iter()
            .flat_map(|&face| mesh.triangles()[face])
            .filter(|site| !movable_compact_vertices.contains(site))
            .collect::<BTreeSet<_>>();

        Ok(Self {
            topology: trial.candidate.clone(),
            reference_positions: mesh.vertices().to_vec(),
            fixed_compact_vertices: fixed_compact_vertices.into_iter().collect(),
            movable_compact_vertices: movable_compact_vertices.into_iter().collect(),
            guard_faces: guard_faces.into_iter().collect(),
            target_mode: ElasticTargetMode::TrialReference,
            target_field: ElasticTargetField::default(),
        })
    }
    pub fn from_full_polygon_merge(
        source: &MotherGrid,
        component: &HierarchyComponent,
        trial: &FullPolygonMergeTrial,
        physical_fixed_sources: &BTreeSet<usize>,
    ) -> Result<Self, String> {
        Self::from_full_polygon_merge_with_domain(
            source,
            component,
            trial,
            physical_fixed_sources,
            GeometryDomainId::CurrentAnnulus,
        )
    }

    pub fn from_full_polygon_merge_with_domain(
        source: &MotherGrid,
        component: &HierarchyComponent,
        trial: &FullPolygonMergeTrial,
        physical_fixed_sources: &BTreeSet<usize>,
        domain_id: GeometryDomainId,
    ) -> Result<Self, String> {
        let mesh = &trial.global_trial.mesh.mesh;
        let source_slots = &trial.global_trial.mesh.source_vertex_slots;
        if source_slots.len() != mesh.vertices().len() {
            return Err("full-polygon source-slot map does not match compact vertices".into());
        }
        let source_to_compact = source_slots
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(compact, source)| source.map(|source| (source, compact)))
            .collect::<BTreeMap<_, _>>();
        let stratified = build_stratified_annulus(source, component).map_err(|error| {
            format!("stratified annulus rejected free-interface patch: {error:?}")
        })?;
        let anchor_sources = source
            .addresses
            .iter()
            .enumerate()
            .filter_map(|(slot, address)| {
                matches!(address, Some(VertexAddress::IcosahedronVertex(_))).then_some(slot)
            })
            .chain(
                stratified
                    .link_contracts
                    .iter()
                    .filter_map(|(&source, contract)| {
                        matches!(
                            contract.anchor_kind,
                            RingAnchorKind::IcosahedronPentagon { .. }
                        )
                        .then_some(source)
                    }),
            )
            .collect::<BTreeSet<_>>();
        let fixed_position_sources = stratified
            .coupled
            .inner_guard
            .vertices
            .iter()
            .chain(&stratified.coupled.coarse_interface.vertices)
            .chain(
                stratified
                    .coupled
                    .intermediate_rings
                    .iter()
                    .flat_map(|ring| ring.vertices.iter()),
            )
            .chain(&stratified.coupled.fine_interface.vertices)
            .chain(&stratified.coupled.outer_guard.vertices)
            .filter_map(|vertex| vertex.fixed_position.then_some(vertex.source_slot))
            .chain(
                stratified
                    .coupled
                    .boundary_contracts
                    .iter()
                    .filter_map(|contract| contract.fixed_position.then_some(contract.source_slot)),
            )
            .collect::<BTreeSet<_>>();
        let current_guard_sources = stratified
            .coupled
            .inner_guard
            .vertices
            .iter()
            .chain(stratified.coupled.outer_guard.vertices.iter())
            .map(|vertex| vertex.source_slot)
            .collect::<BTreeSet<_>>();
        let current_fixed_sources = physical_fixed_sources
            .iter()
            .copied()
            .chain(current_guard_sources.iter().copied())
            .chain(anchor_sources.iter().copied())
            .chain(fixed_position_sources.iter().copied())
            .collect::<BTreeSet<_>>();
        let current_fixed_compact_domain = source_set_to_compact(
            &source_to_compact,
            &current_fixed_sources,
            mesh,
            "free-interface current fixed source vertex",
        )?
        .into_iter()
        .collect::<BTreeSet<_>>();
        let permanent_fixed_sources = physical_fixed_sources
            .iter()
            .copied()
            .chain(anchor_sources.iter().copied())
            .collect::<BTreeSet<_>>();
        let permanent_fixed_compact = source_set_to_compact(
            &source_to_compact,
            &permanent_fixed_sources,
            mesh,
            "free-interface permanent fixed source vertex",
        )?
        .into_iter()
        .collect::<BTreeSet<_>>();
        let current_movable_sources = trial
            .global_trial
            .custom_triangles
            .iter()
            .flat_map(|triangle| triangle.iter().copied())
            .chain(stratified.traces.iter().flat_map(|trace| {
                trace
                    .occurrences
                    .iter()
                    .map(|occurrence| occurrence.source_slot)
            }))
            .chain(
                stratified
                    .shared_junctions
                    .iter()
                    .map(|junction| junction.source_slot),
            )
            .chain(
                trial
                    .global_trial
                    .evidence
                    .selected_ears
                    .iter()
                    .flat_map(|ear| {
                        [
                            ear.removed_neighbour_slot,
                            ear.inserted_chord.0,
                            ear.inserted_chord.1,
                        ]
                    }),
            )
            .filter(|source| {
                !current_fixed_sources.contains(source) && !anchor_sources.contains(source)
            })
            .collect::<BTreeSet<_>>();
        if current_movable_sources.is_empty() {
            return Err("full-polygon free-interface patch has no movable source vertex".into());
        }

        let mut current_movable_compact_vertices = source_set_to_compact(
            &source_to_compact,
            &current_movable_sources,
            mesh,
            "free-interface movable source vertex",
        )?
        .into_iter()
        .collect::<BTreeSet<_>>();
        loop {
            let guard_faces = incident_faces(mesh, &current_movable_compact_vertices);
            let next_movable = guard_faces
                .iter()
                .flat_map(|&face| mesh.triangles()[face])
                .filter(|site| !current_fixed_compact_domain.contains(site))
                .filter(|&site| source_slots[site].is_some())
                .collect::<BTreeSet<_>>();
            if next_movable == current_movable_compact_vertices {
                break;
            }
            current_movable_compact_vertices = next_movable;
        }
        let movable_compact_vertices = expand_movable_domain(
            mesh,
            source_slots,
            &current_movable_compact_vertices,
            &permanent_fixed_compact,
            domain_id,
        );
        let movable_sources = movable_compact_vertices
            .iter()
            .filter_map(|&compact| source_slots[compact])
            .collect::<BTreeSet<_>>();
        if movable_compact_vertices.is_empty() {
            return Err("full-polygon free-interface movable closure is empty".into());
        }
        let guard_faces = incident_faces(mesh, &movable_compact_vertices);
        if guard_faces.is_empty() {
            return Err("full-polygon free-interface movable vertices have no guard faces".into());
        }
        let fixed_compact_vertices = guard_faces
            .iter()
            .flat_map(|&face| mesh.triangles()[face])
            .filter(|site| !movable_compact_vertices.contains(site))
            .collect::<BTreeSet<_>>();
        let topology = TransitionTopologyCandidate {
            component_id: component.id,
            topology_id: trial.evidence.states_examined,
            core_parents: component.core_parents.clone(),
            custom_transition_triangles: BTreeMap::new(),
            source_triangles: trial.global_trial.custom_triangles.clone(),
            source_active_vertices: movable_sources
                .iter()
                .chain(
                    fixed_compact_vertices
                        .iter()
                        .filter_map(|&site| source_slots[site].as_ref()),
                )
                .copied()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            source_degree_forecast: trial.global_trial.evidence.vertex_degrees.clone(),
        };
        Ok(Self {
            topology,
            reference_positions: mesh.vertices().to_vec(),
            fixed_compact_vertices: fixed_compact_vertices.into_iter().collect(),
            movable_compact_vertices: movable_compact_vertices.into_iter().collect(),
            guard_faces: guard_faces.into_iter().collect(),
            target_mode: ElasticTargetMode::TrialReference,
            target_field: ElasticTargetField::default(),
        })
    }

    pub fn with_hierarchy_targets(
        mut self,
        source: &MotherGrid,
        target: &HierarchyLeafMesh,
        source_levels: &[Option<usize>],
        target_mode: ElasticTargetMode,
    ) -> Result<Self, String> {
        self.target_mode = target_mode;
        self.target_field = if target_mode.uses_hierarchy_edges() {
            hierarchy_target_field(source, target, source_levels, &self)?
        } else {
            ElasticTargetField::default()
        };
        Ok(self)
    }
}

fn target_mode_uses_area(mode: ElasticTargetMode) -> bool {
    mode.uses_hierarchy_area_degree()
}

fn hierarchy_target_field(
    source: &MotherGrid,
    target: &HierarchyLeafMesh,
    source_levels: &[Option<usize>],
    patch: &ElasticPatch,
) -> Result<ElasticTargetField, String> {
    if source_levels.len() != source.mesh.vertices().len() {
        return Err("source level slots do not match source mesh vertices".into());
    }
    if target.source_vertex_slots.len() != target.mesh.vertices().len() {
        return Err("target source-slot map does not match target mesh vertices".into());
    }
    let metrics = mother_level_metrics(&source.mesh, source_levels)?;
    let mut field = ElasticTargetField::default();
    let degrees = vertex_degrees(&target.mesh);
    for face in &patch.guard_faces {
        let triangle = target.mesh.triangles()[*face];
        for site in triangle {
            let level = target_source_level(target, source_levels, site)?;
            let metric = metrics
                .get(&level)
                .ok_or_else(|| format!("missing mother level metric for level {level}"))?;
            field
                .target_vertex_scales
                .insert(site, metric.median_edge_length);
            field
                .target_cell_areas
                .insert(site, metric.median_voronoi_area);
            if degrees[site] != 0 {
                field
                    .target_angles
                    .insert(site, std::f64::consts::TAU / degrees[site] as f64);
            }
        }
        for edge in local_triangle_edges(triangle) {
            let left = target_source_level(target, source_levels, edge.0)?;
            let right = target_source_level(target, source_levels, edge.1)?;
            let left = metrics[&left].median_edge_length;
            let right = metrics[&right].median_edge_length;
            field
                .target_edge_lengths
                .insert(edge, (left * right).sqrt());
        }
    }
    Ok(field)
}

fn target_source_level(
    target: &HierarchyLeafMesh,
    source_levels: &[Option<usize>],
    site: usize,
) -> Result<usize, String> {
    let source = target
        .source_vertex_slots
        .get(site)
        .and_then(|source| *source)
        .ok_or_else(|| format!("target site {site} has no source slot for hierarchy targets"))?;
    source_levels
        .get(source)
        .and_then(|level| *level)
        .ok_or_else(|| format!("source site {source} has no hierarchy level"))
}

fn mother_level_metrics(
    mesh: &MeshState,
    source_levels: &[Option<usize>],
) -> Result<BTreeMap<usize, MotherLevelMetric>, String> {
    let mut edge_lengths = BTreeMap::<usize, Vec<f64>>::new();
    let mut areas = mother_level_voronoi_areas(mesh, source_levels);
    let mut edges = BTreeSet::new();
    for face in mesh.active_triangle_slots() {
        let triangle = mesh.triangles()[face];
        for edge in local_triangle_edges(triangle) {
            edges.insert(edge);
        }
    }
    for (left, right) in edges {
        let Some(left_level) = source_levels.get(left).and_then(|level| *level) else {
            continue;
        };
        let Some(right_level) = source_levels.get(right).and_then(|level| *level) else {
            continue;
        };
        if left_level == right_level {
            let length = arc_length_unit_sphere(mesh.vertices()[left], mesh.vertices()[right]);
            if length.is_finite() && length > 0.0 {
                edge_lengths.entry(left_level).or_default().push(length);
            }
        }
    }
    edge_lengths
        .into_iter()
        .map(|(level, mut lengths)| {
            let mut level_areas = areas.remove(&level).unwrap_or_default();
            let median_edge_length = median(&mut lengths)
                .ok_or_else(|| format!("level {level} has no finite source edges"))?;
            let median_voronoi_area = median(&mut level_areas)
                .ok_or_else(|| format!("level {level} has no finite source Voronoi areas"))?;
            Ok((
                level,
                MotherLevelMetric {
                    level,
                    median_edge_length,
                    median_voronoi_area,
                },
            ))
        })
        .collect()
}

fn mother_level_voronoi_areas(
    mesh: &MeshState,
    source_levels: &[Option<usize>],
) -> BTreeMap<usize, Vec<f64>> {
    let mut areas = BTreeMap::<usize, Vec<f64>>::new();
    let mut area_seeds = BTreeMap::<usize, usize>::new();
    for face in mesh.active_triangle_slots() {
        for site in mesh.triangles()[face] {
            if source_levels.get(site).and_then(|level| *level).is_some() {
                area_seeds.entry(site).or_insert(face);
            }
        }
    }
    for (site, seed) in area_seeds {
        let Some(level) = source_levels.get(site).and_then(|level| *level) else {
            continue;
        };
        if let Ok(cell) = mesh.voronoi_cell_from(site, seed) {
            if let Some(area) = cell
                .area_on_unit_sphere()
                .filter(|area| area.is_finite() && *area > 0.0)
            {
                areas.entry(level).or_default().push(area);
            }
        }
    }
    areas
}

fn local_triangle_edges([a, b, c]: [usize; 3]) -> [(usize, usize); 3] {
    [
        (a.min(b), a.max(b)),
        (b.min(c), b.max(c)),
        (c.min(a), c.max(a)),
    ]
}

fn median(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let mid = values.len() / 2;
    if values.len().is_multiple_of(2) {
        Some((values[mid - 1] + values[mid]) * 0.5)
    } else {
        Some(values[mid])
    }
}

pub fn solve_elastic_transition_block(
    transition: &TransitionTopologyTrial,
    limits: ElasticBlockLimits,
) -> ElasticBlockOutcome {
    let patch = match ElasticPatch::from_transition(transition) {
        Ok(patch) => patch,
        Err(reason) => return ElasticBlockOutcome::InvalidPatch { reason },
    };
    solve_elastic_patch(&transition.mesh, patch, limits)
}

pub fn initial_elastic_phase(
    source: &HierarchyLeafMesh,
    patch: &ElasticPatch,
) -> Result<ElasticBlockPhase, String> {
    validate_patch(source, patch)?;
    let guard_faces = patch.guard_faces.iter().copied().collect::<BTreeSet<_>>();
    let context = EnergyContext::new(&source.mesh, patch)?;
    Ok(energy_phase(
        &Certificate::internal(),
        &source.mesh,
        &guard_faces,
        &context,
    ))
}

pub fn solve_elastic_patch(
    source: &HierarchyLeafMesh,
    patch: ElasticPatch,
    limits: ElasticBlockLimits,
) -> ElasticBlockOutcome {
    solve_elastic_patch_impl(
        source,
        patch,
        limits,
        GeometryStartId::MaterializedSource,
        ElasticSolverMode::FiniteDifferenceElastic,
    )
}

pub fn solve_elastic_patch_with_start(
    source: &HierarchyLeafMesh,
    patch: ElasticPatch,
    limits: ElasticBlockLimits,
    start_id: GeometryStartId,
) -> ElasticBlockOutcome {
    solve_elastic_patch_impl(
        source,
        patch,
        limits,
        start_id,
        ElasticSolverMode::FiniteDifferenceElastic,
    )
}

pub fn solve_elastic_patch_with_margin_start(
    source: &HierarchyLeafMesh,
    patch: ElasticPatch,
    limits: ElasticBlockLimits,
    start_id: GeometryStartId,
) -> ElasticBlockOutcome {
    solve_elastic_patch_impl(
        source,
        patch,
        limits,
        start_id,
        ElasticSolverMode::MarginFiniteDifferenceLexicographic,
    )
}

pub fn solve_elastic_patch_with_active_trust_start(
    source: &HierarchyLeafMesh,
    patch: ElasticPatch,
    limits: ElasticBlockLimits,
    start_id: GeometryStartId,
) -> ElasticBlockOutcome {
    solve_elastic_patch_impl(
        source,
        patch,
        limits,
        start_id,
        ElasticSolverMode::ActiveTangentTrust,
    )
}

fn solve_elastic_patch_impl(
    source: &HierarchyLeafMesh,
    patch: ElasticPatch,
    limits: ElasticBlockLimits,
    start_id: GeometryStartId,
    solver_mode: ElasticSolverMode,
) -> ElasticBlockOutcome {
    if let Err(reason) = validate_patch(source, &patch) {
        return ElasticBlockOutcome::InvalidPatch { reason };
    }
    let certificate = Certificate::internal();
    let mut current = source.clone();
    if let Err(reason) = apply_geometry_start(&mut current.mesh, &patch, start_id) {
        return ElasticBlockOutcome::InvalidPatch { reason };
    }
    let input_positions = source.mesh.vertices().to_vec();
    if let Ok(geometry) = certificate.verify_geometry(&current.mesh) {
        return certified(current, patch, geometry, 0, 0.0, 0.0, &input_positions);
    }

    let guard_faces = patch.guard_faces.iter().copied().collect::<BTreeSet<_>>();
    let Some((initial_step, minimum_step)) = relocation_step_window(&current.mesh, &guard_faces)
    else {
        return ElasticBlockOutcome::InvalidPatch {
            reason: "transition guard has no positive finite edge length".into(),
        };
    };
    let context = match EnergyContext::new(&current.mesh, &patch) {
        Ok(context) => context,
        Err(reason) => return ElasticBlockOutcome::InvalidPatch { reason },
    };
    let phase = energy_phase(&certificate, &current.mesh, &guard_faces, &context);
    let Some(initial_energy) = elastic_energy(&current.mesh, &patch, phase, &context) else {
        return ElasticBlockOutcome::InvalidPatch {
            reason: "initial transition geometry has undefined elastic energy".into(),
        };
    };
    if limits.elastic_iterations == 0 {
        return ElasticBlockOutcome::SearchBudgetExhausted {
            elastic_iterations: 0,
            initial_energy,
            final_energy: initial_energy,
            final_phase: phase,
            reason: geometry_failure_reason(&certificate, &current.mesh),
            failed_guard_face: failed_guard_face(&certificate, &current.mesh, &patch),
            global_angle_degrees: angle_range(&current.mesh, current.mesh.active_triangle_slots()),
            guard_angle_degrees: angle_range(&current.mesh, guard_faces.iter().copied()),
            diagnostics: geometry_failure_diagnostics(&current.mesh, &patch, &context),
        };
    }

    let no_step = |mesh: &MeshState, iteration: usize, final_energy: f64| {
        let final_phase = energy_phase(&certificate, mesh, &guard_faces, &context);
        let reason = geometry_failure_reason(&certificate, mesh);
        let failed_guard_face = failed_guard_face(&certificate, mesh, &patch);
        if matches!(final_phase, ElasticBlockPhase::DelaunayVoronoiFeasibility)
            || geometry_failure_requires_different_topology(&certificate, mesh)
        {
            ElasticBlockOutcome::RequiresDifferentTopology {
                elastic_iterations: iteration,
                initial_energy,
                final_energy,
                final_phase,
                reason,
                failed_guard_face,
                global_angle_degrees: angle_range(mesh, mesh.active_triangle_slots()),
                guard_angle_degrees: angle_range(mesh, guard_faces.iter().copied()),
                diagnostics: geometry_failure_diagnostics(mesh, &patch, &context),
            }
        } else {
            ElasticBlockOutcome::ElasticNoImprovement {
                elastic_iterations: iteration,
                initial_energy,
                final_energy,
                final_phase,
                reason,
                failed_guard_face,
                global_angle_degrees: angle_range(mesh, mesh.active_triangle_slots()),
                guard_angle_degrees: angle_range(mesh, guard_faces.iter().copied()),
                diagnostics: geometry_failure_diagnostics(mesh, &patch, &context),
            }
        }
    };

    let mut energy = initial_energy;
    let mut trust_radius = initial_step;
    for iteration in 1..=limits.elastic_iterations {
        let phase = energy_phase(&certificate, &current.mesh, &guard_faces, &context);
        let Some(phase_energy) = elastic_energy(&current.mesh, &patch, phase, &context) else {
            return no_step(&current.mesh, iteration, energy);
        };
        energy = phase_energy;

        if solver_mode == ElasticSolverMode::ActiveTangentTrust
            && matches!(phase, ElasticBlockPhase::AngleFeasibility)
        {
            let before = current.mesh.clone();
            let Some(step_plan) =
                active_trust_angle_step(&before, &patch, &context, &guard_faces, trust_radius)
            else {
                return no_step(&current.mesh, iteration, energy);
            };
            let trust_update = apply_active_trust_step(
                &mut current.mesh,
                ActiveTrustStepContext {
                    before: &before,
                    step_plan: &step_plan,
                    patch: &patch,
                    energy_context: &context,
                    guard_faces: &guard_faces,
                    trust_radius,
                    maximum_radius: initial_step,
                },
            );
            trust_radius = trust_update.next_radius;
            if !trust_update.accepted {
                if trust_radius <= minimum_step {
                    return no_step(&current.mesh, iteration, energy);
                }
                continue;
            }
            energy = elastic_energy(&current.mesh, &patch, phase, &context).unwrap_or(phase_energy);
        } else {
            let Some(gradient) = (if solver_mode
                == ElasticSolverMode::MarginFiniteDifferenceLexicographic
                && matches!(phase, ElasticBlockPhase::AngleFeasibility)
            {
                finite_difference_angle_margin_gradient(&mut current.mesh, &patch, initial_step)
            } else {
                finite_difference_gradient(&mut current.mesh, &patch, phase, initial_step, &context)
            }) else {
                return no_step(&current.mesh, iteration, energy);
            };
            let maximum_norm = gradient
                .iter()
                .map(|(_, vector)| magnitude(*vector))
                .max_by(f64::total_cmp)
                .unwrap_or(0.0);
            if !maximum_norm.is_finite() || maximum_norm <= 1.0e-14 {
                return no_step(&current.mesh, iteration, energy);
            }

            let phase_minimum_step = if matches!(phase, ElasticBlockPhase::Untangle) {
                minimum_step * 1.0e-3
            } else {
                minimum_step
            };
            let mut step = initial_step;
            let accepted = loop {
                let scale = -step / maximum_norm;
                let Some(updates) = synchronous_updates(&current.mesh, &gradient, scale) else {
                    if step <= phase_minimum_step {
                        break None;
                    }
                    step = (step * 0.5).max(phase_minimum_step);
                    continue;
                };
                for &(site, _, point) in &updates {
                    current.mesh.move_vertex(site, point);
                }
                let candidate_energy = elastic_energy(&current.mesh, &patch, phase, &context);
                if let Some(candidate_energy) = candidate_energy {
                    let movement_norm = updates
                        .iter()
                        .map(|&(_, before, after)| arc_length_unit_sphere(before, after).abs())
                        .sum::<f64>();
                    let accepted = if matches!(phase, ElasticBlockPhase::Untangle) {
                        candidate_energy.is_finite() && candidate_energy < phase_energy
                    } else if solver_mode == ElasticSolverMode::MarginFiniteDifferenceLexicographic
                        && matches!(phase, ElasticBlockPhase::AngleFeasibility)
                    {
                        angle_phase_step_is_better(
                            mesh_before_updates(&current.mesh, &updates),
                            &current.mesh,
                            &patch,
                            &context,
                            &guard_faces,
                            movement_norm,
                        )
                    } else {
                        candidate_energy < phase_energy - 1.0e-12 * phase_energy.abs().max(1.0)
                    };
                    if accepted {
                        break Some(candidate_energy);
                    }
                }
                for &(site, point, _) in &updates {
                    current.mesh.move_vertex(site, point);
                }
                if step <= phase_minimum_step {
                    break None;
                }
                step = (step * 0.5).max(phase_minimum_step);
            };

            let Some(candidate_energy) = accepted else {
                return no_step(&current.mesh, iteration, energy);
            };
            energy = candidate_energy;
        }

        if certificate.geometry_region_passes(&current.mesh, &guard_faces) {
            if let Ok(geometry) = certificate.verify_geometry(&current.mesh) {
                return certified(
                    current,
                    patch,
                    geometry,
                    iteration,
                    initial_energy,
                    energy,
                    &input_positions,
                );
            }
        }
    }

    ElasticBlockOutcome::SearchBudgetExhausted {
        elastic_iterations: limits.elastic_iterations,
        initial_energy,
        final_energy: energy,
        final_phase: energy_phase(&certificate, &current.mesh, &guard_faces, &context),
        reason: geometry_failure_reason(&certificate, &current.mesh),
        failed_guard_face: failed_guard_face(&certificate, &current.mesh, &patch),
        global_angle_degrees: angle_range(&current.mesh, current.mesh.active_triangle_slots()),
        guard_angle_degrees: angle_range(&current.mesh, guard_faces.iter().copied()),
        diagnostics: geometry_failure_diagnostics(&current.mesh, &patch, &context),
    }
}

fn apply_geometry_start(
    mesh: &mut MeshState,
    patch: &ElasticPatch,
    start_id: GeometryStartId,
) -> Result<(), String> {
    match start_id {
        GeometryStartId::MaterializedSource => Ok(()),
        GeometryStartId::HierarchySpringEquilibrium => hierarchy_spring_start(mesh, patch),
        GeometryStartId::RingScaleInterpolation => ring_scale_start(mesh, patch),
        GeometryStartId::DegreeAngleEquilibrium => degree_angle_start(mesh, patch),
        GeometryStartId::SignedNormalPlus => signed_normal_start(mesh, patch, 1.0),
        GeometryStartId::SignedNormalMinus => signed_normal_start(mesh, patch, -1.0),
    }
}

fn hierarchy_spring_start(mesh: &mut MeshState, patch: &ElasticPatch) -> Result<(), String> {
    for _ in 0..8 {
        let mut updates = Vec::new();
        for &site in &patch.movable_compact_vertices {
            let Some([first, second]) = tangent_basis(mesh.vertices()[site]) else {
                continue;
            };
            let mut force = CartesianPoint::new(0.0, 0.0, 0.0);
            for &(left, right) in patch.target_field.target_edge_lengths.keys() {
                let other = if left == site {
                    right
                } else if right == site {
                    left
                } else {
                    continue;
                };
                let current = arc_length_unit_sphere(mesh.vertices()[site], mesh.vertices()[other]);
                let target = patch.target_field.target_edge_lengths[&(left, right)];
                if current <= 0.0 || target <= 0.0 || !current.is_finite() || !target.is_finite() {
                    continue;
                }
                let direction = tangent_log(mesh.vertices()[site], mesh.vertices()[other])?;
                force = add_points(force, scale_point(direction, 1.0 - target / current));
            }
            let tangent = add_points(
                scale_point(first, dot(force, first)),
                scale_point(second, dot(force, second)),
            );
            updates.push((
                site,
                exponential_map(mesh.vertices()[site], scale_point(tangent, 0.08)),
            ));
        }
        for (site, point) in updates {
            if let Some(point) = point {
                mesh.move_vertex(site, point);
            }
        }
    }
    Ok(())
}

fn ring_scale_start(mesh: &mut MeshState, patch: &ElasticPatch) -> Result<(), String> {
    let fixed = patch
        .fixed_compact_vertices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let guard_edges = guard_edges_for_faces(mesh, &patch.guard_faces);
    let fixed_scales = fixed
        .iter()
        .filter_map(|site| {
            patch
                .target_field
                .target_vertex_scales
                .get(site)
                .copied()
                .filter(|scale| scale.is_finite() && *scale > 0.0)
                .map(|scale| (*site, scale))
        })
        .collect::<Vec<_>>();
    if fixed_scales.len() < 2 {
        return Ok(());
    }
    let fine_scale = fixed_scales
        .iter()
        .map(|(_, scale)| *scale)
        .min_by(f64::total_cmp)
        .unwrap();
    let coarse_scale = fixed_scales
        .iter()
        .map(|(_, scale)| *scale)
        .max_by(f64::total_cmp)
        .unwrap();
    if fine_scale == coarse_scale {
        return Ok(());
    }
    let fine_fixed = fixed_scales
        .iter()
        .filter_map(|&(site, scale)| (scale == fine_scale).then_some(site))
        .collect::<BTreeSet<_>>();
    let coarse_fixed = fixed_scales
        .iter()
        .filter_map(|&(site, scale)| (scale == coarse_scale).then_some(site))
        .collect::<BTreeSet<_>>();
    let fine_dist = graph_distances(&guard_edges, &fine_fixed);
    let coarse_dist = graph_distances(&guard_edges, &coarse_fixed);
    let mut updates = Vec::new();
    for &site in &patch.movable_compact_vertices {
        let Some(&to_coarse) = coarse_dist.get(&site) else {
            continue;
        };
        let Some(&to_fine) = fine_dist.get(&site) else {
            continue;
        };
        let denominator = to_coarse + to_fine;
        if denominator == 0 {
            continue;
        }
        let s = to_coarse as f64 / denominator as f64;
        let target_scale = ((1.0 - s) * coarse_scale.ln() + s * fine_scale.ln()).exp();
        let coarse_endpoint = nearest_fixed_endpoint(site, &coarse_fixed, mesh)
            .ok_or("missing coarse ring endpoint")?;
        let fine_endpoint =
            nearest_fixed_endpoint(site, &fine_fixed, mesh).ok_or("missing fine ring endpoint")?;
        let interpolated = spherical_lerp(
            mesh.vertices()[coarse_endpoint],
            mesh.vertices()[fine_endpoint],
            s,
        )
        .ok_or("invalid ring interpolation endpoint")?;
        let current_scale = patch
            .target_field
            .target_vertex_scales
            .get(&site)
            .copied()
            .filter(|scale| scale.is_finite() && *scale > 0.0)
            .unwrap_or(target_scale);
        let blend = (target_scale / current_scale).ln().abs().clamp(0.05, 0.35);
        let next = spherical_lerp(mesh.vertices()[site], interpolated, blend)
            .ok_or("invalid ring interpolation update")?;
        updates.push((site, next));
    }
    for (site, point) in updates {
        mesh.move_vertex(site, point);
    }
    Ok(())
}

fn graph_distances(edges: &[(usize, usize)], seeds: &BTreeSet<usize>) -> BTreeMap<usize, usize> {
    let mut adjacency = BTreeMap::<usize, Vec<usize>>::new();
    for &(left, right) in edges {
        adjacency.entry(left).or_default().push(right);
        adjacency.entry(right).or_default().push(left);
    }
    let mut distances = BTreeMap::new();
    let mut frontier = seeds.iter().copied().collect::<Vec<_>>();
    for seed in &frontier {
        distances.insert(*seed, 0usize);
    }
    let mut index = 0;
    while index < frontier.len() {
        let site = frontier[index];
        index += 1;
        let distance = distances[&site];
        if let Some(neighbours) = adjacency.get(&site) {
            for &next in neighbours {
                if let std::collections::btree_map::Entry::Vacant(entry) = distances.entry(next) {
                    entry.insert(distance + 1);
                    frontier.push(next);
                }
            }
        }
    }
    distances
}

fn nearest_fixed_endpoint(
    site: usize,
    candidates: &BTreeSet<usize>,
    mesh: &MeshState,
) -> Option<usize> {
    candidates.iter().copied().min_by(|&left, &right| {
        arc_length_unit_sphere(mesh.vertices()[site], mesh.vertices()[left])
            .total_cmp(&arc_length_unit_sphere(
                mesh.vertices()[site],
                mesh.vertices()[right],
            ))
            .then_with(|| left.cmp(&right))
    })
}

fn spherical_lerp(left: CartesianPoint, right: CartesianPoint, t: f64) -> Option<CartesianPoint> {
    let left = normalized_point(left)?;
    let right = normalized_point(right)?;
    let omega = dot(left, right).clamp(-1.0, 1.0).acos();
    if omega.abs() < 1.0e-14 {
        return Some(left);
    }
    let sin_omega = omega.sin();
    if sin_omega.abs() < 1.0e-14 {
        return normalized_point(add_points(
            scale_point(left, 1.0 - t),
            scale_point(right, t),
        ));
    }
    normalized_point(add_points(
        scale_point(left, ((1.0 - t) * omega).sin() / sin_omega),
        scale_point(right, (t * omega).sin() / sin_omega),
    ))
}

fn degree_angle_start(mesh: &mut MeshState, patch: &ElasticPatch) -> Result<(), String> {
    for _ in 0..4 {
        let Some(gradient) = finite_difference_degree_angle_gradient(mesh, patch, 0.02) else {
            break;
        };
        let maximum_norm = gradient
            .iter()
            .map(|(_, vector)| magnitude(*vector))
            .max_by(f64::total_cmp)
            .unwrap_or(0.0);
        if maximum_norm <= 1.0e-14 || !maximum_norm.is_finite() {
            break;
        }
        let Some(updates) = synchronous_updates(mesh, &gradient, -0.02 / maximum_norm) else {
            break;
        };
        for (site, _, point) in updates {
            mesh.move_vertex(site, point);
        }
    }
    Ok(())
}

fn finite_difference_degree_angle_gradient(
    mesh: &mut MeshState,
    patch: &ElasticPatch,
    initial_step: f64,
) -> Option<Vec<(usize, CartesianPoint)>> {
    let epsilon = (initial_step * 1.0e-3).clamp(1.0e-7, 1.0e-5);
    let mut gradient = Vec::with_capacity(patch.movable_compact_vertices.len());
    for &site in &patch.movable_compact_vertices {
        let point = mesh.vertices()[site];
        let [first, second] = tangent_basis(point)?;
        let base_loss = degree_angle_loss(mesh, patch)?;
        let mut derivative = |direction: CartesianPoint| {
            let plus = exponential_map(point, scale_point(direction, epsilon))?;
            let minus = exponential_map(point, scale_point(direction, -epsilon))?;
            mesh.move_vertex(site, plus);
            let plus_loss = degree_angle_loss(mesh, patch);
            mesh.move_vertex(site, minus);
            let minus_loss = degree_angle_loss(mesh, patch);
            mesh.move_vertex(site, point);
            match (plus_loss, minus_loss) {
                (Some(plus), Some(minus)) => Some((plus - minus) / (2.0 * epsilon)),
                (Some(plus), None) => Some((plus - base_loss) / epsilon),
                (None, Some(minus)) => Some((base_loss - minus) / epsilon),
                (None, None) => None,
            }
        };
        let d_first = derivative(first)?;
        let d_second = derivative(second)?;
        gradient.push((
            site,
            add_points(scale_point(first, d_first), scale_point(second, d_second)),
        ));
    }
    Some(gradient)
}

fn degree_angle_loss(mesh: &MeshState, patch: &ElasticPatch) -> Option<f64> {
    let degrees = vertex_degrees(mesh);
    let mut loss = 0.0;
    let mut count = 0usize;
    for &face in &patch.guard_faces {
        let triangle = mesh.triangles()[face];
        let determinant = dot(
            mesh.vertices()[triangle[0]],
            cross(mesh.vertices()[triangle[1]], mesh.vertices()[triangle[2]]),
        );
        if determinant.is_finite() {
            loss += 1000.0 * (-determinant).max(0.0).powi(2);
        }
        for corner in 0..3 {
            let site = triangle[corner];
            let angle = corner_angle_degrees(mesh, triangle, corner)?;
            let target = patch
                .target_field
                .target_angles
                .get(&site)
                .copied()
                .unwrap_or_else(|| std::f64::consts::TAU / degrees[site] as f64)
                .to_degrees();
            loss += (angle - target).powi(2);
            count += 1;
        }
    }
    (count > 0 && loss.is_finite()).then_some(loss)
}

fn corner_angle_degrees(mesh: &MeshState, triangle: [usize; 3], corner: usize) -> Option<f64> {
    let center = normalized_point(mesh.vertices()[triangle[corner]])?;
    let left = normalized_point(mesh.vertices()[triangle[(corner + 1) % 3]])?;
    let right = normalized_point(mesh.vertices()[triangle[(corner + 2) % 3]])?;
    let left_tangent = normalized_point(subtract_points(
        left,
        scale_point(center, dot(center, left)),
    ))?;
    let right_tangent = normalized_point(subtract_points(
        right,
        scale_point(center, dot(center, right)),
    ))?;
    Some(
        dot(left_tangent, right_tangent)
            .clamp(-1.0, 1.0)
            .acos()
            .to_degrees(),
    )
}

fn signed_normal_start(
    mesh: &mut MeshState,
    patch: &ElasticPatch,
    sign: f64,
) -> Result<(), String> {
    let mut updates = Vec::new();
    for &site in &patch.movable_compact_vertices {
        let mut normal = CartesianPoint::new(0.0, 0.0, 0.0);
        for &face in &patch.guard_faces {
            let triangle = mesh.triangles()[face];
            if !triangle.contains(&site) {
                continue;
            }
            let [a, b, c] = triangle.map(|v| mesh.vertices()[v]);
            normal = add_points(normal, cross(subtract_points(b, a), subtract_points(c, a)));
        }
        let radial = normalized_point(mesh.vertices()[site]).ok_or("invalid start vertex")?;
        let tangent = subtract_points(normal, scale_point(radial, dot(radial, normal)));
        updates.push((
            site,
            exponential_map(mesh.vertices()[site], scale_point(tangent, 0.02 * sign)),
        ));
    }
    for (site, point) in updates {
        if let Some(point) = point {
            mesh.move_vertex(site, point);
        }
    }
    Ok(())
}

fn tangent_log(from: CartesianPoint, to: CartesianPoint) -> Result<CartesianPoint, String> {
    let from = normalized_point(from).ok_or("invalid tangent-log source")?;
    let to = normalized_point(to).ok_or("invalid tangent-log target")?;
    let cosine = dot(from, to).clamp(-1.0, 1.0);
    let angle = cosine.acos();
    if angle == 0.0 {
        return Ok(CartesianPoint::new(0.0, 0.0, 0.0));
    }
    let tangent = subtract_points(to, scale_point(from, cosine));
    let unit = normalized_point(tangent).ok_or("invalid tangent-log direction")?;
    Ok(scale_point(unit, angle))
}

fn guard_edges_for_faces(mesh: &MeshState, guard_faces: &[usize]) -> Vec<(usize, usize)> {
    let mut edges = BTreeSet::new();
    for &face in guard_faces {
        for edge in local_triangle_edges(mesh.triangles()[face]) {
            edges.insert(edge);
        }
    }
    edges.into_iter().collect()
}

fn mesh_before_updates(
    mesh: &MeshState,
    updates: &[(usize, CartesianPoint, CartesianPoint)],
) -> MeshState {
    let mut before = mesh.clone();
    for &(site, point, _) in updates {
        before.move_vertex(site, point);
    }
    before
}

fn angle_phase_step_is_better(
    before: MeshState,
    after: &MeshState,
    patch: &ElasticPatch,
    context: &EnergyContext,
    guard_faces: &BTreeSet<usize>,
    movement_norm: f64,
) -> bool {
    let guard_edges = &context.guard_edges;
    let before_key = angle_acceptance_key(&before, patch, context, guard_faces, guard_edges, 0.0);
    let after_key = angle_acceptance_key(
        after,
        patch,
        context,
        guard_faces,
        guard_edges,
        movement_norm,
    );
    match (before_key, after_key) {
        (Some(before), Some(after)) => after.is_better_than(&before),
        _ => false,
    }
}

#[derive(Clone, Debug)]
struct AngleAcceptanceKey {
    negative_orientation_count: usize,
    crossing_count: usize,
    signed_margin_deg: f64,
    sum_worst_k_violation: f64,
    delaunay_violations: usize,
    invalid_voronoi_cells: usize,
    movement_norm: f64,
}

impl AngleAcceptanceKey {
    fn is_better_than(&self, other: &Self) -> bool {
        if self.negative_orientation_count != other.negative_orientation_count {
            return self.negative_orientation_count < other.negative_orientation_count;
        }
        if self.crossing_count != other.crossing_count {
            return self.crossing_count < other.crossing_count;
        }
        if self.signed_margin_deg + 1.0e-12 < other.signed_margin_deg {
            return false;
        }
        if self.signed_margin_deg > other.signed_margin_deg + 1.0e-12 {
            return true;
        }
        if self
            .sum_worst_k_violation
            .total_cmp(&other.sum_worst_k_violation)
            != std::cmp::Ordering::Equal
        {
            return self.sum_worst_k_violation < other.sum_worst_k_violation;
        }
        if self.delaunay_violations != other.delaunay_violations {
            return self.delaunay_violations < other.delaunay_violations;
        }
        if self.invalid_voronoi_cells != other.invalid_voronoi_cells {
            return self.invalid_voronoi_cells < other.invalid_voronoi_cells;
        }
        self.movement_norm < other.movement_norm
    }
}

fn angle_acceptance_key(
    mesh: &MeshState,
    _patch: &ElasticPatch,
    context: &EnergyContext,
    _guard_faces: &BTreeSet<usize>,
    guard_edges: &[(usize, usize)],
    movement_norm: f64,
) -> Option<AngleAcceptanceKey> {
    let objective = angle_margin_objective(mesh, mesh.active_triangle_slots())?;
    let negative_orientation_count = mesh
        .active_triangle_slots()
        .filter(|&face| {
            let [a, b, c] = mesh.triangles()[face].map(|site| mesh.vertices()[site]);
            dot(a, cross(b, c)) <= 0.0
        })
        .count();
    let mut crossing_count = 0;
    for (i, &(a, b)) in guard_edges.iter().enumerate() {
        for &(c, d) in &guard_edges[i + 1..] {
            if a == c || a == d || b == c || b == d {
                continue;
            }
            if minor_arc_crossing_strength(
                mesh.vertices()[a],
                mesh.vertices()[b],
                mesh.vertices()[c],
                mesh.vertices()[d],
            ) > 1.0e-14
            {
                crossing_count += 1;
            }
        }
    }
    let sum_worst_k_violation = objective
        .worst_constraints
        .iter()
        .map(|constraint| (-constraint.signed_margin_deg).max(0.0))
        .sum();
    Some(AngleAcceptanceKey {
        negative_orientation_count,
        crossing_count,
        signed_margin_deg: objective.signed_margin_deg,
        sum_worst_k_violation,
        delaunay_violations: delaunay_violation_count(mesh, &context.dual_pairs),
        invalid_voronoi_cells: invalid_voronoi_count(mesh, &context.guard_seeds),
        movement_norm,
    })
}

fn delaunay_violation_count(mesh: &MeshState, dual_pairs: &[DualPair]) -> usize {
    dual_pairs
        .iter()
        .filter(|pair| {
            let triangle = mesh.triangles()[pair.face];
            let points = triangle.map(|site| normalized_point(mesh.vertices()[site]));
            let [Some(a), Some(b), Some(c)] = points else {
                return true;
            };
            let Some(d) = normalized_point(mesh.vertices()[pair.opposite]) else {
                return true;
            };
            matches!(in_circle_on_sphere(a, b, c, d), Ok(Sign::Positive) | Err(_))
        })
        .count()
}

fn invalid_voronoi_count(mesh: &MeshState, guard_seeds: &[(usize, usize)]) -> usize {
    guard_seeds
        .iter()
        .filter(|&&(site, seed)| {
            mesh.voronoi_cell_from(site, seed)
                .ok()
                .is_none_or(|cell| !voronoi_cell_is_convex_and_contains_site(mesh, &cell))
        })
        .count()
}

pub fn angle_margin_objective(
    mesh: &MeshState,
    faces: impl IntoIterator<Item = usize>,
) -> Option<AngleMarginObjective> {
    let mut worst_constraints = angle_constraints(mesh, faces)?;
    let signed_margin_deg = worst_constraints[0].signed_margin_deg;
    worst_constraints.truncate(8);
    Some(AngleMarginObjective {
        signed_margin_deg,
        worst_constraints,
    })
}

fn angle_constraints(
    mesh: &MeshState,
    faces: impl IntoIterator<Item = usize>,
) -> Option<Vec<AngleConstraintKey>> {
    let mut constraints = Vec::new();
    for face in faces {
        if !mesh.is_triangle_live(face) {
            continue;
        }
        let angles =
            spherical_triangle_angles(mesh.triangles()[face].map(|site| mesh.vertices()[site]))?;
        for (corner, angle_deg) in angles.into_iter().enumerate() {
            let signed_margin_deg = (angle_deg - 40.2).min(79.8 - angle_deg);
            constraints.push(AngleConstraintKey {
                face,
                corner,
                angle_deg,
                signed_margin_deg,
            });
        }
    }
    if constraints.is_empty() {
        return None;
    }
    constraints.sort_by(|a, b| {
        a.signed_margin_deg
            .total_cmp(&b.signed_margin_deg)
            .then_with(|| a.face.cmp(&b.face))
            .then_with(|| a.corner.cmp(&b.corner))
    });
    Some(constraints)
}

fn angle_range(mesh: &MeshState, faces: impl IntoIterator<Item = usize>) -> Option<(f64, f64)> {
    let mut min_angle = f64::INFINITY;
    let mut max_angle = f64::NEG_INFINITY;
    for face in faces {
        if !mesh.is_triangle_live(face) {
            continue;
        }
        let angles =
            spherical_triangle_angles(mesh.triangles()[face].map(|site| mesh.vertices()[site]))?;
        for angle in angles {
            min_angle = min_angle.min(angle);
            max_angle = max_angle.max(angle);
        }
    }
    min_angle.is_finite().then_some((min_angle, max_angle))
}

fn geometry_failure_diagnostics(
    mesh: &MeshState,
    patch: &ElasticPatch,
    _context: &EnergyContext,
) -> Option<GeometryFailureDiagnostics> {
    let movement_distribution = movement_distribution(mesh, patch)?;
    let worst_triangle_guard_distance = worst_triangle_guard_distance(mesh, patch);
    let active_boundary_constraint_ratio = active_boundary_constraint_ratio(mesh, patch);
    Some(GeometryFailureDiagnostics {
        movement_distribution,
        worst_triangle_guard_distance,
        active_boundary_constraint_ratio,
    })
}

fn movement_distribution(mesh: &MeshState, patch: &ElasticPatch) -> Option<MovementDistribution> {
    let mut distances = patch
        .movable_compact_vertices
        .iter()
        .copied()
        .filter_map(|site| {
            patch
                .reference_positions
                .get(site)
                .map(|&reference| arc_length_unit_sphere(reference, mesh.vertices()[site]).abs())
        })
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    if distances.is_empty() {
        return None;
    }
    distances.sort_by(f64::total_cmp);
    let sum = distances.iter().sum::<f64>();
    Some(MovementDistribution {
        count: distances.len(),
        min: distances[0],
        p50: percentile_sorted(&distances, 0.5),
        p90: percentile_sorted(&distances, 0.9),
        max: *distances.last()?,
        sum,
    })
}

fn percentile_sorted(values: &[f64], fraction: f64) -> f64 {
    let index = ((values.len() - 1) as f64 * fraction).round() as usize;
    values[index]
}

fn worst_triangle_guard_distance(mesh: &MeshState, patch: &ElasticPatch) -> Option<usize> {
    let objective = angle_margin_objective(mesh, mesh.active_triangle_slots())?;
    let worst_face = objective.worst_constraints.first()?.face;
    let fixed = patch
        .fixed_compact_vertices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let fixed_guard_faces = patch
        .guard_faces
        .iter()
        .copied()
        .filter(|&face| {
            mesh.triangles()[face]
                .iter()
                .any(|site| fixed.contains(site))
        })
        .collect::<BTreeSet<_>>();
    face_graph_distance_to_any(mesh, worst_face, &fixed_guard_faces)
}

fn active_boundary_constraint_ratio(
    mesh: &MeshState,
    patch: &ElasticPatch,
) -> Option<ActiveBoundaryConstraintRatio> {
    let active = active_angle_constraints(mesh, mesh.active_triangle_slots())?;
    let fixed = patch
        .fixed_compact_vertices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let numerator = active
        .iter()
        .filter(|constraint| {
            mesh.triangles()[constraint.face]
                .iter()
                .any(|site| fixed.contains(site))
        })
        .count();
    let denominator = active.len();
    (denominator > 0).then_some(ActiveBoundaryConstraintRatio {
        numerator,
        denominator,
        ratio: numerator as f64 / denominator as f64,
    })
}

fn face_graph_distance_to_any(
    mesh: &MeshState,
    start_face: usize,
    targets: &BTreeSet<usize>,
) -> Option<usize> {
    if targets.contains(&start_face) {
        return Some(0);
    }
    let mut edge_to_faces = BTreeMap::<(usize, usize), Vec<usize>>::new();
    for face in mesh.active_triangle_slots() {
        for edge in local_triangle_edges(mesh.triangles()[face]) {
            edge_to_faces.entry(edge).or_default().push(face);
        }
    }
    let mut visited = BTreeSet::from([start_face]);
    let mut frontier = BTreeSet::from([start_face]);
    for distance in 1..=mesh.triangles().len() {
        let mut next = BTreeSet::new();
        for face in &frontier {
            for edge in local_triangle_edges(mesh.triangles()[*face]) {
                if let Some(neighbours) = edge_to_faces.get(&edge) {
                    for &neighbour in neighbours {
                        if visited.insert(neighbour) {
                            if targets.contains(&neighbour) {
                                return Some(distance);
                            }
                            next.insert(neighbour);
                        }
                    }
                }
            }
        }
        if next.is_empty() {
            return None;
        }
        frontier = next;
    }
    None
}

fn failed_guard_face(
    certificate: &Certificate,
    mesh: &MeshState,
    patch: &ElasticPatch,
) -> Option<usize> {
    patch.guard_faces.iter().copied().find(|face| {
        certificate
            .verify_geometry_region(mesh, &BTreeSet::from([*face]))
            .is_err()
    })
}

fn geometry_failure_reason(certificate: &Certificate, mesh: &MeshState) -> String {
    match certificate.verify_geometry(mesh) {
        Ok(_) => "geometry passed but the elastic objective had no descent step".into(),
        Err(error) => format!("{error:?}"),
    }
}

fn geometry_failure_requires_different_topology(
    certificate: &Certificate,
    mesh: &MeshState,
) -> bool {
    matches!(
        certificate.verify_geometry(mesh),
        Err(CertificateError::Delaunay(_) | CertificateError::Dual(_))
    )
}

fn source_set_to_compact(
    source_to_compact: &BTreeMap<usize, usize>,
    sources: &BTreeSet<usize>,
    mesh: &MeshState,
    label: &str,
) -> Result<Vec<usize>, String> {
    sources
        .iter()
        .map(|source| {
            source_to_compact
                .get(source)
                .copied()
                .filter(|&compact| mesh.is_vertex_live(compact))
                .ok_or_else(|| format!("{label} {source} is absent from the compact mesh"))
        })
        .collect()
}

fn incident_faces(mesh: &MeshState, vertices: &BTreeSet<usize>) -> BTreeSet<usize> {
    mesh.active_triangle_slots()
        .filter(|&face| {
            mesh.triangles()[face]
                .iter()
                .any(|site| vertices.contains(site))
        })
        .collect()
}

fn expand_movable_domain(
    mesh: &MeshState,
    source_slots: &[Option<usize>],
    base_movable: &BTreeSet<usize>,
    permanent_fixed: &BTreeSet<usize>,
    domain_id: GeometryDomainId,
) -> BTreeSet<usize> {
    let mut movable = base_movable
        .iter()
        .copied()
        .filter(|site| !permanent_fixed.contains(site))
        .filter(|&site| source_slots.get(site).and_then(|slot| *slot).is_some())
        .collect::<BTreeSet<_>>();
    for _ in 0..domain_id.expansion_rings() {
        let mut next = movable.clone();
        for edge in mesh
            .active_triangle_slots()
            .flat_map(|face| local_triangle_edges(mesh.triangles()[face]))
        {
            for (left, right) in [(edge.0, edge.1), (edge.1, edge.0)] {
                if movable.contains(&left)
                    && !permanent_fixed.contains(&right)
                    && source_slots.get(right).and_then(|slot| *slot).is_some()
                {
                    next.insert(right);
                }
            }
        }
        if next == movable {
            break;
        }
        movable = next;
    }
    movable
}

fn validate_patch(mesh: &HierarchyLeafMesh, patch: &ElasticPatch) -> Result<(), String> {
    if patch.reference_positions.len() != mesh.mesh.vertices().len() {
        return Err("elastic reference positions do not match compact vertices".into());
    }
    if patch.movable_compact_vertices.is_empty() {
        return Err("elastic patch has no movable transition vertex".into());
    }
    let movable = patch
        .movable_compact_vertices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if movable.len() != patch.movable_compact_vertices.len()
        || movable.iter().any(|&site| !mesh.mesh.is_vertex_live(site))
    {
        return Err("elastic patch has duplicate or inactive movable vertices".into());
    }
    let fixed = patch
        .fixed_compact_vertices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if !movable.is_disjoint(&fixed) {
        return Err("elastic fixed and movable vertices overlap".into());
    }
    if fixed.len() != patch.fixed_compact_vertices.len()
        || fixed.iter().any(|&site| !mesh.mesh.is_vertex_live(site))
    {
        return Err("elastic patch has duplicate or inactive fixed vertices".into());
    }
    let guard = patch.guard_faces.iter().copied().collect::<BTreeSet<_>>();
    if guard.len() != patch.guard_faces.len()
        || guard.iter().any(|&face| !mesh.mesh.is_triangle_live(face))
    {
        return Err("elastic patch has duplicate or inactive guard faces".into());
    }
    if movable.iter().any(|site| {
        !guard
            .iter()
            .any(|&face| mesh.mesh.triangles()[face].contains(site))
    }) {
        return Err("elastic movable vertex lies outside the guard faces".into());
    }
    if mesh.mesh.active_triangle_slots().any(|face| {
        mesh.mesh.triangles()[face]
            .iter()
            .any(|site| movable.contains(site))
            && !guard.contains(&face)
    }) {
        return Err("elastic guard omits a face incident to a movable vertex".into());
    }
    let expected_fixed = guard
        .iter()
        .flat_map(|&face| mesh.mesh.triangles()[face])
        .filter(|site| !movable.contains(site))
        .collect::<BTreeSet<_>>();
    if fixed != expected_fixed {
        return Err("elastic fixed vertices do not match the guarded block boundary".into());
    }
    Ok(())
}

fn certified(
    mesh: HierarchyLeafMesh,
    patch: ElasticPatch,
    geometry: GeometryCertificateReport,
    elastic_iterations: usize,
    initial_energy: f64,
    final_energy: f64,
    input_positions: &[CartesianPoint],
) -> ElasticBlockOutcome {
    let moved_compact_vertices = patch
        .movable_compact_vertices
        .iter()
        .copied()
        .filter(|&site| mesh.mesh.vertices()[site] != input_positions[site])
        .collect();
    ElasticBlockOutcome::Certified(Box::new(ElasticBlockTrial {
        report: ElasticBlockReport {
            component_id: patch.topology.component_id,
            topology_id: patch.topology.topology_id,
            elastic_iterations,
            initial_energy,
            final_energy,
            moved_compact_vertices,
        },
        mesh,
        patch,
        geometry,
    }))
}

fn energy_phase(
    certificate: &Certificate,
    mesh: &MeshState,
    guard_faces: &BTreeSet<usize>,
    context: &EnergyContext,
) -> ElasticBlockPhase {
    if !all_faces_positive(mesh, guard_faces)
        || edge_crossing_penalty(mesh, &context.guard_edges) > 1.0e-18
    {
        return ElasticBlockPhase::Untangle;
    }
    if certificate.geometry_penalty_in(mesh, guard_faces) != Some(0.0) {
        return ElasticBlockPhase::AngleFeasibility;
    }
    if !dual_energy(mesh, context, false).is_some_and(|dual| dual.hard_feasible) {
        return ElasticBlockPhase::DelaunayVoronoiFeasibility;
    }
    ElasticBlockPhase::Interior
}

impl EnergyContext {
    fn new(mesh: &MeshState, patch: &ElasticPatch) -> Result<Self, String> {
        let mut guard_seeds = BTreeMap::new();
        let mut guard_edges = BTreeSet::new();
        for &face in &patch.guard_faces {
            let triangle = mesh.triangles()[face];
            for site in triangle {
                guard_seeds.entry(site).or_insert(face);
            }
            for edge in [
                (triangle[0], triangle[1]),
                (triangle[1], triangle[2]),
                (triangle[2], triangle[0]),
            ] {
                guard_edges.insert((edge.0.min(edge.1), edge.0.max(edge.1)));
            }
        }
        let reference =
            MeshState::from_parts(patch.reference_positions.clone(), mesh.triangles().to_vec())
                .ok();
        let mut reference_dual_areas = BTreeMap::new();
        for (&site, &seed) in &guard_seeds {
            let area = reference.as_ref().and_then(|reference| {
                reference
                    .voronoi_cell_from(site, seed)
                    .ok()
                    .and_then(|cell| cell.area_on_unit_sphere())
                    .filter(|area| area.is_finite() && *area > 0.0)
            });
            let area = patch
                .target_field
                .target_cell_areas
                .get(&site)
                .copied()
                .filter(|area| {
                    target_mode_uses_area(patch.target_mode) && area.is_finite() && *area > 0.0
                })
                .or(area);
            reference_dual_areas.insert(site, area);
        }
        let guard_faces = patch.guard_faces.clone();
        let guard_edges = guard_edges.into_iter().collect::<Vec<_>>();
        let guard_seeds = guard_seeds.into_iter().collect::<Vec<_>>();
        let dual_pairs = collect_dual_pairs(mesh, &guard_faces)?;
        let derivatives = patch
            .movable_compact_vertices
            .iter()
            .copied()
            .map(|site| {
                let faces = guard_faces
                    .iter()
                    .copied()
                    .filter(|&face| mesh.triangles()[face].contains(&site))
                    .collect::<Vec<_>>();
                let affected_sites = faces
                    .iter()
                    .flat_map(|&face| mesh.triangles()[face])
                    .collect::<BTreeSet<_>>();
                let derivative = DerivativeContext {
                    guard_edges: guard_edges
                        .iter()
                        .copied()
                        .filter(|&(left, right)| left == site || right == site)
                        .collect(),
                    guard_faces: faces,
                    guard_seeds: guard_seeds
                        .iter()
                        .copied()
                        .filter(|(candidate, _)| affected_sites.contains(candidate))
                        .collect(),
                    dual_pairs: dual_pairs
                        .iter()
                        .copied()
                        .filter(|pair| {
                            pair.opposite == site || mesh.triangles()[pair.face].contains(&site)
                        })
                        .collect(),
                };
                (site, derivative)
            })
            .collect();
        Ok(Self {
            degrees: vertex_degrees(mesh),
            guard_edges,
            guard_faces,
            guard_seeds,
            dual_pairs,
            derivatives,
            reference_dual_areas,
        })
    }
}

fn collect_dual_pairs(mesh: &MeshState, guard_faces: &[usize]) -> Result<Vec<DualPair>, String> {
    let mut seen = BTreeSet::new();
    let mut pairs = Vec::new();
    for &face in guard_faces {
        let triangle = mesh.triangles()[face];
        for corner in 0..3 {
            let other = mesh.neighbours()[face][corner];
            if other == 0 || !mesh.is_triangle_live(other) {
                return Err("elastic guard touches an open or inactive neighbour".into());
            }
            if !seen.insert((face.min(other), face.max(other))) {
                continue;
            }
            let edge = [triangle[(corner + 1) % 3], triangle[(corner + 2) % 3]];
            let opposite = mesh.triangles()[other]
                .iter()
                .copied()
                .find(|site| !edge.contains(site))
                .ok_or_else(|| "elastic guard neighbour has no opposite vertex".to_string())?;
            pairs.push(DualPair { face, opposite });
        }
    }
    Ok(pairs)
}

fn vertex_degrees(mesh: &MeshState) -> Vec<usize> {
    let mut degrees = vec![0usize; mesh.vertices().len()];
    for face in mesh.active_triangle_slots() {
        for site in mesh.triangles()[face] {
            degrees[site] += 1;
        }
    }
    degrees
}

fn elastic_energy(
    mesh: &MeshState,
    patch: &ElasticPatch,
    phase: ElasticBlockPhase,
    context: &EnergyContext,
) -> Option<f64> {
    elastic_energy_in(
        mesh,
        patch,
        phase,
        context,
        &context.guard_faces,
        &context.guard_edges,
        &context.guard_seeds,
        &context.dual_pairs,
    )
}

#[allow(clippy::too_many_arguments)]
fn elastic_energy_in(
    mesh: &MeshState,
    patch: &ElasticPatch,
    phase: ElasticBlockPhase,
    context: &EnergyContext,
    guard_faces: &[usize],
    guard_edges: &[(usize, usize)],
    guard_seeds: &[(usize, usize)],
    dual_pairs: &[DualPair],
) -> Option<f64> {
    let mut energy = 0.0;
    let minimum_angle = (40.2 + GEOMETRY_INTERIOR_MARGIN_DEGREES).to_radians();
    let maximum_angle = (79.8 - GEOMETRY_INTERIOR_MARGIN_DEGREES).to_radians();
    for &face in guard_faces {
        if !mesh.is_triangle_live(face) {
            return None;
        }
        let triangle = mesh.triangles()[face];
        let points = triangle.map(|site| mesh.vertices()[site]);
        let [Some(a), Some(b), Some(c)] = points.map(normalized_point) else {
            return None;
        };
        let determinant = dot(a, cross(b, c));
        if !determinant.is_finite() {
            return None;
        }
        if matches!(phase, ElasticBlockPhase::Untangle) {
            energy += 1_000_000.0 * (1.0e-10 - determinant).max(0.0).powi(2);
            continue;
        }
        if determinant <= 0.0 {
            return None;
        }
        let angles = spherical_triangle_angles(points)?.map(f64::to_radians);
        for corner in 0..3 {
            let angle = angles[corner];
            let site = triangle[corner];
            let target = patch
                .target_field
                .target_angles
                .get(&site)
                .copied()
                .unwrap_or(std::f64::consts::TAU / context.degrees[site] as f64);
            let below = (minimum_angle - angle).max(0.0);
            let above = (angle - maximum_angle).max(0.0);
            energy += 100.0 * (below * below + above * above);
            match phase {
                ElasticBlockPhase::Untangle | ElasticBlockPhase::DelaunayVoronoiFeasibility => {}
                ElasticBlockPhase::AngleFeasibility => {
                    if patch.target_mode.uses_hierarchy_area_degree() {
                        energy += 0.001 * (angle - target).powi(2);
                    }
                }
                ElasticBlockPhase::Interior => {
                    let lower = angle - minimum_angle;
                    let upper = maximum_angle - angle;
                    if lower <= 0.0 || upper <= 0.0 {
                        return None;
                    }
                    energy += 0.2 * (angle - target).powi(2) - 0.001 * (lower.ln() + upper.ln());
                }
            }
        }
        if matches!(phase, ElasticBlockPhase::Interior) {
            energy -= 0.0001 * determinant.ln();
        }
    }

    let edge_weight = match phase {
        ElasticBlockPhase::Untangle
        | ElasticBlockPhase::AngleFeasibility
        | ElasticBlockPhase::DelaunayVoronoiFeasibility => 0.001,
        ElasticBlockPhase::Interior => 0.01,
    };
    for &(left, right) in guard_edges {
        let length = arc_length_unit_sphere(mesh.vertices()[left], mesh.vertices()[right]);
        let edge = (left.min(right), left.max(right));
        let reference = patch
            .target_field
            .target_edge_lengths
            .get(&edge)
            .copied()
            .unwrap_or_else(|| {
                arc_length_unit_sphere(
                    patch.reference_positions[left],
                    patch.reference_positions[right],
                )
            });
        if length <= 0.0 || reference <= 0.0 || !length.is_finite() || !reference.is_finite() {
            return None;
        }
        energy += edge_weight * (length / reference).ln().powi(2);
    }
    if matches!(phase, ElasticBlockPhase::Untangle) {
        energy += 10_000.0 * edge_crossing_penalty(mesh, guard_edges);
        return energy.is_finite().then_some(energy);
    }
    if matches!(phase, ElasticBlockPhase::AngleFeasibility) {
        return energy.is_finite().then_some(energy);
    }

    let include_soft_dual = matches!(phase, ElasticBlockPhase::Interior);
    let dual = dual_energy_in(mesh, context, guard_seeds, dual_pairs, include_soft_dual)?;
    energy += 1_000.0 * dual.violation + 0.02 * dual.center;
    energy += 0.02 * dual.area;
    energy.is_finite().then_some(energy)
}

fn all_faces_positive(mesh: &MeshState, faces: &BTreeSet<usize>) -> bool {
    faces.iter().copied().all(|face| {
        mesh.is_triangle_live(face)
            && orientation_on_sphere(
                mesh.vertices()[mesh.triangles()[face][0]],
                mesh.vertices()[mesh.triangles()[face][1]],
                mesh.vertices()[mesh.triangles()[face][2]],
            ) == Ok(Sign::Positive)
    })
}

fn edge_crossing_penalty(mesh: &MeshState, edges: &[(usize, usize)]) -> f64 {
    let mut penalty = 0.0;
    for (i, &(a, b)) in edges.iter().enumerate() {
        for &(c, d) in &edges[i + 1..] {
            if a == c || a == d || b == c || b == d {
                continue;
            }
            penalty += minor_arc_crossing_strength(
                mesh.vertices()[a],
                mesh.vertices()[b],
                mesh.vertices()[c],
                mesh.vertices()[d],
            );
        }
    }
    penalty
}

fn dual_energy(
    mesh: &MeshState,
    context: &EnergyContext,
    include_soft: bool,
) -> Option<DualEnergy> {
    dual_energy_in(
        mesh,
        context,
        &context.guard_seeds,
        &context.dual_pairs,
        include_soft,
    )
}

fn dual_energy_in(
    mesh: &MeshState,
    context: &EnergyContext,
    guard_seeds: &[(usize, usize)],
    dual_pairs: &[DualPair],
    include_soft: bool,
) -> Option<DualEnergy> {
    let mut hard_feasible = true;
    let mut violation = 0.0;
    let mut center = 0.0;
    let mut area = 0.0;

    for &(site, seed) in guard_seeds {
        let cell = mesh.voronoi_cell_from(site, seed).ok()?;
        let degree_violation = 5usize
            .saturating_sub(cell.degree())
            .max(cell.degree().saturating_sub(7));
        hard_feasible &=
            degree_violation == 0 && voronoi_cell_is_convex_and_contains_site(mesh, &cell);
        violation += (degree_violation * degree_violation) as f64;

        if include_soft {
            let cell_area = cell.area_on_unit_sphere()?;
            if !cell_area.is_finite() || cell_area <= 0.0 {
                return None;
            }
            if let Some(target_area) = context.reference_dual_areas[&site] {
                area += (cell_area / target_area).ln().powi(2);
            }

            let mut centroid_sum = CartesianPoint::new(0.0, 0.0, 0.0);
            for corner in &cell.corners {
                centroid_sum = add_points(centroid_sum, normalized_point(*corner)?);
            }
            let centroid = normalized_point(centroid_sum)?;
            let site_position = normalized_point(mesh.vertices()[site])?;
            center += dot(site_position, centroid).clamp(-1.0, 1.0).acos().powi(2);
        }
    }

    for pair in dual_pairs {
        let face = pair.face;
        let triangle = mesh.triangles()[face];
        let points = triangle.map(|site| normalized_point(mesh.vertices()[site]));
        let [Some(a), Some(b), Some(c)] = points else {
            return None;
        };
        let d = normalized_point(mesh.vertices()[pair.opposite])?;
        match in_circle_on_sphere(a, b, c, d) {
            Ok(Sign::Positive) | Err(_) => hard_feasible = false,
            Ok(Sign::Negative | Sign::Zero) => {}
        }
        let side = dot(
            subtract_points(a, d),
            cross(subtract_points(b, d), subtract_points(c, d)),
        );
        if !side.is_finite() {
            return None;
        }
        violation += (-side).max(0.0).powi(2);
    }

    Some(DualEnergy {
        hard_feasible,
        violation,
        center,
        area,
    })
}

fn angle_margin_loss(mesh: &MeshState) -> Option<f64> {
    let objective = angle_margin_objective(mesh, mesh.active_triangle_slots())?;
    let worst_violation = objective
        .worst_constraints
        .iter()
        .map(|constraint| (-constraint.signed_margin_deg).max(0.0))
        .sum::<f64>();
    Some(-objective.signed_margin_deg + 0.001 * worst_violation)
}

#[derive(Clone, Copy, Debug)]
struct Dual9 {
    value: f64,
    derivative: [f64; 9],
}

impl Dual9 {
    fn variable(value: f64, index: usize) -> Self {
        let mut derivative = [0.0; 9];
        derivative[index] = 1.0;
        Self { value, derivative }
    }

    fn add(self, other: Self) -> Self {
        let mut derivative = [0.0; 9];
        for (i, item) in derivative.iter_mut().enumerate() {
            *item = self.derivative[i] + other.derivative[i];
        }
        Self {
            value: self.value + other.value,
            derivative,
        }
    }

    fn sub(self, other: Self) -> Self {
        self.add(other.scale(-1.0))
    }

    fn mul(self, other: Self) -> Self {
        let mut derivative = [0.0; 9];
        for (i, item) in derivative.iter_mut().enumerate() {
            *item = self.derivative[i] * other.value + other.derivative[i] * self.value;
        }
        Self {
            value: self.value * other.value,
            derivative,
        }
    }

    fn scale(self, scale: f64) -> Self {
        let mut derivative = [0.0; 9];
        for (i, item) in derivative.iter_mut().enumerate() {
            *item = self.derivative[i] * scale;
        }
        Self {
            value: self.value * scale,
            derivative,
        }
    }

    fn div(self, other: Self) -> Option<Self> {
        if other.value.abs() <= 1.0e-14 || !other.value.is_finite() {
            return None;
        }
        let denom = other.value * other.value;
        let mut derivative = [0.0; 9];
        for (i, item) in derivative.iter_mut().enumerate() {
            *item = (self.derivative[i] * other.value - self.value * other.derivative[i]) / denom;
        }
        Some(Self {
            value: self.value / other.value,
            derivative,
        })
    }

    fn sqrt(self) -> Option<Self> {
        if self.value <= 1.0e-28 || !self.value.is_finite() {
            return None;
        }
        let value = self.value.sqrt();
        let mut derivative = [0.0; 9];
        for (i, item) in derivative.iter_mut().enumerate() {
            *item = self.derivative[i] / (2.0 * value);
        }
        Some(Self { value, derivative })
    }

    fn acos_degrees(self) -> Result<Self, LocalGradientStatus> {
        if !self.value.is_finite() || self.value.abs() >= 1.0 - 1.0e-10 {
            return Err(LocalGradientStatus::UndefinedNearDegenerate);
        }
        let value = self.value.acos().to_degrees();
        let scale = -180.0 / std::f64::consts::PI / (1.0 - self.value * self.value).sqrt();
        Ok(Self {
            value,
            derivative: self.derivative.map(|d| d * scale),
        })
    }
}

fn dual_dot(left: [Dual9; 3], right: [Dual9; 3]) -> Dual9 {
    left[0]
        .mul(right[0])
        .add(left[1].mul(right[1]))
        .add(left[2].mul(right[2]))
}

fn dual_scale(point: [Dual9; 3], scale: Dual9) -> [Dual9; 3] {
    [
        point[0].mul(scale),
        point[1].mul(scale),
        point[2].mul(scale),
    ]
}

fn dual_sub(left: [Dual9; 3], right: [Dual9; 3]) -> [Dual9; 3] {
    [
        left[0].sub(right[0]),
        left[1].sub(right[1]),
        left[2].sub(right[2]),
    ]
}

fn dual_normalized(point: [Dual9; 3]) -> Result<[Dual9; 3], LocalGradientStatus> {
    let norm = dual_dot(point, point)
        .sqrt()
        .ok_or(LocalGradientStatus::UndefinedNearDegenerate)?;
    Ok([
        point[0]
            .div(norm)
            .ok_or(LocalGradientStatus::UndefinedNearDegenerate)?,
        point[1]
            .div(norm)
            .ok_or(LocalGradientStatus::UndefinedNearDegenerate)?,
        point[2]
            .div(norm)
            .ok_or(LocalGradientStatus::UndefinedNearDegenerate)?,
    ])
}

fn dual_vertex(point: CartesianPoint, offset: usize) -> [Dual9; 3] {
    [
        Dual9::variable(point.x, offset),
        Dual9::variable(point.y, offset + 1),
        Dual9::variable(point.z, offset + 2),
    ]
}

fn local_angle_gradient(
    mesh: &MeshState,
    face: usize,
    corner: usize,
) -> Result<LocalAngleGradient, LocalGradientStatus> {
    if !mesh.is_triangle_live(face) || corner >= 3 {
        return Err(LocalGradientStatus::UndefinedNearDegenerate);
    }
    let triangle = mesh.triangles()[face];
    let a = dual_normalized(dual_vertex(mesh.vertices()[triangle[corner]], 0))?;
    let b = dual_normalized(dual_vertex(mesh.vertices()[triangle[(corner + 1) % 3]], 3))?;
    let c = dual_normalized(dual_vertex(mesh.vertices()[triangle[(corner + 2) % 3]], 6))?;
    let ab = dual_dot(a, b);
    let ac = dual_dot(a, c);
    let left = dual_normalized(dual_sub(b, dual_scale(a, ab)))?;
    let right = dual_normalized(dual_sub(c, dual_scale(a, ac)))?;
    let angle = dual_dot(left, right).acos_degrees()?;
    let mut derivative = [CartesianPoint::new(0.0, 0.0, 0.0); 3];
    for vertex in 0..3 {
        let raw = CartesianPoint::new(
            angle.derivative[vertex * 3],
            angle.derivative[vertex * 3 + 1],
            angle.derivative[vertex * 3 + 2],
        );
        let position = normalized_point(mesh.vertices()[triangle[(corner + vertex) % 3]])
            .ok_or(LocalGradientStatus::UndefinedNearDegenerate)?;
        derivative[vertex] = subtract_points(raw, scale_point(position, dot(position, raw)));
    }
    Ok(LocalAngleGradient {
        angle_deg: angle.value,
        derivative,
    })
}

fn constraint_margin_gradient(
    mesh: &MeshState,
    constraint: &AngleConstraintKey,
) -> Result<[(usize, CartesianPoint); 3], LocalGradientStatus> {
    let gradient = local_angle_gradient(mesh, constraint.face, constraint.corner)?;
    let sign = if gradient.angle_deg <= 60.0 {
        1.0
    } else {
        -1.0
    };
    let triangle = mesh.triangles()[constraint.face];
    Ok([
        (
            triangle[constraint.corner],
            scale_point(gradient.derivative[0], sign),
        ),
        (
            triangle[(constraint.corner + 1) % 3],
            scale_point(gradient.derivative[1], sign),
        ),
        (
            triangle[(constraint.corner + 2) % 3],
            scale_point(gradient.derivative[2], sign),
        ),
    ])
}

fn active_angle_constraints(
    mesh: &MeshState,
    faces: impl IntoIterator<Item = usize>,
) -> Option<Vec<AngleConstraintKey>> {
    select_active_constraints(angle_constraints(mesh, faces)?)
}

fn select_active_constraints(
    mut constraints: Vec<AngleConstraintKey>,
) -> Option<Vec<AngleConstraintKey>> {
    if constraints.is_empty() {
        return None;
    }
    constraints.sort_by(|a, b| {
        a.signed_margin_deg
            .total_cmp(&b.signed_margin_deg)
            .then_with(|| a.face.cmp(&b.face))
            .then_with(|| a.corner.cmp(&b.corner))
    });
    let nearest_nonviolating_limit = 64.min(constraints.len());
    let mut selected = Vec::new();
    let mut nearest_nonviolating = Vec::new();
    for constraint in constraints {
        if constraint.signed_margin_deg < 0.0 {
            selected.push(constraint);
        } else if nearest_nonviolating.len() < nearest_nonviolating_limit {
            nearest_nonviolating.push(constraint);
        }
    }
    selected.extend(nearest_nonviolating);
    selected.sort_by(|a, b| {
        a.signed_margin_deg
            .total_cmp(&b.signed_margin_deg)
            .then_with(|| a.face.cmp(&b.face))
            .then_with(|| a.corner.cmp(&b.corner))
    });
    Some(selected)
}

#[derive(Clone, Debug)]
struct ActiveTrustRow {
    residual: f64,
    weight: f64,
    coefficients: Vec<(usize, f64)>,
    constraint: AngleConstraintKey,
}

fn active_trust_angle_step(
    mesh: &MeshState,
    patch: &ElasticPatch,
    _context: &EnergyContext,
    _guard_faces: &BTreeSet<usize>,
    trust_radius: f64,
) -> Option<ActiveTrustStep> {
    let active = active_angle_constraints(mesh, mesh.active_triangle_slots())?;
    let variables = active_trust_variables(mesh, patch)?;
    let rows = active_trust_rows(mesh, &active, &variables)?;
    let lambda = 1.0e-3;
    let mut delta = solve_damped_normal_equations(&rows, variables.len() * 2, lambda)?;
    clamp_trust_per_vertex(&mut delta, trust_radius);
    let mut updates = Vec::new();
    let mut delta_by_site = BTreeMap::<usize, CartesianPoint>::new();
    for (index, variable) in variables.iter().enumerate() {
        let first = delta[index * 2];
        let second = delta[index * 2 + 1];
        if first == 0.0 && second == 0.0 {
            continue;
        }
        let tangent = add_points(
            scale_point(variable.basis[0], first),
            scale_point(variable.basis[1], second),
        );
        let after = exponential_map(variable.point, tangent)?;
        delta_by_site.insert(variable.site, tangent);
        updates.push((variable.site, variable.point, after));
    }
    if updates.is_empty() {
        return None;
    }
    let current_margin = active
        .iter()
        .map(|constraint| constraint.signed_margin_deg)
        .min_by(f64::total_cmp)?;
    let mut predicted_margin = f64::INFINITY;
    for row in &rows {
        let linear_delta = row
            .coefficients
            .iter()
            .map(|&(column, value)| value * delta[column])
            .sum::<f64>();
        predicted_margin = predicted_margin.min(row.constraint.signed_margin_deg + linear_delta);
    }
    // Keep deterministic row assembly tied to the same sites used for updates.
    let _updated_sites = delta_by_site.len();
    Some(ActiveTrustStep {
        updates,
        predicted_margin_delta: predicted_margin - current_margin,
    })
}

#[derive(Clone, Debug)]
struct ActiveTrustVariable {
    site: usize,
    point: CartesianPoint,
    basis: [CartesianPoint; 2],
}

fn active_trust_variables(
    mesh: &MeshState,
    patch: &ElasticPatch,
) -> Option<Vec<ActiveTrustVariable>> {
    patch
        .movable_compact_vertices
        .iter()
        .copied()
        .map(|site| {
            Some(ActiveTrustVariable {
                site,
                point: mesh.vertices()[site],
                basis: tangent_basis(mesh.vertices()[site])?,
            })
        })
        .collect()
}

fn active_trust_rows(
    mesh: &MeshState,
    active: &[AngleConstraintKey],
    variables: &[ActiveTrustVariable],
) -> Option<Vec<ActiveTrustRow>> {
    let variable_slots = variables
        .iter()
        .enumerate()
        .map(|(index, variable)| (variable.site, index))
        .collect::<BTreeMap<_, _>>();
    let mut rows = Vec::new();
    for constraint in active {
        let mut coefficients = Vec::new();
        for (site, gradient) in constraint_margin_gradient(mesh, constraint).ok()? {
            let Some(&variable_index) = variable_slots.get(&site) else {
                continue;
            };
            let variable = &variables[variable_index];
            coefficients.push((variable_index * 2, dot(gradient, variable.basis[0])));
            coefficients.push((variable_index * 2 + 1, dot(gradient, variable.basis[1])));
        }
        if coefficients.iter().any(|&(_, value)| value.abs() > 1.0e-14) {
            let violation = (-constraint.signed_margin_deg).max(0.0);
            rows.push(ActiveTrustRow {
                residual: constraint.signed_margin_deg.min(0.0),
                weight: 1.0 + violation.min(100.0),
                coefficients,
                constraint: constraint.clone(),
            });
        }
    }
    (!rows.is_empty()).then_some(rows)
}

fn solve_damped_normal_equations(
    rows: &[ActiveTrustRow],
    columns: usize,
    lambda: f64,
) -> Option<Vec<f64>> {
    if columns == 0 || rows.is_empty() || lambda <= 0.0 || !lambda.is_finite() {
        return None;
    }
    let mut rhs = vec![0.0; columns];
    for row in rows {
        if !row.residual.is_finite() || !row.weight.is_finite() || row.weight <= 0.0 {
            return None;
        }
        for &(column, value) in &row.coefficients {
            rhs[column] -= row.weight * value * row.residual;
        }
    }
    conjugate_gradient_normal(rows, &rhs, lambda, columns)
}

fn conjugate_gradient_normal(
    rows: &[ActiveTrustRow],
    rhs: &[f64],
    lambda: f64,
    columns: usize,
) -> Option<Vec<f64>> {
    let mut x = vec![0.0; columns];
    let mut r = rhs.to_vec();
    let mut p = r.clone();
    let mut rs_old = dot_slice(&r, &r);
    if !rs_old.is_finite() {
        return None;
    }
    if rs_old.sqrt() <= 1.0e-14 {
        return Some(x);
    }
    for _ in 0..(columns * 4).max(16) {
        let ap = apply_damped_normal(rows, &p, lambda, columns);
        let denom = dot_slice(&p, &ap);
        if denom <= 1.0e-30 || !denom.is_finite() {
            return None;
        }
        let alpha = rs_old / denom;
        for i in 0..columns {
            x[i] += alpha * p[i];
            r[i] -= alpha * ap[i];
        }
        let rs_new = dot_slice(&r, &r);
        if !rs_new.is_finite() {
            return None;
        }
        if rs_new.sqrt() <= 1.0e-10 * rhs.len().max(1) as f64 {
            break;
        }
        let beta = rs_new / rs_old;
        for i in 0..columns {
            p[i] = r[i] + beta * p[i];
        }
        rs_old = rs_new;
    }
    Some(x)
}

fn apply_damped_normal(
    rows: &[ActiveTrustRow],
    x: &[f64],
    lambda: f64,
    columns: usize,
) -> Vec<f64> {
    let mut result = x.iter().map(|value| lambda * value).collect::<Vec<_>>();
    result.resize(columns, 0.0);
    for row in rows {
        let ax = row
            .coefficients
            .iter()
            .map(|&(column, value)| value * x[column])
            .sum::<f64>();
        for &(column, value) in &row.coefficients {
            result[column] += row.weight * value * ax;
        }
    }
    result
}

fn dot_slice(left: &[f64], right: &[f64]) -> f64 {
    left.iter().zip(right).map(|(a, b)| a * b).sum()
}

fn clamp_trust_per_vertex(delta: &mut [f64], trust_radius: f64) {
    if !trust_radius.is_finite() || trust_radius <= 0.0 {
        delta.fill(0.0);
        return;
    }
    for chunk in delta.as_chunks_mut::<2>().0 {
        let norm = (chunk[0] * chunk[0] + chunk[1] * chunk[1]).sqrt();
        if norm > trust_radius {
            let scale = trust_radius / norm;
            chunk[0] *= scale;
            chunk[1] *= scale;
        }
    }
}

struct ActiveTrustStepContext<'a> {
    before: &'a MeshState,
    step_plan: &'a ActiveTrustStep,
    patch: &'a ElasticPatch,
    energy_context: &'a EnergyContext,
    guard_faces: &'a BTreeSet<usize>,
    trust_radius: f64,
    maximum_radius: f64,
}

fn apply_active_trust_step(
    mesh: &mut MeshState,
    context: ActiveTrustStepContext<'_>,
) -> TrustUpdate {
    for &(site, _, point) in &context.step_plan.updates {
        mesh.move_vertex(site, point);
    }
    let movement_norm = context
        .step_plan
        .updates
        .iter()
        .map(|&(_, before, after)| arc_length_unit_sphere(before, after).abs())
        .sum::<f64>();
    let accepted = angle_phase_step_is_better(
        context.before.clone(),
        mesh,
        context.patch,
        context.energy_context,
        context.guard_faces,
        movement_norm,
    );
    let ratio = predicted_vs_actual_margin_ratio(
        context.before,
        mesh,
        context.step_plan.predicted_margin_delta,
    )
    .unwrap_or(f64::NEG_INFINITY);
    let trust_update = update_trust_radius(
        context.trust_radius,
        accepted,
        ratio,
        context.maximum_radius,
    );
    if !trust_update.accepted {
        *mesh = context.before.clone();
    }
    trust_update
}

fn predicted_vs_actual_margin_ratio_from_margins(
    before_margin: f64,
    after_margin: f64,
    predicted_margin_delta: f64,
) -> Option<f64> {
    if predicted_margin_delta <= 1.0e-14 || !predicted_margin_delta.is_finite() {
        return None;
    }
    Some((after_margin - before_margin) / predicted_margin_delta)
}

fn predicted_vs_actual_margin_ratio(
    before: &MeshState,
    after: &MeshState,
    predicted_margin_delta: f64,
) -> Option<f64> {
    let before_margin =
        angle_margin_objective(before, before.active_triangle_slots())?.signed_margin_deg;
    let after_margin =
        angle_margin_objective(after, after.active_triangle_slots())?.signed_margin_deg;
    predicted_vs_actual_margin_ratio_from_margins(
        before_margin,
        after_margin,
        predicted_margin_delta,
    )
}

fn update_trust_radius(
    current_radius: f64,
    accepted_by_key: bool,
    ratio: f64,
    maximum_radius: f64,
) -> TrustUpdate {
    if !accepted_by_key || !ratio.is_finite() || ratio < 0.1 {
        return TrustUpdate {
            accepted: false,
            ratio,
            next_radius: current_radius * 0.5,
        };
    }
    let next_radius = if ratio > 0.75 {
        (current_radius * 1.5).min(maximum_radius)
    } else {
        current_radius
    };
    TrustUpdate {
        accepted: true,
        ratio,
        next_radius,
    }
}

fn finite_difference_angle_margin_gradient(
    mesh: &mut MeshState,
    patch: &ElasticPatch,
    initial_step: f64,
) -> Option<Vec<(usize, CartesianPoint)>> {
    let epsilon = (initial_step * 1.0e-3).clamp(1.0e-7, 1.0e-5);
    let mut gradient = Vec::with_capacity(patch.movable_compact_vertices.len());
    for &site in &patch.movable_compact_vertices {
        let point = mesh.vertices()[site];
        let [first, second] = tangent_basis(point)?;
        let base_loss = angle_margin_loss(mesh)?;
        let mut derivative = |direction: CartesianPoint| {
            let plus = exponential_map(point, scale_point(direction, epsilon))?;
            let minus = exponential_map(point, scale_point(direction, -epsilon))?;
            mesh.move_vertex(site, plus);
            let plus_loss = angle_margin_loss(mesh);
            mesh.move_vertex(site, minus);
            let minus_loss = angle_margin_loss(mesh);
            mesh.move_vertex(site, point);
            match (plus_loss, minus_loss) {
                (Some(plus), Some(minus)) => Some((plus - minus) / (2.0 * epsilon)),
                (Some(plus), None) => Some((plus - base_loss) / epsilon),
                (None, Some(minus)) => Some((base_loss - minus) / epsilon),
                (None, None) => None,
            }
        };
        let d_first = derivative(first)?;
        let d_second = derivative(second)?;
        gradient.push((
            site,
            add_points(scale_point(first, d_first), scale_point(second, d_second)),
        ));
    }
    Some(gradient)
}

fn finite_difference_gradient(
    mesh: &mut MeshState,
    patch: &ElasticPatch,
    phase: ElasticBlockPhase,
    initial_step: f64,
    context: &EnergyContext,
) -> Option<Vec<(usize, CartesianPoint)>> {
    // ponytail: finite differences keep PR29 auditable; replace with analytic
    // patch derivatives only if transition-local profiling shows this dominates.
    let epsilon = (initial_step * 1.0e-3).clamp(1.0e-7, 1.0e-5);
    let mut gradient = Vec::with_capacity(patch.movable_compact_vertices.len());
    for &site in &patch.movable_compact_vertices {
        let local = context.derivatives.get(&site)?;
        let point = mesh.vertices()[site];
        let [first, second] = tangent_basis(point)?;
        let energy_at_current = |mesh: &MeshState| {
            if matches!(phase, ElasticBlockPhase::Untangle) {
                elastic_energy(mesh, patch, phase, context)
            } else {
                elastic_energy_in(
                    mesh,
                    patch,
                    phase,
                    context,
                    &local.guard_faces,
                    &local.guard_edges,
                    &local.guard_seeds,
                    &local.dual_pairs,
                )
            }
        };
        let base_energy = energy_at_current(mesh)?;
        let mut derivative = |direction: CartesianPoint| {
            let plus = exponential_map(point, scale_point(direction, epsilon))?;
            let minus = exponential_map(point, scale_point(direction, -epsilon))?;
            mesh.move_vertex(site, plus);
            let plus_energy = energy_at_current(mesh);
            mesh.move_vertex(site, minus);
            let minus_energy = energy_at_current(mesh);
            mesh.move_vertex(site, point);
            match (plus_energy, minus_energy) {
                (Some(plus), Some(minus)) => Some((plus - minus) / (2.0 * epsilon)),
                (Some(plus), None) => Some((plus - base_energy) / epsilon),
                (None, Some(minus)) => Some((base_energy - minus) / epsilon),
                (None, None) => None,
            }
        };
        let d_first = derivative(first)?;
        let d_second = derivative(second)?;
        gradient.push((
            site,
            add_points(scale_point(first, d_first), scale_point(second, d_second)),
        ));
    }
    Some(gradient)
}

fn synchronous_updates(
    mesh: &MeshState,
    gradient: &[(usize, CartesianPoint)],
    scale: f64,
) -> Option<Vec<(usize, CartesianPoint, CartesianPoint)>> {
    gradient
        .iter()
        .map(|&(site, vector)| {
            exponential_map(mesh.vertices()[site], scale_point(vector, scale))
                .map(|point| (site, mesh.vertices()[site], point))
        })
        .collect()
}

fn exponential_map(point: CartesianPoint, tangent_step: CartesianPoint) -> Option<CartesianPoint> {
    let radius = magnitude(point);
    if radius <= 0.0 || !radius.is_finite() {
        return None;
    }
    let unit = scale_point(point, 1.0 / radius);
    let tangent_step = add_points(tangent_step, scale_point(unit, -dot(unit, tangent_step)));
    let length = magnitude(tangent_step);
    if !length.is_finite() {
        return None;
    }
    if length == 0.0 {
        return Some(point);
    }
    let direction = scale_point(tangent_step, 1.0 / length);
    Some(scale_point(
        add_points(
            scale_point(unit, length.cos()),
            scale_point(direction, length.sin()),
        ),
        radius,
    ))
}

fn tangent_basis(point: CartesianPoint) -> Option<[CartesianPoint; 2]> {
    let unit = normalized_point(point)?;
    let axis = if unit.z.abs() < 0.8 {
        CartesianPoint::new(0.0, 0.0, 1.0)
    } else {
        CartesianPoint::new(1.0, 0.0, 0.0)
    };
    let first = normalized_point(cross(unit, axis))?;
    let second = normalized_point(cross(unit, first))?;
    Some([first, second])
}

fn normalized_point(point: CartesianPoint) -> Option<CartesianPoint> {
    let length = magnitude(point);
    (length > 0.0 && length.is_finite()).then(|| scale_point(point, 1.0 / length))
}

fn dot(left: CartesianPoint, right: CartesianPoint) -> f64 {
    left.x * right.x + left.y * right.y + left.z * right.z
}

fn scale_point(point: CartesianPoint, scale: f64) -> CartesianPoint {
    CartesianPoint::new(point.x * scale, point.y * scale, point.z * scale)
}

fn add_points(left: CartesianPoint, right: CartesianPoint) -> CartesianPoint {
    CartesianPoint::new(left.x + right.x, left.y + right.y, left.z + right.z)
}

fn subtract_points(left: CartesianPoint, right: CartesianPoint) -> CartesianPoint {
    CartesianPoint::new(left.x - right.x, left.y - right.y, left.z - right.z)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coarsen::{
            n6_legacy_mixed_fixture_with_source_levels, solve_full_polygon_merge,
            FullPolygonMergeLimits, FullPolygonMergeOutcome,
        },
        mother_grid::MotherGrid,
    };

    fn single_movable_patch() -> (MeshState, ElasticPatch, BTreeSet<usize>, EnergyContext) {
        let grid = MotherGrid::generate(4).unwrap();
        let face = grid.mesh.active_triangle_slots().next().unwrap();
        let site = grid.mesh.triangles()[face][0];
        let guard_faces = grid
            .mesh
            .active_triangle_slots()
            .filter(|&face| grid.mesh.triangles()[face].contains(&site))
            .collect::<Vec<_>>();
        let fixed = guard_faces
            .iter()
            .flat_map(|&face| grid.mesh.triangles()[face])
            .filter(|&candidate| candidate != site)
            .collect::<BTreeSet<_>>();
        let patch = ElasticPatch {
            topology: TransitionTopologyCandidate {
                component_id: 48,
                topology_id: 1,
                core_parents: Vec::new(),
                custom_transition_triangles: BTreeMap::new(),
                source_triangles: guard_faces
                    .iter()
                    .map(|&face| grid.mesh.triangles()[face])
                    .collect(),
                source_active_vertices: std::iter::once(site)
                    .chain(fixed.iter().copied())
                    .collect(),
                source_degree_forecast: BTreeMap::new(),
            },
            reference_positions: grid.mesh.vertices().to_vec(),
            fixed_compact_vertices: fixed.into_iter().collect(),
            movable_compact_vertices: vec![site],
            guard_faces: guard_faces.clone(),
            target_mode: ElasticTargetMode::TrialReference,
            target_field: ElasticTargetField::default(),
        };
        let guard_set = guard_faces.iter().copied().collect::<BTreeSet<_>>();
        let context = EnergyContext::new(&grid.mesh, &patch).unwrap();
        (grid.mesh, patch, guard_set, context)
    }

    #[test]
    fn dual9_matches_finite_difference() {
        let grid = MotherGrid::generate(4).unwrap();
        let face = grid.mesh.active_triangle_slots().next().unwrap();
        let corner = 0;
        let triangle = grid.mesh.triangles()[face];
        let gradient = local_angle_gradient(&grid.mesh, face, corner).unwrap();
        let site = triangle[corner];
        let direction = tangent_basis(grid.mesh.vertices()[site]).unwrap()[0];
        let epsilon = 1.0e-6;
        let mut plus = grid.mesh.clone();
        plus.move_vertex(
            site,
            exponential_map(grid.mesh.vertices()[site], scale_point(direction, epsilon)).unwrap(),
        );
        let mut minus = grid.mesh.clone();
        minus.move_vertex(
            site,
            exponential_map(grid.mesh.vertices()[site], scale_point(direction, -epsilon)).unwrap(),
        );
        let plus_angle = corner_angle_degrees(&plus, triangle, corner).unwrap();
        let minus_angle = corner_angle_degrees(&minus, triangle, corner).unwrap();
        let finite_difference = (plus_angle - minus_angle) / (2.0 * epsilon);
        let dual = dot(gradient.derivative[0], direction);
        assert!((dual - finite_difference).abs() < 1.0e-4);
    }

    #[test]
    fn tangent_projection_is_orthogonal_to_position() {
        let grid = MotherGrid::generate(4).unwrap();
        let face = grid.mesh.active_triangle_slots().next().unwrap();
        let gradient = local_angle_gradient(&grid.mesh, face, 0).unwrap();
        let triangle = grid.mesh.triangles()[face];
        for (offset, derivative) in gradient.derivative.iter().enumerate() {
            let position = normalized_point(grid.mesh.vertices()[triangle[offset]]).unwrap();
            assert!(dot(position, *derivative).abs() < 1.0e-10);
        }
    }

    #[test]
    fn exp_update_stays_on_unit_sphere() {
        let grid = MotherGrid::generate(4).unwrap();
        let site = grid.mesh.active_triangle_slots().next().unwrap();
        let point = grid.mesh.vertices()[grid.mesh.triangles()[site][0]];
        let step = scale_point(tangent_basis(point).unwrap()[0], 0.123);
        let updated = exponential_map(point, step).unwrap();
        assert!((magnitude(updated) - magnitude(point)).abs() < 1.0e-12);
    }

    #[test]
    fn near_degenerate_gradient_is_typed_undefined() {
        let mut grid = MotherGrid::generate(4).unwrap();
        let face = grid.mesh.active_triangle_slots().next().unwrap();
        let triangle = grid.mesh.triangles()[face];
        grid.mesh
            .move_vertex(triangle[1], grid.mesh.vertices()[triangle[0]]);
        assert_eq!(
            local_angle_gradient(&grid.mesh, face, 0),
            Err(LocalGradientStatus::UndefinedNearDegenerate)
        );
    }

    #[test]
    fn predicted_vs_actual_margin_ratio_uses_actual_over_predicted_margin() {
        let ratio = predicted_vs_actual_margin_ratio_from_margins(-8.0, -6.0, 4.0);
        assert_eq!(ratio, Some(0.5));
    }

    #[test]
    fn active_set_keeps_all_violations_and_nearest_64_nonviolations() {
        let mut constraints = Vec::new();
        for face in 0..70 {
            constraints.push(AngleConstraintKey {
                face,
                corner: 0,
                angle_deg: 35.0,
                signed_margin_deg: -((face + 1) as f64),
            });
        }
        for face in 1000..1100 {
            constraints.push(AngleConstraintKey {
                face,
                corner: 1,
                angle_deg: 40.2 + (face - 999) as f64 * 0.001,
                signed_margin_deg: (face - 999) as f64 * 0.001,
            });
        }
        let active = select_active_constraints(constraints).unwrap();
        assert_eq!(
            active.iter().filter(|c| c.signed_margin_deg < 0.0).count(),
            70
        );
        let nonviolating = active
            .iter()
            .filter(|c| c.signed_margin_deg >= 0.0)
            .map(|c| c.face)
            .collect::<Vec<_>>();
        assert_eq!(nonviolating.len(), 64);
        assert_eq!(nonviolating[0], 1000);
        assert_eq!(nonviolating[63], 1063);
    }

    #[test]
    fn active_trust_solver_solves_damped_normal_equations() {
        let rows = vec![
            ActiveTrustRow {
                residual: -1.0,
                weight: 1.0,
                coefficients: vec![(0, 1.0), (1, 1.0)],
                constraint: AngleConstraintKey {
                    face: 0,
                    corner: 0,
                    angle_deg: 39.2,
                    signed_margin_deg: -1.0,
                },
            },
            ActiveTrustRow {
                residual: -2.0,
                weight: 1.0,
                coefficients: vec![(0, 1.0), (1, -1.0)],
                constraint: AngleConstraintKey {
                    face: 1,
                    corner: 0,
                    angle_deg: 38.2,
                    signed_margin_deg: -2.0,
                },
            },
        ];
        let solved = solve_damped_normal_equations(&rows, 2, 0.0_f64.max(1.0e-6)).unwrap();
        // With negligible damping, equations x+y=1 and x-y=2 give x=1.5, y=-0.5.
        assert!((solved[0] - 1.5).abs() < 1.0e-5);
        assert!((solved[1] + 0.5).abs() < 1.0e-5);
    }

    #[test]
    fn active_trust_rejected_step_rolls_back_mesh() {
        let (mesh, patch, guard_faces, context) = single_movable_patch();
        let site = patch.movable_compact_vertices[0];
        let before_vertices = mesh.vertices().to_vec();
        let bad_point = mesh.vertices()[patch.fixed_compact_vertices[0]];
        let step = ActiveTrustStep {
            updates: vec![(site, mesh.vertices()[site], bad_point)],
            predicted_margin_delta: 1.0,
        };
        let mut candidate = mesh.clone();
        let update = apply_active_trust_step(
            &mut candidate,
            ActiveTrustStepContext {
                before: &mesh,
                step_plan: &step,
                patch: &patch,
                energy_context: &context,
                guard_faces: &guard_faces,
                trust_radius: 0.1,
                maximum_radius: 1.0,
            },
        );
        assert!(!update.accepted);
        assert_eq!(candidate.vertices(), before_vertices.as_slice());
    }

    #[test]
    fn orientation_regression_rejects_step() {
        let before = AngleAcceptanceKey {
            negative_orientation_count: 0,
            crossing_count: 0,
            signed_margin_deg: -10.0,
            sum_worst_k_violation: 10.0,
            delaunay_violations: 0,
            invalid_voronoi_cells: 0,
            movement_norm: 0.0,
        };
        let after = AngleAcceptanceKey {
            negative_orientation_count: 1,
            signed_margin_deg: 0.0,
            ..before.clone()
        };
        assert!(!after.is_better_than(&before));
    }

    #[test]
    fn trust_radius_shrinks_on_bad_model() {
        let update = update_trust_radius(0.1, true, 0.05, 1.0);
        assert!(!update.accepted);
        assert!(update.next_radius < 0.1);
    }

    #[test]
    fn trust_radius_expands_on_good_model() {
        let update = update_trust_radius(0.1, true, 0.9, 1.0);
        assert!(update.accepted);
        assert!(update.next_radius > 0.1);
    }

    #[test]
    fn local_finite_difference_matches_the_full_elastic_objective() {
        let grid = MotherGrid::generate(4).unwrap();
        let face = grid.mesh.active_triangle_slots().next().unwrap();
        let [site, neighbour, _] = grid.mesh.triangles()[face];
        let guard_faces = grid
            .mesh
            .active_triangle_slots()
            .filter(|&face| grid.mesh.triangles()[face].contains(&site))
            .collect::<Vec<_>>();
        let fixed = guard_faces
            .iter()
            .flat_map(|&face| grid.mesh.triangles()[face])
            .filter(|&candidate| candidate != site)
            .collect::<BTreeSet<_>>();
        let reference_positions = grid.mesh.vertices().to_vec();
        let patch = ElasticPatch {
            topology: TransitionTopologyCandidate {
                component_id: 1,
                topology_id: 1,
                core_parents: Vec::new(),
                custom_transition_triangles: BTreeMap::new(),
                source_triangles: guard_faces
                    .iter()
                    .map(|&face| grid.mesh.triangles()[face])
                    .collect(),
                source_active_vertices: std::iter::once(site)
                    .chain(fixed.iter().copied())
                    .collect(),
                source_degree_forecast: BTreeMap::new(),
            },
            reference_positions: reference_positions.clone(),
            fixed_compact_vertices: fixed.into_iter().collect(),
            movable_compact_vertices: vec![site],
            guard_faces,
            target_mode: ElasticTargetMode::TrialReference,
            target_field: ElasticTargetField::default(),
        };
        let mut mesh = grid.mesh;
        mesh.move_vertex(
            site,
            normalized_point(add_points(
                scale_point(reference_positions[site], 0.98),
                scale_point(reference_positions[neighbour], 0.02),
            ))
            .unwrap(),
        );
        let context = EnergyContext::new(&mesh, &patch).unwrap();
        let local = finite_difference_gradient(
            &mut mesh.clone(),
            &patch,
            ElasticBlockPhase::AngleFeasibility,
            0.01,
            &context,
        )
        .unwrap();
        let full = full_finite_difference_gradient(
            &mut mesh,
            &patch,
            ElasticBlockPhase::AngleFeasibility,
            0.01,
            &context,
        )
        .unwrap();

        assert_eq!(local.len(), full.len());
        for ((local_site, local), (full_site, full)) in local.into_iter().zip(full) {
            assert_eq!(local_site, full_site);
            for (local, full) in [(local.x, full.x), (local.y, full.y), (local.z, full.z)] {
                assert!((local - full).abs() <= 1.0e-5 * full.abs().max(1.0));
            }
        }
    }

    #[test]
    fn median_averages_even_sample_middle_values() {
        let mut odd = [3.0, 1.0, 2.0];
        assert_eq!(median(&mut odd), Some(2.0));
        let mut even = [4.0, 1.0, 2.0, 10.0];
        assert_eq!(median(&mut even), Some(3.0));
    }

    #[test]
    fn mother_level_area_samples_each_active_vertex_once() {
        let grid = MotherGrid::generate(4).unwrap();
        let source_levels = grid
            .mesh
            .vertices()
            .iter()
            .enumerate()
            .map(|(site, _)| grid.mesh.is_vertex_live(site).then_some(0))
            .collect::<Vec<_>>();
        let samples = mother_level_voronoi_areas(&grid.mesh, &source_levels);
        let mut unique_areas = samples[&0].clone();
        assert_eq!(unique_areas.len(), grid.mesh.active_vertex_slots().count());
        assert!((unique_areas.iter().sum::<f64>() - 4.0 * std::f64::consts::PI).abs() < 1.0e-10);
        let expected_unique = median(&mut unique_areas).unwrap();
        let metrics = mother_level_metrics(&grid.mesh, &source_levels).unwrap();
        assert!((metrics[&0].median_voronoi_area - expected_unique).abs() < 1.0e-15);
    }

    #[test]
    fn hierarchy_area_targets_override_invalid_reference_dual_areas() {
        let grid = MotherGrid::generate(4).unwrap();
        let face = grid.mesh.active_triangle_slots().next().unwrap();
        let triangle = grid.mesh.triangles()[face];
        let site = triangle[0];
        let target_area = 0.123;
        let patch = ElasticPatch {
            topology: TransitionTopologyCandidate {
                component_id: 46,
                topology_id: 0,
                core_parents: Vec::new(),
                custom_transition_triangles: BTreeMap::new(),
                source_triangles: vec![triangle],
                source_active_vertices: triangle.to_vec(),
                source_degree_forecast: BTreeMap::new(),
            },
            reference_positions: vec![
                CartesianPoint::new(0.0, 0.0, 0.0);
                grid.mesh.vertices().len()
            ],
            fixed_compact_vertices: vec![triangle[1], triangle[2]],
            movable_compact_vertices: vec![site],
            guard_faces: vec![face],
            target_mode: ElasticTargetMode::HierarchyEdgeAreaDegree,
            target_field: ElasticTargetField {
                target_cell_areas: BTreeMap::from([(site, target_area)]),
                ..Default::default()
            },
        };
        let context = EnergyContext::new(&grid.mesh, &patch).unwrap();
        assert_eq!(context.reference_dual_areas[&site], Some(target_area));
    }

    #[test]
    fn angle_margin_objective_sorts_worst_constraints_stably() {
        let grid = MotherGrid::generate(4).unwrap();
        let faces = grid
            .mesh
            .active_triangle_slots()
            .take(4)
            .collect::<Vec<_>>();
        let objective = angle_margin_objective(&grid.mesh, faces.iter().copied()).unwrap();
        assert_eq!(
            objective.signed_margin_deg,
            objective.worst_constraints[0].signed_margin_deg
        );
        for window in objective.worst_constraints.windows(2) {
            let left = &window[0];
            let right = &window[1];
            assert!(
                left.signed_margin_deg < right.signed_margin_deg
                    || (left.signed_margin_deg == right.signed_margin_deg
                        && (left.face, left.corner) <= (right.face, right.corner))
            );
        }
    }

    #[test]
    fn angle_acceptance_rejects_signed_margin_regression() {
        let before = AngleAcceptanceKey {
            negative_orientation_count: 0,
            crossing_count: 0,
            signed_margin_deg: -1.0,
            sum_worst_k_violation: 1.0,
            delaunay_violations: 0,
            invalid_voronoi_cells: 0,
            movement_norm: 0.0,
        };
        let lower_energy_but_worse_margin = AngleAcceptanceKey {
            signed_margin_deg: -2.0,
            sum_worst_k_violation: 0.0,
            movement_norm: 0.0,
            ..before.clone()
        };
        assert!(!lower_energy_but_worse_margin.is_better_than(&before));
    }

    #[test]
    fn angle_acceptance_uses_truthful_dual_counts_after_margin_terms() {
        let before = AngleAcceptanceKey {
            negative_orientation_count: 0,
            crossing_count: 0,
            signed_margin_deg: -1.0,
            sum_worst_k_violation: 1.0,
            delaunay_violations: 0,
            invalid_voronoi_cells: 0,
            movement_norm: 0.0,
        };
        let worse_delaunay = AngleAcceptanceKey {
            delaunay_violations: 1,
            ..before.clone()
        };
        assert!(!worse_delaunay.is_better_than(&before));
        let worse_voronoi = AngleAcceptanceKey {
            invalid_voronoi_cells: 1,
            ..before.clone()
        };
        assert!(!worse_voronoi.is_better_than(&before));
        let better_margin_worse_later_counts = AngleAcceptanceKey {
            signed_margin_deg: -0.9,
            delaunay_violations: 10,
            invalid_voronoi_cells: 10,
            ..before.clone()
        };
        assert!(better_margin_worse_later_counts.is_better_than(&before));
    }

    #[test]
    fn ring_scale_interpolation_responds_to_scale_and_distance() {
        let grid = MotherGrid::generate(4).unwrap();
        let face = grid.mesh.active_triangle_slots().next().unwrap();
        let [movable, fixed_a, fixed_b] = grid.mesh.triangles()[face];
        let guard_faces = vec![face];
        let mut target_scales = BTreeMap::from([(fixed_a, 2.0), (fixed_b, 0.5), (movable, 1.0)]);
        let patch = ElasticPatch {
            topology: TransitionTopologyCandidate {
                component_id: 47,
                topology_id: 0,
                core_parents: Vec::new(),
                custom_transition_triangles: BTreeMap::new(),
                source_triangles: vec![grid.mesh.triangles()[face]],
                source_active_vertices: vec![movable, fixed_a, fixed_b],
                source_degree_forecast: BTreeMap::new(),
            },
            reference_positions: grid.mesh.vertices().to_vec(),
            fixed_compact_vertices: vec![fixed_a, fixed_b],
            movable_compact_vertices: vec![movable],
            guard_faces,
            target_mode: ElasticTargetMode::HierarchyEdgeAreaDegree,
            target_field: ElasticTargetField {
                target_vertex_scales: target_scales.clone(),
                ..Default::default()
            },
        };
        let mut first = grid.mesh.clone();
        ring_scale_start(&mut first, &patch).unwrap();
        target_scales.insert(fixed_a, 8.0);
        target_scales.insert(fixed_b, 0.25);
        let mut swapped = patch.clone();
        swapped.target_field.target_vertex_scales = target_scales;
        let mut second = grid.mesh.clone();
        ring_scale_start(&mut second, &swapped).unwrap();
        assert_ne!(first.vertices()[movable], grid.mesh.vertices()[movable]);
        assert_ne!(first.vertices()[movable], second.vertices()[movable]);
        assert_eq!(first.vertices()[fixed_a], grid.mesh.vertices()[fixed_a]);
        assert_eq!(first.vertices()[fixed_b], grid.mesh.vertices()[fixed_b]);
    }

    fn frozen_domain_patch(
        domain_id: GeometryDomainId,
    ) -> (MotherGrid, FullPolygonMergeTrial, ElasticPatch) {
        let (source, component, source_levels) =
            n6_legacy_mixed_fixture_with_source_levels().unwrap();
        let outcome = solve_full_polygon_merge(
            &source,
            &component,
            FullPolygonMergeLimits {
                topology_states: 100_000,
            },
        );
        let FullPolygonMergeOutcome::Closed(trial) = outcome else {
            panic!("frozen N6 full-polygon family must close: {outcome:?}");
        };
        let patch = ElasticPatch::from_full_polygon_merge_with_domain(
            &source,
            &component,
            &trial,
            &BTreeSet::new(),
            domain_id,
        )
        .unwrap()
        .with_hierarchy_targets(
            &source,
            &trial.global_trial.mesh,
            &source_levels,
            ElasticTargetMode::HierarchyEdgeAreaDegree,
        )
        .unwrap();
        (source, *trial, patch)
    }

    #[test]
    fn domain_sets_are_nested() {
        let (_, _, current) = frozen_domain_patch(GeometryDomainId::CurrentAnnulus);
        let (_, _, plus_one) = frozen_domain_patch(GeometryDomainId::PlusOneOrdinaryRing);
        let (_, _, plus_two) = frozen_domain_patch(GeometryDomainId::PlusTwoOrdinaryRings);
        let current_set = current
            .movable_compact_vertices
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let plus_one_set = plus_one
            .movable_compact_vertices
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let plus_two_set = plus_two
            .movable_compact_vertices
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        assert!(current_set.is_subset(&plus_one_set));
        assert!(plus_one_set.is_subset(&plus_two_set));
        assert!(current_set.len() < plus_one_set.len());
        assert!(plus_one_set.len() < plus_two_set.len());
    }

    #[test]
    fn current_annulus_matches_legacy_wrapper_and_fixture_counts() {
        let (source, component, _) = n6_legacy_mixed_fixture_with_source_levels().unwrap();
        let outcome = solve_full_polygon_merge(
            &source,
            &component,
            FullPolygonMergeLimits {
                topology_states: 100_000,
            },
        );
        let FullPolygonMergeOutcome::Closed(trial) = outcome else {
            panic!("frozen N6 full-polygon family must close: {outcome:?}");
        };
        let legacy =
            ElasticPatch::from_full_polygon_merge(&source, &component, &trial, &BTreeSet::new())
                .unwrap();
        let explicit = ElasticPatch::from_full_polygon_merge_with_domain(
            &source,
            &component,
            &trial,
            &BTreeSet::new(),
            GeometryDomainId::CurrentAnnulus,
        )
        .unwrap();
        assert_eq!(
            legacy.movable_compact_vertices,
            explicit.movable_compact_vertices
        );
        assert_eq!(
            legacy.fixed_compact_vertices,
            explicit.fixed_compact_vertices
        );
        assert_eq!(legacy.guard_faces, explicit.guard_faces);
        assert_eq!(
            legacy.topology.source_active_vertices,
            explicit.topology.source_active_vertices
        );
        assert_eq!(explicit.movable_compact_vertices.len(), 40);
        assert_eq!(explicit.fixed_compact_vertices.len(), 40);
        assert_eq!(explicit.guard_faces.len(), 122);
        let source_slots = &trial.global_trial.mesh.source_vertex_slots;
        let guard_sources = explicit
            .guard_faces
            .iter()
            .flat_map(|&face| trial.global_trial.mesh.mesh.triangles()[face])
            .filter_map(|site| source_slots[site])
            .collect::<BTreeSet<_>>();
        assert_eq!(guard_sources.len(), 80);
    }

    #[test]
    fn plus_one_releases_all_current_frontier_ordinary_sources() {
        let (source, trial, current) = frozen_domain_patch(GeometryDomainId::CurrentAnnulus);
        let (_, _, plus_one) = frozen_domain_patch(GeometryDomainId::PlusOneOrdinaryRing);
        let source_slots = &trial.global_trial.mesh.source_vertex_slots;
        let current_movable = current
            .movable_compact_vertices
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let current_frontier = current
            .fixed_compact_vertices
            .iter()
            .copied()
            .filter(|site| source_slots[*site].is_some())
            .filter(|site| {
                trial
                    .global_trial
                    .mesh
                    .mesh
                    .active_triangle_slots()
                    .flat_map(|face| {
                        local_triangle_edges(trial.global_trial.mesh.mesh.triangles()[face])
                    })
                    .any(|edge| {
                        (edge.0 == *site && current_movable.contains(&edge.1))
                            || (edge.1 == *site && current_movable.contains(&edge.0))
                    })
            })
            .filter(|site| {
                !matches!(
                    source.addresses[source_slots[*site].unwrap()],
                    Some(VertexAddress::IcosahedronVertex(_))
                )
            })
            .collect::<BTreeSet<_>>();
        let plus_one_movable = plus_one
            .movable_compact_vertices
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        assert!(current_frontier.is_subset(&plus_one_movable));
        assert!(!current_frontier.is_empty());
    }

    #[test]
    fn fixed_anchors_remain_fixed_across_domain_ladder() {
        let (source, trial, plus_two) = frozen_domain_patch(GeometryDomainId::PlusTwoOrdinaryRings);
        let source_slots = &trial.global_trial.mesh.source_vertex_slots;
        let movable_sources = plus_two
            .movable_compact_vertices
            .iter()
            .filter_map(|&site| source_slots[site])
            .collect::<BTreeSet<_>>();
        for (source_slot, address) in source.addresses.iter().enumerate() {
            if matches!(address, Some(VertexAddress::IcosahedronVertex(_))) {
                assert!(!movable_sources.contains(&source_slot));
            }
        }
    }

    #[test]
    fn physical_fixed_sources_remain_fixed_across_domain_ladder() {
        let (source, component, _source_levels) =
            n6_legacy_mixed_fixture_with_source_levels().unwrap();
        let outcome = solve_full_polygon_merge(
            &source,
            &component,
            FullPolygonMergeLimits {
                topology_states: 100_000,
            },
        );
        let FullPolygonMergeOutcome::Closed(trial) = outcome else {
            panic!("frozen N6 full-polygon family must close: {outcome:?}");
        };
        let current_patch = ElasticPatch::from_full_polygon_merge_with_domain(
            &source,
            &component,
            &trial,
            &BTreeSet::new(),
            GeometryDomainId::CurrentAnnulus,
        )
        .unwrap();
        let source_slots = &trial.global_trial.mesh.source_vertex_slots;
        let physical_source = current_patch
            .movable_compact_vertices
            .iter()
            .filter_map(|&site| source_slots[site].map(|source_slot| (site, source_slot)))
            .find(|&(_site, source_slot)| {
                !matches!(
                    source.addresses[source_slot],
                    Some(VertexAddress::IcosahedronVertex(_))
                )
            })
            .map(|(_site, source_slot)| source_slot)
            .unwrap();
        let patch = ElasticPatch::from_full_polygon_merge_with_domain(
            &source,
            &component,
            &trial,
            &BTreeSet::from([physical_source]),
            GeometryDomainId::PlusTwoOrdinaryRings,
        )
        .unwrap();
        let movable_sources = patch
            .movable_compact_vertices
            .iter()
            .filter_map(|&site| source_slots[site])
            .collect::<BTreeSet<_>>();
        assert!(!movable_sources.contains(&physical_source));
    }

    #[test]
    fn frozen_geometry_starts_move_movable_vertices_and_keep_fixed_vertices() {
        let (source, component, source_levels) =
            n6_legacy_mixed_fixture_with_source_levels().unwrap();
        let outcome = solve_full_polygon_merge(
            &source,
            &component,
            FullPolygonMergeLimits {
                topology_states: 100_000,
            },
        );
        let FullPolygonMergeOutcome::Closed(trial) = outcome else {
            panic!("frozen N6 full-polygon family must close: {outcome:?}");
        };
        let patch =
            ElasticPatch::from_full_polygon_merge(&source, &component, &trial, &BTreeSet::new())
                .unwrap()
                .with_hierarchy_targets(
                    &source,
                    &trial.global_trial.mesh,
                    &source_levels,
                    ElasticTargetMode::HierarchyEdgeAreaDegree,
                )
                .unwrap();
        for start in [
            GeometryStartId::HierarchySpringEquilibrium,
            GeometryStartId::RingScaleInterpolation,
            GeometryStartId::DegreeAngleEquilibrium,
            GeometryStartId::SignedNormalPlus,
            GeometryStartId::SignedNormalMinus,
        ] {
            let mut left = trial.global_trial.mesh.mesh.clone();
            let mut right = trial.global_trial.mesh.mesh.clone();
            apply_geometry_start(&mut left, &patch, start).unwrap();
            apply_geometry_start(&mut right, &patch, start).unwrap();
            assert_eq!(left.vertices(), right.vertices());
            assert!(
                patch
                    .movable_compact_vertices
                    .iter()
                    .any(|&site| left.vertices()[site]
                        != trial.global_trial.mesh.mesh.vertices()[site]),
                "{start:?} must move at least one movable vertex"
            );
            for &site in &patch.fixed_compact_vertices {
                assert_eq!(
                    left.vertices()[site],
                    trial.global_trial.mesh.mesh.vertices()[site]
                );
            }
        }
    }

    #[test]
    fn angle_phase_energy_does_not_require_defined_dual() {
        let grid = MotherGrid::generate(4).unwrap();
        let face = grid.mesh.active_triangle_slots().next().unwrap();
        let triangle = grid.mesh.triangles()[face];
        let patch = ElasticPatch {
            topology: TransitionTopologyCandidate {
                component_id: 44,
                topology_id: 1,
                core_parents: Vec::new(),
                custom_transition_triangles: BTreeMap::new(),
                source_triangles: vec![triangle],
                source_active_vertices: triangle.to_vec(),
                source_degree_forecast: BTreeMap::new(),
            },
            reference_positions: grid.mesh.vertices().to_vec(),
            fixed_compact_vertices: vec![triangle[1], triangle[2]],
            movable_compact_vertices: vec![triangle[0]],
            guard_faces: vec![face],
            target_mode: ElasticTargetMode::TrialReference,
            target_field: ElasticTargetField::default(),
        };
        let context = EnergyContext {
            degrees: vertex_degrees(&grid.mesh),
            guard_edges: vec![(triangle[0], triangle[1]), (triangle[1], triangle[2])],
            guard_faces: vec![face],
            guard_seeds: vec![(usize::MAX, usize::MAX)],
            dual_pairs: vec![DualPair {
                face: usize::MAX,
                opposite: usize::MAX,
            }],
            derivatives: BTreeMap::new(),
            reference_dual_areas: BTreeMap::new(),
        };
        assert!(elastic_energy(
            &grid.mesh,
            &patch,
            ElasticBlockPhase::AngleFeasibility,
            &context,
        )
        .is_some());
    }

    #[test]
    fn negative_geometry_reaches_untangle_phase() {
        let (source, patch) = inverted_elastic_fixture();
        let context = EnergyContext::new(&source.mesh, &patch).unwrap();
        assert_eq!(
            initial_elastic_phase(&source, &patch).unwrap(),
            ElasticBlockPhase::Untangle
        );
        assert!(
            elastic_energy(&source.mesh, &patch, ElasticBlockPhase::Untangle, &context).is_some()
        );
    }

    #[test]
    fn negative_orientation_is_not_rejected_as_topology_no_go() {
        let (source, patch) = inverted_elastic_fixture();
        assert!(matches!(
            solve_elastic_patch(
                &source,
                patch,
                ElasticBlockLimits {
                    elastic_iterations: 0,
                },
            ),
            ElasticBlockOutcome::SearchBudgetExhausted { .. }
        ));
    }

    #[test]
    fn repairable_negative_fixture_leaves_untangle_phase() {
        let (source, patch) = inverted_elastic_fixture();
        match solve_elastic_patch(
            &source,
            patch,
            ElasticBlockLimits {
                elastic_iterations: 64,
            },
        ) {
            ElasticBlockOutcome::Certified(_) => {}
            ElasticBlockOutcome::ElasticNoImprovement { final_phase, .. }
            | ElasticBlockOutcome::SearchBudgetExhausted { final_phase, .. } => {
                assert_ne!(final_phase, ElasticBlockPhase::Untangle)
            }
            other => panic!("repairable negative fixture should enter elastic search: {other:?}"),
        }
    }

    #[test]
    fn cber_does_not_change_topology() {
        let (source, patch) = inverted_elastic_fixture();
        let triangles = source.mesh.triangles().to_vec();
        let neighbours = source.mesh.neighbours().to_vec();
        let outcome = solve_elastic_patch(
            &source,
            patch,
            ElasticBlockLimits {
                elastic_iterations: 1,
            },
        );
        if let ElasticBlockOutcome::Certified(trial) = outcome {
            assert_eq!(trial.mesh.mesh.triangles(), triangles);
            assert_eq!(trial.mesh.mesh.neighbours(), neighbours);
        }
        assert_eq!(source.mesh.triangles(), triangles);
        assert_eq!(source.mesh.neighbours(), neighbours);
    }

    #[test]
    fn untangle_energy_penalizes_crossing_edges() {
        let n = |x, y, z| normalized_point(CartesianPoint::new(x, y, z)).unwrap();
        let mesh = MeshState::from_parts(
            vec![
                CartesianPoint::new(0.0, 0.0, 0.0),
                CartesianPoint::new(0.0, 0.0, 0.0),
                n(1.0, 0.0, 0.0),
                n(0.0, 1.0, 0.0),
                n(0.7, 0.7, -1.0),
                n(0.7, 0.7, 1.0),
            ],
            vec![[0, 0, 0], [0, 0, 0], [2, 3, 4], [2, 3, 5]],
        )
        .unwrap();
        assert!(edge_crossing_penalty(&mesh, &[(2, 3), (4, 5)]) > 0.0);
    }

    fn inverted_elastic_fixture() -> (HierarchyLeafMesh, ElasticPatch) {
        let grid = MotherGrid::generate(8).unwrap();
        let seed = grid.mesh.active_triangle_slots().next().unwrap();
        let site = grid.mesh.triangles()[seed][0];
        let guard_faces = grid
            .mesh
            .active_triangle_slots()
            .filter(|&face| grid.mesh.triangles()[face].contains(&site))
            .collect::<Vec<_>>();
        let fixed_compact_vertices = guard_faces
            .iter()
            .flat_map(|&face| grid.mesh.triangles()[face])
            .filter(|&candidate| candidate != site)
            .collect::<BTreeSet<_>>();
        let patch = ElasticPatch {
            topology: TransitionTopologyCandidate {
                component_id: 43,
                topology_id: 0,
                core_parents: Vec::new(),
                custom_transition_triangles: BTreeMap::new(),
                source_triangles: guard_faces
                    .iter()
                    .map(|&face| grid.mesh.triangles()[face])
                    .collect(),
                source_active_vertices: std::iter::once(site)
                    .chain(fixed_compact_vertices.iter().copied())
                    .collect(),
                source_degree_forecast: BTreeMap::new(),
            },
            reference_positions: grid.mesh.vertices().to_vec(),
            fixed_compact_vertices: fixed_compact_vertices.into_iter().collect(),
            movable_compact_vertices: vec![site],
            guard_faces,
            target_mode: ElasticTargetMode::TrialReference,
            target_field: ElasticTargetField::default(),
        };
        let mut source = HierarchyLeafMesh {
            mesh: grid.mesh,
            triangle_addresses: grid.triangle_addresses,
            source_vertex_slots: (0..patch.reference_positions.len())
                .map(|site| (site >= 2).then_some(site))
                .collect(),
        };
        let face = patch.guard_faces[0];
        let triangle = source.mesh.triangles()[face];
        let [u, v] = triangle
            .into_iter()
            .filter(|&candidate| candidate != site)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let flipped = normalized_point(add_points(
            scale_point(
                cross(source.mesh.vertices()[u], source.mesh.vertices()[v]),
                -1.0,
            ),
            scale_point(
                add_points(source.mesh.vertices()[u], source.mesh.vertices()[v]),
                0.05,
            ),
        ))
        .unwrap();
        source.mesh.move_vertex(site, flipped);
        let guard_faces = patch.guard_faces.iter().copied().collect::<BTreeSet<_>>();
        assert!(!all_faces_positive(&source.mesh, &guard_faces));
        (source, patch)
    }

    fn full_finite_difference_gradient(
        mesh: &mut MeshState,
        patch: &ElasticPatch,
        phase: ElasticBlockPhase,
        initial_step: f64,
        context: &EnergyContext,
    ) -> Option<Vec<(usize, CartesianPoint)>> {
        let epsilon = (initial_step * 1.0e-3).clamp(1.0e-7, 1.0e-5);
        patch
            .movable_compact_vertices
            .iter()
            .copied()
            .map(|site| {
                let point = mesh.vertices()[site];
                let [first, second] = tangent_basis(point)?;
                let mut derivative = |direction: CartesianPoint| {
                    let plus = exponential_map(point, scale_point(direction, epsilon))?;
                    let minus = exponential_map(point, scale_point(direction, -epsilon))?;
                    mesh.move_vertex(site, plus);
                    let plus_energy = elastic_energy(mesh, patch, phase, context);
                    mesh.move_vertex(site, minus);
                    let minus_energy = elastic_energy(mesh, patch, phase, context);
                    mesh.move_vertex(site, point);
                    Some((plus_energy? - minus_energy?) / (2.0 * epsilon))
                };
                let d_first = derivative(first)?;
                let d_second = derivative(second)?;
                Some((
                    site,
                    add_points(scale_point(first, d_first), scale_point(second, d_second)),
                ))
            })
            .collect()
    }
}
