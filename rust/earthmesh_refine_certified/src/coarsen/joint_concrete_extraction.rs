//! Joint zero-ear extraction for two exact annular incidence targets.

use super::{
    build_global_incidence_contract,
    global_exact_merge::{
        final_gate_with_contracts, fixed_triangles_for_face_complex, materialize_for_face_complex,
        replace_fixed_link_contract_map,
    },
    recover_annular_target_witnesses, AnnularCellDomain, AnnularConcreteWitness,
    AnnularIncidenceTarget, AnnularTargetWitnessOutcome, AnnularTopologyKey,
    GlobalExactMergeEvidence, GlobalExactMergeTrial, GlobalIncidencePlan, GlobalIncidencePlanKey,
    HierarchyComponent, OwnedTopologyTriangle, StratifiedTransitionDomainV3, TransitionCellDomain,
    TransitionCellMergeEvidence, TransitionCellMergeTrial,
};
use crate::MotherGrid;
use std::collections::{BTreeMap, BTreeSet};

type Edge = (usize, usize);
type Triangle = [usize; 3];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct JointConcreteExtractionPlanKey {
    pub incidence_plan_key: GlobalIncidencePlanKey,
    pub final_degrees: BTreeMap<usize, u8>,
    pub cell_incidences: BTreeMap<u64, BTreeMap<usize, u8>>,
    pub lower_target: super::AnnularIncidenceTargetKey,
    pub upper_target: super::AnnularIncidenceTargetKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JointConcreteExtractionPlan {
    pub incidence_plan: GlobalIncidencePlan,
    pub lower_target: AnnularIncidenceTarget,
    pub upper_target: AnnularIncidenceTarget,
    pub plan_key: JointConcreteExtractionPlanKey,
}

impl JointConcreteExtractionPlan {
    pub fn new(
        incidence_plan: GlobalIncidencePlan,
        lower_target: AnnularIncidenceTarget,
        upper_target: AnnularIncidenceTarget,
    ) -> Self {
        let plan_key = JointConcreteExtractionPlanKey {
            incidence_plan_key: incidence_plan.plan_key.clone(),
            final_degrees: incidence_plan.final_degrees.clone(),
            cell_incidences: incidence_plan.cell_incidences.clone(),
            lower_target: lower_target.target_key.clone(),
            upper_target: upper_target.target_key.clone(),
        };
        Self {
            incidence_plan,
            lower_target,
            upper_target,
            plan_key,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JointConcreteLimits {
    pub maximum_pairs: usize,
}

impl Default for JointConcreteLimits {
    fn default() -> Self {
        Self {
            maximum_pairs: 4_096,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum JointPairConflict {
    DuplicateNonBoundaryEdge(Edge),
    DuplicateGlobalTriangle(Triangle),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct JointConcreteEvidence {
    pub lower_witnesses: usize,
    pub upper_witnesses: usize,
    pub primary_cell_id: u64,
    pub dynamic_secondary_targets: usize,
    pub dynamic_forbidden_edges: usize,
    pub candidate_pairs: usize,
    pub pairs_examined: usize,
    pub pair_conflicts: BTreeMap<JointPairConflict, usize>,
    pub final_gate_rejects: BTreeMap<String, usize>,
    pub topology_closed: bool,
    pub entered_joint_extraction: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JointConcreteCheckpoint {
    plan_key: JointConcreteExtractionPlanKey,
    candidate_keys: Vec<(AnnularTopologyKey, AnnularTopologyKey)>,
    next_pair: usize,
    evidence: JointConcreteEvidence,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JointConcreteExtractionOutcome {
    Closed {
        trial: Box<TransitionCellMergeTrial>,
        evidence: JointConcreteEvidence,
    },
    ExactNoConcretePair {
        incidence_plan: GlobalIncidencePlanKey,
        lower_targets: usize,
        upper_targets: usize,
        evidence: JointConcreteEvidence,
    },
    SearchIncomplete {
        checkpoint: Box<JointConcreteCheckpoint>,
        evidence: JointConcreteEvidence,
    },
    InvalidInput(String),
}

pub fn solve_joint_concrete_extraction(
    source: &MotherGrid,
    component: &HierarchyComponent,
    domain: &StratifiedTransitionDomainV3,
    plan: &JointConcreteExtractionPlan,
    limits: JointConcreteLimits,
    checkpoint: Option<&JointConcreteCheckpoint>,
) -> JointConcreteExtractionOutcome {
    let cells = match annular_cells(domain) {
        Ok(cells) => cells,
        Err(reason) => return JointConcreteExtractionOutcome::InvalidInput(reason),
    };
    if let Err(reason) = validate_plan(source, component, domain, &cells, plan) {
        return JointConcreteExtractionOutcome::InvalidInput(reason);
    }
    let (candidates, mut evidence) = match build_candidate_pairs(&cells, plan) {
        Ok(result) => result,
        Err(reason) => return JointConcreteExtractionOutcome::InvalidInput(reason),
    };
    let candidate_keys = candidates
        .iter()
        .map(|pair| (pair[0].topology_key.clone(), pair[1].topology_key.clone()))
        .collect::<Vec<_>>();
    let mut next_pair = 0;
    if let Some(checkpoint) = checkpoint {
        if checkpoint.plan_key != plan.plan_key || checkpoint.candidate_keys != candidate_keys {
            return JointConcreteExtractionOutcome::InvalidInput(
                "joint checkpoint identity or candidate order mismatch".into(),
            );
        }
        next_pair = checkpoint.next_pair;
        evidence = checkpoint.evidence.clone();
        if next_pair > candidates.len() {
            return JointConcreteExtractionOutcome::InvalidInput(
                "joint checkpoint pair index is out of range".into(),
            );
        }
    }
    evidence.entered_joint_extraction = true;
    let end = next_pair
        .saturating_add(limits.maximum_pairs)
        .min(candidates.len());
    while next_pair < end {
        let pair = &candidates[next_pair];
        next_pair += 1;
        evidence.pairs_examined += 1;
        let shared = shared_boundary_edges(cells[0], cells[1]);
        if let Some(conflict) = witness_pair_conflict(&pair[0], &pair[1], &shared) {
            *evidence.pair_conflicts.entry(conflict).or_default() += 1;
            continue;
        }
        match close_pair(
            source,
            component,
            domain,
            &cells,
            pair,
            evidence.pairs_examined,
        ) {
            Ok(trial) => {
                if let Err(reason) = verify_final_degrees(plan, &trial) {
                    return JointConcreteExtractionOutcome::InvalidInput(reason);
                }
                evidence.topology_closed = true;
                return JointConcreteExtractionOutcome::Closed {
                    trial: Box::new(trial),
                    evidence,
                };
            }
            Err(PairCloseError::Rejected(reason)) => {
                *evidence.final_gate_rejects.entry(reason).or_default() += 1;
            }
            Err(PairCloseError::Invalid(reason)) => {
                return JointConcreteExtractionOutcome::InvalidInput(reason)
            }
        }
    }
    if next_pair == candidates.len() {
        return JointConcreteExtractionOutcome::ExactNoConcretePair {
            incidence_plan: plan.incidence_plan.plan_key.clone(),
            lower_targets: evidence.lower_witnesses,
            upper_targets: evidence.upper_witnesses,
            evidence,
        };
    }
    let checkpoint = JointConcreteCheckpoint {
        plan_key: plan.plan_key.clone(),
        candidate_keys,
        next_pair,
        evidence: evidence.clone(),
    };
    JointConcreteExtractionOutcome::SearchIncomplete {
        checkpoint: Box::new(checkpoint),
        evidence,
    }
}

fn annular_cells(domain: &StratifiedTransitionDomainV3) -> Result<[&AnnularCellDomain; 2], String> {
    let cells = domain
        .cells
        .iter()
        .map(|cell| match cell {
            TransitionCellDomain::Annulus(cell) => Ok(cell),
            TransitionCellDomain::Disk(_) => Err("joint SDCE supports annular cells only"),
        })
        .collect::<Result<Vec<_>, _>>()?;
    cells
        .try_into()
        .map_err(|cells: Vec<_>| format!("joint SDCE requires two cells, got {}", cells.len()))
}

fn validate_plan(
    source: &MotherGrid,
    component: &HierarchyComponent,
    domain: &StratifiedTransitionDomainV3,
    cells: &[&AnnularCellDomain; 2],
    plan: &JointConcreteExtractionPlan,
) -> Result<(), String> {
    if plan.plan_key
        != JointConcreteExtractionPlan::new(
            plan.incidence_plan.clone(),
            plan.lower_target.clone(),
            plan.upper_target.clone(),
        )
        .plan_key
        || plan.lower_target.cell_id != cells[0].cell_id
        || plan.upper_target.cell_id != cells[1].cell_id
        || plan
            .incidence_plan
            .cell_incidences
            .keys()
            .copied()
            .collect::<Vec<_>>()
            != cells.iter().map(|cell| cell.cell_id).collect::<Vec<_>>()
        || plan.incidence_plan.cell_incidences[&cells[0].cell_id]
            != plan.lower_target.global_vertex_incidences
        || plan.incidence_plan.cell_incidences[&cells[1].cell_id]
            != plan.upper_target.global_vertex_incidences
    {
        return Err("joint plan identity or target incidences are inconsistent".into());
    }
    let contract = build_global_incidence_contract(source, component, domain)
        .map_err(|error| format!("joint incidence contract is invalid: {error:?}"))?;
    let target_vertices = plan
        .incidence_plan
        .cell_incidences
        .values()
        .flat_map(BTreeMap::keys)
        .copied()
        .collect::<BTreeSet<_>>();
    if plan
        .incidence_plan
        .final_degrees
        .keys()
        .copied()
        .collect::<BTreeSet<_>>()
        != target_vertices
    {
        return Err("joint incidence plan final-degree support is inconsistent".into());
    }
    for (&vertex, vertex_domain) in &contract.vertex_domains {
        let owner_counts = vertex_domain
            .owners
            .iter()
            .map(|&cell_id| {
                plan.incidence_plan.cell_incidences[&cell_id]
                    .get(&vertex)
                    .copied()
                    .map(|count| (cell_id, count))
                    .ok_or_else(|| format!("joint incidence plan omits vertex {vertex}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let final_degree = plan.incidence_plan.final_degrees[&vertex];
        if !vertex_domain
            .allowed_owner_tuples
            .iter()
            .any(|tuple| tuple.final_degree == final_degree && tuple.owner_counts == owner_counts)
        {
            return Err(format!(
                "degree contract mismatch at vertex {vertex}: plan {final_degree}, owner counts {owner_counts:?}"
            ));
        }
    }
    Ok(())
}

fn build_candidate_pairs(
    cells: &[&AnnularCellDomain; 2],
    plan: &JointConcreteExtractionPlan,
) -> Result<(Vec<[AnnularConcreteWitness; 2]>, JointConcreteEvidence), String> {
    let targets = [&plan.lower_target, &plan.upper_target];
    let mut base = Vec::new();
    for index in 0..2 {
        match recover_annular_target_witnesses(cells[index], targets[index]) {
            AnnularTargetWitnessOutcome::Found { witnesses, .. } => base.push(witnesses),
            AnnularTargetWitnessOutcome::ExactNoWitness { .. } => base.push(Vec::new()),
            AnnularTargetWitnessOutcome::InvalidInput(reason) => return Err(reason),
        }
    }
    let primary = if (base[0].len(), boundary_len(cells[0]), cells[0].cell_id)
        <= (base[1].len(), boundary_len(cells[1]), cells[1].cell_id)
    {
        0
    } else {
        1
    };
    let secondary = 1 - primary;
    let shared = shared_boundary_edges(cells[0], cells[1]);
    let mut evidence = JointConcreteEvidence {
        lower_witnesses: base[0].len(),
        upper_witnesses: base[1].len(),
        primary_cell_id: cells[primary].cell_id,
        ..JointConcreteEvidence::default()
    };
    let mut unique =
        BTreeMap::<(AnnularTopologyKey, AnnularTopologyKey), [AnnularConcreteWitness; 2]>::new();
    for primary_witness in &base[primary] {
        let dynamic_edges = primary_witness
            .interior_edges
            .difference(&shared)
            .copied()
            .collect::<BTreeSet<_>>();
        let mut dynamic_cell = cells[secondary].clone();
        dynamic_cell
            .forbidden_global_edges
            .extend(dynamic_edges.iter().copied());
        evidence.dynamic_secondary_targets += 1;
        evidence.dynamic_forbidden_edges += dynamic_edges.len();
        let dynamic_target = AnnularIncidenceTarget::new(
            &dynamic_cell,
            targets[secondary].root_bridge,
            targets[secondary].global_vertex_incidences.clone(),
        );
        let secondary_witnesses =
            match recover_annular_target_witnesses(&dynamic_cell, &dynamic_target) {
                AnnularTargetWitnessOutcome::Found { witnesses, .. } => witnesses,
                AnnularTargetWitnessOutcome::ExactNoWitness { .. } => continue,
                AnnularTargetWitnessOutcome::InvalidInput(reason) => return Err(reason),
            };
        for secondary_witness in secondary_witnesses {
            let pair = if primary == 0 {
                [primary_witness.clone(), secondary_witness]
            } else {
                [secondary_witness, primary_witness.clone()]
            };
            let key = (pair[0].topology_key.clone(), pair[1].topology_key.clone());
            unique.insert(key, pair);
        }
    }
    evidence.candidate_pairs = unique.len();
    Ok((unique.into_values().collect(), evidence))
}

fn witness_pair_conflict(
    left: &AnnularConcreteWitness,
    right: &AnnularConcreteWitness,
    shared_boundary: &BTreeSet<Edge>,
) -> Option<JointPairConflict> {
    let left_triangles = left
        .topology
        .triangles
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let right_triangles = right
        .topology
        .triangles
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if let Some(&triangle) = left_triangles.intersection(&right_triangles).next() {
        return Some(JointPairConflict::DuplicateGlobalTriangle(triangle));
    }
    duplicate_nonboundary_edge(&left.interior_edges, &right.interior_edges, shared_boundary)
        .map(JointPairConflict::DuplicateNonBoundaryEdge)
}

fn duplicate_nonboundary_edge(
    left: &BTreeSet<Edge>,
    right: &BTreeSet<Edge>,
    shared_boundary: &BTreeSet<Edge>,
) -> Option<Edge> {
    left.intersection(right)
        .find(|edge| !shared_boundary.contains(edge))
        .copied()
}

enum PairCloseError {
    Rejected(String),
    Invalid(String),
}

fn close_pair(
    source: &MotherGrid,
    component: &HierarchyComponent,
    domain: &StratifiedTransitionDomainV3,
    cells: &[&AnnularCellDomain; 2],
    pair: &[AnnularConcreteWitness; 2],
    state: usize,
) -> Result<TransitionCellMergeTrial, PairCloseError> {
    let annulus_faces = domain
        .topology_domain
        .annulus_face_slots
        .iter()
        .copied()
        .collect::<Vec<_>>();
    let fixed = fixed_triangles_for_face_complex(source, component, &annulus_faces)
        .map_err(PairCloseError::Invalid)?;
    let mut contracts = domain.link_contracts.clone();
    replace_fixed_link_contract_map(&mut contracts, &fixed);
    let custom = pair
        .iter()
        .zip(cells)
        .flat_map(|(witness, cell)| {
            witness
                .topology
                .triangles
                .iter()
                .copied()
                .map(|vertices| OwnedTopologyTriangle {
                    topology_id: state as u64,
                    sector_id: cell.cell_id,
                    vertices,
                })
        })
        .collect::<Vec<_>>();
    let mut final_triangles = fixed.clone();
    final_triangles.extend(custom.iter().map(|triangle| triangle.vertices));
    let mut global = GlobalExactMergeEvidence {
        states_examined: state,
        sector_variant_counts: vec![1, 1],
        source_vertices: source.mesh.vertex_count(),
        source_faces: source.mesh.triangle_count(),
        ..GlobalExactMergeEvidence::default()
    };
    for (witness, cell) in pair.iter().zip(cells) {
        for (&vertex, &incidence) in &witness.target.global_vertex_incidences {
            global
                .vertex_sector_contributions
                .entry(vertex)
                .or_default()
                .push((cell.cell_id, usize::from(incidence)));
        }
    }
    final_gate_with_contracts(source, &contracts, &final_triangles, &mut global)
        .map_err(PairCloseError::Rejected)?;
    let mesh = materialize_for_face_complex(source, component, &annulus_faces, &custom)
        .map_err(PairCloseError::Invalid)?;
    Ok(TransitionCellMergeTrial {
        global_trial: GlobalExactMergeTrial {
            mesh,
            custom_triangles: custom.iter().map(|triangle| triangle.vertices).collect(),
            evidence: global.clone(),
        },
        evidence: TransitionCellMergeEvidence {
            cell_family_counts: vec![1, 1],
            states_examined: state,
            topology_candidates_closed: 1,
            selected_topology_keys: Vec::new(),
            selected_annular_keys: pair
                .iter()
                .map(|witness| witness.topology_key.clone())
                .collect(),
            global,
            ..TransitionCellMergeEvidence::default()
        },
    })
}

fn verify_final_degrees(
    plan: &JointConcreteExtractionPlan,
    trial: &TransitionCellMergeTrial,
) -> Result<(), String> {
    for (&vertex, &degree) in &plan.incidence_plan.final_degrees {
        if trial.global_trial.evidence.vertex_degrees.get(&vertex) != Some(&usize::from(degree)) {
            return Err(format!(
                "closed joint topology degree at vertex {vertex} differs from its incidence plan"
            ));
        }
    }
    if !trial.global_trial.evidence.selected_ears.is_empty() {
        return Err("zero-ear joint topology unexpectedly selected post-hoc ears".into());
    }
    Ok(())
}

fn boundary_len(cell: &AnnularCellDomain) -> usize {
    cell.lower_cycle.len() + cell.upper_cycle.len()
}

fn shared_boundary_edges(left: &AnnularCellDomain, right: &AnnularCellDomain) -> BTreeSet<Edge> {
    cell_boundary_edges(left)
        .intersection(&cell_boundary_edges(right))
        .copied()
        .collect()
}

fn cell_boundary_edges(cell: &AnnularCellDomain) -> BTreeSet<Edge> {
    cycle_edges(&cell.lower_cycle)
        .chain(cycle_edges(&cell.upper_cycle))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_interface_edges_are_not_false_conflicts() {
        let shared = BTreeSet::from([(2, 3)]);
        let left = BTreeSet::from([(1, 2), (2, 3)]);
        let right = BTreeSet::from([(2, 3), (3, 4)]);
        assert_eq!(duplicate_nonboundary_edge(&left, &right, &shared), None);
    }

    #[test]
    fn duplicate_nonboundary_edge_rejects() {
        let shared = BTreeSet::from([(2, 3)]);
        let left = BTreeSet::from([(1, 4), (2, 3)]);
        let right = BTreeSet::from([(1, 4), (2, 3)]);
        assert_eq!(
            duplicate_nonboundary_edge(&left, &right, &shared),
            Some((1, 4))
        );
    }

    #[test]
    fn first_concrete_conflict_does_not_reject_signature_plan() {
        let shared = BTreeSet::from([(2, 3)]);
        let pairs = [
            (
                BTreeSet::from([(1, 4), (2, 3)]),
                BTreeSet::from([(1, 4), (2, 3)]),
            ),
            (
                BTreeSet::from([(1, 2), (2, 3)]),
                BTreeSet::from([(2, 3), (3, 4)]),
            ),
        ];
        assert_eq!(
            pairs.iter().position(
                |(left, right)| duplicate_nonboundary_edge(left, right, &shared).is_none()
            ),
            Some(1)
        );
    }
}
