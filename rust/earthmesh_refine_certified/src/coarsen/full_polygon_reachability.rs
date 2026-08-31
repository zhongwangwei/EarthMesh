//! Degree reachability for abstract full-polygon CAT sectors.
//!
//! PR39 deliberately stops before materialising concrete triangulations: each
//! sector is reduced to exact incidence signatures plus member counts.

use super::global_exact_merge::{fixed_triangles, mesh_edges, MAX_EARS_PER_ANCHOR};
use super::{
    build_stratified_annulus, GlobalExactMergeEvidence, HierarchyComponent, RingAnchorKind,
    StratifiedAnnulus, TraceRole,
};
use crate::MotherGrid;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DegreeDomainOutcome {
    /// Exact sector incidence signatures retain legal degree support under the
    /// recorded ear domain. This is a gate to enumerate the family, not a
    /// claim that a compatible global topology exists.
    NecessaryFeasible,
    ProvenImpossibleWithinPerSectorFullPolygon,
    UnknownDueToUnsupportedSector,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DegreeDefectRecord {
    pub source_slot: usize,
    pub fixed_degree: u8,
    /// Empty for a domain-level contradiction. Concrete PR38 records produced
    /// by `degree_defects_from_global_evidence` contain the selected values.
    pub selected_sector_contributions: Vec<(u64, u8)>,
    pub final_degree: u8,
    pub legal_min: u8,
    pub legal_max: u8,
    pub deficit: u8,
    pub excess: u8,
    pub owner_sector_ids: Vec<u64>,
    pub trace_roles: Vec<TraceRole>,
    pub is_anchor: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VertexIncidenceDomain {
    pub source_slot: usize,
    pub possible_counts: BTreeSet<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SectorDegreeSignature {
    pub contributions: Vec<(usize, u8)>,
    pub member_topology_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FullPolygonReachabilityEvidence {
    pub defect_vertices: Vec<DegreeDefectRecord>,
    pub sector_polygon_sizes: Vec<usize>,
    pub sector_topology_counts: Vec<usize>,
    pub sector_signature_counts: Vec<usize>,
    pub sector_signatures: Vec<Vec<SectorDegreeSignature>>,
    pub incidence_domains: BTreeMap<(u64, usize), BTreeSet<u8>>,
    pub global_degree_domains: BTreeMap<usize, BTreeSet<u8>>,
    pub ear_delta_domains: BTreeMap<usize, BTreeSet<i8>>,
    pub ear_delta_domains_exact: bool,
    pub signatures_before_ac3: usize,
    pub signatures_after_ac3: usize,
    pub outcome: DegreeDomainOutcome,
}

pub fn analyze_full_polygon_degree_reachability(
    source: &MotherGrid,
    component: &HierarchyComponent,
) -> Result<FullPolygonReachabilityEvidence, String> {
    let stratified = build_stratified_annulus(source, component)
        .map_err(|error| format!("stratified annulus rejected component: {error:?}"))?;
    analyze_stratified_full_polygon_degree_reachability(source, component, &stratified)
}

pub fn analyze_stratified_full_polygon_degree_reachability(
    source: &MotherGrid,
    component: &HierarchyComponent,
    stratified: &StratifiedAnnulus,
) -> Result<FullPolygonReachabilityEvidence, String> {
    let fixed = fixed_triangles(source, component)?;
    let fixed_degrees = triangle_incidence_counts(&fixed);
    let fixed_edges = mesh_edges(&fixed);
    let sectors = effective_sector_polygons(stratified)?;
    let mut sector_signatures = Vec::new();
    let mut sector_polygon_sizes = Vec::new();
    let mut incidence_domains = BTreeMap::<(u64, usize), BTreeSet<u8>>::new();
    let mut unsupported = false;

    for sector in sectors {
        sector_polygon_sizes.push(sector.vertices.len());
        if !is_simple_abstract_polygon(&sector.vertices) {
            unsupported = true;
            sector_signatures.push(Vec::new());
            continue;
        }
        let boundary_edges = polygon_boundary_edges(&sector.vertices);
        let forbidden_edges = fixed_edges
            .iter()
            .copied()
            .filter(|edge| !boundary_edges.contains(edge))
            .collect::<BTreeSet<_>>();
        let signatures = incidence_signatures(&sector.vertices, &forbidden_edges)?;
        for signature in &signatures {
            for &(vertex, count) in &signature.contributions {
                incidence_domains
                    .entry((sector.id, vertex))
                    .or_default()
                    .insert(count);
            }
        }
        sector_signatures.push(signatures);
    }

    let signatures_before_ac3 = sector_signatures.iter().map(Vec::len).sum();
    let sector_topology_counts = sector_signatures
        .iter()
        .map(|signatures| signatures.iter().map(|s| s.member_topology_count).sum())
        .collect::<Vec<_>>();
    let (ear_domains, ear_domains_exact) =
        conservative_ear_delta_domains(stratified, &sector_signatures)?;
    if unsupported || sector_signatures.iter().any(Vec::is_empty) {
        return Ok(FullPolygonReachabilityEvidence {
            defect_vertices: Vec::new(),
            sector_polygon_sizes,
            sector_topology_counts,
            sector_signature_counts: sector_signatures.iter().map(Vec::len).collect(),
            sector_signatures,
            incidence_domains,
            global_degree_domains: BTreeMap::new(),
            ear_delta_domains: ear_domains,
            ear_delta_domains_exact: ear_domains_exact,
            signatures_before_ac3,
            signatures_after_ac3: signatures_before_ac3,
            outcome: DegreeDomainOutcome::UnknownDueToUnsupportedSector,
        });
    }

    let mut global_degree_domains =
        global_domains(&fixed_degrees, &sector_signatures, &ear_domains);
    let first_defects = defects(
        stratified,
        &fixed_degrees,
        &global_degree_domains,
        &sector_signatures,
    );
    if ear_domains_exact && !first_defects.is_empty() {
        return Ok(FullPolygonReachabilityEvidence {
            defect_vertices: first_defects,
            sector_polygon_sizes,
            sector_topology_counts,
            sector_signature_counts: sector_signatures.iter().map(Vec::len).collect(),
            sector_signatures,
            incidence_domains,
            global_degree_domains,
            ear_delta_domains: ear_domains,
            ear_delta_domains_exact: ear_domains_exact,
            signatures_before_ac3,
            signatures_after_ac3: signatures_before_ac3,
            outcome: DegreeDomainOutcome::ProvenImpossibleWithinPerSectorFullPolygon,
        });
    }

    ac3(
        stratified,
        &fixed_degrees,
        &ear_domains,
        &mut sector_signatures,
    );
    let signatures_after_ac3 = sector_signatures.iter().map(Vec::len).sum();
    global_degree_domains = global_domains(&fixed_degrees, &sector_signatures, &ear_domains);
    let defect_vertices = defects(
        stratified,
        &fixed_degrees,
        &global_degree_domains,
        &sector_signatures,
    );
    let has_contradiction =
        sector_signatures.iter().any(Vec::is_empty) || !defect_vertices.is_empty();
    let outcome = if has_contradiction && ear_domains_exact {
        DegreeDomainOutcome::ProvenImpossibleWithinPerSectorFullPolygon
    } else if has_contradiction {
        DegreeDomainOutcome::UnknownDueToUnsupportedSector
    } else {
        DegreeDomainOutcome::NecessaryFeasible
    };

    Ok(FullPolygonReachabilityEvidence {
        defect_vertices,
        sector_polygon_sizes,
        sector_topology_counts,
        sector_signature_counts: sector_signatures.iter().map(Vec::len).collect(),
        sector_signatures,
        incidence_domains,
        global_degree_domains,
        ear_delta_domains: ear_domains,
        ear_delta_domains_exact: ear_domains_exact,
        signatures_before_ac3,
        signatures_after_ac3,
        outcome,
    })
}

pub fn degree_defects_from_global_evidence(
    stratified: &StratifiedAnnulus,
    evidence: &GlobalExactMergeEvidence,
) -> Result<Vec<DegreeDefectRecord>, String> {
    let mut out = Vec::new();
    for (&vertex, &degree) in &evidence.vertex_degrees {
        let degree = u8::try_from(degree)
            .map_err(|_| format!("vertex {vertex} degree {degree} exceeds u8"))?;
        let legal = legal_values(stratified, vertex);
        if legal.contains(&degree) {
            continue;
        }
        let legal_min = *legal.first().unwrap_or(&5);
        let legal_max = *legal.last().unwrap_or(&7);
        let selected_sector_contributions = evidence
            .vertex_sector_contributions
            .get(&vertex)
            .into_iter()
            .flat_map(|items| items.iter())
            .map(|&(sector, count)| {
                let count = u8::try_from(count).map_err(|_| {
                    format!("vertex {vertex} sector {sector} contribution {count} exceeds u8")
                })?;
                Ok((sector, count))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let sector_sum: i16 = selected_sector_contributions
            .iter()
            .map(|&(_, count)| i16::from(count))
            .sum();
        let ear_delta = *evidence.vertex_ear_deltas.get(&vertex).unwrap_or(&0);
        let fixed_degree = i16::from(degree) - sector_sum - ear_delta as i16;
        let fixed_degree = u8::try_from(fixed_degree)
            .map_err(|_| format!("vertex {vertex} fixed degree {fixed_degree} is outside u8"))?;
        let owner_sector_ids = selected_sector_contributions
            .iter()
            .map(|(sector, _)| *sector)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        out.push(DegreeDefectRecord {
            source_slot: vertex,
            fixed_degree,
            selected_sector_contributions,
            final_degree: degree,
            legal_min,
            legal_max,
            deficit: legal_min.saturating_sub(degree),
            excess: degree.saturating_sub(legal_max),
            owner_sector_ids,
            trace_roles: trace_roles(stratified, vertex),
            is_anchor: is_anchor(stratified, vertex),
        });
    }
    out.sort_by_key(|record| record.source_slot);
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SectorPolygon {
    pub id: u64,
    pub vertices: Vec<usize>,
}

pub(super) fn effective_sector_polygons(
    stratified: &StratifiedAnnulus,
) -> Result<Vec<SectorPolygon>, String> {
    let connector_bounded = stratified.shared_junctions.is_empty()
        && stratified
            .bands
            .iter()
            .all(|band| matches!(band.kind, super::BandComponentKind::Annular { .. }));
    let coarse_cycle = stratified
        .coupled
        .coarse_interface
        .vertices
        .iter()
        .map(|vertex| vertex.source_slot)
        .collect::<Vec<_>>();
    stratified
        .probe
        .sector_components
        .iter()
        .enumerate()
        .map(|(id, sector)| {
            let lower = if sector.band_id == 0 {
                contract_chain_to_cycle(&sector.lower_chain, &coarse_cycle)?
            } else {
                sector.lower_chain.clone()
            };
            if lower.len() < 2 || sector.upper_chain.is_empty() {
                return Err(format!("sector {id} chains do not form a disk"));
            }
            let mut vertices = lower;
            let shared_endpoints = vertices.first() == sector.upper_chain.first()
                && vertices.last() == sector.upper_chain.last();
            if shared_endpoints {
                vertices.extend(
                    sector
                        .upper_chain
                        .iter()
                        .rev()
                        .skip(1)
                        .take(sector.upper_chain.len().saturating_sub(2))
                        .copied(),
                );
            } else if connector_bounded {
                vertices.extend(sector.upper_chain.iter().rev().copied());
            } else {
                return Err(format!("sector {id} chains do not share endpoints"));
            }
            Ok(SectorPolygon {
                id: id as u64,
                vertices,
            })
        })
        .collect()
}

type IncidenceSignatureCounts = BTreeMap<Vec<(usize, u8)>, usize>;
type IncidenceMemo = BTreeMap<(usize, usize), IncidenceSignatureCounts>;

fn incidence_signatures(
    polygon: &[usize],
    forbidden_edges: &BTreeSet<(usize, usize)>,
) -> Result<Vec<SectorDegreeSignature>, String> {
    if polygon.len() > u8::MAX as usize {
        return Err(format!(
            "polygon has {} vertices; u8 incidence signatures support at most {}",
            polygon.len(),
            u8::MAX
        ));
    }

    fn rec(
        polygon: &[usize],
        forbidden_edges: &BTreeSet<(usize, usize)>,
        lo: usize,
        hi: usize,
        memo: &mut IncidenceMemo,
    ) -> IncidenceSignatureCounts {
        if hi <= lo + 1 {
            return BTreeMap::from([(Vec::new(), 1)]);
        }
        if let Some(cached) = memo.get(&(lo, hi)) {
            return cached.clone();
        }
        let mut out = BTreeMap::new();
        for mid in lo + 1..hi {
            let triangle = [polygon[lo], polygon[mid], polygon[hi]];
            if !distinct(triangle)
                || triangle_edges(triangle)
                    .iter()
                    .any(|edge| forbidden_edges.contains(edge))
            {
                continue;
            }
            let left = rec(polygon, forbidden_edges, lo, mid, memo);
            let right = rec(polygon, forbidden_edges, mid, hi, memo);
            for (left_sig, left_count) in &left {
                for (right_sig, right_count) in &right {
                    if let Some(sig) = add_triangle_signature(left_sig, right_sig, triangle) {
                        *out.entry(sig).or_default() += left_count * right_count;
                    }
                }
            }
        }
        memo.insert((lo, hi), out.clone());
        out
    }

    Ok(rec(
        polygon,
        forbidden_edges,
        0,
        polygon.len() - 1,
        &mut BTreeMap::new(),
    )
    .into_iter()
    .map(
        |(contributions, member_topology_count)| SectorDegreeSignature {
            contributions,
            member_topology_count,
        },
    )
    .collect())
}

fn add_triangle_signature(
    a: &[(usize, u8)],
    b: &[(usize, u8)],
    triangle: [usize; 3],
) -> Option<Vec<(usize, u8)>> {
    let mut counts = BTreeMap::<usize, u8>::new();
    for &(vertex, count) in a.iter().chain(b) {
        let entry = counts.entry(vertex).or_default();
        *entry = entry.checked_add(count)?;
    }
    for vertex in triangle {
        let entry = counts.entry(vertex).or_default();
        *entry = entry.checked_add(1)?;
    }
    Some(counts.into_iter().collect())
}

fn global_domains(
    fixed_degrees: &BTreeMap<usize, u8>,
    sectors: &[Vec<SectorDegreeSignature>],
    ear_domains: &BTreeMap<usize, BTreeSet<i8>>,
) -> BTreeMap<usize, BTreeSet<u8>> {
    let mut vertex_sector_domains = BTreeMap::<usize, Vec<BTreeSet<u8>>>::new();
    for sector in sectors {
        let mut local = BTreeMap::<usize, BTreeSet<u8>>::new();
        for signature in sector {
            for &(vertex, count) in &signature.contributions {
                local.entry(vertex).or_default().insert(count);
            }
        }
        for (vertex, domain) in local {
            vertex_sector_domains
                .entry(vertex)
                .or_default()
                .push(domain);
        }
    }
    let vertices = fixed_degrees
        .keys()
        .copied()
        .chain(vertex_sector_domains.keys().copied())
        .chain(ear_domains.keys().copied())
        .collect::<BTreeSet<_>>();
    vertices
        .into_iter()
        .map(|vertex| {
            let mut domain = BTreeSet::from([*fixed_degrees.get(&vertex).unwrap_or(&0)]);
            if let Some(sector_domains) = vertex_sector_domains.get(&vertex) {
                for next in sector_domains {
                    domain = minkowski_u8(&domain, next);
                }
            }
            domain = apply_ear_domain(&domain, ear_domains.get(&vertex));
            (vertex, domain)
        })
        .collect()
}

pub fn minkowski_u8(a: &BTreeSet<u8>, b: &BTreeSet<u8>) -> BTreeSet<u8> {
    a.iter()
        .flat_map(|&x| b.iter().filter_map(move |&y| x.checked_add(y)))
        .collect()
}

fn ac3(
    stratified: &StratifiedAnnulus,
    fixed_degrees: &BTreeMap<usize, u8>,
    ear_domains: &BTreeMap<usize, BTreeSet<i8>>,
    sectors: &mut [Vec<SectorDegreeSignature>],
) {
    loop {
        let marginal_domains = sector_vertex_domains(sectors);
        let mut changed = false;
        for (sector_index, sector) in sectors.iter_mut().enumerate() {
            let before = sector.len();
            let keep = sector
                .iter()
                .filter(|signature| {
                    signature_supported(
                        stratified,
                        fixed_degrees,
                        ear_domains,
                        &marginal_domains,
                        sector_index,
                        signature,
                    )
                })
                .cloned()
                .collect::<Vec<_>>();
            *sector = keep;
            changed |= sector.len() != before;
        }
        if !changed {
            break;
        }
    }
}

fn sector_vertex_domains(
    sectors: &[Vec<SectorDegreeSignature>],
) -> Vec<BTreeMap<usize, BTreeSet<u8>>> {
    sectors
        .iter()
        .map(|sector| {
            let mut local = BTreeMap::<usize, BTreeSet<u8>>::new();
            for signature in sector {
                for &(vertex, count) in &signature.contributions {
                    local.entry(vertex).or_default().insert(count);
                }
            }
            local
        })
        .collect()
}

fn signature_supported(
    stratified: &StratifiedAnnulus,
    fixed_degrees: &BTreeMap<usize, u8>,
    ear_domains: &BTreeMap<usize, BTreeSet<i8>>,
    marginal_domains: &[BTreeMap<usize, BTreeSet<u8>>],
    sector_index: usize,
    signature: &SectorDegreeSignature,
) -> bool {
    signature.contributions.iter().all(|&(vertex, count)| {
        let Some(fixed) = fixed_degrees
            .get(&vertex)
            .copied()
            .unwrap_or(0)
            .checked_add(count)
        else {
            return false;
        };
        let mut domain = BTreeSet::from([fixed]);
        for (other_index, local) in marginal_domains.iter().enumerate() {
            if other_index == sector_index {
                continue;
            }
            if let Some(next) = local.get(&vertex) {
                domain = minkowski_u8(&domain, next);
            }
        }
        domain = apply_ear_domain(&domain, ear_domains.get(&vertex));
        legal_values(stratified, vertex)
            .iter()
            .any(|v| domain.contains(v))
    })
}

fn defects(
    stratified: &StratifiedAnnulus,
    fixed_degrees: &BTreeMap<usize, u8>,
    domains: &BTreeMap<usize, BTreeSet<u8>>,
    sectors: &[Vec<SectorDegreeSignature>],
) -> Vec<DegreeDefectRecord> {
    let sector_owners = sector_owner_map(sectors);
    let mut out = Vec::new();
    for (vertex, domain) in domains {
        let legal = legal_values(stratified, *vertex);
        if legal.iter().any(|value| domain.contains(value)) {
            continue;
        }
        let legal_min = *legal.first().unwrap_or(&5);
        let legal_max = *legal.last().unwrap_or(&7);
        let final_degree = domain.iter().copied().next().unwrap_or(0);
        out.push(DegreeDefectRecord {
            source_slot: *vertex,
            fixed_degree: *fixed_degrees.get(vertex).unwrap_or(&0),
            selected_sector_contributions: Vec::new(),
            final_degree,
            legal_min,
            legal_max,
            deficit: legal_min.saturating_sub(*domain.iter().max().unwrap_or(&0)),
            excess: domain
                .iter()
                .min()
                .copied()
                .unwrap_or(0)
                .saturating_sub(legal_max),
            owner_sector_ids: sector_owners.get(vertex).cloned().unwrap_or_default(),
            trace_roles: trace_roles(stratified, *vertex),
            is_anchor: is_anchor(stratified, *vertex),
        });
    }
    out.sort_by_key(|record| record.source_slot);
    out
}

fn legal_values(stratified: &StratifiedAnnulus, vertex: usize) -> Vec<u8> {
    stratified.link_contracts.get(&vertex).map_or_else(
        || vec![5, 6, 7],
        |contract| (contract.target_degree_min..=contract.target_degree_max).collect(),
    )
}

fn conservative_ear_delta_domains(
    stratified: &StratifiedAnnulus,
    sectors: &[Vec<SectorDegreeSignature>],
) -> Result<(BTreeMap<usize, BTreeSet<i8>>, bool), String> {
    let anchors = stratified
        .link_contracts
        .iter()
        .filter_map(|(&vertex, contract)| {
            matches!(
                contract.anchor_kind,
                RingAnchorKind::IcosahedronPentagon { .. }
            )
            .then_some(vertex)
        })
        .collect::<BTreeSet<_>>();
    let max_ears = i8::try_from(anchors.len().saturating_mul(MAX_EARS_PER_ANCHOR))
        .map_err(|_| "generic ear bound exceeds i8 delta evidence".to_string())?;
    let mut all_vertices = BTreeSet::new();
    let mut touchable = anchors.clone();
    for sector in sectors {
        let vertices = sector
            .iter()
            .flat_map(|signature| signature.contributions.iter().map(|&(vertex, _)| vertex))
            .collect::<BTreeSet<_>>();
        all_vertices.extend(vertices.iter().copied());
        if !vertices.is_disjoint(&anchors) {
            touchable.extend(vertices);
        }
    }
    let wide = (-max_ears..=max_ears).collect::<BTreeSet<_>>();
    let mut domains = all_vertices
        .into_iter()
        .map(|vertex| (vertex, BTreeSet::from([0])))
        .collect::<BTreeMap<_, _>>();
    for vertex in touchable {
        domains.insert(vertex, wide.clone());
    }
    Ok((domains, anchors.is_empty()))
}

fn apply_ear_domain(domain: &BTreeSet<u8>, ear_domain: Option<&BTreeSet<i8>>) -> BTreeSet<u8> {
    let Some(ear_domain) = ear_domain else {
        return domain.clone();
    };
    domain
        .iter()
        .flat_map(|&base| {
            ear_domain.iter().filter_map(move |&delta| {
                let value = i16::from(base) + i16::from(delta);
                (0..=u8::MAX as i16).contains(&value).then_some(value as u8)
            })
        })
        .collect()
}

fn sector_owner_map(sectors: &[Vec<SectorDegreeSignature>]) -> BTreeMap<usize, Vec<u64>> {
    let mut out = BTreeMap::<usize, BTreeSet<u64>>::new();
    for (sector, signatures) in sectors.iter().enumerate() {
        for signature in signatures {
            for &(vertex, _) in &signature.contributions {
                out.entry(vertex).or_default().insert(sector as u64);
            }
        }
    }
    out.into_iter()
        .map(|(vertex, sectors)| (vertex, sectors.into_iter().collect()))
        .collect()
}

fn trace_roles(stratified: &StratifiedAnnulus, vertex: usize) -> Vec<TraceRole> {
    let mut out = Vec::new();
    for trace in &stratified.traces {
        if trace
            .occurrences
            .iter()
            .any(|occurrence| occurrence.source_slot == vertex)
            && !out.contains(&trace.role)
        {
            out.push(trace.role);
        }
    }
    out
}

fn is_anchor(stratified: &StratifiedAnnulus, vertex: usize) -> bool {
    stratified
        .link_contracts
        .get(&vertex)
        .is_some_and(|contract| {
            matches!(
                contract.anchor_kind,
                RingAnchorKind::IcosahedronPentagon { .. }
            )
        })
}

fn is_simple_abstract_polygon(vertices: &[usize]) -> bool {
    vertices.len() >= 3 && vertices.iter().collect::<BTreeSet<_>>().len() == vertices.len()
}

fn contract_chain_to_cycle(chain: &[usize], cycle: &[usize]) -> Result<Vec<usize>, String> {
    let start = *chain
        .first()
        .ok_or_else(|| "sector chain is empty".to_string())?;
    let end = *chain
        .last()
        .ok_or_else(|| "sector chain is empty".to_string())?;
    let start_index = cycle
        .iter()
        .position(|&vertex| vertex == start)
        .ok_or_else(|| format!("coarse chain start {start} is absent from coarse cycle"))?;
    let end_index = cycle
        .iter()
        .position(|&vertex| vertex == end)
        .ok_or_else(|| format!("coarse chain end {end} is absent from coarse cycle"))?;
    let forward = cycle_path(cycle, start_index, end_index, 1);
    let backward = cycle_path(cycle, start_index, end_index, cycle.len() - 1);
    let mut matches = [forward, backward]
        .into_iter()
        .filter(|path| is_subsequence(path, chain))
        .collect::<Vec<_>>();
    matches.sort();
    matches.dedup();
    match matches.as_slice() {
        [selected] => Ok(selected.clone()),
        [] => Err(format!(
            "coarse chain {chain:?} has no path on coarse cycle {cycle:?}"
        )),
        _ => Err(format!(
            "coarse chain {chain:?} is ambiguous on coarse cycle {cycle:?}"
        )),
    }
}

fn cycle_path(cycle: &[usize], start: usize, end: usize, step: usize) -> Vec<usize> {
    let mut path = vec![cycle[start]];
    let mut index = start;
    while index != end {
        index = (index + step) % cycle.len();
        path.push(cycle[index]);
    }
    path
}

fn is_subsequence(needle: &[usize], haystack: &[usize]) -> bool {
    let mut next = 0usize;
    for &vertex in haystack {
        if needle.get(next) == Some(&vertex) {
            next += 1;
        }
    }
    next == needle.len()
}

fn triangle_incidence_counts(triangles: &[[usize; 3]]) -> BTreeMap<usize, u8> {
    let mut out = BTreeMap::new();
    for triangle in triangles {
        for &vertex in triangle {
            *out.entry(vertex).or_default() += 1;
        }
    }
    out
}

fn polygon_boundary_edges(vertices: &[usize]) -> BTreeSet<(usize, usize)> {
    vertices
        .iter()
        .copied()
        .zip(vertices.iter().copied().cycle().skip(1))
        .take(vertices.len())
        .map(|(a, b)| sorted(a, b))
        .collect()
}

fn triangle_edges([a, b, c]: [usize; 3]) -> [(usize, usize); 3] {
    [sorted(a, b), sorted(b, c), sorted(c, a)]
}

fn sorted(a: usize, b: usize) -> (usize, usize) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

fn distinct([a, b, c]: [usize; 3]) -> bool {
    a != b && b != c && a != c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalan_signature_multiplicity_3_to_9() {
        let expected = [(3, 1), (4, 2), (5, 5), (6, 14), (7, 42), (8, 132), (9, 429)];
        for (n, catalan) in expected {
            let polygon = (0..n).collect::<Vec<_>>();
            let count: usize = incidence_signatures(&polygon, &BTreeSet::new())
                .unwrap()
                .iter()
                .map(|signature| signature.member_topology_count)
                .sum();
            assert_eq!(count, catalan, "n={n}");
        }
    }

    #[test]
    fn minkowski_sum_is_exact() {
        assert_eq!(
            minkowski_u8(&BTreeSet::from([1, 3]), &BTreeSet::from([2, 4])),
            BTreeSet::from([3, 5, 7])
        );
    }
}
