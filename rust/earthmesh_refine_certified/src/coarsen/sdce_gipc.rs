//! Global Incidence Plan CSP (GIPC) for SDCE.
//!
//! GIPC chooses final degrees and per-cell triangle incidences. It never
//! materializes triangles; concrete annular extraction is a later stage.

use super::{
    EssentialCycleKey, GlobalIncidenceContract, GlobalIncidenceContractKey,
    GlobalVertexIncidenceDomain, RingAnchorKind, StratifiedTransitionDomainV3,
    TransitionCellDomain, VertexOwnerIncidenceTuple,
};
use std::{
    cmp::Reverse,
    collections::{BTreeMap, BTreeSet},
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct GlobalIncidencePlanKey(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalIncidencePlan {
    pub cycle_key: EssentialCycleKey,
    pub final_degrees: BTreeMap<usize, u8>,
    pub cell_incidences: BTreeMap<u64, BTreeMap<usize, u8>>,
    pub ordinary_curvature_score: i32,
    pub incidence_roughness_score: i32,
    pub plan_key: GlobalIncidencePlanKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncidencePlanSearchConfig {
    pub maximum_states: u64,
    pub priority_vertices: BTreeSet<usize>,
}

impl Default for IncidencePlanSearchConfig {
    fn default() -> Self {
        Self {
            maximum_states: 4_096,
            priority_vertices: BTreeSet::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncidencePlanOutcomeKind {
    Found,
    ExactNoPlan,
    SearchIncomplete,
    InvalidInput,
}

impl IncidencePlanOutcomeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Found => "Found",
            Self::ExactNoPlan => "ExactNoPlan",
            Self::SearchIncomplete => "SearchIncomplete",
            Self::InvalidInput => "InvalidInput",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncidencePlanEvidence {
    pub states: u64,
    pub incidence_sum_prunes: u64,
    pub charge_prunes: u64,
    pub maximum_frontier: usize,
    pub plans_found: usize,
    pub outcome: IncidencePlanOutcomeKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncidencePlanPartialState {
    pub tuple_indices: Vec<usize>,
    pub cell_sums: BTreeMap<u64, usize>,
    pub transition_charge: i16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncidencePlanCheckpoint {
    pub contract_key: GlobalIncidenceContractKey,
    pub cycle_key: EssentialCycleKey,
    pub variable_order: Vec<usize>,
    pub frontier: Vec<IncidencePlanPartialState>,
    pub evidence: IncidencePlanEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncidencePlanOutcome {
    Found {
        plan: GlobalIncidencePlan,
        evidence: IncidencePlanEvidence,
    },
    ExactNoPlan {
        contract_key: GlobalIncidenceContractKey,
        states: u64,
        evidence: IncidencePlanEvidence,
    },
    SearchIncomplete {
        checkpoint: IncidencePlanCheckpoint,
        evidence: IncidencePlanEvidence,
    },
    InvalidInput(String),
}

pub fn solve_global_incidence_plan(
    cycle_key: &EssentialCycleKey,
    contract: &GlobalIncidenceContract,
    domain: &StratifiedTransitionDomainV3,
    config: &IncidencePlanSearchConfig,
    checkpoint: Option<&IncidencePlanCheckpoint>,
) -> IncidencePlanOutcome {
    let cycles = match cell_boundary_cycles(domain, contract) {
        Ok(cycles) => cycles,
        Err(reason) => return IncidencePlanOutcome::InvalidInput(reason),
    };
    solve_with_cycles(
        cycle_key,
        contract,
        &cycles,
        config,
        checkpoint,
        ValueOrder::Preferred,
    )
}

fn solve_with_cycles(
    cycle_key: &EssentialCycleKey,
    contract: &GlobalIncidenceContract,
    cycles: &BTreeMap<u64, Vec<Vec<usize>>>,
    config: &IncidencePlanSearchConfig,
    checkpoint: Option<&IncidencePlanCheckpoint>,
    value_order: ValueOrder,
) -> IncidencePlanOutcome {
    let context = match SearchContext::new(contract, cycles, config) {
        Ok(context) => context,
        Err(reason) => return IncidencePlanOutcome::InvalidInput(reason),
    };
    let (mut frontier, mut evidence) = match checkpoint {
        Some(checkpoint) => match resume_state(cycle_key, contract, &context, checkpoint) {
            Ok(state) => state,
            Err(reason) => return IncidencePlanOutcome::InvalidInput(reason),
        },
        None => (
            vec![IncidencePlanPartialState {
                tuple_indices: Vec::new(),
                cell_sums: contract.cell_ids.iter().map(|&id| (id, 0)).collect(),
                transition_charge: 0,
            }],
            IncidencePlanEvidence {
                states: 0,
                incidence_sum_prunes: 0,
                charge_prunes: 0,
                maximum_frontier: 1,
                plans_found: 0,
                outcome: IncidencePlanOutcomeKind::SearchIncomplete,
            },
        ),
    };
    let starting_states = evidence.states;
    while evidence.states - starting_states < config.maximum_states {
        let Some(state) = frontier.pop() else {
            evidence.outcome = IncidencePlanOutcomeKind::ExactNoPlan;
            return IncidencePlanOutcome::ExactNoPlan {
                contract_key: contract.contract_key.clone(),
                states: evidence.states,
                evidence,
            };
        };
        evidence.states += 1;
        let depth = state.tuple_indices.len();
        if depth == context.variable_order.len() {
            if exact_targets_hold(&state, contract) {
                let plan = build_plan(cycle_key, contract, cycles, &context, &state);
                evidence.plans_found += 1;
                evidence.outcome = IncidencePlanOutcomeKind::Found;
                return IncidencePlanOutcome::Found { plan, evidence };
            }
            continue;
        }
        if !bounds_hold(&state, depth, contract, &context, &mut evidence) {
            continue;
        }
        let source_slot = context.variable_order[depth];
        let vertex = &contract.vertex_domains[&source_slot];
        let mut tuple_indices = (0..vertex.allowed_owner_tuples.len()).collect::<Vec<_>>();
        sort_values(
            &mut tuple_indices,
            source_slot,
            vertex,
            &state,
            contract,
            &context,
            value_order,
        );
        for tuple_index in tuple_indices.into_iter().rev() {
            let tuple = &vertex.allowed_owner_tuples[tuple_index];
            let Some(child) = extend_state(&state, tuple_index, tuple) else {
                return IncidencePlanOutcome::InvalidInput(
                    "incidence-plan state arithmetic overflow".into(),
                );
            };
            if bounds_hold(&child, depth + 1, contract, &context, &mut evidence) {
                frontier.push(child);
            }
        }
        evidence.maximum_frontier = evidence.maximum_frontier.max(frontier.len());
    }
    if frontier.is_empty() {
        evidence.outcome = IncidencePlanOutcomeKind::ExactNoPlan;
        return IncidencePlanOutcome::ExactNoPlan {
            contract_key: contract.contract_key.clone(),
            states: evidence.states,
            evidence,
        };
    }
    evidence.outcome = IncidencePlanOutcomeKind::SearchIncomplete;
    let checkpoint = IncidencePlanCheckpoint {
        contract_key: contract.contract_key.clone(),
        cycle_key: cycle_key.clone(),
        variable_order: context.variable_order.clone(),
        frontier,
        evidence: evidence.clone(),
    };
    IncidencePlanOutcome::SearchIncomplete {
        checkpoint,
        evidence,
    }
}

struct SearchContext {
    variable_order: Vec<usize>,
    suffix_cell_min: Vec<BTreeMap<u64, usize>>,
    suffix_cell_max: Vec<BTreeMap<u64, usize>>,
    suffix_charge_min: Vec<i16>,
    suffix_charge_max: Vec<i16>,
    neighbours: BTreeMap<(u64, usize), BTreeSet<usize>>,
}

impl SearchContext {
    fn new(
        contract: &GlobalIncidenceContract,
        cycles: &BTreeMap<u64, Vec<Vec<usize>>>,
        config: &IncidencePlanSearchConfig,
    ) -> Result<Self, String> {
        validate_contract(contract, cycles)?;
        let mut variable_order = contract.vertex_domains.keys().copied().collect::<Vec<_>>();
        variable_order.sort_by_key(|slot| {
            let domain = &contract.vertex_domains[slot];
            (
                domain.allowed_owner_tuples.len(),
                !config.priority_vertices.contains(slot),
                !matches!(
                    domain.anchor_kind,
                    RingAnchorKind::IcosahedronPentagon { .. }
                ),
                Reverse(domain.owners.len()),
                Reverse(domain.fixed_degree),
                *slot,
            )
        });
        let mut suffix_cell_min = vec![BTreeMap::new(); variable_order.len() + 1];
        let mut suffix_cell_max = vec![BTreeMap::new(); variable_order.len() + 1];
        let mut suffix_charge_min = vec![0i16; variable_order.len() + 1];
        let mut suffix_charge_max = vec![0i16; variable_order.len() + 1];
        for depth in (0..variable_order.len()).rev() {
            suffix_cell_min[depth] = suffix_cell_min[depth + 1].clone();
            suffix_cell_max[depth] = suffix_cell_max[depth + 1].clone();
            suffix_charge_min[depth] = suffix_charge_min[depth + 1];
            suffix_charge_max[depth] = suffix_charge_max[depth + 1];
            let vertex = &contract.vertex_domains[&variable_order[depth]];
            for &cell in &vertex.owners {
                let counts = vertex.allowed_owner_tuples.iter().map(|tuple| {
                    tuple
                        .owner_counts
                        .iter()
                        .find_map(|&(id, count)| (id == cell).then_some(usize::from(count)))
                        .expect("validated owner tuple contains every owner")
                });
                let min = counts.clone().min().unwrap();
                let max = counts.max().unwrap();
                *suffix_cell_min[depth].entry(cell).or_default() += min;
                *suffix_cell_max[depth].entry(cell).or_default() += max;
            }
            let charges = vertex
                .allowed_owner_tuples
                .iter()
                .map(|tuple| 6 - i16::from(tuple.final_degree));
            suffix_charge_min[depth] = suffix_charge_min[depth]
                .checked_add(charges.clone().min().unwrap())
                .ok_or_else(|| "charge lower bound overflow".to_string())?;
            suffix_charge_max[depth] = suffix_charge_max[depth]
                .checked_add(charges.max().unwrap())
                .ok_or_else(|| "charge upper bound overflow".to_string())?;
        }
        Ok(Self {
            variable_order,
            suffix_cell_min,
            suffix_cell_max,
            suffix_charge_min,
            suffix_charge_max,
            neighbours: cycle_neighbours(cycles),
        })
    }
}

fn validate_contract(
    contract: &GlobalIncidenceContract,
    cycles: &BTreeMap<u64, Vec<Vec<usize>>>,
) -> Result<(), String> {
    if contract.cell_ids.iter().collect::<BTreeSet<_>>().len() != contract.cell_ids.len() {
        return Err("incidence contract contains duplicate cell ids".into());
    }
    if contract.cell_ids.iter().copied().collect::<BTreeSet<_>>()
        != cycles.keys().copied().collect()
    {
        return Err("incidence contract and boundary cycles have different cell ids".into());
    }
    if contract
        .cell_incidence_sums
        .keys()
        .copied()
        .collect::<Vec<_>>()
        != contract.cell_ids
        || contract
            .cell_triangle_counts
            .keys()
            .copied()
            .collect::<Vec<_>>()
            != contract.cell_ids
    {
        return Err("incidence contract cell totals have different ids".into());
    }
    for (&cell, boundaries) in cycles {
        for cycle in boundaries {
            if cycle.is_empty()
                || cycle.iter().copied().collect::<BTreeSet<_>>().len() != cycle.len()
            {
                return Err(format!("cell {cell} has an invalid boundary cycle"));
            }
            for slot in cycle {
                if !contract
                    .vertex_domains
                    .get(slot)
                    .is_some_and(|domain| domain.owners.contains(&cell))
                {
                    return Err(format!(
                        "cell {cell} boundary vertex {slot} is absent from its incidence domains"
                    ));
                }
            }
        }
    }
    for (&slot, domain) in &contract.vertex_domains {
        if domain.source_slot != slot || domain.allowed_owner_tuples.is_empty() {
            return Err(format!("vertex {slot} has an invalid incidence domain"));
        }
        for tuple in &domain.allowed_owner_tuples {
            if !domain.legal_final_degrees.contains(&tuple.final_degree)
                || tuple.owner_counts.iter().any(|(_, count)| *count == 0)
                || tuple
                    .owner_counts
                    .iter()
                    .map(|(id, _)| *id)
                    .collect::<Vec<_>>()
                    != domain.owners
                || u16::from(domain.fixed_degree)
                    + tuple
                        .owner_counts
                        .iter()
                        .map(|(_, count)| u16::from(*count))
                        .sum::<u16>()
                    != u16::from(tuple.final_degree)
            {
                return Err(format!("vertex {slot} has an invalid owner tuple"));
            }
        }
    }
    Ok(())
}

fn cell_boundary_cycles(
    domain: &StratifiedTransitionDomainV3,
    contract: &GlobalIncidenceContract,
) -> Result<BTreeMap<u64, Vec<Vec<usize>>>, String> {
    let mut cycles = BTreeMap::new();
    for cell in &domain.cells {
        let TransitionCellDomain::Annulus(cell) = cell else {
            return Err("GIPC supports annular cells only".into());
        };
        if cycles
            .insert(
                cell.cell_id,
                vec![cell.lower_cycle.clone(), cell.upper_cycle.clone()],
            )
            .is_some()
        {
            return Err(format!("duplicate annular cell {}", cell.cell_id));
        }
    }
    if cycles.keys().copied().collect::<Vec<_>>() != contract.cell_ids {
        return Err("GIPC domain cell order differs from its contract".into());
    }
    Ok(cycles)
}

fn cycle_neighbours(
    cycles: &BTreeMap<u64, Vec<Vec<usize>>>,
) -> BTreeMap<(u64, usize), BTreeSet<usize>> {
    let mut out = BTreeMap::<(u64, usize), BTreeSet<usize>>::new();
    for (&cell, boundaries) in cycles {
        for cycle in boundaries {
            for index in 0..cycle.len() {
                let slot = cycle[index];
                out.entry((cell, slot)).or_default().extend([
                    cycle[(index + cycle.len() - 1) % cycle.len()],
                    cycle[(index + 1) % cycle.len()],
                ]);
            }
        }
    }
    out
}

fn resume_state(
    cycle_key: &EssentialCycleKey,
    contract: &GlobalIncidenceContract,
    context: &SearchContext,
    checkpoint: &IncidencePlanCheckpoint,
) -> Result<(Vec<IncidencePlanPartialState>, IncidencePlanEvidence), String> {
    if checkpoint.contract_key != contract.contract_key
        || checkpoint.cycle_key != *cycle_key
        || checkpoint.variable_order != context.variable_order
        || checkpoint.evidence.outcome != IncidencePlanOutcomeKind::SearchIncomplete
    {
        return Err("GIPC checkpoint identity or ordering mismatch".into());
    }
    for state in &checkpoint.frontier {
        validate_partial_state(state, contract, context)?;
    }
    Ok((checkpoint.frontier.clone(), checkpoint.evidence.clone()))
}

fn validate_partial_state(
    state: &IncidencePlanPartialState,
    contract: &GlobalIncidenceContract,
    context: &SearchContext,
) -> Result<(), String> {
    if state.tuple_indices.len() > context.variable_order.len()
        || state.cell_sums.keys().copied().collect::<Vec<_>>() != contract.cell_ids
    {
        return Err("invalid GIPC checkpoint state shape".into());
    }
    let mut rebuilt = IncidencePlanPartialState {
        tuple_indices: Vec::new(),
        cell_sums: contract.cell_ids.iter().map(|&id| (id, 0)).collect(),
        transition_charge: 0,
    };
    for (depth, &tuple_index) in state.tuple_indices.iter().enumerate() {
        let vertex = &contract.vertex_domains[&context.variable_order[depth]];
        let tuple = vertex
            .allowed_owner_tuples
            .get(tuple_index)
            .ok_or_else(|| "checkpoint tuple index is out of range".to_string())?;
        rebuilt = extend_state(&rebuilt, tuple_index, tuple)
            .ok_or_else(|| "checkpoint arithmetic overflow".to_string())?;
    }
    if rebuilt.cell_sums != state.cell_sums || rebuilt.transition_charge != state.transition_charge
    {
        return Err("checkpoint partial sums do not match its tuple prefix".into());
    }
    Ok(())
}

fn extend_state(
    state: &IncidencePlanPartialState,
    tuple_index: usize,
    tuple: &VertexOwnerIncidenceTuple,
) -> Option<IncidencePlanPartialState> {
    let mut child = state.clone();
    child.tuple_indices.push(tuple_index);
    for &(cell, count) in &tuple.owner_counts {
        let sum = child.cell_sums.get_mut(&cell)?;
        *sum = sum.checked_add(usize::from(count))?;
    }
    child.transition_charge = child
        .transition_charge
        .checked_add(6 - i16::from(tuple.final_degree))?;
    Some(child)
}

fn bounds_hold(
    state: &IncidencePlanPartialState,
    depth: usize,
    contract: &GlobalIncidenceContract,
    context: &SearchContext,
    evidence: &mut IncidencePlanEvidence,
) -> bool {
    for &cell in &contract.cell_ids {
        let assigned = state.cell_sums[&cell];
        let minimum = assigned
            + context.suffix_cell_min[depth]
                .get(&cell)
                .copied()
                .unwrap_or(0);
        let maximum = assigned
            + context.suffix_cell_max[depth]
                .get(&cell)
                .copied()
                .unwrap_or(0);
        let target = contract.cell_incidence_sums[&cell];
        if target < minimum || target > maximum {
            evidence.incidence_sum_prunes += 1;
            return false;
        }
    }
    let minimum = state.transition_charge + context.suffix_charge_min[depth];
    let maximum = state.transition_charge + context.suffix_charge_max[depth];
    if contract.target_transition_charge < minimum || contract.target_transition_charge > maximum {
        evidence.charge_prunes += 1;
        return false;
    }
    true
}

fn exact_targets_hold(
    state: &IncidencePlanPartialState,
    contract: &GlobalIncidenceContract,
) -> bool {
    state.transition_charge == contract.target_transition_charge
        && contract
            .cell_ids
            .iter()
            .all(|cell| state.cell_sums[cell] == contract.cell_incidence_sums[cell])
}

#[derive(Debug, Clone, Copy)]
enum ValueOrder {
    Preferred,
    #[cfg(test)]
    Canonical,
}

fn sort_values(
    tuple_indices: &mut [usize],
    source_slot: usize,
    vertex: &GlobalVertexIncidenceDomain,
    state: &IncidencePlanPartialState,
    contract: &GlobalIncidenceContract,
    context: &SearchContext,
    order: ValueOrder,
) {
    match order {
        ValueOrder::Preferred => tuple_indices.sort_by_key(|&index| {
            let tuple = &vertex.allowed_owner_tuples[index];
            (
                usize::from(
                    matches!(vertex.anchor_kind, RingAnchorKind::Ordinary)
                        && tuple.final_degree != 6,
                ),
                (contract.target_transition_charge
                    - (state.transition_charge + 6 - i16::from(tuple.final_degree)))
                .abs(),
                local_roughness(source_slot, tuple, state, contract, context),
                tuple.final_degree,
                tuple.owner_counts.clone(),
            )
        }),
        #[cfg(test)]
        ValueOrder::Canonical => tuple_indices.sort_by_key(|&index| {
            let tuple = &vertex.allowed_owner_tuples[index];
            (tuple.final_degree, tuple.owner_counts.clone())
        }),
    }
}

fn local_roughness(
    source_slot: usize,
    tuple: &VertexOwnerIncidenceTuple,
    state: &IncidencePlanPartialState,
    contract: &GlobalIncidenceContract,
    context: &SearchContext,
) -> i32 {
    let selected = selected_tuples(state, contract, context);
    let mut score = 0;
    for &(cell, count) in &tuple.owner_counts {
        let Some(neighbours) = context.neighbours.get(&(cell, source_slot)) else {
            continue;
        };
        for neighbour in neighbours {
            let Some(selected) = selected.get(neighbour) else {
                continue;
            };
            let Some(other) = selected
                .owner_counts
                .iter()
                .find_map(|&(id, value)| (id == cell).then_some(value))
            else {
                continue;
            };
            score += (i32::from(count) - i32::from(other)).abs();
        }
    }
    score
}

fn selected_tuples<'a>(
    state: &IncidencePlanPartialState,
    contract: &'a GlobalIncidenceContract,
    context: &SearchContext,
) -> BTreeMap<usize, &'a VertexOwnerIncidenceTuple> {
    state
        .tuple_indices
        .iter()
        .enumerate()
        .map(|(depth, &index)| {
            let slot = context.variable_order[depth];
            (
                slot,
                &contract.vertex_domains[&slot].allowed_owner_tuples[index],
            )
        })
        .collect()
}

fn build_plan(
    cycle_key: &EssentialCycleKey,
    contract: &GlobalIncidenceContract,
    cycles: &BTreeMap<u64, Vec<Vec<usize>>>,
    context: &SearchContext,
    state: &IncidencePlanPartialState,
) -> GlobalIncidencePlan {
    let selected = selected_tuples(state, contract, context);
    let final_degrees = selected
        .iter()
        .map(|(&slot, tuple)| (slot, tuple.final_degree))
        .collect::<BTreeMap<_, _>>();
    let mut cell_incidences = contract
        .cell_ids
        .iter()
        .map(|&cell| (cell, BTreeMap::new()))
        .collect::<BTreeMap<_, _>>();
    for (&slot, tuple) in &selected {
        for &(cell, count) in &tuple.owner_counts {
            cell_incidences.get_mut(&cell).unwrap().insert(slot, count);
        }
    }
    let ordinary_curvature_score = final_degrees
        .iter()
        .filter(|(slot, _)| {
            matches!(
                contract.vertex_domains[slot].anchor_kind,
                RingAnchorKind::Ordinary
            )
        })
        .map(|(_, &degree)| (i32::from(degree) - 6).pow(2))
        .sum();
    let mut incidence_roughness_score = 0;
    for (&cell, boundaries) in cycles {
        for cycle in boundaries {
            for (a, b) in cycle
                .iter()
                .copied()
                .zip(cycle.iter().copied().cycle().skip(1))
                .take(cycle.len())
            {
                incidence_roughness_score += (i32::from(cell_incidences[&cell][&a])
                    - i32::from(cell_incidences[&cell][&b]))
                .abs();
            }
        }
    }
    let plan_key = GlobalIncidencePlanKey(format!(
        "{:016x}",
        fnv1a(
            format!(
                "{:?}|{}|{:?}|{:?}",
                cycle_key.ordered_vertices, contract.contract_key.0, final_degrees, cell_incidences
            )
            .bytes()
        )
    ));
    GlobalIncidencePlan {
        cycle_key: cycle_key.clone(),
        final_degrees,
        cell_incidences,
        ordinary_curvature_score,
        incidence_roughness_score,
        plan_key,
    }
}

fn fnv1a(bytes: impl Iterator<Item = u8>) -> u64 {
    bytes.fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coarsen::{
        FixedFinalTopologyContext, FixedFinalTopologyContextKey, GlobalIncidenceContractKey,
    };

    #[test]
    fn sum_bounds_prune_soundly() {
        let contract = contract(
            [(1, domain(1, &[(5, &[1])])), (2, domain(2, &[(5, &[1])]))],
            3,
            2,
        );
        let outcome = solve(&contract, 100, ValueOrder::Preferred);
        let IncidencePlanOutcome::ExactNoPlan { evidence, .. } = outcome else {
            panic!("infeasible cell sum must exhaust")
        };
        assert!(evidence.incidence_sum_prunes > 0);
    }

    #[test]
    fn charge_bounds_prune_soundly() {
        let contract = contract(
            [(1, domain(1, &[(5, &[1])])), (2, domain(2, &[(5, &[1])]))],
            2,
            0,
        );
        let outcome = solve(&contract, 100, ValueOrder::Preferred);
        let IncidencePlanOutcome::ExactNoPlan { evidence, .. } = outcome else {
            panic!("infeasible charge must exhaust")
        };
        assert!(evidence.charge_prunes > 0);
    }

    #[test]
    fn checkpoint_resume_equals_one_shot() {
        let contract = contract(
            [
                (1, domain(1, &[(5, &[1]), (6, &[2])])),
                (2, domain(2, &[(5, &[1]), (6, &[2])])),
            ],
            4,
            0,
        );
        let one = solve(&contract, 100, ValueOrder::Preferred);
        let IncidencePlanOutcome::Found { plan: one, .. } = one else {
            panic!("one-shot must find a plan")
        };
        let first = solve(&contract, 1, ValueOrder::Preferred);
        let IncidencePlanOutcome::SearchIncomplete { checkpoint, .. } = first else {
            panic!("one state must checkpoint")
        };
        let resumed = solve_with_cycles(
            &cycle_key(),
            &contract,
            &cycles(&contract),
            &IncidencePlanSearchConfig {
                maximum_states: 100,
                priority_vertices: BTreeSet::new(),
            },
            Some(&checkpoint),
            ValueOrder::Preferred,
        );
        let IncidencePlanOutcome::Found { plan, .. } = resumed else {
            panic!("resumed search must find a plan")
        };
        assert_eq!(plan, one);
    }

    #[test]
    fn value_order_does_not_change_plan_family() {
        let contract = contract(
            [
                (1, domain(1, &[(5, &[1]), (6, &[2])])),
                (2, domain(2, &[(5, &[1]), (6, &[2])])),
            ],
            3,
            1,
        );
        assert_eq!(
            all_plan_keys(&contract, ValueOrder::Preferred),
            all_plan_keys(&contract, ValueOrder::Canonical)
        );
    }

    #[test]
    fn no_plan_is_scoped_to_zero_ear_cycle_family() {
        let contract = contract([(1, domain(1, &[(5, &[1])]))], 2, 1);
        let IncidencePlanOutcome::ExactNoPlan { contract_key, .. } =
            solve(&contract, 100, ValueOrder::Preferred)
        else {
            panic!("infeasible zero-ear contract must exhaust")
        };
        assert_eq!(contract_key, contract.contract_key);
    }

    #[test]
    fn target_budget_is_incomplete() {
        let contract = contract([(1, domain(1, &[(5, &[1])]))], 1, 1);
        let IncidencePlanOutcome::SearchIncomplete { checkpoint, .. } =
            solve(&contract, 0, ValueOrder::Preferred)
        else {
            panic!("zero states must checkpoint")
        };
        assert_eq!(checkpoint.frontier.len(), 1);
    }

    fn solve(
        contract: &GlobalIncidenceContract,
        maximum_states: u64,
        order: ValueOrder,
    ) -> IncidencePlanOutcome {
        solve_with_cycles(
            &cycle_key(),
            contract,
            &cycles(contract),
            &IncidencePlanSearchConfig {
                maximum_states,
                priority_vertices: BTreeSet::new(),
            },
            None,
            order,
        )
    }

    fn all_plan_keys(
        contract: &GlobalIncidenceContract,
        order: ValueOrder,
    ) -> BTreeSet<GlobalIncidencePlanKey> {
        let context = SearchContext::new(
            contract,
            &cycles(contract),
            &IncidencePlanSearchConfig::default(),
        )
        .unwrap();
        let mut frontier = vec![IncidencePlanPartialState {
            tuple_indices: Vec::new(),
            cell_sums: BTreeMap::from([(1, 0)]),
            transition_charge: 0,
        }];
        let mut keys = BTreeSet::new();
        let mut evidence = IncidencePlanEvidence {
            states: 0,
            incidence_sum_prunes: 0,
            charge_prunes: 0,
            maximum_frontier: 1,
            plans_found: 0,
            outcome: IncidencePlanOutcomeKind::SearchIncomplete,
        };
        while let Some(state) = frontier.pop() {
            let depth = state.tuple_indices.len();
            if depth == context.variable_order.len() {
                if exact_targets_hold(&state, contract) {
                    keys.insert(
                        build_plan(&cycle_key(), contract, &cycles(contract), &context, &state)
                            .plan_key,
                    );
                }
                continue;
            }
            let slot = context.variable_order[depth];
            let vertex = &contract.vertex_domains[&slot];
            let mut values = (0..vertex.allowed_owner_tuples.len()).collect::<Vec<_>>();
            sort_values(&mut values, slot, vertex, &state, contract, &context, order);
            for index in values {
                let child =
                    extend_state(&state, index, &vertex.allowed_owner_tuples[index]).unwrap();
                if bounds_hold(&child, depth + 1, contract, &context, &mut evidence) {
                    frontier.push(child);
                }
            }
        }
        keys
    }

    fn contract<const N: usize>(
        domains: [(usize, GlobalVertexIncidenceDomain); N],
        cell_sum: usize,
        charge: i16,
    ) -> GlobalIncidenceContract {
        GlobalIncidenceContract {
            cell_ids: vec![1],
            fixed: FixedFinalTopologyContext {
                triangles: Vec::new(),
                vertex_degrees: BTreeMap::new(),
                vertex_link_edges: BTreeMap::new(),
                edge_counts: BTreeMap::new(),
                context_key: FixedFinalTopologyContextKey("test".into()),
            },
            vertex_domains: domains.into_iter().collect(),
            cell_triangle_counts: BTreeMap::from([(1, cell_sum / 3)]),
            cell_incidence_sums: BTreeMap::from([(1, cell_sum)]),
            target_transition_charge: charge,
            contract_key: GlobalIncidenceContractKey(format!("test-{cell_sum}-{charge}")),
        }
    }

    fn domain(source_slot: usize, values: &[(u8, &[u8])]) -> GlobalVertexIncidenceDomain {
        GlobalVertexIncidenceDomain {
            source_slot,
            owners: vec![1],
            fixed_degree: 4,
            fixed_link: super::super::LinkPathSignature::Empty,
            legal_final_degrees: values.iter().map(|(degree, _)| *degree).collect(),
            allowed_owner_tuples: values
                .iter()
                .map(|(degree, counts)| VertexOwnerIncidenceTuple {
                    final_degree: *degree,
                    owner_counts: vec![(1, counts[0])],
                })
                .collect(),
            anchor_kind: RingAnchorKind::Ordinary,
        }
    }

    fn cycles(contract: &GlobalIncidenceContract) -> BTreeMap<u64, Vec<Vec<usize>>> {
        BTreeMap::from([(1, vec![contract.vertex_domains.keys().copied().collect()])])
    }

    fn cycle_key() -> EssentialCycleKey {
        EssentialCycleKey {
            ordered_vertices: Vec::new(),
        }
    }
}
