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
        let fixed_sources = physical_fixed_sources
            .iter()
            .copied()
            .chain(
                stratified
                    .coupled
                    .inner_guard
                    .vertices
                    .iter()
                    .map(|vertex| vertex.source_slot),
            )
            .chain(
                stratified
                    .coupled
                    .outer_guard
                    .vertices
                    .iter()
                    .map(|vertex| vertex.source_slot),
            )
            .chain(anchor_sources.iter().copied())
            .chain(fixed_position_sources.iter().copied())
            .collect::<BTreeSet<_>>();
        let fixed_compact_domain = source_set_to_compact(
            &source_to_compact,
            &fixed_sources,
            mesh,
            "free-interface fixed source vertex",
        )?
        .into_iter()
        .collect::<BTreeSet<_>>();
        let mut movable_sources = trial
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
            .filter(|source| !fixed_sources.contains(source) && !anchor_sources.contains(source))
            .collect::<BTreeSet<_>>();
        if movable_sources.is_empty() {
            return Err("full-polygon free-interface patch has no movable source vertex".into());
        }

        let mut movable_compact_vertices = source_set_to_compact(
            &source_to_compact,
            &movable_sources,
            mesh,
            "free-interface movable source vertex",
        )?
        .into_iter()
        .collect::<BTreeSet<_>>();
        loop {
            let guard_faces = incident_faces(mesh, &movable_compact_vertices);
            let next_movable = guard_faces
                .iter()
                .flat_map(|&face| mesh.triangles()[face])
                .filter(|site| !fixed_compact_domain.contains(site))
                .filter(|&site| source_slots[site].is_some())
                .collect::<BTreeSet<_>>();
            if next_movable == movable_compact_vertices {
                break;
            }
            movable_compact_vertices = next_movable;
            movable_sources = movable_compact_vertices
                .iter()
                .filter_map(|&compact| source_slots[compact])
                .collect();
        }
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
                .chain(fixed_sources.iter())
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
    if let Err(reason) = validate_patch(source, &patch) {
        return ElasticBlockOutcome::InvalidPatch { reason };
    }
    let certificate = Certificate::internal();
    let mut current = source.clone();
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
            }
        }
    };

    let mut energy = initial_energy;
    for iteration in 1..=limits.elastic_iterations {
        let phase = energy_phase(&certificate, &current.mesh, &guard_faces, &context);
        let Some(phase_energy) = elastic_energy(&current.mesh, &patch, phase, &context) else {
            return no_step(&current.mesh, iteration, energy);
        };
        energy = phase_energy;
        let Some(gradient) =
            finite_difference_gradient(&mut current.mesh, &patch, phase, initial_step, &context)
        else {
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
            if candidate_energy.is_some_and(|candidate_energy| {
                if matches!(phase, ElasticBlockPhase::Untangle) {
                    candidate_energy.is_finite() && candidate_energy < phase_energy
                } else {
                    candidate_energy < phase_energy - 1.0e-12 * phase_energy.abs().max(1.0)
                }
            }) {
                break candidate_energy;
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
    }
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
    use crate::mother_grid::MotherGrid;

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
