//! Shared-vertex stratified CAT annulus model.
//!
//! PR36A is topology-free: it labels the PR35 annulus faces into parent-layer
//! bands, records trace occurrences/shared junction ports, and builds fixed
//! outside vertex-link contracts for later exact topology search.

use super::face_band::FaceBandPlan;
use super::{
    annulus::{
        expand_coupled_annulus_to_face_complex, parent_by_source_face, parent_graph,
        parent_layers_from_outside,
    },
    build_transition_topology_domain_from_face_bands, extract_coupled_annulus,
    topology_domain::coupled_annulus_from_topology_domain,
    BoundaryIncidenceContract, CoupledAnnulus, HierarchyComponent, RingAnchorKind, RingCycle,
};
use crate::mother_grid::{MotherGrid, VertexAddress};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DirectedHalfEdge {
    pub from: usize,
    pub to: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RingOccurrenceId {
    pub trace_id: usize,
    pub ordinal: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceRole {
    CoarseInterface,
    Intermediate,
    FineInterface,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RingOccurrence {
    pub occurrence_id: RingOccurrenceId,
    pub source_slot: usize,
    pub role: TraceRole,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectedTrace {
    pub trace_id: usize,
    pub role: TraceRole,
    pub directed_edges: Vec<DirectedHalfEdge>,
    pub occurrences: Vec<RingOccurrence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BandFaceLabel {
    pub face_slot: usize,
    pub band_id: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VertexRotation {
    pub source_slot: usize,
    pub neighbours: Vec<usize>,
    pub incident_faces: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectorPort {
    pub source_slot: usize,
    pub band_id: usize,
    pub first_face: usize,
    pub last_face: usize,
    pub entering_neighbour: usize,
    pub leaving_neighbour: usize,
    pub source_face_slots: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedJunction {
    pub source_slot: usize,
    pub lower_occurrence: RingOccurrenceId,
    pub upper_occurrence: RingOccurrenceId,
    pub band_id: usize,
    pub ports: Vec<SectorPort>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceChain {
    pub trace_id: usize,
    pub vertices: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BandComponentKind {
    Annular {
        lower_cycle: TraceChain,
        upper_cycle: TraceChain,
    },
    SectorDisk {
        start_junction: usize,
        end_junction: usize,
        lower_chain: TraceChain,
        upper_chain: TraceChain,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BandComponent {
    pub band_id: usize,
    pub face_slots: Vec<usize>,
    pub boundary_half_edges: Vec<DirectedHalfEdge>,
    pub kind: BandComponentKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedVertexLink {
    pub source_slot: usize,
    pub edges: BTreeSet<(usize, usize)>,
    pub neighbour_nodes: BTreeSet<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VertexLinkContract {
    pub source_slot: usize,
    pub fixed_link_edges: BTreeSet<(usize, usize)>,
    pub fixed_link_nodes: BTreeSet<usize>,
    pub target_degree_min: u8,
    pub target_degree_max: u8,
    pub anchor_kind: RingAnchorKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StratifiedOutcomeKind {
    Modelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TracePairSharedRecord {
    pub lower_trace_id: usize,
    pub upper_trace_id: usize,
    pub source_slots: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectorComponentRecord {
    pub band_id: usize,
    pub start_junction: usize,
    pub end_junction: usize,
    pub lower_chain: Vec<usize>,
    pub upper_chain: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StratifiedAnnulusRecord {
    pub component_id: u64,
    pub topology_trace_count: usize,
    pub band_count: usize,
    pub sector_count: usize,
    pub shared_junction_count: usize,
    pub shared_vertex_slots: Vec<usize>,
    pub unsupported_shared_edges: usize,
    pub unsupported_interleaved_junctions: usize,
    pub sector_topology_states: u64,
    pub global_merge_states: u64,
    pub anchor_link_lengths: BTreeMap<usize, usize>,
    pub anchor_fixed_link_edges: BTreeMap<usize, Vec<(usize, usize)>>,
    pub anchor_target_degree_ranges: BTreeMap<usize, (u8, u8)>,
    pub ordinary_degree_histogram: BTreeMap<usize, usize>,
    pub trace_pair_shared_slots: Vec<TracePairSharedRecord>,
    pub sector_components: Vec<SectorComponentRecord>,
    pub junction_rotation_intervals: Vec<SectorPort>,
    pub outcome: StratifiedOutcomeKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StratifiedAnnulus {
    pub coupled: CoupledAnnulus,
    pub traces: Vec<DirectedTrace>,
    pub band_face_labels: Vec<BandFaceLabel>,
    pub bands: Vec<BandComponent>,
    pub shared_junctions: Vec<SharedJunction>,
    pub link_contracts: BTreeMap<usize, VertexLinkContract>,
    pub probe: StratifiedAnnulusRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StratifiedAnnulusError {
    Extraction(super::AnnulusExtractionError),
    InvalidComponent(String),
    MissingBoundaryContract {
        source_slot: usize,
    },
    MissingFixedVertexLink {
        source_slot: usize,
    },
    UnsupportedSharedEdge {
        band_id: usize,
        edge: (usize, usize),
    },
    UnsupportedInterleavedJunction {
        band_id: usize,
        source_slot: usize,
    },
    UnsupportedInterleavedSharedOrder {
        band_id: usize,
    },
    UnsupportedMultipleBandIntervals {
        band_id: usize,
        source_slot: usize,
    },
    UnsupportedTripleTraceJunction {
        source_slot: usize,
    },
    UnsupportedNonDiskBandComponent {
        band_id: usize,
    },
    UnsupportedMultiCycleBandComponent {
        band_id: usize,
        cycles: usize,
    },
}

impl From<super::AnnulusExtractionError> for StratifiedAnnulusError {
    fn from(error: super::AnnulusExtractionError) -> Self {
        Self::Extraction(error)
    }
}

pub fn build_stratified_annulus(
    source: &MotherGrid,
    component: &HierarchyComponent,
) -> Result<StratifiedAnnulus, StratifiedAnnulusError> {
    let coupled = extract_coupled_annulus(source, component)?;
    build_stratified_annulus_from_coupled(source, component, coupled)
}

pub fn build_stratified_annulus_from_coupled(
    source: &MotherGrid,
    component: &HierarchyComponent,
    coupled: CoupledAnnulus,
) -> Result<StratifiedAnnulus, StratifiedAnnulusError> {
    if coupled.component_id != component.id {
        return Err(StratifiedAnnulusError::InvalidComponent(format!(
            "component id {} does not match coupled annulus id {}",
            component.id, coupled.component_id
        )));
    }
    let traces = directed_traces(&coupled);
    reject_triple_trace_vertices(&traces)?;
    reject_adjacent_shared_edges(source, &traces)?;
    let band_face_labels = band_face_labels(source, component, &coupled, &traces)?;
    let face_to_band = band_face_labels
        .iter()
        .map(|label| (label.face_slot, label.band_id))
        .collect::<BTreeMap<_, _>>();
    let traces = orient_traces_by_band_side(source, traces, &face_to_band)?;
    let face_components = connected_band_face_components(source, &band_face_labels);
    let contracts = boundary_contract_by_slot(&coupled.boundary_contracts);
    let shared_junctions = shared_junctions(source, &traces, &face_components, &contracts)?;
    let bands = band_components(source, &traces, face_components, &shared_junctions)?;
    let link_contracts = vertex_link_contracts(source, &coupled, &contracts)?;
    let probe = probe(
        &coupled,
        &traces,
        &bands,
        &shared_junctions,
        &link_contracts,
    );
    Ok(StratifiedAnnulus {
        coupled,
        traces,
        band_face_labels,
        bands,
        shared_junctions,
        link_contracts,
        probe,
    })
}

pub fn build_stratified_topology_domain_v2(
    source: &MotherGrid,
    component: &HierarchyComponent,
    plan: &FaceBandPlan,
) -> Result<StratifiedAnnulus, StratifiedAnnulusError> {
    let domain = build_transition_topology_domain_from_face_bands(source, component, plan)
        .map_err(|error| {
            StratifiedAnnulusError::InvalidComponent(format!(
                "topology-domain V2 rejected face-band plan: {error:?}"
            ))
        })?;
    let coupled = coupled_annulus_from_topology_domain(source, &domain).map_err(|error| {
        StratifiedAnnulusError::InvalidComponent(format!(
            "topology-domain V2 compatibility shell failed: {error:?}"
        ))
    })?;
    build_stratified_annulus_from_face_bands_with_coupled(source, component, plan, coupled)
}

pub fn build_stratified_annulus_from_face_bands_v1(
    source: &MotherGrid,
    component: &HierarchyComponent,
    plan: &FaceBandPlan,
) -> Result<StratifiedAnnulus, StratifiedAnnulusError> {
    let coupled = extract_coupled_annulus(source, component)?;
    build_stratified_annulus_from_face_bands_with_coupled(source, component, plan, coupled)
}

pub fn build_stratified_annulus_from_face_bands(
    source: &MotherGrid,
    component: &HierarchyComponent,
    plan: &FaceBandPlan,
) -> Result<StratifiedAnnulus, StratifiedAnnulusError> {
    build_stratified_annulus_from_face_bands_v1(source, component, plan)
}

fn build_stratified_annulus_from_face_bands_with_coupled(
    source: &MotherGrid,
    _component: &HierarchyComponent,
    plan: &FaceBandPlan,
    mut coupled: CoupledAnnulus,
) -> Result<StratifiedAnnulus, StratifiedAnnulusError> {
    if plan.band_count < 2 || plan.interface_edges.len() + 1 != plan.band_count {
        return Err(StratifiedAnnulusError::InvalidComponent(
            "face-band plan has inconsistent band/interface counts".into(),
        ));
    }
    let annulus_faces = coupled
        .annulus_face_slots
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let plan_faces = plan.labels.keys().copied().collect::<BTreeSet<_>>();
    if !annulus_faces.is_subset(&plan_faces)
        || plan
            .labels
            .values()
            .any(|&label| usize::from(label) >= plan.band_count)
    {
        return Err(StratifiedAnnulusError::InvalidComponent(
            "face-band labels do not cover the coupled annulus exactly".into(),
        ));
    }
    let mut face_counts = vec![0usize; plan.band_count];
    for &label in plan.labels.values() {
        face_counts[usize::from(label)] += 1;
    }
    let supplied_interfaces = plan
        .interface_edges
        .iter()
        .map(|edges| edges.iter().copied().collect::<BTreeSet<_>>())
        .collect::<Vec<_>>();
    if face_counts != plan.band_face_counts
        || expected_interface_edges(source, plan) != supplied_interfaces
    {
        return Err(StratifiedAnnulusError::InvalidComponent(
            "face-band plan evidence does not match its labels".into(),
        ));
    }
    if plan_faces != annulus_faces {
        coupled = expand_coupled_annulus_to_face_complex(source, coupled, &plan_faces)?;
    }

    let mut traces = vec![directed_trace(
        0,
        TraceRole::CoarseInterface,
        &coupled.coarse_interface,
    )];
    for (interface, edges) in plan.interface_edges.iter().enumerate() {
        traces.push(directed_interface_trace(interface + 1, edges)?);
    }
    traces.push(directed_trace(
        plan.band_count,
        TraceRole::FineInterface,
        &coupled.fine_interface,
    ));
    reject_triple_trace_vertices(&traces)?;
    reject_adjacent_shared_edges(source, &traces)?;

    let mut band_face_labels = plan
        .labels
        .iter()
        .map(|(&face_slot, &band_id)| BandFaceLabel {
            face_slot,
            band_id: usize::from(band_id),
        })
        .collect::<Vec<_>>();
    band_face_labels.sort_by_key(|label| (label.band_id, label.face_slot));
    let face_to_band = band_face_labels
        .iter()
        .map(|label| (label.face_slot, label.band_id))
        .collect::<BTreeMap<_, _>>();
    let traces = orient_traces_by_band_side(source, traces, &face_to_band)?;
    let face_components = connected_band_face_components(source, &band_face_labels);
    let component_band_ids = face_components
        .iter()
        .map(|component| component.band_id)
        .collect::<Vec<_>>();
    if component_band_ids != (0..plan.band_count).collect::<Vec<_>>() {
        return Err(StratifiedAnnulusError::InvalidComponent(format!(
            "face-band plan components are {component_band_ids:?}, expected one per band"
        )));
    }
    let contracts = boundary_contract_by_slot(&coupled.boundary_contracts);
    let shared_junctions = shared_junctions(source, &traces, &face_components, &contracts)?;
    if !shared_junctions.is_empty() {
        return Err(StratifiedAnnulusError::InvalidComponent(
            "pinch-free face-band interfaces unexpectedly share junctions".into(),
        ));
    }
    let bands = band_components(source, &traces, face_components, &shared_junctions)?;
    if bands
        .iter()
        .any(|band| !matches!(band.kind, BandComponentKind::Annular { .. }))
    {
        return Err(StratifiedAnnulusError::InvalidComponent(
            "face-band plan contains a non-annular band".into(),
        ));
    }
    let mut link_contracts = vertex_link_contracts(source, &coupled, &contracts)?;
    add_internal_interface_anchor_contracts(source, &traces, &mut link_contracts);
    let sectors = face_band_sector_components(source, &traces)?;
    let mut probe = probe(
        &coupled,
        &traces,
        &bands,
        &shared_junctions,
        &link_contracts,
    );
    probe.sector_count = sectors.len();
    probe.sector_components = sectors;
    Ok(StratifiedAnnulus {
        coupled,
        traces,
        band_face_labels,
        bands,
        shared_junctions,
        link_contracts,
        probe,
    })
}

fn add_internal_interface_anchor_contracts(
    source: &MotherGrid,
    traces: &[DirectedTrace],
    contracts: &mut BTreeMap<usize, VertexLinkContract>,
) {
    for source_slot in traces
        .iter()
        .filter(|trace| trace.role == TraceRole::Intermediate)
        .flat_map(|trace| {
            trace
                .occurrences
                .iter()
                .map(|occurrence| occurrence.source_slot)
        })
    {
        let Some(VertexAddress::IcosahedronVertex(base_vertex)) = source.addresses[source_slot]
        else {
            continue;
        };
        contracts.entry(source_slot).or_insert(VertexLinkContract {
            source_slot,
            fixed_link_edges: BTreeSet::new(),
            fixed_link_nodes: BTreeSet::new(),
            target_degree_min: 5,
            target_degree_max: 5,
            anchor_kind: RingAnchorKind::IcosahedronPentagon { base_vertex },
        });
    }
}

fn expected_interface_edges(
    source: &MotherGrid,
    plan: &FaceBandPlan,
) -> Vec<BTreeSet<(usize, usize)>> {
    let mut interfaces = vec![BTreeSet::new(); plan.band_count.saturating_sub(1)];
    for (&face, &label) in &plan.labels {
        let triangle = source.mesh.triangles()[face];
        for side in 0..3 {
            let neighbour = source.mesh.neighbours()[face][side];
            let Some(&other) = plan.labels.get(&neighbour) else {
                continue;
            };
            if label.abs_diff(other) == 1 {
                interfaces[usize::from(label.min(other))].insert(sorted_edge(
                    triangle[(side + 1) % 3],
                    triangle[(side + 2) % 3],
                ));
            }
        }
    }
    interfaces
}

fn directed_interface_trace(
    trace_id: usize,
    edges: &[(usize, usize)],
) -> Result<DirectedTrace, StratifiedAnnulusError> {
    let slots = ordered_edge_cycle(edges).ok_or_else(|| {
        StratifiedAnnulusError::InvalidComponent(format!(
            "face-band interface {trace_id} is not one simple cycle"
        ))
    })?;
    let directed_edges = slots
        .iter()
        .copied()
        .zip(slots.iter().copied().cycle().skip(1))
        .take(slots.len())
        .map(|(from, to)| DirectedHalfEdge { from, to })
        .collect();
    let occurrences = slots
        .into_iter()
        .enumerate()
        .map(|(ordinal, source_slot)| RingOccurrence {
            occurrence_id: RingOccurrenceId { trace_id, ordinal },
            source_slot,
            role: TraceRole::Intermediate,
        })
        .collect();
    Ok(DirectedTrace {
        trace_id,
        role: TraceRole::Intermediate,
        directed_edges,
        occurrences,
    })
}

fn ordered_edge_cycle(edges: &[(usize, usize)]) -> Option<Vec<usize>> {
    let mut adjacency = BTreeMap::<usize, Vec<usize>>::new();
    for &(a, b) in edges {
        if a == b {
            return None;
        }
        adjacency.entry(a).or_default().push(b);
        adjacency.entry(b).or_default().push(a);
    }
    if adjacency.len() < 3 || adjacency.values().any(|neighbours| neighbours.len() != 2) {
        return None;
    }
    for neighbours in adjacency.values_mut() {
        neighbours.sort_unstable();
        neighbours.dedup();
        if neighbours.len() != 2 {
            return None;
        }
    }
    let start = *adjacency.keys().next()?;
    let mut cycle = vec![start];
    let mut previous = usize::MAX;
    let mut current = start;
    loop {
        let next = adjacency[&current]
            .iter()
            .copied()
            .find(|candidate| *candidate != previous)?;
        if next == start {
            break;
        }
        if cycle.contains(&next) {
            return None;
        }
        cycle.push(next);
        previous = current;
        current = next;
    }
    (cycle.len() == adjacency.len()).then_some(cycle)
}

fn face_band_sector_components(
    source: &MotherGrid,
    traces: &[DirectedTrace],
) -> Result<Vec<SectorComponentRecord>, StratifiedAnnulusError> {
    let source_edges = source
        .mesh
        .active_triangle_slots()
        .flat_map(|face| {
            let triangle = source.mesh.triangles()[face];
            [
                sorted_edge(triangle[0], triangle[1]),
                sorted_edge(triangle[1], triangle[2]),
                sorted_edge(triangle[2], triangle[0]),
            ]
        })
        .collect::<BTreeSet<_>>();
    let mut sectors = Vec::new();
    for band_id in 0..traces.len().saturating_sub(1) {
        let lower = traces[band_id]
            .occurrences
            .iter()
            .map(|occurrence| occurrence.source_slot)
            .collect::<Vec<_>>();
        let upper = traces[band_id + 1]
            .occurrences
            .iter()
            .map(|occurrence| occurrence.source_slot)
            .collect::<Vec<_>>();
        let (upper, connectors) = monotone_connectors(&lower, &upper, &source_edges)
            .ok_or(StratifiedAnnulusError::UnsupportedNonDiskBandComponent { band_id })?;
        for index in 0..lower.len() {
            let next = (index + 1) % lower.len();
            let upper_start = connectors[index];
            let upper_end = if next == 0 {
                connectors[0] + upper.len()
            } else {
                connectors[next]
            };
            let upper_chain = (upper_start..=upper_end)
                .map(|position| upper[position % upper.len()])
                .collect::<Vec<_>>();
            if upper_chain.first() == upper_chain.last() && upper_chain.len() > 1 {
                return Err(StratifiedAnnulusError::UnsupportedNonDiskBandComponent { band_id });
            }
            sectors.push(SectorComponentRecord {
                band_id,
                start_junction: lower[index],
                end_junction: lower[next],
                lower_chain: vec![lower[index], lower[next]],
                upper_chain,
            });
        }
    }
    Ok(sectors)
}

pub(super) fn monotone_connectors(
    lower: &[usize],
    upper: &[usize],
    source_edges: &BTreeSet<(usize, usize)>,
) -> Option<(Vec<usize>, Vec<usize>)> {
    if lower.len() < 3 || upper.len() < 3 {
        return None;
    }
    let mut solutions = Vec::new();
    for reverse in [false, true] {
        let oriented = if reverse {
            upper.iter().copied().rev().collect::<Vec<_>>()
        } else {
            upper.to_vec()
        };
        let candidates = lower
            .iter()
            .map(|&a| {
                oriented
                    .iter()
                    .enumerate()
                    .filter_map(|(position, &b)| {
                        source_edges
                            .contains(&sorted_edge(a, b))
                            .then_some(position)
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        if candidates.iter().any(Vec::is_empty) {
            continue;
        }
        for &start in &candidates[0] {
            let mut selected = vec![start];
            for positions in candidates.iter().skip(1) {
                let previous = *selected.last().expect("seeded connectors");
                let next = positions
                    .iter()
                    .map(|&position| {
                        if position < start {
                            position + oriented.len()
                        } else {
                            position
                        }
                    })
                    .filter(|&position| position >= previous && position < start + oriented.len())
                    .min();
                let Some(next) = next else {
                    selected.clear();
                    break;
                };
                selected.push(next);
            }
            if selected.len() != lower.len() || selected.last().copied().unwrap_or(start) == start {
                continue;
            }
            let maximum_polygon_size = (0..lower.len())
                .map(|index| {
                    let next = (index + 1) % lower.len();
                    let end = if next == 0 {
                        selected[0] + oriented.len()
                    } else {
                        selected[next]
                    };
                    3 + end - selected[index]
                })
                .max()
                .unwrap_or(usize::MAX);
            solutions.push((
                maximum_polygon_size,
                reverse,
                start,
                selected,
                oriented.clone(),
            ));
        }
    }
    solutions.sort_by(|left, right| {
        (&left.0, &left.1, &left.2, &left.3).cmp(&(&right.0, &right.1, &right.2, &right.3))
    });
    solutions
        .into_iter()
        .next()
        .map(|(_, _, _, selected, oriented)| (oriented, selected))
}

pub(super) fn directed_traces(coupled: &CoupledAnnulus) -> Vec<DirectedTrace> {
    let mut rings = Vec::with_capacity(coupled.intermediate_rings.len() + 2);
    rings.push((&coupled.coarse_interface, TraceRole::CoarseInterface));
    rings.extend(
        coupled
            .intermediate_rings
            .iter()
            .map(|ring| (ring, TraceRole::Intermediate)),
    );
    rings.push((&coupled.fine_interface, TraceRole::FineInterface));
    rings
        .into_iter()
        .enumerate()
        .map(|(trace_id, (ring, role))| directed_trace(trace_id, role, ring))
        .collect()
}

fn directed_trace(trace_id: usize, role: TraceRole, ring: &RingCycle) -> DirectedTrace {
    let slots = ring
        .vertices
        .iter()
        .map(|vertex| vertex.source_slot)
        .collect::<Vec<_>>();
    let directed_edges = slots
        .iter()
        .copied()
        .zip(slots.iter().copied().cycle().skip(1))
        .take(slots.len())
        .map(|(from, to)| DirectedHalfEdge { from, to })
        .collect();
    let occurrences = slots
        .into_iter()
        .enumerate()
        .map(|(ordinal, source_slot)| RingOccurrence {
            occurrence_id: RingOccurrenceId { trace_id, ordinal },
            source_slot,
            role,
        })
        .collect();
    DirectedTrace {
        trace_id,
        role,
        directed_edges,
        occurrences,
    }
}

fn orient_traces_by_band_side(
    source: &MotherGrid,
    traces: Vec<DirectedTrace>,
    face_to_band: &BTreeMap<usize, usize>,
) -> Result<Vec<DirectedTrace>, StratifiedAnnulusError> {
    let band_count = traces.len().saturating_sub(1);
    traces
        .into_iter()
        .map(|trace| {
            let interior_band = trace.trace_id.min(band_count.saturating_sub(1));
            let boundary = oriented_band_boundary(source, face_to_band, interior_band);
            let forward = expanded_directed_edges(&trace, &boundary);
            let reversed_trace = reversed_trace(trace.clone());
            let reverse = expanded_directed_edges(&reversed_trace, &boundary);
            match (forward, reverse) {
                (Some(directed_edges), None) => Ok(DirectedTrace {
                    directed_edges,
                    ..trace
                }),
                (None, Some(directed_edges)) => Ok(DirectedTrace {
                    directed_edges,
                    ..reversed_trace
                }),
                _ => Err(StratifiedAnnulusError::InvalidComponent(format!(
                    "trace {} cannot be uniquely oriented with band {} on its left",
                    trace.trace_id, interior_band
                ))),
            }
        })
        .collect()
}

fn oriented_band_boundary(
    source: &MotherGrid,
    face_to_band: &BTreeMap<usize, usize>,
    band_id: usize,
) -> BTreeSet<(usize, usize)> {
    face_to_band
        .iter()
        .filter(|(_, actual)| **actual == band_id)
        .flat_map(|(&face, _)| {
            let tri = source.mesh.triangles()[face];
            (0..3).filter_map(move |side| {
                let neighbour = source.mesh.neighbours()[face][side];
                (face_to_band.get(&neighbour).copied() != Some(band_id))
                    .then_some((tri[(side + 1) % 3], tri[(side + 2) % 3]))
            })
        })
        .collect()
}

fn expanded_directed_edges(
    trace: &DirectedTrace,
    boundary: &BTreeSet<(usize, usize)>,
) -> Option<Vec<DirectedHalfEdge>> {
    let mut out = Vec::new();
    for edge in &trace.directed_edges {
        if boundary.contains(&(edge.from, edge.to)) {
            out.push(*edge);
            continue;
        }
        let midpoints = boundary
            .iter()
            .filter_map(|&(from, midpoint)| {
                (from == edge.from && boundary.contains(&(midpoint, edge.to))).then_some(midpoint)
            })
            .collect::<Vec<_>>();
        let [midpoint] = midpoints.as_slice() else {
            return None;
        };
        out.push(DirectedHalfEdge {
            from: edge.from,
            to: *midpoint,
        });
        out.push(DirectedHalfEdge {
            from: *midpoint,
            to: edge.to,
        });
    }
    Some(out)
}

fn reversed_trace(mut trace: DirectedTrace) -> DirectedTrace {
    trace.occurrences.reverse();
    for (ordinal, occurrence) in trace.occurrences.iter_mut().enumerate() {
        occurrence.occurrence_id.ordinal = ordinal;
    }
    let slots = trace
        .occurrences
        .iter()
        .map(|occurrence| occurrence.source_slot)
        .collect::<Vec<_>>();
    trace.directed_edges = slots
        .iter()
        .copied()
        .zip(slots.iter().copied().cycle().skip(1))
        .take(slots.len())
        .map(|(from, to)| DirectedHalfEdge { from, to })
        .collect();
    trace
}

pub(super) fn band_face_labels(
    source: &MotherGrid,
    component: &HierarchyComponent,
    coupled: &CoupledAnnulus,
    traces: &[DirectedTrace],
) -> Result<Vec<BandFaceLabel>, StratifiedAnnulusError> {
    let parent_by_face = parent_by_source_face(source)?;
    let graph = parent_graph(source, &parent_by_face)?;
    let transition = coupled
        .annulus_face_slots
        .iter()
        .map(|face| parent_by_face[face])
        .collect::<BTreeSet<_>>();
    let expected_transition = component
        .transition_parents
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if transition != expected_transition {
        return Err(StratifiedAnnulusError::InvalidComponent(
            "annulus faces do not match component transition parents".into(),
        ));
    }
    let parents = component.parents.iter().copied().collect::<BTreeSet<_>>();
    let layers = parent_layers_from_outside(&parents, &graph)?;
    let max_transition_layer = transition
        .iter()
        .filter_map(|parent| layers.get(parent).copied())
        .max()
        .ok_or_else(|| {
            StratifiedAnnulusError::InvalidComponent("missing transition layer".into())
        })?;
    let mut labels = Vec::with_capacity(coupled.annulus_face_slots.len());
    for &face_slot in &coupled.annulus_face_slots {
        let parent = parent_by_face.get(&face_slot).copied().ok_or_else(|| {
            StratifiedAnnulusError::InvalidComponent(format!(
                "source face {face_slot} has no parent"
            ))
        })?;
        let parent_layer = layers.get(&parent).copied().ok_or_else(|| {
            StratifiedAnnulusError::InvalidComponent(format!(
                "annulus face {face_slot} parent {parent:?} has no transition layer"
            ))
        })?;
        let band_id = max_transition_layer - parent_layer;
        if band_id + 1 >= traces.len() {
            return Err(StratifiedAnnulusError::InvalidComponent(format!(
                "face {face_slot} mapped outside topology band range"
            )));
        }
        labels.push(BandFaceLabel { face_slot, band_id });
    }
    labels.sort_by_key(|label| (label.band_id, label.face_slot));
    Ok(labels)
}

fn reject_triple_trace_vertices(traces: &[DirectedTrace]) -> Result<(), StratifiedAnnulusError> {
    let mut traces_by_slot = BTreeMap::<usize, BTreeSet<usize>>::new();
    for trace in traces {
        for occurrence in &trace.occurrences {
            traces_by_slot
                .entry(occurrence.source_slot)
                .or_default()
                .insert(trace.trace_id);
        }
    }
    if let Some((&source_slot, _)) = traces_by_slot
        .iter()
        .find(|(_, trace_ids)| trace_ids.len() > 2)
    {
        return Err(StratifiedAnnulusError::UnsupportedTripleTraceJunction { source_slot });
    }
    Ok(())
}

fn boundary_contract_by_slot(
    contracts: &[BoundaryIncidenceContract],
) -> BTreeMap<usize, &BoundaryIncidenceContract> {
    contracts
        .iter()
        .map(|contract| (contract.source_slot, contract))
        .collect()
}

fn shared_junctions(
    source: &MotherGrid,
    traces: &[DirectedTrace],
    components: &[BandFaceComponent],
    contracts: &BTreeMap<usize, &BoundaryIncidenceContract>,
) -> Result<Vec<SharedJunction>, StratifiedAnnulusError> {
    let mut out = Vec::new();
    for band_id in 0..traces.len() - 1 {
        reject_interleaved_shared_order(band_id, &traces[band_id], &traces[band_id + 1])?;
        let lower_by_slot = occurrence_by_slot(&traces[band_id]);
        let upper_by_slot = occurrence_by_slot(&traces[band_id + 1]);
        for &source_slot in lower_by_slot
            .keys()
            .filter(|slot| upper_by_slot.contains_key(slot))
        {
            if !contracts.contains_key(&source_slot) {
                return Err(StratifiedAnnulusError::MissingBoundaryContract { source_slot });
            }
            let rotation = vertex_rotation(source, source_slot)?;
            let mut ports = components
                .iter()
                .filter(|component| component.band_id == band_id)
                .filter(|component| {
                    rotation
                        .incident_faces
                        .iter()
                        .any(|face| component.faces.contains(face))
                })
                .map(|component| sector_port(source, &rotation, band_id, &component.faces))
                .collect::<Result<Vec<_>, _>>()?;
            ports.sort_by_key(|port| (port.first_face, port.last_face));
            if ports.is_empty() {
                return Err(StratifiedAnnulusError::UnsupportedInterleavedJunction {
                    band_id,
                    source_slot,
                });
            }
            out.push(SharedJunction {
                source_slot,
                lower_occurrence: lower_by_slot[&source_slot],
                upper_occurrence: upper_by_slot[&source_slot],
                band_id,
                ports,
            });
        }
    }
    out.sort_by_key(|junction| (junction.band_id, junction.source_slot));
    Ok(out)
}

fn reject_adjacent_shared_edges(
    source: &MotherGrid,
    traces: &[DirectedTrace],
) -> Result<(), StratifiedAnnulusError> {
    for band_id in 0..traces.len().saturating_sub(1) {
        let lower_edges = expanded_trace_edges(source, &traces[band_id])?;
        let upper_edges = expanded_trace_edges(source, &traces[band_id + 1])?;
        if let Some(edge) = lower_edges.intersection(&upper_edges).copied().next() {
            return Err(StratifiedAnnulusError::UnsupportedSharedEdge { band_id, edge });
        }
    }
    Ok(())
}

fn expanded_trace_edges(
    source: &MotherGrid,
    trace: &DirectedTrace,
) -> Result<BTreeSet<(usize, usize)>, StratifiedAnnulusError> {
    let source_edges = source
        .mesh
        .active_triangle_slots()
        .flat_map(|face| {
            let tri = source.mesh.triangles()[face];
            [
                sorted_edge(tri[0], tri[1]),
                sorted_edge(tri[1], tri[2]),
                sorted_edge(tri[2], tri[0]),
            ]
        })
        .collect::<BTreeSet<_>>();
    let mut out = BTreeSet::new();
    for edge in &trace.directed_edges {
        let direct = sorted_edge(edge.from, edge.to);
        if source_edges.contains(&direct) {
            out.insert(direct);
            continue;
        }
        let midpoints = source
            .mesh
            .active_vertex_slots()
            .filter(|&candidate| {
                source_edges.contains(&sorted_edge(edge.from, candidate))
                    && source_edges.contains(&sorted_edge(candidate, edge.to))
            })
            .collect::<Vec<_>>();
        let [midpoint] = midpoints.as_slice() else {
            return Err(StratifiedAnnulusError::InvalidComponent(format!(
                "trace {} edge {}-{} has {} two-source-edge paths, expected one",
                trace.trace_id,
                edge.from,
                edge.to,
                midpoints.len()
            )));
        };
        out.insert(sorted_edge(edge.from, *midpoint));
        out.insert(sorted_edge(*midpoint, edge.to));
    }
    Ok(out)
}

fn reject_interleaved_shared_order(
    band_id: usize,
    lower: &DirectedTrace,
    upper: &DirectedTrace,
) -> Result<(), StratifiedAnnulusError> {
    let shared = lower
        .occurrences
        .iter()
        .map(|occurrence| occurrence.source_slot)
        .filter(|slot| {
            upper
                .occurrences
                .iter()
                .any(|occurrence| occurrence.source_slot == *slot)
        })
        .collect::<Vec<_>>();
    if shared.len() <= 2 {
        return Ok(());
    }
    let upper_order = upper
        .occurrences
        .iter()
        .map(|occurrence| occurrence.source_slot)
        .filter(|slot| shared.contains(slot))
        .collect::<Vec<_>>();
    if same_cyclic_order(&shared, &upper_order) {
        return Ok(());
    }
    let mut reversed = upper_order;
    reversed.reverse();
    if same_cyclic_order(&shared, &reversed) {
        return Ok(());
    }
    Err(StratifiedAnnulusError::UnsupportedInterleavedSharedOrder { band_id })
}

fn same_cyclic_order(left: &[usize], right: &[usize]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    (0..right.len()).any(|offset| {
        left.iter()
            .copied()
            .zip(right.iter().copied().cycle().skip(offset))
            .take(left.len())
            .all(|(a, b)| a == b)
    })
}

fn occurrence_by_slot(trace: &DirectedTrace) -> BTreeMap<usize, RingOccurrenceId> {
    trace
        .occurrences
        .iter()
        .map(|occurrence| (occurrence.source_slot, occurrence.occurrence_id))
        .collect()
}

pub(super) fn vertex_rotation(
    source: &MotherGrid,
    source_slot: usize,
) -> Result<VertexRotation, StratifiedAnnulusError> {
    let seed = source
        .mesh
        .active_triangle_slots()
        .find(|&face| source.mesh.triangles()[face].contains(&source_slot))
        .ok_or_else(|| {
            StratifiedAnnulusError::InvalidComponent(format!(
                "source vertex {source_slot} has no seed face"
            ))
        })?;
    let incident_faces = source
        .mesh
        .triangle_fan_from(source_slot, seed)
        .map_err(|error| {
            StratifiedAnnulusError::InvalidComponent(format!(
                "source vertex {source_slot} fan failed: {error}"
            ))
        })?;
    let neighbours = ordered_link_neighbours(source, source_slot, &incident_faces)?;
    Ok(VertexRotation {
        source_slot,
        neighbours,
        incident_faces,
    })
}

fn ordered_link_neighbours(
    source: &MotherGrid,
    source_slot: usize,
    incident_faces: &[usize],
) -> Result<Vec<usize>, StratifiedAnnulusError> {
    let edges = incident_faces
        .iter()
        .map(|&face| directed_link_edge(source, face, source_slot))
        .collect::<Result<Vec<_>, _>>()?;
    if edges.len() < 3 {
        return Err(StratifiedAnnulusError::InvalidComponent(format!(
            "source vertex {source_slot} link has fewer than three edges"
        )));
    }
    let first_shared = shared_endpoint(edges[0], edges[1]).ok_or_else(|| {
        StratifiedAnnulusError::InvalidComponent(format!(
            "source vertex {source_slot} rotation has disconnected consecutive faces"
        ))
    })?;
    let start = other_endpoint(edges[0], first_shared);
    let mut neighbours = vec![start, first_shared];
    for edge in edges.iter().skip(1) {
        let current = *neighbours.last().expect("seeded rotation");
        if edge.0 != current && edge.1 != current {
            return Err(StratifiedAnnulusError::InvalidComponent(format!(
                "source vertex {source_slot} rotation is not a link path"
            )));
        }
        neighbours.push(other_endpoint(*edge, current));
    }
    if neighbours.pop() != Some(start) {
        return Err(StratifiedAnnulusError::InvalidComponent(format!(
            "source vertex {source_slot} rotation does not close"
        )));
    }
    Ok(neighbours)
}

fn sector_port(
    source: &MotherGrid,
    rotation: &VertexRotation,
    band_id: usize,
    component_faces: &BTreeSet<usize>,
) -> Result<SectorPort, StratifiedAnnulusError> {
    let member = rotation
        .incident_faces
        .iter()
        .map(|face| component_faces.contains(face))
        .collect::<Vec<_>>();
    let intervals = cyclic_intervals(&member);
    if intervals.len() != 1 {
        return Err(StratifiedAnnulusError::UnsupportedMultipleBandIntervals {
            band_id,
            source_slot: rotation.source_slot,
        });
    }
    let (start, end) = intervals[0];
    let faces = collect_cyclic(&rotation.incident_faces, start, end);
    let (entering_neighbour, leaving_neighbour) =
        link_path_endpoints(source, band_id, rotation.source_slot, &faces)?;
    Ok(SectorPort {
        source_slot: rotation.source_slot,
        band_id,
        first_face: rotation.incident_faces[start],
        last_face: rotation.incident_faces[end],
        entering_neighbour,
        leaving_neighbour,
        source_face_slots: faces,
    })
}

fn link_path_endpoints(
    source: &MotherGrid,
    band_id: usize,
    source_slot: usize,
    faces: &[usize],
) -> Result<(usize, usize), StratifiedAnnulusError> {
    let edges = faces
        .iter()
        .map(|&face| directed_link_edge(source, face, source_slot))
        .collect::<Result<Vec<_>, _>>()?;
    if edges.len() == 1 {
        return Ok(edges[0]);
    }
    let shared_first = shared_endpoint(edges[0], edges[1]).ok_or(
        StratifiedAnnulusError::UnsupportedInterleavedJunction {
            band_id,
            source_slot,
        },
    )?;
    let shared_last = shared_endpoint(edges[edges.len() - 2], edges[edges.len() - 1]).ok_or(
        StratifiedAnnulusError::UnsupportedInterleavedJunction {
            band_id,
            source_slot,
        },
    )?;
    let entering = other_endpoint(edges[0], shared_first);
    let leaving = other_endpoint(*edges.last().expect("non-empty"), shared_last);
    Ok((entering, leaving))
}

fn directed_link_edge(
    source: &MotherGrid,
    face: usize,
    source_slot: usize,
) -> Result<(usize, usize), StratifiedAnnulusError> {
    let tri = source.mesh.triangles()[face];
    let corner = tri
        .iter()
        .position(|slot| *slot == source_slot)
        .ok_or_else(|| {
            StratifiedAnnulusError::InvalidComponent(format!(
                "face {face} is not incident to source vertex {source_slot}"
            ))
        })?;
    Ok((tri[(corner + 1) % 3], tri[(corner + 2) % 3]))
}

fn shared_endpoint(left: (usize, usize), right: (usize, usize)) -> Option<usize> {
    [left.0, left.1]
        .into_iter()
        .find(|node| *node == right.0 || *node == right.1)
}

fn other_endpoint(edge: (usize, usize), endpoint: usize) -> usize {
    if edge.0 == endpoint {
        edge.1
    } else {
        edge.0
    }
}

fn cyclic_intervals(member: &[bool]) -> Vec<(usize, usize)> {
    if member.is_empty() || member.iter().all(|hit| !hit) {
        return Vec::new();
    }
    if member.iter().all(|hit| *hit) {
        return vec![(0, member.len() - 1)];
    }
    let mut starts = Vec::new();
    for index in 0..member.len() {
        let prev = if index == 0 {
            member.len() - 1
        } else {
            index - 1
        };
        if member[index] && !member[prev] {
            starts.push(index);
        }
    }
    starts
        .into_iter()
        .map(|start| {
            let mut end = start;
            while member[(end + 1) % member.len()] {
                end = (end + 1) % member.len();
            }
            (start, end)
        })
        .collect()
}

fn collect_cyclic<T: Copy>(items: &[T], start: usize, end: usize) -> Vec<T> {
    let mut out = vec![items[start]];
    let mut index = start;
    while index != end {
        index = (index + 1) % items.len();
        out.push(items[index]);
    }
    out
}

#[derive(Debug)]
struct BandFaceComponent {
    band_id: usize,
    faces: BTreeSet<usize>,
}

fn connected_band_face_components(
    source: &MotherGrid,
    labels: &[BandFaceLabel],
) -> Vec<BandFaceComponent> {
    let mut faces_by_band = BTreeMap::<usize, BTreeSet<usize>>::new();
    for label in labels {
        faces_by_band
            .entry(label.band_id)
            .or_default()
            .insert(label.face_slot);
    }
    let mut components = Vec::new();
    for (band_id, faces) in faces_by_band {
        let mut remaining = faces.clone();
        while let Some(&seed) = remaining.first() {
            let mut component = BTreeSet::from([seed]);
            remaining.remove(&seed);
            let mut queue = VecDeque::from([seed]);
            while let Some(face) = queue.pop_front() {
                for neighbour in source.mesh.neighbours()[face] {
                    if neighbour != 0 && faces.contains(&neighbour) && remaining.remove(&neighbour)
                    {
                        component.insert(neighbour);
                        queue.push_back(neighbour);
                    }
                }
            }
            components.push(BandFaceComponent {
                band_id,
                faces: component,
            });
        }
    }
    components.sort_by_key(|component| {
        (
            component.band_id,
            component.faces.first().copied().unwrap_or(usize::MAX),
        )
    });
    components
}

fn band_components(
    source: &MotherGrid,
    traces: &[DirectedTrace],
    face_components: Vec<BandFaceComponent>,
    shared_junctions: &[SharedJunction],
) -> Result<Vec<BandComponent>, StratifiedAnnulusError> {
    face_components
        .into_iter()
        .map(|component| {
            let face_slots = component.faces.iter().copied().collect::<Vec<_>>();
            let boundary_half_edges = boundary_half_edges(source, &component.faces);
            let kind = classify_band_component(
                component.band_id,
                &boundary_half_edges,
                &traces[component.band_id],
                &traces[component.band_id + 1],
                shared_junctions,
            )?;
            Ok(BandComponent {
                band_id: component.band_id,
                face_slots,
                boundary_half_edges,
                kind,
            })
        })
        .collect()
}

fn boundary_half_edges(source: &MotherGrid, component: &BTreeSet<usize>) -> Vec<DirectedHalfEdge> {
    let mut out = Vec::new();
    for &face in component {
        let tri = source.mesh.triangles()[face];
        for side in 0..3 {
            let neighbour = source.mesh.neighbours()[face][side];
            if neighbour == 0 || !component.contains(&neighbour) {
                out.push(DirectedHalfEdge {
                    from: tri[(side + 1) % 3],
                    to: tri[(side + 2) % 3],
                });
            }
        }
    }
    out.sort_by_key(|edge| (edge.from, edge.to));
    out
}

fn classify_band_component(
    band_id: usize,
    boundary_half_edges: &[DirectedHalfEdge],
    lower: &DirectedTrace,
    upper: &DirectedTrace,
    shared_junctions: &[SharedJunction],
) -> Result<BandComponentKind, StratifiedAnnulusError> {
    let cycles = ordered_boundary_cycles(boundary_half_edges)
        .ok_or(StratifiedAnnulusError::UnsupportedNonDiskBandComponent { band_id })?;
    if cycles.len() > 2 {
        return Err(StratifiedAnnulusError::UnsupportedMultiCycleBandComponent {
            band_id,
            cycles: cycles.len(),
        });
    }
    let lower_edges = trace_edge_set(lower);
    let upper_edges = trace_edge_set(upper);
    let boundary_vertices = boundary_half_edges
        .iter()
        .flat_map(|edge| [edge.from, edge.to])
        .collect::<BTreeSet<_>>();
    let shared = shared_junctions
        .iter()
        .filter(|junction| junction.band_id == band_id)
        .map(|junction| junction.source_slot)
        .filter(|slot| boundary_vertices.contains(slot))
        .collect::<BTreeSet<_>>();

    if shared.is_empty() && cycles.len() == 2 {
        let mut lower_cycle = None;
        let mut upper_cycle = None;
        for cycle in &cycles {
            match uniform_cycle_side(cycle, &lower_edges, &upper_edges) {
                Some(BoundarySide::Lower) if lower_cycle.is_none() => {
                    lower_cycle = Some(cycle_vertices(cycle));
                }
                Some(BoundarySide::Upper) if upper_cycle.is_none() => {
                    upper_cycle = Some(cycle_vertices(cycle));
                }
                _ => {
                    return Err(StratifiedAnnulusError::UnsupportedNonDiskBandComponent {
                        band_id,
                    });
                }
            }
        }
        return Ok(BandComponentKind::Annular {
            lower_cycle: TraceChain {
                trace_id: lower.trace_id,
                vertices: lower_cycle.expect("matched lower boundary cycle"),
            },
            upper_cycle: TraceChain {
                trace_id: upper.trace_id,
                vertices: upper_cycle.expect("matched upper boundary cycle"),
            },
        });
    }
    if shared.len() != 2 || cycles.len() != 1 {
        return Err(StratifiedAnnulusError::UnsupportedNonDiskBandComponent { band_id });
    }
    let (lower_chain, upper_chain) = sector_chains(&cycles[0], &lower_edges, &upper_edges)
        .ok_or(StratifiedAnnulusError::UnsupportedNonDiskBandComponent { band_id })?;
    let start = *lower_chain
        .first()
        .expect("sector chain contains a boundary edge");
    let end = *lower_chain
        .last()
        .expect("sector chain contains a boundary edge");
    if !shared.contains(&start)
        || !shared.contains(&end)
        || upper_chain.first() != Some(&start)
        || upper_chain.last() != Some(&end)
    {
        return Err(StratifiedAnnulusError::UnsupportedNonDiskBandComponent { band_id });
    }
    Ok(BandComponentKind::SectorDisk {
        start_junction: start,
        end_junction: end,
        lower_chain: TraceChain {
            trace_id: lower.trace_id,
            vertices: lower_chain,
        },
        upper_chain: TraceChain {
            trace_id: upper.trace_id,
            vertices: upper_chain,
        },
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BoundarySide {
    Lower,
    Upper,
}

fn ordered_boundary_cycles(edges: &[DirectedHalfEdge]) -> Option<Vec<Vec<DirectedHalfEdge>>> {
    if edges.is_empty() {
        return None;
    }
    let mut by_from = BTreeMap::new();
    let mut incoming = BTreeMap::<usize, usize>::new();
    for &edge in edges {
        if by_from.insert(edge.from, edge).is_some() {
            return None;
        }
        *incoming.entry(edge.to).or_default() += 1;
    }
    if incoming.len() != by_from.len() || incoming.values().any(|count| *count != 1) {
        return None;
    }
    let mut remaining = by_from.keys().copied().collect::<BTreeSet<_>>();
    let mut cycles = Vec::new();
    while let Some(&start) = remaining.first() {
        let mut cycle = Vec::new();
        let mut current = start;
        loop {
            if !remaining.remove(&current) {
                if current == start {
                    break;
                }
                return None;
            }
            let edge = *by_from.get(&current)?;
            cycle.push(edge);
            current = edge.to;
        }
        if cycle.len() < 3 {
            return None;
        }
        cycles.push(cycle);
    }
    Some(cycles)
}

fn trace_edge_set(trace: &DirectedTrace) -> BTreeSet<(usize, usize)> {
    trace
        .directed_edges
        .iter()
        .map(|edge| sorted_edge(edge.from, edge.to))
        .collect()
}

fn edge_side(
    edge: DirectedHalfEdge,
    lower: &BTreeSet<(usize, usize)>,
    upper: &BTreeSet<(usize, usize)>,
) -> Option<BoundarySide> {
    let edge = sorted_edge(edge.from, edge.to);
    match (lower.contains(&edge), upper.contains(&edge)) {
        (true, false) => Some(BoundarySide::Lower),
        (false, true) => Some(BoundarySide::Upper),
        _ => None,
    }
}

fn uniform_cycle_side(
    cycle: &[DirectedHalfEdge],
    lower: &BTreeSet<(usize, usize)>,
    upper: &BTreeSet<(usize, usize)>,
) -> Option<BoundarySide> {
    let first = edge_side(cycle[0], lower, upper)?;
    cycle
        .iter()
        .copied()
        .all(|edge| edge_side(edge, lower, upper) == Some(first))
        .then_some(first)
}

fn cycle_vertices(cycle: &[DirectedHalfEdge]) -> Vec<usize> {
    cycle.iter().map(|edge| edge.from).collect()
}

fn sector_chains(
    cycle: &[DirectedHalfEdge],
    lower: &BTreeSet<(usize, usize)>,
    upper: &BTreeSet<(usize, usize)>,
) -> Option<(Vec<usize>, Vec<usize>)> {
    let sides = cycle
        .iter()
        .copied()
        .map(|edge| edge_side(edge, lower, upper))
        .collect::<Option<Vec<_>>>()?;
    let starts = (0..sides.len())
        .filter(|&index| {
            sides[index] == BoundarySide::Lower
                && sides[(index + sides.len() - 1) % sides.len()] == BoundarySide::Upper
        })
        .collect::<Vec<_>>();
    let [lower_start] = starts.as_slice() else {
        return None;
    };
    let mut lower_edges = Vec::new();
    let mut index = *lower_start;
    while sides[index] == BoundarySide::Lower {
        lower_edges.push(cycle[index]);
        index = (index + 1) % cycle.len();
    }
    let mut upper_edges = Vec::new();
    while index != *lower_start {
        if sides[index] != BoundarySide::Upper {
            return None;
        }
        upper_edges.push(cycle[index]);
        index = (index + 1) % cycle.len();
    }
    let lower_chain = edge_chain_vertices(&lower_edges)?;
    let mut upper_chain = edge_chain_vertices(&upper_edges)?;
    upper_chain.reverse();
    Some((lower_chain, upper_chain))
}

fn edge_chain_vertices(edges: &[DirectedHalfEdge]) -> Option<Vec<usize>> {
    let first = *edges.first()?;
    let mut vertices = vec![first.from, first.to];
    for edge in edges.iter().skip(1) {
        if vertices.last() != Some(&edge.from) {
            return None;
        }
        vertices.push(edge.to);
    }
    Some(vertices)
}

fn vertex_link_contracts(
    source: &MotherGrid,
    coupled: &CoupledAnnulus,
    contracts: &BTreeMap<usize, &BoundaryIncidenceContract>,
) -> Result<BTreeMap<usize, VertexLinkContract>, StratifiedAnnulusError> {
    let fixed_links = fixed_vertex_links(source, &coupled.fixed_outside_face_slots);
    let mut out = BTreeMap::new();
    for (&source_slot, contract) in contracts {
        let fixed = fixed_links
            .get(&source_slot)
            .cloned()
            .ok_or(StratifiedAnnulusError::MissingFixedVertexLink { source_slot })?;
        if fixed.edges.len() != usize::from(contract.external_triangle_valence) {
            return Err(StratifiedAnnulusError::InvalidComponent(format!(
                "source vertex {source_slot} fixed link has {} edges, expected external valence {}",
                fixed.edges.len(),
                contract.external_triangle_valence
            )));
        }
        out.insert(
            source_slot,
            VertexLinkContract {
                source_slot,
                fixed_link_edges: fixed.edges,
                fixed_link_nodes: fixed.neighbour_nodes,
                target_degree_min: contract.allowed_global_degree_min,
                target_degree_max: contract.allowed_global_degree_max,
                anchor_kind: contract.anchor_kind,
            },
        );
    }
    Ok(out)
}

fn fixed_vertex_links(
    source: &MotherGrid,
    fixed_faces: &[usize],
) -> BTreeMap<usize, FixedVertexLink> {
    let mut out = BTreeMap::<usize, FixedVertexLink>::new();
    for &face in fixed_faces {
        let tri = source.mesh.triangles()[face];
        for corner in 0..3 {
            let source_slot = tri[corner];
            let a = tri[(corner + 1) % 3];
            let b = tri[(corner + 2) % 3];
            let link = out.entry(source_slot).or_insert_with(|| FixedVertexLink {
                source_slot,
                edges: BTreeSet::new(),
                neighbour_nodes: BTreeSet::new(),
            });
            link.edges.insert(sorted_edge(a, b));
            link.neighbour_nodes.insert(a);
            link.neighbour_nodes.insert(b);
        }
    }
    out
}

fn probe(
    coupled: &CoupledAnnulus,
    traces: &[DirectedTrace],
    bands: &[BandComponent],
    shared_junctions: &[SharedJunction],
    link_contracts: &BTreeMap<usize, VertexLinkContract>,
) -> StratifiedAnnulusRecord {
    let mut anchor_link_lengths = BTreeMap::new();
    let mut anchor_fixed_link_edges = BTreeMap::new();
    let mut anchor_target_degree_ranges = BTreeMap::new();
    let mut ordinary_degree_histogram = BTreeMap::new();
    for (&source_slot, contract) in link_contracts {
        if matches!(
            contract.anchor_kind,
            RingAnchorKind::IcosahedronPentagon { .. }
        ) {
            anchor_link_lengths.insert(source_slot, contract.fixed_link_edges.len());
            anchor_fixed_link_edges.insert(
                source_slot,
                contract.fixed_link_edges.iter().copied().collect(),
            );
            anchor_target_degree_ranges.insert(
                source_slot,
                (contract.target_degree_min, contract.target_degree_max),
            );
        } else {
            *ordinary_degree_histogram
                .entry(contract.fixed_link_edges.len())
                .or_default() += 1;
        }
    }
    let trace_pair_shared_slots = traces
        .windows(2)
        .map(|pair| {
            let lower = occurrence_by_slot(&pair[0]);
            let upper = occurrence_by_slot(&pair[1]);
            TracePairSharedRecord {
                lower_trace_id: pair[0].trace_id,
                upper_trace_id: pair[1].trace_id,
                source_slots: lower
                    .keys()
                    .filter(|slot| upper.contains_key(slot))
                    .copied()
                    .collect(),
            }
        })
        .collect();
    let sector_components = bands
        .iter()
        .filter_map(|band| match &band.kind {
            BandComponentKind::SectorDisk {
                start_junction,
                end_junction,
                lower_chain,
                upper_chain,
            } => Some(SectorComponentRecord {
                band_id: band.band_id,
                start_junction: *start_junction,
                end_junction: *end_junction,
                lower_chain: lower_chain.vertices.clone(),
                upper_chain: upper_chain.vertices.clone(),
            }),
            BandComponentKind::Annular { .. } => None,
        })
        .collect();
    let mut junction_rotation_intervals = shared_junctions
        .iter()
        .flat_map(|junction| junction.ports.iter().cloned())
        .collect::<Vec<_>>();
    junction_rotation_intervals.sort_by_key(|port| {
        (
            port.band_id,
            port.source_slot,
            port.first_face,
            port.last_face,
        )
    });
    StratifiedAnnulusRecord {
        component_id: coupled.component_id,
        topology_trace_count: traces.len(),
        band_count: bands
            .iter()
            .map(|band| band.band_id)
            .collect::<BTreeSet<_>>()
            .len(),
        sector_count: bands
            .iter()
            .filter(|band| matches!(band.kind, BandComponentKind::SectorDisk { .. }))
            .count(),
        shared_junction_count: shared_junctions.len(),
        shared_vertex_slots: shared_junctions
            .iter()
            .map(|junction| junction.source_slot)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        unsupported_shared_edges: 0,
        unsupported_interleaved_junctions: 0,
        sector_topology_states: 0,
        global_merge_states: 0,
        anchor_link_lengths,
        anchor_fixed_link_edges,
        anchor_target_degree_ranges,
        ordinary_degree_histogram,
        trace_pair_shared_slots,
        sector_components,
        junction_rotation_intervals,
        outcome: StratifiedOutcomeKind::Modelled,
    }
}

impl StratifiedAnnulus {
    pub fn to_record(&self) -> StratifiedAnnulusRecord {
        self.probe.clone()
    }
}

impl StratifiedAnnulusRecord {
    pub fn to_json_string(&self) -> String {
        format!(
            "{{\"component_id\":{},\"topology_trace_count\":{},\"band_count\":{},\"sector_count\":{},\"shared_junction_count\":{},\"shared_vertex_slots\":{},\"unsupported_shared_edges\":{},\"unsupported_interleaved_junctions\":{},\"sector_topology_states\":{},\"global_merge_states\":{},\"anchor_link_lengths\":{},\"anchor_fixed_link_edges\":{},\"anchor_target_degree_ranges\":{},\"ordinary_degree_histogram\":{},\"trace_pair_shared_slots\":{},\"sector_components\":{},\"junction_rotation_intervals\":{},\"outcome\":\"Modelled\"}}",
            self.component_id,
            self.topology_trace_count,
            self.band_count,
            self.sector_count,
            self.shared_junction_count,
            json_usize_list(&self.shared_vertex_slots),
            self.unsupported_shared_edges,
            self.unsupported_interleaved_junctions,
            self.sector_topology_states,
            self.global_merge_states,
            json_usize_map(&self.anchor_link_lengths),
            json_edge_map(&self.anchor_fixed_link_edges),
            json_degree_range_map(&self.anchor_target_degree_ranges),
            json_usize_map(&self.ordinary_degree_histogram),
            json_trace_pairs(&self.trace_pair_shared_slots),
            json_sector_components(&self.sector_components),
            json_rotation_intervals(&self.junction_rotation_intervals),
        )
    }
}

fn json_usize_list(items: &[usize]) -> String {
    format!(
        "[{}]",
        items
            .iter()
            .map(|item| item.to_string())
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn json_usize_map(map: &BTreeMap<usize, usize>) -> String {
    format!(
        "{{{}}}",
        map.iter()
            .map(|(key, value)| format!("\"{}\":{}", key, value))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn json_edge_map(map: &BTreeMap<usize, Vec<(usize, usize)>>) -> String {
    format!(
        "{{{}}}",
        map.iter()
            .map(|(key, edges)| format!(
                "\"{}\":[{}]",
                key,
                edges
                    .iter()
                    .map(|(a, b)| format!("[{a},{b}]"))
                    .collect::<Vec<_>>()
                    .join(",")
            ))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn json_degree_range_map(map: &BTreeMap<usize, (u8, u8)>) -> String {
    format!(
        "{{{}}}",
        map.iter()
            .map(|(key, (min, max))| format!("\"{}\":[{},{}]", key, min, max))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn json_trace_pairs(records: &[TracePairSharedRecord]) -> String {
    format!(
        "[{}]",
        records
            .iter()
            .map(|record| format!(
                "{{\"lower_trace_id\":{},\"upper_trace_id\":{},\"source_slots\":{}}}",
                record.lower_trace_id,
                record.upper_trace_id,
                json_usize_list(&record.source_slots)
            ))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn json_sector_components(records: &[SectorComponentRecord]) -> String {
    format!(
        "[{}]",
        records
            .iter()
            .map(|record| format!(
                "{{\"band_id\":{},\"start_junction\":{},\"end_junction\":{},\"lower_chain\":{},\"upper_chain\":{}}}",
                record.band_id,
                record.start_junction,
                record.end_junction,
                json_usize_list(&record.lower_chain),
                json_usize_list(&record.upper_chain),
            ))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn json_rotation_intervals(ports: &[SectorPort]) -> String {
    format!(
        "[{}]",
        ports
            .iter()
            .map(|port| format!(
                "{{\"source_slot\":{},\"band_id\":{},\"first_face\":{},\"last_face\":{},\"entering_neighbour\":{},\"leaving_neighbour\":{},\"source_face_slots\":{}}}",
                port.source_slot,
                port.band_id,
                port.first_face,
                port.last_face,
                port.entering_neighbour,
                port.leaving_neighbour,
                json_usize_list(&port.source_face_slots),
            ))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn sorted_edge(a: usize, b: usize) -> (usize, usize) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjacent_traces_sharing_a_source_edge_fail_closed() {
        let source = MotherGrid::generate(2).unwrap();
        let face = source.mesh.active_triangle_slots().next().unwrap();
        let [a, b, _] = source.mesh.triangles()[face];
        let trace = |trace_id| DirectedTrace {
            trace_id,
            role: TraceRole::Intermediate,
            directed_edges: vec![DirectedHalfEdge { from: a, to: b }],
            occurrences: Vec::new(),
        };

        assert_eq!(
            reject_adjacent_shared_edges(&source, &[trace(0), trace(1)]),
            Err(StratifiedAnnulusError::UnsupportedSharedEdge {
                band_id: 0,
                edge: sorted_edge(a, b),
            })
        );
    }
}
