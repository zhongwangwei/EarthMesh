//! Exact 2:1 coarse/fine interface closure over hierarchy addresses.
//!
//! Core parent edges stay coarse. Only adjacent fine parent patches are
//! retriangulated, using their source slots and a finite non-crossing polygon
//! state space; geometry relocation belongs to the later elastic stage.

use super::{HierarchyComponent, HierarchyLeafMesh, HierarchyLeafSet};
use crate::certificate::spherical_triangle_angles;
use crate::mother_grid::{MotherGrid, TriangleAddress, VertexAddress};
use earthmesh_mesh::{
    orientation_on_sphere, CartesianPoint, MeshState, RetirementPostconditionOutcome,
    RetirementSearchOutcome, Sign,
};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransitionTopologyLimits {
    pub topology_states: usize,
    pub maximum_halo_expansions: usize,
}

/// What a failed candidate asks of the next search (guides 11.122, 11.130).
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct RetryRequest<'a> {
    /// The core parent nearest the failed face, and the halo expansions
    /// its promotion costs.
    pub promotion: Option<(TriangleAddress, usize)>,
    /// A near miss's other places (guide 11.137): the core parents nearest
    /// its other faces outside the window, worst first, with their costs.
    /// Each is promoted with `promotion` where it pinches nothing.
    pub also: &'a [(TriangleAddress, usize)],
    /// How deep a failure's promotion may reach: every parent's ring
    /// distance from the component's first transition ring, and the deepest
    /// ring allowed. Lets the promotion repair the pinch it makes once the
    /// halo budget is spent.
    pub reach: Option<(&'a BTreeMap<TriangleAddress, usize>, usize)>,
    /// The failed candidate and where it failed: when no promotion moves the
    /// transition there, the search offers only transitions that change it
    /// at the failure and keep it as the candidate had it farther away.
    pub focus: Option<&'a RetryFocus>,
}

/// A failed candidate's custom transition and the place it failed (guide
/// 11.130). Distances are in parent edge lengths from `point` to a custom
/// parent's centre. A focused search keeps every custom parent beyond
/// `radius_edges` as the candidate chose it, and offers only configurations
/// that change a parent within `change_radius_edges` -- the transition at
/// the failure -- and, once widened (`previous_radius_edges` above zero), a
/// parent beyond the previous radius: everything inside it was offered
/// already. The degree rule couples the parents along the transition, so a
/// change at the failure needs others to compensate; the search tries each
/// free parent's chosen triangles first and sets the parents from the
/// farthest to the nearest, so the compensations it finds first lie as near
/// the failure as they can.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct RetryFocus {
    pub point: CartesianPoint,
    pub change_radius_edges: f64,
    pub radius_edges: f64,
    pub previous_radius_edges: f64,
    pub chosen: BTreeMap<TriangleAddress, Vec<[usize; 3]>>,
    /// Focused states already examined: the next search starts after them.
    pub cursor: usize,
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
        request: RetryRequest<'_>,
        promote_first: bool,
    ) -> TransitionTopologyOutcome {
        solve_transition_topology_from_cursor_with_promotion(
            source,
            component,
            self,
            topology_states_cursor,
            request,
            promote_first,
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
    /// For a candidate of a focused search (`RetryFocus`): the focused
    /// states examined up to and including it. The layout's own cursor
    /// (`layout_topology_states`) stays where it was.
    pub focus_topology_states: Option<usize>,
    /// The candidate retires a vertex (`solve_retirement_family`): it is
    /// none of the layout's configurations, and no focus is built on it.
    pub retired_vertex: bool,
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
    /// A focused search (`RetryFocus`) found no configuration it had not
    /// already offered.
    FocusExhausted {
        states_examined: usize,
        halo_expansions: usize,
    },
}

/// Parent patches of one source mother, each worked out once. A patch is a
/// function of the source and the parent alone, so a search keeps them for
/// its whole run.
struct Patches<'a> {
    source: &'a MotherGrid,
    known: std::cell::RefCell<crate::mother_grid::AddressMap<ParentPatch>>,
}

impl<'a> Patches<'a> {
    fn new(source: &'a MotherGrid) -> Self {
        Self {
            source,
            known: Default::default(),
        }
    }

    fn get(&self, parent: TriangleAddress) -> Result<ParentPatch, String> {
        if let Some(patch) = self.known.borrow().get(&parent) {
            return Ok(patch.clone());
        }
        let patch = parent_patch(self.source, parent)?;
        self.known.borrow_mut().insert(parent, patch.clone());
        Ok(patch)
    }
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
        RetryRequest::default(),
        false,
    )
}

fn solve_transition_topology_from_cursor_with_promotion(
    source: &MotherGrid,
    component: &HierarchyComponent,
    limits: TransitionTopologyLimits,
    topology_states_cursor: usize,
    request: RetryRequest<'_>,
    promote_first: bool,
) -> TransitionTopologyOutcome {
    let mut preferred_core_promotion = request.promotion;
    // A search asks for the same parent's patch from its preflight, its
    // core forecast, its boundary and every halo expansion: each once.
    let patches = &Patches::new(source);
    let mut core = set(component.core_parents.iter().copied());
    let mut transition = set(component.transition_parents.iter().copied());
    if let Err(reason) = preflight(patches, component, &core, &transition) {
        return TransitionTopologyOutcome::InvalidBoundary {
            states_examined: 0,
            halo_expansions: 0,
            reason,
        };
    }
    let mut halo_expansions = 0usize;
    let mut states_examined = 0usize;
    // A candidate that failed at a face asks for the transition to widen
    // there first (guide 11.122): its nearest core parent's boundary segment
    // is promoted and the new layout is enumerated from its first state, the
    // old layout's `topology_states_cursor` states counted as examined.
    // Going on with the old layout's enumeration changes the transition
    // wherever the search order happens to be, which in a component the
    // size of a region is far from the failure. A parent off the core
    // boundary, or `promote_first` off, keeps the old behaviour: the
    // promotion waits until the layout's states are spent.
    //
    // The old layout is kept (`unpromoted`) until the search returns: a
    // promotion that ends without a candidate -- an invalid boundary the
    // search cannot repair, a core pinched at a vertex once the halo budget
    // is spent as in the 20 km 30 m trial, or no topology at all -- falls
    // back to it, once, and the search goes on as it would have without
    // the promotion. Only a spent state budget ends it either way.
    //
    // A promotion that pinches the core once the halo budget is spent may
    // still repair the pinch (guide 11.130): the parents at the pinch leave
    // the core as well when none lies deeper than a promotion may reach
    // (`request.reach`), at most `MAXIMUM_PINCH_REPAIRS` times.
    let mut unpromoted = None;
    let mut pinch_repairs = 0usize;
    if let Some(preferred) = preferred_core_promotion.filter(|_| promote_first) {
        let kept = (core.clone(), transition.clone());
        let mut promoted = promote_preferred_segment(
            patches,
            &mut core,
            &mut transition,
            preferred,
            limits.maximum_halo_expansions,
        );
        let mut places = usize::from(promoted.is_some());
        if let Some(cost) = promoted.as_mut().filter(|_| !request.also.is_empty()) {
            match promote_other_places(
                patches,
                &mut core,
                &mut transition,
                request.also,
                limits.maximum_halo_expansions,
            ) {
                Ok((others, deepest)) => {
                    places += others;
                    *cost = (*cost).max(deepest);
                }
                Err(reason) => {
                    return invalid(0, 0, reason);
                }
            }
        }
        if crate::construction::cmrc_timing_enabled() {
            eprintln!(
                "earthmesh_cli: cmrc_detail phase=promotion component={} preferred={:?} \
                 cost={} remaining={} promoted={} places={places} asked={}",
                component.id,
                preferred.0,
                preferred.1,
                limits.maximum_halo_expansions,
                kept.0.len() - core.len(),
                1 + request.also.len()
            );
        }
        if let Some(cost) = promoted {
            unpromoted = Some(kept);
            halo_expansions += cost;
            states_examined = topology_states_cursor;
            preferred_core_promotion = None;
        }
    }
    macro_rules! end_or_fall_back {
        ($outcome:expr) => {{
            let outcome = $outcome;
            if let Some((kept_core, kept_transition)) = unpromoted.take() {
                if crate::construction::cmrc_timing_enabled() {
                    eprintln!(
                        "earthmesh_cli: cmrc_detail phase=promotion_fallback component={} \
                         after={outcome:?}",
                        component.id
                    );
                }
                (core, transition) = (kept_core, kept_transition);
                halo_expansions = 0;
                states_examined = 0;
                continue;
            }
            return outcome;
        }};
    }

    loop {
        if core.is_empty() {
            end_or_fall_back!(TransitionTopologyOutcome::ProvenInfeasible {
                states_examined,
                halo_expansions,
                reason: "halo expansion leaves no coarse core".into(),
            });
        }

        let uncovered = core
            .iter()
            .copied()
            .filter(|&parent| {
                patches.get(parent).is_ok_and(|patch| {
                    patch.neighbours.iter().any(|neighbour| {
                        !in_core(&core, *neighbour) && !transition.contains(neighbour)
                    })
                })
            })
            .collect::<BTreeSet<_>>();
        if uncovered.is_empty() && transition.is_empty() {
            return pure_core(patches, component.id, &core, halo_expansions);
        }
        if !uncovered.is_empty() {
            if uncovered.len() == core.len() || halo_expansions == limits.maximum_halo_expansions {
                end_or_fall_back!(TransitionTopologyOutcome::RequiresWiderHalo {
                    states_examined,
                    halo_expansions,
                });
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

        // The layout the failed candidate came from, with no promotion in
        // effect: a focus varies it around the failure only (guide 11.130).
        if let Some(focus) = request
            .focus
            .filter(|_| unpromoted.is_none() && halo_expansions == 0)
        {
            return solve_focused(
                patches,
                component.id,
                &core,
                &transition,
                focus,
                topology_states_cursor,
                limits.topology_states - topology_states_cursor,
            );
        }

        let remaining_states = limits.topology_states - states_examined;
        let remaining_halos = limits.maximum_halo_expansions - halo_expansions + 1;
        let local_limit = remaining_states.div_ceil(remaining_halos);
        let local_cursor = topology_states_cursor.saturating_sub(states_examined);
        match solve_once(
            patches,
            component.id,
            core.clone(),
            transition.clone(),
            halo_expansions,
            local_cursor,
            local_limit,
            None,
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
                    patches,
                    &mut core,
                    &mut transition,
                    preferred_core_promotion.take(),
                    limits.maximum_halo_expansions - halo_expansions,
                ) else {
                    end_or_fall_back!(TransitionTopologyOutcome::SearchBudgetExhausted {
                        states_examined,
                        halo_expansions,
                    });
                };
                halo_expansions += expansion_cost;
            }
            TransitionTopologyOutcome::InvalidBoundary { reason, .. } => {
                if reason.starts_with("coarse inner boundary:") {
                    // Past the halo budget a pinch is still repaired where
                    // no parent at it lies deeper than a promotion may reach
                    // (guide 11.130): a few times after a failure's
                    // promotion, more in the component's own layout, where a
                    // real requirement's core can pinch at many places
                    // (guide 11.138).
                    let pinch_repair_limit = if unpromoted.is_some() {
                        MAXIMUM_PINCH_REPAIRS
                    } else {
                        MAXIMUM_LAYOUT_PINCH_REPAIRS
                    };
                    let repair = if halo_expansions < limits.maximum_halo_expansions {
                        Some(None)
                    } else if pinch_repairs < pinch_repair_limit {
                        request.reach.map(Some)
                    } else {
                        None
                    };
                    if crate::construction::cmrc_timing_enabled() && repair.is_none() {
                        eprintln!(
                            "earthmesh_cli: cmrc_detail phase=pinch_repair component={} \
                             outcome=no_budget core={} halo={} of={} repairs={pinch_repairs}",
                            component.id,
                            core.len(),
                            halo_expansions,
                            limits.maximum_halo_expansions
                        );
                    }
                    if let Some(reach) = repair {
                        let before = core.len();
                        let repaired =
                            promote_pinched_core(patches, &mut core, &mut transition, reach);
                        if crate::construction::cmrc_timing_enabled() {
                            eprintln!(
                                "earthmesh_cli: cmrc_detail phase=pinch_repair component={} \
                                 outcome={:?} promoted={} core={} halo={} of={} repairs={pinch_repairs}",
                                component.id,
                                repaired.as_ref().map_err(|_| "error"),
                                before - core.len(),
                                core.len(),
                                halo_expansions,
                                limits.maximum_halo_expansions
                            );
                        }
                        match repaired {
                            Ok(PinchRepair::Promoted) => {
                                match reach {
                                    None => halo_expansions += 1,
                                    Some(_) => pinch_repairs += 1,
                                }
                                continue;
                            }
                            Ok(PinchRepair::Nothing) => {}
                            // Nothing would be left to coarsen: no topology,
                            // as when a halo expansion leaves no core, and
                            // the component stays as it is (guide 11.136).
                            Ok(PinchRepair::WholeCore) => {
                                end_or_fall_back!(TransitionTopologyOutcome::ProvenInfeasible {
                                    states_examined,
                                    halo_expansions,
                                    reason: format!("every core parent is at a pinch ({reason})"),
                                })
                            }
                            Err(repair_reason) => {
                                end_or_fall_back!(invalid(
                                    states_examined,
                                    halo_expansions,
                                    repair_reason
                                ))
                            }
                        }
                    }
                }
                if reason.starts_with("fine outer boundary:")
                    && halo_expansions < limits.maximum_halo_expansions
                {
                    let retained = retain_fine_at_pinches(patches, &core, &mut transition);
                    if crate::construction::cmrc_timing_enabled() {
                        eprintln!(
                            "earthmesh_cli: cmrc_detail phase=fine_pinch_repair component={} \
                             outcome={:?} transition={} halo={} of={}",
                            component.id,
                            retained.as_ref().map_err(|_| "error"),
                            transition.len(),
                            halo_expansions,
                            limits.maximum_halo_expansions
                        );
                    }
                    match retained {
                        Ok(true) => {
                            halo_expansions += 1;
                            continue;
                        }
                        Ok(false) => {}
                        Err(repair_reason) => {
                            end_or_fall_back!(invalid(
                                states_examined,
                                halo_expansions,
                                repair_reason
                            ))
                        }
                    }
                }
                if crate::construction::cmrc_timing_enabled()
                    && unpromoted.is_some()
                    && reason.starts_with("coarse inner boundary:")
                {
                    let pinches = coarse_boundary_edges(patches, &core, &transition)
                        .map(branched_boundary_vertices)
                        .unwrap_or_default();
                    let at_pinch = |parents: &BTreeSet<TriangleAddress>| {
                        parents
                            .iter()
                            .filter(|parent| {
                                patches.get(**parent).is_ok_and(|patch| {
                                    patch.corners.iter().any(|corner| pinches.contains(corner))
                                })
                            })
                            .count()
                    };
                    eprintln!(
                        "earthmesh_cli: cmrc_detail phase=promotion_pinch component={} \
                         pinches={} core_at_pinch={} transition_at_pinch={} halo={} remaining={}",
                        component.id,
                        pinches.len(),
                        at_pinch(&core),
                        at_pinch(&transition),
                        halo_expansions,
                        limits.maximum_halo_expansions
                    );
                }
                end_or_fall_back!(invalid(states_examined, halo_expansions, reason));
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
                let peel = core_boundary(patches, &core);
                if peel.is_empty() || peel.len() == core.len() {
                    end_or_fall_back!(TransitionTopologyOutcome::ProvenInfeasible {
                        states_examined,
                        halo_expansions,
                        reason,
                    });
                }
                let Some(expansion_cost) = promote_core_boundary(
                    patches,
                    &mut core,
                    &mut transition,
                    preferred_core_promotion.take(),
                    limits.maximum_halo_expansions - halo_expansions,
                ) else {
                    end_or_fall_back!(TransitionTopologyOutcome::RequiresWiderHalo {
                        states_examined,
                        halo_expansions,
                    });
                };
                halo_expansions += expansion_cost;
            }
            TransitionTopologyOutcome::RequiresWiderHalo { .. }
            | TransitionTopologyOutcome::FocusExhausted { .. } => unreachable!(),
        }
    }
}

