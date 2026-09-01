//! Deterministic retained coarse-parent subset planning for Frozen N6 recovery.

use super::{
    annulus::{parent_by_source_face, parent_graph},
    build_face_band_problem, build_stratified_annulus_from_face_bands,
    face_band::solve_exact_face_bands_with_filter,
    solve_full_polygon_merge_from_face_bands, FaceBandLimits, FaceBandOutcomeKind, FaceBandPlan,
    FaceBandSolveOutcome, FullPolygonMergeLimits, FullPolygonMergeOutcome, FullPolygonMergeTrial,
    HierarchyComponent,
};
use crate::{mother_grid::TriangleAddress, MotherGrid};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const MAX_EXACT_CORE_PARENTS: usize = 20;

#[derive(Debug, Clone, PartialEq)]
pub struct RetainedCoreCandidate {
    pub retained_parents: BTreeSet<TriangleAddress>,
    pub released_parents: BTreeSet<TriangleAddress>,
    pub retained_components: usize,
    pub retained_boundary_edges: usize,
    pub violation_influence_score: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetainedCoreSearchPlan {
    pub initial_coarse_parents: BTreeSet<TriangleAddress>,
    pub parent_adjacency: BTreeMap<TriangleAddress, BTreeSet<TriangleAddress>>,
    pub candidates: Vec<RetainedCoreCandidate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetainedCoreTopologyLimits {
    pub face_band_states: u64,
    pub topology_states: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetainedCoreTopologyOutcomeKind {
    Closed,
    TopologyFamilyExhaustedNoSolution,
    SearchBudgetExhausted,
    InvalidInput,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetainedCoreTopologyEvidence {
    pub retained_parents: BTreeSet<TriangleAddress>,
    pub released_parents: BTreeSet<TriangleAddress>,
    pub face_band_outcome: FaceBandOutcomeKind,
    pub face_band_states: u64,
    pub topology_outcome: RetainedCoreTopologyOutcomeKind,
    pub topology_states: usize,
    pub selected_topologies: usize,
    pub vertices: Option<usize>,
    pub edges: Option<usize>,
    pub faces: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RetainedCoreTopologyOutcome {
    Closed {
        component: HierarchyComponent,
        face_band_plan: Box<FaceBandPlan>,
        trial: Box<FullPolygonMergeTrial>,
        evidence: RetainedCoreTopologyEvidence,
    },
    TopologyFamilyExhaustedNoSolution(RetainedCoreTopologyEvidence),
    SearchBudgetExhausted(RetainedCoreTopologyEvidence),
    InvalidInput {
        reason: String,
        evidence: RetainedCoreTopologyEvidence,
    },
}

impl RetainedCoreTopologyOutcome {
    pub fn evidence(&self) -> &RetainedCoreTopologyEvidence {
        match self {
            Self::Closed { evidence, .. }
            | Self::TopologyFamilyExhaustedNoSolution(evidence)
            | Self::SearchBudgetExhausted(evidence)
            | Self::InvalidInput { evidence, .. } => evidence,
        }
    }
}

impl RetainedCoreSearchPlan {
    pub fn connected_candidates(&self) -> impl Iterator<Item = &RetainedCoreCandidate> {
        self.candidates
            .iter()
            .filter(|candidate| candidate.retained_components == 1)
    }
}

pub fn plan_retained_core_subsets(
    source: &MotherGrid,
    initial_core: &BTreeSet<TriangleAddress>,
    violation_parents: &BTreeSet<TriangleAddress>,
) -> Result<RetainedCoreSearchPlan, String> {
    if initial_core.is_empty() {
        return Err("retained-core planning requires at least one coarse parent".into());
    }
    if initial_core.len() > MAX_EXACT_CORE_PARENTS {
        return Err(format!(
            "exact retained-core planning is limited to {MAX_EXACT_CORE_PARENTS} parents"
        ));
    }

    let parent_by_face = parent_by_source_face(source).map_err(|error| format!("{error:?}"))?;
    let graph = parent_graph(source, &parent_by_face).map_err(|error| format!("{error:?}"))?;
    for parent in initial_core.iter().chain(violation_parents) {
        if !graph.contains_key(parent) {
            return Err(format!(
                "retained-core parent {parent:?} is absent from the source hierarchy"
            ));
        }
    }

    let parent_adjacency = initial_core
        .iter()
        .copied()
        .map(|parent| {
            let neighbours = graph[&parent].intersection(initial_core).copied().collect();
            (parent, neighbours)
        })
        .collect::<BTreeMap<_, _>>();
    let distances = graph_distances(&graph, violation_parents);
    let parents = initial_core.iter().copied().collect::<Vec<_>>();
    let mut candidates = Vec::with_capacity(1usize << parents.len());
    for mask in 0..(1usize << parents.len()) {
        let retained_parents = parents
            .iter()
            .enumerate()
            .filter_map(|(index, &parent)| ((mask & (1usize << index)) != 0).then_some(parent))
            .collect::<BTreeSet<_>>();
        let released_parents = initial_core
            .difference(&retained_parents)
            .copied()
            .collect::<BTreeSet<_>>();
        candidates.push(RetainedCoreCandidate {
            retained_components: component_count(&retained_parents, &parent_adjacency),
            retained_boundary_edges: retained_parents
                .iter()
                .map(|parent| {
                    graph[parent]
                        .iter()
                        .filter(|neighbour| !retained_parents.contains(neighbour))
                        .count()
                })
                .sum(),
            violation_influence_score: released_parents
                .iter()
                .filter_map(|parent| distances.get(parent))
                .map(|distance| 1.0 / (1.0 + *distance as f64))
                .sum(),
            retained_parents,
            released_parents,
        });
    }
    candidates.sort_by(|left, right| {
        right
            .retained_parents
            .len()
            .cmp(&left.retained_parents.len())
            .then(left.retained_components.cmp(&right.retained_components))
            .then_with(|| {
                right
                    .violation_influence_score
                    .total_cmp(&left.violation_influence_score)
            })
            .then(
                left.retained_boundary_edges
                    .cmp(&right.retained_boundary_edges),
            )
            .then(left.retained_parents.cmp(&right.retained_parents))
    });

    Ok(RetainedCoreSearchPlan {
        initial_coarse_parents: initial_core.clone(),
        parent_adjacency,
        candidates,
    })
}

pub fn solve_retained_core_topology(
    source: &MotherGrid,
    original: &HierarchyComponent,
    candidate: &RetainedCoreCandidate,
    limits: RetainedCoreTopologyLimits,
) -> RetainedCoreTopologyOutcome {
    let component = match component_for_retained_core(original, candidate) {
        Ok(component) => component,
        Err(reason) => {
            return invalid_topology(candidate, reason, FaceBandOutcomeKind::InvalidInput, 0, 0)
        }
    };
    let problem = match build_face_band_problem(source, &component, 2) {
        Ok(problem) => problem,
        Err(reason) => {
            return invalid_topology(candidate, reason, FaceBandOutcomeKind::InvalidInput, 0, 0)
        }
    };
    let (face_band_plan, face_band_states) = match solve_exact_face_bands_with_filter(
        &problem,
        FaceBandLimits {
            maximum_states: limits.face_band_states,
        },
        |plan| build_stratified_annulus_from_face_bands(source, &component, plan).is_ok(),
    ) {
        FaceBandSolveOutcome::Closed(plan, evidence) => (plan, evidence.states_examined),
        FaceBandSolveOutcome::FamilyExhaustedNoSolution { evidence, .. } => {
            return RetainedCoreTopologyOutcome::TopologyFamilyExhaustedNoSolution(evidence_for(
                candidate,
                evidence.outcome,
                evidence.states_examined,
                RetainedCoreTopologyOutcomeKind::TopologyFamilyExhaustedNoSolution,
                0,
                None,
            ))
        }
        FaceBandSolveOutcome::SearchBudgetExhausted { evidence, .. } => {
            return RetainedCoreTopologyOutcome::SearchBudgetExhausted(evidence_for(
                candidate,
                evidence.outcome,
                evidence.states_examined,
                RetainedCoreTopologyOutcomeKind::SearchBudgetExhausted,
                0,
                None,
            ))
        }
        FaceBandSolveOutcome::InvalidInput { reason } => {
            return invalid_topology(candidate, reason, FaceBandOutcomeKind::InvalidInput, 0, 0)
        }
    };
    match solve_full_polygon_merge_from_face_bands(
        source,
        &component,
        &face_band_plan,
        FullPolygonMergeLimits {
            topology_states: limits.topology_states,
        },
    ) {
        FullPolygonMergeOutcome::Closed(trial) => {
            let global = &trial.global_trial.evidence;
            let evidence = evidence_for(
                candidate,
                FaceBandOutcomeKind::Closed,
                face_band_states,
                RetainedCoreTopologyOutcomeKind::Closed,
                trial.evidence.states_examined,
                Some((
                    trial.evidence.selected_topology_keys.len(),
                    global.vertices,
                    global.edges,
                    global.faces,
                )),
            );
            RetainedCoreTopologyOutcome::Closed {
                component,
                face_band_plan,
                trial,
                evidence,
            }
        }
        FullPolygonMergeOutcome::TopologyFamilyExhaustedNoSolution(topology) => {
            RetainedCoreTopologyOutcome::TopologyFamilyExhaustedNoSolution(evidence_for(
                candidate,
                FaceBandOutcomeKind::Closed,
                face_band_states,
                RetainedCoreTopologyOutcomeKind::TopologyFamilyExhaustedNoSolution,
                topology.states_examined,
                None,
            ))
        }
        FullPolygonMergeOutcome::SearchBudgetExhausted(topology) => {
            RetainedCoreTopologyOutcome::SearchBudgetExhausted(evidence_for(
                candidate,
                FaceBandOutcomeKind::Closed,
                face_band_states,
                RetainedCoreTopologyOutcomeKind::SearchBudgetExhausted,
                topology.states_examined,
                None,
            ))
        }
        FullPolygonMergeOutcome::InvalidInput { reason, evidence } => invalid_topology(
            candidate,
            reason,
            FaceBandOutcomeKind::Closed,
            face_band_states,
            evidence.states_examined,
        ),
    }
}

fn component_for_retained_core(
    original: &HierarchyComponent,
    candidate: &RetainedCoreCandidate,
) -> Result<HierarchyComponent, String> {
    if candidate.retained_parents.is_empty() {
        return Err("retained-core topology requires a non-empty retained set".into());
    }
    let initial = original
        .core_parents
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if candidate
        .retained_parents
        .union(&candidate.released_parents)
        .copied()
        .collect::<BTreeSet<_>>()
        != initial
        || !candidate
            .retained_parents
            .is_disjoint(&candidate.released_parents)
    {
        return Err("retained-core candidate does not partition the original core".into());
    }
    let retained = &candidate.retained_parents;
    Ok(HierarchyComponent {
        id: original.id,
        parents: original.parents.clone(),
        boundary_edges: original.boundary_edges.clone(),
        core_parents: retained.iter().copied().collect(),
        transition_parents: original
            .parents
            .iter()
            .copied()
            .filter(|parent| !retained.contains(parent))
            .collect(),
    })
}

fn evidence_for(
    candidate: &RetainedCoreCandidate,
    face_band_outcome: FaceBandOutcomeKind,
    face_band_states: u64,
    topology_outcome: RetainedCoreTopologyOutcomeKind,
    topology_states: usize,
    closed: Option<(usize, usize, usize, usize)>,
) -> RetainedCoreTopologyEvidence {
    let (selected_topologies, vertices, edges, faces) = closed
        .map(|(selected, vertices, edges, faces)| {
            (selected, Some(vertices), Some(edges), Some(faces))
        })
        .unwrap_or((0, None, None, None));
    RetainedCoreTopologyEvidence {
        retained_parents: candidate.retained_parents.clone(),
        released_parents: candidate.released_parents.clone(),
        face_band_outcome,
        face_band_states,
        topology_outcome,
        topology_states,
        selected_topologies,
        vertices,
        edges,
        faces,
    }
}

fn invalid_topology(
    candidate: &RetainedCoreCandidate,
    reason: String,
    face_band_outcome: FaceBandOutcomeKind,
    face_band_states: u64,
    topology_states: usize,
) -> RetainedCoreTopologyOutcome {
    RetainedCoreTopologyOutcome::InvalidInput {
        reason,
        evidence: evidence_for(
            candidate,
            face_band_outcome,
            face_band_states,
            RetainedCoreTopologyOutcomeKind::InvalidInput,
            topology_states,
            None,
        ),
    }
}

fn graph_distances(
    graph: &BTreeMap<TriangleAddress, BTreeSet<TriangleAddress>>,
    seeds: &BTreeSet<TriangleAddress>,
) -> BTreeMap<TriangleAddress, usize> {
    let mut distances = BTreeMap::new();
    let mut queue = VecDeque::new();
    for &seed in seeds {
        distances.insert(seed, 0);
        queue.push_back(seed);
    }
    while let Some(parent) = queue.pop_front() {
        let next_distance = distances[&parent] + 1;
        for &neighbour in &graph[&parent] {
            if let std::collections::btree_map::Entry::Vacant(entry) = distances.entry(neighbour) {
                entry.insert(next_distance);
                queue.push_back(neighbour);
            }
        }
    }
    distances
}

fn component_count(
    retained: &BTreeSet<TriangleAddress>,
    adjacency: &BTreeMap<TriangleAddress, BTreeSet<TriangleAddress>>,
) -> usize {
    let mut unseen = retained.clone();
    let mut components = 0;
    while let Some(seed) = unseen.pop_first() {
        components += 1;
        let mut queue = VecDeque::from([seed]);
        while let Some(parent) = queue.pop_front() {
            for &neighbour in &adjacency[&parent] {
                if unseen.remove(&neighbour) {
                    queue.push_back(neighbour);
                }
            }
        }
    }
    components
}

pub fn retained_core_search_plan_json(plan: &RetainedCoreSearchPlan) -> String {
    let candidates = plan
        .candidates
        .iter()
        .map(|candidate| {
            format!(
                "{{\"retained_parents\":{},\"released_parents\":{},\"retained_components\":{},\"retained_boundary_edges\":{},\"violation_influence_score\":{:.12}}}",
                address_set_json(&candidate.retained_parents),
                address_set_json(&candidate.released_parents),
                candidate.retained_components,
                candidate.retained_boundary_edges,
                candidate.violation_influence_score,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"initial_coarse_parents\":{},\"candidate_count\":{},\"connected_candidate_count\":{},\"candidates\":[{}]}}",
        address_set_json(&plan.initial_coarse_parents),
        plan.candidates.len(),
        plan.connected_candidates().count(),
        candidates,
    )
}

pub fn retained_core_topology_evidence_json(evidence: &RetainedCoreTopologyEvidence) -> String {
    format!(
        "{{\"retained_parents\":{},\"released_parents\":{},\"face_band_outcome\":\"{:?}\",\"face_band_states\":{},\"topology_outcome\":\"{:?}\",\"topology_states\":{},\"selected_topologies\":{},\"vertices\":{},\"edges\":{},\"faces\":{}}}",
        address_set_json(&evidence.retained_parents),
        address_set_json(&evidence.released_parents),
        evidence.face_band_outcome,
        evidence.face_band_states,
        evidence.topology_outcome,
        evidence.topology_states,
        evidence.selected_topologies,
        option_usize(evidence.vertices),
        option_usize(evidence.edges),
        option_usize(evidence.faces),
    )
}

fn option_usize(value: Option<usize>) -> String {
    value.map_or_else(|| "null".into(), |value| value.to_string())
}

fn address_set_json(values: &BTreeSet<TriangleAddress>) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|address| format!(
                "{{\"base_face\":{},\"i\":{},\"j\":{},\"n\":{},\"orientation\":\"{:?}\"}}",
                address.base_face, address.i, address.j, address.n, address.orientation,
            ))
            .collect::<Vec<_>>()
            .join(",")
    )
}
