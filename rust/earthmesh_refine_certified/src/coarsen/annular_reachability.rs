//! Degree and boundary-link reachability for V3 annular transition cells.
//!
//! The cut-polygon DP stores incidence signatures, not concrete triangle
//! families.  It is a necessary relaxation of CSAE: glue-invalid states may
//! survive, but every concrete annular topology projects into this domain.

use super::{
    AnnularCellDomain, StratifiedTransitionDomainV3, TopologyBoundary, TransitionCellDomain,
    VertexLinkContract,
};
use std::collections::{BTreeMap, BTreeSet};

type Edge = (usize, usize);
type SignatureCounts = BTreeMap<Vec<u8>, usize>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LinkPathSignature {
    Empty,
    OnePath { endpoints: Edge, edge_count: u8 },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AnnularTopologySignature {
    pub vertex_incidences: Vec<(usize, u8)>,
    pub boundary_link_contributions: Vec<(usize, LinkPathSignature)>,
    pub root_bridge: Edge,
    pub member_topology_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnnularReachabilityStorageAudit {
    pub stores_incidence_signatures: bool,
    pub stores_link_path_signatures: bool,
    pub stores_member_counts: bool,
    pub stores_concrete_witnesses: bool,
    pub stores_backpointers: bool,
    pub necessary_relaxation_only: bool,
}

pub fn annular_reachability_storage_audit() -> AnnularReachabilityStorageAudit {
    AnnularReachabilityStorageAudit {
        stores_incidence_signatures: true,
        stores_link_path_signatures: true,
        stores_member_counts: true,
        stores_concrete_witnesses: false,
        stores_backpointers: false,
        necessary_relaxation_only: true,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnnularReachabilityLimits {
    pub maximum_signature_states: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnularSignatureSearchStatus {
    ExhaustedNecessaryRelaxation,
    SearchIncomplete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnularCellSignatureDomain {
    pub cell_id: u64,
    pub signatures: Vec<AnnularTopologySignature>,
    pub root_bridges_considered: u64,
    pub states_examined: usize,
    pub degree_cap_prunes: usize,
    pub status: AnnularSignatureSearchStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnularReachabilityOutcome {
    NecessaryFeasible,
    ProvenImpossibleWithinDeclaredAnnularFamily,
    SearchIncomplete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnularReachabilityEvidence {
    pub cell_signature_counts_before_ac3: Vec<usize>,
    pub cell_signature_counts_after_ac3: Vec<usize>,
    pub cell_member_counts_before_ac3: Vec<usize>,
    pub fixed_contribution_caps: BTreeMap<(u64, usize), u8>,
    pub root_bridges_considered: u64,
    pub states_examined: usize,
    pub degree_cap_prunes: usize,
    pub ac3_prunes: usize,
    pub outcome: AnnularReachabilityOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnnularReachabilityError {
    UnsupportedDiskCell { cell_id: u64 },
    MissingCellDomain { cell_id: u64 },
    DuplicateCellDomain { cell_id: u64 },
    BoundaryTooShort { cell_id: u64 },
    BoundaryIntersection { cell_id: u64 },
    InvalidFixedLink { source_slot: usize },
    InvalidConcreteTopology(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DpStop {
    Budget,
    ArithmeticOverflow,
}

pub fn analyze_stratified_annular_degree_reachability(
    domain: &StratifiedTransitionDomainV3,
    limits: AnnularReachabilityLimits,
) -> Result<AnnularReachabilityEvidence, AnnularReachabilityError> {
    let ears = BTreeMap::new();
    let caps = fixed_contribution_caps(domain, &ears)?;
    let mut signatures = Vec::with_capacity(domain.cells.len());
    for cell in &domain.cells {
        let TransitionCellDomain::Annulus(cell) = cell else {
            let TransitionCellDomain::Disk(cell) = cell else {
                unreachable!()
            };
            return Err(AnnularReachabilityError::UnsupportedDiskCell {
                cell_id: cell.cell_id,
            });
        };
        signatures.push(enumerate_annular_degree_signatures(cell, &caps, limits)?);
    }
    analyze_annular_signature_domains(domain, signatures, &ears)
}

pub fn analyze_annular_signature_domains(
    domain: &StratifiedTransitionDomainV3,
    mut cells: Vec<AnnularCellSignatureDomain>,
    ear_delta_domains: &BTreeMap<usize, BTreeSet<i8>>,
) -> Result<AnnularReachabilityEvidence, AnnularReachabilityError> {
    let expected = domain.cells.iter().map(cell_id).collect::<BTreeSet<_>>();
    let actual = cells.iter().map(|cell| cell.cell_id).collect::<Vec<_>>();
    if actual.iter().copied().collect::<BTreeSet<_>>().len() != actual.len() {
        let duplicate = actual
            .iter()
            .copied()
            .find(|id| actual.iter().filter(|candidate| *candidate == id).count() > 1)
            .unwrap();
        return Err(AnnularReachabilityError::DuplicateCellDomain { cell_id: duplicate });
    }
    if let Some(&missing) = expected
        .iter()
        .find(|id| !actual.iter().any(|candidate| candidate == *id))
    {
        return Err(AnnularReachabilityError::MissingCellDomain { cell_id: missing });
    }
    cells.sort_by_key(|cell| cell.cell_id);
    let before = cells
        .iter()
        .map(|cell| cell.signatures.len())
        .collect::<Vec<_>>();
    let members = cells
        .iter()
        .map(|cell| {
            cell.signatures
                .iter()
                .map(|signature| signature.member_topology_count)
                .sum()
        })
        .collect();
    let incomplete = cells
        .iter()
        .any(|cell| cell.status == AnnularSignatureSearchStatus::SearchIncomplete);
    let mut ac3_prunes = 0;
    ac3(domain, ear_delta_domains, &mut cells, &mut ac3_prunes)?;
    let after = cells
        .iter()
        .map(|cell| cell.signatures.len())
        .collect::<Vec<_>>();
    let empty = cells.iter().any(|cell| cell.signatures.is_empty());
    let outcome = if incomplete {
        AnnularReachabilityOutcome::SearchIncomplete
    } else if empty {
        AnnularReachabilityOutcome::ProvenImpossibleWithinDeclaredAnnularFamily
    } else {
        AnnularReachabilityOutcome::NecessaryFeasible
    };
    Ok(AnnularReachabilityEvidence {
        cell_signature_counts_before_ac3: before,
        cell_signature_counts_after_ac3: after,
        cell_member_counts_before_ac3: members,
        fixed_contribution_caps: fixed_contribution_caps(domain, ear_delta_domains)?,
        root_bridges_considered: cells.iter().map(|cell| cell.root_bridges_considered).sum(),
        states_examined: cells.iter().map(|cell| cell.states_examined).sum(),
        degree_cap_prunes: cells.iter().map(|cell| cell.degree_cap_prunes).sum(),
        ac3_prunes,
        outcome,
    })
}

pub fn enumerate_annular_degree_signatures(
    cell: &AnnularCellDomain,
    caps: &BTreeMap<(u64, usize), u8>,
    limits: AnnularReachabilityLimits,
) -> Result<AnnularCellSignatureDomain, AnnularReachabilityError> {
    validate_cell(cell)?;
    let vertices = cell
        .lower_cycle
        .iter()
        .chain(&cell.upper_cycle)
        .copied()
        .collect::<Vec<_>>();
    let positions = vertices
        .iter()
        .enumerate()
        .map(|(index, &vertex)| (vertex, index))
        .collect::<BTreeMap<_, _>>();
    let local_caps = vertices
        .iter()
        .map(|vertex| *caps.get(&(cell.cell_id, *vertex)).unwrap_or(&7))
        .collect::<Vec<_>>();
    let lower = cell.lower_cycle.iter().copied().collect::<BTreeSet<_>>();
    let upper = cell.upper_cycle.iter().copied().collect::<BTreeSet<_>>();
    let mut signatures =
        BTreeMap::<(Vec<(usize, u8)>, Vec<(usize, LinkPathSignature)>, Edge), usize>::new();
    let mut states_examined = 0;
    let mut root_bridges_considered = 0;
    let mut degree_cap_prunes = 0;
    let mut status = AnnularSignatureSearchStatus::ExhaustedNecessaryRelaxation;

    'roots: for lower_root in 0..cell.lower_cycle.len() {
        for upper_root in 0..cell.upper_cycle.len() {
            root_bridges_considered += 1;
            let root_bridge = edge(cell.lower_cycle[lower_root], cell.upper_cycle[upper_root]);
            let occurrences = cut_occurrences(cell, lower_root, upper_root);
            let mut memo = BTreeMap::new();
            let context = DpContext {
                occurrences: &occurrences,
                positions: &positions,
                caps: &local_caps,
                lower: &lower,
                upper: &upper,
                root_bridge,
                forbidden: &cell.forbidden_global_edges,
                limit: limits.maximum_signature_states,
            };
            let result = polygon_signature_dp(
                0,
                occurrences.len() - 1,
                &context,
                &mut memo,
                &mut states_examined,
                &mut degree_cap_prunes,
            );
            let root_signatures = match result {
                Ok(signatures) => signatures,
                Err(DpStop::Budget | DpStop::ArithmeticOverflow) => {
                    status = AnnularSignatureSearchStatus::SearchIncomplete;
                    break 'roots;
                }
            };
            for (counts, members) in root_signatures {
                let incidences = vertices
                    .iter()
                    .copied()
                    .zip(counts.iter().copied())
                    .collect::<Vec<_>>();
                let links = boundary_links(cell, &counts, &positions);
                let key = (incidences, links, root_bridge);
                let Some(total) = signatures
                    .get(&key)
                    .copied()
                    .unwrap_or(0)
                    .checked_add(members)
                else {
                    status = AnnularSignatureSearchStatus::SearchIncomplete;
                    break 'roots;
                };
                signatures.insert(key, total);
            }
        }
    }
    Ok(AnnularCellSignatureDomain {
        cell_id: cell.cell_id,
        signatures: signatures
            .into_iter()
            .map(
                |(
                    (vertex_incidences, boundary_link_contributions, root_bridge),
                    member_topology_count,
                )| AnnularTopologySignature {
                    vertex_incidences,
                    boundary_link_contributions,
                    root_bridge,
                    member_topology_count,
                },
            )
            .collect(),
        root_bridges_considered,
        states_examined,
        degree_cap_prunes,
        status,
    })
}

pub fn annular_topology_signature(
    cell: &AnnularCellDomain,
    triangles: &[[usize; 3]],
) -> Result<AnnularTopologySignature, AnnularReachabilityError> {
    validate_cell(cell)?;
    let vertices = cell
        .lower_cycle
        .iter()
        .chain(&cell.upper_cycle)
        .copied()
        .collect::<BTreeSet<_>>();
    if triangles.len() != vertices.len()
        || triangles
            .iter()
            .any(|triangle| triangle.iter().any(|vertex| !vertices.contains(vertex)))
    {
        return Err(AnnularReachabilityError::InvalidConcreteTopology(
            "annular triangle count or vertex support is invalid".into(),
        ));
    }
    let mut incidences = BTreeMap::<usize, u8>::new();
    let mut links = BTreeMap::<usize, BTreeSet<Edge>>::new();
    let lower = cell.lower_cycle.iter().copied().collect::<BTreeSet<_>>();
    let upper = cell.upper_cycle.iter().copied().collect::<BTreeSet<_>>();
    let mut bridges = BTreeSet::new();
    for triangle in triangles {
        let mut triangle = *triangle;
        triangle.sort_unstable();
        if triangle[0] == triangle[1] || triangle[1] == triangle[2] {
            return Err(AnnularReachabilityError::InvalidConcreteTopology(
                "degenerate triangle".into(),
            ));
        }
        for corner in 0..3 {
            *incidences.entry(triangle[corner]).or_default() += 1;
            links
                .entry(triangle[corner])
                .or_default()
                .insert(edge(triangle[(corner + 1) % 3], triangle[(corner + 2) % 3]));
        }
        for candidate in triangle_edges(triangle) {
            if (lower.contains(&candidate.0) && upper.contains(&candidate.1))
                || (lower.contains(&candidate.1) && upper.contains(&candidate.0))
            {
                bridges.insert(candidate);
            }
        }
    }
    let root_bridge = bridges.first().copied().ok_or_else(|| {
        AnnularReachabilityError::InvalidConcreteTopology("annulus has no bridge".into())
    })?;
    let vertex_incidences = vertices
        .iter()
        .map(|&vertex| {
            incidences
                .get(&vertex)
                .copied()
                .map(|count| (vertex, count))
                .ok_or_else(|| {
                    AnnularReachabilityError::InvalidConcreteTopology(format!(
                        "vertex {vertex} has no incident triangle"
                    ))
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let boundary_link_contributions = vertices
        .iter()
        .map(|&vertex| {
            let path = path_signature(links.get(&vertex).unwrap()).ok_or_else(|| {
                AnnularReachabilityError::InvalidConcreteTopology(format!(
                    "vertex {vertex} link is not one path"
                ))
            })?;
            Ok((vertex, path))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AnnularTopologySignature {
        vertex_incidences,
        boundary_link_contributions,
        root_bridge,
        member_topology_count: 1,
    })
}

fn fixed_contribution_caps(
    domain: &StratifiedTransitionDomainV3,
    ears: &BTreeMap<usize, BTreeSet<i8>>,
) -> Result<BTreeMap<(u64, usize), u8>, AnnularReachabilityError> {
    let owners = cell_owners(domain);
    let mut out = BTreeMap::new();
    for cell in &domain.cells {
        let TransitionCellDomain::Annulus(cell) = cell else {
            continue;
        };
        for vertex in cell.lower_cycle.iter().chain(&cell.upper_cycle) {
            let contract = domain.link_contracts.get(vertex);
            validate_fixed_contract(domain, *vertex, contract)?;
            let fixed = contract.map_or(0, |contract| contract.fixed_link_edges.len() as i16);
            let legal_max = i16::from(contract.map_or(7, |contract| contract.target_degree_max));
            let other_min = owners
                .get(vertex)
                .map_or(0, |ids| ids.len().saturating_sub(1)) as i16;
            let ear_min = ears
                .get(vertex)
                .and_then(|values| values.first())
                .copied()
                .unwrap_or(0) as i16;
            let cap = (legal_max - fixed - other_min - ear_min).clamp(0, u8::MAX as i16) as u8;
            out.insert((cell.cell_id, *vertex), cap);
        }
    }
    Ok(out)
}

fn validate_fixed_contract(
    domain: &StratifiedTransitionDomainV3,
    vertex: usize,
    contract: Option<&VertexLinkContract>,
) -> Result<(), AnnularReachabilityError> {
    let Some(contract) = contract else {
        return Ok(());
    };
    if contracted_fixed_link_signature(domain, vertex, contract).is_none() {
        return Err(AnnularReachabilityError::InvalidFixedLink {
            source_slot: contract.source_slot,
        });
    }
    Ok(())
}

fn ac3(
    domain: &StratifiedTransitionDomainV3,
    ears: &BTreeMap<usize, BTreeSet<i8>>,
    cells: &mut [AnnularCellSignatureDomain],
    prunes: &mut usize,
) -> Result<(), AnnularReachabilityError> {
    loop {
        let marginals = marginal_domains(cells);
        let mut changed = false;
        for (index, cell) in cells.iter_mut().enumerate() {
            let keep = cell
                .signatures
                .iter()
                .filter(|signature| signature_supported(domain, ears, &marginals, index, signature))
                .cloned()
                .collect::<Vec<_>>();
            *prunes += cell.signatures.len() - keep.len();
            changed |= keep.len() != cell.signatures.len();
            cell.signatures = keep;
        }
        if !changed {
            return Ok(());
        }
    }
}

type Marginal = BTreeMap<usize, BTreeSet<(u8, LinkPathSignature)>>;

fn marginal_domains(cells: &[AnnularCellSignatureDomain]) -> Vec<Marginal> {
    cells
        .iter()
        .map(|cell| {
            let mut out = Marginal::new();
            for signature in &cell.signatures {
                let links = signature
                    .boundary_link_contributions
                    .iter()
                    .copied()
                    .collect::<BTreeMap<_, _>>();
                for &(vertex, incidence) in &signature.vertex_incidences {
                    out.entry(vertex)
                        .or_default()
                        .insert((incidence, links[&vertex]));
                }
            }
            out
        })
        .collect()
}

fn signature_supported(
    domain: &StratifiedTransitionDomainV3,
    ears: &BTreeMap<usize, BTreeSet<i8>>,
    marginals: &[Marginal],
    cell_index: usize,
    signature: &AnnularTopologySignature,
) -> bool {
    let links = signature
        .boundary_link_contributions
        .iter()
        .copied()
        .collect::<BTreeMap<_, _>>();
    signature.vertex_incidences.iter().all(|&(vertex, own)| {
        let own_link = links[&vertex];
        let contract = domain.link_contracts.get(&vertex);
        let fixed = contract.map_or(0, |contract| contract.fixed_link_edges.len() as u8);
        let fixed_link = contract
            .and_then(|contract| contracted_fixed_link_signature(domain, vertex, contract))
            .unwrap_or(LinkPathSignature::Empty);
        let mut degrees = BTreeSet::from([fixed.saturating_add(own)]);
        let mut path_providers = usize::from(fixed_link != LinkPathSignature::Empty) + 1;
        if !same_endpoints(own_link, fixed_link) {
            return false;
        }
        for (other_index, marginal) in marginals.iter().enumerate() {
            if other_index == cell_index {
                continue;
            }
            let Some(values) = marginal.get(&vertex) else {
                continue;
            };
            let compatible = values
                .iter()
                .filter(|(_, link)| same_endpoints(own_link, *link))
                .map(|(count, _)| *count)
                .collect::<BTreeSet<_>>();
            if compatible.is_empty() {
                return false;
            }
            degrees = minkowski(&degrees, &compatible);
            path_providers += 1;
        }
        let ear_domain = ears
            .get(&vertex)
            .cloned()
            .unwrap_or_else(|| BTreeSet::from([0]));
        let degrees = apply_ears(&degrees, &ear_domain);
        let (legal_min, legal_max) = contract.map_or((5, 7), |contract| {
            (contract.target_degree_min, contract.target_degree_max)
        });
        let degree_supported = degrees
            .iter()
            .any(|degree| (legal_min..=legal_max).contains(degree));
        let exact_no_ears = ear_domain == BTreeSet::from([0]);
        degree_supported && (!exact_no_ears || path_providers == 2)
    })
}

fn polygon_signature_dp(
    lo: usize,
    hi: usize,
    context: &DpContext<'_>,
    memo: &mut BTreeMap<(usize, usize), SignatureCounts>,
    states: &mut usize,
    cap_prunes: &mut usize,
) -> Result<SignatureCounts, DpStop> {
    if hi <= lo + 1 {
        return Ok(BTreeMap::from([(vec![0; context.caps.len()], 1)]));
    }
    if let Some(cached) = memo.get(&(lo, hi)) {
        return Ok(cached.clone());
    }
    let mut out = SignatureCounts::new();
    for mid in lo + 1..hi {
        let triangle = [
            context.occurrences[lo],
            context.occurrences[mid],
            context.occurrences[hi],
        ];
        if !triangle_allowed(triangle, context) {
            continue;
        }
        let left = polygon_signature_dp(lo, mid, context, memo, states, cap_prunes)?;
        let right = polygon_signature_dp(mid, hi, context, memo, states, cap_prunes)?;
        for (a, a_members) in &left {
            for (b, b_members) in &right {
                let mut counts = a.clone();
                let mut valid = true;
                for (target, value) in counts.iter_mut().zip(b) {
                    let Some(sum) = target.checked_add(*value) else {
                        return Err(DpStop::ArithmeticOverflow);
                    };
                    *target = sum;
                }
                for vertex in triangle {
                    let position = context.positions[&vertex];
                    let Some(sum) = counts[position].checked_add(1) else {
                        return Err(DpStop::ArithmeticOverflow);
                    };
                    counts[position] = sum;
                }
                for (count, cap) in counts.iter().zip(context.caps) {
                    if count > cap {
                        *cap_prunes += 1;
                        valid = false;
                        break;
                    }
                }
                if !valid {
                    continue;
                }
                let Some(members) = a_members.checked_mul(*b_members) else {
                    return Err(DpStop::ArithmeticOverflow);
                };
                if let Some(existing) = out.get_mut(&counts) {
                    let Some(total) = existing.checked_add(members) else {
                        return Err(DpStop::ArithmeticOverflow);
                    };
                    *existing = total;
                } else {
                    if *states >= context.limit {
                        return Err(DpStop::Budget);
                    }
                    *states += 1;
                    out.insert(counts, members);
                }
            }
        }
    }
    memo.insert((lo, hi), out.clone());
    Ok(out)
}

struct DpContext<'a> {
    occurrences: &'a [usize],
    positions: &'a BTreeMap<usize, usize>,
    caps: &'a [u8],
    lower: &'a BTreeSet<usize>,
    upper: &'a BTreeSet<usize>,
    root_bridge: Edge,
    forbidden: &'a BTreeSet<Edge>,
    limit: usize,
}

fn triangle_allowed(triangle: [usize; 3], context: &DpContext<'_>) -> bool {
    if triangle[0] == triangle[1] || triangle[0] == triangle[2] || triangle[1] == triangle[2] {
        return false;
    }
    triangle_edges(triangle).into_iter().all(|candidate| {
        if context.forbidden.contains(&candidate) {
            return false;
        }
        let bridge = (context.lower.contains(&candidate.0) && context.upper.contains(&candidate.1))
            || (context.lower.contains(&candidate.1) && context.upper.contains(&candidate.0));
        !bridge || candidate >= context.root_bridge
    })
}

pub(super) fn path_signature(edges: &BTreeSet<Edge>) -> Option<LinkPathSignature> {
    if edges.is_empty() {
        return Some(LinkPathSignature::Empty);
    }
    let mut adjacency = BTreeMap::<usize, BTreeSet<usize>>::new();
    for &(a, b) in edges {
        adjacency.entry(a).or_default().insert(b);
        adjacency.entry(b).or_default().insert(a);
    }
    if adjacency.values().any(|neighbours| neighbours.len() > 2) {
        return None;
    }
    let endpoints = adjacency
        .iter()
        .filter_map(|(&vertex, neighbours)| (neighbours.len() == 1).then_some(vertex))
        .collect::<Vec<_>>();
    if endpoints.len() != 2 {
        return None;
    }
    let mut reached = BTreeSet::from([endpoints[0]]);
    let mut stack = vec![endpoints[0]];
    while let Some(vertex) = stack.pop() {
        for &next in &adjacency[&vertex] {
            if reached.insert(next) {
                stack.push(next);
            }
        }
    }
    if reached.len() != adjacency.len() {
        return None;
    }
    Some(LinkPathSignature::OnePath {
        endpoints: edge(endpoints[0], endpoints[1]),
        edge_count: u8::try_from(edges.len()).ok()?,
    })
}

pub(super) fn contracted_fixed_link_signature(
    domain: &StratifiedTransitionDomainV3,
    vertex: usize,
    contract: &VertexLinkContract,
) -> Option<LinkPathSignature> {
    let LinkPathSignature::OnePath {
        endpoints,
        edge_count,
    } = path_signature(&contract.fixed_link_edges)?
    else {
        return Some(LinkPathSignature::Empty);
    };
    let contract_endpoint = |endpoint| {
        domain
            .bands
            .iter()
            .find_map(|band| {
                let TopologyBoundary::ContractedCoarseCycle {
                    source_expansion, ..
                } = &band.lower_boundary
                else {
                    return None;
                };
                source_expansion.coarse_edges.iter().find_map(|coarse| {
                    let first = *coarse.source_path.first()?;
                    let last = *coarse.source_path.last()?;
                    if first == vertex && coarse.source_path.get(1).copied() == Some(endpoint) {
                        Some(last)
                    } else if last == vertex
                        && coarse
                            .source_path
                            .get(coarse.source_path.len().checked_sub(2)?)
                            .copied()
                            == Some(endpoint)
                    {
                        Some(first)
                    } else {
                        None
                    }
                })
            })
            .unwrap_or(endpoint)
    };
    let endpoints = edge(
        contract_endpoint(endpoints.0),
        contract_endpoint(endpoints.1),
    );
    (endpoints.0 != endpoints.1).then_some(LinkPathSignature::OnePath {
        endpoints,
        edge_count,
    })
}

fn same_endpoints(a: LinkPathSignature, b: LinkPathSignature) -> bool {
    match (a, b) {
        (_, LinkPathSignature::Empty) | (LinkPathSignature::Empty, _) => true,
        (
            LinkPathSignature::OnePath { endpoints: a, .. },
            LinkPathSignature::OnePath { endpoints: b, .. },
        ) => a == b,
    }
}

fn boundary_links(
    cell: &AnnularCellDomain,
    counts: &[u8],
    positions: &BTreeMap<usize, usize>,
) -> Vec<(usize, LinkPathSignature)> {
    cell.lower_cycle
        .iter()
        .chain(&cell.upper_cycle)
        .map(|&vertex| {
            let cycle = if cell.lower_cycle.contains(&vertex) {
                &cell.lower_cycle
            } else {
                &cell.upper_cycle
            };
            let index = cycle
                .iter()
                .position(|candidate| *candidate == vertex)
                .unwrap();
            let endpoints = edge(
                cycle[(index + cycle.len() - 1) % cycle.len()],
                cycle[(index + 1) % cycle.len()],
            );
            (
                vertex,
                LinkPathSignature::OnePath {
                    endpoints,
                    edge_count: counts[positions[&vertex]],
                },
            )
        })
        .collect()
}

fn cut_occurrences(cell: &AnnularCellDomain, lower_root: usize, upper_root: usize) -> Vec<usize> {
    let lower_slot = cell.lower_cycle[lower_root];
    let upper_slot = cell.upper_cycle[upper_root];
    let mut occurrences = vec![lower_slot];
    occurrences.extend(
        (1..cell.lower_cycle.len())
            .map(|offset| cell.lower_cycle[(lower_root + offset) % cell.lower_cycle.len()]),
    );
    occurrences.push(lower_slot);
    occurrences.push(upper_slot);
    occurrences.extend((1..cell.upper_cycle.len()).map(|offset| {
        cell.upper_cycle[(upper_root + cell.upper_cycle.len() - offset) % cell.upper_cycle.len()]
    }));
    occurrences.push(upper_slot);
    occurrences
}

fn validate_cell(cell: &AnnularCellDomain) -> Result<(), AnnularReachabilityError> {
    if cell.lower_cycle.len() < 3 || cell.upper_cycle.len() < 3 {
        return Err(AnnularReachabilityError::BoundaryTooShort {
            cell_id: cell.cell_id,
        });
    }
    let lower = cell.lower_cycle.iter().copied().collect::<BTreeSet<_>>();
    let upper = cell.upper_cycle.iter().copied().collect::<BTreeSet<_>>();
    if lower.len() != cell.lower_cycle.len()
        || upper.len() != cell.upper_cycle.len()
        || !lower.is_disjoint(&upper)
    {
        return Err(AnnularReachabilityError::BoundaryIntersection {
            cell_id: cell.cell_id,
        });
    }
    Ok(())
}

fn cell_owners(domain: &StratifiedTransitionDomainV3) -> BTreeMap<usize, BTreeSet<u64>> {
    let mut owners = BTreeMap::<usize, BTreeSet<u64>>::new();
    for cell in &domain.cells {
        let TransitionCellDomain::Annulus(cell) = cell else {
            continue;
        };
        for &vertex in cell.lower_cycle.iter().chain(&cell.upper_cycle) {
            owners.entry(vertex).or_default().insert(cell.cell_id);
        }
    }
    owners
}

fn cell_id(cell: &TransitionCellDomain) -> u64 {
    match cell {
        TransitionCellDomain::Disk(cell) => cell.cell_id,
        TransitionCellDomain::Annulus(cell) => cell.cell_id,
    }
}

fn minkowski(a: &BTreeSet<u8>, b: &BTreeSet<u8>) -> BTreeSet<u8> {
    a.iter()
        .flat_map(|&left| b.iter().filter_map(move |&right| left.checked_add(right)))
        .collect()
}

fn apply_ears(base: &BTreeSet<u8>, ears: &BTreeSet<i8>) -> BTreeSet<u8> {
    base.iter()
        .flat_map(|&degree| {
            ears.iter().filter_map(move |&delta| {
                let degree = i16::from(degree) + i16::from(delta);
                (0..=u8::MAX as i16)
                    .contains(&degree)
                    .then_some(degree as u8)
            })
        })
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