/// Pinch repairs a failure's promotion may make past the halo budget.
const MAXIMUM_PINCH_REPAIRS: usize = 3;
/// Pinch repairs a component's own layout may make past the halo budget,
/// each within the promotion's reach. Every repair takes parents out of the
/// core, so the repairs end; the Heihe trial's first layout needed more
/// rounds than its five rings (guide 11.138).
const MAXIMUM_LAYOUT_PINCH_REPAIRS: usize = 16;

fn preflight(
    patches: &Patches<'_>,
    component: &HierarchyComponent,
    core: &BTreeSet<TriangleAddress>,
    transition: &BTreeSet<TriangleAddress>,
) -> Result<(), String> {
    let source = patches.source;
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
        patches.get(parent)?;
    }
    // Parents of a built region that touch the settled region are joined
    // through it: the planner put them in one component through it.
    let settled_neighbours = parents
        .iter()
        .copied()
        .filter(|&parent| {
            patches
                .get(parent)
                .is_ok_and(|patch| patch.neighbours.iter().any(|p| p.is_outside()))
        })
        .collect::<Vec<_>>();
    let seed = *parents.first().expect("non-empty component");
    let mut seen = BTreeSet::from([seed]);
    let mut stack = vec![seed];
    let mut through_settled = false;
    while let Some(parent) = stack.pop() {
        for neighbour in patches.get(parent)?.neighbours {
            if neighbour.is_outside() && !through_settled {
                through_settled = true;
                for &joined in &settled_neighbours {
                    if seen.insert(joined) {
                        stack.push(joined);
                    }
                }
            }
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

/// Whether a parent's neighbour is core: listed in `core`, or a settled
/// parent beyond a built region's edge -- settled parents are all core.
fn in_core(core: &BTreeSet<TriangleAddress>, parent: TriangleAddress) -> bool {
    parent.is_outside() || core.contains(&parent)
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
    patches: &Patches<'_>,
    core: &mut BTreeSet<TriangleAddress>,
    transition: &mut BTreeSet<TriangleAddress>,
    preferred: Option<(TriangleAddress, usize)>,
    remaining_halo_expansions: usize,
) -> Option<usize> {
    let peel = core_boundary(patches, core);
    if peel.is_empty() || peel.len() == core.len() {
        return None;
    }
    let (promoted, expansion_cost) = match preferred
        .filter(|(parent, cost)| peel.contains(parent) && *cost <= remaining_halo_expansions)
    {
        Some((parent, cost)) => (
            preferred_boundary_segment(patches, &peel, transition, parent)?,
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

/// A failure's promotion alone: the preferred parent's boundary segment
/// (`preferred_boundary_segment`), when the parent lies on the core boundary
/// and its cost fits the halo budget -- never the whole boundary, which is
/// `promote_core_boundary`'s answer for a layout whose states are spent.
fn promote_preferred_segment(
    patches: &Patches<'_>,
    core: &mut BTreeSet<TriangleAddress>,
    transition: &mut BTreeSet<TriangleAddress>,
    (preferred, cost): (TriangleAddress, usize),
    remaining_halo_expansions: usize,
) -> Option<usize> {
    if cost > remaining_halo_expansions {
        return None;
    }
    let peel = core_boundary(patches, core);
    if !peel.contains(&preferred) || peel.len() == core.len() {
        return None;
    }
    let segment = preferred_boundary_segment(patches, &peel, transition, preferred)?;
    promote_to_transition(core, transition, segment);
    Some(cost)
}

/// Promotes a near miss's other places (guide 11.137) after its worst
/// one: each parent still on the core boundary takes its boundary segment
/// into the transition, as the worst place's did, unless that pinches the
/// core at one of the segment's corners -- then it is put back. Returns how
/// many places were promoted and the dearest one's cost.
fn promote_other_places(
    patches: &Patches<'_>,
    core: &mut BTreeSet<TriangleAddress>,
    transition: &mut BTreeSet<TriangleAddress>,
    others: &[(TriangleAddress, usize)],
    remaining_halo_expansions: usize,
) -> Result<(usize, usize), String> {
    let mut peel = core_boundary(patches, core);
    let (mut places, mut deepest) = (0, 0);
    for &(other, cost) in others {
        if cost > remaining_halo_expansions || !core.contains(&other) || !peel.contains(&other) {
            continue;
        }
        let Some(segment) = preferred_boundary_segment(patches, &peel, transition, other) else {
            continue;
        };
        let segment = segment
            .into_iter()
            .filter(|parent| core.contains(parent))
            .collect::<BTreeSet<_>>();
        if segment.len() >= core.len() {
            continue;
        }
        promote_to_transition(core, transition, segment.clone());
        if pinches_at(patches, core, transition, &segment)? {
            for parent in &segment {
                transition.remove(parent);
                core.insert(*parent);
            }
            continue;
        }
        for parent in &segment {
            peel.remove(parent);
            for neighbour in patches.get(*parent)?.neighbours {
                if core.contains(&neighbour) {
                    peel.insert(neighbour);
                }
            }
        }
        places += 1;
        deepest = deepest.max(cost);
    }
    Ok((places, deepest))
}

/// Whether the coarse boundary branches at a corner of `segment`, the only
/// place promoting it can pinch the core. The core parents round such a
/// corner lie within three sides of the segment, so only their boundary
/// edges are read.
fn pinches_at(
    patches: &Patches<'_>,
    core: &BTreeSet<TriangleAddress>,
    transition: &BTreeSet<TriangleAddress>,
    segment: &BTreeSet<TriangleAddress>,
) -> Result<bool, String> {
    let mut corners = BTreeSet::new();
    for parent in segment {
        corners.extend(patches.get(*parent)?.corners);
    }
    let mut seen = segment.clone();
    let mut frontier = segment.iter().copied().collect::<Vec<_>>();
    let mut near = BTreeSet::new();
    for _ in 0..3 {
        let mut next = Vec::new();
        for parent in frontier {
            for neighbour in patches.get(parent)?.neighbours {
                if !neighbour.is_outside() && seen.insert(neighbour) {
                    next.push(neighbour);
                    if core.contains(&neighbour) {
                        near.insert(neighbour);
                    }
                }
            }
        }
        frontier = next;
    }
    let mut edges = Vec::new();
    for parent in near {
        let patch = patches.get(parent)?;
        for side in 0..3 {
            if transition.contains(&patch.neighbours[side]) {
                edges.push((patch.corners[(side + 1) % 3], patch.corners[side]));
            }
        }
    }
    Ok(branched_boundary_vertices(edges)
        .iter()
        .any(|vertex| corners.contains(vertex)))
}

fn preferred_boundary_segment(
    patches: &Patches<'_>,
    peel: &BTreeSet<TriangleAddress>,
    transition: &BTreeSet<TriangleAddress>,
    preferred: TriangleAddress,
) -> Option<BTreeSet<TriangleAddress>> {
    let anchors = patches
        .get(preferred)
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
        if patches
            .get(parent)
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
    patches: &Patches<'_>,
    core: &BTreeSet<TriangleAddress>,
) -> BTreeSet<TriangleAddress> {
    core.iter()
        .copied()
        .filter(|&parent| {
            patches
                .get(parent)
                .is_ok_and(|patch| patch.neighbours.iter().any(|p| !in_core(core, *p)))
        })
        .collect()
}

/// What promoting the core parents at the core's pinches did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PinchRepair {
    /// They left the core for the transition.
    Promoted,
    /// There is no pinch, or a parent at one lies deeper than `reach`.
    Nothing,
    /// Every core parent is at a pinch -- two parents touching at a corner,
    /// say -- so promoting them would leave nothing to coarsen.
    WholeCore,
}

/// Promotes the core parents at every vertex where the core touches itself.
/// With a `reach`, only when none of them lies deeper than it allows.
fn promote_pinched_core(
    patches: &Patches<'_>,
    core: &mut BTreeSet<TriangleAddress>,
    transition: &mut BTreeSet<TriangleAddress>,
    reach: Option<(&BTreeMap<TriangleAddress, usize>, usize)>,
) -> Result<PinchRepair, String> {
    let pinches = branched_boundary_vertices(coarse_boundary_edges(patches, core, transition)?);
    if pinches.is_empty() {
        return Ok(PinchRepair::Nothing);
    }
    let promoted = core
        .iter()
        .copied()
        .filter(|parent| {
            patches
                .get(*parent)
                .is_ok_and(|patch| patch.corners.iter().any(|corner| pinches.contains(corner)))
        })
        .collect::<BTreeSet<_>>();
    if promoted.is_empty() {
        return Ok(PinchRepair::Nothing);
    }
    if promoted.len() == core.len() {
        return Ok(PinchRepair::WholeCore);
    }
    if let Some((depths, deepest)) = reach {
        if !promoted
            .iter()
            .all(|parent| depths.get(parent).is_some_and(|&depth| depth <= deepest))
        {
            return Ok(PinchRepair::Nothing);
        }
    }
    promote_to_transition(core, transition, promoted);
    Ok(PinchRepair::Promoted)
}

fn retain_fine_at_pinches(
    patches: &Patches<'_>,
    core: &BTreeSet<TriangleAddress>,
    transition: &mut BTreeSet<TriangleAddress>,
) -> Result<bool, String> {
    let pinches = branched_boundary_vertices(fine_boundary_edges(patches, core, transition)?);
    if pinches.is_empty() {
        return Ok(false);
    }
    let retained = transition
        .iter()
        .copied()
        .filter(|parent| {
            patches.get(*parent).is_ok_and(|patch| {
                patch
                    .corners
                    .iter()
                    .chain(&patch.midpoints)
                    .any(|vertex| pinches.contains(vertex))
            })
        })
        .collect::<Vec<_>>();
    if retained.is_empty() {
        return Ok(false);
    }
    for parent in retained {
        transition.remove(&parent);
    }
    Ok(true)
}

fn branched_boundary_vertices(edges: Vec<(usize, usize)>) -> BTreeSet<usize> {
    let mut outgoing = BTreeMap::<usize, usize>::new();
    let mut incoming = BTreeMap::<usize, usize>::new();
    for (from, to) in edges {
        *outgoing.entry(from).or_default() += 1;
        *incoming.entry(to).or_default() += 1;
    }
    outgoing
        .into_iter()
        .chain(incoming)
        .filter_map(|(vertex, degree)| (degree > 1).then_some(vertex))
        .collect()
}

fn pure_core(
    patches: &Patches<'_>,
    component_id: u64,
    core: &BTreeSet<TriangleAddress>,
    halo_expansions: usize,
) -> TransitionTopologyOutcome {
    let source = patches.source;
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
    if let Err(reason) = hard_gate(source, &mesh) {
        return TransitionTopologyOutcome::ProvenInfeasible {
            states_examined: 0,
            halo_expansions,
            reason,
        };
    }
    let boundary = match boundary(patches, core, &BTreeSet::new()) {
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
            focus_topology_states: None,
            retired_vertex: false,
        },
    }))
}

/// A focused search (guide 11.130): the layout's configurations that keep
/// every custom parent off the focus as the failed candidate had it,
/// enumerated from the focus's cursor. The layout's own cursor stays where
/// it was; the candidate reports its focused states separately.
fn solve_focused(
    patches: &Patches<'_>,
    component_id: u64,
    core: &BTreeSet<TriangleAddress>,
    transition: &BTreeSet<TriangleAddress>,
    focus: &RetryFocus,
    layout_cursor: usize,
    remaining_states: usize,
) -> TransitionTopologyOutcome {
    match solve_once(
        patches,
        component_id,
        core.clone(),
        transition.clone(),
        0,
        focus.cursor,
        focus.cursor.saturating_add(remaining_states),
        Some(focus),
    ) {
        TransitionTopologyOutcome::Closed(mut trial) => {
            let focused = trial.report.topology_states;
            trial.candidate.topology_id += layout_cursor;
            trial.report.topology_states = layout_cursor;
            trial.report.layout_topology_states = layout_cursor;
            trial.report.focus_topology_states = Some(focused);
            TransitionTopologyOutcome::Closed(trial)
        }
        other => other,
    }
}

/// How a focus narrows a layout's search (`RetryFocus`).
struct FocusPlan {
    /// Groups of positions of which a configuration must change at least
    /// one each -- every free parent's chosen variant is first, so "changed"
    /// is "not variant 0".
    must_change: Vec<Vec<usize>>,
    /// Each free position's distance from the focus, for the search order.
    distance: Vec<Option<f64>>,
}

/// Narrows a layout's variants to a focus: a custom parent beyond the
/// focus's radius keeps only the failed candidate's triangles, and a free
/// one has them first.
fn focus_variants(
    source: &MotherGrid,
    custom_transition: &BTreeSet<TriangleAddress>,
    parent_patches: &BTreeMap<TriangleAddress, ParentPatch>,
    variants: &mut [Vec<Vec<[usize; 3]>>],
    focus: &RetryFocus,
) -> Result<FocusPlan, String> {
    let vertices = source.mesh.vertices();
    let mut at_failure = Vec::new();
    let mut beyond_previous = Vec::new();
    let mut distances = vec![None; custom_transition.len()];
    for (position, parent) in custom_transition.iter().enumerate() {
        let chosen = focus.chosen.get(parent).ok_or_else(|| {
            format!("the focused candidate has no triangles for transition parent {parent:?}")
        })?;
        let variant = variants[position]
            .iter()
            .position(|variant| variant == chosen)
            .ok_or_else(|| {
                format!(
                    "the focused candidate's triangles for transition parent {parent:?} are not \
                     among its variants"
                )
            })?;
        let [a, b, c] = parent_patches[parent]
            .corners
            .map(|corner| vertices[corner]);
        let centre = CartesianPoint::new(a.x + b.x + c.x, a.y + b.y + c.y, a.z + b.z + c.z);
        let distance = angle_between(centre, focus.point) / angle_between(a, b);
        if distance > focus.radius_edges {
            variants[position] = vec![variants[position][variant].clone()];
            continue;
        }
        let first = variants[position].remove(variant);
        variants[position].insert(0, first);
        distances[position] = Some(distance);
        if distance <= focus.change_radius_edges {
            at_failure.push(position);
        }
        if focus.previous_radius_edges > 0.0 && distance > focus.previous_radius_edges {
            beyond_previous.push(position);
        }
    }
    let mut must_change = vec![at_failure];
    if focus.previous_radius_edges > 0.0 {
        must_change.push(beyond_previous);
    }
    Ok(FocusPlan {
        must_change,
        distance: distances,
    })
}

/// A parent's edge length, as an angle on the sphere.
pub(super) fn parent_edge_angle(
    source: &MotherGrid,
    parent: TriangleAddress,
) -> Result<f64, String> {
    let corners = parent_patch(source, parent)?.corners;
    let vertices = source.mesh.vertices();
    Ok(angle_between(vertices[corners[0]], vertices[corners[1]]))
}

pub(super) fn angle_between(a: CartesianPoint, b: CartesianPoint) -> f64 {
    let dot = a.x * b.x + a.y * b.y + a.z * b.z;
    let norms = ((a.x * a.x + a.y * a.y + a.z * a.z) * (b.x * b.x + b.y * b.y + b.z * b.z)).sqrt();
    (dot / norms).clamp(-1.0, 1.0).acos()
}

#[allow(clippy::too_many_arguments)]
fn solve_once(
    patches: &Patches<'_>,
    component_id: u64,
    core: BTreeSet<TriangleAddress>,
    transition: BTreeSet<TriangleAddress>,
    halo_expansions: usize,
    start_index: usize,
    budget: usize,
    focus: Option<&RetryFocus>,
) -> TransitionTopologyOutcome {
    let source = patches.source;
    let mut states = 0usize;
    let mut leaf_set = match HierarchyLeafSet::from_mother_grid(source) {
        Ok(v) => v,
        Err(reason) => return invalid(states, halo_expansions, reason),
    };
    if let Err(reason) = leaf_set.condense_core(&core.iter().copied().collect::<Vec<_>>()) {
        return invalid(states, halo_expansions, reason);
    }
    let mut parent_patches = BTreeMap::<TriangleAddress, ParentPatch>::new();
    for &parent in core.iter().chain(&transition) {
        let patch = match patches.get(parent) {
            Ok(patch) => patch,
            Err(reason) => return invalid(states, halo_expansions, reason),
        };
        parent_patches.insert(parent, patch);
    }
    let custom_transition = transition
        .iter()
        .copied()
        .filter(|parent| {
            parent_patches[parent]
                .neighbours
                .iter()
                .any(|neighbour| in_core(&core, *neighbour))
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
        let patch = &parent_patches[parent];
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
    let plan = match focus
        .map(|focus| {
            focus_variants(
                source,
                &custom_transition,
                &parent_patches,
                &mut variants,
                focus,
            )
        })
        .transpose()
    {
        Ok(plan) => plan,
        Err(reason) => return invalid(states, halo_expansions, reason),
    };

    let boundary = match boundary(patches, &core, &transition) {
        Ok(boundary) => boundary,
        Err(reason) => return invalid(states, halo_expansions, reason),
    };
    let forecast = match base_degree_forecast(source, &core, &custom_transition, &parent_patches) {
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
        substrate_selection: None,
        enumeration_exhausted: &mut enumeration_exhausted,
        focus: plan.as_ref(),
    }
    .run();
    if let Some(hit) = closed {
        return closed_trial(
            component_id,
            &core,
            transition.len(),
            halo_expansions,
            boundary,
            hit,
            states,
        );
    }
    if !enumeration_exhausted {
        return TransitionTopologyOutcome::SearchBudgetExhausted {
            states_examined: states,
            halo_expansions,
        };
    }
    if focus.is_some() {
        return TransitionTopologyOutcome::FocusExhausted {
            states_examined: states,
            halo_expansions,
        };
    }

    let fixed_sources = fixed_boundary_sources(&boundary);
    let Some(base_hit) = select_retirement_substrate(
        source,
        &leaf_set,
        &custom_transition,
        &variants,
        &forecast,
        &fixed_sources,
        states,
    ) else {
        return TransitionTopologyOutcome::ProvenInfeasible {
            states_examined: states,
            halo_expansions,
            reason: "no transition substrate has repairable fixed custom-face angles".into(),
        };
    };
    match solve_retirement_family(
        source,
        component_id,
        &core,
        &transition,
        &leaf_set,
        boundary,
        &base_hit,
        states,
        start_index.saturating_sub(states),
        budget.saturating_sub(states),
        halo_expansions,
    ) {
        Some(outcome) => outcome,
        None => TransitionTopologyOutcome::ProvenInfeasible {
            states_examined: states,
            halo_expansions,
            reason: "no transition or retirement topology passed hard topology gates".into(),
        },
    }
}

fn closed_trial(
    component_id: u64,
    core: &BTreeSet<TriangleAddress>,
    transition_parent_count: usize,
    halo_expansions: usize,
    boundary: TransitionBoundary,
    hit: SearchHit,
    states: usize,
) -> TransitionTopologyOutcome {
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
            transition_parent_count,
            halo_expansions,
            topology_states: states,
            layout_topology_states: states,
            focus_topology_states: None,
            retired_vertex: hit.retired,
        },
    }))
}

