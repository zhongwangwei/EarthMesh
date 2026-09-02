//! Exact annular witnesses for fixed root and global incidence targets.

use super::{
    annular_enumerator::glue_cut_topology, annular_reachability::annular_topology_signature,
    cut_annulus_polygon, enumerate_polygon_incidence_witnesses,
    polygon_incidence_ear::occurrence_triangle_indices, AnnularCellDomain, AnnularTopology,
    AnnularTopologyKey, AnnularTopologySignature, OccurrenceIncidenceTarget,
};
use std::collections::{BTreeMap, BTreeSet};

type Edge = (usize, usize);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AnnularTargetProblemKey {
    pub contract_version: u32,
    pub lower_cycle: Vec<usize>,
    pub upper_cycle: Vec<usize>,
    pub forbidden_edges: Vec<Edge>,
    pub target_incidences: Vec<(usize, u8)>,
    pub root_bridge: Edge,
}

pub type AnnularIncidenceTargetKey = AnnularTargetProblemKey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnularIncidenceTarget {
    pub cell_id: u64,
    pub root_bridge: Edge,
    pub global_vertex_incidences: BTreeMap<usize, u8>,
    pub target_key: AnnularIncidenceTargetKey,
}

impl AnnularIncidenceTarget {
    pub fn new(
        cell: &AnnularCellDomain,
        root_bridge: Edge,
        global_vertex_incidences: BTreeMap<usize, u8>,
    ) -> Self {
        let root_bridge = edge(root_bridge.0, root_bridge.1);
        let target_key = target_key(cell, root_bridge, &global_vertex_incidences);
        Self {
            cell_id: cell.cell_id,
            root_bridge,
            global_vertex_incidences,
            target_key,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnularConcreteWitness {
    pub cell_id: u64,
    pub target: AnnularIncidenceTarget,
    pub topology: AnnularTopology,
    pub exact_signature: AnnularTopologySignature,
    pub interior_edges: BTreeSet<Edge>,
    pub topology_key: AnnularTopologyKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AnnularTargetWitnessEvidence {
    pub root_splits_considered: u64,
    pub split_bound_rejects: u64,
    pub pier_states: u64,
    pub occurrence_witnesses: u64,
    pub glue_rejects: BTreeMap<String, u64>,
    pub exact_signature_rejects: u64,
    pub duplicate_topologies: u64,
    pub topologies_found: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnnularTargetWitnessOutcome {
    Found {
        witnesses: Vec<AnnularConcreteWitness>,
        evidence: AnnularTargetWitnessEvidence,
    },
    ExactNoWitness {
        target: AnnularIncidenceTargetKey,
        evidence: AnnularTargetWitnessEvidence,
    },
    InvalidInput(String),
}

pub fn enumerate_annular_incidence_targets(
    cell: &AnnularCellDomain,
    global_vertex_incidences: &BTreeMap<usize, u8>,
) -> Result<Vec<AnnularIncidenceTarget>, String> {
    validate_incidences(cell, global_vertex_incidences)?;
    let mut targets = cell
        .lower_cycle
        .iter()
        .flat_map(|&lower| {
            cell.upper_cycle.iter().map(move |&upper| {
                AnnularIncidenceTarget::new(
                    cell,
                    edge(lower, upper),
                    global_vertex_incidences.clone(),
                )
            })
        })
        .collect::<Vec<_>>();
    targets.sort_by_key(|target| target.root_bridge);
    Ok(targets)
}

pub fn recover_annular_target_witnesses(
    cell: &AnnularCellDomain,
    target: &AnnularIncidenceTarget,
) -> AnnularTargetWitnessOutcome {
    if let Err(reason) = validate_target(cell, target) {
        return AnnularTargetWitnessOutcome::InvalidInput(reason);
    }
    let lower_root = cell
        .lower_cycle
        .iter()
        .position(|slot| target.root_bridge.0 == *slot || target.root_bridge.1 == *slot)
        .expect("validated lower root");
    let upper_root = cell
        .upper_cycle
        .iter()
        .position(|slot| target.root_bridge.0 == *slot || target.root_bridge.1 == *slot)
        .expect("validated upper root");
    let cut =
        match cut_annulus_polygon(&cell.lower_cycle, &cell.upper_cycle, lower_root, upper_root) {
            Ok(cut) => cut,
            Err(error) => return AnnularTargetWitnessOutcome::InvalidInput(format!("{error:?}")),
        };
    let lower_slot = cell.lower_cycle[lower_root];
    let upper_slot = cell.upper_cycle[upper_root];
    let lower_total = target.global_vertex_incidences[&lower_slot];
    let upper_total = target.global_vertex_incidences[&upper_slot];
    let mut evidence = AnnularTargetWitnessEvidence::default();
    let mut unique = BTreeMap::<AnnularTopologyKey, AnnularConcreteWitness>::new();
    for lower_first in 1..lower_total {
        for upper_first in 1..upper_total {
            evidence.root_splits_considered += 1;
            let incidences = occurrence_incidences(
                target,
                &cut,
                lower_slot,
                upper_slot,
                lower_first,
                upper_first,
            );
            if incidences
                .iter()
                .any(|&incidence| usize::from(incidence) > cut.occurrences.len() - 2)
            {
                evidence.split_bound_rejects += 1;
                continue;
            }
            let occurrence_target = OccurrenceIncidenceTarget::new(
                cell.lower_cycle.clone(),
                cell.upper_cycle.clone(),
                cut.clone(),
                incidences,
                cell.forbidden_global_edges.clone(),
            );
            let (occurrence_witnesses, pier_evidence) =
                match enumerate_polygon_incidence_witnesses(&occurrence_target) {
                    Ok(result) => result,
                    Err(reason) => return AnnularTargetWitnessOutcome::InvalidInput(reason),
                };
            evidence.pier_states += pier_evidence.states;
            evidence.occurrence_witnesses += occurrence_witnesses.len() as u64;
            for occurrence_witness in occurrence_witnesses {
                let triangles = match occurrence_triangle_indices(&cut, &occurrence_witness) {
                    Ok(triangles) => triangles,
                    Err(reason) => return AnnularTargetWitnessOutcome::InvalidInput(reason),
                };
                let topology = match glue_cut_topology(
                    &cell.lower_cycle,
                    &cell.upper_cycle,
                    &cut,
                    &triangles,
                    &cell.forbidden_global_edges,
                ) {
                    Ok(topology) => topology,
                    Err(reason) => {
                        *evidence.glue_rejects.entry(reason).or_default() += 1;
                        continue;
                    }
                };
                let exact_signature = match annular_topology_signature(cell, &topology.triangles) {
                    Ok(signature) => signature,
                    Err(error) => {
                        *evidence
                            .glue_rejects
                            .entry(format!("{error:?}"))
                            .or_default() += 1;
                        continue;
                    }
                };
                if exact_signature.root_bridge != target.root_bridge
                    || exact_signature.vertex_incidences
                        != target
                            .global_vertex_incidences
                            .iter()
                            .map(|(&slot, &incidence)| (slot, incidence))
                            .collect::<Vec<_>>()
                {
                    evidence.exact_signature_rejects += 1;
                    continue;
                }
                let topology_key = topology.topology_key.clone();
                let witness = AnnularConcreteWitness {
                    cell_id: cell.cell_id,
                    target: target.clone(),
                    interior_edges: interior_edges(cell, &topology.triangles),
                    exact_signature,
                    topology,
                    topology_key: topology_key.clone(),
                };
                if unique.insert(topology_key, witness).is_some() {
                    evidence.duplicate_topologies += 1;
                }
            }
        }
    }
    evidence.topologies_found = unique.len();
    if unique.is_empty() {
        AnnularTargetWitnessOutcome::ExactNoWitness {
            target: target.target_key.clone(),
            evidence,
        }
    } else {
        AnnularTargetWitnessOutcome::Found {
            witnesses: unique.into_values().collect(),
            evidence,
        }
    }
}

fn validate_target(
    cell: &AnnularCellDomain,
    target: &AnnularIncidenceTarget,
) -> Result<(), String> {
    validate_incidences(cell, &target.global_vertex_incidences)?;
    let lower = cell.lower_cycle.iter().copied().collect::<BTreeSet<_>>();
    let upper = cell.upper_cycle.iter().copied().collect::<BTreeSet<_>>();
    if target.cell_id != cell.cell_id
        || target.root_bridge.0 == target.root_bridge.1
        || !((lower.contains(&target.root_bridge.0) && upper.contains(&target.root_bridge.1))
            || (lower.contains(&target.root_bridge.1) && upper.contains(&target.root_bridge.0)))
        || target.target_key
            != target_key(cell, target.root_bridge, &target.global_vertex_incidences)
    {
        return Err("annular incidence target identity or root is invalid".into());
    }
    Ok(())
}

fn validate_incidences(
    cell: &AnnularCellDomain,
    incidences: &BTreeMap<usize, u8>,
) -> Result<(), String> {
    let vertices = cell
        .lower_cycle
        .iter()
        .chain(&cell.upper_cycle)
        .copied()
        .collect::<BTreeSet<_>>();
    if cell.lower_cycle.len() < 3
        || cell.upper_cycle.len() < 3
        || vertices.len() != cell.lower_cycle.len() + cell.upper_cycle.len()
        || incidences.keys().copied().collect::<BTreeSet<_>>() != vertices
        || incidences.values().any(|&incidence| incidence == 0)
        || incidences
            .values()
            .map(|&value| usize::from(value))
            .sum::<usize>()
            != 3 * vertices.len()
    {
        return Err("annular target incidences do not exactly cover the cell".into());
    }
    Ok(())
}

fn occurrence_incidences(
    target: &AnnularIncidenceTarget,
    cut: &super::CutAnnulusPolygon,
    lower_slot: usize,
    upper_slot: usize,
    lower_first: u8,
    upper_first: u8,
) -> Vec<u8> {
    cut.occurrences
        .iter()
        .map(|occurrence| {
            if occurrence.global_source_slot == lower_slot {
                if occurrence.occurrence_ordinal == 0 {
                    lower_first
                } else {
                    target.global_vertex_incidences[&lower_slot] - lower_first
                }
            } else if occurrence.global_source_slot == upper_slot {
                if occurrence.occurrence_ordinal == 0 {
                    upper_first
                } else {
                    target.global_vertex_incidences[&upper_slot] - upper_first
                }
            } else {
                target.global_vertex_incidences[&occurrence.global_source_slot]
            }
        })
        .collect()
}

fn target_key(
    cell: &AnnularCellDomain,
    root_bridge: Edge,
    incidences: &BTreeMap<usize, u8>,
) -> AnnularTargetProblemKey {
    AnnularTargetProblemKey {
        contract_version: 1,
        lower_cycle: cell.lower_cycle.clone(),
        upper_cycle: cell.upper_cycle.clone(),
        forbidden_edges: cell.forbidden_global_edges.iter().copied().collect(),
        target_incidences: incidences
            .iter()
            .map(|(&slot, &value)| (slot, value))
            .collect(),
        root_bridge: edge(root_bridge.0, root_bridge.1),
    }
}

fn interior_edges(cell: &AnnularCellDomain, triangles: &[[usize; 3]]) -> BTreeSet<Edge> {
    let boundary = cycle_edges(&cell.lower_cycle)
        .chain(cycle_edges(&cell.upper_cycle))
        .collect::<BTreeSet<_>>();
    triangles
        .iter()
        .flat_map(|triangle| {
            [
                edge(triangle[0], triangle[1]),
                edge(triangle[1], triangle[2]),
                edge(triangle[2], triangle[0]),
            ]
        })
        .filter(|candidate| !boundary.contains(candidate))
        .collect()
}

fn cycle_edges(cycle: &[usize]) -> impl Iterator<Item = Edge> + '_ {
    cycle
        .iter()
        .copied()
        .zip(cycle.iter().copied().cycle().skip(1))
        .take(cycle.len())
        .map(|(a, b)| edge(a, b))
}

fn edge(a: usize, b: usize) -> Edge {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}
