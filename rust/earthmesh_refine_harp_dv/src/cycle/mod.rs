//! The loop: read every cell, serve what it asks, stop for a stated reason.
//!
//! Specification section 8. One cycle evaluates the active cells, builds
//! demands, drops what is satisfied, orders what is left, and runs a
//! transaction per demand. The next cycle re-reads everything, because the mesh
//! it is reading is not the mesh the last cycle read.
//!
//! # Re-reading is the point
//!
//! A backend that turned demand into geometry once would order the cells by a
//! violation measured before any of them changed. Here a cell refined in cycle
//! k is measured again in cycle k+1 and usually stops asking; its neighbours,
//! whose cells moved when it split, are measured again too.
//!
//! # Every exit names itself
//!
//! Four ways out, and the report carries which: everything satisfied, the cycle
//! limit, the site budget, or a cycle in which no transaction was accepted. The
//! last is the one worth keeping separate -- a run that stops because nothing
//! it proposed was legal has not finished, and a report calling that success
//! would be the silent failure this backend exists to avoid.

use std::collections::{BTreeMap, BTreeSet};

use earthmesh_mesh::{lonlat_degrees_to_unit_xyz, magnitude, CartesianPoint, MESH_STATE_FIRST_ID};
use earthmesh_refine::{
    order_demands, CriterionSemantics, DemandEvidence, RefinementCause, RefinementDemand,
};

use crate::candidate::CandidatePolicy;
use crate::criteria::{CellCriterion, CellView};
use crate::error::Result;
use crate::report::{HarpDvRunReport, RejectionTally, StopReason};
use crate::state::AdaptiveMesh;
use crate::transaction::{Acceptance, DemandOutcome, HardGates, Rejection};
use earthmesh_mesh::MeshState;

/// What bounds a run, over and above the per-transaction gates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CycleLimits {
    pub max_cycles: u32,
    /// The most sites the mesh may end with.
    pub max_sites: usize,
    /// Cells at or below this stop asking, whatever a criterion measures.
    ///
    /// Checked in the driver rather than in each criterion, so one criterion
    /// cannot drive the mesh past a floor another respects.
    pub minimum_cell_width_m: f64,
    /// The most two adjacent cells' effective scales may differ.
    ///
    /// Specification section 14. Method-C bounds this with fixed transition
    /// rows and red-green with its judge chain; here it is a demand like any
    /// other, raised against the coarser of the two and served by the same
    /// ladder. Measured on an unbalanced run: a target that halves the scale
    /// inside a circle leaves 58 adjacent pairs past 1.75 and a worst ratio of
    /// 2.46.
    pub max_neighbour_scale_ratio: f64,
}

/// One run's account, and what it could not serve.
#[derive(Debug)]
pub struct CycleOutcome {
    pub report: HarpDvRunReport,
    /// Cells whose demand no ladder could satisfy, from the last cycle that
    /// tried them.
    pub unresolved_cells: Vec<usize>,
}

/// Every cell's effective scale, indexed by site. `None` where unreadable.
fn scales(mesh: &AdaptiveMesh) -> Vec<Option<f64>> {
    let state = mesh.state();
    let radius_m = state.sphere_radius();
    (0..state.vertices().len())
        .map(|site| {
            if site < MESH_STATE_FIRST_ID {
                return None;
            }
            let cell = state.voronoi_cell(site).ok()?;
            CellView {
                site,
                cell: &cell,
                state,
                radius_m,
            }
            .effective_scale_m()
        })
        .collect()
}

