//! Exact source-face label search for small transition complexes.

use super::{annulus::parent_by_source_face, HierarchyComponent};
use crate::mother_grid::{MotherGrid, TriangleAddress, VertexAddress};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AnchorBandPolicy {
    InteriorOfSingleBand,
    OnSingleInterface,
    FineCapConnectedToExterior,
}

impl AnchorBandPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InteriorOfSingleBand => "InteriorOfSingleBand",
            Self::OnSingleInterface => "OnSingleInterface",
            Self::FineCapConnectedToExterior => "FineCapConnectedToExterior",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceBandProblem {
    pub transition_faces: Vec<usize>,
    pub coarse_boundary_faces: BTreeSet<usize>,
    pub fine_boundary_faces: BTreeSet<usize>,
    pub face_adjacency: BTreeMap<usize, Vec<usize>>,
    pub vertex_incident_faces: BTreeMap<usize, Vec<usize>>,
    pub face_vertex_neighbours: BTreeMap<usize, Vec<usize>>,
    pub band_count: usize,
    pub anchor_policies: BTreeMap<usize, AnchorBandPolicy>,
    pub face_shared_edges: BTreeMap<(usize, usize), (usize, usize)>,
    pub coarse_boundary_vertices: BTreeSet<usize>,
    pub fine_boundary_vertices: BTreeSet<usize>,
    pub face_addresses: BTreeMap<usize, TriangleAddress>,
    pub core_nonempty: bool,
    pub source_face_rings: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceBandDomain {
    pub face: usize,
    pub labels: BTreeSet<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaceBandLimits {
    pub maximum_states: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceBandPlan {
    pub band_count: usize,
    pub labels: BTreeMap<usize, u8>,
    pub interface_edges: Vec<Vec<(usize, usize)>>,
    pub band_face_counts: Vec<usize>,
    pub face_complex_fingerprint: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceBandOutcomeKind {
    Closed,
    FamilyExhaustedNoSolution,
    SearchBudgetExhausted,
    InvalidInput,
}

impl FaceBandOutcomeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Closed => "Closed",
            Self::FamilyExhaustedNoSolution => "FamilyExhaustedNoSolution",
            Self::SearchBudgetExhausted => "SearchBudgetExhausted",
            Self::InvalidInput => "InvalidInput",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceBandEvidence {
    pub band_count: usize,
    pub face_complex_fingerprint: u64,
    pub transition_faces: usize,
    pub coarse_boundary_faces: usize,
    pub fine_boundary_faces: usize,
    pub states_examined: u64,
    pub propagation_rounds: usize,
    pub pruned_domains: usize,
    pub band_face_counts: Vec<usize>,
    pub interface_edge_counts: Vec<usize>,
    pub interface_vertex_counts: Vec<usize>,
    pub true_pinch_count: usize,
    pub one_face_wedge_count: usize,
    pub multi_face_wedge_count: usize,
    pub anchor_policies: BTreeMap<usize, AnchorBandPolicy>,
    pub cap_faces: usize,
    pub corridor_faces: usize,
    pub core_faces_sacrificed: usize,
    pub source_face_rings: usize,
    pub outcome: FaceBandOutcomeKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FaceBandSolveOutcome {
    Closed(Box<FaceBandPlan>, FaceBandEvidence),
    FamilyExhaustedNoSolution {
        band_count: usize,
        states_examined: u64,
        evidence: FaceBandEvidence,
    },
    SearchBudgetExhausted {
        band_count: usize,
        states_examined: u64,
        evidence: FaceBandEvidence,
    },
    InvalidInput {
        reason: String,
    },
}

pub fn build_face_band_problem(
    source: &MotherGrid,
    component: &HierarchyComponent,
    band_count: usize,
) -> Result<FaceBandProblem, String> {
    build_face_band_problem_with_source_face_rings(source, component, band_count, 0)
}

pub fn build_face_band_problem_with_source_face_rings(
    source: &MotherGrid,
    component: &HierarchyComponent,
    band_count: usize,
    source_face_rings: usize,
) -> Result<FaceBandProblem, String> {
    if band_count < 2 || band_count > u8::MAX as usize + 1 {
        return Err("face-band count must be in 2..=256".into());
    }
    if source_face_rings > 2 {
        return Err("registered face-band expansion supports at most two source-face rings".into());
    }
    let parent_by_face = parent_by_source_face(source).map_err(|error| format!("{error:?}"))?;
    let transition_parents = component
        .transition_parents
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let core_parents = component
        .core_parents
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut transition = source
        .mesh
        .active_triangle_slots()
        .filter(|face| transition_parents.contains(&parent_by_face[face]))
        .collect::<BTreeSet<_>>();
    if transition.is_empty() {
        return Err("face-band problem requires transition faces".into());
    }
    let mut frontier = transition.clone();
    for _ in 0..source_face_rings {
        let next = frontier
            .iter()
            .flat_map(|&face| source.mesh.neighbours()[face])
            .filter(|&face| {
                source.mesh.is_triangle_live(face)
                    && !transition.contains(&face)
                    && !core_parents.contains(&parent_by_face[&face])
            })
            .collect::<BTreeSet<_>>();
        transition.extend(next.iter().copied());
        frontier = next;
    }
    let transition_faces = transition.iter().copied().collect::<Vec<_>>();
    let mut coarse_boundary_faces = BTreeSet::new();
    let mut fine_boundary_faces = BTreeSet::new();
    let mut coarse_boundary_vertices = BTreeSet::new();
    let mut fine_boundary_vertices = BTreeSet::new();
    let mut face_adjacency = BTreeMap::<usize, Vec<usize>>::new();
    let mut face_shared_edges = BTreeMap::new();
    let mut vertex_incident_faces = BTreeMap::<usize, Vec<usize>>::new();
    for &face in &transition_faces {
        let triangle = source.mesh.triangles()[face];
        for vertex in triangle {
            vertex_incident_faces.entry(vertex).or_default().push(face);
        }
        for side in 0..3 {
            let neighbour = source.mesh.neighbours()[face][side];
            let edge = canonical_edge(triangle[(side + 1) % 3], triangle[(side + 2) % 3]);
            if transition.contains(&neighbour) {
                face_adjacency.entry(face).or_default().push(neighbour);
                face_shared_edges.insert(canonical_face_pair(face, neighbour), edge);
            } else if source.mesh.is_triangle_live(neighbour)
                && core_parents.contains(&parent_by_face[&neighbour])
            {
                coarse_boundary_faces.insert(face);
                coarse_boundary_vertices.extend([edge.0, edge.1]);
            } else {
                fine_boundary_faces.insert(face);
                fine_boundary_vertices.extend([edge.0, edge.1]);
            }
        }
    }
    for neighbours in face_adjacency.values_mut() {
        neighbours.sort_unstable();
        neighbours.dedup();
    }
    for faces in vertex_incident_faces.values_mut() {
        faces.sort_unstable();
        faces.dedup();
    }
    let mut face_vertex_neighbours = BTreeMap::<usize, Vec<usize>>::new();
    for faces in vertex_incident_faces.values() {
        for &face in faces {
            face_vertex_neighbours
                .entry(face)
                .or_default()
                .extend(faces.iter().copied());
        }
    }
    for faces in face_vertex_neighbours.values_mut() {
        faces.sort_unstable();
        faces.dedup();
    }
    let anchor_policies = vertex_incident_faces
        .keys()
        .filter_map(|&vertex| {
            matches!(
                source.addresses[vertex],
                Some(VertexAddress::IcosahedronVertex(_))
            )
            .then_some((vertex, AnchorBandPolicy::InteriorOfSingleBand))
        })
        .collect();
    let face_addresses = transition_faces
        .iter()
        .map(|&face| {
            source.triangle_addresses[face]
                .map(|address| (face, address))
                .ok_or_else(|| format!("transition face {face} has no address"))
        })
        .collect::<Result<_, _>>()?;
    Ok(FaceBandProblem {
        transition_faces,
        coarse_boundary_faces,
        fine_boundary_faces,
        face_adjacency,
        vertex_incident_faces,
        face_vertex_neighbours,
        band_count,
        anchor_policies,
        face_shared_edges,
        coarse_boundary_vertices,
        fine_boundary_vertices,
        face_addresses,
        core_nonempty: !core_parents.is_empty(),
        source_face_rings,
    })
}

pub fn solve_exact_face_bands(
    problem: &FaceBandProblem,
    limits: FaceBandLimits,
) -> FaceBandSolveOutcome {
    let Err(reason) = validate_problem(problem) else {
        let fingerprint = fingerprint(problem);
        let mut search = Search::new(problem, limits.maximum_states, fingerprint);
        let outcome = search.run();
        return match outcome {
            SearchResult::Closed(plan) => {
                let evidence = search.evidence(FaceBandOutcomeKind::Closed, Some(&plan));
                FaceBandSolveOutcome::Closed(Box::new(plan), evidence)
            }
            SearchResult::Exhausted => {
                let evidence =
                    search.evidence(FaceBandOutcomeKind::FamilyExhaustedNoSolution, None);
                FaceBandSolveOutcome::FamilyExhaustedNoSolution {
                    band_count: problem.band_count,
                    states_examined: search.states,
                    evidence,
                }
            }
            SearchResult::Budget => {
                let evidence = search.evidence(FaceBandOutcomeKind::SearchBudgetExhausted, None);
                FaceBandSolveOutcome::SearchBudgetExhausted {
                    band_count: problem.band_count,
                    states_examined: search.states,
                    evidence,
                }
            }
        };
    };
    FaceBandSolveOutcome::InvalidInput { reason }
}

pub fn face_band_plan_json(plan: &FaceBandPlan) -> String {
    let labels = plan
        .labels
        .iter()
        .map(|(face, label)| format!("{{\"face\":{face},\"label\":{label}}}"))
        .collect::<Vec<_>>()
        .join(",");
    let interfaces = plan
        .interface_edges
        .iter()
        .map(|edges| {
            format!(
                "[{}]",
                edges
                    .iter()
                    .map(|(a, b)| format!("[{a},{b}]"))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"band_count\":{},\"face_complex_fingerprint\":{},\"labels\":[{}],\"interface_edges\":[{}],\"band_face_counts\":{}}}",
        plan.band_count,
        plan.face_complex_fingerprint,
        labels,
        interfaces,
        usize_json(&plan.band_face_counts),
    )
}

pub fn face_band_evidence_json(evidence: &FaceBandEvidence) -> String {
    let anchors = evidence
        .anchor_policies
        .iter()
        .map(|(vertex, policy)| format!("\"{vertex}\":\"{}\"", policy.as_str()))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"ladder_step\":\"{}\",\"band_count\":{},\"face_complex_fingerprint\":{},\"transition_faces\":{},\"coarse_boundary_faces\":{},\"fine_boundary_faces\":{},\"states_examined\":{},\"propagation_rounds\":{},\"pruned_domains\":{},\"band_face_counts\":{},\"interface_edge_counts\":{},\"interface_vertex_counts\":{},\"true_pinch_count\":{},\"one_face_wedge_count\":{},\"multi_face_wedge_count\":{},\"anchor_policies\":{{{}}},\"cap_faces\":{},\"corridor_faces\":{},\"core_faces_sacrificed\":{},\"outcome\":\"{}\"}}",
        ladder_step(evidence.source_face_rings),
        evidence.band_count,
        evidence.face_complex_fingerprint,
        evidence.transition_faces,
        evidence.coarse_boundary_faces,
        evidence.fine_boundary_faces,
        evidence.states_examined,
        evidence.propagation_rounds,
        evidence.pruned_domains,
        usize_json(&evidence.band_face_counts),
        usize_json(&evidence.interface_edge_counts),
        usize_json(&evidence.interface_vertex_counts),
        evidence.true_pinch_count,
        evidence.one_face_wedge_count,
        evidence.multi_face_wedge_count,
        anchors,
        evidence.cap_faces,
        evidence.corridor_faces,
        evidence.core_faces_sacrificed,
        evidence.outcome.as_str(),
    )
}

fn usize_json(values: &[usize]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn validate_problem(problem: &FaceBandProblem) -> Result<(), String> {
    if !matches!(problem.band_count, 2 | 3) {
        return Err("exact face-band solver supports band_count=2 or 3".into());
    }
    if problem.source_face_rings > 2 {
        return Err("registered face-band expansion supports at most two source-face rings".into());
    }
    let faces = problem
        .transition_faces
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if faces.len() != problem.transition_faces.len() || faces.is_empty() {
        return Err("transition faces must be non-empty and unique".into());
    }
    if !problem.coarse_boundary_faces.is_subset(&faces)
        || !problem.fine_boundary_faces.is_subset(&faces)
        || problem.coarse_boundary_faces.is_empty()
        || problem.fine_boundary_faces.is_empty()
    {
        return Err("coarse/fine boundary faces must be non-empty transition subsets".into());
    }
    if !problem
        .coarse_boundary_faces
        .is_disjoint(&problem.fine_boundary_faces)
    {
        return Err("a transition face cannot belong to both fixed boundaries".into());
    }
    if !problem.core_nonempty {
        return Err("face-band problem requires a non-empty coarse core".into());
    }
    Ok(())
}

enum SearchResult {
    Closed(FaceBandPlan),
    Exhausted,
    Budget,
}

struct Search<'a> {
    problem: &'a FaceBandProblem,
    domains: BTreeMap<usize, BTreeSet<u8>>,
    preferred: BTreeMap<usize, u8>,
    maximum_states: u64,
    states: u64,
    propagation_rounds: usize,
    pruned_domains: usize,
    fingerprint: u64,
    budget_hit: bool,
}

impl<'a> Search<'a> {
    fn new(problem: &'a FaceBandProblem, maximum_states: u64, fingerprint: u64) -> Self {
        let coarse_distances = face_distances(problem, &problem.coarse_boundary_faces);
        let fine_distances = face_distances(problem, &problem.fine_boundary_faces);
        let last = (problem.band_count - 1) as u8;
        let mut domains = BTreeMap::new();
        let mut preferred = BTreeMap::new();
        for &face in &problem.transition_faces {
            let labels = if problem.coarse_boundary_faces.contains(&face) {
                BTreeSet::from([0])
            } else if problem.fine_boundary_faces.contains(&face) {
                BTreeSet::from([last])
            } else {
                (0..=last).collect()
            };
            let coarse = coarse_distances
                .get(&face)
                .copied()
                .unwrap_or(usize::MAX / 4);
            let fine = fine_distances.get(&face).copied().unwrap_or(usize::MAX / 4);
            let denominator = coarse.saturating_add(fine).max(1);
            let estimate = ((last as usize * coarse + denominator / 2) / denominator) as u8;
            domains.insert(face, labels);
            preferred.insert(face, estimate.min(last));
        }
        Self {
            problem,
            domains,
            preferred,
            maximum_states,
            states: 0,
            propagation_rounds: 0,
            pruned_domains: 0,
            fingerprint,
            budget_hit: false,
        }
    }

    fn run(&mut self) -> SearchResult {
        if !self.propagate() {
            return SearchResult::Exhausted;
        }
        if let Some(plan) = self.search() {
            SearchResult::Closed(plan)
        } else if self.budget_hit {
            SearchResult::Budget
        } else {
            SearchResult::Exhausted
        }
    }

    fn search(&mut self) -> Option<FaceBandPlan> {
        if self.states >= self.maximum_states {
            self.budget_hit = true;
            return None;
        }
        let next = self
            .domains
            .iter()
            .filter(|(_, labels)| labels.len() > 1)
            .min_by_key(|(face, labels)| {
                (
                    labels.len(),
                    self.problem
                        .face_addresses
                        .get(face)
                        .copied()
                        .expect("validated face address"),
                )
            })
            .map(|(&face, _)| face);
        let Some(face) = next else {
            self.states += 1;
            return validate_complete(self.problem, &self.domains, self.fingerprint);
        };
        let mut labels = self.domains[&face].iter().copied().collect::<Vec<_>>();
        labels.sort_by_key(|label| (label.abs_diff(self.preferred[&face]), *label));
        let checkpoint = self.domains.clone();
        for label in labels {
            if self.states >= self.maximum_states {
                self.budget_hit = true;
                break;
            }
            self.states += 1;
            self.domains.insert(face, BTreeSet::from([label]));
            if self.propagate() {
                if let Some(plan) = self.search() {
                    return Some(plan);
                }
            }
            self.domains.clone_from(&checkpoint);
        }
        None
    }

    fn propagate(&mut self) -> bool {
        loop {
            self.propagation_rounds += 1;
            let mut changed = false;
            for (&anchor, policy) in &self.problem.anchor_policies {
                let Some(faces) = self.problem.vertex_incident_faces.get(&anchor) else {
                    continue;
                };
                match policy {
                    AnchorBandPolicy::InteriorOfSingleBand => {
                        let common = faces
                            .iter()
                            .filter_map(|face| self.domains.get(face))
                            .cloned()
                            .reduce(|left, right| left.intersection(&right).copied().collect())
                            .unwrap_or_default();
                        if common.is_empty() {
                            return false;
                        }
                        for face in faces {
                            if let Some(domain) = self.domains.get_mut(face) {
                                let before = domain.len();
                                domain.retain(|label| common.contains(label));
                                changed |= domain.len() != before;
                                self.pruned_domains += before - domain.len();
                            }
                        }
                    }
                    AnchorBandPolicy::FineCapConnectedToExterior => {
                        let fine = (self.problem.band_count - 1) as u8;
                        for face in faces {
                            if let Some(domain) = self.domains.get_mut(face) {
                                if !domain.contains(&fine) {
                                    return false;
                                }
                                let before = domain.len();
                                domain.retain(|label| *label == fine);
                                changed |= domain.len() != before;
                                self.pruned_domains += before - domain.len();
                            }
                        }
                    }
                    AnchorBandPolicy::OnSingleInterface => {}
                }
            }
            if self.problem.band_count == 3 {
                let Some(pruned) = prune_local_label_constraints(self.problem, &mut self.domains)
                else {
                    return false;
                };
                changed |= pruned > 0;
                self.pruned_domains += pruned;
            }
            for label in 0..self.problem.band_count as u8 {
                let possible = self
                    .problem
                    .transition_faces
                    .iter()
                    .copied()
                    .filter(|face| self.domains[face].contains(&label))
                    .collect::<BTreeSet<_>>();
                if possible.is_empty()
                    || !assigned_connected_through_possible(
                        self.problem,
                        &self.domains,
                        label,
                        &possible,
                    )
                {
                    return false;
                }
            }
            if !changed {
                return true;
            }
        }
    }

    fn evidence(
        &self,
        outcome: FaceBandOutcomeKind,
        plan: Option<&FaceBandPlan>,
    ) -> FaceBandEvidence {
        let band_face_counts = plan
            .map(|plan| plan.band_face_counts.clone())
            .unwrap_or_default();
        let interface_edge_counts = plan
            .map(|plan| plan.interface_edges.iter().map(Vec::len).collect())
            .unwrap_or_default();
        let interface_vertex_counts = plan
            .map(|plan| {
                plan.interface_edges
                    .iter()
                    .map(|edges| {
                        edges
                            .iter()
                            .flat_map(|edge| [edge.0, edge.1])
                            .collect::<BTreeSet<_>>()
                            .len()
                    })
                    .collect()
            })
            .unwrap_or_default();
        FaceBandEvidence {
            band_count: self.problem.band_count,
            face_complex_fingerprint: self.fingerprint,
            transition_faces: self.problem.transition_faces.len(),
            coarse_boundary_faces: self.problem.coarse_boundary_faces.len(),
            fine_boundary_faces: self.problem.fine_boundary_faces.len(),
            states_examined: self.states,
            propagation_rounds: self.propagation_rounds,
            pruned_domains: self.pruned_domains,
            band_face_counts,
            interface_edge_counts,
            interface_vertex_counts,
            true_pinch_count: 0,
            one_face_wedge_count: 0,
            multi_face_wedge_count: 0,
            anchor_policies: self.problem.anchor_policies.clone(),
            cap_faces: 0,
            corridor_faces: 0,
            core_faces_sacrificed: 0,
            source_face_rings: self.problem.source_face_rings,
            outcome,
        }
    }
}

fn prune_local_label_constraints(
    problem: &FaceBandProblem,
    domains: &mut BTreeMap<usize, BTreeSet<u8>>,
) -> Option<usize> {
    let snapshot = domains.clone();
    let mut pruned = 0;
    for &face in &problem.transition_faces {
        let mut allowed = snapshot[&face].clone();
        allowed.retain(|&label| {
            problem
                .face_adjacency
                .get(&face)
                .into_iter()
                .flatten()
                .all(|neighbour| {
                    snapshot[neighbour]
                        .iter()
                        .any(|&other| label.abs_diff(other) <= 1)
                })
                && problem
                    .face_vertex_neighbours
                    .get(&face)
                    .into_iter()
                    .flatten()
                    .all(|other_face| {
                        snapshot[other_face]
                            .iter()
                            .any(|&other| label.abs_diff(other) <= 1)
                    })
        });
        if allowed.is_empty() {
            return None;
        }
        pruned += domains[&face].len() - allowed.len();
        domains.insert(face, allowed);
    }
    Some(pruned)
}

fn assigned_connected_through_possible(
    problem: &FaceBandProblem,
    domains: &BTreeMap<usize, BTreeSet<u8>>,
    label: u8,
    possible: &BTreeSet<usize>,
) -> bool {
    let assigned = possible
        .iter()
        .copied()
        .filter(|face| domains[face].len() == 1)
        .collect::<Vec<_>>();
    let Some(&start) = assigned.first() else {
        return true;
    };
    let reachable = flood_faces(problem, start, possible);
    assigned.iter().all(|face| reachable.contains(face)) && domains[&start].contains(&label)
}

fn validate_complete(
    problem: &FaceBandProblem,
    domains: &BTreeMap<usize, BTreeSet<u8>>,
    fingerprint: u64,
) -> Option<FaceBandPlan> {
    let labels = domains
        .iter()
        .map(|(&face, domain)| Some((face, *domain.iter().next()?)))
        .collect::<Option<BTreeMap<_, _>>>()?;
    for (&face, neighbours) in &problem.face_adjacency {
        for neighbour in neighbours {
            if labels[&face].abs_diff(labels[neighbour]) > 1 {
                return None;
            }
        }
    }
    for faces in problem.vertex_incident_faces.values() {
        let incident = faces
            .iter()
            .map(|face| labels[face])
            .collect::<BTreeSet<_>>();
        if incident
            .iter()
            .next_back()?
            .abs_diff(*incident.iter().next()?)
            > 1
        {
            return None;
        }
    }
    let mut band_face_counts = Vec::new();
    for label in 0..problem.band_count as u8 {
        let faces = labels
            .iter()
            .filter_map(|(&face, &actual)| (actual == label).then_some(face))
            .collect::<BTreeSet<_>>();
        if faces.is_empty() || flood_faces(problem, *faces.first()?, &faces).len() != faces.len() {
            return None;
        }
        if !is_annular_strip(problem, &faces) {
            return None;
        }
        band_face_counts.push(faces.len());
    }
    let mut interface_edges = vec![Vec::new(); problem.band_count - 1];
    for (&(left, right), &edge) in &problem.face_shared_edges {
        let a = labels[&left];
        let b = labels[&right];
        if a.abs_diff(b) == 1 {
            interface_edges[a.min(b) as usize].push(edge);
        }
    }
    let mut interface_vertices = Vec::new();
    for edges in &mut interface_edges {
        edges.sort_unstable();
        edges.dedup();
        if !is_one_cycle(edges) {
            return None;
        }
        let vertices = edges
            .iter()
            .flat_map(|edge| [edge.0, edge.1])
            .collect::<BTreeSet<_>>();
        if !vertices.is_disjoint(&problem.coarse_boundary_vertices)
            || !vertices.is_disjoint(&problem.fine_boundary_vertices)
        {
            return None;
        }
        interface_vertices.push(vertices);
    }
    for left in 0..interface_vertices.len() {
        for right in left + 1..interface_vertices.len() {
            if !interface_vertices[left].is_disjoint(&interface_vertices[right]) {
                return None;
            }
        }
    }
    for (&anchor, policy) in &problem.anchor_policies {
        let incident = problem.vertex_incident_faces[&anchor]
            .iter()
            .map(|face| labels[face])
            .collect::<BTreeSet<_>>();
        match policy {
            AnchorBandPolicy::InteriorOfSingleBand if incident.len() != 1 => return None,
            AnchorBandPolicy::OnSingleInterface => {
                let degree = interface_edges
                    .iter()
                    .flatten()
                    .filter(|edge| edge.0 == anchor || edge.1 == anchor)
                    .count();
                if incident.len() > 2 || degree != 2 {
                    return None;
                }
            }
            AnchorBandPolicy::FineCapConnectedToExterior
                if incident != BTreeSet::from([(problem.band_count - 1) as u8]) =>
            {
                return None;
            }
            _ => {}
        }
    }
    Some(FaceBandPlan {
        band_count: problem.band_count,
        labels,
        interface_edges,
        band_face_counts,
        face_complex_fingerprint: fingerprint,
    })
}

fn is_annular_strip(problem: &FaceBandProblem, faces: &BTreeSet<usize>) -> bool {
    let mut edge_counts = BTreeMap::<(usize, usize), usize>::new();
    let mut vertices = BTreeSet::new();
    for &face in faces {
        for edge in problem_edges(problem, face) {
            *edge_counts.entry(edge).or_default() += 1;
            vertices.extend([edge.0, edge.1]);
        }
    }
    let euler = vertices.len() as isize - edge_counts.len() as isize + faces.len() as isize;
    let boundary = edge_counts
        .into_iter()
        .filter_map(|(edge, count)| (count == 1).then_some(edge))
        .collect::<Vec<_>>();
    euler == 0 && cycle_component_count(&boundary) == Some(2)
}

fn problem_edges(problem: &FaceBandProblem, face: usize) -> Vec<(usize, usize)> {
    let mut edges = problem
        .face_shared_edges
        .iter()
        .filter_map(|(&(left, right), &edge)| (left == face || right == face).then_some(edge))
        .collect::<BTreeSet<_>>();
    let known_vertices = problem
        .vertex_incident_faces
        .iter()
        .filter_map(|(&vertex, faces)| faces.contains(&face).then_some(vertex))
        .collect::<Vec<_>>();
    for a in 0..known_vertices.len() {
        for b in a + 1..known_vertices.len() {
            edges.insert(canonical_edge(known_vertices[a], known_vertices[b]));
        }
    }
    edges.into_iter().collect()
}

fn is_one_cycle(edges: &[(usize, usize)]) -> bool {
    cycle_component_count(edges) == Some(1)
}

fn cycle_component_count(edges: &[(usize, usize)]) -> Option<usize> {
    if edges.is_empty() {
        return None;
    }
    let mut graph = BTreeMap::<usize, BTreeSet<usize>>::new();
    for &(a, b) in edges {
        graph.entry(a).or_default().insert(b);
        graph.entry(b).or_default().insert(a);
    }
    if graph.values().any(|neighbours| neighbours.len() != 2) {
        return None;
    }
    let mut unseen = graph.keys().copied().collect::<BTreeSet<_>>();
    let mut components = 0;
    while let Some(&start) = unseen.first() {
        components += 1;
        let mut queue = VecDeque::from([start]);
        unseen.remove(&start);
        while let Some(vertex) = queue.pop_front() {
            for &next in &graph[&vertex] {
                if unseen.remove(&next) {
                    queue.push_back(next);
                }
            }
        }
    }
    Some(components)
}

fn flood_faces(
    problem: &FaceBandProblem,
    start: usize,
    allowed: &BTreeSet<usize>,
) -> BTreeSet<usize> {
    let mut reached = BTreeSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some(face) = queue.pop_front() {
        for &next in problem.face_adjacency.get(&face).into_iter().flatten() {
            if allowed.contains(&next) && reached.insert(next) {
                queue.push_back(next);
            }
        }
    }
    reached
}

fn face_distances(problem: &FaceBandProblem, seeds: &BTreeSet<usize>) -> BTreeMap<usize, usize> {
    let mut distances = seeds
        .iter()
        .map(|&face| (face, 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut queue = seeds.iter().copied().collect::<VecDeque<_>>();
    while let Some(face) = queue.pop_front() {
        let distance = distances[&face];
        for &next in problem.face_adjacency.get(&face).into_iter().flatten() {
            if let std::collections::btree_map::Entry::Vacant(entry) = distances.entry(next) {
                entry.insert(distance + 1);
                queue.push_back(next);
            }
        }
    }
    distances
}

fn fingerprint(problem: &FaceBandProblem) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for value in problem
        .transition_faces
        .iter()
        .copied()
        .chain(problem.coarse_boundary_faces.iter().copied())
        .chain(problem.fine_boundary_faces.iter().copied())
        .chain([problem.band_count])
    {
        hash ^= value as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn ladder_step(source_face_rings: usize) -> &'static str {
    match source_face_rings {
        0 => "F0CurrentTransitionFaces",
        1 => "F1OneSourceFaceRing",
        2 => "F2TwoSourceFaceRings",
        _ => "Unsupported",
    }
}

fn canonical_face_pair(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}

fn canonical_edge(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}
