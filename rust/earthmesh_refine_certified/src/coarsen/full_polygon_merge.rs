//! Exact merge over PR40 full-polygon sector families.
//!
//! Topology only: no CBER/geometry. This path never calls the legacy two-chain
//! sector solver.

use super::elastic_block::GeometryFailureWitness;
use super::full_polygon::{
    enumerate_stratified_full_polygon_families, FullPolygonFamily, FullPolygonTopologyKey,
};
use super::global_exact_merge::{
    fixed_triangles_for_face_complex, materialize_for_face_complex, mesh_edges,
    replace_fixed_link_contracts, solve_ears, EarSolve, GlobalExactMergeEvidence,
    GlobalExactMergeTrial, GlobalExactSelectedEar, MAX_EARS_PER_ANCHOR,
};
use super::{
    analyze_stratified_full_polygon_degree_reachability, build_stratified_annulus,
    build_stratified_annulus_from_face_bands, build_stratified_topology_domain_v2,
    solve_elastic_patch_with_active_trust_start, solve_elastic_patch_with_margin_start,
    solve_elastic_patch_with_start, ElasticBlockLimits, ElasticBlockOutcome, ElasticBlockPhase,
    ElasticPatch, ElasticTargetMode, FaceBandPlan, FullPolygonReachabilityEvidence,
    GeometryDomainId, GeometryFailureDiagnostics, GeometryStartId, HierarchyComponent,
    RingAnchorKind, StratifiedAnnulus,
};
use crate::mother_grid::MotherGrid;
use earthmesh_mesh::CartesianPoint;
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fmt::Write as _,
};

