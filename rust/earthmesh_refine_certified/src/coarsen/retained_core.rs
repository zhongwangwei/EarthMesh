//! Deterministic retained coarse-parent subset planning for Frozen N6 recovery.

use super::annulus::{parent_by_source_face, parent_graph};
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
