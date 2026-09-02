//! Read-only audit of V3 transition-cell topology pairs and anchor-ear work.

use super::global_exact_merge::{
    edge_counts, final_gate_with_contracts, fixed_triangles_for_face_complex, mesh_edges,
    replace_fixed_link_contract_map, single_cycle_link,
    solve_ears_with_contracts_limited_and_telemetry, vertex_degrees, AnchorEarSearchTelemetry,
    EarSolve, MAX_EARS_PER_ANCHOR,
};
use super::{
    transition_cell_topology_from_annular, FullPolygonGeometryKey, GlobalExactMergeEvidence,
    HierarchyComponent, RingAnchorKind, StratifiedTransitionDomainV3, TransitionCellFamily,
    TransitionCellTopology, TransitionCellTopologyKey,
};
use crate::MotherGrid;
use std::collections::{BTreeMap, BTreeSet};

type Edge = (usize, usize);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellTopologyMergeSignature {
    pub cell_id: u64,
    pub topology_key: TransitionCellTopologyKey,
    pub vertex_incidences: Vec<(usize, u8)>,
    pub vertex_link_edges: BTreeMap<usize, BTreeSet<Edge>>,
    pub edge_counts: Vec<(Edge, u8)>,
    pub triangle_keys: Vec<[usize; 3]>,
    pub anchor_incidence: Vec<(usize, u8)>,
    pub anchor_star_triangles: BTreeMap<usize, Vec<[usize; 3]>>,
    pub geometry_key: FullPolygonGeometryKey,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TopologyPairKey {
    pub lower_topology: TransitionCellTopologyKey,
    pub upper_topology: TransitionCellTopologyKey,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PairRejectReason {
    DuplicateTriangle,
    AnchorBelowRepairRange { anchor: usize, degree: usize },
    AnchorAboveRepairRange { anchor: usize, degree: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TopologyPairClass {
    DirectlyClosedWithoutEar,
    NoEarFinalGateCandidate,
    EarRepairCandidate {
        total_required_ears: u8,
        overfull_anchors: u8,
    },
    ImpossibleBeforeEar {
        reason: PairRejectReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopologyPairSignature {
    pub pair_key: TopologyPairKey,
    pub anchor_degrees_before_ears: BTreeMap<usize, u8>,
    pub required_ears: BTreeMap<usize, u8>,
    pub ordinary_degree_defect_lower_bound: usize,
    pub unmatched_edge_count: usize,
    pub nonrepairable_link_count: usize,
    pub pair_class: TopologyPairClass,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FairPairAuditEvidence {
    pub cell_family_counts: Vec<usize>,
    pub total_pair_product: usize,
    pub zero_ear_pairs: usize,
    pub direct_zero_ear_closures: usize,
    pub repairable_pairs: usize,
    pub impossible_pairs: usize,
    pub low_ear_pairs: BTreeMap<u8, usize>,
    pub anchor_degree_histogram: BTreeMap<usize, usize>,
    pub impossible_reasons: BTreeMap<String, usize>,
    pub zero_ear_final_rejects: BTreeMap<String, usize>,
    pub first_pair_rank_by_repair_score: usize,
    pub first_pair: Option<TopologyPairSignature>,
    pub best_ranked_pair: Option<TopologyPairSignature>,
    pub first_pair_ear_outcome: Option<String>,
    pub first_pair_ear_telemetry: Option<AnchorEarSearchTelemetry>,
}

pub fn audit_transition_cell_pairs(
    source: &MotherGrid,
    component: &HierarchyComponent,
    domain: &StratifiedTransitionDomainV3,
    families: &[TransitionCellFamily],
    trace_first_pair_ears: bool,
) -> Result<FairPairAuditEvidence, String> {
    let concrete = concrete_families(families)?;
    if concrete.len() != 2 {
        return Err(format!(
            "fair W2 pair audit requires two cell families, got {}",
            concrete.len()
        ));
    }
    let cell_family_counts = concrete.iter().map(Vec::len).collect::<Vec<_>>();
    let total_pair_product = concrete[0]
        .len()
        .checked_mul(concrete[1].len())
        .ok_or_else(|| "fair W2 pair product exceeds usize".to_string())?;
    if total_pair_product == 0 {
        return Ok(FairPairAuditEvidence {
            cell_family_counts,
            ..FairPairAuditEvidence::default()
        });
    }
    let annulus_faces = domain
        .topology_domain
        .annulus_face_slots
        .iter()
        .copied()
        .collect::<Vec<_>>();
    let fixed = fixed_triangles_for_face_complex(source, component, &annulus_faces)?;
    let fixed_edges = mesh_edges(&fixed);
    let mut contracts = domain.link_contracts.clone();
    replace_fixed_link_contract_map(&mut contracts, &fixed);
    let anchors = contracts
        .iter()
        .filter_map(|(&slot, contract)| {
            matches!(
                contract.anchor_kind,
                RingAnchorKind::IcosahedronPentagon { .. }
            )
            .then_some(slot)
        })
        .collect::<BTreeSet<_>>();
    let signatures = concrete
        .iter()
        .map(|family| {
            family
                .iter()
                .map(|topology| cell_signature(topology, &anchors))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let fixed_keys = fixed
        .iter()
        .copied()
        .map(canonical_triangle)
        .collect::<BTreeSet<_>>();
    let duplicates_fixed = signatures
        .iter()
        .map(|family| {
            family
                .iter()
                .map(|signature| {
                    signature
                        .triangle_keys
                        .iter()
                        .any(|triangle| fixed_keys.contains(triangle))
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut evidence = FairPairAuditEvidence {
        cell_family_counts,
        total_pair_product,
        ..FairPairAuditEvidence::default()
    };
    let mut first_score = None;
    let mut first_pair_rank = 1;
    let mut best = None;
    let mut zero_ear_candidates = Vec::new();
    let mut impossible_reasons = BTreeMap::<PairRejectReason, usize>::new();
    for (lower_index, lower) in signatures[0].iter().enumerate() {
        for (upper_index, upper) in signatures[1].iter().enumerate() {
            let class = classify_pair(
                lower,
                upper,
                &contracts,
                &anchors,
                duplicates_fixed[0][lower_index]
                    || duplicates_fixed[1][upper_index]
                    || has_duplicate_triangle_pair(lower, upper),
                |_, degree| {
                    *evidence.anchor_degree_histogram.entry(degree).or_default() += 1;
                },
            );
            match &class {
                TopologyPairClass::NoEarFinalGateCandidate => {
                    evidence.zero_ear_pairs += 1;
                    zero_ear_candidates.push((lower_index, upper_index));
                }
                TopologyPairClass::EarRepairCandidate {
                    total_required_ears,
                    ..
                } => {
                    evidence.repairable_pairs += 1;
                    *evidence
                        .low_ear_pairs
                        .entry(*total_required_ears)
                        .or_default() += 1;
                }
                TopologyPairClass::ImpossibleBeforeEar { reason } => {
                    evidence.impossible_pairs += 1;
                    *impossible_reasons.entry(reason.clone()).or_default() += 1;
                }
                TopologyPairClass::DirectlyClosedWithoutEar => unreachable!(),
            }
            let score = repair_score(&class, lower_index, upper_index);
            match first_score {
                None => first_score = Some(score),
                Some(first) if score < first => first_pair_rank += 1,
                Some(_) => {}
            }
            if best.is_none_or(|(best_score, _, _)| score < best_score) {
                best = Some((score, lower_index, upper_index));
            }
        }
    }
    evidence.impossible_reasons = impossible_reasons
        .into_iter()
        .map(|(reason, count)| (format!("{reason:?}"), count))
        .collect();
    evidence.first_pair_rank_by_repair_score = first_pair_rank;
    evidence.first_pair = Some(detailed_pair_signature(
        &fixed,
        &contracts,
        &anchors,
        &concrete[0][0],
        &concrete[1][0],
    ));
    if let Some((_, lower, upper)) = best {
        evidence.best_ranked_pair = Some(detailed_pair_signature(
            &fixed,
            &contracts,
            &anchors,
            &concrete[0][lower],
            &concrete[1][upper],
        ));
    }
    for (lower, upper) in zero_ear_candidates {
        let mut signature = detailed_pair_signature(
            &fixed,
            &contracts,
            &anchors,
            &concrete[0][lower],
            &concrete[1][upper],
        );
        let mut final_triangles = fixed.clone();
        final_triangles.extend(pair_triangles(&concrete[0][lower], &concrete[1][upper]));
        let mut global = GlobalExactMergeEvidence::default();
        match final_gate_with_contracts(source, &contracts, &final_triangles, &mut global) {
            Ok(()) => {
                evidence.direct_zero_ear_closures += 1;
                signature.pair_class = TopologyPairClass::DirectlyClosedWithoutEar;
                if evidence
                    .best_ranked_pair
                    .as_ref()
                    .is_some_and(|best| best.pair_key == signature.pair_key)
                {
                    evidence.best_ranked_pair = Some(signature);
                }
            }
            Err(reason) => {
                *evidence.zero_ear_final_rejects.entry(reason).or_default() += 1;
            }
        }
    }
    if trace_first_pair_ears {
        let mutable = owned_pair_triangles(&concrete[0][0], &concrete[1][0]);
        let mut global = GlobalExactMergeEvidence::default();
        let mut states = 0;
        let mut telemetry = AnchorEarSearchTelemetry::default();
        let outcome = solve_ears_with_contracts_limited_and_telemetry(
            source,
            &contracts,
            &fixed_edges,
            &fixed,
            mutable,
            &mut global,
            &mut states,
            256,
            &mut telemetry,
        );
        evidence.first_pair_ear_outcome = Some(
            match outcome {
                EarSolve::Solved { .. } => "Closed",
                EarSolve::NoSolution => "ExactNoSolution",
                EarSolve::SearchIncomplete => "SearchIncomplete",
                EarSolve::Invalid(_) => "Invalid",
            }
            .into(),
        );
        evidence.first_pair_ear_telemetry = Some(telemetry);
    }
    Ok(evidence)
}

fn concrete_families(
    families: &[TransitionCellFamily],
) -> Result<Vec<Vec<TransitionCellTopology>>, String> {
    families
        .iter()
        .map(|family| match family {
            TransitionCellFamily::Disk { topologies, .. } => Ok(topologies.clone()),
            TransitionCellFamily::Annulus(family) => family
                .family
                .topologies
                .iter()
                .map(|topology| transition_cell_topology_from_annular(family.cell_id, topology))
                .collect(),
        })
        .collect()
}

fn cell_signature(
    topology: &TransitionCellTopology,
    anchors: &BTreeSet<usize>,
) -> CellTopologyMergeSignature {
    let mut triangle_keys = topology
        .triangles
        .iter()
        .map(|triangle| canonical_triangle(triangle.vertices))
        .collect::<Vec<_>>();
    triangle_keys.sort_unstable();
    let counts = edge_counts(&triangle_keys);
    let anchor_star_triangles = anchors
        .iter()
        .filter_map(|anchor| {
            let triangles = triangle_keys
                .iter()
                .copied()
                .filter(|triangle| triangle.contains(anchor))
                .collect::<Vec<_>>();
            (!triangles.is_empty()).then_some((*anchor, triangles))
        })
        .collect();
    CellTopologyMergeSignature {
        cell_id: topology.cell_id,
        topology_key: topology.topology_key.clone(),
        vertex_incidences: topology
            .vertex_incidences
            .iter()
            .map(|(&vertex, &count)| (vertex, count))
            .collect(),
        vertex_link_edges: topology.vertex_link_edges.clone(),
        edge_counts: counts
            .into_iter()
            .map(|(edge, count)| (edge, u8::try_from(count).unwrap_or(u8::MAX)))
            .collect(),
        triangle_keys,
        anchor_incidence: anchors
            .iter()
            .filter_map(|anchor| {
                topology
                    .vertex_incidences
                    .get(anchor)
                    .map(|&count| (*anchor, count))
            })
            .collect(),
        anchor_star_triangles,
        // PR110 is topology-only; geometry is neither evaluated nor used to reject pairs.
        geometry_key: FullPolygonGeometryKey::default(),
    }
}

fn classify_pair(
    lower: &CellTopologyMergeSignature,
    upper: &CellTopologyMergeSignature,
    contracts: &BTreeMap<usize, super::VertexLinkContract>,
    anchors: &BTreeSet<usize>,
    duplicate_triangle: bool,
    mut observe_degree: impl FnMut(usize, usize),
) -> TopologyPairClass {
    let mut total_required_ears = 0;
    let mut overfull_anchors = 0;
    let mut rejection = duplicate_triangle.then_some(PairRejectReason::DuplicateTriangle);
    for &anchor in anchors {
        let contract = &contracts[&anchor];
        let degree = union_link_count(
            &contract.fixed_link_edges,
            lower.vertex_link_edges.get(&anchor),
            upper.vertex_link_edges.get(&anchor),
        );
        observe_degree(anchor, degree);
        if degree < usize::from(contract.target_degree_min) {
            rejection.get_or_insert(PairRejectReason::AnchorBelowRepairRange { anchor, degree });
        } else if degree
            > usize::from(contract.target_degree_max).saturating_add(MAX_EARS_PER_ANCHOR)
        {
            rejection.get_or_insert(PairRejectReason::AnchorAboveRepairRange { anchor, degree });
        } else if degree > usize::from(contract.target_degree_max) {
            total_required_ears += degree - usize::from(contract.target_degree_max);
            overfull_anchors += 1;
        }
    }
    if let Some(reason) = rejection {
        TopologyPairClass::ImpossibleBeforeEar { reason }
    } else if total_required_ears == 0 {
        TopologyPairClass::NoEarFinalGateCandidate
    } else {
        TopologyPairClass::EarRepairCandidate {
            total_required_ears: u8::try_from(total_required_ears).unwrap_or(u8::MAX),
            overfull_anchors: u8::try_from(overfull_anchors).unwrap_or(u8::MAX),
        }
    }
}

fn detailed_pair_signature(
    fixed: &[[usize; 3]],
    contracts: &BTreeMap<usize, super::VertexLinkContract>,
    anchors: &BTreeSet<usize>,
    lower: &TransitionCellTopology,
    upper: &TransitionCellTopology,
) -> TopologyPairSignature {
    let lower_signature = cell_signature(lower, anchors);
    let upper_signature = cell_signature(upper, anchors);
    let mut anchor_degrees = BTreeMap::new();
    let mut required_ears = BTreeMap::new();
    let pair_class = classify_pair(
        &lower_signature,
        &upper_signature,
        contracts,
        anchors,
        has_fixed_duplicate(fixed, &lower_signature)
            || has_fixed_duplicate(fixed, &upper_signature)
            || has_duplicate_triangle_pair(&lower_signature, &upper_signature),
        |anchor, degree| {
            let degree = u8::try_from(degree).unwrap_or(u8::MAX);
            anchor_degrees.insert(anchor, degree);
            let target = contracts[&anchor].target_degree_max;
            if degree > target {
                required_ears.insert(anchor, degree - target);
            }
        },
    );
    let mut triangles = fixed.to_vec();
    triangles.extend(pair_triangles(lower, upper));
    let degrees = vertex_degrees(&triangles);
    let ordinary_degree_defect_lower_bound = degrees
        .iter()
        .filter(|(vertex, degree)| !anchors.contains(vertex) && !(5..=7).contains(*degree))
        .count();
    let unmatched_edge_count = edge_counts(&triangles)
        .values()
        .filter(|&&count| count != 2)
        .count();
    let nonrepairable_link_count = degrees
        .keys()
        .filter(|vertex| !anchors.contains(vertex) && !single_cycle_link(**vertex, &triangles))
        .count();
    TopologyPairSignature {
        pair_key: TopologyPairKey {
            lower_topology: lower.topology_key.clone(),
            upper_topology: upper.topology_key.clone(),
        },
        anchor_degrees_before_ears: anchor_degrees,
        required_ears,
        ordinary_degree_defect_lower_bound,
        unmatched_edge_count,
        nonrepairable_link_count,
        pair_class,
    }
}

fn repair_score(
    class: &TopologyPairClass,
    lower: usize,
    upper: usize,
) -> (u8, u8, u8, usize, usize) {
    match class {
        TopologyPairClass::DirectlyClosedWithoutEar
        | TopologyPairClass::NoEarFinalGateCandidate => (0, 0, 0, lower, upper),
        TopologyPairClass::EarRepairCandidate {
            total_required_ears,
            overfull_anchors,
        } => (1, *total_required_ears, *overfull_anchors, lower, upper),
        TopologyPairClass::ImpossibleBeforeEar { .. } => (2, u8::MAX, u8::MAX, lower, upper),
    }
}

fn union_link_count(
    fixed: &BTreeSet<Edge>,
    lower: Option<&BTreeSet<Edge>>,
    upper: Option<&BTreeSet<Edge>>,
) -> usize {
    let mut fixed = fixed.iter().peekable();
    let mut lower = lower.into_iter().flat_map(BTreeSet::iter).peekable();
    let mut upper = upper.into_iter().flat_map(BTreeSet::iter).peekable();
    let mut count = 0;
    loop {
        let Some(next) = [
            fixed.peek().map(|&&edge| edge),
            lower.peek().map(|&&edge| edge),
            upper.peek().map(|&&edge| edge),
        ]
        .into_iter()
        .flatten()
        .min() else {
            return count;
        };
        count += 1;
        if fixed.peek().is_some_and(|&&edge| edge == next) {
            fixed.next();
        }
        if lower.peek().is_some_and(|&&edge| edge == next) {
            lower.next();
        }
        if upper.peek().is_some_and(|&&edge| edge == next) {
            upper.next();
        }
    }
}

fn has_duplicate_triangle_pair(
    lower: &CellTopologyMergeSignature,
    upper: &CellTopologyMergeSignature,
) -> bool {
    let mut lower = lower.triangle_keys.iter().peekable();
    let mut upper = upper.triangle_keys.iter().peekable();
    while let (Some(left), Some(right)) = (lower.peek(), upper.peek()) {
        match left.cmp(right) {
            std::cmp::Ordering::Less => {
                lower.next();
            }
            std::cmp::Ordering::Greater => {
                upper.next();
            }
            std::cmp::Ordering::Equal => return true,
        }
    }
    false
}

fn has_fixed_duplicate(fixed: &[[usize; 3]], signature: &CellTopologyMergeSignature) -> bool {
    fixed.iter().any(|triangle| {
        signature
            .triangle_keys
            .binary_search(&canonical_triangle(*triangle))
            .is_ok()
    })
}

fn pair_triangles(
    lower: &TransitionCellTopology,
    upper: &TransitionCellTopology,
) -> Vec<[usize; 3]> {
    lower
        .triangles
        .iter()
        .chain(&upper.triangles)
        .map(|triangle| triangle.vertices)
        .collect()
}

fn owned_pair_triangles(
    lower: &TransitionCellTopology,
    upper: &TransitionCellTopology,
) -> Vec<super::OwnedTopologyTriangle> {
    lower
        .triangles
        .iter()
        .chain(&upper.triangles)
        .copied()
        .map(|mut triangle| {
            triangle.topology_id = 1;
            triangle
        })
        .collect()
}

fn canonical_triangle(mut triangle: [usize; 3]) -> [usize; 3] {
    triangle.sort_unstable();
    triangle
}
