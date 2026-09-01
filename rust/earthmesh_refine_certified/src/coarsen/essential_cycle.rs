//! Canonical W2 interface-cycle construction and lossless face-label conversion.

use super::{
    face_band::{face_band_fingerprint, validate_face_band_plan},
    problem_identity::{canonical_edge, canonical_vertex},
    AnchorBandPolicy, CanonicalEdgeId, CanonicalFaceId, CanonicalVertexId,
    EssentialCycleProblemKey, FaceBandPlan, FaceBandProblem, RetainedCoreCorridorFamily,
};
use crate::{MotherGrid, TriangleAddress};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitSet {
    words: Vec<u64>,
    len: usize,
}

impl BitSet {
    pub fn empty(len: usize) -> Self {
        Self {
            words: vec![0; len.div_ceil(64)],
            len,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn contains(&self, index: usize) -> bool {
        index < self.len && self.words[index / 64] & (1 << (index % 64)) != 0
    }

    pub fn set(&mut self, index: usize, value: bool) -> Result<(), String> {
        if index >= self.len {
            return Err(format!("bit index {index} is outside length {}", self.len));
        }
        if value {
            self.words[index / 64] |= 1 << (index % 64);
        } else {
            self.words[index / 64] &= !(1 << (index % 64));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EssentialCycleKey {
    pub ordered_vertices: Vec<CanonicalVertexId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EssentialCycleProblem {
    pub source_n: usize,
    pub transition_faces: Vec<CanonicalFaceId>,
    pub coarse_boundary_faces: BTreeSet<CanonicalFaceId>,
    pub fine_boundary_faces: BTreeSet<CanonicalFaceId>,
    pub candidate_vertices: Vec<CanonicalVertexId>,
    pub candidate_edges: Vec<CanonicalEdgeId>,
    pub edge_incident_faces: Vec<[CanonicalFaceId; 2]>,
    pub vertex_incident_edges: Vec<Vec<usize>>,
    pub coarse_boundary_vertices: BTreeSet<CanonicalVertexId>,
    pub fine_boundary_vertices: BTreeSet<CanonicalVertexId>,
    pub anchor_policies: BTreeMap<CanonicalVertexId, AnchorBandPolicy>,
    pub dual_seam_crossing_edges: BitSet,
    pub problem_key: EssentialCycleProblemKey,
}

pub fn build_essential_cycle_problem(
    source: &MotherGrid,
    face_problem: &FaceBandProblem,
    retained_parents: impl IntoIterator<Item = TriangleAddress>,
    corridor_family: RetainedCoreCorridorFamily,
) -> Result<EssentialCycleProblem, String> {
    if face_problem.band_count != 2 {
        return Err("essential-cycle construction supports W2 only".into());
    }
    let problem_key = super::essential_cycle_problem_key(
        source,
        face_problem,
        retained_parents,
        corridor_family,
    )?;
    let incident_by_edge = problem_key
        .candidate_edge_incident_faces
        .iter()
        .cloned()
        .collect::<BTreeMap<_, _>>();
    if incident_by_edge.len() != problem_key.candidate_edges.len() {
        return Err("candidate primal edges do not have unique dual incidences".into());
    }
    let candidate_edges = problem_key.candidate_edges.clone();
    let edge_incident_faces = candidate_edges
        .iter()
        .map(|edge| {
            incident_by_edge
                .get(edge)
                .cloned()
                .ok_or_else(|| "candidate primal edge has no dual incidence".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let candidate_vertices = candidate_edges
        .iter()
        .flat_map(|edge| edge.vertices.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let vertex_index = candidate_vertices
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, vertex)| (vertex, index))
        .collect::<BTreeMap<_, _>>();
    let mut vertex_incident_edges = vec![Vec::new(); candidate_vertices.len()];
    for (edge_index, edge) in candidate_edges.iter().enumerate() {
        for vertex in &edge.vertices {
            vertex_incident_edges[vertex_index[vertex]].push(edge_index);
        }
    }
    let mut dual_seam_crossing_edges = BitSet::empty(candidate_edges.len());
    for edge in dual_seam(&problem_key)? {
        dual_seam_crossing_edges.set(edge, true)?;
    }

    Ok(EssentialCycleProblem {
        source_n: source.subdivision,
        transition_faces: problem_key
            .transition_faces
            .iter()
            .copied()
            .map(|address| CanonicalFaceId { address })
            .collect(),
        coarse_boundary_faces: problem_key
            .coarse_boundary_faces
            .iter()
            .copied()
            .map(|address| CanonicalFaceId { address })
            .collect(),
        fine_boundary_faces: problem_key
            .fine_boundary_faces
            .iter()
            .copied()
            .map(|address| CanonicalFaceId { address })
            .collect(),
        candidate_vertices,
        candidate_edges,
        edge_incident_faces,
        vertex_incident_edges,
        coarse_boundary_vertices: face_problem
            .coarse_boundary_vertices
            .iter()
            .map(|slot| canonical_vertex(source, *slot))
            .collect(),
        fine_boundary_vertices: face_problem
            .fine_boundary_vertices
            .iter()
            .map(|slot| canonical_vertex(source, *slot))
            .collect(),
        anchor_policies: face_problem
            .anchor_policies
            .iter()
            .map(|(slot, policy)| (canonical_vertex(source, *slot), *policy))
            .collect(),
        dual_seam_crossing_edges,
        problem_key,
    })
}

pub fn essential_cycle_from_face_band_plan(
    source: &MotherGrid,
    face_problem: &FaceBandProblem,
    problem: &EssentialCycleProblem,
    plan: &FaceBandPlan,
) -> Result<EssentialCycleKey, String> {
    if source.subdivision != problem.source_n || !validate_face_band_plan(face_problem, plan) {
        return Err("face-band plan does not satisfy the matching W2 contract".into());
    }
    let edge_index = candidate_edge_index(problem);
    let selected = plan
        .interface_edges
        .first()
        .ok_or_else(|| "W2 face-band plan has no interface".to_string())?
        .iter()
        .map(|&(left, right)| {
            let edge = canonical_edge(
                canonical_vertex(source, left),
                canonical_vertex(source, right),
            );
            edge_index
                .get(&edge)
                .copied()
                .ok_or_else(|| "face-band interface contains a non-candidate edge".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    validate_selected_essential_cycle(problem, &selected)
}

pub fn face_band_plan_from_essential_cycle(
    source: &MotherGrid,
    face_problem: &FaceBandProblem,
    problem: &EssentialCycleProblem,
    cycle: &EssentialCycleKey,
) -> Result<FaceBandPlan, String> {
    if source.subdivision != problem.source_n || face_problem.band_count != 2 {
        return Err("essential cycle does not match the supplied W2 problem".into());
    }
    let edge_index = candidate_edge_index(problem);
    let selected = cycle_edges(cycle)?
        .iter()
        .map(|edge| {
            edge_index
                .get(edge)
                .copied()
                .ok_or_else(|| "cycle contains a non-candidate edge".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if validate_selected_essential_cycle(problem, &selected)? != *cycle {
        return Err("cycle key is not in canonical order".into());
    }
    let coarse_faces = coarse_side(problem, &selected)?;
    let face_slot = face_problem
        .face_addresses
        .iter()
        .map(|(&slot, &address)| (CanonicalFaceId { address }, slot))
        .collect::<BTreeMap<_, _>>();
    let labels = problem
        .transition_faces
        .iter()
        .map(|face| {
            face_slot
                .get(face)
                .copied()
                .map(|slot| (slot, u8::from(!coarse_faces.contains(face))))
                .ok_or_else(|| "canonical face has no runtime slot".to_string())
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let mut interface_edges = cycle_edges(cycle)?
        .iter()
        .map(|edge| {
            let left = runtime_vertex(source, &edge.vertices[0])?;
            let right = runtime_vertex(source, &edge.vertices[1])?;
            Ok((left.min(right), left.max(right)))
        })
        .collect::<Result<Vec<_>, String>>()?;
    interface_edges.sort_unstable();
    let band_face_counts = vec![
        labels.values().filter(|&&label| label == 0).count(),
        labels.values().filter(|&&label| label == 1).count(),
    ];
    let plan = FaceBandPlan {
        band_count: 2,
        labels,
        interface_edges: vec![interface_edges],
        band_face_counts,
        face_complex_fingerprint: face_band_fingerprint(face_problem),
    };
    if !validate_face_band_plan(face_problem, &plan) {
        return Err("cycle-derived labels fail the legacy W2 hard contract".into());
    }
    Ok(plan)
}

pub fn validate_selected_essential_cycle(
    problem: &EssentialCycleProblem,
    selected_edge_indices: &[usize],
) -> Result<EssentialCycleKey, String> {
    let selected = selected_edge_indices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if selected.is_empty()
        || selected.len() != selected_edge_indices.len()
        || selected
            .iter()
            .any(|index| *index >= problem.candidate_edges.len())
    {
        return Err("selected cycle edges must be non-empty, unique candidates".into());
    }
    let mut graph = BTreeMap::<CanonicalVertexId, BTreeSet<CanonicalVertexId>>::new();
    for index in &selected {
        let [left, right] = problem.candidate_edges[*index].vertices.clone();
        graph.entry(left.clone()).or_default().insert(right.clone());
        graph.entry(right).or_default().insert(left);
    }
    if graph.len() < 3 || graph.values().any(|neighbours| neighbours.len() != 2) {
        return Err("selected primal edges do not form degree-two cycle vertices".into());
    }
    let vertices = graph.keys().cloned().collect::<BTreeSet<_>>();
    let reached = flood_vertices(
        graph.keys().next().expect("non-empty cycle graph").clone(),
        &graph,
    );
    if reached != vertices {
        return Err("selected primal edges form multiple cycles".into());
    }
    if !vertices.is_disjoint(&problem.coarse_boundary_vertices)
        || !vertices.is_disjoint(&problem.fine_boundary_vertices)
    {
        return Err("selected cycle touches a fixed boundary vertex".into());
    }
    for (anchor, policy) in &problem.anchor_policies {
        let degree = graph.get(anchor).map(BTreeSet::len).unwrap_or(0);
        match policy {
            AnchorBandPolicy::OnSingleInterface if degree != 2 => {
                return Err("OnSingleInterface anchor is not on the selected cycle".into());
            }
            AnchorBandPolicy::InteriorOfSingleBand
            | AnchorBandPolicy::FineCapConnectedToExterior
                if degree != 0 =>
            {
                return Err("selected cycle violates an anchor interior policy".into());
            }
            _ => {}
        }
    }
    if essential_cycle_seam_parity(problem, selected.iter().copied()) != 1 {
        return Err("selected cycle has even dual-seam parity".into());
    }
    coarse_side(problem, selected_edge_indices)?;
    Ok(canonical_cycle_key(&graph))
}

pub fn essential_cycle_seam_parity(
    problem: &EssentialCycleProblem,
    selected_edge_indices: impl IntoIterator<Item = usize>,
) -> u8 {
    selected_edge_indices.into_iter().fold(0, |parity, index| {
        parity ^ u8::from(problem.dual_seam_crossing_edges.contains(index))
    })
}

fn dual_seam(problem: &EssentialCycleProblemKey) -> Result<Vec<usize>, String> {
    let candidate_index = problem
        .candidate_edges
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, edge)| (edge, index))
        .collect::<BTreeMap<_, _>>();
    let edge_by_faces = problem
        .candidate_edge_incident_faces
        .iter()
        .map(|(edge, faces)| {
            candidate_index
                .get(edge)
                .copied()
                .map(|index| ((faces[0].clone(), faces[1].clone()), index))
                .ok_or_else(|| "dual incidence references a non-candidate edge".to_string())
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let mut adjacency = BTreeMap::<CanonicalFaceId, Vec<CanonicalFaceId>>::new();
    for (left, right) in &problem.face_adjacency_edges {
        adjacency
            .entry(left.clone())
            .or_default()
            .push(right.clone());
        adjacency
            .entry(right.clone())
            .or_default()
            .push(left.clone());
    }
    for neighbours in adjacency.values_mut() {
        neighbours.sort_unstable();
        neighbours.dedup();
    }
    let starts = problem
        .coarse_boundary_faces
        .iter()
        .copied()
        .map(|address| CanonicalFaceId { address })
        .collect::<Vec<_>>();
    let targets = problem
        .fine_boundary_faces
        .iter()
        .copied()
        .map(|address| CanonicalFaceId { address })
        .collect::<BTreeSet<_>>();
    let mut previous = starts
        .iter()
        .cloned()
        .map(|face| (face, None))
        .collect::<BTreeMap<_, Option<CanonicalFaceId>>>();
    let mut queue = starts.into_iter().collect::<VecDeque<_>>();
    let target = loop {
        let Some(face) = queue.pop_front() else {
            return Err("coarse and fine boundaries have no dual seam".into());
        };
        if targets.contains(&face) {
            break face;
        }
        for next in adjacency.get(&face).into_iter().flatten() {
            if !previous.contains_key(next) {
                previous.insert(next.clone(), Some(face.clone()));
                queue.push_back(next.clone());
            }
        }
    };
    let mut crossing_edges = Vec::new();
    let mut face = target;
    while let Some(parent) = previous[&face].clone() {
        let pair = if parent <= face {
            (parent.clone(), face.clone())
        } else {
            (face.clone(), parent.clone())
        };
        if let Some(index) = edge_by_faces.get(&pair) {
            crossing_edges.push(*index);
        }
        face = parent;
    }
    Ok(crossing_edges)
}

fn coarse_side(
    problem: &EssentialCycleProblem,
    selected_edge_indices: &[usize],
) -> Result<BTreeSet<CanonicalFaceId>, String> {
    let selected_edges = selected_edge_indices
        .iter()
        .map(|index| {
            problem
                .candidate_edges
                .get(*index)
                .cloned()
                .ok_or_else(|| "selected cycle edge index is out of range".to_string())
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let primal_by_dual = problem
        .candidate_edges
        .iter()
        .cloned()
        .zip(problem.edge_incident_faces.iter().cloned())
        .map(|(edge, mut faces)| {
            faces.sort_unstable();
            ((faces[0].clone(), faces[1].clone()), edge)
        })
        .collect::<BTreeMap<_, _>>();
    let mut adjacency = BTreeMap::<CanonicalFaceId, BTreeSet<CanonicalFaceId>>::new();
    for (left, right) in &problem.problem_key.face_adjacency_edges {
        if primal_by_dual
            .get(&(left.clone(), right.clone()))
            .is_some_and(|edge| selected_edges.contains(edge))
        {
            continue;
        }
        adjacency
            .entry(left.clone())
            .or_default()
            .insert(right.clone());
        adjacency
            .entry(right.clone())
            .or_default()
            .insert(left.clone());
    }
    let coarse_start = problem
        .coarse_boundary_faces
        .first()
        .cloned()
        .ok_or_else(|| "essential-cycle problem has no coarse boundary face".to_string())?;
    let coarse = flood_faces(coarse_start, &adjacency);
    if !problem.coarse_boundary_faces.is_subset(&coarse)
        || !problem.fine_boundary_faces.is_disjoint(&coarse)
    {
        return Err("selected cycle does not separate coarse and fine boundaries".into());
    }
    let all = problem
        .transition_faces
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let fine = all.difference(&coarse).cloned().collect::<BTreeSet<_>>();
    let fine_start = problem
        .fine_boundary_faces
        .first()
        .cloned()
        .ok_or_else(|| "essential-cycle problem has no fine boundary face".to_string())?;
    if !problem.fine_boundary_faces.is_subset(&fine) || flood_faces(fine_start, &adjacency) != fine
    {
        return Err("selected cycle does not leave one connected fine side".into());
    }
    Ok(coarse)
}

fn candidate_edge_index(problem: &EssentialCycleProblem) -> BTreeMap<CanonicalEdgeId, usize> {
    problem
        .candidate_edges
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, edge)| (edge, index))
        .collect()
}

fn cycle_edges(cycle: &EssentialCycleKey) -> Result<Vec<CanonicalEdgeId>, String> {
    if cycle.ordered_vertices.len() < 3
        || cycle.ordered_vertices.iter().collect::<BTreeSet<_>>().len()
            != cycle.ordered_vertices.len()
    {
        return Err("cycle key requires at least three unique vertices".into());
    }
    Ok((0..cycle.ordered_vertices.len())
        .map(|index| {
            canonical_edge(
                cycle.ordered_vertices[index].clone(),
                cycle.ordered_vertices[(index + 1) % cycle.ordered_vertices.len()].clone(),
            )
        })
        .collect())
}

fn canonical_cycle_key(
    graph: &BTreeMap<CanonicalVertexId, BTreeSet<CanonicalVertexId>>,
) -> EssentialCycleKey {
    let start = graph.keys().next().expect("validated cycle").clone();
    let directions = graph[&start].iter().cloned().collect::<Vec<_>>();
    let walk = |mut next: CanonicalVertexId| {
        let mut ordered = vec![start.clone()];
        let mut previous = start.clone();
        while next != start {
            ordered.push(next.clone());
            let following = graph[&next]
                .iter()
                .find(|vertex| **vertex != previous)
                .expect("degree-two cycle")
                .clone();
            previous = next;
            next = following;
        }
        ordered
    };
    let forward = walk(directions[0].clone());
    let reverse = walk(directions[1].clone());
    EssentialCycleKey {
        ordered_vertices: forward.min(reverse),
    }
}

fn flood_vertices(
    start: CanonicalVertexId,
    graph: &BTreeMap<CanonicalVertexId, BTreeSet<CanonicalVertexId>>,
) -> BTreeSet<CanonicalVertexId> {
    let mut reached = BTreeSet::from([start.clone()]);
    let mut queue = VecDeque::from([start]);
    while let Some(vertex) = queue.pop_front() {
        for next in graph.get(&vertex).into_iter().flatten() {
            if reached.insert(next.clone()) {
                queue.push_back(next.clone());
            }
        }
    }
    reached
}

fn flood_faces(
    start: CanonicalFaceId,
    graph: &BTreeMap<CanonicalFaceId, BTreeSet<CanonicalFaceId>>,
) -> BTreeSet<CanonicalFaceId> {
    let mut reached = BTreeSet::from([start.clone()]);
    let mut queue = VecDeque::from([start]);
    while let Some(face) = queue.pop_front() {
        for next in graph.get(&face).into_iter().flatten() {
            if reached.insert(next.clone()) {
                queue.push_back(next.clone());
            }
        }
    }
    reached
}

fn runtime_vertex(source: &MotherGrid, id: &CanonicalVertexId) -> Result<usize, String> {
    match id {
        CanonicalVertexId::Address(address) => source
            .addresses
            .iter()
            .position(|candidate| candidate.as_ref() == Some(address))
            .ok_or_else(|| "canonical vertex address is absent from source grid".into()),
        CanonicalVertexId::FrozenSourceSlot { source_n, slot }
            if *source_n == source.subdivision && *slot < source.addresses.len() =>
        {
            Ok(*slot)
        }
        CanonicalVertexId::FrozenSourceSlot { .. } => {
            Err("frozen vertex slot belongs to a different source grid".into())
        }
    }
}