#[allow(clippy::too_many_arguments)]
fn solve_retirement_family(
    source: &MotherGrid,
    component_id: u64,
    core: &BTreeSet<TriangleAddress>,
    transition: &BTreeSet<TriangleAddress>,
    base_leaf_set: &HierarchyLeafSet,
    boundary: TransitionBoundary,
    base_hit: &SearchHit,
    base_states: usize,
    start_index: usize,
    budget: usize,
    halo_expansions: usize,
) -> Option<TransitionTopologyOutcome> {
    if start_index >= budget {
        return Some(TransitionTopologyOutcome::SearchBudgetExhausted {
            states_examined: base_states + budget,
            halo_expansions,
        });
    }
    let fixed_sources = fixed_boundary_sources(&boundary);
    let eligible = retirement_candidates(&base_hit.mesh, &boundary, transition);
    let mut offset = 0usize;
    for (vertex, degree) in eligible {
        let block = retirement_block_size(degree)?;
        let local_start = start_index.saturating_sub(offset);
        if local_start >= block {
            offset += block;
            continue;
        }
        let local_budget = (budget - offset).min(block);
        let mut trial_mesh = base_hit.mesh.mesh.clone();
        let mut accepted = None;
        match trial_mesh.retire_vertex_from_cursor_with_budget_transactionally_repairing(
            vertex,
            local_start,
            local_budget,
            |candidate, report, _| match retirement_hit(
                source,
                transition,
                base_leaf_set,
                base_hit,
                candidate,
                report,
            ) {
                Ok(hit)
                    if fixed_custom_face_angles_are_repairable(
                        source,
                        &hit.triangles,
                        &fixed_sources,
                    ) =>
                {
                    accepted = Some(hit);
                    RetirementPostconditionOutcome::Accepted { states_examined: 0 }
                }
                Ok(_) | Err(_) => RetirementPostconditionOutcome::Rejected { states_examined: 0 },
            },
        ) {
            RetirementSearchOutcome::Committed { attempted, .. } => {
                return Some(closed_trial(
                    component_id,
                    core,
                    transition.len(),
                    halo_expansions,
                    boundary,
                    {
                        let mut hit =
                            accepted.expect("accepted retirement staged a transition hit");
                        hit.topology_id = base_states + offset + attempted - 1;
                        hit
                    },
                    base_states + offset + attempted,
                ));
            }
            RetirementSearchOutcome::SearchBudgetExhausted { attempted } => {
                return Some(TransitionTopologyOutcome::SearchBudgetExhausted {
                    states_examined: base_states + offset + attempted,
                    halo_expansions,
                });
            }
            RetirementSearchOutcome::ProvenInfeasible { attempted, .. } => {
                offset += attempted;
            }
            RetirementSearchOutcome::InvalidBoundary(error) => {
                return Some(invalid(
                    base_states + offset,
                    halo_expansions,
                    error.to_string(),
                ));
            }
        }
    }
    if offset >= budget {
        Some(TransitionTopologyOutcome::SearchBudgetExhausted {
            states_examined: base_states + budget,
            halo_expansions,
        })
    } else {
        Some(TransitionTopologyOutcome::ProvenInfeasible {
            states_examined: base_states + offset,
            halo_expansions,
            reason: "no transition or retirement topology passed hard topology gates".into(),
        })
    }
}

fn select_retirement_substrate(
    source: &MotherGrid,
    leaf_set: &HierarchyLeafSet,
    transition: &BTreeSet<TriangleAddress>,
    variants: &[Vec<Vec<[usize; 3]>>],
    forecast: &BTreeMap<usize, isize>,
    fixed_sources: &BTreeSet<usize>,
    base_states: usize,
) -> Option<SearchHit> {
    let mut states = 0;
    let mut closed = None;
    let mut substrate = None;
    let mut exhausted = false;
    ProductSearch {
        source,
        leaf_set,
        transition,
        variants,
        start_index: 0,
        budget: base_states,
        states: &mut states,
        forecast,
        closed: &mut closed,
        substrate_selection: Some(SubstrateSelection {
            fixed_sources,
            substrate: &mut substrate,
        }),
        enumeration_exhausted: &mut exhausted,
        focus: None,
    }
    .run();
    substrate
}

#[allow(clippy::too_many_arguments)]
fn retirement_hit(
    source: &MotherGrid,
    transition: &BTreeSet<TriangleAddress>,
    base_leaf_set: &HierarchyLeafSet,
    base_hit: &SearchHit,
    candidate: &MeshState,
    report: &earthmesh_mesh::RetirementReport,
) -> Result<SearchHit, String> {
    let mut affected = base_hit
        .triangles_by_parent
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    for &face in &report.fan {
        let Some(Some(address)) = base_hit.mesh.triangle_addresses.get(face).copied() else {
            continue;
        };
        let parent = address
            .parent_2_to_1()
            .ok_or_else(|| format!("retired face {face} address has no coarse parent"))?;
        if transition.contains(&parent) {
            affected.insert(parent);
        }
    }
    if affected.is_empty() {
        return Err("retirement affected no transition parents".into());
    }

    let mut leaf_set = base_leaf_set.clone();
    for parent in &affected {
        let children = parent
            .children_2_to_1()
            .ok_or_else(|| format!("invalid affected parent {parent:?}"))?;
        for child in children {
            leaf_set.leaves.remove(&child);
        }
    }

    let reused = report.reused_faces.iter().copied().collect::<BTreeSet<_>>();
    let mut triangles = Vec::new();
    for face in candidate.active_triangle_slots() {
        let address = base_hit
            .mesh
            .triangle_addresses
            .get(face)
            .copied()
            .flatten();
        let include = address.is_none()
            || address
                .and_then(TriangleAddress::parent_2_to_1)
                .is_some_and(|parent| affected.contains(&parent))
            || reused.contains(&face);
        if include {
            triangles.push(compact_triangle_to_source(
                &base_hit.mesh.source_vertex_slots,
                candidate.triangles()[face],
            )?);
        }
    }

    let custom_parents = affected.clone();
    let mesh = super::core_condensation::rebuild_from_leaf_set_with_custom_triangles(
        source,
        &leaf_set,
        &custom_parents,
        &triangles,
    )?;
    hard_gate(source, &mesh)?;

    let mut triangles_by_parent = BTreeMap::new();
    let first = *affected.first().expect("affected is non-empty");
    for parent in affected {
        // The key set carries affected-parent coverage; placing the flattened
        // custom region under one key avoids inventing per-parent ownership.
        triangles_by_parent.insert(
            parent,
            if parent == first {
                triangles.clone()
            } else {
                Vec::new()
            },
        );
    }
    let active_vertices = triangles
        .iter()
        .flat_map(|triangle| triangle.iter().copied())
        .collect::<BTreeSet<_>>();
    let forecast_vertices = active_vertices
        .iter()
        .copied()
        .chain(base_hit.mesh.source_vertex_slots[report.vertex]);
    let degree_forecast = actual_degree_forecast(&mesh, forecast_vertices);
    Ok(SearchHit {
        mesh,
        triangles_by_parent,
        triangles,
        degree_forecast,
        topology_id: 0,
        retired: true,
    })
}

