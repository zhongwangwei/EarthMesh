//! Exact 2:1 coarse/fine interface closure over hierarchy addresses.
//!
//! Core parent edges stay coarse. Only adjacent fine parent patches are
//! retriangulated, using their source slots and a finite non-crossing polygon
//! state space; geometry relocation belongs to the later elastic stage.

use super::{HierarchyComponent, HierarchyLeafMesh, HierarchyLeafSet};
use crate::mother_grid::{MotherGrid, TriangleAddress, VertexAddress};
use earthmesh_mesh::{orientation_on_sphere, MeshState, Sign};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransitionTopologyLimits {
    pub topology_states: usize,
    pub maximum_halo_expansions: usize,
}

impl TransitionTopologyLimits {
    pub fn solve_from_cursor(
        self,
        source: &MotherGrid,
        component: &HierarchyComponent,
        topology_states_cursor: usize,
    ) -> TransitionTopologyOutcome {
        solve_transition_topology_from_cursor(source, component, self, topology_states_cursor)
    }

    pub(super) fn solve_from_cursor_with_promotion(
        self,
        source: &MotherGrid,
        component: &HierarchyComponent,
        topology_states_cursor: usize,
        preferred_core_promotion: Option<(TriangleAddress, usize)>,
    ) -> TransitionTopologyOutcome {
        solve_transition_topology_from_cursor_with_promotion(
            source,
            component,
            self,
            topology_states_cursor,
            preferred_core_promotion,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TransitionBoundary {
    pub fine_outer_cycles: Vec<Vec<usize>>,
    pub coarse_inner_cycles: Vec<Vec<usize>>,
    pub halo_parents: Vec<TriangleAddress>,
    pub seam: Vec<usize>,
    pub pentagon: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionTopologyCandidate {
    pub component_id: u64,
    pub topology_id: usize,
    pub core_parents: Vec<TriangleAddress>,
    pub custom_transition_triangles: BTreeMap<TriangleAddress, Vec<[usize; 3]>>,
    pub source_triangles: Vec<[usize; 3]>,
    pub source_active_vertices: Vec<usize>,
    pub source_degree_forecast: BTreeMap<usize, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TransitionTopologyReport {
    pub component_id: u64,
    pub core_parent_count: usize,
    pub transition_parent_count: usize,
    pub halo_expansions: usize,
    pub topology_states: usize,
    pub layout_topology_states: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransitionTopologyTrial {
    pub mesh: HierarchyLeafMesh,
    pub boundary: TransitionBoundary,
    pub candidate: TransitionTopologyCandidate,
    pub report: TransitionTopologyReport,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TransitionTopologyOutcome {
    Closed(Box<TransitionTopologyTrial>),
    RequiresWiderHalo {
        states_examined: usize,
        halo_expansions: usize,
    },
    ProvenInfeasible {
        states_examined: usize,
        halo_expansions: usize,
        reason: String,
    },
    SearchBudgetExhausted {
        states_examined: usize,
        halo_expansions: usize,
    },
    InvalidBoundary {
        states_examined: usize,
        halo_expansions: usize,
        reason: String,
    },
}

#[derive(Debug, Clone)]
struct ParentPatch {
    corners: [usize; 3],
    midpoints: [usize; 3],
    neighbours: [TriangleAddress; 3],
    child_triangles: [[usize; 3]; 4],
}

pub fn solve_transition_topology(
    source: &MotherGrid,
    component: &HierarchyComponent,
    limits: TransitionTopologyLimits,
) -> TransitionTopologyOutcome {
    solve_transition_topology_from_cursor(source, component, limits, 0)
}

pub fn solve_transition_topology_from_cursor(
    source: &MotherGrid,
    component: &HierarchyComponent,
    limits: TransitionTopologyLimits,
    topology_states_cursor: usize,
) -> TransitionTopologyOutcome {
    solve_transition_topology_from_cursor_with_promotion(
        source,
        component,
        limits,
        topology_states_cursor,
        None,
    )
}

fn solve_transition_topology_from_cursor_with_promotion(
    source: &MotherGrid,
    component: &HierarchyComponent,
    limits: TransitionTopologyLimits,
    topology_states_cursor: usize,
    mut preferred_core_promotion: Option<(TriangleAddress, usize)>,
) -> TransitionTopologyOutcome {
    let mut core = set(component.core_parents.iter().copied());
    let mut transition = set(component.transition_parents.iter().copied());
    if let Err(reason) = preflight(source, component, &core, &transition) {
        return TransitionTopologyOutcome::InvalidBoundary {
            states_examined: 0,
            halo_expansions: 0,
            reason,
        };
    }
    let mut halo_expansions = 0usize;
    let mut states_examined = 0usize;

    loop {
        if core.is_empty() {
            return TransitionTopologyOutcome::ProvenInfeasible {
                states_examined,
                halo_expansions,
                reason: "halo expansion leaves no coarse core".into(),
            };
        }

        let uncovered = core
            .iter()
            .copied()
            .filter(|&parent| {
                parent_patch(source, parent).is_ok_and(|patch| {
                    patch.neighbours.iter().any(|neighbour| {
                        !core.contains(neighbour) && !transition.contains(neighbour)
                    })
                })
            })
            .collect::<BTreeSet<_>>();
        if uncovered.is_empty() && transition.is_empty() {
            return pure_core(source, component.id, &core, halo_expansions);
        }
        if !uncovered.is_empty() {
            if uncovered.len() == core.len() {
                return TransitionTopologyOutcome::RequiresWiderHalo {
                    states_examined,
                    halo_expansions,
                };
            }
            if halo_expansions == limits.maximum_halo_expansions {
                return TransitionTopologyOutcome::RequiresWiderHalo {
                    states_examined,
                    halo_expansions,
                };
            }
            promote_to_transition(&mut core, &mut transition, uncovered);
            halo_expansions += 1;
            continue;
        }

        if topology_states_cursor >= limits.topology_states {
            return TransitionTopologyOutcome::SearchBudgetExhausted {
                states_examined: limits.topology_states,
                halo_expansions,
            };
        }
        if states_examined == limits.topology_states {
            return TransitionTopologyOutcome::SearchBudgetExhausted {
                states_examined,
                halo_expansions,
            };
        }

        let remaining_states = limits.topology_states - states_examined;
        let remaining_halos = limits.maximum_halo_expansions - halo_expansions + 1;
        let local_limit = remaining_states.div_ceil(remaining_halos);
        let local_cursor = topology_states_cursor.saturating_sub(states_examined);
        match solve_once(
            source,
            component.id,
            core.clone(),
            transition.clone(),
            halo_expansions,
            local_cursor,
            local_limit,
        ) {
            TransitionTopologyOutcome::Closed(mut trial) => {
                let layout_topology_states = trial.report.topology_states;
                trial.candidate.topology_id += states_examined;
                states_examined += trial.report.topology_states;
                trial.report.topology_states = states_examined;
                trial.report.layout_topology_states = layout_topology_states;
                trial.report.halo_expansions = halo_expansions;
                return TransitionTopologyOutcome::Closed(trial);
            }
            TransitionTopologyOutcome::SearchBudgetExhausted {
                states_examined: local,
                ..
            } => {
                states_examined += local;
                if states_examined == limits.topology_states {
                    return TransitionTopologyOutcome::SearchBudgetExhausted {
                        states_examined,
                        halo_expansions,
                    };
                }
                let Some(expansion_cost) = promote_core_boundary(
                    source,
                    &mut core,
                    &mut transition,
                    preferred_core_promotion.take(),
                    limits.maximum_halo_expansions - halo_expansions,
                ) else {
                    return TransitionTopologyOutcome::SearchBudgetExhausted {
                        states_examined,
                        halo_expansions,
                    };
                };
                halo_expansions += expansion_cost;
            }
            TransitionTopologyOutcome::InvalidBoundary { reason, .. } => {
                return invalid(states_examined, halo_expansions, reason);
            }
            TransitionTopologyOutcome::ProvenInfeasible {
                states_examined: local,
                reason,
                ..
            } => {
                states_examined += local;
                if states_examined == limits.topology_states {
                    return TransitionTopologyOutcome::SearchBudgetExhausted {
                        states_examined,
                        halo_expansions,
                    };
                }
                let peel = core_boundary(source, &core);
                if peel.is_empty() || peel.len() == core.len() {
                    return TransitionTopologyOutcome::ProvenInfeasible {
                        states_examined,
                        halo_expansions,
                        reason,
                    };
                }
                let Some(expansion_cost) = promote_core_boundary(
                    source,
                    &mut core,
                    &mut transition,
                    preferred_core_promotion.take(),
                    limits.maximum_halo_expansions - halo_expansions,
                ) else {
                    return TransitionTopologyOutcome::RequiresWiderHalo {
                        states_examined,
                        halo_expansions,
                    };
                };
                halo_expansions += expansion_cost;
            }
            TransitionTopologyOutcome::RequiresWiderHalo { .. } => unreachable!(),
        }
    }
}

fn preflight(
    source: &MotherGrid,
    component: &HierarchyComponent,
    core: &BTreeSet<TriangleAddress>,
    transition: &BTreeSet<TriangleAddress>,
) -> Result<(), String> {
    if source.subdivision < 2 || !source.subdivision.is_multiple_of(2) {
        return Err("transition topology requires an even source subdivision >= 2".into());
    }
    let expected_n = source.subdivision / 2;
    let parents = set(component.parents.iter().copied());
    if parents.is_empty() {
        return Err("transition component has no parents".into());
    }
    if parents.len() != component.parents.len()
        || core.len() != component.core_parents.len()
        || transition.len() != component.transition_parents.len()
    {
        return Err("transition component contains duplicate parents".into());
    }
    if !core.is_disjoint(transition) {
        return Err("component core and transition parents overlap".into());
    }
    let union = core.union(transition).copied().collect::<BTreeSet<_>>();
    if union != parents {
        return Err("component parents must equal core union transition parents".into());
    }
    for &parent in &parents {
        if parent.n != expected_n {
            return Err(format!(
                "component parent {:?} is not at expected coarse subdivision {expected_n}",
                parent
            ));
        }
        parent_patch(source, parent)?;
    }
    let seed = *parents.first().expect("non-empty component");
    let mut seen = BTreeSet::from([seed]);
    let mut stack = vec![seed];
    while let Some(parent) = stack.pop() {
        for neighbour in parent_patch(source, parent)?.neighbours {
            if parents.contains(&neighbour) && seen.insert(neighbour) {
                stack.push(neighbour);
            }
        }
    }
    if seen != parents {
        return Err("transition component parents are disconnected".into());
    }
    Ok(())
}

fn promote_to_transition(
    core: &mut BTreeSet<TriangleAddress>,
    transition: &mut BTreeSet<TriangleAddress>,
    promoted: BTreeSet<TriangleAddress>,
) {
    for parent in promoted {
        core.remove(&parent);
        transition.insert(parent);
    }
}

fn promote_core_boundary(
    source: &MotherGrid,
    core: &mut BTreeSet<TriangleAddress>,
    transition: &mut BTreeSet<TriangleAddress>,
    preferred: Option<(TriangleAddress, usize)>,
    remaining_halo_expansions: usize,
) -> Option<usize> {
    let peel = core_boundary(source, core);
    if peel.is_empty() || peel.len() == core.len() {
        return None;
    }
    let (promoted, expansion_cost) = match preferred
        .filter(|(parent, cost)| peel.contains(parent) && *cost <= remaining_halo_expansions)
    {
        Some((parent, cost)) => (
            preferred_boundary_segment(source, &peel, transition, parent)?,
            cost,
        ),
        None => (peel, 1),
    };
    if expansion_cost > remaining_halo_expansions {
        return None;
    }
    promote_to_transition(core, transition, promoted);
    Some(expansion_cost)
}

fn preferred_boundary_segment(
    source: &MotherGrid,
    peel: &BTreeSet<TriangleAddress>,
    transition: &BTreeSet<TriangleAddress>,
    preferred: TriangleAddress,
) -> Option<BTreeSet<TriangleAddress>> {
    let anchors = parent_patch(source, preferred)
        .ok()?
        .neighbours
        .into_iter()
        .filter(|parent| transition.contains(parent))
        .collect::<BTreeSet<_>>();
    let mut segment = BTreeSet::from([preferred]);
    if anchors.is_empty() {
        return Some(segment);
    }
    for &parent in peel {
        if parent_patch(source, parent)
            .ok()?
            .neighbours
            .iter()
            .any(|neighbour| anchors.contains(neighbour))
        {
            segment.insert(parent);
        }
    }
    Some(segment)
}

fn core_boundary(
    source: &MotherGrid,
    core: &BTreeSet<TriangleAddress>,
) -> BTreeSet<TriangleAddress> {
    core.iter()
        .copied()
        .filter(|&parent| {
            parent_patch(source, parent)
                .is_ok_and(|patch| patch.neighbours.iter().any(|p| !core.contains(p)))
        })
        .collect()
}

fn pure_core(
    source: &MotherGrid,
    component_id: u64,
    core: &BTreeSet<TriangleAddress>,
    halo_expansions: usize,
) -> TransitionTopologyOutcome {
    let mut leaf_set = match HierarchyLeafSet::from_mother_grid(source) {
        Ok(v) => v,
        Err(reason) => return invalid(0, halo_expansions, reason),
    };
    if let Err(reason) = leaf_set.condense_core(&core.iter().copied().collect::<Vec<_>>()) {
        return invalid(0, halo_expansions, reason);
    }
    let mesh = match super::core_condensation::rebuild_from_leaf_set(source, &leaf_set) {
        Ok(mesh) => mesh,
        Err(reason) => return invalid(0, halo_expansions, reason),
    };
    if let Err(reason) = hard_gate(&mesh.mesh) {
        return TransitionTopologyOutcome::ProvenInfeasible {
            states_examined: 0,
            halo_expansions,
            reason,
        };
    }
    let boundary = match boundary(source, core, &BTreeSet::new()) {
        Ok(boundary) => boundary,
        Err(reason) => return invalid(0, halo_expansions, reason),
    };
    TransitionTopologyOutcome::Closed(Box::new(TransitionTopologyTrial {
        mesh,
        boundary,
        candidate: TransitionTopologyCandidate {
            component_id,
            topology_id: 0,
            core_parents: core.iter().copied().collect(),
            custom_transition_triangles: BTreeMap::new(),
            source_triangles: Vec::new(),
            source_active_vertices: Vec::new(),
            source_degree_forecast: BTreeMap::new(),
        },
        report: TransitionTopologyReport {
            component_id,
            core_parent_count: core.len(),
            transition_parent_count: 0,
            halo_expansions,
            topology_states: 0,
            layout_topology_states: 0,
        },
    }))
}

fn solve_once(
    source: &MotherGrid,
    component_id: u64,
    core: BTreeSet<TriangleAddress>,
    transition: BTreeSet<TriangleAddress>,
    halo_expansions: usize,
    start_index: usize,
    budget: usize,
) -> TransitionTopologyOutcome {
    let mut states = 0usize;
    let mut leaf_set = match HierarchyLeafSet::from_mother_grid(source) {
        Ok(v) => v,
        Err(reason) => return invalid(states, halo_expansions, reason),
    };
    if let Err(reason) = leaf_set.condense_core(&core.iter().copied().collect::<Vec<_>>()) {
        return invalid(states, halo_expansions, reason);
    }
    let mut patches = BTreeMap::<TriangleAddress, ParentPatch>::new();
    for &parent in core.iter().chain(&transition) {
        let patch = match parent_patch(source, parent) {
            Ok(patch) => patch,
            Err(reason) => return invalid(states, halo_expansions, reason),
        };
        patches.insert(parent, patch);
    }
    let custom_transition = transition
        .iter()
        .copied()
        .filter(|parent| {
            patches[parent]
                .neighbours
                .iter()
                .any(|neighbour| core.contains(neighbour))
        })
        .collect::<BTreeSet<_>>();

    for parent in &custom_transition {
        let Some(children) = parent.children_2_to_1() else {
            return invalid(
                states,
                halo_expansions,
                format!("invalid transition parent {parent:?}"),
            );
        };
        for child in children {
            leaf_set.leaves.remove(&child);
        }
    }

    let mut variants = Vec::<Vec<Vec<[usize; 3]>>>::new();
    for parent in &custom_transition {
        let patch = &patches[parent];
        let polygon = transition_polygon(patch, &core);
        if !(3..=5).contains(&polygon.len()) {
            return invalid(
                states,
                halo_expansions,
                format!(
                    "transition parent {:?} produced {} boundary vertices",
                    parent,
                    polygon.len()
                ),
            );
        }
        let variants_for_parent = ranked_triangulations(source, &polygon, &patch.child_triangles);
        if variants_for_parent.is_empty() {
            return TransitionTopologyOutcome::ProvenInfeasible {
                states_examined: states,
                halo_expansions,
                reason: format!("transition parent {parent:?} has no positive topology candidate"),
            };
        }
        variants.push(variants_for_parent);
    }

    let boundary = match boundary(source, &core, &transition) {
        Ok(boundary) => boundary,
        Err(reason) => return invalid(states, halo_expansions, reason),
    };
    let forecast = match base_degree_forecast(source, &core, &custom_transition, &patches) {
        Ok(forecast) => forecast,
        Err(reason) => return invalid(states, halo_expansions, reason),
    };
    let mut closed = None;
    let mut enumeration_exhausted = false;
    ProductSearch {
        source,
        leaf_set: &leaf_set,
        transition: &custom_transition,
        variants: &variants,
        start_index,
        budget,
        states: &mut states,
        forecast: &forecast,
        closed: &mut closed,
        enumeration_exhausted: &mut enumeration_exhausted,
    }
    .run();
    let Some(hit) = closed else {
        if enumeration_exhausted {
            return TransitionTopologyOutcome::ProvenInfeasible {
                states_examined: states,
                halo_expansions,
                reason: "no transition triangulation passed hard topology gates".into(),
            };
        }
        return TransitionTopologyOutcome::SearchBudgetExhausted {
            states_examined: states,
            halo_expansions,
        };
    };

    let candidate_triangles = hit.triangles.clone();
    let active_vertices = hit
        .triangles
        .iter()
        .flat_map(|tri| tri.iter().copied())
        .collect::<BTreeSet<_>>();
    TransitionTopologyOutcome::Closed(Box::new(TransitionTopologyTrial {
        mesh: hit.mesh,
        boundary,
        candidate: TransitionTopologyCandidate {
            component_id,
            topology_id: hit.topology_id,
            core_parents: core.iter().copied().collect(),
            custom_transition_triangles: hit.triangles_by_parent,
            source_triangles: candidate_triangles,
            source_active_vertices: active_vertices.into_iter().collect(),
            source_degree_forecast: hit.degree_forecast,
        },
        report: TransitionTopologyReport {
            component_id,
            core_parent_count: core.len(),
            transition_parent_count: transition.len(),
            halo_expansions,
            topology_states: states,
            layout_topology_states: states,
        },
    }))
}

struct ProductSearch<'a> {
    source: &'a MotherGrid,
    leaf_set: &'a HierarchyLeafSet,
    transition: &'a BTreeSet<TriangleAddress>,
    variants: &'a [Vec<Vec<[usize; 3]>>],
    start_index: usize,
    budget: usize,
    states: &'a mut usize,
    forecast: &'a BTreeMap<usize, isize>,
    closed: &'a mut Option<SearchHit>,
    enumeration_exhausted: &'a mut bool,
}

struct SearchHit {
    mesh: HierarchyLeafMesh,
    triangles_by_parent: BTreeMap<TriangleAddress, Vec<[usize; 3]>>,
    triangles: Vec<[usize; 3]>,
    degree_forecast: BTreeMap<usize, usize>,
    topology_id: usize,
}

struct SearchVariable {
    original_position: usize,
    variants: Vec<VariantChoice>,
    touched: Vec<usize>,
}

struct VariantChoice {
    variant_index: usize,
    delta: Vec<(usize, isize)>,
}

impl ProductSearch<'_> {
    fn run(&mut self) {
        if self.start_index >= self.budget {
            *self.states = self.budget;
            return;
        }
        *self.states = self.start_index;

        let mut forecast = DenseForecast::new(self.source.mesh.vertices().len(), self.forecast);
        let mut chosen = vec![None; self.variants.len()];
        let transition = self.transition.iter().copied().collect::<Vec<_>>();
        let (variables, preassigned_touched) =
            search_variables(self.variants, &transition, &mut chosen);
        for position in chosen
            .iter()
            .enumerate()
            .filter_map(|(position, chosen)| chosen.map(|_| position))
        {
            forecast.apply_triangles(&self.variants[position][0], 1);
        }
        let suffix_masks = suffix_degree_masks(&variables);
        if !forecast.can_finish_all(&preassigned_touched, &suffix_masks[0]) {
            *self.enumeration_exhausted = true;
            *self.states = 0;
            return;
        }

        let mut indices = vec![0usize; variables.len()];
        let mut position = 0usize;
        let mut feasible_ordinal = 0usize;

        loop {
            if position == variables.len() {
                let touched = touched_vertices(&variables, &preassigned_touched);
                if forecast.can_finish_all(&touched, &[]) {
                    if feasible_ordinal >= self.budget {
                        *self.states = self.budget;
                        return;
                    }
                    if feasible_ordinal >= self.start_index {
                        *self.states = feasible_ordinal.saturating_add(1);
                        let chosen_by_parent = self.chosen_by_parent(&chosen);
                        let chosen_triangles = flatten_custom_triangles(&chosen_by_parent);
                        if let Ok(mesh) =
                            super::core_condensation::rebuild_from_leaf_set_with_custom_triangles(
                                self.source,
                                self.leaf_set,
                                self.transition,
                                &chosen_triangles,
                            )
                        {
                            if let Ok(()) = hard_gate(&mesh.mesh) {
                                *self.closed = Some(SearchHit {
                                    mesh,
                                    triangles_by_parent: chosen_by_parent,
                                    triangles: chosen_triangles,
                                    degree_forecast: forecast.to_map(self.forecast),
                                    topology_id: feasible_ordinal,
                                });
                                return;
                            }
                        }
                    }
                    feasible_ordinal = feasible_ordinal.saturating_add(1);
                }
                if !backtrack(&mut position, &mut forecast, &mut chosen, &variables) {
                    *self.states = feasible_ordinal;
                    *self.enumeration_exhausted = true;
                    return;
                }
                continue;
            }

            if indices[position] == variables[position].variants.len() {
                indices[position] = 0;
                if !backtrack(&mut position, &mut forecast, &mut chosen, &variables) {
                    *self.states = feasible_ordinal;
                    *self.enumeration_exhausted = true;
                    return;
                }
                continue;
            }

            let choice_index = indices[position];
            indices[position] += 1;
            let variable = &variables[position];
            let choice = &variable.variants[choice_index];
            forecast.apply_delta(&choice.delta, 1);
            if forecast.can_finish_all(&variable.touched, &suffix_masks[position + 1]) {
                chosen[variable.original_position] = Some(choice.variant_index);
                position += 1;
                if position < indices.len() {
                    indices[position] = 0;
                }
            } else {
                forecast.apply_delta(&choice.delta, -1);
            }
        }
    }

    fn chosen_by_parent(
        &self,
        chosen: &[Option<usize>],
    ) -> BTreeMap<TriangleAddress, Vec<[usize; 3]>> {
        self.transition
            .iter()
            .zip(self.variants)
            .zip(chosen.iter().copied())
            .map(|((parent, parent_variants), variant_index)| {
                (
                    *parent,
                    parent_variants[variant_index.expect("complete candidate")].clone(),
                )
            })
            .collect()
    }
}

struct DenseForecast {
    degrees: Vec<isize>,
}

impl DenseForecast {
    fn new(vertex_count: usize, forecast: &BTreeMap<usize, isize>) -> Self {
        let mut degrees = vec![0; vertex_count];
        for (&vertex, &degree) in forecast {
            degrees[vertex] = degree;
        }
        Self { degrees }
    }

    fn apply_triangles(&mut self, triangles: &[[usize; 3]], sign: isize) {
        for vertex in triangles
            .iter()
            .flat_map(|triangle| triangle.iter().copied())
        {
            self.degrees[vertex] += sign;
        }
    }

    fn apply_delta(&mut self, delta: &[(usize, isize)], sign: isize) {
        for &(vertex, count) in delta {
            self.degrees[vertex] += sign * count;
        }
    }

    fn can_finish_all(&self, vertices: &[usize], suffix_masks: &[(usize, u128)]) -> bool {
        vertices.iter().copied().all(|vertex| {
            let mask = suffix_masks
                .binary_search_by_key(&vertex, |&(candidate, _)| candidate)
                .map(|index| suffix_masks[index].1)
                .unwrap_or(1);
            degree_mask_can_finish(self.degrees[vertex], mask)
        })
    }

    fn to_map(&self, keys: &BTreeMap<usize, isize>) -> BTreeMap<usize, usize> {
        keys.iter()
            .filter_map(|(&site, _)| {
                usize::try_from(self.degrees[site])
                    .ok()
                    .map(|degree| (site, degree))
            })
            .collect()
    }
}

fn backtrack(
    position: &mut usize,
    forecast: &mut DenseForecast,
    chosen: &mut [Option<usize>],
    variables: &[SearchVariable],
) -> bool {
    if *position == 0 {
        return false;
    }
    *position -= 1;
    let variable = &variables[*position];
    let variant_index = chosen[variable.original_position]
        .take()
        .expect("only entered positions can be backtracked");
    let choice = variable
        .variants
        .iter()
        .find(|choice| choice.variant_index == variant_index)
        .expect("chosen variant belongs to the current search variable");
    forecast.apply_delta(&choice.delta, -1);
    true
}

fn search_variables(
    variants: &[Vec<Vec<[usize; 3]>>],
    parents: &[TriangleAddress],
    chosen: &mut [Option<usize>],
) -> (Vec<SearchVariable>, Vec<usize>) {
    let mut fixed_touched = BTreeSet::new();
    let mut pending = Vec::new();
    for (position, parent_variants) in variants.iter().enumerate() {
        if parent_variants.len() == 1 {
            chosen[position] = Some(0);
            fixed_touched.extend(triangle_vertices(&parent_variants[0]));
        } else {
            pending.push(SearchVariable {
                original_position: position,
                variants: parent_variants
                    .iter()
                    .enumerate()
                    .map(|(variant_index, variant)| VariantChoice {
                        variant_index,
                        delta: triangle_delta(variant),
                    })
                    .collect(),
                touched: parent_variants
                    .iter()
                    .flat_map(|variant| triangle_vertices(variant))
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect(),
            });
        }
    }
    let mut ordered = Vec::with_capacity(pending.len());
    let mut frontier = fixed_touched.clone();
    while !pending.is_empty() {
        let best = pending
            .iter()
            .enumerate()
            .max_by(|(_, left), (_, right)| {
                let left_shared = shared_count(&left.touched, &frontier);
                let right_shared = shared_count(&right.touched, &frontier);
                left_shared
                    .cmp(&right_shared)
                    .then_with(|| left.touched.len().cmp(&right.touched.len()))
                    .then_with(|| {
                        parents[right.original_position].cmp(&parents[left.original_position])
                    })
            })
            .map(|(index, _)| index)
            .unwrap();
        let variable = pending.remove(best);
        frontier.extend(variable.touched.iter().copied());
        ordered.push(variable);
    }
    (ordered, fixed_touched.into_iter().collect())
}

fn shared_count(vertices: &[usize], frontier: &BTreeSet<usize>) -> usize {
    vertices
        .iter()
        .filter(|vertex| frontier.contains(vertex))
        .count()
}

fn touched_vertices(variables: &[SearchVariable], fixed: &[usize]) -> Vec<usize> {
    fixed
        .iter()
        .copied()
        .chain(
            variables
                .iter()
                .flat_map(|variable| variable.touched.iter().copied()),
        )
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn suffix_degree_masks(variables: &[SearchVariable]) -> Vec<Vec<(usize, u128)>> {
    let mut suffix = vec![Vec::new(); variables.len() + 1];
    for position in (0..variables.len()).rev() {
        suffix[position] = combine_suffix_masks(
            &local_degree_masks(&variables[position]),
            &suffix[position + 1],
        );
    }
    suffix
}

fn local_degree_masks(variable: &SearchVariable) -> Vec<(usize, u128)> {
    variable
        .touched
        .iter()
        .copied()
        .map(|vertex| {
            let mask = variable.variants.iter().fold(0u128, |mask, choice| {
                let count = choice
                    .delta
                    .binary_search_by_key(&vertex, |&(candidate, _)| candidate)
                    .map(|index| choice.delta[index].1 as usize)
                    .unwrap_or(0);
                mask | (1u128 << count)
            });
            (vertex, mask)
        })
        .collect()
}

fn combine_suffix_masks(left: &[(usize, u128)], right: &[(usize, u128)]) -> Vec<(usize, u128)> {
    let mut out = Vec::new();
    let mut left_index = 0;
    let mut right_index = 0;
    while left_index < left.len() || right_index < right.len() {
        let vertex = match (left.get(left_index), right.get(right_index)) {
            (Some((left, _)), Some((right, _))) => (*left).min(*right),
            (Some((left, _)), None) => *left,
            (None, Some((right, _))) => *right,
            (None, None) => unreachable!(),
        };
        let left_mask = if left.get(left_index).is_some_and(|&(v, _)| v == vertex) {
            let mask = left[left_index].1;
            left_index += 1;
            mask
        } else {
            1
        };
        let right_mask = if right.get(right_index).is_some_and(|&(v, _)| v == vertex) {
            let mask = right[right_index].1;
            right_index += 1;
            mask
        } else {
            1
        };
        let mask = convolve_degree_masks(left_mask, right_mask);
        if mask != 1 {
            out.push((vertex, mask));
        }
    }
    out
}

fn convolve_degree_masks(left: u128, right: u128) -> u128 {
    let mut out = 0u128;
    let mut left_bits = left;
    while left_bits != 0 {
        let l = left_bits.trailing_zeros();
        left_bits &= left_bits - 1;
        let mut right_bits = right;
        while right_bits != 0 {
            let r = right_bits.trailing_zeros();
            right_bits &= right_bits - 1;
            let sum = l + r;
            if sum < u128::BITS {
                out |= 1u128 << sum;
            }
        }
    }
    out
}

fn degree_mask_can_finish(degree: isize, mask: u128) -> bool {
    let mut bits = mask;
    while bits != 0 {
        let add = bits.trailing_zeros() as isize;
        bits &= bits - 1;
        let final_degree = degree + add;
        if final_degree == 0 || (5..=7).contains(&final_degree) {
            return true;
        }
    }
    false
}

fn triangle_vertices(triangles: &[[usize; 3]]) -> BTreeSet<usize> {
    triangles
        .iter()
        .flat_map(|triangle| triangle.iter().copied())
        .collect()
}

fn triangle_delta(triangles: &[[usize; 3]]) -> Vec<(usize, isize)> {
    let mut counts = BTreeMap::new();
    for vertex in triangles
        .iter()
        .flat_map(|triangle| triangle.iter().copied())
    {
        *counts.entry(vertex).or_default() += 1;
    }
    counts.into_iter().collect()
}

fn flatten_custom_triangles(
    triangles_by_parent: &BTreeMap<TriangleAddress, Vec<[usize; 3]>>,
) -> Vec<[usize; 3]> {
    triangles_by_parent
        .values()
        .flat_map(|triangles| triangles.iter().copied())
        .collect()
}

#[cfg(test)]
fn advance_mixed_radix(indices: &mut [usize], variants: &[Vec<Vec<[usize; 3]>>]) -> bool {
    for position in (0..indices.len()).rev() {
        indices[position] += 1;
        if indices[position] < variants[position].len() {
            return true;
        }
        indices[position] = 0;
    }
    false
}

fn invalid(states: usize, halo_expansions: usize, reason: String) -> TransitionTopologyOutcome {
    TransitionTopologyOutcome::InvalidBoundary {
        states_examined: states,
        halo_expansions,
        reason,
    }
}

fn set(values: impl Iterator<Item = TriangleAddress>) -> BTreeSet<TriangleAddress> {
    values.collect()
}

fn source_face_slot(source: &MotherGrid, address: TriangleAddress) -> Result<usize, String> {
    super::core_condensation::source_face_slot(source, address)
}

fn parent_patch(source: &MotherGrid, parent: TriangleAddress) -> Result<ParentPatch, String> {
    let children = parent
        .children_2_to_1()
        .ok_or_else(|| format!("invalid hierarchy parent {parent:?}"))?;
    let mut child_triangles = [[0usize; 3]; 4];
    for (index, child) in children.into_iter().enumerate() {
        child_triangles[index] = source.mesh.triangles()[source_face_slot(source, child)?];
    }
    let corners = match parent.orientation {
        crate::mother_grid::TriangleOrientation::Up => [
            child_triangles[0][0],
            child_triangles[1][1],
            child_triangles[2][2],
        ],
        crate::mother_grid::TriangleOrientation::Down => [
            child_triangles[0][0],
            child_triangles[2][1],
            child_triangles[1][2],
        ],
    };
    let edge_set = child_triangles
        .iter()
        .flat_map(|t| [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])])
        .map(|(a, b)| edge(a, b))
        .collect::<BTreeSet<_>>();
    let sites = child_triangles
        .iter()
        .flat_map(|t| t.iter().copied())
        .collect::<BTreeSet<_>>();
    let mut midpoints = [0usize; 3];
    for side in 0..3 {
        let a = corners[side];
        let b = corners[(side + 1) % 3];
        midpoints[side] = sites
            .iter()
            .copied()
            .find(|&m| {
                m != a && m != b && edge_set.contains(&edge(a, m)) && edge_set.contains(&edge(m, b))
            })
            .ok_or_else(|| format!("parent {parent:?} side {side} has no exact midpoint"))?;
    }
    let mut neighbours = [parent; 3];
    for side in 0..3 {
        neighbours[side] = neighbour_parent(
            source,
            parent,
            corners[side],
            midpoints[side],
            corners[(side + 1) % 3],
        )?;
    }
    Ok(ParentPatch {
        corners,
        midpoints,
        neighbours,
        child_triangles,
    })
}

pub(super) fn hierarchy_parent_neighbours(
    source: &MotherGrid,
    parent: TriangleAddress,
) -> Result<[TriangleAddress; 3], String> {
    Ok(parent_patch(source, parent)?.neighbours)
}

fn neighbour_parent(
    source: &MotherGrid,
    parent: TriangleAddress,
    a: usize,
    midpoint: usize,
    b: usize,
) -> Result<TriangleAddress, String> {
    let mut neighbours = BTreeSet::new();
    for target in [edge(a, midpoint), edge(midpoint, b)] {
        let mut found = None;
        for child in parent.children_2_to_1().unwrap() {
            let slot = source_face_slot(source, child)?;
            let tri = source.mesh.triangles()[slot];
            for side in 0..3 {
                if edge(tri[side], tri[(side + 1) % 3]) != target {
                    continue;
                }
                if found.is_some() {
                    return Err(format!(
                        "parent {parent:?} boundary segment {target:?} has multiple child claims"
                    ));
                }
                let neighbour = source.mesh.neighbours()[slot][(side + 2) % 3];
                if neighbour == 0 {
                    return Err(format!(
                        "parent {parent:?} boundary segment {target:?} is open"
                    ));
                }
                found =
                    source.triangle_addresses[neighbour].and_then(TriangleAddress::parent_2_to_1);
            }
        }
        neighbours.insert(found.ok_or_else(|| {
            format!("parent {parent:?} boundary segment {target:?} has no neighbour")
        })?);
    }
    if neighbours.len() != 1 {
        return Err(format!(
            "parent {parent:?} coarse side has inconsistent fine neighbours"
        ));
    }
    let neighbour = *neighbours.first().expect("one exact neighbour");
    if neighbour == parent {
        return Err(format!(
            "parent {parent:?} names itself across a coarse side"
        ));
    }
    Ok(neighbour)
}

fn transition_polygon(patch: &ParentPatch, core: &BTreeSet<TriangleAddress>) -> Vec<usize> {
    let mut polygon = Vec::with_capacity(6);
    for side in 0..3 {
        polygon.push(patch.corners[side]);
        if !core.contains(&patch.neighbours[side]) {
            polygon.push(patch.midpoints[side]);
        }
    }
    polygon.dedup();
    if polygon.first() == polygon.last() {
        polygon.pop();
    }
    polygon
}

fn triangulations(polygon: &[usize]) -> Vec<Vec<[usize; 3]>> {
    if polygon.len() == 3 {
        return vec![vec![[polygon[0], polygon[1], polygon[2]]]];
    }
    let mut out = Vec::new();
    for split in 1..polygon.len() - 1 {
        let tri = [polygon[0], polygon[split], polygon[polygon.len() - 1]];
        for left in triangulations_or_empty(&polygon[..=split]) {
            for right in triangulations_or_empty(&polygon[split..]) {
                let mut candidate = vec![tri];
                candidate.extend(left.iter().copied());
                candidate.extend(right.iter().copied());
                out.push(candidate);
            }
        }
    }
    out
}

fn triangulations_or_empty(polygon: &[usize]) -> Vec<Vec<[usize; 3]>> {
    if polygon.len() < 3 {
        vec![Vec::new()]
    } else {
        triangulations(polygon)
    }
}

fn ranked_triangulations(
    source: &MotherGrid,
    polygon: &[usize],
    original: &[[usize; 3]; 4],
) -> Vec<Vec<[usize; 3]>> {
    // Maximum face reuse puts the known one-site-retirement signatures first.
    // The remaining Catalan candidates are exactly the finite diagonal/flip
    // alternatives for this convex hierarchy-parent polygon.
    let original = original
        .iter()
        .copied()
        .map(canonical_triangle)
        .collect::<BTreeSet<_>>();
    let mut variants = triangulations(polygon)
        .into_iter()
        .filter(|candidate| {
            candidate.iter().all(|triangle| {
                orientation_on_sphere(
                    source.mesh.vertices()[triangle[0]],
                    source.mesh.vertices()[triangle[1]],
                    source.mesh.vertices()[triangle[2]],
                ) == Ok(Sign::Positive)
            })
        })
        .collect::<Vec<_>>();
    variants.sort_by(|left, right| {
        let left_reuse = left
            .iter()
            .filter(|triangle| original.contains(&canonical_triangle(**triangle)))
            .count();
        let right_reuse = right
            .iter()
            .filter(|triangle| original.contains(&canonical_triangle(**triangle)))
            .count();
        right_reuse
            .cmp(&left_reuse)
            .then_with(|| canonical_candidate(left).cmp(&canonical_candidate(right)))
    });
    variants.dedup_by(|left, right| canonical_candidate(left) == canonical_candidate(right));
    variants
}

fn canonical_triangle(mut triangle: [usize; 3]) -> [usize; 3] {
    triangle.sort_unstable();
    triangle
}

fn canonical_candidate(triangles: &[[usize; 3]]) -> Vec<[usize; 3]> {
    let mut canonical = triangles
        .iter()
        .copied()
        .map(canonical_triangle)
        .collect::<Vec<_>>();
    canonical.sort_unstable();
    canonical
}

fn base_degree_forecast(
    source: &MotherGrid,
    core: &BTreeSet<TriangleAddress>,
    custom_transition: &BTreeSet<TriangleAddress>,
    patches: &BTreeMap<TriangleAddress, ParentPatch>,
) -> Result<BTreeMap<usize, isize>, String> {
    let mut forecast = BTreeMap::new();
    for parent in core {
        let patch = &patches[parent];
        adjust_source_triangles(source, &mut forecast, &patch.child_triangles, -1)?;
        adjust_source_triangles(source, &mut forecast, &[patch.corners], 1)?;
    }
    for parent in custom_transition {
        adjust_source_triangles(source, &mut forecast, &patches[parent].child_triangles, -1)?;
    }
    Ok(forecast)
}

fn adjust_source_triangles(
    source: &MotherGrid,
    forecast: &mut BTreeMap<usize, isize>,
    triangles: &[[usize; 3]],
    delta: isize,
) -> Result<(), String> {
    for vertex in triangles
        .iter()
        .flat_map(|triangle| triangle.iter().copied())
    {
        let degree = forecast
            .entry(vertex)
            .or_insert(source_degree(source, vertex)?);
        *degree += delta;
    }
    Ok(())
}

fn source_degree(source: &MotherGrid, vertex: usize) -> Result<isize, String> {
    match source.addresses.get(vertex).and_then(Option::as_ref) {
        Some(VertexAddress::IcosahedronVertex(_)) => Ok(5),
        Some(_) => Ok(6),
        None => Err(format!("source vertex {vertex} has no hierarchy address")),
    }
}

fn edge(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}

fn boundary(
    source: &MotherGrid,
    core: &BTreeSet<TriangleAddress>,
    transition: &BTreeSet<TriangleAddress>,
) -> Result<TransitionBoundary, String> {
    let mut coarse_edges = Vec::new();
    for &parent in core {
        let patch = parent_patch(source, parent)?;
        for side in 0..3 {
            if transition.contains(&patch.neighbours[side]) {
                coarse_edges.push((patch.corners[(side + 1) % 3], patch.corners[side]));
            }
        }
    }
    let mut fine_edges = Vec::new();
    for &parent in transition {
        let patch = parent_patch(source, parent)?;
        for side in 0..3 {
            if !core.contains(&patch.neighbours[side])
                && !transition.contains(&patch.neighbours[side])
            {
                fine_edges.extend([
                    (patch.corners[side], patch.midpoints[side]),
                    (patch.midpoints[side], patch.corners[(side + 1) % 3]),
                ]);
            }
        }
    }
    let coarse = cycles_from_edges(coarse_edges)?;
    let fine = cycles_from_edges(fine_edges)?;
    let boundary_sites = coarse
        .iter()
        .chain(&fine)
        .flat_map(|cycle| cycle.iter().copied())
        .collect::<BTreeSet<_>>();
    let seam = boundary_sites
        .iter()
        .copied()
        .filter(|&site| {
            matches!(
                source.addresses.get(site).and_then(Option::as_ref),
                Some(VertexAddress::IcosahedronEdge { .. } | VertexAddress::IcosahedronVertex(_))
            )
        })
        .collect();
    let pentagon = boundary_sites
        .into_iter()
        .filter(|&site| {
            matches!(
                source.addresses.get(site).and_then(Option::as_ref),
                Some(VertexAddress::IcosahedronVertex(_))
            )
        })
        .collect();
    Ok(TransitionBoundary {
        fine_outer_cycles: fine,
        coarse_inner_cycles: coarse,
        halo_parents: transition.iter().copied().collect(),
        seam,
        pentagon,
    })
}

fn cycles_from_edges(edges: Vec<(usize, usize)>) -> Result<Vec<Vec<usize>>, String> {
    if edges.is_empty() {
        return Ok(Vec::new());
    }
    let mut next = BTreeMap::<usize, usize>::new();
    let mut incoming = BTreeMap::<usize, usize>::new();
    for (a, b) in edges {
        if next.insert(a, b).is_some() {
            return Err(format!("boundary vertex {a} has multiple outgoing edges"));
        }
        *incoming.entry(b).or_default() += 1;
    }
    if let Some((&vertex, &degree)) = incoming.iter().find(|(_, degree)| **degree != 1) {
        return Err(format!(
            "boundary vertex {vertex} has incoming degree {degree}, expected 1"
        ));
    }
    if next.keys().copied().collect::<BTreeSet<_>>()
        != incoming.keys().copied().collect::<BTreeSet<_>>()
    {
        return Err("boundary directed edges do not form closed cycles".into());
    }
    let mut cycles = Vec::new();
    while let Some(&start) = next.keys().next() {
        let mut cycle = Vec::new();
        let mut current = start;
        loop {
            cycle.push(current);
            let following = next
                .remove(&current)
                .ok_or_else(|| "boundary cycle ended before returning to its start".to_string())?;
            current = following;
            if current == start {
                break;
            }
            if cycle.contains(&current) {
                return Err("boundary cycle repeats a vertex before closing".into());
            }
        }
        cycles.push(cycle);
    }
    cycles.sort();
    Ok(cycles)
}

fn hard_gate(mesh: &MeshState) -> Result<(), String> {
    mesh.validate().map_err(|errors| {
        errors
            .into_iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    if mesh.open_edge_count() != 0 {
        return Err(format!("mesh has {} open edges", mesh.open_edge_count()));
    }
    let mut edges = BTreeSet::new();
    let mut degrees = vec![0usize; mesh.vertices().len()];
    let mut seeds = vec![0usize; mesh.vertices().len()];
    let mut triangles = BTreeSet::new();
    for face in mesh.active_triangle_slots() {
        let tri = mesh.triangles()[face];
        if orientation_on_sphere(
            mesh.vertices()[tri[0]],
            mesh.vertices()[tri[1]],
            mesh.vertices()[tri[2]],
        )
        .map_err(|e| e.to_string())?
            != Sign::Positive
        {
            return Err(format!("triangle {face} is not positively oriented"));
        }
        let mut canonical = tri;
        canonical.sort_unstable();
        if !triangles.insert(canonical) {
            return Err(format!("duplicate triangle {canonical:?}"));
        }
        for [u, v] in [[tri[0], tri[1]], [tri[1], tri[2]], [tri[2], tri[0]]] {
            edges.insert(edge(u, v));
        }
        for vertex in tri {
            degrees[vertex] += 1;
            if seeds[vertex] == 0 {
                seeds[vertex] = face;
            }
        }
    }
    let euler =
        mesh.vertex_count() as isize - edges.len() as isize + mesh.triangle_count() as isize;
    if euler != 2 {
        return Err(format!("Euler characteristic is {euler}, expected 2"));
    }
    if let Some((vertex, degree)) = degrees
        .iter()
        .copied()
        .enumerate()
        .find(|&(_, degree)| degree != 0 && !(5..=7).contains(&degree))
    {
        return Err(format!("vertex {vertex} degree {degree} outside 5..=7"));
    }
    for vertex in mesh.active_vertex_slots() {
        let fan = mesh
            .triangle_fan_from(vertex, seeds[vertex])
            .map_err(|error| error.to_string())?;
        if fan.len() != degrees[vertex] {
            return Err(format!(
                "vertex {vertex} has {} incident faces but a {}-face connected fan",
                degrees[vertex],
                fan.len()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finite_polygon_enumeration_has_the_catalan_counts() {
        assert_eq!(triangulations(&[1, 2, 3]).len(), 1);
        assert_eq!(triangulations(&[1, 2, 3, 4]).len(), 2);
        assert_eq!(triangulations(&[1, 2, 3, 4, 5]).len(), 5);
    }

    #[test]
    fn boundary_walk_rejects_an_open_chain() {
        assert!(cycles_from_edges(vec![(1, 2), (2, 3)]).is_err());
        assert_eq!(
            cycles_from_edges(vec![(1, 2), (2, 3), (3, 1)]).unwrap(),
            vec![vec![1, 2, 3]]
        );
    }

    #[test]
    fn mixed_radix_walks_the_product_with_last_slot_fastest() {
        let variants = vec![
            vec![vec![[1, 2, 3]], vec![[1, 3, 4]]],
            vec![vec![[5, 6, 7]], vec![[5, 7, 8]], vec![[5, 8, 9]]],
        ];
        let mut indices = vec![0, 0];
        let mut visited = vec![indices.clone()];
        while advance_mixed_radix(&mut indices, &variants) {
            visited.push(indices.clone());
        }
        assert_eq!(
            visited,
            vec![
                vec![0, 0],
                vec![0, 1],
                vec![0, 2],
                vec![1, 0],
                vec![1, 1],
                vec![1, 2],
            ]
        );
    }

    #[test]
    fn degree_prefix_bounds_keep_later_repairable_candidates() {
        let variants = vec![
            vec![vec![[2, 3, 4]]],
            vec![vec![[1, 5, 6]], vec![[2, 5, 6]]],
        ];
        let parents = [
            TriangleAddress {
                base_face: 0,
                i: 0,
                j: 0,
                n: 1,
                orientation: crate::mother_grid::TriangleOrientation::Up,
            },
            TriangleAddress {
                base_face: 0,
                i: 0,
                j: 0,
                n: 1,
                orientation: crate::mother_grid::TriangleOrientation::Down,
            },
        ];
        let mut chosen = vec![None; variants.len()];
        let (variables, fixed) = search_variables(&variants, &parents, &mut chosen);
        let suffix = suffix_degree_masks(&variables);
        let mut forecast = DenseForecast::new(
            7,
            &BTreeMap::from([(1, 4), (2, 4), (3, 4), (4, 4), (5, 4), (6, 4)]),
        );
        for position in chosen
            .iter()
            .enumerate()
            .filter_map(|(position, chosen)| chosen.map(|_| position))
        {
            forecast.apply_triangles(&variants[position][0], 1);
        }
        assert!(forecast.can_finish_all(&fixed, &suffix[0]));

        let repair = variables[0]
            .variants
            .iter()
            .find(|choice| choice.variant_index == 0)
            .unwrap();
        forecast.apply_delta(&repair.delta, 1);
        assert!(forecast.can_finish_all(&variables[0].touched, &[]));
        forecast.apply_delta(&repair.delta, -1);

        let bad = variables[0]
            .variants
            .iter()
            .find(|choice| choice.variant_index == 1)
            .unwrap();
        forecast.apply_delta(&bad.delta, 1);
        assert!(!forecast.can_finish_all(&variables[0].touched, &[]));
    }

    #[test]
    fn hinted_halo_promotion_moves_the_connected_failed_boundary_segment() {
        let source = MotherGrid::generate(8).unwrap();
        let mut core = source
            .triangle_addresses
            .iter()
            .flatten()
            .filter_map(|child| child.parent_2_to_1())
            .collect::<BTreeSet<_>>();
        let initial_transition = *core.first().unwrap();
        core.remove(&initial_transition);
        let mut transition = BTreeSet::from([initial_transition]);
        let peel = core_boundary(&source, &core);
        assert!(peel.len() > 1);
        let preferred = *peel.first().unwrap();
        let expected = preferred_boundary_segment(&source, &peel, &transition, preferred).unwrap();
        assert!(expected.len() > 1);
        let untouched = core
            .iter()
            .copied()
            .find(|parent| !expected.contains(parent))
            .unwrap();
        let initial_core_len = core.len();

        assert_eq!(
            promote_core_boundary(&source, &mut core, &mut transition, Some((preferred, 1)), 1,),
            Some(1)
        );
        assert_eq!(core.len(), initial_core_len - expected.len());
        assert!(expected.iter().all(|parent| transition.contains(parent)));
        assert!(core.contains(&untouched));
    }
}
