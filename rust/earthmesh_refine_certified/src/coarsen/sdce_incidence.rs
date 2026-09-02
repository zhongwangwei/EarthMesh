//! Exact fixed-topology and global degree/link contracts for SDCE.
//!
//! This module deliberately stops before incidence-plan or concrete-topology
//! search.  It only builds the domains those later exact solvers consume.

use super::{
    annular_reachability::{contracted_fixed_link_signature, path_signature},
    global_exact_merge::{edge_counts, fixed_triangles_for_face_complex, vertex_degrees},
    AnnularCellDomain, HierarchyComponent, LinkPathSignature, RingAnchorKind,
    StratifiedTransitionDomainV3, TransitionCellDomain,
};
use crate::{MotherGrid, VertexAddress};
use std::collections::{BTreeMap, BTreeSet};

type Edge = (usize, usize);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedFinalTopologyContextKey(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalIncidenceContractKey(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedFinalTopologyContext {
    pub triangles: Vec<[usize; 3]>,
    pub vertex_degrees: BTreeMap<usize, u8>,
    pub vertex_link_edges: BTreeMap<usize, BTreeSet<Edge>>,
    pub edge_counts: BTreeMap<Edge, u8>,
    pub context_key: FixedFinalTopologyContextKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalIncidenceContract {
    pub cell_ids: Vec<u64>,
    pub fixed: FixedFinalTopologyContext,
    pub vertex_domains: BTreeMap<usize, GlobalVertexIncidenceDomain>,
    pub cell_triangle_counts: BTreeMap<u64, usize>,
    pub cell_incidence_sums: BTreeMap<u64, usize>,
    pub target_transition_charge: i16,
    pub contract_key: GlobalIncidenceContractKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalVertexIncidenceDomain {
    pub source_slot: usize,
    pub owners: Vec<u64>,
    pub fixed_degree: u8,
    pub fixed_link: LinkPathSignature,
    pub legal_final_degrees: BTreeSet<u8>,
    pub allowed_owner_tuples: Vec<VertexOwnerIncidenceTuple>,
    pub anchor_kind: RingAnchorKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VertexOwnerIncidenceTuple {
    pub final_degree: u8,
    pub owner_counts: Vec<(u64, u8)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GlobalIncidenceContractError {
    FixedTopology(String),
    ArithmeticOverflow(&'static str),
    NonAnnularCell {
        cell_id: u64,
    },
    InvalidCellBoundary {
        cell_id: u64,
        source_slot: usize,
    },
    InvalidFixedLink {
        source_slot: usize,
    },
    AdapterMismatch {
        source_slot: usize,
        reason: String,
    },
    InvalidFixedOnlyDegree {
        source_slot: usize,
        actual: u8,
        expected: String,
    },
    InvalidPathProviderCount {
        source_slot: usize,
        actual: usize,
    },
    IncompatiblePathEndpoints {
        source_slot: usize,
    },
    NoLegalOwnerTuple {
        source_slot: usize,
    },
}

pub fn build_fixed_final_topology_context(
    source: &MotherGrid,
    component: &HierarchyComponent,
    domain: &StratifiedTransitionDomainV3,
) -> Result<FixedFinalTopologyContext, GlobalIncidenceContractError> {
    let annulus_faces = domain
        .topology_domain
        .annulus_face_slots
        .iter()
        .copied()
        .collect::<Vec<_>>();
    let triangles = fixed_triangles_for_face_complex(source, component, &annulus_faces)
        .map_err(GlobalIncidenceContractError::FixedTopology)?;
    let vertex_degrees = vertex_degrees(&triangles)
        .into_iter()
        .map(|(slot, degree)| {
            u8::try_from(degree)
                .map(|degree| (slot, degree))
                .map_err(|_| GlobalIncidenceContractError::ArithmeticOverflow("fixed degree"))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let edge_counts = edge_counts(&triangles)
        .into_iter()
        .map(|(edge, count)| {
            u8::try_from(count)
                .map(|count| (edge, count))
                .map_err(|_| GlobalIncidenceContractError::ArithmeticOverflow("edge count"))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let mut vertex_link_edges = BTreeMap::<usize, BTreeSet<Edge>>::new();
    for [a, b, c] in triangles.iter().copied() {
        vertex_link_edges.entry(a).or_default().insert(edge(b, c));
        vertex_link_edges.entry(b).or_default().insert(edge(a, c));
        vertex_link_edges.entry(c).or_default().insert(edge(a, b));
    }
    let context_key = FixedFinalTopologyContextKey(format!(
        "{:016x}",
        fnv1a(format!("{:?}|{:?}", triangles, edge_counts).bytes())
    ));
    Ok(FixedFinalTopologyContext {
        triangles,
        vertex_degrees,
        vertex_link_edges,
        edge_counts,
        context_key,
    })
}

pub fn build_global_incidence_contract(
    source: &MotherGrid,
    component: &HierarchyComponent,
    domain: &StratifiedTransitionDomainV3,
) -> Result<GlobalIncidenceContract, GlobalIncidenceContractError> {
    let fixed = build_fixed_final_topology_context(source, component, domain)?;
    let cells = annular_cells(domain)?;
    let owners = cell_owners(cells.values().copied());
    validate_fixed_only_vertices(source, &fixed, &owners)?;

    let mut vertex_domains = BTreeMap::new();
    for (&source_slot, owner_ids) in &owners {
        let anchor_kind = anchor_kind(source, source_slot);
        let fixed_degree = fixed.vertex_degrees.get(&source_slot).copied().unwrap_or(0);
        let fixed_edges = fixed
            .vertex_link_edges
            .get(&source_slot)
            .cloned()
            .unwrap_or_default();
        let fixed_link = path_signature(&fixed_edges)
            .ok_or(GlobalIncidenceContractError::InvalidFixedLink { source_slot })?;
        validate_adapter_contract(
            domain,
            source_slot,
            anchor_kind,
            fixed_link,
            domain.link_contracts.get(&source_slot),
        )?;
        validate_path_providers(source_slot, fixed_link, owner_ids, &cells)?;
        let legal_final_degrees = legal_degrees(anchor_kind);
        let allowed_owner_tuples = owner_tuples(fixed_degree, owner_ids, &legal_final_degrees);
        if allowed_owner_tuples.is_empty() {
            return Err(GlobalIncidenceContractError::NoLegalOwnerTuple { source_slot });
        }
        vertex_domains.insert(
            source_slot,
            GlobalVertexIncidenceDomain {
                source_slot,
                owners: owner_ids.iter().copied().collect(),
                fixed_degree,
                fixed_link,
                legal_final_degrees,
                allowed_owner_tuples,
                anchor_kind,
            },
        );
    }

    let cell_triangle_counts = cells
        .iter()
        .map(|(&id, cell)| (id, cell.lower_cycle.len() + cell.upper_cycle.len()))
        .collect::<BTreeMap<_, _>>();
    let cell_incidence_sums = cell_triangle_counts
        .iter()
        .map(|(&id, &triangles)| {
            triangles.checked_mul(3).map(|sum| (id, sum)).ok_or(
                GlobalIncidenceContractError::ArithmeticOverflow("cell incidence sum"),
            )
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let target_transition_charge = transition_charge(&fixed.vertex_degrees, &owners)?;
    let cell_ids = cells.keys().copied().collect::<Vec<_>>();
    let contract_key = GlobalIncidenceContractKey(format!(
        "{:016x}",
        fnv1a(
            format!(
                "{}|{:?}|{:?}|{}",
                fixed.context_key.0, vertex_domains, cell_triangle_counts, target_transition_charge
            )
            .bytes()
        )
    ));
    Ok(GlobalIncidenceContract {
        cell_ids,
        fixed,
        vertex_domains,
        cell_triangle_counts,
        cell_incidence_sums,
        target_transition_charge,
        contract_key,
    })
}

fn annular_cells(
    domain: &StratifiedTransitionDomainV3,
) -> Result<BTreeMap<u64, &AnnularCellDomain>, GlobalIncidenceContractError> {
    domain
        .cells
        .iter()
        .map(|cell| match cell {
            TransitionCellDomain::Annulus(cell) => Ok((cell.cell_id, cell)),
            TransitionCellDomain::Disk(cell) => Err(GlobalIncidenceContractError::NonAnnularCell {
                cell_id: cell.cell_id,
            }),
        })
        .collect()
}

fn cell_owners<'a>(
    cells: impl Iterator<Item = &'a AnnularCellDomain>,
) -> BTreeMap<usize, BTreeSet<u64>> {
    let mut owners = BTreeMap::<usize, BTreeSet<u64>>::new();
    for cell in cells {
        for &slot in cell.lower_cycle.iter().chain(&cell.upper_cycle) {
            owners.entry(slot).or_default().insert(cell.cell_id);
        }
    }
    owners
}

fn validate_fixed_only_vertices(
    source: &MotherGrid,
    fixed: &FixedFinalTopologyContext,
    owners: &BTreeMap<usize, BTreeSet<u64>>,
) -> Result<(), GlobalIncidenceContractError> {
    for (&source_slot, &actual) in &fixed.vertex_degrees {
        if owners.contains_key(&source_slot) {
            continue;
        }
        let anchor = anchor_kind(source, source_slot);
        let valid = match anchor {
            RingAnchorKind::Ordinary => (5..=7).contains(&actual),
            RingAnchorKind::IcosahedronPentagon { .. } => actual == 5,
        };
        if !valid {
            return Err(GlobalIncidenceContractError::InvalidFixedOnlyDegree {
                source_slot,
                actual,
                expected: match anchor {
                    RingAnchorKind::Ordinary => "5..=7".into(),
                    RingAnchorKind::IcosahedronPentagon { .. } => "5".into(),
                },
            });
        }
    }
    Ok(())
}

fn validate_adapter_contract(
    domain: &StratifiedTransitionDomainV3,
    source_slot: usize,
    anchor_kind: RingAnchorKind,
    exact_fixed_link: LinkPathSignature,
    contract: Option<&super::VertexLinkContract>,
) -> Result<(), GlobalIncidenceContractError> {
    let expected_degrees = legal_degrees(anchor_kind);
    match contract {
        Some(contract) => {
            if contract.anchor_kind != anchor_kind
                || expected_degrees
                    != (contract.target_degree_min..=contract.target_degree_max).collect()
            {
                return Err(GlobalIncidenceContractError::AdapterMismatch {
                    source_slot,
                    reason: "anchor or legal-degree domain differs".into(),
                });
            }
            let contracted = contracted_fixed_link_signature(domain, source_slot, contract)
                .ok_or(GlobalIncidenceContractError::InvalidFixedLink { source_slot })?;
            if !same_exact_endpoints(exact_fixed_link, contracted) {
                return Err(GlobalIncidenceContractError::AdapterMismatch {
                    source_slot,
                    reason: format!(
                        "fixed-link endpoints differ: exact={exact_fixed_link:?} contract={contracted:?}"
                    ),
                });
            }
        }
        None if exact_fixed_link != LinkPathSignature::Empty => {
            return Err(GlobalIncidenceContractError::AdapterMismatch {
                source_slot,
                reason: "fixed link has no VertexLinkContract".into(),
            });
        }
        None => {}
    }
    Ok(())
}

fn validate_path_providers(
    source_slot: usize,
    fixed_link: LinkPathSignature,
    owners: &BTreeSet<u64>,
    cells: &BTreeMap<u64, &AnnularCellDomain>,
) -> Result<(), GlobalIncidenceContractError> {
    let provider_count = owners.len() + usize::from(fixed_link != LinkPathSignature::Empty);
    if provider_count != 2 {
        return Err(GlobalIncidenceContractError::InvalidPathProviderCount {
            source_slot,
            actual: provider_count,
        });
    }
    let mut endpoints = owners
        .iter()
        .map(|id| cycle_neighbours(cells[id], source_slot))
        .collect::<Result<Vec<_>, _>>()?;
    if let LinkPathSignature::OnePath {
        endpoints: fixed, ..
    } = fixed_link
    {
        endpoints.push(fixed);
    }
    if endpoints.windows(2).any(|pair| pair[0] != pair[1]) {
        return Err(GlobalIncidenceContractError::IncompatiblePathEndpoints { source_slot });
    }
    Ok(())
}

fn cycle_neighbours(
    cell: &AnnularCellDomain,
    source_slot: usize,
) -> Result<Edge, GlobalIncidenceContractError> {
    let cycles = [&cell.lower_cycle, &cell.upper_cycle];
    let Some(cycle) = cycles
        .into_iter()
        .find(|cycle| cycle.contains(&source_slot))
    else {
        return Err(GlobalIncidenceContractError::InvalidCellBoundary {
            cell_id: cell.cell_id,
            source_slot,
        });
    };
    let positions = cycle
        .iter()
        .enumerate()
        .filter_map(|(index, &slot)| (slot == source_slot).then_some(index))
        .collect::<Vec<_>>();
    if cycle.len() < 3 || positions.len() != 1 {
        return Err(GlobalIncidenceContractError::InvalidCellBoundary {
            cell_id: cell.cell_id,
            source_slot,
        });
    }
    let index = positions[0];
    Ok(edge(
        cycle[(index + cycle.len() - 1) % cycle.len()],
        cycle[(index + 1) % cycle.len()],
    ))
}

fn owner_tuples(
    fixed_degree: u8,
    owners: &BTreeSet<u64>,
    legal_degrees: &BTreeSet<u8>,
) -> Vec<VertexOwnerIncidenceTuple> {
    let owner_ids = owners.iter().copied().collect::<Vec<_>>();
    let mut out = Vec::new();
    for &final_degree in legal_degrees {
        let Some(remaining) = final_degree.checked_sub(fixed_degree) else {
            continue;
        };
        positive_compositions(remaining, owner_ids.len(), &mut |counts| {
            out.push(VertexOwnerIncidenceTuple {
                final_degree,
                owner_counts: owner_ids.iter().copied().zip(counts).collect(),
            });
        });
    }
    out
}

fn positive_compositions(total: u8, parts: usize, emit: &mut impl FnMut(Vec<u8>)) {
    fn visit(
        total: u8,
        remaining_parts: usize,
        prefix: &mut Vec<u8>,
        emit: &mut impl FnMut(Vec<u8>),
    ) {
        if remaining_parts == 1 {
            if total > 0 {
                prefix.push(total);
                emit(prefix.clone());
                prefix.pop();
            }
            return;
        }
        for value in 1..total {
            prefix.push(value);
            visit(total - value, remaining_parts - 1, prefix, emit);
            prefix.pop();
        }
    }
    if parts > 0 && total >= parts as u8 {
        visit(total, parts, &mut Vec::with_capacity(parts), emit);
    }
}

fn transition_charge(
    fixed_degrees: &BTreeMap<usize, u8>,
    owners: &BTreeMap<usize, BTreeSet<u64>>,
) -> Result<i16, GlobalIncidenceContractError> {
    fixed_degrees
        .iter()
        .filter(|(slot, _)| !owners.contains_key(slot))
        .try_fold(12i16, |charge, (_, &degree)| {
            charge.checked_sub(6 - i16::from(degree)).ok_or(
                GlobalIncidenceContractError::ArithmeticOverflow("transition charge"),
            )
        })
}

fn legal_degrees(anchor_kind: RingAnchorKind) -> BTreeSet<u8> {
    match anchor_kind {
        RingAnchorKind::Ordinary => BTreeSet::from([5, 6, 7]),
        RingAnchorKind::IcosahedronPentagon { .. } => BTreeSet::from([5]),
    }
}

fn anchor_kind(source: &MotherGrid, source_slot: usize) -> RingAnchorKind {
    match source.addresses.get(source_slot).and_then(Option::as_ref) {
        Some(VertexAddress::IcosahedronVertex(base_vertex)) => {
            RingAnchorKind::IcosahedronPentagon {
                base_vertex: *base_vertex,
            }
        }
        _ => RingAnchorKind::Ordinary,
    }
}

fn same_exact_endpoints(a: LinkPathSignature, b: LinkPathSignature) -> bool {
    match (a, b) {
        (LinkPathSignature::Empty, LinkPathSignature::Empty) => true,
        (
            LinkPathSignature::OnePath { endpoints: a, .. },
            LinkPathSignature::OnePath { endpoints: b, .. },
        ) => a == b,
        _ => false,
    }
}

fn edge(a: usize, b: usize) -> Edge {
    (a.min(b), a.max(b))
}

fn fnv1a(bytes: impl Iterator<Item = u8>) -> u64 {
    bytes.fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclusive_boundary_domains_are_exact() {
        let tuples = owner_tuples(3, &BTreeSet::from([7]), &BTreeSet::from([5, 6, 7]));
        assert_eq!(
            tuples
                .iter()
                .map(|tuple| (tuple.final_degree, tuple.owner_counts[0].1))
                .collect::<Vec<_>>(),
            [(5, 2), (6, 3), (7, 4)]
        );
    }

    #[test]
    fn shared_interface_tuples_sum_to_legal_degree() {
        let tuples = owner_tuples(0, &BTreeSet::from([1, 2]), &BTreeSet::from([5, 6, 7]));
        assert!(tuples.iter().all(|tuple| {
            tuple.owner_counts.iter().all(|(_, count)| *count > 0)
                && tuple
                    .owner_counts
                    .iter()
                    .map(|(_, count)| *count)
                    .sum::<u8>()
                    == tuple.final_degree
        }));
    }

    #[test]
    fn anchor_final_degree_is_five() {
        let tuples = owner_tuples(
            2,
            &BTreeSet::from([1]),
            &legal_degrees(RingAnchorKind::IcosahedronPentagon { base_vertex: 0 }),
        );
        assert_eq!(tuples.len(), 1);
        assert_eq!(tuples[0].final_degree, 5);
        assert_eq!(tuples[0].owner_counts, [(1, 3)]);
    }

    #[test]
    fn cell_incidence_sum_is_three_times_triangle_count() {
        let triangle_count = 8usize;
        assert_eq!(triangle_count * 3, 24);
    }

    #[test]
    fn global_transition_charge_is_exact() {
        let fixed = BTreeMap::from([(0, 5), (1, 6), (2, 7), (3, 4)]);
        let owners = BTreeMap::from([(3, BTreeSet::from([1]))]);
        assert_eq!(transition_charge(&fixed, &owners), Ok(12));
    }
}
