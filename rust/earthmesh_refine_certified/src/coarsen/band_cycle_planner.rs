//! Scoped evidence for the legacy parent-layer trace family.
//!
//! This module only re-extracts parent-layer contours and selects their
//! subsequences. It is not a source-face band solver.

use super::annulus::{parent_by_source_face, parent_graph, parent_layers_from_outside};
use super::{extract_coupled_annulus, CoupledAnnulus, HierarchyComponent, RingCycle};
use crate::mother_grid::{MotherGrid, TriangleAddress, VertexAddress};
use earthmesh_mesh::arc_length_unit_sphere;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const MAX_OUTWARD_EXPANSIONS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionBandPlanningFamily {
    ParentLayerTraceFamily,
}

impl TransitionBandPlanningFamily {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ParentLayerTraceFamily => "ParentLayerTraceFamily",
        }
    }
}

pub const TRANSITION_BAND_PLANNING_FAMILY: TransitionBandPlanningFamily =
    TransitionBandPlanningFamily::ParentLayerTraceFamily;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionBandMode {
    LegacyTwoLogicalBands,
    ThreeEffectiveBands,
    FourEffectiveBandsNearSingularities,
}

impl TransitionBandMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LegacyTwoLogicalBands => "LegacyTwoLogicalBands",
            Self::ThreeEffectiveBands => "ThreeEffectiveBands",
            Self::FourEffectiveBandsNearSingularities => "FourEffectiveBandsNearSingularities",
        }
    }

    fn planning_bands(self) -> usize {
        match self {
            Self::LegacyTwoLogicalBands => 2,
            Self::ThreeEffectiveBands => 3,
            Self::FourEffectiveBandsNearSingularities => 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransitionBandPlan {
    pub mode: TransitionBandMode,
    pub traces: Vec<RingCycle>,
    pub parent_faces: Vec<TriangleAddress>,
    pub effective_band_count_min: usize,
    pub effective_band_count_p50: usize,
    pub adjacent_shared_vertices: usize,
    pub adjacent_shared_edges: usize,
    pub singularity_zone_count: usize,
    pub singularity_effective_band_count_min: usize,
    pub core_faces_sacrificed: usize,
    pub extra_transition_faces: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectiveBandError {
    InvalidInput(String),
    InsufficientAnnulusWidth {
        mode: TransitionBandMode,
        best_effective_band_count: usize,
        adjacent_shared_vertices: usize,
        adjacent_shared_edges: usize,
        outward_expansions: usize,
        reason: String,
    },
}

pub fn plan_effective_transition_bands(
    source: &MotherGrid,
    component: &HierarchyComponent,
    mode: TransitionBandMode,
) -> Result<TransitionBandPlan, EffectiveBandError> {
    if mode == TransitionBandMode::LegacyTwoLogicalBands {
        let coupled = extract_coupled_annulus(source, component)
            .map_err(|error| EffectiveBandError::InvalidInput(format!("{error:?}")))?;
        return Ok(build_plan(
            component,
            component,
            mode,
            TraceSelection::global(source, coupled_traces(&coupled)),
        ));
    }
    if mode == TransitionBandMode::FourEffectiveBandsNearSingularities {
        if let Err(error) = plan_effective_transition_bands(
            source,
            component,
            TransitionBandMode::ThreeEffectiveBands,
        ) {
            return Err(remap_w4_prerequisite_error(error));
        }
    }
    let parent_by_face = parent_by_source_face(source)
        .map_err(|error| EffectiveBandError::InvalidInput(format!("{error:?}")))?;
    let graph = parent_graph(source, &parent_by_face)
        .map_err(|error| EffectiveBandError::InvalidInput(format!("{error:?}")))?;
    let mut expanded = component.parents.iter().copied().collect::<BTreeSet<_>>();
    if expanded.is_empty() || expanded.iter().any(|parent| !graph.contains_key(parent)) {
        return Err(EffectiveBandError::InvalidInput(
            "transition band planner requires source parent faces".into(),
        ));
    }

    let requested = mode.planning_bands();
    let mut attempts = Vec::new();
    let mut best: Option<(usize, usize, usize)> = None;
    let mut completed_expansions = 0;
    for outward in 0..=MAX_OUTWARD_EXPANSIONS {
        completed_expansions = outward;
        let layers = parent_layers_from_outside(&expanded, &graph)
            .map_err(|error| EffectiveBandError::InvalidInput(format!("{error:?}")))?;
        let maximum_layer = layers.values().copied().max().unwrap_or(0);
        for width in requested.saturating_sub(1)..maximum_layer {
            let candidate = split_component(component.id, &expanded, &layers, width);
            let coupled = match extract_coupled_annulus(source, &candidate) {
                Ok(coupled) => coupled,
                Err(error) => {
                    attempts.push(format!(
                        "outward={outward},width={width}: extraction={error:?}"
                    ));
                    continue;
                }
            };
            let nominal = coupled_traces(&coupled);
            let Some(selection) = select_transition_traces(source, &nominal, mode) else {
                let metrics = metrics(source, &nominal);
                best = better_best(
                    best,
                    (
                        metrics.effective_bands,
                        metrics.shared_vertices,
                        metrics.shared_edges,
                    ),
                );
                attempts.push(format!(
                    "outward={outward},width={width}: nominal={},effective={},shared_vertices={},shared_edges={}",
                    nominal.len(), metrics.effective_bands, metrics.shared_vertices,
                    metrics.shared_edges,
                ));
                continue;
            };
            let plan = build_plan(component, &candidate, mode, selection);
            if plan.effective_band_count_min >= 3
                && plan.adjacent_shared_vertices == 0
                && plan.adjacent_shared_edges == 0
                && (mode != TransitionBandMode::FourEffectiveBandsNearSingularities
                    || plan.singularity_zone_count == 0
                    || plan.singularity_effective_band_count_min >= 4)
            {
                return Ok(plan);
            }
        }
        if outward == MAX_OUTWARD_EXPANSIONS {
            break;
        }
        let next = expanded
            .iter()
            .flat_map(|parent| graph[parent].iter().copied())
            .filter(|parent| !expanded.contains(parent))
            .collect::<BTreeSet<_>>();
        if next.is_empty() {
            attempts.push("no additional outward parent ring".into());
            break;
        }
        expanded.extend(next);
    }
    let (best_effective_band_count, adjacent_shared_vertices, adjacent_shared_edges) =
        best.unwrap_or((0, 0, 0));
    Err(EffectiveBandError::InsufficientAnnulusWidth {
        mode,
        best_effective_band_count,
        adjacent_shared_vertices,
        adjacent_shared_edges,
        outward_expansions: completed_expansions,
        reason: attempts.join("; "),
    })
}

pub fn parent_layer_trace_family_candidate(
    source: &MotherGrid,
    component: &HierarchyComponent,
    outward_expansions: usize,
    transition_width: usize,
) -> Result<HierarchyComponent, EffectiveBandError> {
    if outward_expansions > MAX_OUTWARD_EXPANSIONS {
        return Err(EffectiveBandError::InvalidInput(format!(
            "ParentLayerTraceFamily supports at most {MAX_OUTWARD_EXPANSIONS} outward expansions"
        )));
    }
    let parent_by_face = parent_by_source_face(source)
        .map_err(|error| EffectiveBandError::InvalidInput(format!("{error:?}")))?;
    let graph = parent_graph(source, &parent_by_face)
        .map_err(|error| EffectiveBandError::InvalidInput(format!("{error:?}")))?;
    let mut expanded = component.parents.iter().copied().collect::<BTreeSet<_>>();
    for _ in 0..outward_expansions {
        let next = expanded
            .iter()
            .flat_map(|parent| graph[parent].iter().copied())
            .filter(|parent| !expanded.contains(parent))
            .collect::<BTreeSet<_>>();
        if next.is_empty() {
            return Err(EffectiveBandError::InvalidInput(
                "ParentLayerTraceFamily has no additional outward parent ring".into(),
            ));
        }
        expanded.extend(next);
    }
    let layers = parent_layers_from_outside(&expanded, &graph)
        .map_err(|error| EffectiveBandError::InvalidInput(format!("{error:?}")))?;
    let maximum_layer = layers.values().copied().max().unwrap_or(0);
    if transition_width == 0 || transition_width >= maximum_layer {
        return Err(EffectiveBandError::InvalidInput(format!(
            "transition width {transition_width} must be in 1..{maximum_layer}"
        )));
    }
    Ok(split_component(
        component.id,
        &expanded,
        &layers,
        transition_width,
    ))
}

fn remap_w4_prerequisite_error(error: EffectiveBandError) -> EffectiveBandError {
    match error {
        EffectiveBandError::InvalidInput(reason) => EffectiveBandError::InvalidInput(reason),
        EffectiveBandError::InsufficientAnnulusWidth {
            best_effective_band_count,
            adjacent_shared_vertices,
            adjacent_shared_edges,
            outward_expansions,
            reason,
            ..
        } => EffectiveBandError::InsufficientAnnulusWidth {
            mode: TransitionBandMode::FourEffectiveBandsNearSingularities,
            best_effective_band_count,
            adjacent_shared_vertices,
            adjacent_shared_edges,
            outward_expansions,
            reason: format!(
                "global W3 prerequisite failed; local W4 zones were not evaluated: {reason}"
            ),
        },
    }
}

pub fn transition_band_plan_json(plan: &TransitionBandPlan) -> String {
    let traces = plan
        .traces
        .iter()
        .map(|trace| {
            format!(
                "[{}]",
                trace
                    .vertices
                    .iter()
                    .map(|vertex| vertex.source_slot.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let parents = plan
        .parent_faces
        .iter()
        .map(|parent| {
            format!(
                "{{\"base_face\":{},\"i\":{},\"j\":{},\"n\":{},\"orientation\":\"{:?}\"}}",
                parent.base_face, parent.i, parent.j, parent.n, parent.orientation
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"planning_family\":\"{}\",\"mode\":\"{}\",\"traces\":[{}],\"parent_faces\":[{}],\"effective_band_count_min\":{},\"effective_band_count_p50\":{},\"adjacent_shared_vertices\":{},\"adjacent_shared_edges\":{},\"singularity_zone_count\":{},\"singularity_effective_band_count_min\":{},\"core_faces_sacrificed\":{},\"extra_transition_faces\":{}}}",
        TRANSITION_BAND_PLANNING_FAMILY.as_str(), plan.mode.as_str(), traces, parents, plan.effective_band_count_min,
        plan.effective_band_count_p50, plan.adjacent_shared_vertices,
        plan.adjacent_shared_edges, plan.singularity_zone_count,
        plan.singularity_effective_band_count_min, plan.core_faces_sacrificed,
        plan.extra_transition_faces,
    )
}

pub fn effective_band_error_json(error: &EffectiveBandError) -> String {
    match error {
        EffectiveBandError::InvalidInput(reason) => format!(
            "{{\"planning_family\":\"{}\",\"outcome\":\"InvalidInput\",\"reason\":\"{}\"}}",
            TRANSITION_BAND_PLANNING_FAMILY.as_str(), json_escape(reason)
        ),
        EffectiveBandError::InsufficientAnnulusWidth {
            mode,
            best_effective_band_count,
            adjacent_shared_vertices,
            adjacent_shared_edges,
            outward_expansions,
            reason,
        } => format!(
            "{{\"planning_family\":\"{}\",\"outcome\":\"InsufficientAnnulusWidth\",\"mode\":\"{}\",\"best_effective_band_count\":{},\"adjacent_shared_vertices\":{},\"adjacent_shared_edges\":{},\"outward_expansions\":{},\"reason\":\"{}\"}}",
            TRANSITION_BAND_PLANNING_FAMILY.as_str(), mode.as_str(), best_effective_band_count, adjacent_shared_vertices,
            adjacent_shared_edges, outward_expansions, json_escape(reason),
        ),
    }
}

fn split_component(
    id: u64,
    parents: &BTreeSet<TriangleAddress>,
    layers: &BTreeMap<TriangleAddress, usize>,
    width: usize,
) -> HierarchyComponent {
    HierarchyComponent {
        id,
        parents: parents.iter().copied().collect(),
        boundary_edges: Vec::new(),
        core_parents: parents
            .iter()
            .copied()
            .filter(|parent| layers[parent] > width)
            .collect(),
        transition_parents: parents
            .iter()
            .copied()
            .filter(|parent| layers[parent] <= width)
            .collect(),
    }
}

fn coupled_traces(coupled: &CoupledAnnulus) -> Vec<RingCycle> {
    std::iter::once(coupled.coarse_interface.clone())
        .chain(coupled.intermediate_rings.iter().cloned())
        .chain(std::iter::once(coupled.fine_interface.clone()))
        .collect()
}

#[derive(Debug, Clone)]
struct TraceSelection {
    traces: Vec<RingCycle>,
    base_metrics: BandMetrics,
    singularity_zone_count: usize,
    singularity_effective_band_count_min: usize,
}

impl TraceSelection {
    fn global(source: &MotherGrid, traces: Vec<RingCycle>) -> Self {
        Self {
            base_metrics: metrics(source, &traces),
            traces,
            singularity_zone_count: 0,
            singularity_effective_band_count_min: 0,
        }
    }
}

fn select_transition_traces(
    source: &MotherGrid,
    nominal: &[RingCycle],
    mode: TransitionBandMode,
) -> Option<TraceSelection> {
    match mode {
        TransitionBandMode::LegacyTwoLogicalBands => {
            Some(TraceSelection::global(source, nominal.to_vec()))
        }
        TransitionBandMode::ThreeEffectiveBands => {
            let indices = choose_positive_indices(source, nominal, 3)?;
            Some(TraceSelection::global(
                source,
                selected_traces(nominal, &indices),
            ))
        }
        TransitionBandMode::FourEffectiveBandsNearSingularities => choose_local_w4(source, nominal),
    }
}

fn choose_positive_indices(
    source: &MotherGrid,
    nominal: &[RingCycle],
    bands: usize,
) -> Option<Vec<usize>> {
    if nominal.len() < bands + 1 {
        return None;
    }
    let mut combinations = Vec::new();
    choose_indices(
        1,
        nominal.len() - 1,
        bands - 1,
        &mut Vec::new(),
        &mut combinations,
    );
    combinations
        .into_iter()
        .filter_map(|mut internal| {
            internal.insert(0, 0);
            internal.push(nominal.len() - 1);
            internal
                .windows(2)
                .all(|pair| measure_pair(source, &nominal[pair[0]], &nominal[pair[1]]).positive)
                .then_some(internal)
        })
        .min_by_key(|indices| selection_energy(source, nominal, indices))
}

fn choose_local_w4(source: &MotherGrid, nominal: &[RingCycle]) -> Option<TraceSelection> {
    let global_w3 = choose_positive_indices(source, nominal, 3)?;
    let graph = vertex_graph(source);
    let seeds = singularity_seeds(source, nominal, &graph);
    if seeds.is_empty() {
        return Some(TraceSelection::global(
            source,
            selected_traces(nominal, &global_w3),
        ));
    }
    if nominal.len() < 5 {
        return None;
    }
    let singularity_distances = seeds
        .iter()
        .map(|&seed| graph_distances(&graph, [seed]))
        .collect::<Vec<_>>();
    let mut combinations = Vec::new();
    choose_indices(1, nominal.len() - 1, 3, &mut Vec::new(), &mut combinations);
    combinations
        .into_iter()
        .filter_map(|mut indices| {
            indices.insert(0, 0);
            indices.push(nominal.len() - 1);
            let traces = selected_traces(nominal, &indices);
            let base_indices = choose_positive_indices(source, &traces, 3)?;
            let base_traces = selected_traces(&traces, &base_indices);
            let base_metrics = metrics(source, &base_traces);
            let local = singularity_distances
                .iter()
                .map(|distances| metrics_near_seed(source, distances, &traces))
                .collect::<Vec<_>>();
            let singularity_effective_band_count_min = local
                .iter()
                .map(|metrics| metrics.effective_bands)
                .min()
                .unwrap_or(0);
            (singularity_effective_band_count_min >= 4
                && local
                    .iter()
                    .all(|metrics| metrics.shared_vertices == 0 && metrics.shared_edges == 0))
            .then_some((
                indices,
                traces,
                base_metrics,
                singularity_effective_band_count_min,
            ))
        })
        .min_by_key(|(indices, _, _, _)| selection_energy(source, nominal, indices))
        .map(
            |(_, traces, base_metrics, singularity_effective_band_count_min)| TraceSelection {
                traces,
                base_metrics,
                singularity_zone_count: seeds.len(),
                singularity_effective_band_count_min,
            },
        )
}

fn selected_traces(nominal: &[RingCycle], indices: &[usize]) -> Vec<RingCycle> {
    indices
        .iter()
        .map(|&index| nominal[index].clone())
        .collect()
}

fn choose_indices(
    next: usize,
    end: usize,
    remaining: usize,
    current: &mut Vec<usize>,
    out: &mut Vec<Vec<usize>>,
) {
    if remaining == 0 {
        out.push(current.clone());
        return;
    }
    for index in next..end {
        if end - index < remaining {
            break;
        }
        current.push(index);
        choose_indices(index + 1, end, remaining - 1, current, out);
        current.pop();
    }
}

fn selection_energy(
    source: &MotherGrid,
    nominal: &[RingCycle],
    indices: &[usize],
) -> (usize, usize, usize, usize, usize, Vec<usize>) {
    let bands = indices.len() - 1;
    let span = nominal.len() - 1;
    let distance = indices
        .windows(2)
        .map(|pair| ((pair[1] - pair[0]) * bands).abs_diff(span))
        .sum();
    let roughness = indices
        .windows(2)
        .map(|pair| {
            nominal[pair[0]]
                .vertices
                .len()
                .abs_diff(nominal[pair[1]].vertices.len())
        })
        .sum();
    let selected_vertices = indices
        .iter()
        .flat_map(|&index| nominal[index].vertices.iter())
        .filter_map(|vertex| source.addresses[vertex.source_slot].as_ref());
    let (pentagon, seam) =
        selected_vertices.fold((0usize, 0usize), |counts, address| match address {
            VertexAddress::IcosahedronVertex(_) => (counts.0 + 1, counts.1 + 1),
            VertexAddress::IcosahedronEdge { .. } => (counts.0, counts.1 + 1),
            VertexAddress::IcosahedronFace { .. } => counts,
        });
    (
        distance + roughness + pentagon + seam,
        pentagon,
        seam,
        distance,
        roughness,
        indices.to_vec(),
    )
}

fn build_plan(
    original: &HierarchyComponent,
    planned: &HierarchyComponent,
    mode: TransitionBandMode,
    selection: TraceSelection,
) -> TransitionBandPlan {
    let original_core = original
        .core_parents
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let planned_core = planned
        .core_parents
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let original_transition = original
        .transition_parents
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let planned_transition = planned
        .transition_parents
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    TransitionBandPlan {
        mode,
        traces: selection.traces,
        parent_faces: planned.parents.clone(),
        effective_band_count_min: selection.base_metrics.effective_bands,
        effective_band_count_p50: selection.base_metrics.effective_bands,
        adjacent_shared_vertices: selection.base_metrics.shared_vertices,
        adjacent_shared_edges: selection.base_metrics.shared_edges,
        singularity_zone_count: selection.singularity_zone_count,
        singularity_effective_band_count_min: selection.singularity_effective_band_count_min,
        core_faces_sacrificed: original_core.difference(&planned_core).count() * 4,
        extra_transition_faces: planned_transition.difference(&original_transition).count() * 4,
    }
}

#[derive(Debug, Clone, Copy)]
struct PairMeasurement {
    shared_vertices: usize,
    shared_edges: usize,
    positive: bool,
}

#[derive(Debug, Clone, Copy)]
struct BandMetrics {
    effective_bands: usize,
    shared_vertices: usize,
    shared_edges: usize,
}

fn metrics(source: &MotherGrid, traces: &[RingCycle]) -> BandMetrics {
    let pairs = traces
        .windows(2)
        .map(|pair| measure_pair(source, &pair[0], &pair[1]))
        .collect::<Vec<_>>();
    BandMetrics {
        effective_bands: pairs.iter().filter(|pair| pair.positive).count(),
        shared_vertices: pairs.iter().map(|pair| pair.shared_vertices).sum(),
        shared_edges: pairs.iter().map(|pair| pair.shared_edges).sum(),
    }
}

fn singularity_seeds(
    source: &MotherGrid,
    traces: &[RingCycle],
    graph: &BTreeMap<usize, BTreeSet<usize>>,
) -> Vec<usize> {
    let trace_vertices = traces
        .iter()
        .flat_map(cycle_vertices)
        .collect::<BTreeSet<_>>();
    let mut seeds = traces
        .windows(2)
        .flat_map(|pair| {
            cycle_vertices(&pair[0])
                .intersection(&cycle_vertices(&pair[1]))
                .copied()
                .collect::<Vec<_>>()
        })
        .chain(trace_vertices.iter().copied().filter(|&slot| {
            matches!(
                source.addresses[slot],
                Some(VertexAddress::IcosahedronEdge { .. } | VertexAddress::IcosahedronVertex(_))
            )
        }))
        .collect::<BTreeSet<_>>();
    let distance_to_traces = graph_distances(graph, trace_vertices.iter().copied());
    seeds.extend(
        source
            .addresses
            .iter()
            .enumerate()
            .filter_map(|(slot, address)| {
                (matches!(address, Some(VertexAddress::IcosahedronVertex(_)))
                    && distance_to_traces
                        .get(&slot)
                        .is_some_and(|&distance| distance <= 2))
                .then_some(slot)
            }),
    );
    seeds.into_iter().collect()
}

fn metrics_near_seed(
    source: &MotherGrid,
    distances: &BTreeMap<usize, usize>,
    traces: &[RingCycle],
) -> BandMetrics {
    let pairs = traces
        .windows(2)
        .map(|pair| measure_pair_near_seed(source, distances, &pair[0], &pair[1]))
        .collect::<Vec<_>>();
    BandMetrics {
        effective_bands: pairs.iter().filter(|pair| pair.positive).count(),
        shared_vertices: pairs.iter().map(|pair| pair.shared_vertices).sum(),
        shared_edges: pairs.iter().map(|pair| pair.shared_edges).sum(),
    }
}

fn vertex_graph(source: &MotherGrid) -> BTreeMap<usize, BTreeSet<usize>> {
    let mut graph = BTreeMap::<usize, BTreeSet<usize>>::new();
    for face in source.mesh.active_triangle_slots() {
        let triangle = source.mesh.triangles()[face];
        for (left, right) in triangle_edges(triangle) {
            graph.entry(left).or_default().insert(right);
            graph.entry(right).or_default().insert(left);
        }
    }
    graph
}

fn graph_distances(
    graph: &BTreeMap<usize, BTreeSet<usize>>,
    seeds: impl IntoIterator<Item = usize>,
) -> BTreeMap<usize, usize> {
    let mut distances = seeds
        .into_iter()
        .map(|seed| (seed, 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut queue = distances.keys().copied().collect::<VecDeque<_>>();
    while let Some(vertex) = queue.pop_front() {
        let distance = distances[&vertex];
        for &next in graph.get(&vertex).into_iter().flatten() {
            if let std::collections::btree_map::Entry::Vacant(entry) = distances.entry(next) {
                entry.insert(distance + 1);
                queue.push_back(next);
            }
        }
    }
    distances
}

fn better_best(
    current: Option<(usize, usize, usize)>,
    candidate: (usize, usize, usize),
) -> Option<(usize, usize, usize)> {
    match current {
        Some(current)
            if (current.0, usize::MAX - current.1, usize::MAX - current.2)
                >= (
                    candidate.0,
                    usize::MAX - candidate.1,
                    usize::MAX - candidate.2,
                ) =>
        {
            Some(current)
        }
        _ => Some(candidate),
    }
}

fn measure_pair(source: &MotherGrid, left: &RingCycle, right: &RingCycle) -> PairMeasurement {
    let left_vertices = cycle_vertices(left);
    let right_vertices = cycle_vertices(right);
    let left_edges = cycle_edges(left);
    let right_edges = cycle_edges(right);
    measure_sets(
        source,
        &left_vertices,
        &right_vertices,
        &left_edges,
        &right_edges,
    )
}

fn measure_pair_near_seed(
    source: &MotherGrid,
    distances: &BTreeMap<usize, usize>,
    left: &RingCycle,
    right: &RingCycle,
) -> PairMeasurement {
    let left_vertices = nearest_cycle_slice(left, distances);
    let right_vertices = nearest_cycle_slice(right, distances);
    let left_edges = cycle_edges(left)
        .into_iter()
        .filter(|(a, b)| left_vertices.contains(a) && left_vertices.contains(b))
        .collect::<BTreeSet<_>>();
    let right_edges = cycle_edges(right)
        .into_iter()
        .filter(|(a, b)| right_vertices.contains(a) && right_vertices.contains(b))
        .collect::<BTreeSet<_>>();
    measure_sets(
        source,
        &left_vertices,
        &right_vertices,
        &left_edges,
        &right_edges,
    )
}

fn nearest_cycle_slice(cycle: &RingCycle, distances: &BTreeMap<usize, usize>) -> BTreeSet<usize> {
    let minimum = cycle
        .vertices
        .iter()
        .filter_map(|vertex| distances.get(&vertex.source_slot).copied())
        .min();
    cycle
        .vertices
        .iter()
        .filter_map(|vertex| {
            let distance = distances.get(&vertex.source_slot).copied()?;
            minimum
                .is_some_and(|minimum| distance <= minimum + 1)
                .then_some(vertex.source_slot)
        })
        .collect()
}

fn measure_sets(
    source: &MotherGrid,
    left_vertices: &BTreeSet<usize>,
    right_vertices: &BTreeSet<usize>,
    left_edges: &BTreeSet<(usize, usize)>,
    right_edges: &BTreeSet<(usize, usize)>,
) -> PairMeasurement {
    let shared_vertices = left_vertices.intersection(right_vertices).count();
    let shared_edges = left_edges.intersection(right_edges).count();
    let strip_width = minimum_face_strip_width(source, left_vertices, right_vertices).unwrap_or(0);
    let separation = left_vertices
        .iter()
        .flat_map(|&left| {
            right_vertices.iter().map(move |&right| {
                arc_length_unit_sphere(source.mesh.vertices()[left], source.mesh.vertices()[right])
            })
        })
        .filter(|distance| distance.is_finite())
        .min_by(f64::total_cmp)
        .unwrap_or(0.0);
    PairMeasurement {
        shared_vertices,
        shared_edges,
        positive: !left_vertices.is_empty()
            && !right_vertices.is_empty()
            && shared_vertices == 0
            && shared_edges == 0
            && strip_width >= 1
            && separation > 0.0,
    }
}

fn cycle_vertices(cycle: &RingCycle) -> BTreeSet<usize> {
    cycle
        .vertices
        .iter()
        .map(|vertex| vertex.source_slot)
        .collect()
}

fn cycle_edges(cycle: &RingCycle) -> BTreeSet<(usize, usize)> {
    let vertices = cycle
        .vertices
        .iter()
        .map(|vertex| vertex.source_slot)
        .collect::<Vec<_>>();
    vertices
        .iter()
        .copied()
        .zip(vertices.iter().copied().cycle().skip(1))
        .take(vertices.len())
        .map(|(left, right)| canonical_edge(left, right))
        .collect()
}

fn minimum_face_strip_width(
    source: &MotherGrid,
    left: &BTreeSet<usize>,
    right: &BTreeSet<usize>,
) -> Option<usize> {
    let mut edge_faces = BTreeMap::<(usize, usize), Vec<usize>>::new();
    let mut left_faces = BTreeSet::new();
    let mut right_faces = BTreeSet::new();
    for face in source.mesh.active_triangle_slots() {
        let triangle = source.mesh.triangles()[face];
        if triangle.iter().any(|vertex| left.contains(vertex)) {
            left_faces.insert(face);
        }
        if triangle.iter().any(|vertex| right.contains(vertex)) {
            right_faces.insert(face);
        }
        for edge in triangle_edges(triangle) {
            edge_faces.entry(edge).or_default().push(face);
        }
    }
    if left_faces.is_empty() || right_faces.is_empty() {
        return None;
    }
    let mut adjacency = BTreeMap::<usize, BTreeSet<usize>>::new();
    for faces in edge_faces.into_values() {
        for &a in &faces {
            for &b in &faces {
                if a != b {
                    adjacency.entry(a).or_default().insert(b);
                }
            }
        }
    }
    let mut distance = left_faces
        .iter()
        .map(|&face| (face, 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut queue = left_faces.iter().copied().collect::<VecDeque<_>>();
    while let Some(face) = queue.pop_front() {
        let current = distance[&face];
        if right_faces.contains(&face) {
            return Some(current + 1);
        }
        for &next in adjacency.get(&face).into_iter().flatten() {
            if let std::collections::btree_map::Entry::Vacant(entry) = distance.entry(next) {
                entry.insert(current + 1);
                queue.push_back(next);
            }
        }
    }
    None
}

fn triangle_edges([a, b, c]: [usize; 3]) -> [(usize, usize); 3] {
    [
        canonical_edge(a, b),
        canonical_edge(b, c),
        canonical_edge(c, a),
    ]
}

fn canonical_edge(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}

fn json_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::super::{CycleOrientation, RingAnchorKind, RingVertex, RingVertexRole};
    use super::*;

    #[test]
    fn combination_order_is_stable() {
        let mut out = Vec::new();
        choose_indices(1, 5, 2, &mut Vec::new(), &mut out);
        assert_eq!(
            out,
            vec![
                vec![1, 2],
                vec![1, 3],
                vec![1, 4],
                vec![2, 3],
                vec![2, 4],
                vec![3, 4]
            ]
        );
    }

    #[test]
    fn planner_accepts_a_positive_width_w3_trace_family() {
        let source = MotherGrid::generate(24).unwrap();
        let graph = vertex_graph(&source);
        let seed = source
            .addresses
            .iter()
            .position(|address| matches!(address, Some(VertexAddress::IcosahedronVertex(0))))
            .unwrap();
        let distances = graph_distances(&graph, [seed]);
        let traces = [1usize, 3, 5, 7]
            .into_iter()
            .enumerate()
            .map(|(id, distance)| shell_cycle(&source, &graph, &distances, id, distance))
            .collect::<Vec<_>>();
        let selection =
            select_transition_traces(&source, &traces, TransitionBandMode::ThreeEffectiveBands)
                .unwrap();
        assert_eq!(selection.base_metrics.effective_bands, 3);
        assert_eq!(selection.base_metrics.shared_vertices, 0);
        assert_eq!(selection.base_metrics.shared_edges, 0);
    }

    #[test]
    fn planner_applies_w4_only_in_singularity_slices() {
        let source = MotherGrid::generate(24).unwrap();
        let graph = vertex_graph(&source);
        let anchor = source
            .addresses
            .iter()
            .position(|address| matches!(address, Some(VertexAddress::IcosahedronVertex(0))))
            .unwrap();
        let distances = graph_distances(&graph, [anchor]);
        let traces = [3usize, 5, 7, 9, 11]
            .into_iter()
            .enumerate()
            .map(|(id, distance)| shell_cycle(&source, &graph, &distances, id, distance))
            .collect::<Vec<_>>();
        let selection = select_transition_traces(
            &source,
            &traces,
            TransitionBandMode::FourEffectiveBandsNearSingularities,
        )
        .unwrap();
        assert_eq!(selection.base_metrics.effective_bands, 3);
        assert!(selection.singularity_zone_count > 0);
        assert_eq!(selection.singularity_effective_band_count_min, 4);
    }

    fn shell_cycle(
        source: &MotherGrid,
        graph: &BTreeMap<usize, BTreeSet<usize>>,
        distances: &BTreeMap<usize, usize>,
        id: usize,
        distance: usize,
    ) -> RingCycle {
        let shell = distances
            .iter()
            .filter_map(|(&slot, &actual)| (actual == distance).then_some(slot))
            .collect::<BTreeSet<_>>();
        let start = *shell.first().unwrap();
        let mut ordered = vec![start];
        let mut previous = None;
        let mut current = start;
        loop {
            let next = graph[&current]
                .iter()
                .copied()
                .filter(|next| shell.contains(next) && Some(*next) != previous)
                .min()
                .unwrap();
            if next == start {
                break;
            }
            ordered.push(next);
            previous = Some(current);
            current = next;
        }
        assert_eq!(ordered.len(), shell.len());
        RingCycle {
            id,
            vertices: ordered
                .into_iter()
                .map(|source_slot| RingVertex {
                    source_slot,
                    address: source.addresses[source_slot].clone().unwrap(),
                    role: RingVertexRole::Intermediate,
                    anchor_kind: RingAnchorKind::Ordinary,
                    fixed_position: false,
                })
                .collect(),
            orientation: CycleOrientation::SourceOrder,
            target_scale: 1.0,
        }
    }
}
