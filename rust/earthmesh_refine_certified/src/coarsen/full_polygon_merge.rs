//! Exact merge over PR40 full-polygon sector families.
//!
//! Topology only: no CBER/geometry. This path never calls the legacy two-chain
//! sector solver.

use super::full_polygon::{
    enumerate_stratified_full_polygon_families, FullPolygonFamily, FullPolygonTopologyKey,
};
use super::global_exact_merge::{
    fixed_triangles, materialize, mesh_edges, replace_fixed_link_contracts, solve_ears, EarSolve,
    GlobalExactMergeEvidence, GlobalExactMergeTrial, GlobalExactSelectedEar, MAX_EARS_PER_ANCHOR,
};
use super::{
    analyze_stratified_full_polygon_degree_reachability, build_stratified_annulus,
    solve_elastic_patch, ElasticBlockLimits, ElasticBlockOutcome, ElasticPatch,
    FullPolygonReachabilityEvidence, HierarchyComponent, RingAnchorKind, StratifiedAnnulus,
};
use crate::mother_grid::MotherGrid;
use std::collections::{BTreeMap, BTreeSet, HashSet};

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

#[derive(Debug, Clone, PartialEq, Eq)]
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
    pub selected_topology_keys: Vec<FullPolygonTopologyKey>,
    pub selected_ears: Vec<GlobalExactSelectedEar>,
    pub best_global_evidence: GlobalExactMergeEvidence,
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
    solve_full_polygon_merge_inner(source, component, limits.topology_states, None)
}

pub fn solve_full_polygon_merge_free_interface_cber(
    source: &MotherGrid,
    component: &HierarchyComponent,
    physical_fixed_sources: &BTreeSet<usize>,
    limits: FullPolygonCberLimits,
) -> FullPolygonMergeOutcome {
    solve_full_polygon_merge_inner(
        source,
        component,
        limits.topology_states,
        Some(FreeInterfaceCberConfig {
            elastic_iterations: limits.elastic_iterations,
            physical_fixed_sources,
        }),
    )
}

fn solve_full_polygon_merge_inner(
    source: &MotherGrid,
    component: &HierarchyComponent,
    topology_states: usize,
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
        selected_topology_keys: Vec::new(),
        selected_ears: Vec::new(),
        best_global_evidence: GlobalExactMergeEvidence {
            source_vertices: source.mesh.vertex_count(),
            source_faces: source.mesh.triangle_count(),
            ..Default::default()
        },
    };

    let mut stratified = match build_stratified_annulus(source, component) {
        Ok(v) => v,
        Err(err) => {
            return invalid(
                format!("stratified annulus rejected component: {err:?}"),
                evidence,
            )
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
    let fixed = match fixed_triangles(source, component) {
        Ok(v) => v,
        Err(err) => return invalid(err, evidence),
    };
    replace_fixed_link_contracts(&mut stratified, &fixed);
    let mut families = match enumerate_stratified_full_polygon_families(source, &stratified, &fixed)
    {
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
}

enum Step {
    Closed(Box<FullPolygonMergeTrial>),
    Invalid(String),
    Exhausted,
    GeometryUnknown,
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
            Step::GeometryUnknown => {
                self.evidence.states_examined = self.states;
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
                Step::NoSolution | Step::GeometryUnknown => {}
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
                let mesh = match materialize(self.source, self.component, &triangles) {
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
                        FreeInterfaceStep::RequiresDifferentTopology
                        | FreeInterfaceStep::GeometryUnknown => return Step::GeometryUnknown,
                        FreeInterfaceStep::BudgetExhausted => return Step::Exhausted,
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
    RequiresDifferentTopology,
    GeometryUnknown,
    BudgetExhausted,
    Invalid(String),
}

fn certify_free_interface_geometry(
    source: &MotherGrid,
    component: &HierarchyComponent,
    mut trial: FullPolygonMergeTrial,
    config: FreeInterfaceCberConfig<'_>,
) -> FreeInterfaceStep {
    let patch = match ElasticPatch::from_full_polygon_merge(
        source,
        component,
        &trial,
        config.physical_fixed_sources,
    ) {
        Ok(patch) => patch,
        Err(reason) => return FreeInterfaceStep::Invalid(reason),
    };
    let input_triangles = trial.global_trial.mesh.mesh.triangles().to_vec();
    let input_neighbours = trial.global_trial.mesh.mesh.neighbours().to_vec();
    match solve_elastic_patch(
        &trial.global_trial.mesh,
        patch,
        ElasticBlockLimits {
            elastic_iterations: config.elastic_iterations,
        },
    ) {
        ElasticBlockOutcome::Certified(elastic) => {
            if elastic.mesh.mesh.triangles() != input_triangles
                || elastic.mesh.mesh.neighbours() != input_neighbours
            {
                return FreeInterfaceStep::Invalid(
                    "free-interface CBER changed topology without exact-search ownership".into(),
                );
            }
            trial.global_trial.mesh = elastic.mesh.clone();
            FreeInterfaceStep::Certified(Box::new(trial))
        }
        ElasticBlockOutcome::RequiresDifferentTopology { .. } => {
            FreeInterfaceStep::RequiresDifferentTopology
        }
        ElasticBlockOutcome::ElasticNoImprovement { .. } => FreeInterfaceStep::GeometryUnknown,
        ElasticBlockOutcome::SearchBudgetExhausted { .. } => FreeInterfaceStep::BudgetExhausted,
        ElasticBlockOutcome::InvalidPatch { reason } => FreeInterfaceStep::Invalid(reason),
    }
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
}
