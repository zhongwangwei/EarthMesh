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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AnchorRepairDepth {
    R2,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PairPermanentRejectReason {
    DuplicateTriangle,
    AnchorBelowTarget {
        anchor: usize,
        degree: usize,
    },
    UnaffectedOrdinaryDegree {
        vertex: usize,
        degree: usize,
    },
    UnaffectedLinkNotSingleCycle {
        vertex: usize,
    },
    AffectedDegreeCapacity {
        after_ears: usize,
        lower: usize,
        upper: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TopologyPairClassV2 {
    DirectNoEarCandidate,
    RepairDepthR2Candidate {
        total_required_ears: u8,
    },
    OutsideRepairDepthR2 {
        required_ears_per_anchor: BTreeMap<usize, u8>,
    },
    ExactImpossibleForAllEarDepths {
        reason: PairPermanentRejectReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum RepairSupportRejectReason {
    DuplicateTriangle,
    AnchorBelowTarget {
        anchor: usize,
        degree: usize,
    },
    UnaffectedOrdinaryDegree {
        vertex: usize,
        degree: usize,
    },
    UnaffectedLinkNotSingleCycle {
        vertex: usize,
    },
    AffectedDegreeCapacity {
        after_ears: usize,
        lower: usize,
        upper: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepairSupportPreflightOutcome {
    Proceed,
    ExactReject {
        reason: RepairSupportRejectReason,
    },
    OutsideRegisteredRepairDepth {
        required_per_anchor: BTreeMap<usize, u8>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairSupportPreflight {
    pub pair_key: TopologyPairKey,
    pub anchor_degrees: BTreeMap<usize, u8>,
    pub required_ears: BTreeMap<usize, u8>,
    pub total_required_ears: u8,
    pub potential_ear_vertices: BTreeSet<usize>,
    pub unaffected_ordinary_defects: Vec<(usize, usize)>,
    pub affected_degree_sum: usize,
    pub affected_degree_lower_capacity: usize,
    pub affected_degree_upper_capacity: usize,
    pub repair_depth: AnchorRepairDepth,
    pub outcome: RepairSupportPreflightOutcome,
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
    pub pair_class_v2: TopologyPairClassV2,
    pub repair_support_preflight: RepairSupportPreflight,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AnchorRepairPortfolioEvidence {
    pub pair_total: usize,
    pub direct_no_ear_candidates: usize,
    pub permanent_impossible: usize,
    pub outside_r2: usize,
    pub r2_anchor_necessary_candidates: usize,
    pub r2_preflight_passed: usize,
    pub unaffected_degree_rejects: usize,
    pub total_capacity_rejects: usize,
    pub fixed_link_rejects: usize,
    pub k_tiers: BTreeMap<u8, usize>,
    pub preflight_passed_by_k: BTreeMap<u8, usize>,
    pub preflight_rejected_by_k: BTreeMap<u8, usize>,
    pub permanent_reasons: BTreeMap<String, usize>,
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
    pub anchor_repair_portfolio: AnchorRepairPortfolioEvidence,
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
        anchor_repair_portfolio: AnchorRepairPortfolioEvidence {
            pair_total: total_pair_product,
            ..AnchorRepairPortfolioEvidence::default()
        },
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
                    record_preflight(
                        &mut evidence.anchor_repair_portfolio,
                        fast_repair_support_preflight(lower, upper, &contracts, &anchors, 0),
                        None,
                    );
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
                    evidence
                        .anchor_repair_portfolio
                        .r2_anchor_necessary_candidates += 1;
                    *evidence
                        .anchor_repair_portfolio
                        .k_tiers
                        .entry(*total_required_ears)
                        .or_default() += 1;
                    record_preflight(
                        &mut evidence.anchor_repair_portfolio,
                        fast_repair_support_preflight(
                            lower,
                            upper,
                            &contracts,
                            &anchors,
                            *total_required_ears,
                        ),
                        Some(*total_required_ears),
                    );
                }
                TopologyPairClass::ImpossibleBeforeEar { reason } => {
                    evidence.impossible_pairs += 1;
                    *impossible_reasons.entry(reason.clone()).or_default() += 1;
                    match reason {
                        PairRejectReason::AnchorAboveRepairRange { .. } => {
                            evidence.anchor_repair_portfolio.outside_r2 += 1;
                        }
                        PairRejectReason::DuplicateTriangle => record_permanent_reject(
                            &mut evidence.anchor_repair_portfolio,
                            PairPermanentRejectReason::DuplicateTriangle,
                        ),
                        PairRejectReason::AnchorBelowRepairRange { anchor, degree } => {
                            record_permanent_reject(
                                &mut evidence.anchor_repair_portfolio,
                                PairPermanentRejectReason::AnchorBelowTarget {
                                    anchor: *anchor,
                                    degree: *degree,
                                },
                            );
                        }
                    }
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
        .collect::<BTreeMap<_, _>>();
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

pub fn repair_support_preflight(
    lower: &CellTopologyMergeSignature,
    upper: &CellTopologyMergeSignature,
    contracts: &BTreeMap<usize, super::VertexLinkContract>,
    anchors: &BTreeSet<usize>,
    duplicate_triangle: bool,
) -> RepairSupportPreflight {
    let mut anchor_degrees = BTreeMap::new();
    let mut required_ears = BTreeMap::new();
    let mut outcome = duplicate_triangle.then_some(RepairSupportPreflightOutcome::ExactReject {
        reason: RepairSupportRejectReason::DuplicateTriangle,
    });
    for &anchor in anchors {
        let contract = &contracts[&anchor];
        let degree = union_link_count(
            &contract.fixed_link_edges,
            lower.vertex_link_edges.get(&anchor),
            upper.vertex_link_edges.get(&anchor),
        );
        anchor_degrees.insert(anchor, u8::try_from(degree).unwrap_or(u8::MAX));
        if degree < usize::from(contract.target_degree_min) {
            outcome.get_or_insert(RepairSupportPreflightOutcome::ExactReject {
                reason: RepairSupportRejectReason::AnchorBelowTarget { anchor, degree },
            });
        }
        let required = degree.saturating_sub(usize::from(contract.target_degree_max));
        if required > 0 {
            required_ears.insert(anchor, u8::try_from(required).unwrap_or(u8::MAX));
        }
    }
    let total_required_ears = required_ears
        .values()
        .copied()
        .fold(0_u8, u8::saturating_add);
    if outcome.is_none() && required_ears.values().any(|&required| required > 2) {
        outcome = Some(
            RepairSupportPreflightOutcome::OutsideRegisteredRepairDepth {
                required_per_anchor: required_ears.clone(),
            },
        );
    }
    let potential_ear_vertices = required_ears
        .keys()
        .flat_map(|anchor| {
            lower
                .anchor_star_triangles
                .get(anchor)
                .into_iter()
                .chain(upper.anchor_star_triangles.get(anchor))
                .flat_map(|triangles| triangles.iter())
        })
        .flat_map(|triangle| triangle.iter().copied())
        .filter(|vertex| !anchors.contains(vertex))
        .collect::<BTreeSet<_>>();
    let mut unaffected_ordinary_defects = Vec::new();
    let mut affected_degree_sum = 0;
    let mut affected_vertices = 0;
    if outcome.is_none() {
        for (&vertex, contract) in contracts {
            if anchors.contains(&vertex) {
                continue;
            }
            let degree = union_link_count(
                &contract.fixed_link_edges,
                lower.vertex_link_edges.get(&vertex),
                upper.vertex_link_edges.get(&vertex),
            );
            // Contracts also cover dormant domain slots; absent vertices are not final defects.
            if degree == 0 {
                continue;
            }
            if potential_ear_vertices.contains(&vertex) {
                affected_vertices += 1;
                affected_degree_sum += degree;
            } else if !(5..=7).contains(&degree) {
                unaffected_ordinary_defects.push((vertex, degree));
                outcome.get_or_insert(RepairSupportPreflightOutcome::ExactReject {
                    reason: RepairSupportRejectReason::UnaffectedOrdinaryDegree { vertex, degree },
                });
            } else if !union_link_is_single_cycle(
                &contract.fixed_link_edges,
                lower.vertex_link_edges.get(&vertex),
                upper.vertex_link_edges.get(&vertex),
            ) {
                outcome.get_or_insert(RepairSupportPreflightOutcome::ExactReject {
                    reason: RepairSupportRejectReason::UnaffectedLinkNotSingleCycle { vertex },
                });
            }
        }
    }
    let affected_degree_lower_capacity = affected_vertices * 5;
    let affected_degree_upper_capacity = affected_vertices * 7;
    // Every anchor ear adds exactly one to the ordinary degree sum on this support.
    let after_ears = affected_degree_sum + usize::from(total_required_ears);
    if outcome.is_none()
        && !(affected_degree_lower_capacity..=affected_degree_upper_capacity).contains(&after_ears)
    {
        outcome = Some(RepairSupportPreflightOutcome::ExactReject {
            reason: RepairSupportRejectReason::AffectedDegreeCapacity {
                after_ears,
                lower: affected_degree_lower_capacity,
                upper: affected_degree_upper_capacity,
            },
        });
    }
    RepairSupportPreflight {
        pair_key: TopologyPairKey {
            lower_topology: lower.topology_key.clone(),
            upper_topology: upper.topology_key.clone(),
        },
        anchor_degrees,
        required_ears,
        total_required_ears,
        potential_ear_vertices,
        unaffected_ordinary_defects,
        affected_degree_sum,
        affected_degree_lower_capacity,
        affected_degree_upper_capacity,
        repair_depth: AnchorRepairDepth::R2,
        outcome: outcome.unwrap_or(RepairSupportPreflightOutcome::Proceed),
    }
}

fn fast_repair_support_preflight(
    lower: &CellTopologyMergeSignature,
    upper: &CellTopologyMergeSignature,
    contracts: &BTreeMap<usize, super::VertexLinkContract>,
    anchors: &BTreeSet<usize>,
    total_required_ears: u8,
) -> Result<(), RepairSupportRejectReason> {
    // Registered domains have four anchors; larger domains retain exactness via the slow path.
    let mut active_anchors = [0; 64];
    let mut active_anchor_count = 0;
    let mut active_anchor_overflow = false;
    for &anchor in anchors {
        if anchor_is_overfull(anchor, lower, upper, contracts) {
            if active_anchor_count < active_anchors.len() {
                active_anchors[active_anchor_count] = anchor;
                active_anchor_count += 1;
            } else {
                active_anchor_overflow = true;
            }
        }
    }
    let mut affected_degree_sum = 0;
    let mut affected_vertices = 0;
    for (&vertex, contract) in contracts {
        if anchors.contains(&vertex) {
            continue;
        }
        let degree = union_link_count(
            &contract.fixed_link_edges,
            lower.vertex_link_edges.get(&vertex),
            upper.vertex_link_edges.get(&vertex),
        );
        if degree == 0 {
            continue;
        }
        let affected = if active_anchor_overflow {
            anchors.iter().copied().any(|anchor| {
                anchor_is_overfull(anchor, lower, upper, contracts)
                    && anchor_star_contains(vertex, anchor, lower, upper)
            })
        } else {
            active_anchors[..active_anchor_count]
                .iter()
                .copied()
                .any(|anchor| anchor_star_contains(vertex, anchor, lower, upper))
        };
        if affected {
            affected_vertices += 1;
            affected_degree_sum += degree;
        } else if !(5..=7).contains(&degree) {
            return Err(RepairSupportRejectReason::UnaffectedOrdinaryDegree { vertex, degree });
        } else if !union_link_is_single_cycle(
            &contract.fixed_link_edges,
            lower.vertex_link_edges.get(&vertex),
            upper.vertex_link_edges.get(&vertex),
        ) {
            return Err(RepairSupportRejectReason::UnaffectedLinkNotSingleCycle { vertex });
        }
    }
    let lower_capacity = affected_vertices * 5;
    let upper_capacity = affected_vertices * 7;
    let after_ears = affected_degree_sum + usize::from(total_required_ears);
    if !(lower_capacity..=upper_capacity).contains(&after_ears) {
        return Err(RepairSupportRejectReason::AffectedDegreeCapacity {
            after_ears,
            lower: lower_capacity,
            upper: upper_capacity,
        });
    }
    Ok(())
}

fn anchor_is_overfull(
    anchor: usize,
    lower: &CellTopologyMergeSignature,
    upper: &CellTopologyMergeSignature,
    contracts: &BTreeMap<usize, super::VertexLinkContract>,
) -> bool {
    let contract = &contracts[&anchor];
    union_link_count(
        &contract.fixed_link_edges,
        lower.vertex_link_edges.get(&anchor),
        upper.vertex_link_edges.get(&anchor),
    ) > usize::from(contract.target_degree_max)
}

fn anchor_star_contains(
    vertex: usize,
    anchor: usize,
    lower: &CellTopologyMergeSignature,
    upper: &CellTopologyMergeSignature,
) -> bool {
    lower
        .anchor_star_triangles
        .get(&anchor)
        .into_iter()
        .chain(upper.anchor_star_triangles.get(&anchor))
        .flatten()
        .any(|triangle| triangle.contains(&vertex))
}

fn record_preflight(
    evidence: &mut AnchorRepairPortfolioEvidence,
    outcome: Result<(), RepairSupportRejectReason>,
    ear_tier: Option<u8>,
) {
    match outcome {
        Ok(()) => match ear_tier {
            Some(ears) => {
                evidence.r2_preflight_passed += 1;
                *evidence.preflight_passed_by_k.entry(ears).or_default() += 1;
            }
            None => evidence.direct_no_ear_candidates += 1,
        },
        Err(reason) => {
            if let Some(ears) = ear_tier {
                *evidence.preflight_rejected_by_k.entry(ears).or_default() += 1;
            }
            match reason {
                RepairSupportRejectReason::UnaffectedOrdinaryDegree { .. } => {
                    evidence.unaffected_degree_rejects += 1;
                }
                RepairSupportRejectReason::UnaffectedLinkNotSingleCycle { .. } => {
                    evidence.fixed_link_rejects += 1;
                }
                RepairSupportRejectReason::AffectedDegreeCapacity { .. } => {
                    evidence.total_capacity_rejects += 1;
                }
                RepairSupportRejectReason::DuplicateTriangle
                | RepairSupportRejectReason::AnchorBelowTarget { .. } => {}
            }
            record_permanent_reject(evidence, permanent_reason(reason));
        }
    }
}

fn record_permanent_reject(
    evidence: &mut AnchorRepairPortfolioEvidence,
    reason: PairPermanentRejectReason,
) {
    evidence.permanent_impossible += 1;
    *evidence
        .permanent_reasons
        .entry(format!("{reason:?}"))
        .or_default() += 1;
}

fn permanent_reason(reason: RepairSupportRejectReason) -> PairPermanentRejectReason {
    match reason {
        RepairSupportRejectReason::DuplicateTriangle => {
            PairPermanentRejectReason::DuplicateTriangle
        }
        RepairSupportRejectReason::AnchorBelowTarget { anchor, degree } => {
            PairPermanentRejectReason::AnchorBelowTarget { anchor, degree }
        }
        RepairSupportRejectReason::UnaffectedOrdinaryDegree { vertex, degree } => {
            PairPermanentRejectReason::UnaffectedOrdinaryDegree { vertex, degree }
        }
        RepairSupportRejectReason::UnaffectedLinkNotSingleCycle { vertex } => {
            PairPermanentRejectReason::UnaffectedLinkNotSingleCycle { vertex }
        }
        RepairSupportRejectReason::AffectedDegreeCapacity {
            after_ears,
            lower,
            upper,
        } => PairPermanentRejectReason::AffectedDegreeCapacity {
            after_ears,
            lower,
            upper,
        },
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
    let duplicate_triangle = has_fixed_duplicate(fixed, &lower_signature)
        || has_fixed_duplicate(fixed, &upper_signature)
        || has_duplicate_triangle_pair(&lower_signature, &upper_signature);
    let mut anchor_degrees = BTreeMap::new();
    let mut required_ears = BTreeMap::new();
    let pair_class = classify_pair(
        &lower_signature,
        &upper_signature,
        contracts,
        anchors,
        duplicate_triangle,
        |anchor, degree| {
            let degree = u8::try_from(degree).unwrap_or(u8::MAX);
            anchor_degrees.insert(anchor, degree);
            let target = contracts[&anchor].target_degree_max;
            if degree > target {
                required_ears.insert(anchor, degree - target);
            }
        },
    );
    let repair_support_preflight = repair_support_preflight(
        &lower_signature,
        &upper_signature,
        contracts,
        anchors,
        duplicate_triangle,
    );
    let pair_class_v2 = pair_class_v2(&repair_support_preflight);
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
        pair_class_v2,
        repair_support_preflight,
    }
}

fn pair_class_v2(preflight: &RepairSupportPreflight) -> TopologyPairClassV2 {
    match &preflight.outcome {
        RepairSupportPreflightOutcome::Proceed if preflight.total_required_ears == 0 => {
            TopologyPairClassV2::DirectNoEarCandidate
        }
        RepairSupportPreflightOutcome::Proceed => TopologyPairClassV2::RepairDepthR2Candidate {
            total_required_ears: preflight.total_required_ears,
        },
        RepairSupportPreflightOutcome::OutsideRegisteredRepairDepth {
            required_per_anchor,
        } => TopologyPairClassV2::OutsideRepairDepthR2 {
            required_ears_per_anchor: required_per_anchor.clone(),
        },
        RepairSupportPreflightOutcome::ExactReject { reason } => {
            TopologyPairClassV2::ExactImpossibleForAllEarDepths {
                reason: permanent_reason(reason.clone()),
            }
        }
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
    while let Some(next) = [
        fixed.peek().map(|&&edge| edge),
        lower.peek().map(|&&edge| edge),
        upper.peek().map(|&&edge| edge),
    ]
    .into_iter()
    .flatten()
    .min()
    {
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
    count
}

fn union_link_is_single_cycle(
    fixed: &BTreeSet<Edge>,
    lower: Option<&BTreeSet<Edge>>,
    upper: Option<&BTreeSet<Edge>>,
) -> bool {
    let mut fixed = fixed.iter().peekable();
    let mut lower = lower.into_iter().flat_map(BTreeSet::iter).peekable();
    let mut upper = upper.into_iter().flat_map(BTreeSet::iter).peekable();
    let mut edges = [(0, 0); 7];
    let mut edge_count = 0;
    while let Some(next) = [
        fixed.peek().map(|&&edge| edge),
        lower.peek().map(|&&edge| edge),
        upper.peek().map(|&&edge| edge),
    ]
    .into_iter()
    .flatten()
    .min()
    {
        if edge_count == edges.len() {
            return false;
        }
        edges[edge_count] = next;
        edge_count += 1;
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
    if edge_count == 0 {
        return false;
    }
    let mut nodes = [0; 14];
    let mut degrees = [0_u8; 14];
    let mut node_count = 0;
    for &(a, b) in &edges[..edge_count] {
        for node in [a, b] {
            let index = if let Some(index) = nodes[..node_count]
                .iter()
                .position(|&candidate| candidate == node)
            {
                index
            } else {
                nodes[node_count] = node;
                node_count += 1;
                node_count - 1
            };
            degrees[index] += 1;
        }
    }
    if degrees[..node_count].iter().any(|&degree| degree != 2) {
        return false;
    }
    let mut seen = [false; 14];
    seen[0] = true;
    loop {
        let mut changed = false;
        for &(a, b) in &edges[..edge_count] {
            let a = nodes[..node_count]
                .iter()
                .position(|&node| node == a)
                .unwrap();
            let b = nodes[..node_count]
                .iter()
                .position(|&node| node == b)
                .unwrap();
            if seen[a] && !seen[b] {
                seen[b] = true;
                changed = true;
            } else if seen[b] && !seen[a] {
                seen[a] = true;
                changed = true;
            }
        }
        if !changed {
            return seen[..node_count].iter().all(|&visited| visited);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coarsen::VertexLinkContract;

    #[test]
    fn degree8_is_outside_r2_not_permanent_impossible() {
        let (lower, upper, contracts, anchors) = fixture(8, None);
        let preflight = repair_support_preflight(&lower, &upper, &contracts, &anchors, false);
        assert!(matches!(
            pair_class_v2(&preflight),
            TopologyPairClassV2::OutsideRepairDepthR2 { .. }
        ));
    }

    #[test]
    fn anchor_below5_is_permanent_impossible() {
        let (lower, upper, contracts, anchors) = fixture(4, None);
        let preflight = repair_support_preflight(&lower, &upper, &contracts, &anchors, false);
        assert!(matches!(
            pair_class_v2(&preflight),
            TopologyPairClassV2::ExactImpossibleForAllEarDepths {
                reason: PairPermanentRejectReason::AnchorBelowTarget { degree: 4, .. }
            }
        ));
    }

    #[test]
    fn affected_capacity_reject_is_sound() {
        let (lower, upper, mut contracts, anchors) = fixture(6, Some(10));
        contracts.insert(10, contract(10, 7, RingAnchorKind::Ordinary));
        let preflight = repair_support_preflight(&lower, &upper, &contracts, &anchors, false);
        assert_eq!(preflight.affected_degree_sum, 7);
        assert_eq!(preflight.total_required_ears, 1);
        assert!(matches!(
            preflight.outcome,
            RepairSupportPreflightOutcome::ExactReject {
                reason: RepairSupportRejectReason::AffectedDegreeCapacity {
                    after_ears: 8,
                    lower: 5,
                    upper: 7
                }
            }
        ));
    }

    #[test]
    fn unaffected_illegal_vertex_rejects() {
        let (lower, upper, mut contracts, anchors) = fixture(6, None);
        contracts.insert(10, contract(10, 8, RingAnchorKind::Ordinary));
        let preflight = repair_support_preflight(&lower, &upper, &contracts, &anchors, false);
        assert_eq!(preflight.unaffected_ordinary_defects, vec![(10, 8)]);
        assert!(matches!(
            preflight.outcome,
            RepairSupportPreflightOutcome::ExactReject {
                reason: RepairSupportRejectReason::UnaffectedOrdinaryDegree {
                    vertex: 10,
                    degree: 8
                }
            }
        ));
    }

    #[test]
    fn unaffected_broken_link_rejects() {
        let (lower, upper, mut contracts, anchors) = fixture(6, None);
        let mut broken = contract(10, 6, RingAnchorKind::Ordinary);
        broken.fixed_link_edges = BTreeSet::from([
            (100, 101),
            (100, 102),
            (101, 102),
            (103, 104),
            (103, 105),
            (104, 105),
        ]);
        contracts.insert(10, broken);
        let preflight = repair_support_preflight(&lower, &upper, &contracts, &anchors, false);
        assert!(matches!(
            preflight.outcome,
            RepairSupportPreflightOutcome::ExactReject {
                reason: RepairSupportRejectReason::UnaffectedLinkNotSingleCycle { vertex: 10 }
            }
        ));
    }

    #[test]
    fn target_degree_anchor_has_no_ear_support() {
        let (lower, upper, mut contracts, anchors) = fixture(5, Some(10));
        contracts.insert(10, contract(10, 8, RingAnchorKind::Ordinary));
        let preflight = repair_support_preflight(&lower, &upper, &contracts, &anchors, false);
        assert!(preflight.potential_ear_vertices.is_empty());
        assert!(matches!(
            preflight.outcome,
            RepairSupportPreflightOutcome::ExactReject {
                reason: RepairSupportRejectReason::UnaffectedOrdinaryDegree {
                    vertex: 10,
                    degree: 8
                }
            }
        ));
    }

    fn fixture(
        anchor_degree: usize,
        support: Option<usize>,
    ) -> (
        CellTopologyMergeSignature,
        CellTopologyMergeSignature,
        BTreeMap<usize, VertexLinkContract>,
        BTreeSet<usize>,
    ) {
        let mut lower = empty_signature(1);
        if let Some(support) = support {
            lower
                .anchor_star_triangles
                .insert(1, vec![[1, support, support]]);
        }
        let upper = empty_signature(2);
        let anchors = BTreeSet::from([1]);
        let contracts = BTreeMap::from([(
            1,
            contract(
                1,
                anchor_degree,
                RingAnchorKind::IcosahedronPentagon { base_vertex: 0 },
            ),
        )]);
        (lower, upper, contracts, anchors)
    }

    fn empty_signature(cell_id: u64) -> CellTopologyMergeSignature {
        CellTopologyMergeSignature {
            cell_id,
            topology_key: TransitionCellTopologyKey {
                cell_id,
                triangles: Vec::new(),
            },
            vertex_incidences: Vec::new(),
            vertex_link_edges: BTreeMap::new(),
            edge_counts: Vec::new(),
            triangle_keys: Vec::new(),
            anchor_incidence: Vec::new(),
            anchor_star_triangles: BTreeMap::new(),
            geometry_key: FullPolygonGeometryKey::default(),
        }
    }

    fn contract(slot: usize, degree: usize, anchor_kind: RingAnchorKind) -> VertexLinkContract {
        let nodes = (100..100 + degree).collect::<Vec<_>>();
        let fixed_link_edges = nodes
            .iter()
            .copied()
            .zip(nodes.iter().copied().cycle().skip(1))
            .take(nodes.len())
            .map(|(a, b)| (a.min(b), a.max(b)))
            .collect::<BTreeSet<_>>();
        VertexLinkContract {
            source_slot: slot,
            fixed_link_nodes: fixed_link_edges.iter().flat_map(|&(a, b)| [a, b]).collect(),
            fixed_link_edges,
            target_degree_min: 5,
            target_degree_max: if matches!(anchor_kind, RingAnchorKind::Ordinary) {
                7
            } else {
                5
            },
            anchor_kind,
        }
    }
}