/// Demands raised by the mesh against itself, where two neighbours differ in
/// scale by more than the run allows.
///
/// Raised against the *coarser* of the pair: refining the finer one would
/// widen the ratio it was called to close.
fn balance_demands(
    mesh: &AdaptiveMesh,
    scales: &[Option<f64>],
    limits: CycleLimits,
) -> Vec<RefinementDemand> {
    let state = mesh.state();
    // Worst offender per cell, so one cell surrounded by fine neighbours
    // produces one demand rather than six.
    let mut worst: BTreeMap<usize, f64> = BTreeMap::new();
    for site in MESH_STATE_FIRST_ID..state.vertices().len() {
        let Some(here) = scales[site] else { continue };
        if here <= limits.minimum_cell_width_m {
            continue;
        }
        let Ok(fan) = state.triangle_fan(site) else {
            continue;
        };
        for triangle in fan {
            for corner in state.triangles()[triangle] {
                if corner == site {
                    continue;
                }
                let Some(there) = scales[corner] else {
                    continue;
                };
                // Only the coarser side is asked to refine.
                if here <= there {
                    continue;
                }
                let ratio = here / there;
                if ratio > limits.max_neighbour_scale_ratio {
                    let entry = worst.entry(site).or_insert(ratio);
                    *entry = entry.max(ratio);
                }
            }
        }
    }
    worst
        .into_iter()
        .map(|(site, ratio_before)| {
            let evidence = DemandEvidence {
                criterion_id: "scale-balance".to_string(),
                semantics: CriterionSemantics::MeshQuality,
                measured_value: ratio_before,
                threshold: limits.max_neighbour_scale_ratio,
                normalized_violation: (ratio_before - limits.max_neighbour_scale_ratio)
                    / limits.max_neighbour_scale_ratio,
                requested_scale_m: None,
                witness: None,
                confidence: 1.0,
                source_resolution_m: None,
                // A hard requirement: an unbalanced mesh is not a coarser
                // answer to the same question, it is one the solver cannot use.
                hard_requirement: true,
                satisfiable: true,
                stop_reason: None,
            };
            RefinementDemand::from_evidence(
                site as u64,
                vec![evidence],
                RefinementCause::ScaleBalance { ratio_before },
            )
        })
        .collect()
}

/// Adjacent pairs still past the bound.
///
/// Reported even though unconstrained r-adaptation normally closes them: a
/// protected-segment run does not move sites, and a hard gate may still leave
/// a residue that the caller must decide on.
fn unbalanced_pairs(mesh: &AdaptiveMesh, limits: CycleLimits) -> usize {
    let state = mesh.state();
    let scales = scales(mesh);
    let mut over = 0;
    for site in MESH_STATE_FIRST_ID..state.vertices().len() {
        let Some(here) = scales[site] else { continue };
        let Ok(fan) = state.triangle_fan(site) else {
            continue;
        };
        for triangle in fan {
            for corner in state.triangles()[triangle] {
                if corner == site {
                    continue;
                }
                let Some(there) = scales[corner] else {
                    continue;
                };
                if here.max(there) / here.min(there) > limits.max_neighbour_scale_ratio {
                    over += 1;
                }
            }
        }
    }
    over
}

/// The part of the global scale objective that a local move can change.
///
/// Every edge not incident to `sites` contributes the same value before and
/// after the move, so comparing this tuple is exactly the global objective
/// delta without rescanning the whole mesh for every candidate.
fn balance_objective(state: &MeshState, sites: &BTreeSet<usize>, bound: f64) -> Option<[f64; 3]> {
    let mut edges = BTreeSet::new();
    for &site in sites {
        for triangle in state.triangle_fan(site).ok()? {
            for corner in state.triangles()[triangle] {
                if corner != site {
                    edges.insert((site.min(corner), site.max(corner)));
                }
            }
        }
    }
    let mut cached = BTreeMap::new();
    let mut violations = 0usize;
    let mut worst = 1.0_f64;
    let mut energy = 0.0;
    for (left, right) in edges {
        let mut scale = |site| {
            if let Some(value) = cached.get(&site) {
                return Some(*value);
            }
            let value = site_scale(state, site)?;
            cached.insert(site, value);
            Some(value)
        };
        let here = scale(left)?;
        let there = scale(right)?;
        let ratio = here.max(there) / here.min(there);
        if !ratio.is_finite() {
            return None;
        }
        worst = worst.max(ratio);
        if ratio > bound {
            violations += 1;
            let excess = ratio / bound - 1.0;
            energy += excess * excess;
        }
    }
    Some([violations as f64, worst, energy])
}

fn site_scale(state: &MeshState, site: usize) -> Option<f64> {
    let cell = state.voronoi_cell(site).ok()?;
    CellView {
        site,
        cell: &cell,
        state,
        radius_m: state.sphere_radius(),
    }
    .effective_scale_m()
}