fn compact_triangle_to_source(
    source_vertex_slots: &[Option<usize>],
    triangle: [usize; 3],
) -> Result<[usize; 3], String> {
    Ok([
        source_vertex_slots
            .get(triangle[0])
            .and_then(|source| *source)
            .ok_or_else(|| format!("compact vertex {} has no source slot", triangle[0]))?,
        source_vertex_slots
            .get(triangle[1])
            .and_then(|source| *source)
            .ok_or_else(|| format!("compact vertex {} has no source slot", triangle[1]))?,
        source_vertex_slots
            .get(triangle[2])
            .and_then(|source| *source)
            .ok_or_else(|| format!("compact vertex {} has no source slot", triangle[2]))?,
    ])
}

fn retirement_candidates(
    mesh: &HierarchyLeafMesh,
    boundary: &TransitionBoundary,
    transition: &BTreeSet<TriangleAddress>,
) -> Vec<(usize, usize)> {
    let blocked = boundary
        .fine_outer_cycles
        .iter()
        .chain(&boundary.coarse_inner_cycles)
        .flat_map(|cycle| cycle.iter().copied())
        .chain(boundary.seam.iter().copied())
        .chain(boundary.pentagon.iter().copied())
        .collect::<BTreeSet<_>>();
    // Collect degree and first incident face once, as in hard_gate. Scanning all
    // faces separately for each vertex makes global candidate selection quadratic.
    let mut incidence = vec![(0usize, 0usize); mesh.mesh.vertices().len()];
    for face in mesh.mesh.active_triangle_slots() {
        for vertex in mesh.mesh.triangles()[face] {
            let (degree, seed) = &mut incidence[vertex];
            *degree += 1;
            if *seed == 0 {
                *seed = face;
            }
        }
    }
    let mut vertices = mesh
        .mesh
        .active_vertex_slots()
        .filter_map(|vertex| {
            let source = mesh.source_vertex_slots.get(vertex).copied().flatten()?;
            let (degree, seed) = incidence[vertex];
            (!blocked.contains(&source)
                && (3..=7).contains(&degree)
                && retirement_fan_is_internal(mesh, vertex, seed, transition))
            .then_some((source, vertex, degree))
        })
        .collect::<Vec<_>>();
    vertices.sort_unstable();
    vertices
        .into_iter()
        .map(|(_, vertex, degree)| (vertex, degree))
        .collect()
}

fn retirement_fan_is_internal(
    mesh: &HierarchyLeafMesh,
    vertex: usize,
    seed: usize,
    transition: &BTreeSet<TriangleAddress>,
) -> bool {
    let Ok(fan) = mesh.mesh.triangle_fan_from(vertex, seed) else {
        return false;
    };
    fan.iter().all(
        |&face| match mesh.triangle_addresses.get(face).copied().flatten() {
            None => true,
            Some(address) => address
                .parent_2_to_1()
                .is_some_and(|parent| transition.contains(&parent)),
        },
    )
}

fn retirement_block_size(degree: usize) -> Option<usize> {
    Some(match degree {
        3 => 1,
        4 => 2,
        5 => 5,
        6 => 14,
        7 => 42,
        _ => return None,
    })
}

fn actual_degree_forecast(
    mesh: &HierarchyLeafMesh,
    source_vertices: impl Iterator<Item = usize>,
) -> BTreeMap<usize, usize> {
    let requested = source_vertices.collect::<BTreeSet<_>>();
    let mut out = requested
        .iter()
        .copied()
        .map(|source| (source, 0usize))
        .collect::<BTreeMap<_, _>>();
    let compact_to_source = mesh.source_vertex_slots.to_vec();
    for face in mesh.mesh.active_triangle_slots() {
        for compact in mesh.mesh.triangles()[face] {
            let Some(source) = compact_to_source.get(compact).and_then(|source| *source) else {
                continue;
            };
            if let Some(degree) = out.get_mut(&source) {
                *degree += 1;
            }
        }
    }
    out
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
    substrate_selection: Option<SubstrateSelection<'a>>,
    enumeration_exhausted: &'a mut bool,
    /// A focused search's plan (`focus_variants`): its order, and the
    /// configurations it counts but never offers.
    focus: Option<&'a FocusPlan>,
}

struct SubstrateSelection<'a> {
    fixed_sources: &'a BTreeSet<usize>,
    substrate: &'a mut Option<SearchHit>,
}

#[derive(Clone)]
struct SearchHit {
    mesh: HierarchyLeafMesh,
    triangles_by_parent: BTreeMap<TriangleAddress, Vec<[usize; 3]>>,
    triangles: Vec<[usize; 3]>,
    degree_forecast: BTreeMap<usize, usize>,
    topology_id: usize,
    /// A retirement family's candidate: its triangles are no parent's
    /// variant, so no focus can be built on it.
    retired: bool,
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
        let mut gate_failures = 0usize;

        let mut forecast = DenseForecast::new(self.source.mesh.vertices().len(), self.forecast);
        let mut chosen = vec![None; self.variants.len()];
        let transition = self.transition.iter().copied().collect::<Vec<_>>();
        let (variables, preassigned_touched) = search_variables(
            self.variants,
            &transition,
            &mut chosen,
            self.focus.map(|plan| plan.distance.as_slice()),
        );
        for position in chosen
            .iter()
            .enumerate()
            .filter_map(|(position, chosen)| chosen.map(|_| position))
        {
            forecast.apply_triangles(&self.variants[position][0], 1);
        }
        let suffix_masks = SuffixDegreeMasks::new(&variables);
        if !forecast.can_finish_all(&preassigned_touched, &suffix_masks, 0) {
            *self.enumeration_exhausted = true;
            *self.states = 0;
            return;
        }

        let mut indices = vec![0usize; variables.len()];
        let mut position = 0usize;
        let mut feasible_ordinal = 0usize;
        // ponytail: bound pruned prefix work at 64 tries per variable per
        // public topology ordinal; cursors still need to traverse skipped ordinals.
        let mut remaining_work = self
            .budget
            .saturating_mul(variables.len().max(1))
            .saturating_mul(64);
        // Where the work went when it runs out, for the timing log: the
        // deepest position the search reached, and how often each position
        // ran out of choices.
        let mut deepest = 0usize;
        let mut dead_ends = vec![0u32; variables.len()];