type Edge = (usize, usize);
type TopologyEdgeCounts = Vec<(Edge, usize)>;
type TopologyAnchorNeighbours = Vec<(usize, usize)>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopologyFamilyId {
    FullPolygonAnchorEar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FullPolygonMergeLimits {
    pub topology_states: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FullPolygonCberLimits {
    pub topology_states: usize,
    pub elastic_iterations: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceBandAdapterVersion {
    LegacyV1,
    TopologyDomainV2,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FullPolygonGeometryFailureEvidence {
    pub topology_keys: Vec<FullPolygonTopologyKey>,
    pub start_id: &'static str,
    pub elastic_iterations: usize,
    pub initial_energy: f64,
    pub final_energy: f64,
    pub final_phase: ElasticBlockPhase,
    pub reason: String,
    pub failed_guard_face: Option<usize>,
    pub global_angle_degrees: Option<(f64, f64)>,
    pub guard_angle_degrees: Option<(f64, f64)>,
    pub negative_orientation_count: Option<usize>,
    pub crossing_count: Option<usize>,
    pub delaunay_violations: Option<usize>,
    pub invalid_voronoi_cells: Option<usize>,
    pub diagnostics: Option<GeometryFailureDiagnostics>,
    pub witness: Option<Box<GeometryFailureWitness>>,
}

impl FullPolygonGeometryFailureEvidence {
    pub fn signed_margin_degrees(&self) -> Option<f64> {
        self.global_angle_degrees
            .map(|(min, max)| (min - 40.2).min(79.8 - max))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FullPolygonMergeEvidence {
    pub family_id: TopologyFamilyId,
    pub sector_family_counts: Vec<usize>,
    pub retained_topology_counts: Vec<usize>,
    pub reachability: Option<FullPolygonReachabilityEvidence>,
    pub states_examined: usize,
    pub states_by_depth: Vec<usize>,
    pub ear_states_examined: usize,
    pub topology_candidates_closed: usize,
    pub ear_degree_feasible_candidates: usize,
    pub geometry_candidates_attempted: usize,
    pub last_geometry_failure: Option<FullPolygonGeometryFailureEvidence>,
    pub best_geometry_failure: Option<FullPolygonGeometryFailureEvidence>,
    pub geometry_failure_phase_counts: BTreeMap<ElasticBlockPhase, usize>,
    pub selected_topology_keys: Vec<FullPolygonTopologyKey>,
    pub selected_ears: Vec<GlobalExactSelectedEar>,
    pub best_global_evidence: GlobalExactMergeEvidence,
}

impl FullPolygonMergeEvidence {
    pub fn record_geometry_failure(&mut self, failure: FullPolygonGeometryFailureEvidence) {
        self.last_geometry_failure = Some(failure.clone());
        *self
            .geometry_failure_phase_counts
            .entry(failure.final_phase)
            .or_default() += 1;
        if self
            .best_geometry_failure
            .as_ref()
            .is_none_or(|best| geometry_failure_is_better(&failure, best))
        {
            self.best_geometry_failure = Some(failure);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FullPolygonMergeTrial {
    pub global_trial: GlobalExactMergeTrial,
    pub evidence: FullPolygonMergeEvidence,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FullPolygonMergeOutcome {
    Closed(Box<FullPolygonMergeTrial>),
    TopologyFamilyExhaustedNoSolution(FullPolygonMergeEvidence),
    SearchBudgetExhausted(FullPolygonMergeEvidence),
    InvalidInput {
        reason: String,
        evidence: FullPolygonMergeEvidence,
    },
}

pub fn solve_full_polygon_merge(
    source: &MotherGrid,
    component: &HierarchyComponent,
    limits: FullPolygonMergeLimits,
) -> FullPolygonMergeOutcome {
    solve_full_polygon_merge_inner(
        source,
        component,
        limits.topology_states,
        None,
        FaceBandAdapterVersion::LegacyV1,
        None,
    )
}

pub fn solve_full_polygon_merge_from_face_bands(
    source: &MotherGrid,
    component: &HierarchyComponent,
    plan: &FaceBandPlan,
    limits: FullPolygonMergeLimits,
) -> FullPolygonMergeOutcome {
    solve_full_polygon_merge_inner(
        source,
        component,
        limits.topology_states,
        Some(plan),
        FaceBandAdapterVersion::LegacyV1,
        None,
    )
}

pub fn solve_full_polygon_merge_from_face_bands_v2(
    source: &MotherGrid,
    component: &HierarchyComponent,
    plan: &FaceBandPlan,
    limits: FullPolygonMergeLimits,
) -> FullPolygonMergeOutcome {
    solve_full_polygon_merge_inner(
        source,
        component,
        limits.topology_states,
        Some(plan),
        FaceBandAdapterVersion::TopologyDomainV2,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn solve_full_polygon_merge_from_face_bands_with_geometry_witness(
    source: &MotherGrid,
    component: &HierarchyComponent,
    plan: &FaceBandPlan,
    inherited_witness: &GeometryFailureWitness,
    physical_fixed_sources: &BTreeSet<usize>,
    limits: FullPolygonCberLimits,
    target_mode: ElasticTargetMode,
    source_levels: Option<&[Option<usize>]>,
    starts: &[GeometryStartId],
    domain_id: GeometryDomainId,
) -> FullPolygonMergeOutcome {
    solve_full_polygon_merge_inner(
        source,
        component,
        limits.topology_states,
        Some(plan),
        FaceBandAdapterVersion::LegacyV1,
        Some(FreeInterfaceCberConfig {
            elastic_iterations: limits.elastic_iterations,
            physical_fixed_sources,
            target_mode,
            source_levels,
            starts,
            solver_mode: GeometrySolverMode::ActiveTangentTrust,
            domain_id,
            face_band_plan: Some(plan),
            inherited_witness: Some(inherited_witness),
        }),
    )
}

pub fn solve_full_polygon_merge_free_interface_cber(
    source: &MotherGrid,
    component: &HierarchyComponent,
    physical_fixed_sources: &BTreeSet<usize>,
    limits: FullPolygonCberLimits,
) -> FullPolygonMergeOutcome {
    solve_full_polygon_merge_free_interface_cber_with_targets(
        source,
        component,
        physical_fixed_sources,
        limits,
        ElasticTargetMode::TrialReference,
        None,
    )
}

pub fn solve_full_polygon_merge_free_interface_cber_with_targets(
    source: &MotherGrid,
    component: &HierarchyComponent,
    physical_fixed_sources: &BTreeSet<usize>,
    limits: FullPolygonCberLimits,
    target_mode: ElasticTargetMode,
    source_levels: Option<&[Option<usize>]>,
) -> FullPolygonMergeOutcome {
    solve_full_polygon_merge_inner(
        source,
        component,
        limits.topology_states,
        None,
        FaceBandAdapterVersion::LegacyV1,
        Some(FreeInterfaceCberConfig {
            elastic_iterations: limits.elastic_iterations,
            physical_fixed_sources,
            target_mode,
            source_levels,
            starts: &[GeometryStartId::MaterializedSource],
            solver_mode: GeometrySolverMode::FiniteDifferenceElastic,
            domain_id: GeometryDomainId::CurrentAnnulus,
            face_band_plan: None,
            inherited_witness: None,
        }),
    )
}

pub fn solve_full_polygon_merge_free_interface_cber_with_targets_and_starts(
    source: &MotherGrid,
    component: &HierarchyComponent,
    physical_fixed_sources: &BTreeSet<usize>,
    limits: FullPolygonCberLimits,
    target_mode: ElasticTargetMode,
    source_levels: Option<&[Option<usize>]>,
    starts: &[GeometryStartId],
) -> FullPolygonMergeOutcome {
    solve_full_polygon_merge_inner(
        source,
        component,
        limits.topology_states,
        None,
        FaceBandAdapterVersion::LegacyV1,
        Some(FreeInterfaceCberConfig {
            elastic_iterations: limits.elastic_iterations,
            physical_fixed_sources,
            target_mode,
            source_levels,
            starts,
            solver_mode: GeometrySolverMode::MarginFiniteDifferenceLexicographic,
            domain_id: GeometryDomainId::CurrentAnnulus,
            face_band_plan: None,
            inherited_witness: None,
        }),
    )
}

pub fn solve_full_polygon_merge_free_interface_cber_with_targets_and_active_trust_starts(
    source: &MotherGrid,
    component: &HierarchyComponent,
    physical_fixed_sources: &BTreeSet<usize>,
    limits: FullPolygonCberLimits,
    target_mode: ElasticTargetMode,
    source_levels: Option<&[Option<usize>]>,
    starts: &[GeometryStartId],
) -> FullPolygonMergeOutcome {
    solve_full_polygon_merge_inner(
        source,
        component,
        limits.topology_states,
        None,
        FaceBandAdapterVersion::LegacyV1,
        Some(FreeInterfaceCberConfig {
            elastic_iterations: limits.elastic_iterations,
            physical_fixed_sources,
            target_mode,
            source_levels,
            starts,
            solver_mode: GeometrySolverMode::ActiveTangentTrust,
            domain_id: GeometryDomainId::CurrentAnnulus,
            face_band_plan: None,
            inherited_witness: None,
        }),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn solve_full_polygon_merge_free_interface_cber_with_targets_active_trust_starts_and_domain(
    source: &MotherGrid,
    component: &HierarchyComponent,
    physical_fixed_sources: &BTreeSet<usize>,
    limits: FullPolygonCberLimits,
    target_mode: ElasticTargetMode,
    source_levels: Option<&[Option<usize>]>,
    starts: &[GeometryStartId],
    domain_id: GeometryDomainId,
) -> FullPolygonMergeOutcome {
    solve_full_polygon_merge_inner(
        source,
        component,
        limits.topology_states,
        None,
        FaceBandAdapterVersion::LegacyV1,
        Some(FreeInterfaceCberConfig {
            elastic_iterations: limits.elastic_iterations,
            physical_fixed_sources,
            target_mode,
            source_levels,
            starts,
            solver_mode: GeometrySolverMode::ActiveTangentTrust,
            domain_id,
            face_band_plan: None,
            inherited_witness: None,
        }),
    )
}

fn solve_full_polygon_merge_inner(
    source: &MotherGrid,
    component: &HierarchyComponent,
    topology_states: usize,
    face_band_plan: Option<&FaceBandPlan>,
    face_band_adapter: FaceBandAdapterVersion,
    free_interface_cber: Option<FreeInterfaceCberConfig<'_>>,
) -> FullPolygonMergeOutcome {
    let mut evidence = FullPolygonMergeEvidence {
        family_id: TopologyFamilyId::FullPolygonAnchorEar,
        sector_family_counts: Vec::new(),
        retained_topology_counts: Vec::new(),
        reachability: None,
        states_examined: 0,
        states_by_depth: Vec::new(),
        ear_states_examined: 0,
        topology_candidates_closed: 0,
        ear_degree_feasible_candidates: 0,
        geometry_candidates_attempted: 0,
        last_geometry_failure: None,
        best_geometry_failure: None,
        geometry_failure_phase_counts: BTreeMap::new(),
        selected_topology_keys: Vec::new(),
        selected_ears: Vec::new(),
        best_global_evidence: GlobalExactMergeEvidence {
            source_vertices: source.mesh.vertex_count(),
            source_faces: source.mesh.triangle_count(),
            ..Default::default()
        },
    };

    let mut stratified = match face_band_plan.map_or_else(
        || build_stratified_annulus(source, component),
        |plan| match face_band_adapter {
            FaceBandAdapterVersion::LegacyV1 => {
                build_stratified_annulus_from_face_bands(source, component, plan)
            }
            FaceBandAdapterVersion::TopologyDomainV2 => {
                build_stratified_topology_domain_v2(source, component, plan)
            }
        },
    ) {
        Ok(v) => v,
        Err(err) => {
            let adapter = match face_band_adapter {
                FaceBandAdapterVersion::LegacyV1 => "stratified annulus",
                FaceBandAdapterVersion::TopologyDomainV2 => "stratified topology-domain V2",
            };
            return invalid(format!("{adapter} rejected component: {err:?}"), evidence);
        }
    };
    let reachability =
        match analyze_stratified_full_polygon_degree_reachability(source, component, &stratified) {
            Ok(v) => v,
            Err(err) => return invalid(err, evidence),
        };
    evidence.reachability = Some(reachability.clone());
    if reachability.outcome != super::DegreeDomainOutcome::NecessaryFeasible {
        return invalid(
            format!(
                "full-polygon degree reachability is {:?}",
                reachability.outcome
            ),
            evidence,
        );
    }
    let fixed = match fixed_triangles_for_face_complex(
        source,
        component,
        &stratified.coupled.annulus_face_slots,
    ) {
        Ok(v) => v,
        Err(err) => return invalid(err, evidence),
    };
    replace_fixed_link_contracts(&mut stratified, &fixed);
    let ordering_source = match free_interface_cber.and_then(|config| config.inherited_witness) {
        Some(witness) => match mother_with_witness_positions(source, witness) {
            Ok(value) => Some(value),
            Err(reason) => return invalid(reason, evidence),
        },
        None => None,
    };
    let family_source = ordering_source.as_ref().unwrap_or(source);
    let mut families =
        match enumerate_stratified_full_polygon_families(family_source, &stratified, &fixed) {
            Ok(v) => v,
            Err(err) => return invalid(err, evidence),
        };
    evidence.sector_family_counts = families.iter().map(|f| f.topology_count).collect();
    evidence.best_global_evidence.sector_variant_counts = evidence.sector_family_counts.clone();
    if families.is_empty() || families.iter().any(|f| f.topologies.is_empty()) {
        return invalid("full-polygon sector product is empty".into(), evidence);
    }
    if families.len() != reachability.sector_signatures.len() {
        return invalid("reachability/family sector count mismatch".into(), evidence);
    }
    for (family, signatures) in families.iter_mut().zip(&reachability.sector_signatures) {
        let supported = signatures
            .iter()
            .map(|signature| signature.contributions.clone())
            .collect::<BTreeSet<_>>();
        family.topologies.retain(|topology| {
            supported.contains(
                &topology
                    .vertex_incidences
                    .iter()
                    .map(|(&vertex, &count)| (vertex, count))
                    .collect::<Vec<_>>(),
            )
        });
        let expected = signatures
            .iter()
            .map(|signature| signature.member_topology_count)
            .sum::<usize>();
        if family.topologies.len() != expected {
            return invalid(
                format!(
                    "sector {} signature members mismatch: expected {expected}, got {}",
                    family.sector_id,
                    family.topologies.len()
                ),
                evidence,
            );
        }
    }
    evidence.retained_topology_counts = families.iter().map(|f| f.topologies.len()).collect();
    let topology_edges = families
        .iter()
        .map(|family| {
            family
                .topologies
                .iter()
                .map(|topology| {
                    edge_counts(&topology.topology_key.triangles)
                        .into_iter()
                        .collect()
                })
                .collect()
        })
        .collect();

    Search {
        source,
        component,
        stratified: &stratified,
        fixed_edges: mesh_edges(&fixed),
        fixed,
        families,
        topology_edges,
        topology_anchor_neighbours: Vec::new(),
        limit: topology_states,
        states: 0,
        evidence,
        selected: Vec::new(),
        edge_counts: BTreeMap::new(),
        triangle_keys: BTreeSet::new(),
        degrees: BTreeMap::new(),
        link_edges: BTreeMap::new(),
        duplicate_link: false,
        anchors: BTreeSet::new(),
        ear_touchable: BTreeSet::new(),
        ear_capacities: BTreeMap::new(),
        vertex_owners: BTreeMap::new(),
        edge_providers: BTreeMap::new(),
        seen: HashSet::new(),
        free_interface_cber,
    }
    .init()
    .run()
}

struct Search<'a> {
    source: &'a MotherGrid,
    component: &'a HierarchyComponent,
    stratified: &'a StratifiedAnnulus,
    fixed_edges: BTreeSet<(usize, usize)>,
    fixed: Vec<[usize; 3]>,
    families: Vec<FullPolygonFamily>,
    topology_edges: Vec<Vec<TopologyEdgeCounts>>,
    topology_anchor_neighbours: Vec<Vec<TopologyAnchorNeighbours>>,
    limit: usize,
    states: usize,
    evidence: FullPolygonMergeEvidence,
    selected: Vec<Option<usize>>,
    edge_counts: BTreeMap<(usize, usize), usize>,
    triangle_keys: BTreeSet<[usize; 3]>,
    degrees: BTreeMap<usize, usize>,
    link_edges: BTreeMap<usize, BTreeSet<(usize, usize)>>,
    duplicate_link: bool,
    anchors: BTreeSet<usize>,
    ear_touchable: BTreeSet<usize>,
    ear_capacities: BTreeMap<usize, usize>,
    vertex_owners: BTreeMap<usize, BTreeSet<usize>>,
    edge_providers: BTreeMap<(usize, usize), Vec<(usize, usize)>>,
    seen: HashSet<Vec<Option<usize>>>,
    free_interface_cber: Option<FreeInterfaceCberConfig<'a>>,
}

#[derive(Clone, Copy)]
struct FreeInterfaceCberConfig<'a> {
    elastic_iterations: usize,
    physical_fixed_sources: &'a BTreeSet<usize>,
    target_mode: ElasticTargetMode,
    source_levels: Option<&'a [Option<usize>]>,
    starts: &'a [GeometryStartId],
    solver_mode: GeometrySolverMode,
    domain_id: GeometryDomainId,
    face_band_plan: Option<&'a FaceBandPlan>,
    inherited_witness: Option<&'a GeometryFailureWitness>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GeometrySolverMode {
    FiniteDifferenceElastic,
    MarginFiniteDifferenceLexicographic,
    ActiveTangentTrust,
}

enum Step {
    Closed(Box<FullPolygonMergeTrial>),
    Invalid(String),
    Exhausted,
    GeometryUnknown(Box<FullPolygonGeometryFailureEvidence>),
    NoSolution,
}

impl Search<'_> {
    fn init(mut self) -> Self {
        self.selected = vec![None; self.families.len()];
        for triangle in self.fixed.clone() {
            self.add_triangle(triangle);
        }
        self.anchors = self
            .stratified
            .link_contracts
            .iter()
            .filter_map(|(&v, c)| {
                matches!(c.anchor_kind, RingAnchorKind::IcosahedronPentagon { .. }).then_some(v)
            })
            .collect();
        self.topology_anchor_neighbours = self
            .families
            .iter()
            .map(|family| {
                family
                    .topologies
                    .iter()
                    .map(|topology| {
                        let mut neighbours = topology
                            .triangles
                            .iter()
                            .flat_map(|triangle| {
                                let anchors = triangle
                                    .vertices
                                    .iter()
                                    .copied()
                                    .filter(|vertex| self.anchors.contains(vertex))
                                    .collect::<Vec<_>>();
                                anchors.into_iter().flat_map(move |anchor| {
                                    triangle
                                        .vertices
                                        .into_iter()
                                        .filter(move |&vertex| vertex != anchor)
                                        .map(move |vertex| (anchor, vertex))
                                })
                            })
                            .collect::<Vec<_>>();
                        neighbours.sort_unstable();
                        neighbours.dedup();
                        neighbours
                    })
                    .collect()
            })
            .collect();
        let mut vertex_anchors = BTreeMap::<usize, BTreeSet<usize>>::new();
        for triangle in self
            .families
            .iter()
            .flat_map(|family| &family.topologies)
            .flat_map(|topology| topology.triangles.iter().map(|triangle| triangle.vertices))
        {
            let triangle_anchors = triangle
                .iter()
                .copied()
                .filter(|vertex| self.anchors.contains(vertex))
                .collect::<Vec<_>>();
            for vertex in triangle {
                if !self.anchors.contains(&vertex) {
                    vertex_anchors
                        .entry(vertex)
                        .or_default()
                        .extend(triangle_anchors.iter().copied());
                }
            }
        }
        self.ear_capacities = vertex_anchors
            .into_iter()
            .map(|(vertex, anchors)| (vertex, anchors.len() * MAX_EARS_PER_ANCHOR))
            .collect();
        self.ear_touchable = self.ear_capacities.keys().copied().collect();
        for (sector, family) in self.families.iter().enumerate() {
            for (choice, topology) in family.topologies.iter().enumerate() {
                for &vertex in topology.vertex_incidences.keys() {
                    self.vertex_owners.entry(vertex).or_default().insert(sector);
                }
                for &(edge, count) in &self.topology_edges[sector][choice] {
                    if count == 1 {
                        self.edge_providers
                            .entry(edge)
                            .or_default()
                            .push((sector, choice));
                    }
                }
            }
        }
        self
    }

    fn run(mut self) -> FullPolygonMergeOutcome {
        match self.visit() {
            Step::Closed(trial) => FullPolygonMergeOutcome::Closed(trial),
            Step::Invalid(reason) => invalid(reason, self.evidence),
            Step::Exhausted => {
                self.evidence.states_examined = self.states;
                FullPolygonMergeOutcome::SearchBudgetExhausted(self.evidence)
            }
            Step::GeometryUnknown(failure) => {
                self.evidence.states_examined = self.states;
                self.evidence.record_geometry_failure(*failure);
                FullPolygonMergeOutcome::SearchBudgetExhausted(self.evidence)
            }
            Step::NoSolution => {
                self.evidence.states_examined = self.states;
                no_solution_outcome(self.evidence)
            }
        }
    }

    fn visit(&mut self) -> Step {
        let Some(choices) = self.next_choices() else {
            return self.evaluate();
        };
        if choices.is_empty() {
            return Step::NoSolution;
        }
        for (sector, choice) in choices {
            self.selected[sector] = Some(choice);
            if self.seen.contains(&self.selected) {
                self.selected[sector] = None;
                continue;
            }
            if self.states >= self.limit {
                self.selected[sector] = None;
                return Step::Exhausted;
            }
            self.seen.insert(self.selected.clone());
            self.states += 1;
            let depth = self.selected.iter().flatten().count();
            if self.evidence.states_by_depth.len() <= depth {
                self.evidence.states_by_depth.resize(depth + 1, 0);
            }
            self.evidence.states_by_depth[depth] += 1;
            self.add_topology(sector, choice);
            let result = if self.partial_ok() {
                self.visit()
            } else {
                Step::NoSolution
            };
            self.remove_topology(sector, choice);
            self.selected[sector] = None;
            match result {
                Step::NoSolution => {}
                Step::GeometryUnknown(failure) => self.evidence.record_geometry_failure(*failure),
                terminal => return terminal,
            }
        }
        Step::NoSolution
    }

    fn next_choices(&self) -> Option<Vec<(usize, usize)>> {
        let mut compatible = vec![Vec::<usize>::new(); self.families.len()];
        let mut best = None::<Vec<(usize, usize)>>;
        for (sector, selected) in self.selected.iter().enumerate() {
            if selected.is_some() {
                continue;
            }
            let choices = self.families[sector]
                .topologies
                .iter()
                .enumerate()
                .filter_map(|(choice, _)| self.topology_fits(sector, choice).then_some(choice))
                .collect::<Vec<_>>();
            compatible[sector] = choices.clone();
            if best
                .as_ref()
                .is_none_or(|current| choices.len() < current.len())
            {
                best = Some(choices.into_iter().map(|choice| (sector, choice)).collect());
            }
        }
        let mut best = best?;
        if !self.compatible_degree_bounds_hold(&compatible) {
            return Some(Vec::new());
        }
        for (&edge, &count) in &self.edge_counts {
            if count != 1 {
                continue;
            }
            let providers =
                compatible_edge_providers(edge, &self.edge_providers, &self.selected, &compatible);
            if providers.len() < best.len() {
                best = providers;
            }
            if best.is_empty() {
                break;
            }
        }
        best.sort_by_key(|&(sector, choice)| self.choice_score(sector, choice));
        Some(best)
    }

    fn choice_score(&self, sector: usize, choice: usize) -> (usize, usize, usize, usize) {
        let topology = &self.families[sector].topologies[choice];
        let excess = topology
            .vertex_incidences
            .iter()
            .filter(|(vertex, _)| !self.anchors.contains(vertex))
            .map(|(&vertex, &count)| {
                (self.degrees.get(&vertex).copied().unwrap_or_default() + count as usize)
                    .saturating_sub(7)
            })
            .sum();
        let anchor_count: usize = self
            .anchors
            .iter()
            .map(|anchor| {
                topology
                    .vertex_incidences
                    .get(anchor)
                    .copied()
                    .unwrap_or_default() as usize
            })
            .sum();
        (excess, anchor_count, sector, choice)
    }

    fn topology_fits(&self, sector: usize, choice: usize) -> bool {
        let topology = &self.families[sector].topologies[choice];
        topology
            .topology_key
            .triangles
            .iter()
            .all(|t| !self.triangle_keys.contains(t))
            && self.topology_edges[sector][choice]
                .iter()
                .all(|&(edge, n)| self.edge_counts.get(&edge).copied().unwrap_or(0) + n <= 2)
            && self.anchors.iter().all(|&anchor| {
                let current = self.degrees.get(&anchor).copied().unwrap_or_default();
                let added = topology
                    .vertex_incidences
                    .get(&anchor)
                    .copied()
                    .unwrap_or_default() as usize;
                let max = self.stratified.link_contracts[&anchor].target_degree_max as usize
                    + MAX_EARS_PER_ANCHOR;
                current + added <= max
            })
    }

    fn compatible_degree_bounds_hold(&self, compatible: &[Vec<usize>]) -> bool {
        let vertices = self
            .degrees
            .keys()
            .copied()
            .chain(self.vertex_owners.keys().copied())
            .collect::<BTreeSet<_>>();
        let mut raw_bounds = BTreeMap::new();
        for &vertex in &vertices {
            let mut lower = self.degrees.get(&vertex).copied().unwrap_or_default();
            let mut upper = lower;
            for (sector, selected) in self.selected.iter().enumerate() {
                if selected.is_some()
                    || !self
                        .vertex_owners
                        .get(&vertex)
                        .is_some_and(|owners| owners.contains(&sector))
                {
                    continue;
                }
                let Some((min, max)) = min_max(compatible[sector].iter().map(|&choice| {
                    self.families[sector].topologies[choice]
                        .vertex_incidences
                        .get(&vertex)
                        .copied()
                        .unwrap_or_default() as usize
                })) else {
                    return false;
                };
                lower += min;
                upper += max;
            }
            raw_bounds.insert(vertex, (lower, upper));
        }

        let mut anchor_demands = BTreeMap::new();
        for &anchor in &self.anchors {
            let (lower, upper) = raw_bounds[&anchor];
            let contract = &self.stratified.link_contracts[&anchor];
            let target_min = contract.target_degree_min as usize;
            let target_max = contract.target_degree_max as usize;
            if lower > target_max + MAX_EARS_PER_ANCHOR || upper < target_min {
                return false;
            }
            anchor_demands.insert(
                anchor,
                upper.saturating_sub(target_max).min(MAX_EARS_PER_ANCHOR),
            );
        }
        let max_ears = anchor_demands.values().sum::<usize>();
        let mut possible_neighbours = self
            .anchors
            .iter()
            .map(|&anchor| {
                let current = self
                    .link_edges
                    .get(&anchor)
                    .into_iter()
                    .flat_map(|edges| edges.iter().flat_map(|&(a, b)| [a, b]))
                    .collect::<BTreeSet<_>>();
                (anchor, current)
            })
            .collect::<BTreeMap<_, _>>();
        for (sector, selected) in self.selected.iter().enumerate() {
            if selected.is_some() {
                continue;
            }
            for &choice in &compatible[sector] {
                for &(anchor, vertex) in &self.topology_anchor_neighbours[sector][choice] {
                    possible_neighbours
                        .entry(anchor)
                        .or_default()
                        .insert(vertex);
                }
            }
        }

        let mut min_deficit = 0usize;
        let mut min_excess = 0usize;
        for (&vertex, &(lower, upper)) in &raw_bounds {
            if !self.anchors.contains(&vertex) {
                let capacity = anchor_demands
                    .iter()
                    .filter(|(anchor, _)| {
                        possible_neighbours
                            .get(anchor)
                            .is_some_and(|neighbours| neighbours.contains(&vertex))
                    })
                    .map(|(_, demand)| demand)
                    .sum::<usize>();
                if !degree_range_repairable(lower, upper, capacity, 5, 7) {
                    return false;
                }
                min_deficit += 5usize.saturating_sub(upper + capacity);
                min_excess += lower.saturating_sub(7 + capacity);
            }
        }
        min_deficit <= max_ears * 2 && min_excess <= max_ears
    }

    fn partial_ok(&self) -> bool {
        !self.duplicate_link
            && self.partial_links_ok()
            && self.partial_degree_bounds_ok()
            && self.partial_ear_capacity_ok()
    }

    fn partial_ear_capacity_ok(&self) -> bool {
        self.degrees
            .iter()
            .filter(|(vertex, _)| !self.anchors.contains(vertex))
            .map(|(_, &degree)| degree.saturating_sub(7))
            .sum::<usize>()
            <= self.anchors.len() * MAX_EARS_PER_ANCHOR
    }

    fn partial_degree_bounds_ok(&self) -> bool {
        let remaining = self.selected.iter().position(Option::is_none).is_some();
        let vertices = self
            .degrees
            .keys()
            .copied()
            .chain(
                self.families
                    .iter()
                    .flat_map(|f| f.incidence_domains.keys().copied()),
            )
            .collect::<BTreeSet<_>>();
        vertices.into_iter().all(|vertex| {
            if self.anchors.contains(&vertex) {
                return true;
            }
            let ear_capacity = self
                .ear_capacities
                .get(&vertex)
                .copied()
                .unwrap_or_default();
            let mut lo = *self.degrees.get(&vertex).unwrap_or(&0);
            let mut hi = lo;
            for (sector, selected) in self.selected.iter().enumerate() {
                if selected.is_some() {
                    continue;
                }
                if let Some(domain) = self.families[sector].incidence_domains.get(&vertex) {
                    lo += usize::from(*domain.iter().next().unwrap());
                    hi += usize::from(*domain.iter().next_back().unwrap());
                }
            }
            if remaining {
                degree_range_repairable(lo, hi, ear_capacity, 5, 7)
            } else {
                degree_range_repairable(lo, lo, ear_capacity, 5, 7)
            }
        })
    }

    fn partial_links_ok(&self) -> bool {
        self.link_edges.iter().all(|(&vertex, edges)| {
            let mut node_degrees = BTreeMap::<usize, usize>::new();
            for &(a, b) in edges {
                *node_degrees.entry(a).or_default() += 1;
                *node_degrees.entry(b).or_default() += 1;
            }
            if node_degrees.values().any(|&degree| degree > 2) {
                return false;
            }
            let has_remaining_owner = self
                .vertex_owners
                .get(&vertex)
                .is_some_and(|owners| owners.iter().any(|&sector| self.selected[sector].is_none()));
            !has_remaining_owner || !has_cycle_component(edges)
        })
    }

    fn evaluate(&mut self) -> Step {
        if self.edge_counts.values().any(|&n| n != 2)
            || self
                .link_edges
                .values()
                .any(|edges| !is_single_cycle(edges))
        {
            return Step::NoSolution;
        }
        self.evidence.topology_candidates_closed += 1;
        if !self.partial_degree_bounds_ok() || !self.ear_degree_bounds_hold() {
            return Step::NoSolution;
        }
        self.evidence.ear_degree_feasible_candidates += 1;
        let global_topology_id = self.states as u64;
        let mut mutable = Vec::new();
        let mut keys = Vec::new();
        for (sector, selected) in self.selected.iter().enumerate() {
            let topology = &self.families[sector].topologies[selected.expect("all selected")];
            keys.push(topology.topology_key.clone());
            mutable.extend(topology.triangles.iter().copied().map(|mut t| {
                t.topology_id = global_topology_id;
                t
            }));
        }
        let mut global = self.evidence.best_global_evidence.clone();
        global.states_examined = self.states;
        let mut ear_states = 0;
        match solve_ears(
            self.source,
            self.stratified,
            &self.fixed_edges,
            &self.fixed,
            mutable,
            &mut global,
            &mut ear_states,
        ) {
            EarSolve::Solved { triangles, ears } => {
                global.selected_ears = ears.clone();
                global.ear_states_examined += ear_states;
                self.evidence.ear_states_examined += ear_states;
                self.evidence.selected_topology_keys = keys.clone();
                self.evidence.selected_ears = ears.clone();
                self.evidence.best_global_evidence = global.clone();
                if self.free_interface_cber.is_some() {
                    self.evidence.geometry_candidates_attempted += 1;
                }
                let mesh = match materialize_for_face_complex(
                    self.source,
                    self.component,
                    &self.stratified.coupled.annulus_face_slots,
                    &triangles,
                ) {
                    Ok(mesh) => mesh,
                    Err(reason) => return Step::Invalid(reason),
                };
                let mut evidence = self.evidence.clone();
                evidence.states_examined = self.states;
                evidence.best_global_evidence = global.clone();
                let mut trial = FullPolygonMergeTrial {
                    global_trial: GlobalExactMergeTrial {
                        mesh,
                        custom_triangles: triangles.into_iter().map(|t| t.vertices).collect(),
                        evidence: global,
                    },
                    evidence,
                };
                if let Some(config) = self.free_interface_cber {
                    match certify_free_interface_geometry(
                        self.source,
                        self.component,
                        trial,
                        config,
                    ) {
                        FreeInterfaceStep::Certified(certified) => trial = *certified,
                        FreeInterfaceStep::RequiresDifferentTopology(mut failure)
                        | FreeInterfaceStep::GeometryUnknown(mut failure) => {
                            failure.topology_keys = keys.clone();
                            return Step::GeometryUnknown(Box::new(failure));
                        }
                        FreeInterfaceStep::Invalid(reason) => return Step::Invalid(reason),
                    }
                }
                Step::Closed(Box::new(trial))
            }
            EarSolve::NoSolution => {
                global.ear_states_examined += ear_states;
                self.evidence.ear_states_examined += ear_states;
                self.evidence.best_global_evidence = global;
                Step::NoSolution
            }
            EarSolve::Invalid(reason) => Step::Invalid(reason),
        }
    }

    fn ear_degree_bounds_hold(&self) -> bool {
        let Some(demands) = self
            .anchors
            .iter()
            .map(|anchor| {
                let target = self.stratified.link_contracts[anchor].target_degree_max as usize;
                self.degrees
                    .get(anchor)
                    .copied()
                    .unwrap_or_default()
                    .checked_sub(target)
                    .map(|demand| (*anchor, demand))
            })
            .collect::<Option<BTreeMap<_, _>>>()
        else {
            return false;
        };
        let ears = demands.values().sum::<usize>();
        if ears > self.anchors.len() * MAX_EARS_PER_ANCHOR {
            return false;
        }
        let mut deficit = 0usize;
        let mut excess = 0usize;
        for (&vertex, &degree) in self
            .degrees
            .iter()
            .filter(|(vertex, _)| !self.anchors.contains(vertex))
        {
            let capacity = demands
                .iter()
                .filter(|(anchor, _)| {
                    self.link_edges
                        .get(anchor)
                        .is_some_and(|edges| edges.iter().any(|&(a, b)| a == vertex || b == vertex))
                })
                .map(|(_, demand)| demand)
                .sum::<usize>();
            if !degree_range_repairable(degree, degree, capacity, 5, 7) {
                return false;
            }
            deficit += 5usize.saturating_sub(degree);
            excess += degree.saturating_sub(7);
        }
        deficit <= ears * 2 && excess <= ears
    }

    fn add_topology(&mut self, sector: usize, choice: usize) {
        let triangles = self.families[sector].topologies[choice]
            .topology_key
            .triangles
            .clone();
        for triangle in triangles {
            self.add_triangle(triangle);
        }
    }

    fn remove_topology(&mut self, sector: usize, choice: usize) {
        let triangles = self.families[sector].topologies[choice]
            .topology_key
            .triangles
            .clone();
        for triangle in triangles {
            self.remove_triangle(triangle);
        }
        self.rebuild_links();
    }

    fn add_triangle(&mut self, triangle: [usize; 3]) {
        let triangle = canonical(triangle);
        self.triangle_keys.insert(triangle);
        for edge in triangle_edges(triangle) {
            *self.edge_counts.entry(edge).or_default() += 1;
        }
        for vertex in triangle {
            *self.degrees.entry(vertex).or_default() += 1;
        }
        for (vertex, link) in triangle_links(triangle) {
            if !self.link_edges.entry(vertex).or_default().insert(link) {
                self.duplicate_link = true;
            }
        }
    }

    fn remove_triangle(&mut self, triangle: [usize; 3]) {
        let triangle = canonical(triangle);
        self.triangle_keys.remove(&triangle);
        for edge in triangle_edges(triangle) {
            let n = self.edge_counts.get_mut(&edge).unwrap();
            *n -= 1;
            if *n == 0 {
                self.edge_counts.remove(&edge);
            }
        }
        for vertex in triangle {
            let n = self.degrees.get_mut(&vertex).unwrap();
            *n -= 1;
            if *n == 0 {
                self.degrees.remove(&vertex);
            }
        }
    }

    fn rebuild_links(&mut self) {
        self.link_edges.clear();
        self.duplicate_link = false;
        let triangles = self.triangle_keys.iter().copied().collect::<Vec<_>>();
        for triangle in triangles {
            for (vertex, link) in triangle_links(triangle) {
                if !self.link_edges.entry(vertex).or_default().insert(link) {
                    self.duplicate_link = true;
                }
            }
        }
    }
}

enum FreeInterfaceStep {
    Certified(Box<FullPolygonMergeTrial>),
    RequiresDifferentTopology(FullPolygonGeometryFailureEvidence),
    GeometryUnknown(FullPolygonGeometryFailureEvidence),
    Invalid(String),
}

#[allow(clippy::too_many_arguments)]
fn record_start_failure(
    best_failure: &mut Option<(bool, FullPolygonGeometryFailureEvidence)>,
    requires_different_topology: bool,
    start_id: GeometryStartId,
    elastic_iterations: usize,
    initial_energy: f64,
    final_energy: f64,
    final_phase: ElasticBlockPhase,
    reason: String,
    failed_guard_face: Option<usize>,
    global_angle_degrees: Option<(f64, f64)>,
    guard_angle_degrees: Option<(f64, f64)>,
    diagnostics: Option<GeometryFailureDiagnostics>,
    witness: Box<GeometryFailureWitness>,
) {
    let failure = FullPolygonGeometryFailureEvidence {
        topology_keys: Vec::new(),
        start_id: start_id.as_str(),
        elastic_iterations,
        initial_energy,
        final_energy,
        final_phase,
        reason,
        failed_guard_face,
        global_angle_degrees,
        guard_angle_degrees,
        negative_orientation_count: None,
        crossing_count: None,
        delaunay_violations: None,
        invalid_voronoi_cells: None,
        diagnostics,
        witness: Some(witness),
    };
    if best_failure
        .as_ref()
        .is_none_or(|(_, best)| geometry_failure_is_better(&failure, best))
    {
        *best_failure = Some((requires_different_topology, failure));
    }
}

fn mother_with_witness_positions(
    source: &MotherGrid,
    witness: &GeometryFailureWitness,
) -> Result<MotherGrid, String> {
    let positions = witness_source_positions(witness)?;
    let mut transferred = source.clone();
    for (source_slot, point) in positions {
        if source_slot >= source.addresses.len() {
            return Err(format!(
                "inherited witness has invalid source slot {source_slot}"
            ));
        }
        if transferred.mesh.is_vertex_live(source_slot) {
            transferred.mesh.move_vertex(source_slot, point);
        }
    }
    Ok(transferred)
}

fn witness_source_positions(
    witness: &GeometryFailureWitness,
) -> Result<BTreeMap<usize, CartesianPoint>, String> {
    if witness.mesh.source_vertex_slots.len() != witness.mesh.mesh.vertices().len() {
        return Err("inherited witness source-slot map does not match its mesh".into());
    }
    let mut positions = BTreeMap::new();
    for (compact, source_slot) in witness.mesh.source_vertex_slots.iter().copied().enumerate() {
        let Some(source_slot) = source_slot else {
            continue;
        };
        if positions
            .insert(source_slot, witness.mesh.mesh.vertices()[compact])
            .is_some()
        {
            return Err(format!(
                "inherited witness has duplicate source slot {source_slot}"
            ));
        }
    }
    Ok(positions)
}

fn transfer_witness_positions(
    target: &mut super::HierarchyLeafMesh,
    witness: &GeometryFailureWitness,
) -> Result<(usize, usize), String> {
    if target.source_vertex_slots.len() != target.mesh.vertices().len() {
        return Err("topology-transfer target source-slot map does not match its mesh".into());
    }
    let positions = witness_source_positions(witness)?;
    let mut common = 0;
    let mut fallback = 0;
    for (compact, source_slot) in target.source_vertex_slots.iter().copied().enumerate() {
        if let Some(point) = source_slot.and_then(|slot| positions.get(&slot).copied()) {
            target.mesh.move_vertex(compact, point);
            common += 1;
        } else {
            fallback += 1;
        }
    }
    Ok((common, fallback))
}

fn certify_free_interface_geometry(
    source: &MotherGrid,
    component: &HierarchyComponent,
    mut trial: FullPolygonMergeTrial,
    config: FreeInterfaceCberConfig<'_>,
) -> FreeInterfaceStep {
    if config
        .inherited_witness
        .is_some_and(|witness| witness.patch.domain_id != config.domain_id)
    {
        return FreeInterfaceStep::Invalid(
            "inherited witness and face-band target domains do not match".into(),
        );
    }
    let patch_result = match config.face_band_plan {
        Some(plan) => ElasticPatch::from_face_band_full_polygon_merge_with_domain(
            source,
            component,
            plan,
            &trial,
            config.physical_fixed_sources,
            config.domain_id,
        ),
        None => ElasticPatch::from_full_polygon_merge_with_domain(
            source,
            component,
            &trial,
            config.physical_fixed_sources,
            config.domain_id,
        ),
    };
    let mut patch = match patch_result {
        Ok(patch) => patch,
        Err(reason) => return FreeInterfaceStep::Invalid(reason),
    };
    if config.target_mode != ElasticTargetMode::TrialReference {
        let Some(source_levels) = config.source_levels else {
            return FreeInterfaceStep::Invalid(
                "hierarchy elastic targets require source levels".into(),
            );
        };
        patch = match patch.with_hierarchy_targets(
            source,
            &trial.global_trial.mesh,
            source_levels,
            config.target_mode,
        ) {
            Ok(patch) => patch,
            Err(reason) => return FreeInterfaceStep::Invalid(reason),
        };
        if let Some(plan) = config.face_band_plan {
            patch = match patch.with_face_band_trace_targets(
                source,
                &trial.global_trial.mesh,
                component,
                plan,
                source_levels,
            ) {
                Ok(patch) => patch,
                Err(reason) => return FreeInterfaceStep::Invalid(reason),
            };
        }
    }
    let mut initial_mesh = trial.global_trial.mesh.clone();
    if let Some(witness) = config.inherited_witness {
        if let Err(reason) = transfer_witness_positions(&mut initial_mesh, witness) {
            return FreeInterfaceStep::Invalid(reason);
        }
    }
    let input_triangles = trial.global_trial.mesh.mesh.triangles().to_vec();
    let input_neighbours = trial.global_trial.mesh.mesh.neighbours().to_vec();
    let mut best_failure = None::<(bool, FullPolygonGeometryFailureEvidence)>;
    for &start_id in config.starts {
        let limits = ElasticBlockLimits {
            elastic_iterations: config.elastic_iterations,
        };
        let outcome = match config.solver_mode {
            GeometrySolverMode::FiniteDifferenceElastic => {
                solve_elastic_patch_with_start(&initial_mesh, patch.clone(), limits, start_id)
            }
            GeometrySolverMode::MarginFiniteDifferenceLexicographic => {
                solve_elastic_patch_with_margin_start(
                    &initial_mesh,
                    patch.clone(),
                    limits,
                    start_id,
                )
            }
            GeometrySolverMode::ActiveTangentTrust => solve_elastic_patch_with_active_trust_start(
                &initial_mesh,
                patch.clone(),
                limits,
                start_id,
            ),
        };
        match outcome {
            ElasticBlockOutcome::Certified(elastic) => {
                if elastic.mesh.mesh.triangles() != input_triangles
                    || elastic.mesh.mesh.neighbours() != input_neighbours
                {
                    return FreeInterfaceStep::Invalid(
                        "free-interface CBER changed topology without exact-search ownership"
                            .into(),
                    );
                }
                trial.global_trial.mesh = elastic.mesh.clone();
                return FreeInterfaceStep::Certified(Box::new(trial));
            }
            ElasticBlockOutcome::RequiresDifferentTopology {
                elastic_iterations,
                initial_energy,
                final_energy,
                final_phase,
                reason,
                failed_guard_face,
                global_angle_degrees,
                guard_angle_degrees,
                diagnostics,
                witness,
            } => record_start_failure(
                &mut best_failure,
                true,
                start_id,
                elastic_iterations,
                initial_energy,
                final_energy,
                final_phase,
                reason,
                failed_guard_face,
                global_angle_degrees,
                guard_angle_degrees,
                diagnostics,
                witness,
            ),
            ElasticBlockOutcome::ElasticNoImprovement {
                elastic_iterations,
                initial_energy,
                final_energy,
                final_phase,
                reason,
                failed_guard_face,
                global_angle_degrees,
                guard_angle_degrees,
                diagnostics,
                witness,
            }
            | ElasticBlockOutcome::SearchBudgetExhausted {
                elastic_iterations,
                initial_energy,
                final_energy,
                final_phase,
                reason,
                failed_guard_face,
                global_angle_degrees,
                guard_angle_degrees,
                diagnostics,
                witness,
            } => record_start_failure(
                &mut best_failure,
                false,
                start_id,
                elastic_iterations,
                initial_energy,
                final_energy,
                final_phase,
                reason,
                failed_guard_face,
                global_angle_degrees,
                guard_angle_degrees,
                diagnostics,
                witness,
            ),
            ElasticBlockOutcome::InvalidPatch { reason } => {
                return FreeInterfaceStep::Invalid(reason)
            }
        }
    }
    match best_failure {
        Some((true, failure)) => FreeInterfaceStep::RequiresDifferentTopology(failure),
        Some((false, failure)) => FreeInterfaceStep::GeometryUnknown(failure),
        None => FreeInterfaceStep::Invalid("free-interface CBER has no geometry starts".into()),
    }
}

pub fn frozen_n6_geometry_evidence_json(
    outcome: &FullPolygonMergeOutcome,
    fixture_fingerprint: u64,
    topology_limit: usize,
    elastic_iterations: usize,
    commit_sha: Option<&str>,
    starts: &[&str],
) -> String {
    frozen_n6_geometry_evidence_json_with_target_mode(
        outcome,
        fixture_fingerprint,
        topology_limit,
        elastic_iterations,
        commit_sha,
        ElasticTargetMode::TrialReference,
        starts,
    )
}

pub fn frozen_n6_geometry_evidence_json_with_target_mode(
    outcome: &FullPolygonMergeOutcome,
    fixture_fingerprint: u64,
    topology_limit: usize,
    elastic_iterations: usize,
    commit_sha: Option<&str>,
    target_mode: ElasticTargetMode,
    starts: &[&str],
) -> String {
    frozen_n6_geometry_evidence_json_with_solver_mode(
        outcome,
        fixture_fingerprint,
        topology_limit,
        elastic_iterations,
        commit_sha,
        target_mode,
        starts,
        "FiniteDifferenceElastic",
    )
}

#[allow(clippy::too_many_arguments)]
pub fn frozen_n6_geometry_evidence_json_with_solver_mode(
    outcome: &FullPolygonMergeOutcome,
    fixture_fingerprint: u64,
    topology_limit: usize,
    elastic_iterations: usize,
    commit_sha: Option<&str>,
    target_mode: ElasticTargetMode,
    starts: &[&str],
    solver_mode: &str,
) -> String {
    frozen_n6_geometry_evidence_json_with_solver_domain(
        outcome,
        fixture_fingerprint,
        topology_limit,
        elastic_iterations,
        commit_sha,
        target_mode,
        starts,
        solver_mode,
        GeometryDomainId::CurrentAnnulus,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn frozen_n6_geometry_evidence_json_with_solver_domain(
    outcome: &FullPolygonMergeOutcome,
    fixture_fingerprint: u64,
    topology_limit: usize,
    elastic_iterations: usize,
    commit_sha: Option<&str>,
    target_mode: ElasticTargetMode,
    starts: &[&str],
    solver_mode: &str,
    domain_id: GeometryDomainId,
) -> String {
    let (kind, evidence, certified) = match outcome {
        FullPolygonMergeOutcome::Closed(trial) => ("Certified", &trial.evidence, true),
        FullPolygonMergeOutcome::SearchBudgetExhausted(evidence) => {
            ("ContinuousSearchIncomplete", evidence, false)
        }
        FullPolygonMergeOutcome::TopologyFamilyExhaustedNoSolution(evidence) => {
            ("RequiresDifferentTopology", evidence, false)
        }
        FullPolygonMergeOutcome::InvalidInput { evidence, .. } => ("InvalidPatch", evidence, false),
    };
    let best = evidence.best_geometry_failure.as_ref();
    let mut json = String::new();
    write!(
        json,
        "{{\"schema_version\":1,\"commit_sha\":{},\"fixture_fingerprint\":{},\"topology_limit\":{},\"elastic_iteration_limit\":{},\"target_mode\":\"{}\",\"solver_mode\":\"{}\",\"domain_id\":\"{}\",\"starts\":[{}],\"topology_candidates_closed\":{},\"geometry_candidates_attempted\":{},\"best_signed_margin_deg\":{},\"best_topology_key\":{},\"best_start_id\":{},\"phase_counts\":{},\"last_failure\":{},\"best_failure\":{},\"outcome\":\"{}\",\"certified\":{}}}",
        option_json(commit_sha),
        fixture_fingerprint,
        topology_limit,
        elastic_iterations,
        target_mode.as_str(),
        json_escape(solver_mode),
        domain_id.as_str(),
        starts
            .iter()
            .map(|s| format!("\"{}\"", json_escape(s)))
            .collect::<Vec<_>>()
            .join(","),
        evidence.topology_candidates_closed,
        evidence.geometry_candidates_attempted,
        json_number_or_null(best.and_then(|failure| failure.signed_margin_degrees())),
        best.map(|failure| topology_keys_json(&failure.topology_keys))
            .unwrap_or_else(|| "null".into()),
        best.map(|failure| format!("\"{}\"", failure.start_id))
            .unwrap_or_else(|| "null".into()),
        phase_counts_json(evidence),
        evidence
            .last_geometry_failure
            .as_ref()
            .map(failure_json)
            .unwrap_or_else(|| "null".into()),
        best.map(failure_json).unwrap_or_else(|| "null".into()),
        kind,
        certified
    )
    .unwrap();
    json
}

fn phase_counts_json(evidence: &FullPolygonMergeEvidence) -> String {
    let body = evidence
        .geometry_failure_phase_counts
        .iter()
        .map(|(phase, count)| format!("\"{:?}\":{}", phase, count))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{body}}}")
}

fn failure_json(failure: &FullPolygonGeometryFailureEvidence) -> String {
    format!(
        "{{\"topology_key\":{},\"start_id\":\"{}\",\"elastic_iterations\":{},\"initial_energy\":{},\"final_energy\":{},\"final_phase\":\"{:?}\",\"reason\":\"{}\",\"failed_guard_face\":{},\"global_angle_degrees\":{},\"guard_angle_degrees\":{},\"signed_margin_deg\":{},\"negative_orientation_count\":{},\"crossing_count\":{},\"delaunay_violations\":{},\"invalid_voronoi_cells\":{},\"diagnostics\":{}}}",
        topology_keys_json(&failure.topology_keys),
        failure.start_id,
        failure.elastic_iterations,
        finite_json_number(failure.initial_energy),
        finite_json_number(failure.final_energy),
        failure.final_phase,
        json_escape(&failure.reason),
        failure
            .failed_guard_face
            .map(|v| v.to_string())
            .unwrap_or_else(|| "null".into()),
        angle_pair_json(failure.global_angle_degrees),
        angle_pair_json(failure.guard_angle_degrees),
        json_number_or_null(failure.signed_margin_degrees()),
        usize_option_json(failure.negative_orientation_count),
        usize_option_json(failure.crossing_count),
        usize_option_json(failure.delaunay_violations),
        usize_option_json(failure.invalid_voronoi_cells),
        diagnostics_json(failure.diagnostics.as_ref()),
    )
}

fn diagnostics_json(diagnostics: Option<&GeometryFailureDiagnostics>) -> String {
    let Some(diagnostics) = diagnostics else {
        return "null".into();
    };
    format!(
        "{{\"movement_distribution\":{},\"worst_triangle_guard_distance\":{},\"active_boundary_constraint_ratio\":{}}}",
        movement_distribution_json(&diagnostics.movement_distribution),
        usize_option_json(diagnostics.worst_triangle_guard_distance),
        active_boundary_constraint_ratio_json(diagnostics.active_boundary_constraint_ratio.as_ref())
    )
}

fn movement_distribution_json(distribution: &super::MovementDistribution) -> String {
    format!(
        "{{\"count\":{},\"min\":{},\"p50\":{},\"p90\":{},\"max\":{},\"sum\":{}}}",
        distribution.count,
        finite_json_number(distribution.min),
        finite_json_number(distribution.p50),
        finite_json_number(distribution.p90),
        finite_json_number(distribution.max),
        finite_json_number(distribution.sum)
    )
}

fn active_boundary_constraint_ratio_json(
    ratio: Option<&super::ActiveBoundaryConstraintRatio>,
) -> String {
    let Some(ratio) = ratio else {
        return "null".into();
    };
    format!(
        "{{\"numerator\":{},\"denominator\":{},\"ratio\":{}}}",
        ratio.numerator,
        ratio.denominator,
        finite_json_number(ratio.ratio)
    )
}

fn topology_keys_json(keys: &[FullPolygonTopologyKey]) -> String {
    let keys = keys
        .iter()
        .map(|key| {
            let triangles = key
                .triangles
                .iter()
                .map(|t| format!("[{},{},{}]", t[0], t[1], t[2]))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"sector_id\":{},\"triangles\":[{}]}}",
                key.sector_id, triangles
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("[{keys}]")
}

fn angle_pair_json(value: Option<(f64, f64)>) -> String {
    value
        .map(|(min, max)| format!("[{},{}]", finite_json_number(min), finite_json_number(max)))
        .unwrap_or_else(|| "null".into())
}

fn finite_json_number(value: f64) -> String {
    if value.is_finite() {
        format!("{value:.12}")
    } else {
        "null".into()
    }
}

fn json_number_or_null(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite())
        .map(finite_json_number)
        .unwrap_or_else(|| "null".into())
}

fn usize_option_json(value: Option<usize>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".into())
}

fn option_json(value: Option<&str>) -> String {
    value
        .map(|value| format!("\"{}\"", json_escape(value)))
        .unwrap_or_else(|| "null".into())
}

fn json_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn geometry_failure_is_better(
    candidate: &FullPolygonGeometryFailureEvidence,
    current: &FullPolygonGeometryFailureEvidence,
) -> bool {
    if known_count_key(candidate.negative_orientation_count)
        != known_count_key(current.negative_orientation_count)
    {
        return known_count_key(candidate.negative_orientation_count)
            < known_count_key(current.negative_orientation_count);
    }
    if known_count_key(candidate.crossing_count) != known_count_key(current.crossing_count) {
        return known_count_key(candidate.crossing_count) < known_count_key(current.crossing_count);
    }
    let candidate_margin = candidate
        .signed_margin_degrees()
        .unwrap_or(f64::NEG_INFINITY);
    let current_margin = current.signed_margin_degrees().unwrap_or(f64::NEG_INFINITY);
    if candidate_margin.total_cmp(&current_margin) != std::cmp::Ordering::Equal {
        return candidate_margin > current_margin;
    }
    if known_count_key(candidate.delaunay_violations)
        != known_count_key(current.delaunay_violations)
    {
        return known_count_key(candidate.delaunay_violations)
            < known_count_key(current.delaunay_violations);
    }
    if known_count_key(candidate.invalid_voronoi_cells)
        != known_count_key(current.invalid_voronoi_cells)
    {
        return known_count_key(candidate.invalid_voronoi_cells)
            < known_count_key(current.invalid_voronoi_cells);
    }
    (&candidate.topology_keys, candidate.start_id) < (&current.topology_keys, current.start_id)
}

fn known_count_key(value: Option<usize>) -> (bool, usize) {
    value
        .map(|value| (false, value))
        .unwrap_or((true, usize::MAX))
}

fn no_solution_outcome(evidence: FullPolygonMergeEvidence) -> FullPolygonMergeOutcome {
    if evidence.geometry_candidates_attempted > 0 {
        FullPolygonMergeOutcome::SearchBudgetExhausted(evidence)
    } else {
        FullPolygonMergeOutcome::TopologyFamilyExhaustedNoSolution(evidence)
    }
}

fn invalid(reason: String, evidence: FullPolygonMergeEvidence) -> FullPolygonMergeOutcome {
    FullPolygonMergeOutcome::InvalidInput { reason, evidence }
}

fn compatible_edge_providers(
    edge: Edge,
    providers: &BTreeMap<Edge, Vec<(usize, usize)>>,
    selected: &[Option<usize>],
    compatible: &[Vec<usize>],
) -> Vec<(usize, usize)> {
    providers
        .get(&edge)
        .into_iter()
        .flatten()
        .copied()
        .filter(|&(sector, choice)| {
            selected[sector].is_none() && compatible[sector].binary_search(&choice).is_ok()
        })
        .collect()
}

fn edge_counts(triangles: &[[usize; 3]]) -> BTreeMap<(usize, usize), usize> {
    let mut out = BTreeMap::new();
    for triangle in triangles {
        for edge in triangle_edges(*triangle) {
            *out.entry(edge).or_default() += 1;
        }
    }
    out
}

fn min_max(values: impl Iterator<Item = usize>) -> Option<(usize, usize)> {
    values.fold(None, |bounds, value| {
        Some(match bounds {
            Some((min, max)) => (min.min(value), max.max(value)),
            None => (value, value),
        })
    })
}

fn degree_range_repairable(
    lower: usize,
    upper: usize,
    capacity: usize,
    legal_min: usize,
    legal_max: usize,
) -> bool {
    lower.saturating_sub(capacity) <= legal_max && upper.saturating_add(capacity) >= legal_min
}

fn triangle_edges([a, b, c]: [usize; 3]) -> [(usize, usize); 3] {
    [sorted(a, b), sorted(b, c), sorted(c, a)]
}

fn triangle_links([a, b, c]: [usize; 3]) -> [(usize, (usize, usize)); 3] {
    [(a, sorted(b, c)), (b, sorted(a, c)), (c, sorted(a, b))]
}

fn has_cycle_component(edges: &BTreeSet<(usize, usize)>) -> bool {
    let mut adjacency = BTreeMap::<usize, BTreeSet<usize>>::new();
    for &(a, b) in edges {
        adjacency.entry(a).or_default().insert(b);
        adjacency.entry(b).or_default().insert(a);
    }
    let mut unseen = adjacency.keys().copied().collect::<BTreeSet<_>>();
    while let Some(start) = unseen.pop_first() {
        let mut nodes = BTreeSet::from([start]);
        let mut stack = vec![start];
        let mut degree_sum = 0usize;
        while let Some(node) = stack.pop() {
            degree_sum += adjacency[&node].len();
            for &next in &adjacency[&node] {
                if nodes.insert(next) {
                    unseen.remove(&next);
                    stack.push(next);
                }
            }
        }
        if degree_sum / 2 >= nodes.len() {
            return true;
        }
    }
    false
}

fn is_single_cycle(edges: &BTreeSet<(usize, usize)>) -> bool {
    let mut adjacency = BTreeMap::<usize, BTreeSet<usize>>::new();
    for &(a, b) in edges {
        adjacency.entry(a).or_default().insert(b);
        adjacency.entry(b).or_default().insert(a);
    }
    let Some(&start) = adjacency.keys().next() else {
        return false;
    };
    if adjacency.values().any(|neighbours| neighbours.len() != 2) {
        return false;
    }
    let mut seen = BTreeSet::from([start]);
    let mut stack = vec![start];
    while let Some(node) = stack.pop() {
        for &next in &adjacency[&node] {
            if seen.insert(next) {
                stack.push(next);
            }
        }
    }
    seen.len() == adjacency.len()
}

fn canonical(mut triangle: [usize; 3]) -> [usize; 3] {
    triangle.sort_unstable();
    triangle
}

fn sorted(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coarsen::{HierarchyLeafMesh, TransitionTopologyCandidate};

    #[test]
    fn partial_degree_upper_bound_prunes() {
        assert!(degree_range_repairable(8, 8, 1, 5, 7));
        assert!(!degree_range_repairable(9, 9, 1, 5, 7));
    }

    #[test]
    fn partial_degree_lower_bound_prunes() {
        assert!(degree_range_repairable(4, 4, 1, 5, 7));
        assert!(!degree_range_repairable(3, 3, 1, 5, 7));
    }

    #[test]
    fn partial_link_small_cycle_prunes() {
        let cycle = BTreeSet::from([(1, 2), (1, 3), (2, 3)]);
        let path = BTreeSet::from([(1, 2), (2, 3)]);
        assert!(has_cycle_component(&cycle));
        assert!(!has_cycle_component(&path));
    }

    #[test]
    fn final_link_requires_one_connected_cycle() {
        let cycle = BTreeSet::from([(1, 2), (1, 3), (2, 3)]);
        let path = BTreeSet::from([(1, 2), (2, 3)]);
        let disconnected = BTreeSet::from([(1, 2), (1, 3), (2, 3), (4, 5), (4, 6), (5, 6)]);
        assert!(is_single_cycle(&cycle));
        assert!(!is_single_cycle(&path));
        assert!(!is_single_cycle(&disconnected));
    }

    #[test]
    fn geometry_attempts_make_complete_family_no_solution_unknown() {
        let evidence = |geometry_candidates_attempted| FullPolygonMergeEvidence {
            family_id: TopologyFamilyId::FullPolygonAnchorEar,
            sector_family_counts: Vec::new(),
            retained_topology_counts: Vec::new(),
            reachability: None,
            states_examined: 7,
            states_by_depth: Vec::new(),
            ear_states_examined: 0,
            topology_candidates_closed: 0,
            ear_degree_feasible_candidates: 0,
            geometry_candidates_attempted,
            last_geometry_failure: None,
            best_geometry_failure: None,
            geometry_failure_phase_counts: BTreeMap::new(),
            selected_topology_keys: Vec::new(),
            selected_ears: Vec::new(),
            best_global_evidence: GlobalExactMergeEvidence::default(),
        };
        assert!(matches!(
            no_solution_outcome(evidence(0)),
            FullPolygonMergeOutcome::TopologyFamilyExhaustedNoSolution(_)
        ));
        assert!(matches!(
            no_solution_outcome(evidence(1)),
            FullPolygonMergeOutcome::SearchBudgetExhausted(_)
        ));
    }

    #[test]
    fn edge_provider_prunes() {
        let edge = (10, 20);
        let providers = BTreeMap::from([(edge, vec![(0, 1), (1, 0), (1, 2)])]);
        let selected = vec![Some(0), None];
        let compatible = vec![Vec::new(), vec![0, 1]];
        assert_eq!(
            compatible_edge_providers(edge, &providers, &selected, &compatible),
            vec![(1, 0)]
        );
        assert!(compatible_edge_providers((30, 40), &providers, &selected, &compatible).is_empty());
    }

    #[test]
    fn pf_w2_geometry_keeps_incumbent() {
        let source = MotherGrid::generate(1).unwrap();
        let mut inherited = HierarchyLeafMesh {
            mesh: source.mesh.clone(),
            triangle_addresses: source.triangle_addresses.clone(),
            source_vertex_slots: (0..source.mesh.vertices().len()).map(Some).collect(),
        };
        let inherited_point = source.mesh.vertices()[3];
        inherited.mesh.move_vertex(2, inherited_point);
        let witness = GeometryFailureWitness {
            patch: empty_patch(&inherited),
            mesh: inherited,
        };
        let mut target = witness.mesh.clone();
        target.source_vertex_slots[2] = Some(2);
        target.source_vertex_slots[3] = None;
        let fallback = target.mesh.vertices()[3];

        let (common, fallback_count) = transfer_witness_positions(&mut target, &witness).unwrap();
        assert_eq!(
            (common, fallback_count),
            (target.mesh.vertices().len() - 1, 1)
        );
        assert_eq!(target.mesh.vertices()[2], inherited_point);
        assert_eq!(target.mesh.vertices()[3], fallback);

        let mut duplicate = witness;
        duplicate.mesh.source_vertex_slots[3] = Some(2);
        assert!(witness_source_positions(&duplicate).is_err());
    }

    fn empty_patch(mesh: &HierarchyLeafMesh) -> ElasticPatch {
        ElasticPatch {
            domain_id: GeometryDomainId::PlusTwoOrdinaryRings,
            topology: TransitionTopologyCandidate {
                component_id: 0,
                topology_id: 0,
                core_parents: Vec::new(),
                custom_transition_triangles: BTreeMap::new(),
                source_triangles: Vec::new(),
                source_active_vertices: Vec::new(),
                source_degree_forecast: BTreeMap::new(),
            },
            reference_positions: mesh.mesh.vertices().to_vec(),
            fixed_compact_vertices: Vec::new(),
            movable_compact_vertices: Vec::new(),
            guard_faces: Vec::new(),
            target_mode: ElasticTargetMode::TrialReference,
            target_field: Default::default(),
        }
    }
}