fn projected_step(
    here: CartesianPoint,
    target: CartesianPoint,
    step: f64,
) -> Option<CartesianPoint> {
    let radius = magnitude(here);
    let stepped = CartesianPoint::new(
        here.x + (target.x - here.x) * step,
        here.y + (target.y - here.y) * step,
        here.z + (target.z - here.z) * step,
    );
    let length = magnitude(stepped);
    if !length.is_finite() || length <= 0.0 {
        return None;
    }
    Some(CartesianPoint::new(
        stepped.x / length * radius,
        stepped.y / length * radius,
        stepped.z / length * radius,
    ))
}

fn neighbour_sites(state: &MeshState, site: usize) -> BTreeSet<usize> {
    let Ok(fan) = state.triangle_fan(site) else {
        return BTreeSet::new();
    };
    fan.into_iter()
        .flat_map(|triangle| state.triangles()[triangle])
        .filter(|&corner| corner != site)
        .collect()
}

/// Deterministic positions that can change the Delaunay degree of `site`.
///
/// A centroid-only move was too symmetric to cross an edge-flip boundary and
/// fired zero times. Moving away from each current neighbour directly targets
/// the incident edge that must disappear for a degree-seven site to make room.
fn degree_relief_destinations(state: &MeshState, site: usize) -> Vec<CartesianPoint> {
    let here = state.vertices()[site];
    let neighbours = neighbour_sites(state, site);
    if neighbours.is_empty() {
        return Vec::new();
    }
    let mut candidates = Vec::with_capacity(neighbours.len() * 2);
    for neighbour in neighbours {
        let point = state.vertices()[neighbour];
        let away = CartesianPoint::new(
            here.x + (here.x - point.x),
            here.y + (here.y - point.y),
            here.z + (here.z - point.z),
        );
        for step in [0.5, 0.25] {
            if let Some(point) = projected_step(here, away, step) {
                candidates.push(point);
            }
        }
    }
    candidates
}

fn sites_are_adjacent(state: &MeshState, left: usize, right: usize) -> Option<bool> {
    Some(state.triangle_fan(left).ok()?.into_iter().any(|triangle| {
        state.triangles()[triangle]
            .into_iter()
            .any(|corner| corner == right)
    }))
}

fn tally_refusals(
    reasons: &[(crate::candidate::CandidateSource, Rejection)],
    tally: &mut RejectionTally,
) {
    for (_, reason) in reasons {
        match reason {
            Rejection::DegreeOverBudget { .. } => tally.degree += 1,
            Rejection::ProtectedPentagonDisturbed { .. } => tally.pentagon += 1,
            Rejection::NotInsertable(_) => tally.not_insertable += 1,
            Rejection::PatchTooLarge { .. }
            | Rejection::SurfaceOpened { .. }
            | Rejection::TopologyInvalid { .. }
            | Rejection::CouldNotLegalize(_) => tally.topology += 1,
            Rejection::SliverTriangle { .. } => tally.sliver += 1,
            Rejection::NoImprovement { .. } => tally.no_improvement += 1,
            Rejection::Unmeasurable(_) => tally.unmeasurable += 1,
        }
    }
}

