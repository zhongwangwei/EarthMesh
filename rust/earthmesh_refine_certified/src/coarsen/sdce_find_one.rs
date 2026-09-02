//! Fair fixed-prefix screening for a concrete topology whose incidences satisfy SDCE.

use super::{
    build_global_incidence_contract, build_stratified_transition_domain_v3,
    certify_annular_topology, enumerate_balanced_annular_strips, find_one_essential_cycle,
    solve_joint_concrete_extraction, AnnularCellDomain, AnnularIncidenceTarget, AnnularTopology,
    AnnularTopologyKey, EssentialCycleFindOneEvidence, EssentialCycleFindOneLimits,
    EssentialCycleFindOneOutcome, EssentialCycleKey, EssentialCycleProblem, FaceBandPlan,
    FaceBandPlanEvaluator, FaceBandProblem, GlobalIncidenceContract, GlobalIncidencePlan,
    GlobalIncidencePlanKey, HierarchyComponent, JointConcreteEvidence,
    JointConcreteExtractionOutcome, JointConcreteExtractionPlan, JointConcreteLimits,
    PlanEvaluation, RingAnchorKind, StratifiedTransitionDomainV3, TransitionCellDomain,
    TransitionCellMergeTrial,
};
use crate::MotherGrid;
use std::collections::{BTreeMap, BTreeSet};

