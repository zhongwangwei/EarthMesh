//! Exact polygon triangulation recovery from occurrence incidence targets.
//!
//! PIER works on a seam-cut annulus. It reconstructs occurrence triangles by
//! recursively removing incidence-one polygon ears; annular glue remains a
//! separate certification step.

use super::{
    annular_enumerator::glue_cut_topology, cut_annulus_polygon, enumerate_canonical_seam_annulus,
    AnnularTopologyKey, CutAnnulusPolygon, VertexOccurrenceId,
};
use std::collections::{BTreeMap, BTreeSet};

type Edge = (usize, usize);
type OccurrenceTriangle = [VertexOccurrenceId; 3];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct OccurrenceIncidenceTargetKey {
    pub root_bridge: Edge,
    pub occurrences: Vec<VertexOccurrenceId>,
    pub incidences: Vec<u8>,
    pub lower: Vec<usize>,
    pub upper: Vec<usize>,
    pub forbidden_global_edges: Vec<Edge>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OccurrenceIncidenceTarget {
    pub lower: Vec<usize>,
    pub upper: Vec<usize>,
    pub cut: CutAnnulusPolygon,
    pub incidences: Vec<u8>,
    pub forbidden_global_edges: BTreeSet<Edge>,
    pub target_key: OccurrenceIncidenceTargetKey,
}

impl OccurrenceIncidenceTarget {
    pub fn new(
        lower: Vec<usize>,
        upper: Vec<usize>,
        cut: CutAnnulusPolygon,
        incidences: Vec<u8>,
        forbidden_global_edges: BTreeSet<Edge>,
    ) -> Self {
        let target_key = OccurrenceIncidenceTargetKey {
            root_bridge: cut.root_bridge,
            occurrences: cut.occurrences.clone(),
            incidences: incidences.clone(),
            lower: lower.clone(),
            upper: upper.clone(),
            forbidden_global_edges: forbidden_global_edges.iter().copied().collect(),
        };
        Self {
            lower,
            upper,
            cut,
            incidences,
            forbidden_global_edges,
            target_key,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PolygonIncidenceStateKey {
    pub occurrences: Vec<VertexOccurrenceId>,
    pub remaining_incidences: Vec<u8>,
    pub inserted_global_edges: BTreeSet<Edge>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolygonIncidenceState {
    pub occurrences: Vec<VertexOccurrenceId>,
    pub remaining_incidences: Vec<u8>,
    pub triangles: Vec<OccurrenceTriangle>,
    pub inserted_global_edges: BTreeSet<Edge>,
    pub state_key: PolygonIncidenceStateKey,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct OccurrenceTriangulation {
    pub triangles: Vec<OccurrenceTriangle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolygonIncidenceOutcomeKind {
    Found,
    ExactNoWitness,
    SearchIncomplete,
    InvalidInput,
}

impl PolygonIncidenceOutcomeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Found => "Found",
            Self::ExactNoWitness => "ExactNoWitness",
            Self::SearchIncomplete => "SearchIncomplete",
            Self::InvalidInput => "InvalidInput",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolygonIncidenceEvidence {
    pub states: u64,
    pub ears_considered: u64,
    pub constraint_rejects: u64,
    pub duplicate_states: u64,
    pub maximum_frontier: usize,
    pub outcome: PolygonIncidenceOutcomeKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolygonIncidenceCheckpoint {
    target_key: OccurrenceIncidenceTargetKey,
    frontier: Vec<PolygonIncidenceState>,
    seen_states: BTreeSet<PolygonIncidenceStateKey>,
    evidence: PolygonIncidenceEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolygonIncidenceWitnessOutcome {
    Found {
        witness: OccurrenceTriangulation,
        evidence: PolygonIncidenceEvidence,
    },
    ExactNoWitness {
        target: OccurrenceIncidenceTargetKey,
        states: u64,
        evidence: PolygonIncidenceEvidence,
    },
    SearchIncomplete {
        checkpoint: PolygonIncidenceCheckpoint,
        evidence: PolygonIncidenceEvidence,
    },
    InvalidInput(String),
}

pub fn solve_polygon_incidence_witness(
    target: &OccurrenceIncidenceTarget,
    maximum_states: u64,
    checkpoint: Option<&PolygonIncidenceCheckpoint>,
) -> PolygonIncidenceWitnessOutcome {
    let context = match SearchContext::new(target) {
        Ok(context) => context,
        Err(reason) => return PolygonIncidenceWitnessOutcome::InvalidInput(reason),
    };
    let (mut frontier, mut seen, mut evidence) = match checkpoint {
        Some(checkpoint) => match resume(target, checkpoint, &context) {
            Ok(state) => state,
            Err(reason) => return PolygonIncidenceWitnessOutcome::InvalidInput(reason),
        },
        None => {
            let state = initial_state(target);
            (
                vec![state.clone()],
                BTreeSet::from([state.state_key]),
                empty_evidence(),
            )
        }
    };
    let starting_states = evidence.states;
    while evidence.states - starting_states < maximum_states {
        let Some(state) = frontier.pop() else {
            evidence.outcome = PolygonIncidenceOutcomeKind::ExactNoWitness;
            return PolygonIncidenceWitnessOutcome::ExactNoWitness {
                target: target.target_key.clone(),
                states: evidence.states,
                evidence,
            };
        };
        evidence.states += 1;
        if let Some(witness) = finish_triangle(&state) {
            evidence.outcome = PolygonIncidenceOutcomeKind::Found;
            return PolygonIncidenceWitnessOutcome::Found { witness, evidence };
        }
        push_children(&state, &context, &mut frontier, &mut seen, &mut evidence);
        evidence.maximum_frontier = evidence.maximum_frontier.max(frontier.len());
    }
    if frontier.is_empty() {
        evidence.outcome = PolygonIncidenceOutcomeKind::ExactNoWitness;
        return PolygonIncidenceWitnessOutcome::ExactNoWitness {
            target: target.target_key.clone(),
            states: evidence.states,
            evidence,
        };
    }
    evidence.outcome = PolygonIncidenceOutcomeKind::SearchIncomplete;
    let checkpoint = PolygonIncidenceCheckpoint {
        target_key: target.target_key.clone(),
        frontier,
        seen_states: seen,
        evidence: evidence.clone(),
    };
    PolygonIncidenceWitnessOutcome::SearchIncomplete {
        checkpoint,
        evidence,
    }
}

pub fn pier_small_exact_oracle_json() -> Result<String, String> {
    let mut fixtures = Vec::new();
    let mut all_equal = true;
    for (m, n) in [(3, 3), (3, 4), (4, 4), (4, 5)] {
        let lower = (0..m).collect::<Vec<_>>();
        let upper = (100..100 + n).collect::<Vec<_>>();
        let family = enumerate_canonical_seam_annulus(&lower, &upper, &BTreeSet::new())
            .map_err(|error| format!("{error:?}"))?;
        let mut groups = BTreeMap::<IncidenceGroupKey, BTreeSet<AnnularTopologyKey>>::new();
        for topology in &family.topologies {
            groups
                .entry(IncidenceGroupKey {
                    root_bridge: topology.root_bridge,
                    incidences: global_incidences(&topology.triangles),
                })
                .or_default()
                .insert(topology.topology_key.clone());
        }
        let mut recovered = 0usize;
        let mut states = 0u64;
        let mut equal = true;
        for (group, expected) in &groups {
            let (actual, group_states) = recover_group(&lower, &upper, group)?;
            recovered += actual.len();
            states += group_states;
            equal &= &actual == expected;
        }
        all_equal &= equal;
        fixtures.push(format!(
            "{{\"lower\":{m},\"upper\":{n},\"targets\":{},\"csae_topologies\":{},\"pier_topologies\":{recovered},\"pier_states\":{states},\"families_equal\":{equal}}}",
            groups.len(),
            family.topologies.len(),
        ));
    }
    Ok(format!(
        "{{\"schema_version\":1,\"taskbook_sha256\":\"65f26b64c78dd7dfadaaf2a1099f52d11c6a67461afb0a9558edbbf5941ef473\",\"declared_topology_family\":\"FixedTwoBoundaryAnnulusNoInteriorVertices+PIER\",\"fixtures\":[{}],\"all_families_equal\":{all_equal},\"geometry_attempted\":false,\"product_gate_changed\":false}}",
        fixtures.join(",")
    ))
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct IncidenceGroupKey {
    root_bridge: Edge,
    incidences: BTreeMap<usize, u8>,
}

struct SearchContext<'a> {
    target: &'a OccurrenceIncidenceTarget,
    lower: BTreeSet<usize>,
    upper: BTreeSet<usize>,
    boundary_global_edges: BTreeSet<Edge>,
}

impl<'a> SearchContext<'a> {
    fn new(target: &'a OccurrenceIncidenceTarget) -> Result<Self, String> {
        validate_target(target)?;
        Ok(Self {
            target,
            lower: target.lower.iter().copied().collect(),
            upper: target.upper.iter().copied().collect(),
            boundary_global_edges: target
                .cut
                .boundary_edges
                .iter()
                .map(|edge| global_edge(edge.a, edge.b))
                .collect(),
        })
    }
}

fn recover_group(
    lower: &[usize],
    upper: &[usize],
    group: &IncidenceGroupKey,
) -> Result<(BTreeSet<AnnularTopologyKey>, u64), String> {
    let lower_root = lower
        .iter()
        .position(|slot| group.root_bridge.0 == *slot || group.root_bridge.1 == *slot)
        .ok_or_else(|| "group root has no lower endpoint".to_string())?;
    let upper_root = upper
        .iter()
        .position(|slot| group.root_bridge.0 == *slot || group.root_bridge.1 == *slot)
        .ok_or_else(|| "group root has no upper endpoint".to_string())?;
    let cut = cut_annulus_polygon(lower, upper, lower_root, upper_root)
        .map_err(|error| format!("{error:?}"))?;
    if cut.root_bridge != group.root_bridge {
        return Err("group root bridge mismatch".into());
    }
    let lower_slot = lower[lower_root];
    let upper_slot = upper[upper_root];
    let lower_total = group.incidences[&lower_slot];
    let upper_total = group.incidences[&upper_slot];
    let mut keys = BTreeSet::new();
    let mut states = 0;
    for lower_first in 1..lower_total {
        for upper_first in 1..upper_total {
            let incidences = cut
                .occurrences
                .iter()
                .map(|occurrence| {
                    if occurrence.global_source_slot == lower_slot {
                        if occurrence.occurrence_ordinal == 0 {
                            lower_first
                        } else {
                            lower_total - lower_first
                        }
                    } else if occurrence.global_source_slot == upper_slot {
                        if occurrence.occurrence_ordinal == 0 {
                            upper_first
                        } else {
                            upper_total - upper_first
                        }
                    } else {
                        group.incidences[&occurrence.global_source_slot]
                    }
                })
                .collect::<Vec<_>>();
            let target = OccurrenceIncidenceTarget::new(
                lower.to_vec(),
                upper.to_vec(),
                cut.clone(),
                incidences,
                BTreeSet::new(),
            );
            let (witnesses, evidence) = enumerate_all(&target)?;
            states += evidence.states;
            for witness in witnesses {
                let occurrence_indices = occurrence_triangle_indices(&cut, &witness)?;
                if let Ok(topology) =
                    glue_cut_topology(lower, upper, &cut, &occurrence_indices, &BTreeSet::new())
                {
                    if topology.root_bridge == group.root_bridge
                        && global_incidences(&topology.triangles) == group.incidences
                    {
                        keys.insert(topology.topology_key);
                    }
                }
            }
        }
    }
    Ok((keys, states))
}

fn enumerate_all(
    target: &OccurrenceIncidenceTarget,
) -> Result<(BTreeSet<OccurrenceTriangulation>, PolygonIncidenceEvidence), String> {
    let context = SearchContext::new(target)?;
    let state = initial_state(target);
    let mut frontier = vec![state.clone()];
    let mut seen = BTreeSet::from([state.state_key]);
    let mut evidence = empty_evidence();
    let mut witnesses = BTreeSet::new();
    while let Some(state) = frontier.pop() {
        evidence.states += 1;
        if let Some(witness) = finish_triangle(&state) {
            witnesses.insert(witness);
            continue;
        }
        push_children(&state, &context, &mut frontier, &mut seen, &mut evidence);
        evidence.maximum_frontier = evidence.maximum_frontier.max(frontier.len());
    }
    evidence.outcome = if witnesses.is_empty() {
        PolygonIncidenceOutcomeKind::ExactNoWitness
    } else {
        PolygonIncidenceOutcomeKind::Found
    };
    Ok((witnesses, evidence))
}

fn validate_target(target: &OccurrenceIncidenceTarget) -> Result<(), String> {
    let size = target.cut.occurrences.len();
    if size < 3
        || target.incidences.len() != size
        || target.cut.occurrence_to_global.len() != size
        || target.cut.boundary_edges.len() != size
    {
        return Err("PIER target shape is invalid".into());
    }
    if target.target_key
        != OccurrenceIncidenceTarget::new(
            target.lower.clone(),
            target.upper.clone(),
            target.cut.clone(),
            target.incidences.clone(),
            target.forbidden_global_edges.clone(),
        )
        .target_key
    {
        return Err("PIER target key mismatch".into());
    }
    let lower = target.lower.iter().copied().collect::<BTreeSet<_>>();
    let upper = target.upper.iter().copied().collect::<BTreeSet<_>>();
    if target.lower.len() < 3
        || target.upper.len() < 3
        || lower.len() != target.lower.len()
        || upper.len() != target.upper.len()
        || !lower.is_disjoint(&upper)
    {
        return Err("PIER boundaries are invalid".into());
    }
    let lower_root = target
        .lower
        .iter()
        .position(|slot| target.cut.root_bridge.0 == *slot || target.cut.root_bridge.1 == *slot)
        .ok_or_else(|| "PIER root has no lower endpoint".to_string())?;
    let upper_root = target
        .upper
        .iter()
        .position(|slot| target.cut.root_bridge.0 == *slot || target.cut.root_bridge.1 == *slot)
        .ok_or_else(|| "PIER root has no upper endpoint".to_string())?;
    let expected_cut = cut_annulus_polygon(&target.lower, &target.upper, lower_root, upper_root)
        .map_err(|error| format!("{error:?}"))?;
    if target.cut != expected_cut {
        return Err("PIER cut is not canonical for its root bridge".into());
    }
    if target.incidences.contains(&0)
        || target
            .incidences
            .iter()
            .any(|&incidence| usize::from(incidence) > size - 2)
        || target
            .incidences
            .iter()
            .map(|&value| usize::from(value))
            .sum::<usize>()
            != 3 * (size - 2)
    {
        return Err("PIER incidence sum is invalid".into());
    }
    if target.cut.boundary_edges.iter().any(|occurrence_edge| {
        target
            .forbidden_global_edges
            .contains(&global_edge(occurrence_edge.a, occurrence_edge.b))
    }) {
        return Err("PIER cut boundary contains a forbidden edge".into());
    }
    Ok(())
}

fn initial_state(target: &OccurrenceIncidenceTarget) -> PolygonIncidenceState {
    build_state(
        target.cut.occurrences.clone(),
        target.incidences.clone(),
        Vec::new(),
        BTreeSet::new(),
    )
}

fn empty_evidence() -> PolygonIncidenceEvidence {
    PolygonIncidenceEvidence {
        states: 0,
        ears_considered: 0,
        constraint_rejects: 0,
        duplicate_states: 0,
        maximum_frontier: 1,
        outcome: PolygonIncidenceOutcomeKind::SearchIncomplete,
    }
}

fn push_children(
    state: &PolygonIncidenceState,
    context: &SearchContext<'_>,
    frontier: &mut Vec<PolygonIncidenceState>,
    seen: &mut BTreeSet<PolygonIncidenceStateKey>,
    evidence: &mut PolygonIncidenceEvidence,
) {
    if state.occurrences.len() <= 3 {
        return;
    }
    let mut children = Vec::new();
    for ear in 0..state.occurrences.len() {
        if state.remaining_incidences[ear] != 1 {
            continue;
        }
        evidence.ears_considered += 1;
        match remove_ear(state, ear, context) {
            Some(child) if seen.insert(child.state_key.clone()) => children.push(child),
            Some(_) => evidence.duplicate_states += 1,
            None => evidence.constraint_rejects += 1,
        }
    }
    children.sort_by(|left, right| left.state_key.cmp(&right.state_key));
    frontier.extend(children.into_iter().rev());
}

fn remove_ear(
    state: &PolygonIncidenceState,
    ear: usize,
    context: &SearchContext<'_>,
) -> Option<PolygonIncidenceState> {
    let size = state.occurrences.len();
    let previous = (ear + size - 1) % size;
    let next = (ear + 1) % size;
    let triangle = occurrence_triangle([
        state.occurrences[previous],
        state.occurrences[ear],
        state.occurrences[next],
    ]);
    if triangle
        .iter()
        .map(|occurrence| occurrence.global_source_slot)
        .collect::<BTreeSet<_>>()
        .len()
        != 3
    {
        return None;
    }
    let diagonal = global_edge(state.occurrences[previous], state.occurrences[next]);
    if context.target.forbidden_global_edges.contains(&diagonal)
        || context.boundary_global_edges.contains(&diagonal)
        || state.inserted_global_edges.contains(&diagonal)
        || is_noncanonical_bridge(diagonal, context)
    {
        return None;
    }
    let mut incidences = state.remaining_incidences.clone();
    incidences[previous] = incidences[previous].checked_sub(1)?;
    incidences[next] = incidences[next].checked_sub(1)?;
    if incidences[previous] == 0 || incidences[next] == 0 {
        return None;
    }
    let mut occurrences = state.occurrences.clone();
    occurrences.remove(ear);
    incidences.remove(ear);
    if incidences
        .iter()
        .map(|&value| usize::from(value))
        .sum::<usize>()
        != 3 * (occurrences.len() - 2)
    {
        return None;
    }
    let mut triangles = state.triangles.clone();
    triangles.push(triangle);
    triangles.sort_unstable();
    let mut inserted = state.inserted_global_edges.clone();
    inserted.insert(diagonal);
    Some(build_state(occurrences, incidences, triangles, inserted))
}

fn finish_triangle(state: &PolygonIncidenceState) -> Option<OccurrenceTriangulation> {
    if state.occurrences.len() != 3 || state.remaining_incidences != [1, 1, 1] {
        return None;
    }
    let triangle = occurrence_triangle([
        state.occurrences[0],
        state.occurrences[1],
        state.occurrences[2],
    ]);
    if triangle
        .iter()
        .map(|occurrence| occurrence.global_source_slot)
        .collect::<BTreeSet<_>>()
        .len()
        != 3
    {
        return None;
    }
    let mut triangles = state.triangles.clone();
    triangles.push(triangle);
    triangles.sort_unstable();
    Some(OccurrenceTriangulation { triangles })
}

fn build_state(
    occurrences: Vec<VertexOccurrenceId>,
    remaining_incidences: Vec<u8>,
    triangles: Vec<OccurrenceTriangle>,
    inserted_global_edges: BTreeSet<Edge>,
) -> PolygonIncidenceState {
    let state_key = PolygonIncidenceStateKey {
        occurrences: occurrences.clone(),
        remaining_incidences: remaining_incidences.clone(),
        inserted_global_edges: inserted_global_edges.clone(),
    };
    PolygonIncidenceState {
        occurrences,
        remaining_incidences,
        triangles,
        inserted_global_edges,
        state_key,
    }
}

fn resume(
    target: &OccurrenceIncidenceTarget,
    checkpoint: &PolygonIncidenceCheckpoint,
    context: &SearchContext<'_>,
) -> Result<
    (
        Vec<PolygonIncidenceState>,
        BTreeSet<PolygonIncidenceStateKey>,
        PolygonIncidenceEvidence,
    ),
    String,
> {
    if checkpoint.target_key != target.target_key
        || checkpoint.evidence.outcome != PolygonIncidenceOutcomeKind::SearchIncomplete
    {
        return Err("PIER checkpoint identity mismatch".into());
    }
    for state in &checkpoint.frontier {
        validate_checkpoint_state(state, context)?;
        if !checkpoint.seen_states.contains(&state.state_key) {
            return Err("PIER checkpoint frontier is absent from seen states".into());
        }
    }
    Ok((
        checkpoint.frontier.clone(),
        checkpoint.seen_states.clone(),
        checkpoint.evidence.clone(),
    ))
}

fn validate_checkpoint_state(
    state: &PolygonIncidenceState,
    context: &SearchContext<'_>,
) -> Result<(), String> {
    let target_occurrences = context
        .target
        .cut
        .occurrences
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let state_occurrences = state.occurrences.iter().copied().collect::<BTreeSet<_>>();
    if state.occurrences.len() != state.remaining_incidences.len()
        || state.occurrences.len() < 3
        || state_occurrences.len() != state.occurrences.len()
        || !state_occurrences.is_subset(&target_occurrences)
        || !is_cyclic_subsequence(&context.target.cut.occurrences, &state.occurrences)
        || state.remaining_incidences.contains(&0)
        || state
            .remaining_incidences
            .iter()
            .map(|&value| usize::from(value))
            .sum::<usize>()
            != 3 * (state.occurrences.len() - 2)
        || state.triangles.len() != context.target.cut.occurrences.len() - state.occurrences.len()
        || state.inserted_global_edges.len() != state.triangles.len()
        || state.state_key
            != (PolygonIncidenceStateKey {
                occurrences: state.occurrences.clone(),
                remaining_incidences: state.remaining_incidences.clone(),
                inserted_global_edges: state.inserted_global_edges.clone(),
            })
        || !state
            .inserted_global_edges
            .is_disjoint(&context.boundary_global_edges)
        || state
            .inserted_global_edges
            .iter()
            .any(|edge| context.target.forbidden_global_edges.contains(edge))
    {
        return Err("PIER checkpoint state is invalid".into());
    }
    let mut consumed = BTreeMap::<VertexOccurrenceId, usize>::new();
    for (index, triangle) in state.triangles.iter().enumerate() {
        if triangle != &occurrence_triangle(*triangle)
            || index > 0 && state.triangles[index - 1] >= *triangle
            || triangle.iter().copied().collect::<BTreeSet<_>>().len() != 3
            || triangle
                .iter()
                .map(|occurrence| occurrence.global_source_slot)
                .collect::<BTreeSet<_>>()
                .len()
                != 3
            || triangle
                .iter()
                .any(|occurrence| !target_occurrences.contains(occurrence))
        {
            return Err("PIER checkpoint triangle history is invalid".into());
        }
        for occurrence in triangle {
            *consumed.entry(*occurrence).or_default() += 1;
        }
    }
    let remaining = state
        .occurrences
        .iter()
        .copied()
        .zip(state.remaining_incidences.iter().copied())
        .collect::<BTreeMap<_, _>>();
    for (index, occurrence) in context.target.cut.occurrences.iter().enumerate() {
        let initial = usize::from(context.target.incidences[index]);
        let used = consumed.get(occurrence).copied().unwrap_or(0);
        let expected_remaining = remaining.get(occurrence).copied().map(usize::from);
        if expected_remaining.is_some_and(|value| initial.checked_sub(used) != Some(value))
            || expected_remaining.is_none() && used != initial
        {
            return Err("PIER checkpoint incidence history is invalid".into());
        }
    }
    if state.inserted_global_edges.iter().any(|edge| {
        is_noncanonical_bridge(*edge, context)
            || !state.triangles.iter().any(|triangle| {
                [(0, 1), (1, 2), (2, 0)]
                    .into_iter()
                    .any(|(a, b)| global_edge(triangle[a], triangle[b]) == *edge)
            })
    }) {
        return Err("PIER checkpoint diagonal history is invalid".into());
    }
    Ok(())
}

fn is_cyclic_subsequence(
    original: &[VertexOccurrenceId],
    candidate: &[VertexOccurrenceId],
) -> bool {
    let Some(first) = candidate.first() else {
        return false;
    };
    let Some(start) = original.iter().position(|occurrence| occurrence == first) else {
        return false;
    };
    let mut previous = start;
    for occurrence in candidate.iter().skip(1) {
        let Some(position) = original.iter().position(|item| item == occurrence) else {
            return false;
        };
        let mut position = position + previous / original.len() * original.len();
        position += usize::from(position <= previous) * original.len();
        if position >= start + original.len() {
            return false;
        }
        previous = position;
    }
    true
}

fn occurrence_triangle_indices(
    cut: &CutAnnulusPolygon,
    witness: &OccurrenceTriangulation,
) -> Result<Vec<[usize; 3]>, String> {
    let indices = cut
        .occurrences
        .iter()
        .copied()
        .enumerate()
        .map(|(index, occurrence)| (occurrence, index))
        .collect::<BTreeMap<_, _>>();
    witness
        .triangles
        .iter()
        .map(|triangle| {
            let mut out = [0; 3];
            for index in 0..3 {
                out[index] = *indices
                    .get(&triangle[index])
                    .ok_or_else(|| "PIER witness contains an unknown occurrence".to_string())?;
            }
            out.sort_unstable();
            Ok(out)
        })
        .collect()
}

fn global_incidences(triangles: &[[usize; 3]]) -> BTreeMap<usize, u8> {
    let mut out = BTreeMap::new();
    for triangle in triangles {
        for &slot in triangle {
            *out.entry(slot).or_default() += 1;
        }
    }
    out
}

fn is_noncanonical_bridge(candidate: Edge, context: &SearchContext<'_>) -> bool {
    let cross_boundary = context.lower.contains(&candidate.0)
        && context.upper.contains(&candidate.1)
        || context.lower.contains(&candidate.1) && context.upper.contains(&candidate.0);
    cross_boundary
        && candidate != context.target.cut.root_bridge
        && candidate < context.target.cut.root_bridge
}

fn occurrence_triangle(mut triangle: OccurrenceTriangle) -> OccurrenceTriangle {
    triangle.sort_unstable();
    triangle
}

fn global_edge(a: VertexOccurrenceId, b: VertexOccurrenceId) -> Edge {
    edge(a.global_source_slot, b.global_source_slot)
}

fn edge(a: usize, b: usize) -> Edge {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known_target() -> OccurrenceIncidenceTarget {
        let lower = vec![0, 1, 2];
        let upper = vec![100, 101, 102];
        let cut = cut_annulus_polygon(&lower, &upper, 0, 0).unwrap();
        OccurrenceIncidenceTarget::new(
            lower,
            upper,
            cut,
            vec![1, 2, 3, 3, 1, 2, 3, 3],
            BTreeSet::new(),
        )
    }

    #[test]
    fn triangle_base_requires_1_1_1() {
        let state = build_state(
            vec![
                VertexOccurrenceId {
                    global_source_slot: 0,
                    occurrence_ordinal: 0,
                },
                VertexOccurrenceId {
                    global_source_slot: 1,
                    occurrence_ordinal: 0,
                },
                VertexOccurrenceId {
                    global_source_slot: 2,
                    occurrence_ordinal: 0,
                },
            ],
            vec![1, 2, 1],
            Vec::new(),
            BTreeSet::new(),
        );
        assert!(finish_triangle(&state).is_none());
    }

    #[test]
    fn ear_removal_decrements_neighbours() {
        let occurrences = (0..4)
            .map(|slot| VertexOccurrenceId {
                global_source_slot: slot,
                occurrence_ordinal: 0,
            })
            .collect::<Vec<_>>();
        let target = OccurrenceIncidenceTarget::new(
            vec![0, 1, 2],
            vec![3, 4, 5],
            CutAnnulusPolygon {
                root_bridge: (0, 3),
                boundary_edges: Vec::new(),
                occurrence_to_global: vec![0, 1, 2, 3],
                occurrences: occurrences.clone(),
            },
            vec![2, 1, 2, 1],
            BTreeSet::new(),
        );
        let context = SearchContext {
            target: &target,
            lower: BTreeSet::from([0, 1, 2]),
            upper: BTreeSet::from([3, 4, 5]),
            boundary_global_edges: BTreeSet::from([(0, 1), (1, 2), (2, 3), (0, 3)]),
        };
        let state = build_state(occurrences, vec![2, 1, 2, 1], Vec::new(), BTreeSet::new());
        let child = remove_ear(&state, 1, &context).unwrap();
        assert_eq!(child.remaining_incidences, vec![1, 1, 1]);
    }

    #[test]
    fn forbidden_diagonal_rejects() {
        let mut target = known_target();
        target.forbidden_global_edges.insert((0, 2));
        let context = SearchContext {
            target: &target,
            lower: target.lower.iter().copied().collect(),
            upper: target.upper.iter().copied().collect(),
            boundary_global_edges: BTreeSet::new(),
        };
        let state = build_state(
            vec![
                VertexOccurrenceId {
                    global_source_slot: 0,
                    occurrence_ordinal: 0,
                },
                VertexOccurrenceId {
                    global_source_slot: 1,
                    occurrence_ordinal: 0,
                },
                VertexOccurrenceId {
                    global_source_slot: 2,
                    occurrence_ordinal: 0,
                },
                VertexOccurrenceId {
                    global_source_slot: 100,
                    occurrence_ordinal: 0,
                },
            ],
            vec![2, 1, 2, 1],
            Vec::new(),
            BTreeSet::new(),
        );
        assert!(remove_ear(&state, 1, &context).is_none());
    }

    #[test]
    fn root_bridge_canonicality() {
        let mut target = known_target();
        target.cut.root_bridge = (1, 100);
        let context = SearchContext {
            target: &target,
            lower: target.lower.iter().copied().collect(),
            upper: target.upper.iter().copied().collect(),
            boundary_global_edges: BTreeSet::new(),
        };
        let state = build_state(
            vec![
                VertexOccurrenceId {
                    global_source_slot: 0,
                    occurrence_ordinal: 0,
                },
                VertexOccurrenceId {
                    global_source_slot: 2,
                    occurrence_ordinal: 0,
                },
                VertexOccurrenceId {
                    global_source_slot: 100,
                    occurrence_ordinal: 0,
                },
                VertexOccurrenceId {
                    global_source_slot: 101,
                    occurrence_ordinal: 0,
                },
            ],
            vec![2, 1, 2, 1],
            Vec::new(),
            BTreeSet::new(),
        );
        assert!(remove_ear(&state, 1, &context).is_none());
    }

    #[test]
    fn same_final_state_deduplicates() {
        let (_, evidence) = enumerate_all(&known_target()).unwrap();
        assert!(evidence.duplicate_states > 0);
    }

    #[test]
    fn all_polygon_triangulations_have_recoverable_ear_sequence() {
        fn triangulations(first: usize, last: usize) -> Vec<Vec<[usize; 3]>> {
            if last <= first + 1 {
                return vec![Vec::new()];
            }
            let mut out = Vec::new();
            for middle in first + 1..last {
                for left in triangulations(first, middle) {
                    for right in triangulations(middle, last) {
                        let mut triangles = left.clone();
                        triangles.extend(right);
                        triangles.push([first, middle, last]);
                        out.push(triangles);
                    }
                }
            }
            out
        }
        fn reducible(incidences: &[u8]) -> bool {
            if incidences.len() == 3 {
                return incidences == [1, 1, 1];
            }
            for ear in 0..incidences.len() {
                if incidences[ear] != 1 {
                    continue;
                }
                let previous = (ear + incidences.len() - 1) % incidences.len();
                let next = (ear + 1) % incidences.len();
                let mut child = incidences.to_vec();
                let Some(previous_value) = child[previous].checked_sub(1) else {
                    continue;
                };
                let Some(next_value) = child[next].checked_sub(1) else {
                    continue;
                };
                if previous_value == 0 || next_value == 0 {
                    continue;
                }
                child[previous] = previous_value;
                child[next] = next_value;
                child.remove(ear);
                if reducible(&child) {
                    return true;
                }
            }
            false
        }
        for vertices in 3..=8 {
            for triangles in triangulations(0, vertices - 1) {
                let mut incidences = vec![0u8; vertices];
                for triangle in triangles {
                    for vertex in triangle {
                        incidences[vertex] += 1;
                    }
                }
                assert!(reducible(&incidences), "{vertices}: {incidences:?}");
            }
        }
    }

    #[test]
    fn checkpoint_resume_equals_one_shot() {
        let target = known_target();
        let one_shot = solve_polygon_incidence_witness(&target, u64::MAX, None);
        let checkpoint = match solve_polygon_incidence_witness(&target, 1, None) {
            PolygonIncidenceWitnessOutcome::SearchIncomplete { checkpoint, .. } => checkpoint,
            other => panic!("expected checkpoint, got {other:?}"),
        };
        let resumed = solve_polygon_incidence_witness(&target, u64::MAX, Some(&checkpoint));
        assert_eq!(one_shot, resumed);
    }
}
