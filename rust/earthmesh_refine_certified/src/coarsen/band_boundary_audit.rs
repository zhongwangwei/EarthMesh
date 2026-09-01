//! Read-only diagnostics for legacy stratified-band representation failures.

use super::{
    build_transition_topology_domain_from_face_bands, stratified_annulus::monotone_connectors,
    topology_domain::cycles_from_edges, EssentialCycleKey, FaceBandPlan, HierarchyComponent,
};
use crate::MotherGrid;
use std::collections::{BTreeMap, BTreeSet};

type Edge = (usize, usize);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BandClassificationFailure {
    UndirectedBoundaryNotTwoRegular {
        bad_vertices: Vec<(usize, usize)>,
    },
    UndirectedBoundaryCycleCount {
        count: usize,
    },
    DirectedBoundaryWindingMismatch {
        outgoing_conflicts: Vec<usize>,
        incoming_conflicts: Vec<usize>,
    },
    LowerTraceEdgeMismatch {
        missing_from_trace: Vec<Edge>,
        extra_in_trace: Vec<Edge>,
    },
    UpperTraceEdgeMismatch {
        missing_from_trace: Vec<Edge>,
        extra_in_trace: Vec<Edge>,
    },
    BoundaryCycleMixesTraceSides,
    DirectConnectorCapacityMissing {
        lower_vertices_without_upper_edge: Vec<usize>,
    },
    NonMonotoneDirectConnectors,
    UnexpectedBandEuler {
        vertices: usize,
        edges: usize,
        faces: usize,
        euler: isize,
    },
}