type Edge = (usize, usize);
type PairKey = (AnnularTopologyKey, AnnularTopologyKey);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SdcePlanQuantum {
    pub balanced_topologies_per_cell: usize,
    pub beam_width: usize,
    pub maximum_flip_depth: usize,
    pub maximum_joint_pairs: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdcePlanSearchStage {
    IncidenceSampling,
    TargetWitnessRecovery,
    JointConcreteExtraction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdcePlanFindOneEvidence {
    pub initial_family_counts: Vec<usize>,
    pub pairs_scored: usize,
    pub flip_depth: usize,
    pub best_incidence_distance: usize,
    pub incidence_plan_found: bool,
    pub selected_plan_key: Option<GlobalIncidencePlanKey>,
    pub selected_roots: Option<[Edge; 2]>,
    pub joint: Option<JointConcreteEvidence>,
    pub stage: SdcePlanSearchStage,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SdcePlanFindOneOutcome {
    Closed {
        trial: Box<TransitionCellMergeTrial>,
        evidence: SdcePlanFindOneEvidence,
    },
    SearchIncomplete(SdcePlanFindOneEvidence),
    InvalidInput(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SdceCycleFindOneLimits {
    pub maximum_cycle_states: u64,
    pub finalists: usize,
    pub screening_quantum: SdcePlanQuantum,
    pub finalist_quantum: SdcePlanQuantum,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SdceCycleFindOneEvidence {
    pub cycles_screened: usize,
    pub invalid_plans: usize,
    pub screening_pairs_scored: usize,
    pub finalists_selected: usize,
    pub finalists_examined: usize,
    pub finalist_best_distances: Vec<usize>,
    pub incidence_plans_found: usize,
    pub target_witness_incomplete: usize,
    pub joint_incomplete: usize,
    pub best_screening_distance: Option<usize>,
    pub closed_cycle: Option<EssentialCycleKey>,
    pub closed_plan: Option<GlobalIncidencePlanKey>,
    pub closed_flip_depth: Option<usize>,
    pub closed_initial_family_counts: Vec<usize>,
    pub closed_pairs_scored: usize,
    pub selected_roots: Option<[Edge; 2]>,
    pub joint: Option<JointConcreteEvidence>,
    pub critical_final_degrees: BTreeMap<usize, u8>,
    pub cec: Option<EssentialCycleFindOneEvidence>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SdceCycleFindOneOutcome {
    Closed {
        trial: Box<TransitionCellMergeTrial>,
        evidence: SdceCycleFindOneEvidence,
    },
    SearchIncomplete(SdceCycleFindOneEvidence),
    InvalidInput(String),
}

pub fn solve_sdce_plan_find_one(
    source: &MotherGrid,
    component: &HierarchyComponent,
    cycle_key: &EssentialCycleKey,
    face_plan: &FaceBandPlan,
    limits: SdcePlanQuantum,
) -> SdcePlanFindOneOutcome {
    if limits.balanced_topologies_per_cell == 0 || limits.beam_width == 0 {
        return SdcePlanFindOneOutcome::InvalidInput(
            "SDCE plan quantum requires nonzero topology and beam limits".into(),
        );
    }
    let domain = match build_stratified_transition_domain_v3(source, component, face_plan) {
        Ok(domain) => domain,
        Err(error) => return SdcePlanFindOneOutcome::InvalidInput(format!("{error:?}")),
    };
    let contract = match build_global_incidence_contract(source, component, &domain) {
        Ok(contract) => contract,
        Err(error) => return SdcePlanFindOneOutcome::InvalidInput(format!("{error:?}")),
    };
    let cells = match annular_cells(&domain) {
        Ok(cells) => cells,
        Err(reason) => return SdcePlanFindOneOutcome::InvalidInput(reason),
    };
    let families = match initial_families(&cells, limits.balanced_topologies_per_cell) {
        Ok(families) => families,
        Err(reason) => return SdcePlanFindOneOutcome::InvalidInput(reason),
    };
    let mut pairs_scored = 0;
    let mut beam = families[0]
        .iter()
        .flat_map(|lower| {
            families[1]
                .iter()
                .map(move |upper| [lower.clone(), upper.clone()])
        })
        .map(|pair| BeamState::new(pair, &contract, &cells, &mut pairs_scored))
        .collect::<Vec<_>>();
    if beam.is_empty() {
        return SdcePlanFindOneOutcome::InvalidInput(
            "SDCE plan quantum produced an empty seed product".into(),
        );
    }
    rank_and_truncate(&mut beam, limits.beam_width);
    let mut tested_zero_pairs = BTreeSet::new();
    let mut last_downstream = None;
    for depth in 0..=limits.maximum_flip_depth {
        let best_score = beam[0].score;
        let zero_pairs = beam
            .iter()
            .take_while(|state| state.score == 0)
            .filter(|state| !tested_zero_pairs.contains(&state.key()))
            .map(|state| state.pair.clone())
            .collect::<Vec<_>>();
        for pair in zero_pairs {
            tested_zero_pairs.insert(pair_key(&pair));
            let incidence_plan = incidence_plan_for_pair(cycle_key, &pair, &contract, &cells);
            let selected_plan_key = incidence_plan.plan_key.clone();
            let roots = [pair[0].root_bridge, pair[1].root_bridge];
            let plan = JointConcreteExtractionPlan::new(
                incidence_plan,
                AnnularIncidenceTarget::new(cells[0], roots[0], vertex_incidences(&pair[0])),
                AnnularIncidenceTarget::new(cells[1], roots[1], vertex_incidences(&pair[1])),
            );
            let outcome = solve_joint_concrete_extraction(
                source,
                component,
                &domain,
                &plan,
                JointConcreteLimits {
                    maximum_pairs: limits.maximum_joint_pairs,
                },
                None,
            );
            let evidence = |joint, stage| SdcePlanFindOneEvidence {
                initial_family_counts: families.iter().map(Vec::len).collect(),
                pairs_scored,
                flip_depth: depth,
                best_incidence_distance: 0,
                incidence_plan_found: true,
                selected_plan_key: Some(selected_plan_key.clone()),
                selected_roots: Some(roots),
                joint: Some(joint),
                stage,
            };
            match outcome {
                JointConcreteExtractionOutcome::Closed {
                    trial,
                    evidence: joint,
                } => {
                    return SdcePlanFindOneOutcome::Closed {
                        trial,
                        evidence: evidence(joint, SdcePlanSearchStage::JointConcreteExtraction),
                    }
                }
                JointConcreteExtractionOutcome::ExactNoConcretePair {
                    evidence: joint, ..
                } => {
                    let stage = if joint.candidate_pairs == 0 {
                        SdcePlanSearchStage::TargetWitnessRecovery
                    } else {
                        SdcePlanSearchStage::JointConcreteExtraction
                    };
                    last_downstream = Some(evidence(joint, stage));
                }
                JointConcreteExtractionOutcome::SearchIncomplete {
                    evidence: joint, ..
                } => {
                    last_downstream = Some(evidence(
                        joint,
                        SdcePlanSearchStage::JointConcreteExtraction,
                    ));
                }
                JointConcreteExtractionOutcome::InvalidInput(reason) => {
                    return SdcePlanFindOneOutcome::InvalidInput(reason)
                }
            }
        }
        if depth == limits.maximum_flip_depth {
            if let Some(mut evidence) = last_downstream {
                evidence.pairs_scored = pairs_scored;
                evidence.flip_depth = depth;
                return SdcePlanFindOneOutcome::SearchIncomplete(evidence);
            }
            return SdcePlanFindOneOutcome::SearchIncomplete(SdcePlanFindOneEvidence {
                initial_family_counts: families.iter().map(Vec::len).collect(),
                pairs_scored,
                flip_depth: depth,
                best_incidence_distance: best_score,
                incidence_plan_found: false,
                selected_plan_key: None,
                selected_roots: None,
                joint: None,
                stage: SdcePlanSearchStage::IncidenceSampling,
            });
        }
        let mut next = BTreeMap::<PairKey, [AnnularTopology; 2]>::new();
        for state in &beam {
            let state_key = state.key();
            if !tested_zero_pairs.contains(&state_key) {
                next.insert(state_key, state.pair.clone());
            }
            for side in 0..2 {
                for neighbor in flip_neighbors(cells[side], &state.pair[side]) {
                    let mut pair = state.pair.clone();
                    pair[side] = neighbor;
                    let key = pair_key(&pair);
                    if !tested_zero_pairs.contains(&key) {
                        next.insert(key, pair);
                    }
                }
            }
        }
        if next.is_empty() {
            let mut evidence = last_downstream.expect("only tested zero states can exhaust a beam");
            evidence.pairs_scored = pairs_scored;
            evidence.flip_depth = depth;
            return SdcePlanFindOneOutcome::SearchIncomplete(evidence);
        }
        beam = next
            .into_values()
            .map(|pair| BeamState::new(pair, &contract, &cells, &mut pairs_scored))
            .collect();
        rank_and_truncate(&mut beam, limits.beam_width);
    }
    unreachable!()
}

pub fn find_one_sdce_essential_cycle(
    source: &MotherGrid,
    component: &HierarchyComponent,
    face_problem: &FaceBandProblem,
    cycle_problem: &EssentialCycleProblem,
    limits: SdceCycleFindOneLimits,
) -> SdceCycleFindOneOutcome {
    if limits.finalists == 0 {
        return SdceCycleFindOneOutcome::InvalidInput(
            "SDCE cycle search requires at least one finalist".into(),
        );
    }
    let mut evaluator = ScreeningEvaluator {
        source,
        component,
        quantum: limits.screening_quantum,
        current_cycle: None,
        candidates: Vec::new(),
        evidence: SdceCycleFindOneEvidence::default(),
        maximum_candidates: limits.finalists,
    };
    let cec_outcome = find_one_essential_cycle(
        source,
        face_problem,
        cycle_problem,
        EssentialCycleFindOneLimits {
            maximum_unique_states: limits.maximum_cycle_states,
        },
        &mut evaluator,
    );
    let cec = match cec_outcome {
        EssentialCycleFindOneOutcome::Closed { evidence, .. }
        | EssentialCycleFindOneOutcome::CycleSearchIncomplete { evidence }
        | EssentialCycleFindOneOutcome::DownstreamSearchIncomplete { evidence } => evidence,
        EssentialCycleFindOneOutcome::InvalidInput { reason } => {
            return SdceCycleFindOneOutcome::InvalidInput(reason)
        }
    };
    evaluator.evidence.cec = Some(cec);
    evaluator.evidence.finalists_selected = evaluator.candidates.len();
    for candidate in evaluator.candidates {
        evaluator.evidence.finalists_examined += 1;
        match solve_sdce_plan_find_one(
            source,
            component,
            &candidate.cycle,
            &candidate.plan,
            limits.finalist_quantum,
        ) {
            SdcePlanFindOneOutcome::Closed { trial, evidence } => {
                evaluator.evidence.finalist_best_distances.push(0);
                evaluator.evidence.incidence_plans_found += 1;
                evaluator.evidence.closed_cycle = Some(candidate.cycle);
                evaluator.evidence.closed_plan = evidence.selected_plan_key;
                evaluator.evidence.closed_flip_depth = Some(evidence.flip_depth);
                evaluator.evidence.closed_initial_family_counts = evidence.initial_family_counts;
                evaluator.evidence.closed_pairs_scored = evidence.pairs_scored;
                evaluator.evidence.selected_roots = evidence.selected_roots;
                evaluator.evidence.joint = evidence.joint;
                evaluator.evidence.critical_final_degrees = critical_degrees(&trial);
                return SdceCycleFindOneOutcome::Closed {
                    trial,
                    evidence: evaluator.evidence,
                };
            }
            SdcePlanFindOneOutcome::SearchIncomplete(evidence) => {
                evaluator
                    .evidence
                    .finalist_best_distances
                    .push(evidence.best_incidence_distance);
                if evidence.incidence_plan_found {
                    evaluator.evidence.incidence_plans_found += 1;
                    if evidence.stage == SdcePlanSearchStage::TargetWitnessRecovery {
                        evaluator.evidence.target_witness_incomplete += 1;
                    } else {
                        evaluator.evidence.joint_incomplete += 1;
                    }
                }
            }
            SdcePlanFindOneOutcome::InvalidInput(_) => evaluator.evidence.invalid_plans += 1,
        }
    }
    SdceCycleFindOneOutcome::SearchIncomplete(evaluator.evidence)
}

fn critical_degrees(trial: &TransitionCellMergeTrial) -> BTreeMap<usize, u8> {
    [48, 52, 78, 252, 256, 343]
        .into_iter()
        .filter_map(|slot| {
            trial
                .global_trial
                .evidence
                .vertex_degrees
                .get(&slot)
                .and_then(|&degree| u8::try_from(degree).ok())
                .map(|degree| (slot, degree))
        })
        .collect()
}

struct ScreeningCandidate {
    score: usize,
    cycle: EssentialCycleKey,
    plan: FaceBandPlan,
}

struct ScreeningEvaluator<'a> {
    source: &'a MotherGrid,
    component: &'a HierarchyComponent,
    quantum: SdcePlanQuantum,
    current_cycle: Option<EssentialCycleKey>,
    candidates: Vec<ScreeningCandidate>,
    evidence: SdceCycleFindOneEvidence,
    maximum_candidates: usize,
}

impl FaceBandPlanEvaluator for ScreeningEvaluator<'_> {
    fn observe_cycle(&mut self, cycle: &EssentialCycleKey, _: &FaceBandPlan) {
        self.current_cycle = Some(cycle.clone());
    }

    fn evaluate(&mut self, plan: &FaceBandPlan) -> PlanEvaluation {
        let cycle = self
            .current_cycle
            .clone()
            .expect("cycle observed before plan");
        self.evidence.cycles_screened += 1;
        match solve_sdce_plan_find_one(self.source, self.component, &cycle, plan, self.quantum) {
            SdcePlanFindOneOutcome::Closed { evidence, .. }
            | SdcePlanFindOneOutcome::SearchIncomplete(evidence) => {
                self.evidence.screening_pairs_scored += evidence.pairs_scored;
                let score = evidence.best_incidence_distance;
                self.candidates.push(ScreeningCandidate {
                    score,
                    cycle,
                    plan: plan.clone(),
                });
                self.candidates.sort_by(|left, right| {
                    (left.score, &left.cycle).cmp(&(right.score, &right.cycle))
                });
                self.candidates.truncate(self.maximum_candidates);
                self.evidence.best_screening_distance =
                    self.candidates.first().map(|candidate| candidate.score);
            }
            SdcePlanFindOneOutcome::InvalidInput(_) => self.evidence.invalid_plans += 1,
        }
        PlanEvaluation::AuditOnly
    }
}

struct BeamState {
    pair: [AnnularTopology; 2],
    score: usize,
}

impl BeamState {
    fn new(
        pair: [AnnularTopology; 2],
        contract: &GlobalIncidenceContract,
        cells: &[&AnnularCellDomain; 2],
        pairs_scored: &mut usize,
    ) -> Self {
        *pairs_scored += 1;
        let score = pair_score(&pair, contract, cells);
        Self { pair, score }
    }

    fn key(&self) -> PairKey {
        pair_key(&self.pair)
    }
}

fn rank_and_truncate(beam: &mut Vec<BeamState>, maximum: usize) {
    beam.sort_by_key(|state| (state.score, state.key()));
    beam.truncate(maximum);
}

fn initial_families(
    cells: &[&AnnularCellDomain; 2],
    maximum: usize,
) -> Result<[Vec<AnnularTopology>; 2], String> {
    cells
        .iter()
        .map(|cell| {
            enumerate_balanced_annular_strips(
                &cell.lower_cycle,
                &cell.upper_cycle,
                &cell.forbidden_global_edges,
                maximum,
            )
            .map(|search| search.family.topologies)
            .map_err(|error| format!("{error:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|families: Vec<_>| format!("SDCE requires two families, got {}", families.len()))
}

fn annular_cells(domain: &StratifiedTransitionDomainV3) -> Result<[&AnnularCellDomain; 2], String> {
    domain
        .cells
        .iter()
        .map(|cell| match cell {
            TransitionCellDomain::Annulus(cell) => Ok(cell),
            TransitionCellDomain::Disk(_) => Err("SDCE find-one supports annular cells only"),
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|cells: Vec<_>| format!("SDCE requires two cells, got {}", cells.len()))
}

fn incidence_plan_for_pair(
    cycle_key: &EssentialCycleKey,
    pair: &[AnnularTopology; 2],
    contract: &GlobalIncidenceContract,
    cells: &[&AnnularCellDomain; 2],
) -> GlobalIncidencePlan {
    let cell_incidences = cells
        .iter()
        .enumerate()
        .map(|(side, cell)| (cell.cell_id, vertex_incidences(&pair[side])))
        .collect::<BTreeMap<_, _>>();
    let final_degrees = contract
        .vertex_domains
        .iter()
        .map(|(&vertex, domain)| {
            let tuple = domain
                .allowed_owner_tuples
                .iter()
                .find(|tuple| {
                    tuple
                        .owner_counts
                        .iter()
                        .all(|&(cell_id, count)| cell_incidences[&cell_id][&vertex] == count)
                })
                .expect("zero-score pair must satisfy every incidence tuple");
            (vertex, tuple.final_degree)
        })
        .collect::<BTreeMap<_, _>>();
    let key_material = format!("{cycle_key:?}:{cell_incidences:?}");
    let incidence_roughness_score = cells
        .iter()
        .map(|cell| {
            [&cell.lower_cycle, &cell.upper_cycle]
                .into_iter()
                .map(|cycle| {
                    cycle
                        .iter()
                        .copied()
                        .zip(cycle.iter().copied().cycle().skip(1))
                        .take(cycle.len())
                        .map(|(left, right)| {
                            (i32::from(cell_incidences[&cell.cell_id][&left])
                                - i32::from(cell_incidences[&cell.cell_id][&right]))
                            .abs()
                        })
                        .sum::<i32>()
                })
                .sum::<i32>()
        })
        .sum();
    GlobalIncidencePlan {
        cycle_key: cycle_key.clone(),
        final_degrees: final_degrees.clone(),
        cell_incidences: cell_incidences.clone(),
        ordinary_curvature_score: final_degrees
            .iter()
            .filter(|(slot, _)| {
                matches!(
                    contract.vertex_domains[slot].anchor_kind,
                    RingAnchorKind::Ordinary
                )
            })
            .map(|(_, degree)| (i32::from(*degree) - 6).pow(2))
            .sum(),
        incidence_roughness_score,
        plan_key: GlobalIncidencePlanKey(format!(
            "bounded-flip-{:016x}",
            fnv1a(key_material.bytes())
        )),
    }
}

fn pair_score(
    pair: &[AnnularTopology; 2],
    contract: &GlobalIncidenceContract,
    cells: &[&AnnularCellDomain; 2],
) -> usize {
    let counts = [vertex_incidences(&pair[0]), vertex_incidences(&pair[1])];
    contract
        .vertex_domains
        .iter()
        .map(|(&vertex, domain)| {
            domain
                .allowed_owner_tuples
                .iter()
                .map(|tuple| {
                    tuple
                        .owner_counts
                        .iter()
                        .map(|&(cell_id, expected)| {
                            let side = usize::from(cell_id != cells[0].cell_id);
                            counts[side][&vertex].abs_diff(expected) as usize
                        })
                        .sum::<usize>()
                })
                .min()
                .unwrap_or(usize::MAX)
        })
        .sum()
}

fn vertex_incidences(topology: &AnnularTopology) -> BTreeMap<usize, u8> {
    topology
        .triangles
        .iter()
        .flatten()
        .fold(BTreeMap::new(), |mut counts, &vertex| {
            *counts.entry(vertex).or_default() += 1;
            counts
        })
}

fn flip_neighbors(cell: &AnnularCellDomain, topology: &AnnularTopology) -> Vec<AnnularTopology> {
    let boundary = boundary_edges(&cell.lower_cycle)
        .chain(boundary_edges(&cell.upper_cycle))
        .collect::<BTreeSet<_>>();
    let mut incidence = BTreeMap::<Edge, Vec<usize>>::new();
    for (index, triangle) in topology.triangles.iter().enumerate() {
        for candidate in triangle_edges(*triangle) {
            incidence.entry(candidate).or_default().push(index);
        }
    }
    let mut out = BTreeMap::new();
    for (shared, owners) in incidence
        .iter()
        .filter(|(edge, owners)| owners.len() == 2 && !boundary.contains(edge))
    {
        let first = owners[0];
        let second = owners[1];
        let opposite = |triangle: [usize; 3]| {
            triangle
                .into_iter()
                .find(|vertex| ![shared.0, shared.1].contains(vertex))
                .unwrap()
        };
        let a = opposite(topology.triangles[first]);
        let b = opposite(topology.triangles[second]);
        if a == b || incidence.contains_key(&edge(a, b)) {
            continue;
        }
        let mut triangles = topology.triangles.clone();
        triangles[first] = triangle(a, b, shared.0);
        triangles[second] = triangle(a, b, shared.1);
        if let Ok(neighbor) = certify_annular_topology(
            &cell.lower_cycle,
            &cell.upper_cycle,
            &cell.forbidden_global_edges,
            &triangles,
        ) {
            out.insert(neighbor.topology_key.clone(), neighbor);
        }
    }
    out.into_values().collect()
}

fn pair_key(pair: &[AnnularTopology; 2]) -> PairKey {
    (pair[0].topology_key.clone(), pair[1].topology_key.clone())
}

fn boundary_edges(cycle: &[usize]) -> impl Iterator<Item = Edge> + '_ {
    cycle
        .iter()
        .copied()
        .zip(cycle.iter().copied().cycle().skip(1))
        .take(cycle.len())
        .map(|(a, b)| edge(a, b))
}

fn edge(a: usize, b: usize) -> Edge {
    (a.min(b), a.max(b))
}

fn triangle(a: usize, b: usize, c: usize) -> [usize; 3] {
    let mut triangle = [a, b, c];
    triangle.sort_unstable();
    triangle
}

fn triangle_edges(triangle: [usize; 3]) -> [Edge; 3] {
    [
        edge(triangle[0], triangle[1]),
        edge(triangle[1], triangle[2]),
        edge(triangle[2], triangle[0]),
    ]
}

fn fnv1a(bytes: impl Iterator<Item = u8>) -> u64 {
    bytes.fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}
