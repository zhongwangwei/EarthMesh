//! Plan-native V3 transition cells without a `CoupledAnnulus` compatibility shell.

use super::{
    build_plan_band_domains, build_transition_topology_domain_from_face_bands,
    stratified_annulus::fixed_vertex_links, FaceBandPlan, HierarchyComponent, PlanBandDomain,
    PlanBandTopologyKind, TopologyBoundary, TransitionTopologyDomain, VertexLinkContract,
};
use crate::MotherGrid;
use std::collections::{BTreeMap, BTreeSet};

type Edge = (usize, usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CellVertexOccurrence {
    pub source_slot: usize,
    pub ordinal: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskCellDomain {
    pub cell_id: u64,
    pub polygon_vertices: Vec<CellVertexOccurrence>,
    pub boundary_edges: Vec<(CellVertexOccurrence, CellVertexOccurrence)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopologyBoundaryKind {
    SourceCycle,
    ContractedCoarseCycle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnularCellKey(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnularCellDomain {
    pub cell_id: u64,
    pub lower_cycle: Vec<usize>,
    pub upper_cycle: Vec<usize>,
    pub lower_boundary_kind: TopologyBoundaryKind,
    pub upper_boundary_kind: TopologyBoundaryKind,
    pub forbidden_global_edges: BTreeSet<Edge>,
    pub fixed_outside_link_contracts: BTreeMap<usize, VertexLinkContract>,
    pub cell_key: AnnularCellKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionCellDomain {
    Disk(DiskCellDomain),
    Annulus(AnnularCellDomain),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StratifiedTransitionDomainV3Key(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StratifiedTransitionDomainV3 {
    pub topology_domain: TransitionTopologyDomain,
    pub bands: Vec<PlanBandDomain>,
    pub cells: Vec<TransitionCellDomain>,
    pub link_contracts: BTreeMap<usize, VertexLinkContract>,
    pub domain_key: StratifiedTransitionDomainV3Key,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StratifiedV3Error {
    TopologyDomain(String),
    BandDomain(String),
    MissingFixedVertexLink {
        source_slot: usize,
    },
    InvalidFixedVertexLink {
        source_slot: usize,
        actual: usize,
        expected: usize,
    },
    NonAnnularW2Band {
        band_id: usize,
    },
}

pub fn build_stratified_transition_domain_v3(
    source: &MotherGrid,
    component: &HierarchyComponent,
    plan: &FaceBandPlan,
) -> Result<StratifiedTransitionDomainV3, StratifiedV3Error> {
    let topology_domain = build_transition_topology_domain_from_face_bands(source, component, plan)
        .map_err(|error| StratifiedV3Error::TopologyDomain(format!("{error:?}")))?;
    let bands = build_plan_band_domains(source, component, plan)
        .map_err(|error| StratifiedV3Error::BandDomain(format!("{error:?}")))?;
    let link_contracts = link_contracts_from_topology_domain(source, &topology_domain)?;
    let fixed_edges = fixed_face_edges(source, &topology_domain.fixed_outside_face_slots);
    let cells = bands
        .iter()
        .map(|band| {
            if band.topology_kind != PlanBandTopologyKind::Annulus {
                return Err(StratifiedV3Error::NonAnnularW2Band {
                    band_id: band.band_id,
                });
            }
            let lower_cycle = band.lower_boundary.topology_vertices().to_vec();
            let upper_cycle = band.upper_boundary.topology_vertices().to_vec();
            let boundary_edges = band
                .lower_boundary
                .topology_edges()
                .into_iter()
                .chain(band.upper_boundary.topology_edges())
                .collect::<BTreeSet<_>>();
            let cell_vertices = lower_cycle
                .iter()
                .chain(&upper_cycle)
                .copied()
                .collect::<BTreeSet<_>>();
            let forbidden_global_edges = fixed_edges
                .iter()
                .copied()
                .filter(|(a, b)| {
                    cell_vertices.contains(a)
                        && cell_vertices.contains(b)
                        && !boundary_edges.contains(&edge(*a, *b))
                })
                .collect::<BTreeSet<_>>();
            let fixed_outside_link_contracts = link_contracts
                .iter()
                .filter(|(vertex, _)| cell_vertices.contains(vertex))
                .map(|(&vertex, contract)| (vertex, contract.clone()))
                .collect::<BTreeMap<_, _>>();
            let cell_id = component
                .id
                .wrapping_mul(2)
                .wrapping_add(band.band_id as u64);
            let cell_key = AnnularCellKey(format!(
                "{:016x}",
                fnv1a(
                    format!(
                        "{cell_id}|{:?}|{:?}|{:?}",
                        lower_cycle, upper_cycle, forbidden_global_edges
                    )
                    .bytes()
                )
            ));
            Ok(TransitionCellDomain::Annulus(AnnularCellDomain {
                cell_id,
                lower_cycle,
                upper_cycle,
                lower_boundary_kind: boundary_kind(&band.lower_boundary),
                upper_boundary_kind: boundary_kind(&band.upper_boundary),
                forbidden_global_edges,
                fixed_outside_link_contracts,
                cell_key,
            }))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let domain_key = StratifiedTransitionDomainV3Key(format!(
        "{:016x}",
        fnv1a(
            format!(
                "{}|{:?}|{:?}",
                topology_domain.topology_key.0,
                bands
                    .iter()
                    .map(|band| &band.band_key.0)
                    .collect::<Vec<_>>(),
                cells
                    .iter()
                    .map(|cell| match cell {
                        TransitionCellDomain::Disk(cell) => cell.cell_id.to_string(),
                        TransitionCellDomain::Annulus(cell) => cell.cell_key.0.clone(),
                    })
                    .collect::<Vec<_>>()
            )
            .bytes()
        )
    ));
    Ok(StratifiedTransitionDomainV3 {
        topology_domain,
        bands,
        cells,
        link_contracts,
        domain_key,
    })
}

fn link_contracts_from_topology_domain(
    source: &MotherGrid,
    domain: &TransitionTopologyDomain,
) -> Result<BTreeMap<usize, VertexLinkContract>, StratifiedV3Error> {
    let fixed_faces = domain
        .fixed_outside_face_slots
        .iter()
        .copied()
        .collect::<Vec<_>>();
    let fixed_links = fixed_vertex_links(source, &fixed_faces);
    domain
        .boundary_contracts
        .iter()
        .map(|contract| {
            let fixed = fixed_links.get(&contract.source_slot).ok_or(
                StratifiedV3Error::MissingFixedVertexLink {
                    source_slot: contract.source_slot,
                },
            )?;
            let expected = usize::from(contract.external_triangle_valence);
            if fixed.edges.len() != expected {
                return Err(StratifiedV3Error::InvalidFixedVertexLink {
                    source_slot: contract.source_slot,
                    actual: fixed.edges.len(),
                    expected,
                });
            }
            Ok((
                contract.source_slot,
                VertexLinkContract {
                    source_slot: contract.source_slot,
                    fixed_link_edges: fixed.edges.clone(),
                    fixed_link_nodes: fixed.neighbour_nodes.clone(),
                    target_degree_min: contract.allowed_global_degree_min,
                    target_degree_max: contract.allowed_global_degree_max,
                    anchor_kind: contract.anchor_kind,
                },
            ))
        })
        .collect()
}

fn fixed_face_edges(source: &MotherGrid, faces: &BTreeSet<usize>) -> BTreeSet<Edge> {
    faces
        .iter()
        .flat_map(|&face| {
            let triangle = source.mesh.triangles()[face];
            [
                edge(triangle[0], triangle[1]),
                edge(triangle[1], triangle[2]),
                edge(triangle[2], triangle[0]),
            ]
        })
        .collect()
}

fn boundary_kind(boundary: &TopologyBoundary) -> TopologyBoundaryKind {
    match boundary {
        TopologyBoundary::SourceCycle(_) => TopologyBoundaryKind::SourceCycle,
        TopologyBoundary::ContractedCoarseCycle { .. } => {
            TopologyBoundaryKind::ContractedCoarseCycle
        }
    }
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