impl BandClassificationFailure {
    pub fn kind(&self) -> BandClassificationFailureKind {
        match self {
            Self::UndirectedBoundaryNotTwoRegular { .. } => {
                BandClassificationFailureKind::UndirectedBoundaryNotTwoRegular
            }
            Self::UndirectedBoundaryCycleCount { .. } => {
                BandClassificationFailureKind::UndirectedBoundaryCycleCount
            }
            Self::DirectedBoundaryWindingMismatch { .. } => {
                BandClassificationFailureKind::DirectedBoundaryWindingMismatch
            }
            Self::LowerTraceEdgeMismatch { .. } => {
                BandClassificationFailureKind::LowerTraceEdgeMismatch
            }
            Self::UpperTraceEdgeMismatch { .. } => {
                BandClassificationFailureKind::UpperTraceEdgeMismatch
            }
            Self::BoundaryCycleMixesTraceSides => {
                BandClassificationFailureKind::BoundaryCycleMixesTraceSides
            }
            Self::DirectConnectorCapacityMissing { .. } => {
                BandClassificationFailureKind::DirectConnectorCapacityMissing
            }
            Self::NonMonotoneDirectConnectors => {
                BandClassificationFailureKind::NonMonotoneDirectConnectors
            }
            Self::UnexpectedBandEuler { .. } => BandClassificationFailureKind::UnexpectedBandEuler,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BandClassificationFailureKind {
    UndirectedBoundaryNotTwoRegular,
    UndirectedBoundaryCycleCount,
    DirectedBoundaryWindingMismatch,
    LowerTraceEdgeMismatch,
    UpperTraceEdgeMismatch,
    BoundaryCycleMixesTraceSides,
    DirectConnectorCapacityMissing,
    NonMonotoneDirectConnectors,
    UnexpectedBandEuler,
    None,
}

impl BandClassificationFailureKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UndirectedBoundaryNotTwoRegular => "UndirectedBoundaryNotTwoRegular",
            Self::UndirectedBoundaryCycleCount => "UndirectedBoundaryCycleCount",
            Self::DirectedBoundaryWindingMismatch => "DirectedBoundaryWindingMismatch",
            Self::LowerTraceEdgeMismatch => "LowerTraceEdgeMismatch",
            Self::UpperTraceEdgeMismatch => "UpperTraceEdgeMismatch",
            Self::BoundaryCycleMixesTraceSides => "BoundaryCycleMixesTraceSides",
            Self::DirectConnectorCapacityMissing => "DirectConnectorCapacityMissing",
            Self::NonMonotoneDirectConnectors => "NonMonotoneDirectConnectors",
            Self::UnexpectedBandEuler => "UnexpectedBandEuler",
            Self::None => "None",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BandBoundaryAudit {
    pub band_id: usize,
    pub face_count: usize,
    pub vertices: usize,
    pub edges: usize,
    pub euler: isize,
    pub undirected_boundary_edges: usize,
    pub undirected_boundary_cycle_count: usize,
    pub undirected_boundary_degree_histogram: BTreeMap<usize, usize>,
    pub directed_outdegree_violations: Vec<usize>,
    pub directed_indegree_violations: Vec<usize>,
    pub lower_trace_edges: usize,
    pub upper_trace_edges: usize,
    pub lower_boundary_match: bool,
    pub upper_boundary_match: bool,
    pub lower_vertices_with_direct_upper_connector: usize,
    pub lower_vertices_without_direct_upper_connector: Vec<usize>,
    pub failure: Option<BandClassificationFailure>,
}

impl BandBoundaryAudit {
    pub fn is_topological_annulus(&self) -> bool {
        self.euler == 0
            && self.undirected_boundary_cycle_count == 2
            && self
                .undirected_boundary_degree_histogram
                .keys()
                .all(|degree| *degree == 2)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandAuditConclusion {
    LegalAnnuliLegacyRepresentationFailure,
    BandTopologyContractFailure,
    Mixed,
}

impl BandAuditConclusion {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LegalAnnuliLegacyRepresentationFailure => {
                "LegalAnnuliLegacyRepresentationFailure"
            }
            Self::BandTopologyContractFailure => "BandTopologyContractFailure",
            Self::Mixed => "Mixed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BandBoundaryAuditSummary {
    pub cycles_audited: u64,
    pub bands_audited: u64,
    pub topological_annuli: u64,
    pub topology_contract_failures: u64,
    pub by_band_and_failure: BTreeMap<(usize, BandClassificationFailureKind), u64>,
    pub first_cycle_by_band_and_failure:
        BTreeMap<(usize, BandClassificationFailureKind), EssentialCycleKey>,
    pub first_audit_by_band_and_failure:
        BTreeMap<(usize, BandClassificationFailureKind), BandBoundaryAudit>,
}

impl BandBoundaryAuditSummary {
    pub fn record(&mut self, cycle: &EssentialCycleKey, audits: Vec<BandBoundaryAudit>) {
        self.cycles_audited += 1;
        for audit in audits {
            self.bands_audited += 1;
            if audit.is_topological_annulus() {
                self.topological_annuli += 1;
            } else {
                self.topology_contract_failures += 1;
            }
            let kind = audit
                .failure
                .as_ref()
                .map_or(BandClassificationFailureKind::None, |failure| {
                    failure.kind()
                });
            let key = (audit.band_id, kind);
            *self.by_band_and_failure.entry(key).or_default() += 1;
            self.first_cycle_by_band_and_failure
                .entry(key)
                .or_insert_with(|| cycle.clone());
            self.first_audit_by_band_and_failure
                .entry(key)
                .or_insert(audit);
        }
    }

    pub fn conclusion(&self) -> BandAuditConclusion {
        match (
            self.topological_annuli > 0,
            self.topology_contract_failures > 0,
        ) {
            (true, false) => BandAuditConclusion::LegalAnnuliLegacyRepresentationFailure,
            (false, true) => BandAuditConclusion::BandTopologyContractFailure,
            _ => BandAuditConclusion::Mixed,
        }
    }
}

pub fn audit_face_band_boundaries(
    source: &MotherGrid,
    component: &HierarchyComponent,
    plan: &FaceBandPlan,
) -> Result<Vec<BandBoundaryAudit>, String> {
    if plan.band_count != 2 || plan.interface_edges.len() != 1 {
        return Err("band boundary audit currently requires one W2 plan".into());
    }
    let domain = build_transition_topology_domain_from_face_bands(source, component, plan)
        .map_err(|error| format!("topology domain rejected audit plan: {error:?}"))?;
    let internal = domain.internal_interfaces[0].edges.clone();
    let source_edges = source
        .mesh
        .active_triangle_slots()
        .flat_map(|face| triangle_edges(source.mesh.triangles()[face]))
        .collect::<BTreeSet<_>>();

    (0..2)
        .map(|band_id| {
            let faces = plan
                .labels
                .iter()
                .filter_map(|(&face, &label)| (usize::from(label) == band_id).then_some(face))
                .collect::<BTreeSet<_>>();
            audit_band(
                source,
                band_id,
                &faces,
                &internal,
                if band_id == 0 {
                    &domain.coarse_interface.edges
                } else {
                    &domain.fine_interface.edges
                },
                &source_edges,
            )
        })
        .collect()
}

fn audit_band(
    source: &MotherGrid,
    band_id: usize,
    faces: &BTreeSet<usize>,
    internal: &BTreeSet<Edge>,
    outside_trace: &BTreeSet<Edge>,
    source_edges: &BTreeSet<Edge>,
) -> Result<BandBoundaryAudit, String> {
    let mut incidence = BTreeMap::<Edge, usize>::new();
    let mut vertices = BTreeSet::new();
    for &face in faces {
        let triangle = source.mesh.triangles()[face];
        vertices.extend(triangle);
        for edge in triangle_edges(triangle) {
            *incidence.entry(edge).or_default() += 1;
        }
    }
    let boundary = incidence
        .iter()
        .filter_map(|(&edge, &count)| (count == 1).then_some(edge))
        .collect::<BTreeSet<_>>();
    let mut boundary_degrees = BTreeMap::<usize, usize>::new();
    for &(a, b) in &boundary {
        *boundary_degrees.entry(a).or_default() += 1;
        *boundary_degrees.entry(b).or_default() += 1;
    }
    let mut degree_histogram = BTreeMap::new();
    for &degree in boundary_degrees.values() {
        *degree_histogram.entry(degree).or_default() += 1;
    }
    let bad_vertices = boundary_degrees
        .iter()
        .filter_map(|(&vertex, &degree)| (degree != 2).then_some((vertex, degree)))
        .collect::<Vec<_>>();
    let cycles = cycles_from_edges(&boundary).unwrap_or_default();
    let cycle_edges = cycles
        .iter()
        .map(|cycle| cycle_edge_set(cycle))
        .collect::<Vec<_>>();

    let mut outgoing = BTreeMap::<usize, usize>::new();
    let mut incoming = BTreeMap::<usize, usize>::new();
    for &face in faces {
        let triangle = source.mesh.triangles()[face];
        for side in 0..3 {
            let from = triangle[(side + 1) % 3];
            let to = triangle[(side + 2) % 3];
            if boundary.contains(&edge(from, to)) {
                *outgoing.entry(from).or_default() += 1;
                *incoming.entry(to).or_default() += 1;
            }
        }
    }
    let directed_outdegree_violations = boundary_degrees
        .keys()
        .filter(|vertex| outgoing.get(vertex).copied().unwrap_or_default() != 1)
        .copied()
        .collect::<Vec<_>>();
    let directed_indegree_violations = boundary_degrees
        .keys()
        .filter(|vertex| incoming.get(vertex).copied().unwrap_or_default() != 1)
        .copied()
        .collect::<Vec<_>>();

    let internal_matches = cycle_edges
        .iter()
        .enumerate()
        .filter_map(|(index, edges)| (edges == internal).then_some(index))
        .collect::<Vec<_>>();
    let (lower_cycle, upper_cycle, lower_trace, upper_trace): (
        &[usize],
        &[usize],
        &BTreeSet<Edge>,
        &BTreeSet<Edge>,
    ) = if cycle_edges.len() == 2 && internal_matches.len() == 1 {
        let internal_index = internal_matches[0];
        let outside_index = 1 - internal_index;
        if band_id == 0 {
            (
                &cycles[outside_index],
                &cycles[internal_index],
                outside_trace,
                internal,
            )
        } else {
            (
                &cycles[internal_index],
                &cycles[outside_index],
                internal,
                outside_trace,
            )
        }
    } else {
        (&[], &[], outside_trace, internal)
    };
    let lower_edges = cycle_edge_set(lower_cycle);
    let upper_edges = cycle_edge_set(upper_cycle);
    let lower_boundary_match = !lower_edges.is_empty() && lower_edges == *lower_trace;
    let upper_boundary_match = !upper_edges.is_empty() && upper_edges == *upper_trace;
    let upper_vertices = upper_cycle.iter().copied().collect::<BTreeSet<_>>();
    let lower_vertices_without_direct_upper_connector = lower_cycle
        .iter()
        .copied()
        .filter(|lower| {
            !upper_vertices
                .iter()
                .any(|upper| source_edges.contains(&edge(*lower, *upper)))
        })
        .collect::<Vec<_>>();
    let lower_vertices_with_direct_upper_connector = lower_cycle
        .len()
        .saturating_sub(lower_vertices_without_direct_upper_connector.len());
    let monotone = if lower_vertices_without_direct_upper_connector.is_empty() {
        monotone_connectors(lower_cycle, upper_cycle, source_edges).is_some()
    } else {
        false
    };
    let euler = vertices.len() as isize - incidence.len() as isize + faces.len() as isize;

    let failure = if !bad_vertices.is_empty() {
        Some(BandClassificationFailure::UndirectedBoundaryNotTwoRegular { bad_vertices })
    } else if cycles.len() != 2 {
        Some(BandClassificationFailure::UndirectedBoundaryCycleCount {
            count: cycles.len(),
        })
    } else if euler != 0 {
        Some(BandClassificationFailure::UnexpectedBandEuler {
            vertices: vertices.len(),
            edges: incidence.len(),
            faces: faces.len(),
            euler,
        })
    } else if internal_matches.len() != 1 {
        Some(BandClassificationFailure::BoundaryCycleMixesTraceSides)
    } else if !directed_outdegree_violations.is_empty() || !directed_indegree_violations.is_empty()
    {
        Some(BandClassificationFailure::DirectedBoundaryWindingMismatch {
            outgoing_conflicts: directed_outdegree_violations.clone(),
            incoming_conflicts: directed_indegree_violations.clone(),
        })
    } else if !lower_boundary_match {
        Some(BandClassificationFailure::LowerTraceEdgeMismatch {
            missing_from_trace: lower_edges.difference(lower_trace).copied().collect(),
            extra_in_trace: lower_trace.difference(&lower_edges).copied().collect(),
        })
    } else if !upper_boundary_match {
        Some(BandClassificationFailure::UpperTraceEdgeMismatch {
            missing_from_trace: upper_edges.difference(upper_trace).copied().collect(),
            extra_in_trace: upper_trace.difference(&upper_edges).copied().collect(),
        })
    } else if !lower_vertices_without_direct_upper_connector.is_empty() {
        Some(BandClassificationFailure::DirectConnectorCapacityMissing {
            lower_vertices_without_upper_edge: lower_vertices_without_direct_upper_connector
                .clone(),
        })
    } else if !monotone {
        Some(BandClassificationFailure::NonMonotoneDirectConnectors)
    } else {
        None
    };

    Ok(BandBoundaryAudit {
        band_id,
        face_count: faces.len(),
        vertices: vertices.len(),
        edges: incidence.len(),
        euler,
        undirected_boundary_edges: boundary.len(),
        undirected_boundary_cycle_count: cycles.len(),
        undirected_boundary_degree_histogram: degree_histogram,
        directed_outdegree_violations,
        directed_indegree_violations,
        lower_trace_edges: lower_trace.len(),
        upper_trace_edges: upper_trace.len(),
        lower_boundary_match,
        upper_boundary_match,
        lower_vertices_with_direct_upper_connector,
        lower_vertices_without_direct_upper_connector,
        failure,
    })
}

fn cycle_edge_set(cycle: &[usize]) -> BTreeSet<Edge> {
    cycle
        .iter()
        .copied()
        .zip(cycle.iter().copied().cycle().skip(1))
        .take(cycle.len())
        .map(|(a, b)| edge(a, b))
        .collect()
}

fn triangle_edges(triangle: [usize; 3]) -> [Edge; 3] {
    [
        edge(triangle[0], triangle[1]),
        edge(triangle[1], triangle[2]),
        edge(triangle[2], triangle[0]),
    ]
}

fn edge(a: usize, b: usize) -> Edge {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}