/// Where to move a site to even out the scales around it.
///
/// Toward the centroid of its neighbours, weighted so the coarse side pulls
/// harder: a site sitting between a fine neighbourhood and a coarse one is
/// what makes the ratio, and moving it into the coarse side shrinks the coarse
/// cell and grows the fine one at once.
///
/// A fraction of the way, not all of it. A site landing on the centroid would
/// overshoot past the balance it was moving toward, and the next cycle would
/// move it back.
fn balance_destinations(mesh: &AdaptiveMesh, site: usize) -> Vec<CartesianPoint> {
    let state = mesh.state();
    let here = state.vertices()[site];
    let Ok(fan) = state.triangle_fan(site) else {
        return Vec::new();
    };
    let mut neighbours = BTreeMap::new();
    for triangle in &fan {
        for corner in state.triangles()[*triangle] {
            if corner != site {
                neighbours.insert(corner, ());
            }
        }
    }
    if neighbours.is_empty() {
        return Vec::new();
    }
    let radius_m = state.sphere_radius();
    let mut total = 0.0;
    let mut target = CartesianPoint::new(0.0, 0.0, 0.0);
    for &corner in neighbours.keys() {
        // A *fine* neighbour pulls hardest. The demand was raised on the
        // coarser cell, so the ratio falls when this site moves toward the
        // fine side: the coarse cell loses area there and the fine one gains
        // it. Weighting the other way -- toward the coarse side -- was tried
        // first and measured worse than doing nothing.
        let scale = state
            .voronoi_cell(corner)
            .ok()
            .and_then(|cell| {
                CellView {
                    site: corner,
                    cell: &cell,
                    state,
                    radius_m,
                }
                .effective_scale_m()
            })
            .unwrap_or(0.0);
        if scale <= 0.0 {
            continue;
        }
        let weight = 1.0 / scale;
        let point = state.vertices()[corner];
        target = CartesianPoint::new(
            target.x + point.x * weight,
            target.y + point.y * weight,
            target.z + point.z * weight,
        );
        total += weight;
    }
    if total <= 0.0 {
        return Vec::new();
    }
    let centroid = CartesianPoint::new(target.x / total, target.y / total, target.z / total);
    [0.5, 0.25, 0.125, 0.0625]
        .into_iter()
        .filter_map(|step| projected_step(here, centroid, step))
        .collect()
}

/// What a pass over the cells found, beyond the demands themselves.
///
/// `AllSatisfied` used to be reported whenever the demand list came back
/// empty, which folded three different endings into one: every cell is small
/// enough, every cell that still wants something has hit the minimum width, and
/// every remaining demand is one the data cannot support. Only the first is
/// "satisfied"; the other two are the run stopping short, and a caller that
/// cannot tell them apart cannot know whether it got what it asked for.
#[derive(Clone, Copy, Debug, Default)]
struct EvaluationTally {
    /// Cells skipped because they already reached `minimum_cell_width_m`.
    ///
    /// Skipped whole, criteria and all -- so a cell at the floor stops being
    /// asked about its *angles* too, not only its size.
    at_minimum_scale: usize,
    /// Cells whose only remaining evidence says the data cannot support it.
    unsatisfiable: usize,
}

