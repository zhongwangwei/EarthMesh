//! Rollback find-one search for canonical W2 essential cycles.

use super::{
    essential_cycle_seam_parity, face_band_plan_from_essential_cycle,
    solve_full_polygon_merge_from_face_bands, validate_selected_essential_cycle, AnchorBandPolicy,
    EssentialCycleKey, EssentialCycleProblem, EssentialCycleProblemKey, FaceBandPlan,
    FaceBandProblem, FullPolygonMergeEvidence, FullPolygonMergeLimits, FullPolygonMergeOutcome,
    FullPolygonMergeTrial, HierarchyComponent,
};
use crate::MotherGrid;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, VecDeque},
    time::Instant,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EdgeDecision {
    Undecided = 0,
    Excluded = 1,
    Included = 2,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackedTwoBitArray {
    words: Vec<u64>,
    len: usize,
}

impl PackedTwoBitArray {
    pub fn undecided(len: usize) -> Self {
        Self {
            words: vec![0; len.div_ceil(32)],
            len,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn get(&self, index: usize) -> Option<EdgeDecision> {
        (index < self.len).then(
            || match (self.words[index / 32] >> (2 * (index % 32))) & 3 {
                0 => EdgeDecision::Undecided,
                1 => EdgeDecision::Excluded,
                2 => EdgeDecision::Included,
                _ => unreachable!("two-bit edge state is written only through EdgeDecision"),
            },
        )
    }

    fn set(&mut self, index: usize, decision: EdgeDecision) {
        let shift = 2 * (index % 32);
        let word = &mut self.words[index / 32];
        *word = (*word & !(3 << shift)) | ((decision as u64) << shift);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EssentialCycleFindOneLimits {
    pub maximum_unique_states: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EssentialCycleFindOneOutcomeKind {
    Closed,
    CycleSearchIncomplete,
    DownstreamSearchIncomplete,
    InvalidInput,
}

impl EssentialCycleFindOneOutcomeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Closed => "Closed",
            Self::CycleSearchIncomplete => "CycleSearchIncomplete",
            Self::DownstreamSearchIncomplete => "DownstreamSearchIncomplete",
            Self::InvalidInput => "InvalidInput",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EssentialCycleFindOneEvidence {
    pub problem_key: EssentialCycleProblemKey,
    pub candidate_vertices: usize,
    pub candidate_edges: usize,
    pub raw_decisions: u64,
    pub propagation_events: u64,
    pub unique_states: u64,
    pub forced_includes: u64,
    pub forced_excludes: u64,
    pub path_connectivity_prunes: u64,
    pub dual_forced_path_prunes: u64,
    pub premature_cycle_prunes: u64,
    pub closed_cycles: u64,
    pub essential_cycles: u64,
    pub contractible_cycles: u64,
    pub downstream_exact_rejects: u64,
    pub downstream_incomplete: u64,
    pub downstream_invalid: u64,
    pub peak_trail_records: usize,
    pub peak_selected_edges: usize,
    pub elapsed_micros: u128,
    pub propagation_events_per_decision: f64,
    pub outcome: EssentialCycleFindOneOutcomeKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlanEvaluation {
    Accepted(Box<FullPolygonMergeTrial>),
    RejectedExact {
        evidence: FullPolygonMergeEvidence,
    },
    RejectedSearchIncomplete {
        evidence: FullPolygonMergeEvidence,
    },
    RejectedInvalid {
        reason: String,
        evidence: FullPolygonMergeEvidence,
    },
}

pub trait FaceBandPlanEvaluator {
    fn evaluate(&mut self, plan: &FaceBandPlan) -> PlanEvaluation;
}

pub struct FullPolygonPlanEvaluator<'a> {
    source: &'a MotherGrid,
    component: &'a HierarchyComponent,
    limits: FullPolygonMergeLimits,
}

impl<'a> FullPolygonPlanEvaluator<'a> {
    pub fn new(
        source: &'a MotherGrid,
        component: &'a HierarchyComponent,
        limits: FullPolygonMergeLimits,
    ) -> Self {
        Self {
            source,
            component,
            limits,
        }
    }
}

impl FaceBandPlanEvaluator for FullPolygonPlanEvaluator<'_> {
    fn evaluate(&mut self, plan: &FaceBandPlan) -> PlanEvaluation {
        match solve_full_polygon_merge_from_face_bands(
            self.source,
            self.component,
            plan,
            self.limits,
        ) {
            FullPolygonMergeOutcome::Closed(trial) => PlanEvaluation::Accepted(trial),
            FullPolygonMergeOutcome::TopologyFamilyExhaustedNoSolution(evidence) => {
                PlanEvaluation::RejectedExact { evidence }
            }
            FullPolygonMergeOutcome::SearchBudgetExhausted(evidence) => {
                PlanEvaluation::RejectedSearchIncomplete { evidence }
            }
            FullPolygonMergeOutcome::InvalidInput { reason, evidence } => {
                PlanEvaluation::RejectedInvalid { reason, evidence }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EssentialCycleFindOneOutcome {
    Closed {
        cycle: EssentialCycleKey,
        plan: FaceBandPlan,
        trial: Box<FullPolygonMergeTrial>,
        evidence: EssentialCycleFindOneEvidence,
    },
    CycleSearchIncomplete {
        evidence: EssentialCycleFindOneEvidence,
    },
    DownstreamSearchIncomplete {
        evidence: EssentialCycleFindOneEvidence,
    },
    InvalidInput {
        reason: String,
    },
}

impl EssentialCycleFindOneOutcome {
    pub fn evidence(&self) -> Option<&EssentialCycleFindOneEvidence> {
        match self {
            Self::Closed { evidence, .. }
            | Self::CycleSearchIncomplete { evidence }
            | Self::DownstreamSearchIncomplete { evidence } => Some(evidence),
            Self::InvalidInput { .. } => None,
        }
    }
}

pub fn find_one_essential_cycle(
    source: &MotherGrid,
    face_problem: &FaceBandProblem,
    problem: &EssentialCycleProblem,
    limits: EssentialCycleFindOneLimits,
    evaluator: &mut impl FaceBandPlanEvaluator,
) -> EssentialCycleFindOneOutcome {
    let mut search = match Search::new(source, face_problem, problem, limits, evaluator) {
        Ok(search) => search,
        Err(reason) => return EssentialCycleFindOneOutcome::InvalidInput { reason },
    };
    search.run()
}

pub fn essential_cycle_find_one_evidence_json(evidence: &EssentialCycleFindOneEvidence) -> String {
    format!(
        "{{\"schema_version\":1,\"source_n\":{},\"candidate_vertices\":{},\"candidate_edges\":{},\"raw_decisions\":{},\"propagation_events\":{},\"unique_states\":{},\"forced_includes\":{},\"forced_excludes\":{},\"path_connectivity_prunes\":{},\"dual_forced_path_prunes\":{},\"premature_cycle_prunes\":{},\"closed_cycles\":{},\"essential_cycles\":{},\"contractible_cycles\":{},\"downstream_exact_rejects\":{},\"downstream_incomplete\":{},\"downstream_invalid\":{},\"peak_trail_records\":{},\"peak_selected_edges\":{},\"elapsed_micros\":{},\"propagation_events_per_decision\":{:.12},\"outcome\":\"{}\"}}",
        evidence.problem_key.source_n,
        evidence.candidate_vertices,
        evidence.candidate_edges,
        evidence.raw_decisions,
        evidence.propagation_events,
        evidence.unique_states,
        evidence.forced_includes,
        evidence.forced_excludes,
        evidence.path_connectivity_prunes,
        evidence.dual_forced_path_prunes,
        evidence.premature_cycle_prunes,
        evidence.closed_cycles,
        evidence.essential_cycles,
        evidence.contractible_cycles,
        evidence.downstream_exact_rejects,
        evidence.downstream_incomplete,
        evidence.downstream_invalid,
        evidence.peak_trail_records,
        evidence.peak_selected_edges,
        evidence.elapsed_micros,
        evidence.propagation_events_per_decision,
        evidence.outcome.as_str(),
    )
}

struct Found {
    cycle: EssentialCycleKey,
    plan: FaceBandPlan,
    trial: Box<FullPolygonMergeTrial>,
}

struct Search<'a, E> {
    source: &'a MotherGrid,
    face_problem: &'a FaceBandProblem,
    problem: &'a EssentialCycleProblem,
    limits: EssentialCycleFindOneLimits,
    evaluator: &'a mut E,
    edge_vertices: Vec<[usize; 2]>,
    required_vertices: Vec<bool>,
    edge_potential: Vec<f64>,
    dual_adjacency: Vec<Vec<(usize, Option<usize>)>>,
    coarse_faces: Vec<usize>,
    fine_faces: BTreeSet<usize>,
    state: SearchState,
    seen: BTreeSet<PackedTwoBitArray>,
    evidence: EssentialCycleFindOneEvidence,
    started: Instant,
    downstream_unknown: bool,
    budget_hit: bool,
}

impl<'a, E: FaceBandPlanEvaluator> Search<'a, E> {
    fn new(
        source: &'a MotherGrid,
        face_problem: &'a FaceBandProblem,
        problem: &'a EssentialCycleProblem,
        limits: EssentialCycleFindOneLimits,
        evaluator: &'a mut E,
    ) -> Result<Self, String> {
        if face_problem.band_count != 2
            || source.subdivision != problem.source_n
            || problem.candidate_edges.len() != problem.edge_incident_faces.len()
            || problem.candidate_vertices.len() != problem.vertex_incident_edges.len()
            || problem.problem_key.band_count != 2
            || problem
                .vertex_incident_edges
                .iter()
                .any(|edges| edges.len() > u8::MAX as usize)
        {
            return Err("find-one solver requires one internally consistent W2 problem".into());
        }
        let vertex_index = problem
            .candidate_vertices
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, vertex)| (vertex, index))
            .collect::<BTreeMap<_, _>>();
        let edge_vertices = problem
            .candidate_edges
            .iter()
            .map(|edge| {
                Ok([
                    *vertex_index
                        .get(&edge.vertices[0])
                        .ok_or_else(|| "candidate edge endpoint is absent".to_string())?,
                    *vertex_index
                        .get(&edge.vertices[1])
                        .ok_or_else(|| "candidate edge endpoint is absent".to_string())?,
                ])
            })
            .collect::<Result<Vec<_>, String>>()?;
        for (vertex, incident) in problem.vertex_incident_edges.iter().enumerate() {
            if incident.iter().any(|edge| {
                edge_vertices
                    .get(*edge)
                    .is_none_or(|ends| !ends.contains(&vertex))
            }) {
                return Err("candidate vertex incidence is inconsistent".into());
            }
        }
        let required_vertices = problem
            .candidate_vertices
            .iter()
            .map(|vertex| {
                problem.anchor_policies.get(vertex) == Some(&AnchorBandPolicy::OnSingleInterface)
            })
            .collect::<Vec<_>>();
        let (dual_adjacency, coarse_faces, fine_faces) = dual_graph(problem)?;
        let edge_potential = edge_potentials(problem, &dual_adjacency)?;
        let state = SearchState::new(problem);
        Ok(Self {
            source,
            face_problem,
            problem,
            limits,
            evaluator,
            edge_vertices,
            required_vertices,
            edge_potential,
            dual_adjacency,
            coarse_faces,
            fine_faces,
            state,
            seen: BTreeSet::new(),
            evidence: EssentialCycleFindOneEvidence {
                problem_key: problem.problem_key.clone(),
                candidate_vertices: problem.candidate_vertices.len(),
                candidate_edges: problem.candidate_edges.len(),
                raw_decisions: 0,
                propagation_events: 0,
                unique_states: 0,
                forced_includes: 0,
                forced_excludes: 0,
                path_connectivity_prunes: 0,
                dual_forced_path_prunes: 0,
                premature_cycle_prunes: 0,
                closed_cycles: 0,
                essential_cycles: 0,
                contractible_cycles: 0,
                downstream_exact_rejects: 0,
                downstream_incomplete: 0,
                downstream_invalid: 0,
                peak_trail_records: 0,
                peak_selected_edges: 0,
                elapsed_micros: 0,
                propagation_events_per_decision: 0.0,
                outcome: EssentialCycleFindOneOutcomeKind::CycleSearchIncomplete,
            },
            started: Instant::now(),
            downstream_unknown: false,
            budget_hit: false,
        })
    }

    fn run(&mut self) -> EssentialCycleFindOneOutcome {
        let found = self.search();
        self.evidence.elapsed_micros = self.started.elapsed().as_micros();
        self.evidence.propagation_events_per_decision = if self.evidence.raw_decisions == 0 {
            0.0
        } else {
            self.evidence.propagation_events as f64 / self.evidence.raw_decisions as f64
        };
        if let Some(found) = found {
            self.evidence.outcome = EssentialCycleFindOneOutcomeKind::Closed;
            return EssentialCycleFindOneOutcome::Closed {
                cycle: found.cycle,
                plan: found.plan,
                trial: found.trial,
                evidence: self.evidence.clone(),
            };
        }
        if self.downstream_unknown {
            self.evidence.outcome = EssentialCycleFindOneOutcomeKind::DownstreamSearchIncomplete;
            EssentialCycleFindOneOutcome::DownstreamSearchIncomplete {
                evidence: self.evidence.clone(),
            }
        } else {
            self.evidence.outcome = EssentialCycleFindOneOutcomeKind::CycleSearchIncomplete;
            EssentialCycleFindOneOutcome::CycleSearchIncomplete {
                evidence: self.evidence.clone(),
            }
        }
    }

    fn search(&mut self) -> Option<Found> {
        let checkpoint = self.state.checkpoint();
        if !self.propagate() {
            self.state.rollback(checkpoint);
            return None;
        }
        if self.evidence.unique_states >= self.limits.maximum_unique_states {
            self.budget_hit = true;
            self.state.rollback(checkpoint);
            return None;
        }
        if !self.seen.insert(self.state.edge_state.clone()) {
            self.state.rollback(checkpoint);
            return None;
        }
        self.evidence.unique_states += 1;

        if self.state.dsu.has_closed_component() {
            let found = self.evaluate_closed_cycle();
            self.state.rollback(checkpoint);
            return found;
        }
        if self.has_forced_open_dual_path() {
            self.evidence.dual_forced_path_prunes += 1;
            self.state.rollback(checkpoint);
            return None;
        }
        if !self.paths_remain_connectable() {
            self.evidence.path_connectivity_prunes += 1;
            self.state.rollback(checkpoint);
            return None;
        }
        let Some(edge) = self.choose_edge() else {
            self.state.rollback(checkpoint);
            return None;
        };
        for decision in [EdgeDecision::Included, EdgeDecision::Excluded] {
            if self.budget_hit {
                break;
            }
            let branch = self.state.checkpoint();
            self.evidence.raw_decisions += 1;
            let mut queue = VecDeque::new();
            let mut queued = vec![false; self.problem.candidate_vertices.len()];
            if self.assign(edge, decision, false, &mut queue, &mut queued) {
                if let Some(found) = self.search() {
                    self.state.rollback(branch);
                    self.state.rollback(checkpoint);
                    return Some(found);
                }
            }
            self.state.rollback(branch);
        }
        self.state.rollback(checkpoint);
        None
    }

    fn propagate(&mut self) -> bool {
        let mut queue = (0..self.problem.candidate_vertices.len()).collect::<VecDeque<_>>();
        let mut queued = vec![true; self.problem.candidate_vertices.len()];
        while let Some(vertex) = queue.pop_front() {
            queued[vertex] = false;
            let rule = degree_rule(
                self.state.selected_degree[vertex],
                self.state.undecided_degree[vertex],
                self.required_vertices[vertex],
            );
            let decision = match rule {
                DegreeRule::None => continue,
                DegreeRule::Fail => return false,
                DegreeRule::IncludeAll => EdgeDecision::Included,
                DegreeRule::ExcludeAll => EdgeDecision::Excluded,
            };
            let undecided = self.problem.vertex_incident_edges[vertex]
                .iter()
                .copied()
                .filter(|edge| self.state.edge_state.get(*edge) == Some(EdgeDecision::Undecided))
                .collect::<Vec<_>>();
            for edge in undecided {
                if !self.assign(edge, decision, true, &mut queue, &mut queued) {
                    return false;
                }
            }
        }
        if has_premature_cycle(&self.state.dsu) {
            self.evidence.premature_cycle_prunes += 1;
            return false;
        }
        true
    }

    fn assign(
        &mut self,
        edge: usize,
        decision: EdgeDecision,
        forced: bool,
        queue: &mut VecDeque<usize>,
        queued: &mut [bool],
    ) -> bool {
        match self.state.edge_state.get(edge) {
            Some(current) if current == decision => return true,
            Some(EdgeDecision::Undecided) => {}
            Some(_) | None => return false,
        }
        self.state.trail.push(RollbackRecord::EdgeState {
            edge,
            old: EdgeDecision::Undecided,
        });
        self.state.edge_state.set(edge, decision);
        for &vertex in &self.edge_vertices[edge] {
            self.state.trail.push(RollbackRecord::UndecidedDegree {
                vertex,
                old: self.state.undecided_degree[vertex],
            });
            self.state.undecided_degree[vertex] -= 1;
            if decision == EdgeDecision::Included {
                self.state.trail.push(RollbackRecord::SelectedDegree {
                    vertex,
                    old: self.state.selected_degree[vertex],
                });
                self.state.selected_degree[vertex] += 1;
            }
            if !queued[vertex] {
                queued[vertex] = true;
                queue.push_back(vertex);
            }
        }
        if decision == EdgeDecision::Included {
            self.state.trail.push(RollbackRecord::SelectedEdges {
                old: self.state.selected_edges,
            });
            self.state.selected_edges += 1;
            if self.problem.dual_seam_crossing_edges.contains(edge) {
                self.state.trail.push(RollbackRecord::SeamParity {
                    old: self.state.seam_parity,
                });
                self.state.seam_parity ^= 1;
            }
            let [left, right] = self.edge_vertices[edge];
            self.state.dsu.add_edge(left, right);
        }
        if forced {
            self.evidence.propagation_events += 1;
            match decision {
                EdgeDecision::Included => self.evidence.forced_includes += 1,
                EdgeDecision::Excluded => self.evidence.forced_excludes += 1,
                EdgeDecision::Undecided => unreachable!(),
            }
        }
        self.evidence.peak_trail_records = self
            .evidence
            .peak_trail_records
            .max(self.state.trail.len() + self.state.dsu.trail.len());
        self.evidence.peak_selected_edges = self
            .evidence
            .peak_selected_edges
            .max(self.state.selected_edges);
        true
    }

    fn evaluate_closed_cycle(&mut self) -> Option<Found> {
        self.evidence.closed_cycles += 1;
        let selected = (0..self.problem.candidate_edges.len())
            .filter(|edge| self.state.edge_state.get(*edge) == Some(EdgeDecision::Included))
            .collect::<Vec<_>>();
        if self.state.seam_parity != 1
            || essential_cycle_seam_parity(self.problem, selected.iter().copied()) != 1
        {
            self.evidence.contractible_cycles += 1;
            return None;
        }
        let cycle = match validate_selected_essential_cycle(self.problem, &selected) {
            Ok(cycle) => cycle,
            Err(_) => {
                self.evidence.contractible_cycles += 1;
                return None;
            }
        };
        self.evidence.essential_cycles += 1;
        let plan = match face_band_plan_from_essential_cycle(
            self.source,
            self.face_problem,
            self.problem,
            &cycle,
        ) {
            Ok(plan) => plan,
            Err(_) => {
                self.evidence.contractible_cycles += 1;
                return None;
            }
        };
        match self.evaluator.evaluate(&plan) {
            PlanEvaluation::Accepted(trial) => Some(Found { cycle, plan, trial }),
            PlanEvaluation::RejectedExact { .. } => {
                self.evidence.downstream_exact_rejects += 1;
                None
            }
            PlanEvaluation::RejectedSearchIncomplete { .. } => {
                self.evidence.downstream_incomplete += 1;
                self.downstream_unknown = true;
                None
            }
            PlanEvaluation::RejectedInvalid { .. } => {
                self.evidence.downstream_invalid += 1;
                self.downstream_unknown = true;
                None
            }
        }
    }

    fn choose_edge(&self) -> Option<usize> {
        (0..self.problem.candidate_edges.len())
            .filter(|edge| self.state.edge_state.get(*edge) == Some(EdgeDecision::Undecided))
            .min_by(|left, right| self.compare_edges(*left, *right))
    }

    fn compare_edges(&self, left: usize, right: usize) -> Ordering {
        let endpoint = |edge: usize| {
            self.edge_vertices[edge]
                .iter()
                .any(|vertex| self.state.selected_degree[*vertex] == 1)
        };
        let required = |edge: usize| {
            self.edge_vertices[edge].iter().any(|vertex| {
                self.required_vertices[*vertex] && self.state.selected_degree[*vertex] < 2
            })
        };
        let constraint = |edge: usize| {
            self.edge_vertices[edge]
                .iter()
                .map(|vertex| self.problem.vertex_incident_edges[*vertex].len())
                .sum::<usize>()
        };
        (!endpoint(left))
            .cmp(&!endpoint(right))
            .then_with(|| (!required(left)).cmp(&!required(right)))
            .then_with(|| {
                (!self.problem.dual_seam_crossing_edges.contains(left))
                    .cmp(&!self.problem.dual_seam_crossing_edges.contains(right))
            })
            .then_with(|| self.edge_potential[left].total_cmp(&self.edge_potential[right]))
            .then_with(|| constraint(right).cmp(&constraint(left)))
            .then_with(|| {
                self.problem.candidate_edges[left].cmp(&self.problem.candidate_edges[right])
            })
    }

    fn paths_remain_connectable(&self) -> bool {
        let selected_vertices = self
            .state
            .selected_degree
            .iter()
            .enumerate()
            .filter_map(|(vertex, degree)| (*degree > 0).then_some(vertex))
            .collect::<BTreeSet<_>>();
        let Some(&start) = selected_vertices.first() else {
            return true;
        };
        let reachable =
            self.flood_candidate_vertices(start, |decision| decision != EdgeDecision::Excluded);
        if !selected_vertices.is_subset(&reachable) {
            return false;
        }
        let mut endpoints_by_root = BTreeMap::<usize, Vec<usize>>::new();
        for &vertex in &selected_vertices {
            if self.state.selected_degree[vertex] == 1 {
                endpoints_by_root
                    .entry(self.state.dsu.root(vertex))
                    .or_default()
                    .push(vertex);
            }
        }
        endpoints_by_root.values().all(|endpoints| {
            endpoints.len() == 2
                && self
                    .flood_candidate_vertices(endpoints[0], |decision| {
                        decision == EdgeDecision::Undecided
                    })
                    .contains(&endpoints[1])
        })
    }

    fn flood_candidate_vertices(
        &self,
        start: usize,
        allow: impl Fn(EdgeDecision) -> bool,
    ) -> BTreeSet<usize> {
        let mut reached = BTreeSet::from([start]);
        let mut queue = VecDeque::from([start]);
        while let Some(vertex) = queue.pop_front() {
            for &edge in &self.problem.vertex_incident_edges[vertex] {
                let decision = self.state.edge_state.get(edge).expect("validated edge");
                if !allow(decision) {
                    continue;
                }
                for &next in &self.edge_vertices[edge] {
                    if reached.insert(next) {
                        queue.push_back(next);
                    }
                }
            }
        }
        reached
    }

    fn has_forced_open_dual_path(&self) -> bool {
        let mut reached = self.coarse_faces.iter().copied().collect::<BTreeSet<_>>();
        let mut queue = self.coarse_faces.iter().copied().collect::<VecDeque<_>>();
        while let Some(face) = queue.pop_front() {
            if self.fine_faces.contains(&face) {
                return true;
            }
            for &(next, edge) in &self.dual_adjacency[face] {
                let forced_open = edge.is_none_or(|edge| {
                    self.state.edge_state.get(edge) == Some(EdgeDecision::Excluded)
                });
                if forced_open && reached.insert(next) {
                    queue.push_back(next);
                }
            }
        }
        false
    }
}

#[derive(Debug)]
struct SearchState {
    edge_state: PackedTwoBitArray,
    selected_degree: Vec<u8>,
    undecided_degree: Vec<u8>,
    selected_edges: usize,
    seam_parity: u8,
    trail: Vec<RollbackRecord>,
    dsu: RollbackPathDsu,
}

impl SearchState {
    fn new(problem: &EssentialCycleProblem) -> Self {
        Self {
            edge_state: PackedTwoBitArray::undecided(problem.candidate_edges.len()),
            selected_degree: vec![0; problem.candidate_vertices.len()],
            undecided_degree: problem
                .vertex_incident_edges
                .iter()
                .map(|edges| edges.len() as u8)
                .collect(),
            selected_edges: 0,
            seam_parity: 0,
            trail: Vec::new(),
            dsu: RollbackPathDsu::new(problem.candidate_vertices.len()),
        }
    }

    fn checkpoint(&self) -> (usize, usize) {
        (self.trail.len(), self.dsu.trail.len())
    }

    fn rollback(&mut self, checkpoint: (usize, usize)) {
        self.dsu.rollback(checkpoint.1);
        while self.trail.len() > checkpoint.0 {
            match self.trail.pop().expect("trail length checked") {
                RollbackRecord::EdgeState { edge, old } => self.edge_state.set(edge, old),
                RollbackRecord::SelectedDegree { vertex, old } => {
                    self.selected_degree[vertex] = old
                }
                RollbackRecord::UndecidedDegree { vertex, old } => {
                    self.undecided_degree[vertex] = old
                }
                RollbackRecord::SelectedEdges { old } => self.selected_edges = old,
                RollbackRecord::SeamParity { old } => self.seam_parity = old,
            }
        }
    }
}

#[derive(Debug)]
enum RollbackRecord {
    EdgeState { edge: usize, old: EdgeDecision },
    SelectedDegree { vertex: usize, old: u8 },
    UndecidedDegree { vertex: usize, old: u8 },
    SelectedEdges { old: usize },
    SeamParity { old: u8 },
}

#[derive(Debug)]
struct RollbackPathDsu {
    parent: Vec<usize>,
    size: Vec<usize>,
    active: Vec<bool>,
    vertices: Vec<usize>,
    edges: Vec<usize>,
    trail: Vec<DsuRollback>,
}

impl RollbackPathDsu {
    fn new(vertices: usize) -> Self {
        Self {
            parent: (0..vertices).collect(),
            size: vec![1; vertices],
            active: vec![false; vertices],
            vertices: vec![0; vertices],
            edges: vec![0; vertices],
            trail: Vec::new(),
        }
    }

    fn root(&self, mut vertex: usize) -> usize {
        while self.parent[vertex] != vertex {
            vertex = self.parent[vertex];
        }
        vertex
    }

    fn add_edge(&mut self, left: usize, right: usize) {
        self.activate(left);
        self.activate(right);
        let mut left_root = self.root(left);
        let mut right_root = self.root(right);
        if left_root != right_root {
            if (self.size[left_root], left_root) < (self.size[right_root], right_root) {
                std::mem::swap(&mut left_root, &mut right_root);
            }
            self.trail.push(DsuRollback::Union {
                child: right_root,
                parent: left_root,
                parent_size: self.size[left_root],
                parent_vertices: self.vertices[left_root],
                parent_edges: self.edges[left_root],
            });
            self.parent[right_root] = left_root;
            self.size[left_root] += self.size[right_root];
            self.vertices[left_root] += self.vertices[right_root];
            self.edges[left_root] += self.edges[right_root];
        }
        let root = self.root(left);
        self.trail.push(DsuRollback::EdgeCount {
            root,
            old: self.edges[root],
        });
        self.edges[root] += 1;
    }

    fn activate(&mut self, vertex: usize) {
        if self.active[vertex] {
            return;
        }
        self.trail.push(DsuRollback::Activate { vertex });
        self.active[vertex] = true;
        self.parent[vertex] = vertex;
        self.size[vertex] = 1;
        self.vertices[vertex] = 1;
        self.edges[vertex] = 0;
    }

    fn component_count(&self) -> usize {
        (0..self.parent.len())
            .filter(|vertex| self.active[*vertex] && self.parent[*vertex] == *vertex)
            .count()
    }

    fn has_closed_component(&self) -> bool {
        (0..self.parent.len()).any(|vertex| {
            self.active[vertex]
                && self.parent[vertex] == vertex
                && self.edges[vertex] == self.vertices[vertex]
        })
    }

    fn rollback(&mut self, checkpoint: usize) {
        while self.trail.len() > checkpoint {
            match self.trail.pop().expect("DSU trail length checked") {
                DsuRollback::Activate { vertex } => {
                    self.active[vertex] = false;
                    self.parent[vertex] = vertex;
                    self.size[vertex] = 1;
                    self.vertices[vertex] = 0;
                    self.edges[vertex] = 0;
                }
                DsuRollback::Union {
                    child,
                    parent,
                    parent_size,
                    parent_vertices,
                    parent_edges,
                } => {
                    self.parent[child] = child;
                    self.size[parent] = parent_size;
                    self.vertices[parent] = parent_vertices;
                    self.edges[parent] = parent_edges;
                }
                DsuRollback::EdgeCount { root, old } => self.edges[root] = old,
            }
        }
    }
}

#[derive(Debug)]
enum DsuRollback {
    Activate {
        vertex: usize,
    },
    Union {
        child: usize,
        parent: usize,
        parent_size: usize,
        parent_vertices: usize,
        parent_edges: usize,
    },
    EdgeCount {
        root: usize,
        old: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DegreeRule {
    None,
    IncludeAll,
    ExcludeAll,
    Fail,
}

fn degree_rule(selected: u8, undecided: u8, required: bool) -> DegreeRule {
    if selected > 2 || (selected == 1 && undecided == 0) {
        return DegreeRule::Fail;
    }
    if required {
        if selected + undecided < 2 {
            return DegreeRule::Fail;
        }
        if selected == 2 {
            return DegreeRule::ExcludeAll;
        }
        if selected + undecided == 2 {
            return DegreeRule::IncludeAll;
        }
    }
    match (selected, undecided) {
        (2, _) => DegreeRule::ExcludeAll,
        (1, 1) => DegreeRule::IncludeAll,
        (0, 1) => DegreeRule::ExcludeAll,
        _ => DegreeRule::None,
    }
}

fn has_premature_cycle(dsu: &RollbackPathDsu) -> bool {
    dsu.has_closed_component() && dsu.component_count() > 1
}

type DualGraph = (
    Vec<Vec<(usize, Option<usize>)>>,
    Vec<usize>,
    BTreeSet<usize>,
);

fn dual_graph(problem: &EssentialCycleProblem) -> Result<DualGraph, String> {
    let face_index = problem
        .transition_faces
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, face)| (face, index))
        .collect::<BTreeMap<_, _>>();
    let candidate_by_faces = problem
        .edge_incident_faces
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, mut faces)| {
            faces.sort_unstable();
            ((faces[0].clone(), faces[1].clone()), index)
        })
        .collect::<BTreeMap<_, _>>();
    let mut adjacency = vec![Vec::new(); problem.transition_faces.len()];
    for (left, right) in &problem.problem_key.face_adjacency_edges {
        let left_index = *face_index
            .get(left)
            .ok_or_else(|| "dual adjacency references an unknown face".to_string())?;
        let right_index = *face_index
            .get(right)
            .ok_or_else(|| "dual adjacency references an unknown face".to_string())?;
        let edge = candidate_by_faces
            .get(&(left.clone(), right.clone()))
            .copied();
        adjacency[left_index].push((right_index, edge));
        adjacency[right_index].push((left_index, edge));
    }
    for neighbours in &mut adjacency {
        neighbours.sort_unstable();
        neighbours.dedup();
    }
    let coarse = problem
        .coarse_boundary_faces
        .iter()
        .map(|face| {
            face_index
                .get(face)
                .copied()
                .ok_or_else(|| "coarse boundary face is absent from dual graph".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let fine = problem
        .fine_boundary_faces
        .iter()
        .map(|face| {
            face_index
                .get(face)
                .copied()
                .ok_or_else(|| "fine boundary face is absent from dual graph".to_string())
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    Ok((adjacency, coarse, fine))
}

fn edge_potentials(
    problem: &EssentialCycleProblem,
    adjacency: &[Vec<(usize, Option<usize>)>],
) -> Result<Vec<f64>, String> {
    let face_index = problem
        .transition_faces
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, face)| (face, index))
        .collect::<BTreeMap<_, _>>();
    let coarse = problem
        .coarse_boundary_faces
        .iter()
        .map(|face| face_index[face])
        .collect::<Vec<_>>();
    let fine = problem
        .fine_boundary_faces
        .iter()
        .map(|face| face_index[face])
        .collect::<Vec<_>>();
    let coarse_distance = dual_distances(adjacency, &coarse);
    let fine_distance = dual_distances(adjacency, &fine);
    let face_potential = (0..problem.transition_faces.len())
        .map(|face| {
            let denominator = coarse_distance[face].saturating_add(fine_distance[face]);
            if denominator == 0 {
                0.5
            } else {
                coarse_distance[face] as f64 / denominator as f64
            }
        })
        .collect::<Vec<_>>();
    problem
        .edge_incident_faces
        .iter()
        .map(|faces| {
            let left = *face_index
                .get(&faces[0])
                .ok_or_else(|| "candidate incidence references an unknown face".to_string())?;
            let right = *face_index
                .get(&faces[1])
                .ok_or_else(|| "candidate incidence references an unknown face".to_string())?;
            Ok(((face_potential[left] + face_potential[right]) * 0.5 - 0.5).abs())
        })
        .collect()
}

fn dual_distances(adjacency: &[Vec<(usize, Option<usize>)>], starts: &[usize]) -> Vec<usize> {
    let mut distance = vec![usize::MAX / 4; adjacency.len()];
    let mut queue = VecDeque::new();
    for &start in starts {
        distance[start] = 0;
        queue.push_back(start);
    }
    while let Some(face) = queue.pop_front() {
        for &(next, _) in &adjacency[face] {
            if distance[next] > distance[face] + 1 {
                distance[next] = distance[face] + 1;
                queue.push_back(next);
            }
        }
    }
    distance
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn degree_two_forces_remaining_off() {
        assert_eq!(degree_rule(2, 3, false), DegreeRule::ExcludeAll);
    }

    #[test]
    fn degree_one_single_option_forces_on() {
        assert_eq!(degree_rule(1, 1, false), DegreeRule::IncludeAll);
    }

    #[test]
    fn degree_one_no_option_fails() {
        assert_eq!(degree_rule(1, 0, false), DegreeRule::Fail);
    }

    #[test]
    fn anchor_exact_degree_two() {
        assert_eq!(degree_rule(0, 2, true), DegreeRule::IncludeAll);
        assert_eq!(degree_rule(1, 1, true), DegreeRule::IncludeAll);
        assert_eq!(degree_rule(0, 1, true), DegreeRule::Fail);
    }

    #[test]
    fn premature_cycle_with_other_component_fails() {
        let mut dsu = RollbackPathDsu::new(5);
        dsu.add_edge(0, 1);
        dsu.add_edge(1, 2);
        dsu.add_edge(2, 0);
        dsu.add_edge(3, 4);
        assert!(has_premature_cycle(&dsu));
    }
}