        loop {
            if position == variables.len() {
                let touched = touched_vertices(&variables, &preassigned_touched);
                if forecast.can_finish_all(&touched, &suffix_masks, variables.len()) {
                    if feasible_ordinal >= self.budget {
                        *self.states = self.budget;
                        return;
                    }
                    if self.substrate_selection.is_none() && feasible_ordinal < self.start_index {
                        feasible_ordinal = feasible_ordinal.saturating_add(1);
                        if !backtrack(&mut position, &mut forecast, &mut chosen, &variables) {
                            *self.states = feasible_ordinal;
                            *self.enumeration_exhausted = true;
                            return;
                        }
                        continue;
                    }
                    if self.focus.is_some_and(|plan| {
                        plan.must_change
                            .iter()
                            .any(|group| group.iter().all(|&slot| chosen[slot] == Some(0)))
                    }) {
                        feasible_ordinal = feasible_ordinal.saturating_add(1);
                        if !backtrack(&mut position, &mut forecast, &mut chosen, &variables) {
                            *self.states = feasible_ordinal;
                            *self.enumeration_exhausted = true;
                            return;
                        }
                        continue;
                    }
                    let chosen_by_parent = self.chosen_by_parent(&chosen);
                    let chosen_triangles = flatten_custom_triangles(&chosen_by_parent);
                    let rebuilt =
                        super::core_condensation::rebuild_from_leaf_set_with_custom_triangles(
                            self.source,
                            self.leaf_set,
                            self.transition,
                            &chosen_triangles,
                        );
                    // Why a degree-feasible state was turned down: the
                    // first few per search, for the timing log.
                    let gate = rebuilt
                        .as_ref()
                        .map_err(|reason| format!("rebuild: {reason}"))
                        .and_then(|mesh| hard_gate(self.source, mesh));
                    if let Err(reason) = &gate {
                        if crate::construction::cmrc_timing_enabled() && gate_failures < 3 {
                            gate_failures += 1;
                            eprintln!(
                                "earthmesh_cli: cmrc_detail phase=hard_gate_failure state={feasible_ordinal} \
                                 transition_parents={} reason={}",
                                self.transition.len(),
                                reason.chars().take(400).collect::<String>()
                            );
                        }
                    }
                    if let (Ok(mesh), Ok(())) = (rebuilt, gate) {
                        let hit = SearchHit {
                            mesh,
                            triangles_by_parent: chosen_by_parent,
                            triangles: chosen_triangles,
                            degree_forecast: forecast.to_map(self.forecast),
                            topology_id: feasible_ordinal,
                            retired: false,
                        };
                        if self.substrate_selection.is_some() {
                            let selected = self.consider_retirement_substrate(&hit);
                            feasible_ordinal = feasible_ordinal.saturating_add(1);
                            if selected {
                                *self.states = feasible_ordinal;
                                return;
                            }
                            if feasible_ordinal >= self.budget {
                                *self.states = self.budget;
                                *self.enumeration_exhausted = true;
                                return;
                            }
                            if !backtrack(&mut position, &mut forecast, &mut chosen, &variables) {
                                *self.states = feasible_ordinal;
                                *self.enumeration_exhausted = true;
                                return;
                            }
                            continue;
                        }
                        if feasible_ordinal >= self.start_index {
                            *self.states = feasible_ordinal.saturating_add(1);
                            *self.closed = Some(hit);
                            return;
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
                dead_ends[position] = dead_ends[position].saturating_add(1);
                if !backtrack(&mut position, &mut forecast, &mut chosen, &variables) {
                    *self.states = feasible_ordinal;
                    *self.enumeration_exhausted = true;
                    return;
                }
                continue;
            }

            let choice_index = indices[position];
            indices[position] += 1;
            if remaining_work == 0 {
                if crate::construction::cmrc_timing_enabled() {
                    let parent = |position: usize| {
                        variables
                            .get(position)
                            .map(|variable| transition[variable.original_position])
                    };
                    let (worst, worst_dead_ends) = dead_ends
                        .iter()
                        .enumerate()
                        .max_by_key(|&(position, &count)| (count, Reverse(position)))
                        .map_or((0, 0), |(position, &count)| (position, count));
                    eprintln!(
                        "earthmesh_cli: cmrc_detail phase=search_work_exhausted variables={} \
                         deepest={deepest} deepest_parent={:?} dead_ends_at_deepest={} \
                         most_dead_ends={worst_dead_ends} at={worst} parent={:?} states={feasible_ordinal}",
                        variables.len(),
                        parent(deepest),
                        dead_ends.get(deepest).copied().unwrap_or(0),
                        parent(worst)
                    );
                }
                *self.states = self.budget;
                return;
            }
            remaining_work -= 1;
            let variable = &variables[position];
            let choice = &variable.variants[choice_index];
            forecast.apply_delta(&choice.delta, 1);
            if forecast.can_finish_all(&variable.touched, &suffix_masks, position + 1) {
                chosen[variable.original_position] = Some(choice.variant_index);
                position += 1;
                deepest = deepest.max(position);
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

    fn consider_retirement_substrate(&mut self, hit: &SearchHit) -> bool {
        let Some(selection) = &mut self.substrate_selection else {
            return false;
        };
        if !fixed_custom_face_angles_are_repairable(
            self.source,
            &hit.triangles,
            selection.fixed_sources,
        ) {
            return false;
        }
        *selection.substrate = Some(hit.clone());
        true
    }
}

fn fixed_boundary_sources(boundary: &TransitionBoundary) -> BTreeSet<usize> {
    boundary
        .fine_outer_cycles
        .iter()
        .chain(&boundary.coarse_inner_cycles)
        .flat_map(|cycle| cycle.iter().copied())
        .chain(boundary.seam.iter().copied())
        .chain(boundary.pentagon.iter().copied())
        .collect()
}

fn fixed_custom_face_angles_are_repairable(
    source: &MotherGrid,
    triangles: &[[usize; 3]],
    fixed_sources: &BTreeSet<usize>,
) -> bool {
    for triangle in triangles {
        if !triangle.iter().all(|site| fixed_sources.contains(site)) {
            continue;
        }
        let Some(angles) =
            spherical_triangle_angles(triangle.map(|site| source.mesh.vertices()[site]))
        else {
            return false;
        };
        if angles
            .into_iter()
            .any(|angle| !(40.2..=79.8).contains(&angle))
        {
            return false;
        }
    }
    true
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

    /// Whether every vertex can still reach a valid degree with what the
    /// variables from `position` on may add to it.
    fn can_finish_all(
        &self,
        vertices: &[usize],
        suffix_masks: &SuffixDegreeMasks,
        position: usize,
    ) -> bool {
        vertices.iter().copied().all(|vertex| {
            degree_mask_can_finish(self.degrees[vertex], suffix_masks.mask(vertex, position))
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

/// The search's variables and the vertices fixed parents touch. A focused
/// search (`distance`) sets its free parents from the farthest to the
/// nearest; otherwise `greedy_order` decides.
fn search_variables(
    variants: &[Vec<Vec<[usize; 3]>>],
    parents: &[TriangleAddress],
    chosen: &mut [Option<usize>],
    distance: Option<&[Option<f64>]>,
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
    let order = match distance {
        None => greedy_order(&pending, parents, &fixed_touched),
        Some(distance) => {
            let mut order = (0..pending.len()).collect::<Vec<_>>();
            let key = |index: usize| distance[pending[index].original_position].unwrap_or(0.0);
            order.sort_by(|&left, &right| key(right).total_cmp(&key(left)).then(left.cmp(&right)));
            order
        }
    };
    let mut slots = pending.into_iter().map(Some).collect::<Vec<_>>();
    let ordered = order
        .into_iter()
        .map(|index| slots[index].take().expect("each variable is ordered once"))
        .collect();
    (ordered, fixed_touched.into_iter().collect())
}

/// The order the search sets its variables in: again and again the one with
/// the most vertices already touched, then the most vertices, then the lowest
/// parent. A variable's count changes only when one of its vertices joins the
/// touched set, so the counts are kept per vertex and the best is the last of
/// an ordered set: n log n, where rescanning every pending variable for each
/// pick cost a 40 km component 110 s a search (guide 11.125).
fn greedy_order(
    pending: &[SearchVariable],
    parents: &[TriangleAddress],
    fixed_touched: &BTreeSet<usize>,
) -> Vec<usize> {
    let mut touching = HashMap::<usize, Vec<usize>>::new();
    for (index, variable) in pending.iter().enumerate() {
        for &vertex in &variable.touched {
            touching.entry(vertex).or_default().push(index);
        }
    }
    let key = |index: usize, shared: usize| {
        let variable = &pending[index];
        (
            shared,
            variable.touched.len(),
            Reverse(parents[variable.original_position]),
            index,
        )
    };
    let mut shared = pending
        .iter()
        .map(|variable| shared_count(&variable.touched, fixed_touched))
        .collect::<Vec<_>>();
    let mut queue = (0..pending.len())
        .map(|index| key(index, shared[index]))
        .collect::<BTreeSet<_>>();
    let mut frontier = fixed_touched.clone();
    let mut order = Vec::with_capacity(pending.len());
    while let Some((_, _, _, index)) = queue.pop_last() {
        order.push(index);
        for &vertex in &pending[index].touched {
            if !frontier.insert(vertex) {
                continue;
            }
            for &other in touching.get(&vertex).map_or(&[][..], Vec::as_slice) {
                // A variable already ordered is no longer queued.
                if queue.remove(&key(other, shared[other])) {
                    shared[other] += 1;
                    queue.insert(key(other, shared[other]));
                }
            }
        }
    }
    order
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

/// What the variables from each position on may add to each vertex's
/// degree, as bit masks (bit k: k more faces), kept per vertex (guide 11.128).
/// Each vertex is touched by a handful of variables, so a vertex keeps the
/// positions of those and the convolution of their masks from each one to
/// the end. Merging every suffix into one list per position cost the square
/// of the variables: a 100 km component held 400 GB and searched for 10
/// minutes in them.
struct SuffixDegreeMasks {
    /// Vertex -> (position of a variable touching it, the convolution of
    /// its mask and every later touching variable's), positions ascending.
    by_vertex: HashMap<usize, Vec<(usize, u128)>>,
}

impl SuffixDegreeMasks {
    fn new(variables: &[SearchVariable]) -> Self {
        let mut by_vertex = HashMap::<usize, Vec<(usize, u128)>>::new();
        for (position, variable) in variables.iter().enumerate() {
            for (vertex, mask) in local_degree_masks(variable) {
                by_vertex.entry(vertex).or_default().push((position, mask));
            }
        }
        for touching in by_vertex.values_mut() {
            let mut suffix = 1u128;
            for (_, mask) in touching.iter_mut().rev() {
                suffix = convolve_degree_masks(*mask, suffix);
                *mask = suffix;
            }
        }
        Self { by_vertex }
    }

    /// What the variables at `position` and after may add to `vertex`: the
    /// convolution of their masks, 1 (nothing) when none touches it.
    fn mask(&self, vertex: usize, position: usize) -> u128 {
        let Some(touching) = self.by_vertex.get(&vertex) else {
            return 1;
        };
        let first = touching.partition_point(|&(at, _)| at < position);
        touching.get(first).map_or(1, |&(_, mask)| mask)
    }
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
    // Each child's slot once: the neighbours below read the same four.
    let mut child_slots = [0usize; 4];
    let mut child_triangles = [[0usize; 3]; 4];
    for (index, child) in children.into_iter().enumerate() {
        child_slots[index] = source_face_slot(source, child)?;
        child_triangles[index] = source.mesh.triangles()[child_slots[index]];
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
    let mut edges = [(0usize, 0usize); 12];
    let mut sites = [0usize; 12];
    for (index, t) in child_triangles.iter().enumerate() {
        for corner in 0..3 {
            edges[3 * index + corner] = edge(t[corner], t[(corner + 1) % 3]);
            sites[3 * index + corner] = t[corner];
        }
    }
    // Ascending: the first qualifying site is taken. A site repeats, which
    // never changes which one that is.
    sites.sort_unstable();
    let mut midpoints = [0usize; 3];
    for side in 0..3 {
        let a = corners[side];
        let b = corners[(side + 1) % 3];
        midpoints[side] = sites
            .iter()
            .copied()
            .find(|&m| {
                m != a && m != b && edges.contains(&edge(a, m)) && edges.contains(&edge(m, b))
            })
            .ok_or_else(|| format!("parent {parent:?} side {side} has no exact midpoint"))?;
    }
    let mut neighbours = [parent; 3];
    for side in 0..3 {
        neighbours[side] = neighbour_parent(
            source,
            parent,
            &child_slots,
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
    child_slots: &[usize; 4],
    a: usize,
    midpoint: usize,
    b: usize,
) -> Result<TriangleAddress, String> {
    let mut neighbours = [None; 2];
    for (target, claimed) in [edge(a, midpoint), edge(midpoint, b)]
        .into_iter()
        .zip(&mut neighbours)
    {
        let mut found = None;
        for &slot in child_slots {
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
                    // A built region's edge: the settled parent beyond it.
                    if source.region.is_some() {
                        found = Some(TriangleAddress::outside(parent.n));
                        continue;
                    }
                    return Err(format!(
                        "parent {parent:?} boundary segment {target:?} is open"
                    ));
                }
                found =
                    source.triangle_addresses[neighbour].and_then(TriangleAddress::parent_2_to_1);
            }
        }
        *claimed = Some(found.ok_or_else(|| {
            format!("parent {parent:?} boundary segment {target:?} has no neighbour")
        })?);
    }
    let [Some(first), Some(second)] = neighbours else {
        unreachable!("both segments claimed or returned");
    };
    if first != second {
        return Err(format!(
            "parent {parent:?} coarse side has inconsistent fine neighbours"
        ));
    }
    let neighbour = first;
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
        if !in_core(core, patch.neighbours[side]) {
            polygon.push(patch.midpoints[side]);
        }
    }
    polygon.dedup();
    if polygon.first() == polygon.last() {
        polygon.pop();
    }
    polygon
}

pub(super) fn triangulations(polygon: &[usize]) -> Vec<Vec<[usize; 3]>> {
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
        let left_penalty = candidate_angle_penalty(source, left);
        let right_penalty = candidate_angle_penalty(source, right);
        let left_reuse = left
            .iter()
            .filter(|triangle| original.contains(&canonical_triangle(**triangle)))
            .count();
        let right_reuse = right
            .iter()
            .filter(|triangle| original.contains(&canonical_triangle(**triangle)))
            .count();
        left_penalty
            .total_cmp(&right_penalty)
            .then_with(|| right_reuse.cmp(&left_reuse))
            .then_with(|| canonical_candidate(left).cmp(&canonical_candidate(right)))
    });
    variants.dedup_by(|left, right| canonical_candidate(left) == canonical_candidate(right));
    variants
}

fn candidate_angle_penalty(source: &MotherGrid, candidate: &[[usize; 3]]) -> f64 {
    candidate
        .iter()
        .flat_map(|triangle| {
            spherical_triangle_angles(triangle.map(|site| source.mesh.vertices()[site]))
                .unwrap_or([f64::INFINITY; 3])
        })
        .map(|angle| {
            if angle < 40.2 {
                (40.2 - angle).powi(2)
            } else if angle > 79.8 {
                (angle - 79.8).powi(2)
            } else {
                0.0
            }
        })
        .sum()
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
    patches: &Patches<'_>,
    core: &BTreeSet<TriangleAddress>,
    transition: &BTreeSet<TriangleAddress>,
) -> Result<TransitionBoundary, String> {
    let source = patches.source;
    let coarse_edges = coarse_boundary_edges(patches, core, transition)?;
    let fine_edges = fine_boundary_edges(patches, core, transition)?;
    let coarse = cycles_from_edges(coarse_edges)
        .map_err(|reason| format!("coarse inner boundary: {reason}"))?;
    let fine =
        cycles_from_edges(fine_edges).map_err(|reason| format!("fine outer boundary: {reason}"))?;
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

fn coarse_boundary_edges(
    patches: &Patches<'_>,
    core: &BTreeSet<TriangleAddress>,
    transition: &BTreeSet<TriangleAddress>,
) -> Result<Vec<(usize, usize)>, String> {
    let mut edges = Vec::new();
    for &parent in core {
        let patch = patches.get(parent)?;
        for side in 0..3 {
            if transition.contains(&patch.neighbours[side]) {
                edges.push((patch.corners[(side + 1) % 3], patch.corners[side]));
            }
        }
    }
    Ok(edges)
}

fn fine_boundary_edges(
    patches: &Patches<'_>,
    core: &BTreeSet<TriangleAddress>,
    transition: &BTreeSet<TriangleAddress>,
) -> Result<Vec<(usize, usize)>, String> {
    let mut edges = Vec::new();
    for &parent in transition {
        let patch = patches.get(parent)?;
        for side in 0..3 {
            if !in_core(core, patch.neighbours[side])
                && !transition.contains(&patch.neighbours[side])
            {
                edges.extend([
                    (patch.corners[side], patch.midpoints[side]),
                    (patch.midpoints[side], patch.corners[(side + 1) % 3]),
                ]);
            }
        }
    }
    Ok(edges)
}

pub(super) fn cycles_from_edges(edges: Vec<(usize, usize)>) -> Result<Vec<Vec<usize>>, String> {
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

fn hard_gate(source: &MotherGrid, mesh: &HierarchyLeafMesh) -> Result<(), String> {
    let state = &mesh.mesh;
    state.validate().map_err(|errors| {
        errors
            .into_iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    // A built region's mesh is open along the region's edge, and only there;
    // vertices there have open fans and are settled by construction.
    let outer = source.region.as_ref().map(|region| region.outer_boundary());
    let on_edge = |vertex: usize| {
        outer.is_some_and(|outer| {
            mesh.source_vertex_slots
                .get(vertex)
                .copied()
                .flatten()
                .is_some_and(|slot| outer.contains(&slot))
        })
    };
    if outer.is_none() && state.open_edge_count() != 0 {
        return Err(format!("mesh has {} open edges", state.open_edge_count()));
    }
    if outer.is_some() {
        for face in state.active_triangle_slots() {
            let corners = state.triangles()[face];
            for (corner, &neighbour) in state.neighbours()[face].iter().enumerate() {
                let (a, b) = (corners[(corner + 1) % 3], corners[(corner + 2) % 3]);
                if neighbour == 0 && !(on_edge(a) && on_edge(b)) {
                    return Err(format!("edge ({a}, {b}) is open inside the region"));
                }
            }
        }
    }
    // Membership only; triangle-order first-error reporting stays deterministic.
    let mut degrees = vec![0usize; state.vertices().len()];
    let mut seeds = vec![0usize; state.vertices().len()];
    let mut triangles = HashSet::new();
    for face in state.active_triangle_slots() {
        let tri = state.triangles()[face];
        if orientation_on_sphere(
            state.vertices()[tri[0]],
            state.vertices()[tri[1]],
            state.vertices()[tri[2]],
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
        for vertex in tri {
            degrees[vertex] += 1;
            if seeds[vertex] == 0 {
                seeds[vertex] = face;
            }
        }
    }
    // All callers rebuild adjacency from canonical edge claims. After validation
    // and the closed-edge check above, each edge has exactly two face claims:
    // 3F = 2E. Reuse that invariant instead of hashing every edge again.
    let faces = state.triangle_count();
    if outer.is_none() {
        let edges = faces * 3 / 2;
        let euler = state.vertex_count() as isize - edges as isize + faces as isize;
        if euler != 2 {
            return Err(format!("Euler characteristic is {euler}, expected 2"));
        }
    } else {
        // A region keeps its own surface: re-triangulating it keeps the Euler
        // characteristic of the built level grid it was condensed from.
        let euler_of = |state: &MeshState| {
            let faces = state.triangle_count();
            let edges = (3 * faces + state.open_edge_count()) / 2;
            state.vertex_count() as isize - edges as isize + faces as isize
        };
        let (euler, expected) = (euler_of(state), euler_of(&source.mesh));
        if euler != expected {
            return Err(format!(
                "Euler characteristic is {euler}, expected the region's {expected}"
            ));
        }
    }
    if let Some((vertex, degree)) = degrees
        .iter()
        .copied()
        .enumerate()
        .find(|&(vertex, degree)| degree != 0 && !on_edge(vertex) && !(5..=7).contains(&degree))
    {
        return Err(format!("vertex {vertex} degree {degree} outside 5..=7"));
    }
    for (vertex, source_slot) in mesh.source_vertex_slots.iter().copied().enumerate() {
        if source_slot.is_some_and(|source_slot| {
            matches!(
                source.addresses.get(source_slot).and_then(Option::as_ref),
                Some(VertexAddress::IcosahedronVertex(_))
            )
        }) && !on_edge(vertex)
            && degrees[vertex] != 5
        {
            return Err(format!(
                "protected icosahedron vertex {vertex} has degree {}, expected 5",
                degrees[vertex]
            ));
        }
    }
    for vertex in state
        .active_vertex_slots()
        .filter(|&vertex| !on_edge(vertex))
    {
        let fan = state
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
    use crate::coarsen::ElasticPatch;

    #[test]
    fn hard_gate_preserves_topology_checks_and_first_failure() {
        let source = MotherGrid::generate(2).unwrap();
        let with_triangles = |triangles: Vec<[usize; 3]>| HierarchyLeafMesh {
            triangle_addresses: vec![None; triangles.len()],
            source_vertex_slots: (0..source.mesh.vertices().len()).map(Some).collect(),
            mesh: MeshState::from_parts(source.mesh.vertices().to_vec(), triangles).unwrap(),
        };
        let valid = with_triangles(source.mesh.triangles().to_vec());
        assert_eq!(hard_gate(&source, &valid), Ok(()));

        let mut open = source.mesh.triangles().to_vec();
        open.pop();
        assert_eq!(
            hard_gate(&source, &with_triangles(open)),
            Err("mesh has 3 open edges".into())
        );

        let triangle = source.mesh.triangles()[2];
        let mut reversed = source.mesh.triangles().to_vec();
        reversed[2].swap(0, 1);
        assert_eq!(
            hard_gate(&source, &with_triangles(reversed)),
            Err("triangle 2 is not positively oriented".into())
        );

        // Each pair closes its own three edges. The first face-level error
        // must win, independent of how membership sets are implemented.
        let mut other = source
            .mesh
            .active_triangle_slots()
            .map(|face| source.mesh.triangles()[face])
            .find(|candidate| candidate.iter().all(|site| !triangle.contains(site)))
            .unwrap();
        other.swap(0, 1);
        let mut canonical = triangle;
        canonical.sort_unstable();
        assert_eq!(
            hard_gate(
                &source,
                &with_triangles(vec![[1; 3], [1; 3], triangle, triangle, other, other])
            ),
            Err(format!("duplicate triangle {canonical:?}"))
        );
        assert_eq!(
            hard_gate(
                &source,
                &with_triangles(vec![[1; 3], [1; 3], other, other, triangle, triangle])
            ),
            Err("triangle 2 is not positively oriented".into())
        );
    }

    #[test]
    fn closed_topology_euler_count_matches_unique_edges() {
        let assert_edge_count = |mesh: &MeshState| {
            mesh.validate().unwrap();
            assert_eq!(mesh.open_edge_count(), 0);
            let edges = mesh
                .active_triangle_slots()
                .flat_map(|face| {
                    let [a, b, c] = mesh.triangles()[face];
                    [edge(a, b), edge(b, c), edge(c, a)]
                })
                .collect::<BTreeSet<_>>();
            assert_eq!(mesh.triangle_count() * 3, edges.len() * 2);
        };
        for n in [2, 4, 8] {
            let source = MotherGrid::generate(n).unwrap();
            assert_edge_count(&source.mesh);
            let mut sparse = source.mesh.clone();
            assert!(matches!(
                sparse.retire_vertex_from_cursor_with_budget_transactionally_repairing(
                    2,
                    0,
                    100,
                    |_, _, _| RetirementPostconditionOutcome::Accepted { states_examined: 0 },
                ),
                RetirementSearchOutcome::Committed { .. }
            ));
            assert!(sparse.triangle_count() + 2 < sparse.triangles().len());
            assert_edge_count(&sparse);
            let mut flipped = 0;
            for face in source.mesh.active_triangle_slots().take(12) {
                let mut changed = source.mesh.clone();
                if changed.flip_edge(face, 0).is_ok() {
                    flipped += 1;
                    assert_edge_count(&changed);
                }
            }
            assert!(flipped > 0);
        }

        // Two closed components still satisfy 3F = 2E, but must fail Euler.
        let source = MotherGrid::generate(2).unwrap();
        let mut vertices = source.mesh.vertices().to_vec();
        let offset = vertices.len() - 2;
        vertices.extend_from_slice(&source.mesh.vertices()[2..]);
        let mut triangles = source.mesh.triangles().to_vec();
        triangles.extend(
            source
                .mesh
                .active_triangle_slots()
                .map(|face| source.mesh.triangles()[face].map(|site| site + offset)),
        );
        let mesh = HierarchyLeafMesh {
            source_vertex_slots: vec![None; vertices.len()],
            triangle_addresses: vec![None; triangles.len()],
            mesh: MeshState::from_parts(vertices, triangles).unwrap(),
        };
        assert_edge_count(&mesh.mesh);
        assert_eq!(
            hard_gate(&source, &mesh),
            Err("Euler characteristic is 4, expected 2".into())
        );
    }

    // Reference the original full scans, independently of the optimized path.
    fn scan_retirement_candidates(
        mesh: &HierarchyLeafMesh,
        boundary: &TransitionBoundary,
        transition: &BTreeSet<TriangleAddress>,
    ) -> Vec<(usize, usize)> {
        let blocked = boundary
            .fine_outer_cycles
            .iter()
            .chain(&boundary.coarse_inner_cycles)
            .flat_map(|cycle| cycle.iter().copied())
            .chain(boundary.seam.iter().copied())
            .chain(boundary.pentagon.iter().copied())
            .collect::<BTreeSet<_>>();
        let mut candidates = Vec::new();
        for vertex in mesh.mesh.active_vertex_slots() {
            let Some(source) = mesh.source_vertex_slots.get(vertex).copied().flatten() else {
                continue;
            };
            if blocked.contains(&source) {
                continue;
            }
            let degree = mesh
                .mesh
                .active_triangle_slots()
                .filter(|&face| mesh.mesh.triangles()[face].contains(&vertex))
                .count();
            if !(3..=7).contains(&degree) {
                continue;
            }
            let seed = mesh
                .mesh
                .active_triangle_slots()
                .find(|&face| mesh.mesh.triangles()[face].contains(&vertex))
                .unwrap();
            let Ok(fan) = mesh.mesh.triangle_fan_from(vertex, seed) else {
                continue;
            };
            if fan.iter().all(
                |&face| match mesh.triangle_addresses.get(face).copied().flatten() {
                    None => true,
                    Some(address) => address
                        .parent_2_to_1()
                        .is_some_and(|parent| transition.contains(&parent)),
                },
            ) {
                candidates.push((source, vertex, degree));
            }
        }
        candidates.sort_unstable();
        candidates
            .into_iter()
            .map(|(_, vertex, degree)| (vertex, degree))
            .collect()
    }

    fn assert_retirement_candidates_match_scan(
        mesh: &HierarchyLeafMesh,
        boundary: &TransitionBoundary,
        transition: &BTreeSet<TriangleAddress>,
    ) {
        let actual = retirement_candidates(mesh, boundary, transition);
        assert_eq!(
            actual,
            scan_retirement_candidates(mesh, boundary, transition)
        );
    }

    #[test]
    fn retirement_candidates_preserve_scan_semantics() {
        let (_, transition, _, hit, boundary) = retirement_family_fixture();
        assert!(!scan_retirement_candidates(&hit.mesh, &boundary, &transition).is_empty());
        assert_retirement_candidates_match_scan(&hit.mesh, &boundary, &transition);
        assert_retirement_candidates_match_scan(&hit.mesh, &boundary, &BTreeSet::new());

        let mut mesh = hit.mesh;
        // Custom faces are eligible even without a hierarchy transition address.
        mesh.triangle_addresses.fill(None);
        // Sorting and boundary protection use source IDs, not compact slots.
        let slots = mesh.source_vertex_slots.len();
        for (vertex, source) in mesh.source_vertex_slots.iter_mut().enumerate() {
            *source = Some(slots - vertex);
        }
        let candidates = scan_retirement_candidates(&mesh, &boundary, &transition);
        assert!(candidates.len() > 5);
        assert!(candidates.windows(2).all(|pair| pair[0].0 > pair[1].0));
        let sources = candidates
            .iter()
            .take(5)
            .map(|&(vertex, _)| mesh.source_vertex_slots[vertex].unwrap())
            .collect::<Vec<_>>();
        let blocked = TransitionBoundary {
            fine_outer_cycles: vec![vec![sources[0]]],
            coarse_inner_cycles: vec![vec![sources[1]]],
            seam: vec![sources[2]],
            pentagon: vec![sources[3]],
            ..TransitionBoundary::default()
        };
        mesh.source_vertex_slots[candidates[4].0] = None;
        assert_retirement_candidates_match_scan(&mesh, &blocked, &transition);
        let filtered = scan_retirement_candidates(&mesh, &blocked, &transition);
        assert_eq!(filtered.len(), candidates.len() - 5);
        mesh.source_vertex_slots.truncate(slots / 2);
        assert_retirement_candidates_match_scan(&mesh, &blocked, &transition);

        // Missing incident faces create open fans; an unused vertex has degree 0.
        let mut vertices = mesh.mesh.vertices().to_vec();
        vertices.push(vertices[2]);
        let mut triangles = mesh.mesh.triangles().to_vec();
        triangles.pop();
        mesh.mesh = MeshState::from_parts(vertices, triangles).unwrap();
        mesh.source_vertex_slots = (0..mesh.mesh.vertices().len()).map(Some).collect();
        assert_retirement_candidates_match_scan(&mesh, &boundary, &transition);
    }

    #[test]
    #[ignore = "manual release-mode timing of candidate enumeration, not full mesh construction"]
    fn retirement_candidate_selection_scaling() {
        for n in [16, 32, 64] {
            let source = MotherGrid::generate(n).unwrap();
            let source_vertex_slots = (0..source.mesh.vertices().len()).map(Some).collect();
            let mesh = HierarchyLeafMesh {
                mesh: source.mesh,
                triangle_addresses: source.triangle_addresses,
                source_vertex_slots,
            };
            let boundary = TransitionBoundary::default();
            let transition = mesh
                .triangle_addresses
                .iter()
                .flatten()
                .filter_map(|address| address.parent_2_to_1())
                .collect();
            let start = std::time::Instant::now();
            let expected = scan_retirement_candidates(&mesh, &boundary, &transition);
            let scan = start.elapsed();
            let start = std::time::Instant::now();
            let actual = retirement_candidates(&mesh, &boundary, &transition);
            let elapsed = start.elapsed();
            assert_eq!(actual, expected);
            eprintln!("retirement_candidates n={n} vertices={} triangles={} scan_ms={:.3} candidate_ms={:.3} speedup={:.1}x",
                mesh.mesh.vertex_count(), mesh.mesh.triangle_count(),
                scan.as_secs_f64() * 1000., elapsed.as_secs_f64() * 1000.,
                scan.as_secs_f64() / elapsed.as_secs_f64());
        }
    }

    #[test]
    fn finite_polygon_enumeration_has_the_catalan_counts() {
        assert_eq!(triangulations(&[1, 2, 3]).len(), 1);
        assert_eq!(triangulations(&[1, 2, 3, 4]).len(), 2);
        assert_eq!(triangulations(&[1, 2, 3, 4, 5]).len(), 5);
        assert_eq!(retirement_block_size(3), Some(1));
        assert_eq!(retirement_block_size(4), Some(2));
        assert_eq!(retirement_block_size(5), Some(5));
        assert_eq!(retirement_block_size(6), Some(14));
        assert_eq!(retirement_block_size(7), Some(42));
    }

    #[test]
    fn immutable_custom_face_angles_are_pruned_before_elastic_search() {
        let source = MotherGrid::generate(6).unwrap();
        let valid = source.mesh.triangles()[source.mesh.active_triangle_slots().next().unwrap()];
        let invalid = [25, 26, 21];

        assert!(fixed_custom_face_angles_are_repairable(
            &source,
            &[valid],
            &valid.into_iter().collect()
        ));
        assert!(!fixed_custom_face_angles_are_repairable(
            &source,
            &[invalid],
            &invalid.into_iter().collect()
        ));
        assert!(fixed_custom_face_angles_are_repairable(
            &source,
            &[invalid],
            &[invalid[0], invalid[1]].into_iter().collect()
        ));
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

    /// The order rescanning every pending variable per pick gave -- the
    /// implementation `greedy_order` replaced, kept as its oracle.
    fn rescanned_order(
        pending: &[SearchVariable],
        parents: &[TriangleAddress],
        fixed_touched: &BTreeSet<usize>,
    ) -> Vec<usize> {
        let mut left_over = (0..pending.len()).collect::<Vec<_>>();
        let mut frontier = fixed_touched.clone();
        let mut order = Vec::new();
        while !left_over.is_empty() {
            let best = left_over
                .iter()
                .enumerate()
                .max_by(|(_, &left), (_, &right)| {
                    let (left, right) = (&pending[left], &pending[right]);
                    shared_count(&left.touched, &frontier)
                        .cmp(&shared_count(&right.touched, &frontier))
                        .then_with(|| left.touched.len().cmp(&right.touched.len()))
                        .then_with(|| {
                            parents[right.original_position].cmp(&parents[left.original_position])
                        })
                })
                .map(|(position, _)| position)
                .unwrap();
            let index = left_over.remove(best);
            frontier.extend(pending[index].touched.iter().copied());
            order.push(index);
        }
        order
    }

    /// The incremental order is the rescanning one, variable for variable,
    /// on random variables with many ties in their counts.
    #[test]
    fn variables_are_ordered_as_rescanning_ordered_them() {
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = move |bound: u64| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state % bound
        };
        for round in 0..200 {
            let count = next(60) as usize + usize::from(round % 7 == 0) * 300;
            let vertices = 4 + next(40);
            let mut positions = (0..count).collect::<Vec<_>>();
            for i in (1..positions.len()).rev() {
                positions.swap(i, next(i as u64 + 1) as usize);
            }
            let parents = positions
                .iter()
                .map(|&position| TriangleAddress {
                    base_face: (position % 20) as u8,
                    i: position / 20,
                    j: 0,
                    n: 1 << 10,
                    orientation: crate::mother_grid::TriangleOrientation::Up,
                })
                .collect::<Vec<_>>();
            let pending = (0..count)
                .map(|position| SearchVariable {
                    original_position: position,
                    variants: Vec::new(),
                    touched: (0..1 + next(6))
                        .map(|_| next(vertices) as usize)
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect(),
                })
                .collect::<Vec<_>>();
            let fixed = (0..next(5))
                .map(|_| next(vertices) as usize)
                .collect::<BTreeSet<_>>();
            assert_eq!(
                greedy_order(&pending, &parents, &fixed),
                rescanned_order(&pending, &parents, &fixed),
                "round {round}"
            );
        }
    }

    /// The suffix of every position merged into one list -- the
    /// implementation `SuffixDegreeMasks` replaced, kept as its oracle.
    fn merged_suffix_masks(variables: &[SearchVariable]) -> Vec<Vec<(usize, u128)>> {
        let mut suffix = vec![Vec::new(); variables.len() + 1];
        for position in (0..variables.len()).rev() {
            suffix[position] = combine_suffix_masks(
                &local_degree_masks(&variables[position]),
                &suffix[position + 1],
            );
        }
        suffix
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

    /// Per-vertex suffix masks are the merged ones, position for position
    /// and vertex for vertex, on random variables.
    #[test]
    fn suffix_masks_per_vertex_are_the_merged_ones() {
        let mut state = 0x2545_f491_4f6c_dd1du64;
        let mut next = move |bound: u64| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state % bound
        };
        for round in 0..100 {
            let vertices = 4 + next(30) as usize;
            let variables = (0..next(40) as usize)
                .map(|position| {
                    let variants = (0..1 + next(4))
                        .map(|variant_index| VariantChoice {
                            variant_index: variant_index as usize,
                            delta: (0..1 + next(5))
                                .map(|_| (next(vertices as u64) as usize, 1 + next(3) as isize))
                                .collect::<BTreeMap<_, _>>()
                                .into_iter()
                                .collect(),
                        })
                        .collect::<Vec<_>>();
                    let touched = variants
                        .iter()
                        .flat_map(|choice| choice.delta.iter().map(|&(vertex, _)| vertex))
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect();
                    SearchVariable {
                        original_position: position,
                        variants,
                        touched,
                    }
                })
                .collect::<Vec<_>>();
            let merged = merged_suffix_masks(&variables);
            let per_vertex = SuffixDegreeMasks::new(&variables);
            for (position, list) in merged.iter().enumerate() {
                for vertex in 0..vertices {
                    let expected = list
                        .binary_search_by_key(&vertex, |&(candidate, _)| candidate)
                        .map(|index| list[index].1)
                        .unwrap_or(1);
                    assert_eq!(
                        per_vertex.mask(vertex, position),
                        expected,
                        "round {round} position {position} vertex {vertex}"
                    );
                }
            }
        }
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
        let (variables, fixed) = search_variables(&variants, &parents, &mut chosen, None);
        let suffix = SuffixDegreeMasks::new(&variables);
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
        assert!(forecast.can_finish_all(&fixed, &suffix, 0));

        let repair = variables[0]
            .variants
            .iter()
            .find(|choice| choice.variant_index == 0)
            .unwrap();
        forecast.apply_delta(&repair.delta, 1);
        assert!(forecast.can_finish_all(&variables[0].touched, &suffix, variables.len()));
        forecast.apply_delta(&repair.delta, -1);

        let bad = variables[0]
            .variants
            .iter()
            .find(|choice| choice.variant_index == 1)
            .unwrap();
        forecast.apply_delta(&bad.delta, 1);
        assert!(!forecast.can_finish_all(&variables[0].touched, &suffix, variables.len()));
    }

    fn retirement_family_fixture() -> (
        MotherGrid,
        BTreeSet<TriangleAddress>,
        HierarchyLeafSet,
        SearchHit,
        TransitionBoundary,
    ) {
        let source = MotherGrid::generate(8).unwrap();
        let vertex = source
            .mesh
            .active_vertex_slots()
            .find(|&vertex| {
                let Some(seed) = source
                    .mesh
                    .active_triangle_slots()
                    .find(|&face| source.mesh.triangles()[face].contains(&vertex))
                else {
                    return false;
                };
                source
                    .mesh
                    .triangle_fan_from(vertex, seed)
                    .is_ok_and(|fan| fan.len() == 6)
            })
            .unwrap();
        let seed = source
            .mesh
            .active_triangle_slots()
            .find(|&face| source.mesh.triangles()[face].contains(&vertex))
            .unwrap();
        let transition = source
            .mesh
            .triangle_fan_from(vertex, seed)
            .unwrap()
            .into_iter()
            .map(|face| {
                source.triangle_addresses[face]
                    .and_then(TriangleAddress::parent_2_to_1)
                    .unwrap()
            })
            .collect::<BTreeSet<_>>();
        let source_vertex_slots = (0..source.mesh.vertices().len())
            .map(|slot| source.mesh.is_vertex_live(slot).then_some(slot))
            .collect();
        let hit = SearchHit {
            mesh: HierarchyLeafMesh {
                mesh: source.mesh.clone(),
                triangle_addresses: source.triangle_addresses.clone(),
                source_vertex_slots,
            },
            triangles_by_parent: BTreeMap::new(),
            triangles: Vec::new(),
            degree_forecast: BTreeMap::new(),
            topology_id: 0,
            retired: false,
        };
        let boundary = TransitionBoundary {
            halo_parents: transition.iter().copied().collect(),
            ..TransitionBoundary::default()
        };
        (
            source.clone(),
            transition,
            HierarchyLeafSet::from_mother_grid(&source).unwrap(),
            hit,
            boundary,
        )
    }

    #[test]
    fn retirement_family_builds_fresh_trial_and_cursor_advances() {
        let (source, transition, leaf_set, hit, boundary) = retirement_family_fixture();
        let core = BTreeSet::new();
        let base_states = 5;

        let Some(TransitionTopologyOutcome::Closed(trial)) = solve_retirement_family(
            &source,
            99,
            &core,
            &transition,
            &leaf_set,
            boundary.clone(),
            &hit,
            base_states,
            0,
            42,
            0,
        ) else {
            panic!("synthetic transition halo must enter retirement family");
        };

        let active = trial
            .candidate
            .source_active_vertices
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let retired = trial
            .candidate
            .source_degree_forecast
            .iter()
            .find_map(|(&source, &degree)| {
                (degree == 0 && !active.contains(&source)).then_some(source)
            })
            .unwrap();
        assert!(trial.candidate.topology_id >= base_states);
        assert!(!trial.candidate.source_active_vertices.contains(&retired));
        assert_eq!(trial.candidate.source_degree_forecast[&retired], 0);
        assert_eq!(
            trial
                .mesh
                .mesh
                .active_triangle_slots()
                .filter(|&face| trial.mesh.triangle_addresses[face].is_none())
                .count(),
            trial.candidate.source_triangles.len()
        );
        assert!(trial
            .candidate
            .custom_transition_triangles
            .keys()
            .all(|parent| transition.contains(parent)));
        ElasticPatch::from_transition(&trial).unwrap();
        hard_gate(&source, &trial.mesh).unwrap();
        assert_retirement_candidates_match_scan(&trial.mesh, &boundary, &transition);

        assert!(matches!(
            solve_retirement_family(
                &source,
                99,
                &core,
                &transition,
                &leaf_set,
                boundary.clone(),
                &hit,
                base_states,
                0,
                0,
                0,
            ),
            Some(TransitionTopologyOutcome::SearchBudgetExhausted { .. })
        ));

        if let Some(TransitionTopologyOutcome::Closed(next)) = solve_retirement_family(
            &source,
            99,
            &core,
            &transition,
            &leaf_set,
            boundary,
            &hit,
            base_states,
            trial.candidate.topology_id + 1 - base_states,
            42,
            0,
        ) {
            assert!(next.candidate.topology_id > trial.candidate.topology_id);
        }
    }

    #[test]
    fn exhausted_retirement_family_reports_retirement_states() {
        let (source, transition, _, hit, boundary) = retirement_family_fixture();
        let core = BTreeSet::new();
        let base_states = 7;
        let (_, degree) = retirement_candidates(&hit.mesh, &boundary, &transition)[0];
        let block = retirement_block_size(degree).unwrap();
        let bad_leaf_set = HierarchyLeafSet {
            leaves: BTreeSet::new(),
        };

        let outcome = solve_retirement_family(
            &source,
            99,
            &core,
            &transition,
            &bad_leaf_set,
            boundary,
            &hit,
            base_states,
            0,
            block + 1,
            0,
        );
        assert!(
            matches!(
                outcome,
                Some(TransitionTopologyOutcome::ProvenInfeasible {
                    states_examined,
                    ..
                }) if states_examined == base_states + block
            ),
            "unexpected outcome: {outcome:?}, block={block}"
        );
    }

    /// A failed candidate's promotion comes before the search goes on: the
    /// preferred parent's segment leaves the core, the new layout is
    /// enumerated from its first state, and the old layout's cursor counts as
    /// examined. A parent off the core boundary, or past the halo budget,
    /// leaves the old layout to go on from the cursor.
    #[test]
    fn a_failure_widens_the_transition_at_its_face_before_enumerating() {
        let fine = MotherGrid::generate(64).unwrap();
        let coarse = MotherGrid::generate(32).unwrap();
        let core = coarse
            .triangle_addresses
            .iter()
            .flatten()
            .copied()
            .filter(|parent| {
                parent.base_face == 0 && parent.i >= 8 && parent.j >= 8 && parent.i + parent.j < 24
            })
            .collect::<BTreeSet<_>>();
        let patches = Patches::new(&fine);
        let transition = core
            .iter()
            .flat_map(|&parent| patches.get(parent).unwrap().neighbours)
            .filter(|parent| !core.contains(parent))
            .collect::<BTreeSet<_>>();
        let component = HierarchyComponent {
            id: 12,
            parents: core.union(&transition).copied().collect(),
            boundary_edges: Vec::new(),
            core_parents: core.iter().copied().collect(),
            transition_parents: transition.iter().copied().collect(),
        };
        let limits = TransitionTopologyLimits {
            topology_states: 1_000,
            maximum_halo_expansions: 1,
        };
        let peel = core_boundary(&patches, &core);
        let preferred = *peel.first().unwrap();
        let segment = preferred_boundary_segment(&patches, &peel, &transition, preferred).unwrap();
        let interior = core
            .iter()
            .copied()
            .find(|parent| !peel.contains(parent))
            .unwrap();
        let cursor = 1;
        let search = |promotion| match solve_transition_topology_from_cursor_with_promotion(
            &fine,
            &component,
            limits,
            cursor,
            RetryRequest {
                promotion,
                ..RetryRequest::default()
            },
            true,
        ) {
            TransitionTopologyOutcome::Closed(trial) => trial,
            other => panic!("the fixture's topology must close: {other:?}"),
        };

        let widened = search(Some((preferred, 1)));
        assert_eq!(
            widened.candidate.core_parents,
            core.difference(&segment).copied().collect::<Vec<_>>()
        );
        assert_eq!(widened.report.halo_expansions, 1);
        assert_eq!(
            widened.report.topology_states,
            cursor + widened.report.layout_topology_states
        );
        assert_eq!(
            widened.candidate.topology_id + 1,
            widened.report.topology_states
        );

        let plain = search(None);
        assert_eq!(plain.candidate.core_parents, component.core_parents);
        for promotion in [(interior, 1), (preferred, 2)] {
            let kept = search(Some(promotion));
            assert_eq!(kept.candidate.core_parents, component.core_parents);
            assert_eq!(kept.candidate.topology_id, plain.candidate.topology_id);
            assert_eq!(kept.report.halo_expansions, 0);
        }
    }

    /// A near miss is retried at every place it missed (guide 11.137): the
    /// search promotes the other places' boundary segments with the worst
    /// one's, and leaves out a place whose segment would pinch the core.
    #[test]
    fn a_near_miss_promotes_its_other_places_where_they_pinch_nothing() {
        let (fine, component, pinching) = pinching_promotion();
        let patches = Patches::new(&fine);
        let core = component
            .core_parents
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let transition = component
            .transition_parents
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let peel = core_boundary(&patches, &core);
        let clean = |parent: TriangleAddress| {
            preferred_boundary_segment(&patches, &peel, &transition, parent).filter(|segment| {
                boundary(
                    &patches,
                    &core.difference(segment).copied().collect(),
                    &transition.union(segment).copied().collect(),
                )
                .is_ok()
            })
        };
        let far =
            |a: TriangleAddress, b: TriangleAddress| a.i.abs_diff(b.i) + a.j.abs_diff(b.j) >= 6;
        let worst = peel
            .iter()
            .copied()
            .find(|&parent| far(parent, pinching) && clean(parent).is_some())
            .unwrap();
        let other = peel
            .iter()
            .copied()
            .find(|&parent| far(parent, pinching) && far(parent, worst) && clean(parent).is_some())
            .unwrap();
        let limits = TransitionTopologyLimits {
            topology_states: 1_000,
            maximum_halo_expansions: 1,
        };
        let core_after = |also: &[(TriangleAddress, usize)]| {
            let TransitionTopologyOutcome::Closed(trial) =
                solve_transition_topology_from_cursor_with_promotion(
                    &fine,
                    &component,
                    limits,
                    1,
                    RetryRequest {
                        promotion: Some((worst, 0)),
                        also,
                        ..RetryRequest::default()
                    },
                    true,
                )
            else {
                panic!("the promoted layout must close");
            };
            trial
                .candidate
                .core_parents
                .into_iter()
                .collect::<BTreeSet<_>>()
        };
        let alone = core_after(&[]);
        assert!(!alone.contains(&worst) && alone.contains(&other));
        let both = core_after(&[(other, 0)]);
        assert!(!both.contains(&worst) && !both.contains(&other));
        assert!(both.is_subset(&alone));
        let pinched = core_after(&[(pinching, 0)]);
        assert!(!pinched.contains(&worst) && pinched.contains(&pinching));
        assert_eq!(pinched, alone);
    }

    /// A component's own layout pinched at a vertex is repaired past the
    /// halo budget when no parent at the pinch lies deeper than a promotion
    /// may reach (guide 11.138); with a reach that excludes them the
    /// boundary stays invalid, as before.
    #[test]
    fn a_layout_pinch_is_repaired_within_reach_past_the_halo_budget() {
        let (fine, component, preferred) = pinching_promotion();
        let patches = Patches::new(&fine);
        let core = component
            .core_parents
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let transition = component
            .transition_parents
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let peel = core_boundary(&patches, &core);
        let segment = preferred_boundary_segment(&patches, &peel, &transition, preferred).unwrap();
        let pinched_core = core.difference(&segment).copied().collect::<BTreeSet<_>>();
        let pinched_transition = transition.union(&segment).copied().collect::<BTreeSet<_>>();
        let reason = boundary(&patches, &pinched_core, &pinched_transition).unwrap_err();
        assert!(reason.starts_with("coarse inner boundary:"), "{reason}");
        let pinched = HierarchyComponent {
            core_parents: pinched_core.iter().copied().collect(),
            transition_parents: pinched_transition.iter().copied().collect(),
            ..component.clone()
        };
        let depths = pinched
            .parents
            .iter()
            .map(|&parent| (parent, 1))
            .collect::<BTreeMap<_, _>>();
        let search = |deepest| {
            solve_transition_topology_from_cursor_with_promotion(
                &fine,
                &pinched,
                TransitionTopologyLimits {
                    topology_states: 1_000,
                    maximum_halo_expansions: 0,
                },
                0,
                RetryRequest {
                    reach: Some((&depths, deepest)),
                    ..RetryRequest::default()
                },
                true,
            )
        };
        assert!(matches!(
            search(0),
            TransitionTopologyOutcome::InvalidBoundary { .. }
        ));
        let TransitionTopologyOutcome::Closed(repaired) = search(1) else {
            panic!("the pinch repaired within reach must close");
        };
        assert!(repaired.candidate.core_parents.len() < pinched_core.len());
    }

    /// A core of two parents that touch at one corner has nothing left to
    /// coarsen once the pinch is repaired: the search reports no topology,
    /// as when a halo expansion leaves no core, not an invalid boundary
    /// (the 100 km 30 m trial stopped at level 1 -> 0 on one).
    #[test]
    fn a_core_that_is_all_pinch_has_no_topology() {
        let fine = MotherGrid::generate(64).unwrap();
        let coarse = MotherGrid::generate(32).unwrap();
        let patches = Patches::new(&fine);
        let parents = coarse
            .triangle_addresses
            .iter()
            .flatten()
            .copied()
            .filter(|parent| parent.base_face == 0 && parent.i >= 8 && parent.j >= 8)
            .collect::<Vec<_>>();
        let (first, second) = parents
            .iter()
            .find_map(|&first| {
                let patch = patches.get(first).unwrap();
                parents.iter().copied().find_map(|second| {
                    let other = patches.get(second).unwrap();
                    let shared = patch
                        .corners
                        .iter()
                        .filter(|corner| other.corners.contains(corner))
                        .count();
                    (second != first && shared == 1 && !patch.neighbours.contains(&second))
                        .then_some((first, second))
                })
            })
            .unwrap();
        let core = BTreeSet::from([first, second]);
        let transition = core
            .iter()
            .flat_map(|&parent| patches.get(parent).unwrap().neighbours)
            .filter(|parent| !core.contains(parent))
            .collect::<BTreeSet<_>>();
        let reason = boundary(&patches, &core, &transition).unwrap_err();
        assert!(reason.starts_with("coarse inner boundary:"), "{reason}");
        let component = HierarchyComponent {
            id: 20,
            parents: core.union(&transition).copied().collect(),
            boundary_edges: Vec::new(),
            core_parents: core.iter().copied().collect(),
            transition_parents: transition.iter().copied().collect(),
        };
        let outcome = solve_transition_topology(
            &fine,
            &component,
            TransitionTopologyLimits {
                topology_states: 1_000,
                maximum_halo_expansions: 3,
            },
        );
        assert!(
            matches!(
                &outcome,
                TransitionTopologyOutcome::ProvenInfeasible { reason, .. }
                    if reason.starts_with("every core parent is at a pinch")
            ),
            "{outcome:?}"
        );
    }

    /// A core with an interior parent left out, and a boundary parent whose
    /// promotion makes the core touch itself at a vertex.
    fn pinching_promotion() -> (MotherGrid, HierarchyComponent, TriangleAddress) {
        let fine = MotherGrid::generate(64).unwrap();
        let coarse = MotherGrid::generate(32).unwrap();
        let patches = Patches::new(&fine);
        let whole = coarse
            .triangle_addresses
            .iter()
            .flatten()
            .copied()
            .filter(|parent| {
                parent.base_face == 0 && parent.i >= 8 && parent.j >= 8 && parent.i + parent.j < 24
            })
            .collect::<BTreeSet<_>>();
        let around = |core: &BTreeSet<TriangleAddress>| {
            core.iter()
                .flat_map(|&parent| patches.get(parent).unwrap().neighbours)
                .filter(|parent| !core.contains(parent))
                .collect::<BTreeSet<_>>()
        };
        let whole_peel = core_boundary(&patches, &whole);
        let (core, transition, preferred) = whole
            .iter()
            .filter(|parent| !whole_peel.contains(parent))
            .find_map(|&hole| {
                let mut core = whole.clone();
                core.remove(&hole);
                let transition = around(&core);
                boundary(&patches, &core, &transition).ok()?;
                let peel = core_boundary(&patches, &core);
                let preferred = peel.iter().copied().find(|&preferred| {
                    preferred_boundary_segment(&patches, &peel, &transition, preferred).is_some_and(
                        |segment| {
                            boundary(
                                &patches,
                                &core.difference(&segment).copied().collect(),
                                &transition.union(&segment).copied().collect(),
                            )
                            .is_err_and(|reason| reason.starts_with("coarse inner boundary:"))
                        },
                    )
                })?;
                Some((core, transition, preferred))
            })
            .unwrap();
        let component = HierarchyComponent {
            id: 12,
            parents: core.union(&transition).copied().collect(),
            boundary_edges: Vec::new(),
            core_parents: core.iter().copied().collect(),
            transition_parents: transition.iter().copied().collect(),
        };
        (fine, component, preferred)
    }

    /// A failure's promotion whose layout has an invalid boundary the
    /// search cannot repair -- here a core pinched at a vertex, the halo
    /// budget spent on the promotion -- falls back to the old layout: the
    /// search goes on as if no promotion had been asked for.
    #[test]
    fn a_promotion_that_pinches_the_core_falls_back_to_the_old_layout() {
        let (fine, component, preferred) = pinching_promotion();
        let limits = TransitionTopologyLimits {
            topology_states: 1_000,
            maximum_halo_expansions: 1,
        };
        let search = |promotion| {
            format!(
                "{:?}",
                solve_transition_topology_from_cursor_with_promotion(
                    &fine,
                    &component,
                    limits,
                    1,
                    RetryRequest {
                        promotion,
                        ..RetryRequest::default()
                    },
                    true,
                )
            )
        };
        let plain = search(None);
        assert!(!plain.starts_with("InvalidBoundary"), "{plain}");
        assert_eq!(search(Some((preferred, 1))), plain);
    }

    /// The same pinch, repaired within the promotion's reach (guide 11.130):
    /// the parents at the pinch leave the core too when none lies deeper
    /// than a promotion may reach, and the search keeps the promotion. With
    /// a reach that excludes them it falls back as before.
    #[test]
    fn a_promotion_repairs_its_pinch_within_its_reach() {
        let (fine, component, preferred) = pinching_promotion();
        let limits = TransitionTopologyLimits {
            topology_states: 1_000,
            maximum_halo_expansions: 1,
        };
        let depths = component
            .parents
            .iter()
            .map(|&parent| (parent, 1))
            .collect::<BTreeMap<_, _>>();
        let search = |reach| {
            solve_transition_topology_from_cursor_with_promotion(
                &fine,
                &component,
                limits,
                1,
                RetryRequest {
                    also: &[],
                    promotion: Some((preferred, 1)),
                    reach,
                    focus: None,
                },
                true,
            )
        };
        let plain = format!(
            "{:?}",
            solve_transition_topology_from_cursor_with_promotion(
                &fine,
                &component,
                limits,
                1,
                RetryRequest::default(),
                true,
            )
        );
        assert_eq!(format!("{:?}", search(Some((&depths, 0)))), plain);
        let TransitionTopologyOutcome::Closed(repaired) = search(Some((&depths, 1))) else {
            panic!("the repaired promotion must close");
        };
        let patches = Patches::new(&fine);
        let core = component
            .core_parents
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let transition = component
            .transition_parents
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let segment = preferred_boundary_segment(
            &patches,
            &core_boundary(&patches, &core),
            &transition,
            preferred,
        )
        .unwrap();
        assert!(repaired.candidate.core_parents.len() < core.len() - segment.len());
        assert!(repaired
            .candidate
            .core_parents
            .iter()
            .all(|parent| core.contains(parent) && !segment.contains(parent)));
        assert_eq!(repaired.report.halo_expansions, 1);
    }

    /// A focus (guide 11.130) offers configurations that change a custom
    /// parent near its point, keep every parent beyond its radius as the
    /// failed candidate had it, are never the failed one or one offered
    /// before, count their own states and leave the layout's cursor alone;
    /// and it runs out. Widened, it offers only configurations that also
    /// change a parent beyond the previous radius.
    #[test]
    fn a_focus_varies_the_transition_at_the_failure_only() {
        let fine = MotherGrid::generate(64).unwrap();
        let coarse = MotherGrid::generate(32).unwrap();
        let patches = Patches::new(&fine);
        let core = coarse
            .triangle_addresses
            .iter()
            .flatten()
            .copied()
            .filter(|parent| {
                parent.base_face == 0 && parent.i >= 8 && parent.j >= 8 && parent.i + parent.j < 24
            })
            .collect::<BTreeSet<_>>();
        let transition = core
            .iter()
            .flat_map(|&parent| patches.get(parent).unwrap().neighbours)
            .filter(|parent| !core.contains(parent))
            .collect::<BTreeSet<_>>();
        let component = HierarchyComponent {
            id: 12,
            parents: core.union(&transition).copied().collect(),
            boundary_edges: Vec::new(),
            core_parents: core.iter().copied().collect(),
            transition_parents: transition.iter().copied().collect(),
        };
        let limits = TransitionTopologyLimits {
            topology_states: 1_000,
            maximum_halo_expansions: 1,
        };
        let cursor = 2;
        let TransitionTopologyOutcome::Closed(failed) =
            solve_transition_topology_from_cursor(&fine, &component, limits, cursor)
        else {
            panic!("the fixture's topology must close");
        };
        let base = failed.candidate.custom_transition_triangles.clone();
        let vertices = fine.mesh.vertices();
        let corners =
            |parent: TriangleAddress| patches.get(parent).unwrap().corners.map(|c| vertices[c]);
        let centre = |parent: TriangleAddress| {
            let [a, b, c] = corners(parent);
            CartesianPoint::new(a.x + b.x + c.x, a.y + b.y + c.y, a.z + b.z + c.z)
        };
        let point = centre(*base.keys().next().unwrap());
        let distance = |parent: TriangleAddress| {
            let [a, b, _] = corners(parent);
            angle_between(centre(parent), point) / angle_between(a, b)
        };
        let search = |focus: &RetryFocus| {
            solve_transition_topology_from_cursor_with_promotion(
                &fine,
                &component,
                limits,
                cursor,
                RetryRequest {
                    focus: Some(focus),
                    ..RetryRequest::default()
                },
                true,
            )
        };
        let changed = |triangles: &BTreeMap<TriangleAddress, Vec<[usize; 3]>>,
                       within: &dyn Fn(f64) -> bool| {
            triangles
                .iter()
                .any(|(parent, chosen)| within(distance(*parent)) && chosen != &base[parent])
        };
        let mut focus = RetryFocus {
            point,
            change_radius_edges: 2.0,
            radius_edges: 8.0,
            previous_radius_edges: 0.0,
            chosen: base.clone(),
            cursor: 0,
        };
        let mut offered = vec![base.clone()];
        let enumerate = |focus: &mut RetryFocus, offered: &mut Vec<_>| loop {
            match search(focus) {
                TransitionTopologyOutcome::Closed(trial) => {
                    let states = trial.report.focus_topology_states.unwrap();
                    assert!(states > focus.cursor);
                    assert_eq!(trial.report.layout_topology_states, cursor);
                    assert_eq!(trial.candidate.core_parents, failed.candidate.core_parents);
                    let triangles = trial.candidate.custom_transition_triangles;
                    for (parent, chosen) in &triangles {
                        if distance(*parent) > focus.radius_edges {
                            assert_eq!(chosen, &base[parent], "{parent:?} is off the focus");
                        }
                    }
                    let near = focus.change_radius_edges;
                    assert!(changed(&triangles, &|distance| distance <= near));
                    if focus.previous_radius_edges > 0.0 {
                        let previous = focus.previous_radius_edges;
                        assert!(changed(&triangles, &|distance| distance > previous));
                    }
                    assert!(!offered.contains(&triangles));
                    offered.push(triangles);
                    focus.cursor = states;
                }
                TransitionTopologyOutcome::FocusExhausted { .. } => break,
                other => panic!("unexpected focused outcome: {other:?}"),
            }
        };
        enumerate(&mut focus, &mut offered);
        let first = offered.len();
        assert!(first > 1, "the focus must offer a configuration");

        focus = RetryFocus {
            change_radius_edges: 4.0,
            radius_edges: 16.0,
            previous_radius_edges: 8.0,
            cursor: 0,
            ..focus
        };
        enumerate(&mut focus, &mut offered);
        assert!(offered.len() >= first);
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
        let peel = core_boundary(&Patches::new(&source), &core);
        assert!(peel.len() > 1);
        let preferred = *peel.first().unwrap();
        let expected =
            preferred_boundary_segment(&Patches::new(&source), &peel, &transition, preferred)
                .unwrap();
        assert!(expected.len() > 1);
        let untouched = core
            .iter()
            .copied()
            .find(|parent| !expected.contains(parent))
            .unwrap();
        let initial_core_len = core.len();

        assert_eq!(
            promote_core_boundary(
                &Patches::new(&source),
                &mut core,
                &mut transition,
                Some((preferred, 1)),
                1,
            ),
            Some(1)
        );
        assert_eq!(core.len(), initial_core_len - expected.len());
        assert!(expected.iter().all(|parent| transition.contains(parent)));
        assert!(core.contains(&untouched));
    }
}