/// Read every active cell once.
fn evaluate(
    mesh: &AdaptiveMesh,
    criteria: &[Box<dyn CellCriterion>],
    limits: CycleLimits,
) -> Result<(Vec<RefinementDemand>, EvaluationTally, Vec<Option<f64>>)> {
    let state = mesh.state();
    let radius_m = state.sphere_radius();
    let mut demands = Vec::new();
    let mut tally = EvaluationTally::default();
    let mut scales = vec![None; state.vertices().len()];
    for (site, scale_slot) in scales.iter_mut().enumerate().skip(MESH_STATE_FIRST_ID) {
        // A cell that cannot be read is not a demand. Skipping it here keeps
        // evaluation total; the transaction layer reports the same cell as
        // `NotAttempted` if anything later asks for it.
        let Ok(cell) = state.voronoi_cell(site) else {
            continue;
        };
        let view = CellView {
            site,
            cell: &cell,
            state,
            radius_m,
        };
        let scale = view.effective_scale_m();
        *scale_slot = scale;
        // Read the criteria first, even for a cell already at the floor.
        //
        // The floor used to short-circuit the whole cell, and the tally with
        // it, so any small cell counted as "stopped short" -- including cells
        // no criterion covers, which are simply outside the region and want
        // nothing. That reported `MinimumScaleReached` for a run where nothing
        // was ever asked, which is the same class of wrong answer the tally was
        // added to stop.
        //
        // Evaluating and then declining to act costs one pass over criteria for
        // the cells at the floor, and buys the distinction between "this cell
        // wanted more and could not have it" and "this cell wanted nothing".
        let at_floor = scale.is_some_and(|scale| scale <= limits.minimum_cell_width_m);
        let mut evidences = Vec::new();
        let mut unsatisfiable = false;
        for criterion in criteria {
            let evidence = criterion.evaluate(&view)?;
            unsatisfiable |= !evidence.satisfiable;
            if evidence.demands_work() || !evidence.satisfiable {
                evidences.push(evidence);
            }
        }
        // The cause names the criterion that asked hardest, so a report can
        // say which one drove a cell rather than only that something did.
        let cause = evidences
            .iter()
            .filter(|evidence| evidence.demands_work())
            .max_by(|left, right| {
                left.normalized_violation
                    .partial_cmp(&right.normalized_violation)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map_or(RefinementCause::UserSpecified, |evidence| {
                RefinementCause::PhysicalCriterion {
                    criterion_id: evidence.criterion_id.clone(),
                }
            });
        // Counted before the filter, because `demands_work()` is false for an
        // unsatisfiable demand and true for a satisfied one -- and the two mean
        // opposite things to a caller.
        let demand = RefinementDemand::from_evidence(site as u64, evidences, cause);
        if !demand.demands_work() {
            if unsatisfiable {
                tally.unsatisfiable += 1;
            }
        } else if at_floor {
            // It wanted work and cannot have it: the floor is what stopped it,
            // and that is what the run should say when nothing else is left.
            tally.at_minimum_scale += 1;
        } else {
            demands.push(demand);
        }
    }
    Ok((demands, tally, scales))
}

/// Run cycles until something says to stop, and say which.
pub fn run_cycles(
    mesh: &mut AdaptiveMesh,
    criteria: &[Box<dyn CellCriterion>],
    policy: CandidatePolicy,
    gates: HardGates,
    limits: CycleLimits,
) -> Result<CycleOutcome> {
    let initial_sites = mesh.active_site_count();
    let mut attempted = 0usize;
    let mut committed = 0usize;
    let mut rolled_back = 0usize;
    let mut balanced = 0usize;
    let mut refusals = RejectionTally::default();
    let mut relieved = 0usize;
    let mut r_adapted = 0usize;
    let mut unresolved_cells = Vec::new();
    let mut quality_constrained = 0usize;
    let mut cycles = 0u32;
    let mut stop_reason = StopReason::MaximumCyclesReached;
    let mut adaptation_probe = None;

    while cycles < limits.max_cycles {
        unresolved_cells.clear();
        quality_constrained = 0;
        let (mut demands, tally, scales) = evaluate(mesh, criteria, limits)?;
        let physical_demand_count = demands.len();
        // Section 14, folded into the same list rather than run as a pass of
        // its own: one loop serves both, and `RefinementCause` is what keeps
        // physical refinement and transition balance apart in the report --
        // which the spec says is the reason the distinction exists.
        let balance = balance_demands(mesh, &scales, limits);
        let balance_demand_count = balance.len();
        demands.extend(balance);
        if demands.is_empty() {
            // Three endings, told apart. A cell parked at the floor or asking
            // for something the data cannot give is not a satisfied cell, and
            // saying `AllSatisfied` for either is the report claiming the run
            // delivered what was asked.
            stop_reason = if tally.at_minimum_scale > 0 {
                StopReason::MinimumScaleReached
            } else if tally.unsatisfiable > 0 {
                StopReason::SourceResolutionReached
            } else {
                StopReason::AllSatisfied
            };
            break;
        }
        order_demands(&mut demands);

        let mut accepted_this_cycle = 0usize;
        let mut adapted_this_cycle = 0usize;
        let mut out_of_budget = false;
        let mut degree_blocked_sites = BTreeSet::new();
        let mut pentagon_blocked_pairs = BTreeSet::new();
        let mut balance_blocked_sites = BTreeSet::new();
        for demand in &demands {
            if mesh.active_site_count() >= limits.max_sites {
                out_of_budget = true;
                break;
            }
            let site = demand.cell as usize;
            let witness = demand.preferred_witness.map(|witness| {
                let unit = lonlat_degrees_to_unit_xyz(witness);
                let radius = magnitude(mesh.state().vertices()[site]);
                CartesianPoint::new(unit.x * radius, unit.y * radius, unit.z * radius)
            });
            let for_balance = matches!(demand.cause, RefinementCause::ScaleBalance { .. });
            attempted += 1;
            match mesh.refine_cell(site, witness, policy, gates)? {
                DemandOutcome::Resolved { .. } => {
                    committed += 1;
                    accepted_this_cycle += 1;
                    if for_balance {
                        balanced += 1;
                    }
                }
                DemandOutcome::Unresolved {
                    refusals: reasons, ..
                } => {
                    rolled_back += reasons.len();
                    // Record the hard vertices and balance cells a local move
                    // can change; the phase below deduplicates them before it
                    // pays for legalization.
                    degree_blocked_sites.extend(reasons.iter().filter_map(
                        |(_, reason)| match reason {
                            Rejection::DegreeOverBudget { site, .. } => Some(*site),
                            _ => None,
                        },
                    ));
                    pentagon_blocked_pairs.extend(reasons.iter().filter_map(|(_, reason)| {
                        match reason {
                            Rejection::ProtectedPentagonDisturbed { site: pentagon, .. } => {
                                Some((*pentagon, site))
                            }
                            _ => None,
                        }
                    }));
                    if for_balance {
                        balance_blocked_sites.insert(site);
                    }
                    if !reasons.is_empty()
                        && reasons
                            .iter()
                            .all(|(_, reason)| matches!(reason, Rejection::SliverTriangle { .. }))
                    {
                        quality_constrained += 1;
                    }
                    tally_refusals(&reasons, &mut refusals);
                    unresolved_cells.push(site);
                }
                DemandOutcome::NotAttempted(_) => unresolved_cells.push(site),
            }
        }
        // Ruppert's protected-segment path has its own termination invariant:
        // accepted sites and split segments stay where the proof put them.
        // Moving even an unmarked interior site afterwards invalidates that
        // invariant, so r-adaptation is confined to unconstrained runs.
        if !mesh.segments_are_empty() {
            degree_blocked_sites.clear();
            pentagon_blocked_pairs.clear();
            balance_blocked_sites.clear();
        }
        let pentagon_sites: BTreeSet<usize> = pentagon_blocked_pairs
            .iter()
            .flat_map(|&(pentagon, demand)| [pentagon, demand])
            .collect();
        balance_blocked_sites.extend(pentagon_sites.iter().copied());
        // Insertion reports the vertex that would exceed the writer's degree
        // limit. Move that vertex -- not the cell that happened to ask -- and
        // legalize until one of its incident edges disappears. Doing this as a
        // phase avoids retrying the same blocker once per failed demand.
        balance_blocked_sites.extend(degree_blocked_sites.iter().copied());
        for blocked_site in degree_blocked_sites {
            if !mesh.can_move_site(blocked_site)
                || mesh.state().vertex_degree(blocked_site).ok() < Some(gates.max_vertex_degree)
            {
                continue;
            }
            // This phase removes one hard writer blocker. Scale has its own
            // phase below; scoring it here was expensive and admitted moves
            // that balanced a neighbourhood without lowering this degree.
            let objective =
                |state: &MeshState, _: &BTreeSet<usize>| state.vertex_degree(blocked_site).ok();
            let Ok(before) = mesh.state().vertex_degree(blocked_site) else {
                continue;
            };
            for destination in degree_relief_destinations(mesh.state(), blocked_site) {
                // `?`, not a discarded `Ok`: an error here means rollback
                // failed and the mesh cannot safely be used further.
                if let Acceptance::Committed(_) = mesh.propose_move_cached(
                    blocked_site,
                    destination,
                    gates,
                    &objective,
                    Some(&before),
                    false,
                )? {
                    relieved += 1;
                    r_adapted += 1;
                    adapted_this_cycle += 1;
                    break;
                }
            }
        }
        // A protected pentagon cannot gain a sixth neighbour. Move the pair
        // apart transactionally; the hard gate keeps the pentagon at degree
        // five, while the distance term permits the staged moves one edge
        // swap can require.
        for (pentagon, demand_site) in pentagon_blocked_pairs.iter().copied() {
            if sites_are_adjacent(mesh.state(), pentagon, demand_site) != Some(true) {
                continue;
            }
            let mut moved = false;
            for (index, moving_site) in [demand_site, pentagon].into_iter().enumerate() {
                if moved || (index == 1 && pentagon == demand_site) {
                    break;
                }
                if !mesh.can_move_site(moving_site) {
                    continue;
                }
                let objective = |state: &MeshState, _: &BTreeSet<usize>| {
                    let left = *state.vertices().get(pentagon)?;
                    let right = *state.vertices().get(demand_site)?;
                    let distance_squared = (left.x - right.x).powi(2)
                        + (left.y - right.y).powi(2)
                        + (left.z - right.z).powi(2);
                    Some([
                        f64::from(sites_are_adjacent(state, pentagon, demand_site)?),
                        -distance_squared,
                    ])
                };
                let Some(before) = objective(mesh.state(), &BTreeSet::new()) else {
                    continue;
                };
                for destination in degree_relief_destinations(mesh.state(), moving_site) {
                    if let Acceptance::Committed(_) = mesh.propose_move_cached(
                        moving_site,
                        destination,
                        gates,
                        &objective,
                        Some(&before),
                        false,
                    )? {
                        adapted_this_cycle += 1;
                        r_adapted += 1;
                        moved = true;
                        break;
                    }
                }
            }
        }
        for site in balance_blocked_sites {
            if !mesh.can_move_site(site) {
                continue;
            }
            let objective = |state: &MeshState, affected: &BTreeSet<usize>| {
                balance_objective(state, affected, limits.max_neighbour_scale_ratio)
            };
            let Some(before) = mesh.score_before_move(site, &objective) else {
                continue;
            };
            if before[0] == 0.0 && !pentagon_sites.contains(&site) {
                continue;
            }
            let mut destinations = balance_destinations(mesh, site);
            let mut relief = degree_relief_destinations(mesh.state(), site);
            if before[0] == 0.0 {
                // No bound is broken here. Keep the protected-pentagon escape
                // search bounded; broad search belongs to actual violations.
                relief.truncate(3);
            }
            destinations.extend(relief);
            for destination in destinations {
                if let Acceptance::Committed(_) = mesh.propose_move_cached(
                    site,
                    destination,
                    gates,
                    &objective,
                    Some(&before),
                    true,
                )? {
                    adapted_this_cycle += 1;
                    r_adapted += 1;
                    break;
                }
            }
        }
        cycles += 1;
        eprintln!(
            "harp_dv cycle {cycles}/{}: {} insertions, {} r-adaptations, {} unresolved ({} \
             angle-constrained), {} active cells",
            limits.max_cycles,
            accepted_this_cycle,
            adapted_this_cycle,
            unresolved_cells.len(),
            quality_constrained,
            mesh.active_site_count()
        );
        if out_of_budget {
            stop_reason = StopReason::BudgetReached;
            break;
        }
        // A cycle that accepted nothing would read the same mesh next time and
        // build the same demands. Stopping here rather than running out the
        // cycle limit is what keeps "unmet" distinguishable from "out of
        // cycles" in the report.
        if accepted_this_cycle == 0 {
            if adapted_this_cycle == 0 {
                stop_reason =
                    if quality_constrained == unresolved_cells.len() && quality_constrained > 0 {
                        StopReason::QualityConstraintReached
                    } else {
                        StopReason::NoAcceptedTransactions
                    };
                break;
            }
            let signature = (physical_demand_count, balance_demand_count);
            if adaptation_probe.is_some_and(|before: (usize, usize)| {
                signature.0 >= before.0 && signature.1 >= before.1
            }) {
                stop_reason = StopReason::NoProductiveAdaptation;
                break;
            }
            adaptation_probe = Some(signature);
        } else {
            adaptation_probe = None;
        }
    }

    // Counted at the end rather than tracked through: it is what a caller has
    // to decide on, and a number carried through the loop would be the one
    // from whichever cycle last looked.
    let unbalanced_pairs_remaining = unbalanced_pairs(mesh, limits);
    let final_sites = mesh.active_site_count();
    Ok(CycleOutcome {
        report: HarpDvRunReport {
            schema_version: HarpDvRunReport::SCHEMA_VERSION,
            cycles_completed: cycles,
            stop_reason,
            initial_sites,
            final_sites,
            transactions_attempted: attempted,
            transactions_committed: committed,
            transactions_rolled_back: rolled_back,
            balance_transactions_committed: balanced,
            refusals,
            degree_relieving_moves: relieved,
            r_adaptation_moves: r_adapted,
            unbalanced_pairs_remaining,
            unresolved_count: unresolved_cells.len(),
            quality_constrained_count: quality_constrained,
            deterministic: true,
        },
        unresolved_cells,
    })
}

#[cfg(test)]
mod tests;
