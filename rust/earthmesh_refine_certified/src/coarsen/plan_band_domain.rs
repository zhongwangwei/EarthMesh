//! Plan-native topology domains for general W2 annular bands.

use super::{
    build_transition_topology_domain_from_face_bands, topology_domain::cycles_from_edges,
    FaceBandPlan, HierarchyComponent,
};
use crate::MotherGrid;
use std::collections::{BTreeMap, BTreeSet};

type Edge = (usize, usize);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceBoundaryCycle {
    pub ordered_vertices: Vec<usize>,
    pub edges: BTreeSet<Edge>,
    pub cycle_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractedBoundaryEdge {
    pub topology_edge: Edge,
    pub source_path: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryContractionMap {
    pub coarse_edges: Vec<ContractedBoundaryEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TopologyBoundary {
    SourceCycle(SourceBoundaryCycle),
    ContractedCoarseCycle {
        topology_vertices: Vec<usize>,
        topology_edges: BTreeSet<Edge>,
        source_expansion: BoundaryContractionMap,
    },
}

impl TopologyBoundary {
    pub fn topology_vertices(&self) -> &[usize] {
        match self {
            Self::SourceCycle(cycle) => &cycle.ordered_vertices,
            Self::ContractedCoarseCycle {
                topology_vertices, ..
            } => topology_vertices,
        }
    }

    pub fn source_edges(&self) -> BTreeSet<Edge> {
        match self {
            Self::SourceCycle(cycle) => cycle.edges.clone(),
            Self::ContractedCoarseCycle {
                source_expansion, ..
            } => source_expansion
                .coarse_edges
                .iter()
                .flat_map(|edge| path_edges(&edge.source_path))
                .collect(),
        }
    }

    pub fn topology_edges(&self) -> BTreeSet<Edge> {
        match self {
            Self::SourceCycle(cycle) => cycle.edges.clone(),
            Self::ContractedCoarseCycle { topology_edges, .. } => topology_edges.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanBandTopologyKind {
    Disk,
    Annulus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanBandDomainKey(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanBandDomain {
    pub band_id: usize,
    pub face_slots: BTreeSet<usize>,
    pub source_boundary_cycles: Vec<SourceBoundaryCycle>,
    pub lower_boundary: TopologyBoundary,
    pub upper_boundary: TopologyBoundary,
    pub euler: isize,
    pub topology_kind: PlanBandTopologyKind,
    pub band_key: PlanBandDomainKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanBandDomainError {
    InvalidPlan(String),
    BoundaryNotTwoRegular { band_id: usize },
    UnexpectedBoundaryCycleCount { band_id: usize, count: usize },
    UnexpectedEuler { band_id: usize, euler: isize },
    InternalInterfaceMismatch { band_id: usize },
    OutsideInterfaceMismatch { band_id: usize },
    CoarseBoundaryVertexMissing { vertex: usize },
    UnsupportedCoarseSourcePathLength { edge: Edge, source_edges: usize },
    InvalidCoarseContraction,
}

pub fn build_plan_band_domains(
    source: &MotherGrid,
    component: &HierarchyComponent,
    plan: &FaceBandPlan,
) -> Result<Vec<PlanBandDomain>, PlanBandDomainError> {
    if plan.band_count != 2 || plan.interface_edges.len() != 1 {
        return Err(PlanBandDomainError::InvalidPlan(
            "plan-native band domains currently require W2".into(),
        ));
    }
    let topology = build_transition_topology_domain_from_face_bands(source, component, plan)
        .map_err(|error| PlanBandDomainError::InvalidPlan(format!("{error:?}")))?;
    let internal = &topology.internal_interfaces[0].edges;

    (0..2)
        .map(|band_id| {
            let face_slots = plan
                .labels
                .iter()
                .filter_map(|(&face, &label)| (usize::from(label) == band_id).then_some(face))
                .collect::<BTreeSet<_>>();
            let (vertices, edges, boundary_edges) = band_complex(source, &face_slots);
            let euler = vertices.len() as isize - edges.len() as isize + face_slots.len() as isize;
            if euler != 0 {
                return Err(PlanBandDomainError::UnexpectedEuler { band_id, euler });
            }
            let Some(cycles) = cycles_from_edges(&boundary_edges) else {
                return Err(PlanBandDomainError::BoundaryNotTwoRegular { band_id });
            };
            if cycles.len() != 2 {
                return Err(PlanBandDomainError::UnexpectedBoundaryCycleCount {
                    band_id,
                    count: cycles.len(),
                });
            }
            let source_boundary_cycles = cycles.into_iter().map(source_cycle).collect::<Vec<_>>();
            let internal_matches = source_boundary_cycles
                .iter()
                .enumerate()
                .filter_map(|(index, cycle)| (cycle.edges == *internal).then_some(index))
                .collect::<Vec<_>>();
            if internal_matches.len() != 1 {
                return Err(PlanBandDomainError::InternalInterfaceMismatch { band_id });
            }
            let internal_index = internal_matches[0];
            let outside_index = 1 - internal_index;
            let internal_cycle = source_boundary_cycles[internal_index].clone();
            let outside_cycle = source_boundary_cycles[outside_index].clone();
            let (lower_boundary, upper_boundary) = if band_id == 0 {
                let source_expansion = contract_coarse_boundary(
                    &topology.coarse_interface.ordered_vertices,
                    &topology.coarse_interface.edges,
                    &outside_cycle,
                )?;
                (
                    TopologyBoundary::ContractedCoarseCycle {
                        topology_vertices: topology.coarse_interface.ordered_vertices.clone(),
                        topology_edges: topology.coarse_interface.edges.clone(),
                        source_expansion,
                    },
                    TopologyBoundary::SourceCycle(internal_cycle),
                )
            } else {
                if outside_cycle.edges != topology.fine_interface.edges {
                    return Err(PlanBandDomainError::OutsideInterfaceMismatch { band_id });
                }
                (
                    TopologyBoundary::SourceCycle(internal_cycle),
                    TopologyBoundary::SourceCycle(outside_cycle),
                )
            };
            let band_key = PlanBandDomainKey(format!(
                "{:016x}",
                fnv1a(
                    format!(
                        "{band_id}|{:?}|{:?}|{:?}",
                        face_slots,
                        lower_boundary.topology_vertices(),
                        upper_boundary.topology_vertices(),
                    )
                    .bytes()
                )
            ));
            Ok(PlanBandDomain {
                band_id,
                face_slots,
                source_boundary_cycles,
                lower_boundary,
                upper_boundary,
                euler,
                topology_kind: PlanBandTopologyKind::Annulus,
                band_key,
            })
        })
        .collect()
}

fn contract_coarse_boundary(
    topology_vertices: &[usize],
    topology_edges: &BTreeSet<Edge>,
    source_cycle: &SourceBoundaryCycle,
) -> Result<BoundaryContractionMap, PlanBandDomainError> {
    if topology_vertices
        .iter()
        .any(|vertex| !source_cycle.ordered_vertices.contains(vertex))
    {
        let vertex = topology_vertices
            .iter()
            .find(|vertex| !source_cycle.ordered_vertices.contains(vertex))
            .copied()
            .unwrap();
        return Err(PlanBandDomainError::CoarseBoundaryVertexMissing { vertex });
    }
    let mut coarse_edges = Vec::with_capacity(topology_vertices.len());
    for (a, b) in topology_vertices
        .iter()
        .copied()
        .zip(topology_vertices.iter().copied().cycle().skip(1))
        .take(topology_vertices.len())
    {
        let topology_edge = edge(a, b);
        if !topology_edges.contains(&topology_edge) {
            return Err(PlanBandDomainError::InvalidCoarseContraction);
        }
        let source_path = shortest_cycle_path(&source_cycle.ordered_vertices, a, b)
            .ok_or(PlanBandDomainError::CoarseBoundaryVertexMissing { vertex: a })?;
        let source_edge_count = source_path.len().saturating_sub(1);
        if !(1..=2).contains(&source_edge_count) {
            return Err(PlanBandDomainError::UnsupportedCoarseSourcePathLength {
                edge: topology_edge,
                source_edges: source_edge_count,
            });
        }
        coarse_edges.push(ContractedBoundaryEdge {
            topology_edge,
            source_path,
        });
    }
    let covered = coarse_edges
        .iter()
        .flat_map(|edge| path_edges(&edge.source_path))
        .collect::<BTreeSet<_>>();
    let interior_vertices = coarse_edges
        .iter()
        .flat_map(|edge| {
            edge.source_path
                .iter()
                .copied()
                .skip(1)
                .take(edge.source_path.len().saturating_sub(2))
        })
        .collect::<Vec<_>>();
    if covered != source_cycle.edges
        || interior_vertices
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len()
            != interior_vertices.len()
    {
        return Err(PlanBandDomainError::InvalidCoarseContraction);
    }
    Ok(BoundaryContractionMap { coarse_edges })
}

fn shortest_cycle_path(cycle: &[usize], from: usize, to: usize) -> Option<Vec<usize>> {
    let start = cycle.iter().position(|vertex| *vertex == from)?;
    let end = cycle.iter().position(|vertex| *vertex == to)?;
    let walk = |step: isize| {
        let mut path = vec![from];
        let mut index = start;
        while index != end {
            index = (index as isize + step).rem_euclid(cycle.len() as isize) as usize;
            path.push(cycle[index]);
        }
        path
    };
    let forward = walk(1);
    let reverse = walk(-1);
    Some(if (forward.len(), &forward) <= (reverse.len(), &reverse) {
        forward
    } else {
        reverse
    })
}

fn band_complex(
    source: &MotherGrid,
    faces: &BTreeSet<usize>,
) -> (BTreeSet<usize>, BTreeSet<Edge>, BTreeSet<Edge>) {
    let mut vertices = BTreeSet::new();
    let mut incidence = BTreeMap::<Edge, usize>::new();
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
        .collect();
    (vertices, incidence.into_keys().collect(), boundary)
}

fn source_cycle(ordered_vertices: Vec<usize>) -> SourceBoundaryCycle {
    let edges = path_edges_closed(&ordered_vertices);
    let cycle_key = format!("{:016x}", fnv1a(format!("{ordered_vertices:?}").bytes()));
    SourceBoundaryCycle {
        ordered_vertices,
        edges,
        cycle_key,
    }
}

fn path_edges(path: &[usize]) -> impl Iterator<Item = Edge> + '_ {
    path.windows(2).map(|pair| edge(pair[0], pair[1]))
}

fn path_edges_closed(path: &[usize]) -> BTreeSet<Edge> {
    path.iter()
        .copied()
        .zip(path.iter().copied().cycle().skip(1))
        .take(path.len())
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

fn fnv1a(bytes: impl Iterator<Item = u8>) -> u64 {
    bytes.fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}
