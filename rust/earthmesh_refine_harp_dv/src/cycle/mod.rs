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
fn balance_demands(mesh: &AdaptiveMesh, limits: CycleLimits) -> Vec<RefinementDemand> {
    let state = mesh.state();
    let scales = scales(mesh);
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
/// Not zero in general. Closing the last of them takes cells the degree gate
/// refuses, and insertion is this backend's only move -- section 8.1's
/// r-adaptation, which would resolve it by moving sites rather than adding
/// them, is not implemented. Reported so a caller can decide, rather than left
/// for them to discover.
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

/// The largest vertex degree in a site's fan, counting the site itself.
///
/// What a degree-relieving move has to lower. Section 11.13 measured the degree
/// bound as 96% of everything this backend cannot do, and section 11.14 that
/// motion alone cannot change a degree -- so the move this feeds is motion
/// *and* legalization, and the flips are what redistribute degree.
fn neighbourhood_max_degree(state: &MeshState, site: usize) -> usize {
    let Ok(fan) = state.triangle_fan(site) else {
        return usize::MAX;
    };
    let mut region: BTreeSet<usize> = BTreeSet::new();
    for triangle in &fan {
        for corner in state.triangles()[*triangle] {
            region.insert(corner);
        }
    }
    region
        .iter()
        .filter_map(|&member| state.vertex_degree(member).ok())
        .max()
        .unwrap_or(usize::MAX)
}

/// The centroid of a site's neighbours, on the sphere: one relaxation step.
///
/// Unweighted, unlike `balance_destination`. This move is not trying to even
/// out scales -- it is trying to give the legalization that follows a
/// configuration whose flips lower degree, and the even spacing is what does
/// that.
fn relaxation_destination(state: &MeshState, site: usize) -> Option<CartesianPoint> {
    const STEP: f64 = 0.5;
    let here = state.vertices()[site];
    let radius = magnitude(here);
    let fan = state.triangle_fan(site).ok()?;
    let mut neighbours: BTreeSet<usize> = BTreeSet::new();
    for triangle in &fan {
        for corner in state.triangles()[*triangle] {
            if corner != site {
                neighbours.insert(corner);
            }
        }
    }
    if neighbours.is_empty() {
        return None;
    }
    let mut centroid = CartesianPoint::new(0.0, 0.0, 0.0);
    for &corner in &neighbours {
        let point = state.vertices()[corner];
        centroid = CartesianPoint::new(
            centroid.x + point.x,
            centroid.y + point.y,
            centroid.z + point.z,
        );
    }
    let count = neighbours.len() as f64;
    let stepped = CartesianPoint::new(
        here.x + (centroid.x / count - here.x) * STEP,
        here.y + (centroid.y / count - here.y) * STEP,
        here.z + (centroid.z / count - here.z) * STEP,
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

/// How badly the neighbourhood around a site breaks the scale bound.
///
/// Not called. Three improvement gates were built and measured, each wider
/// than the last, and each lowered the violation count without closing the
/// bound or letting the run converge (guide 11.9). Kept because the sequence
/// is the finding: the objective has to be global, and these are the local
/// ones already ruled out.
#[allow(dead_code)]
///
/// Sum of squared excess over every adjacent pair in the site's fan and one
/// ring out -- not the site's own worst ratio, which is what the first attempt
/// at an improvement gate measured and why it failed. Moving a site changes
/// every pair around it at once, so a gate reading one pair accepts moves that
/// improve that pair and push the violation next door. Guide section 11.9.
///
/// Local, because the whole point of a per-transaction gate is that it costs
/// the neighbourhood rather than the mesh (section 11.7).
fn neighbourhood_violation(state: &MeshState, site: usize, bound: f64) -> f64 {
    let radius_m = state.sphere_radius();
    let scale = |site: usize| {
        let cell = state.voronoi_cell(site).ok()?;
        CellView {
            site,
            cell: &cell,
            state,
            radius_m,
        }
        .effective_scale_m()
    };
    let Ok(fan) = state.triangle_fan(site) else {
        return f64::INFINITY;
    };
    // Every site in the fan, and every site in theirs: the ring whose pairs a
    // move can shift.
    let mut region: BTreeSet<usize> = BTreeSet::new();
    for triangle in &fan {
        for corner in state.triangles()[*triangle] {
            region.insert(corner);
        }
    }
    let inner: Vec<usize> = region.iter().copied().collect();
    for &member in &inner {
        if let Ok(their_fan) = state.triangle_fan(member) {
            for triangle in their_fan {
                for corner in state.triangles()[triangle] {
                    region.insert(corner);
                }
            }
        }
    }

    let mut total = 0.0;
    let mut seen: BTreeSet<(usize, usize)> = BTreeSet::new();
    for &member in &region {
        let Some(here) = scale(member) else {
            continue;
        };
        let Ok(their_fan) = state.triangle_fan(member) else {
            continue;
        };
        for triangle in their_fan {
            for corner in state.triangles()[triangle] {
                if corner == member || !region.contains(&corner) {
                    continue;
                }
                let key = (member.min(corner), member.max(corner));
                if !seen.insert(key) {
                    continue;
                }
                let Some(there) = scale(corner) else { continue };
                let ratio = here.max(there) / here.min(there);
                if ratio > bound {
                    let excess = ratio - bound;
                    total += excess * excess;
                }
            }
        }
    }
    total
}

/// The worst neighbour scale ratio at one site.
#[allow(dead_code)]
///
/// Local: the improvement gate compares this before and after a move, and a
/// global objective would cost the mesh per transaction -- the shape guide
/// section 11.7 records.
fn worst_ratio_at(state: &MeshState, site: usize, limits: CycleLimits) -> f64 {
    let radius_m = state.sphere_radius();
    let scale = |site: usize| {
        let cell = state.voronoi_cell(site).ok()?;
        CellView {
            site,
            cell: &cell,
            state,
            radius_m,
        }
        .effective_scale_m()
    };
    let Some(here) = scale(site) else {
        return f64::INFINITY;
    };
    let Ok(fan) = state.triangle_fan(site) else {
        return f64::INFINITY;
    };
    let mut worst = 1.0_f64;
    for triangle in fan {
        for corner in state.triangles()[triangle] {
            if corner == site {
                continue;
            }
            if let Some(there) = scale(corner) {
                worst = worst.max(here.max(there) / here.min(there));
            }
        }
    }
    let _ = limits;
    worst
}

/// Where to move a site to even out the scales around it.
///
/// Not called; see `neighbourhood_violation` for why the balance path does not
/// move sites today.
#[allow(dead_code)]
///
/// Toward the centroid of its neighbours, weighted so the coarse side pulls
/// harder: a site sitting between a fine neighbourhood and a coarse one is
/// what makes the ratio, and moving it into the coarse side shrinks the coarse
/// cell and grows the fine one at once.
///
/// A fraction of the way, not all of it. A site landing on the centroid would
/// overshoot past the balance it was moving toward, and the next cycle would
/// move it back.
fn balance_destination(mesh: &AdaptiveMesh, site: usize) -> Option<CartesianPoint> {
    const STEP: f64 = 0.25;
    let state = mesh.state();
    let here = state.vertices()[site];
    let radius = magnitude(here);
    let fan = state.triangle_fan(site).ok()?;
    let mut neighbours = BTreeMap::new();
    for triangle in &fan {
        for corner in state.triangles()[*triangle] {
            if corner != site {
                neighbours.insert(corner, ());
            }
        }
    }
    if neighbours.is_empty() {
        return None;
    }
    let scales = scales(mesh);
    let mut total = 0.0;
    let mut target = CartesianPoint::new(0.0, 0.0, 0.0);
    for &corner in neighbours.keys() {
        // A *fine* neighbour pulls hardest. The demand was raised on the
        // coarser cell, so the ratio falls when this site moves toward the
        // fine side: the coarse cell loses area there and the fine one gains
        // it. Weighting the other way -- toward the coarse side -- was tried
        // first and measured worse than doing nothing.
        let scale = scales[corner].unwrap_or(0.0);
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
        return None;
    }
    let centroid = CartesianPoint::new(target.x / total, target.y / total, target.z / total);
    let stepped = CartesianPoint::new(
        here.x + (centroid.x - here.x) * STEP,
        here.y + (centroid.y - here.y) * STEP,
        here.z + (centroid.z - here.z) * STEP,
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

/// Read every active cell once.
fn evaluate(
    mesh: &AdaptiveMesh,
    criteria: &[Box<dyn CellCriterion>],
    limits: CycleLimits,
) -> Result<Vec<RefinementDemand>> {
    let state = mesh.state();
    let radius_m = state.sphere_radius();
    let mut demands = Vec::new();
    for site in MESH_STATE_FIRST_ID..state.vertices().len() {
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
        if view
            .effective_scale_m()
            .is_some_and(|scale| scale <= limits.minimum_cell_width_m)
        {
            continue;
        }
        let mut evidences = Vec::with_capacity(criteria.len());
        for criterion in criteria {
            evidences.push(criterion.evaluate(&view)?);
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
        let demand = RefinementDemand::from_evidence(site as u64, evidences, cause);
        if demand.demands_work() {
            demands.push(demand);
        }
    }
    Ok(demands)
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
    let mut unresolved_cells = Vec::new();
    let mut cycles = 0u32;
    let mut stop_reason = StopReason::MaximumCyclesReached;

    while cycles < limits.max_cycles {
        let mut demands = evaluate(mesh, criteria, limits)?;
        // Section 14, folded into the same list rather than run as a pass of
        // its own: one loop serves both, and `RefinementCause` is what keeps
        // physical refinement and transition balance apart in the report --
        // which the spec says is the reason the distinction exists.
        demands.extend(balance_demands(mesh, limits));
        if demands.is_empty() {
            stop_reason = StopReason::AllSatisfied;
            break;
        }
        order_demands(&mut demands);

        let mut accepted_this_cycle = 0usize;
        let mut out_of_budget = false;
        unresolved_cells.clear();
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
                    // A demand the degree bound turned away is the one case
                    // worth a second move: relax the neighbourhood and let the
                    // legalization redistribute degree, so the next cycle can
                    // try the same demand into a mesh with room. Only when
                    // degree was the reason -- anything else is a different
                    // problem and this would not touch it.
                    if reasons
                        .iter()
                        .any(|(_, reason)| matches!(reason, Rejection::DegreeOverBudget { .. }))
                    {
                        if let Some(destination) = relaxation_destination(mesh.state(), site) {
                            let before = neighbourhood_max_degree(mesh.state(), site);
                            let improves =
                                |state: &MeshState| neighbourhood_max_degree(state, site) < before;
                            // `?`, not a discarded `Ok`. The only error this
                            // returns is a rollback that failed, which leaves
                            // the mesh inconsistent -- swallowing it would
                            // carry on refining a mesh nobody can trust.
                            if let Acceptance::Committed(_) =
                                mesh.propose_move(site, destination, gates, &improves)?
                            {
                                relieved += 1;
                                // Deliberately *not* counted as an accepted
                                // transaction. A relief move serves no demand,
                                // so a cycle that only relieves has made no
                                // progress on what was asked -- counting it
                                // would keep `NoAcceptedTransactions` from ever
                                // firing and let such a run spin to the cycle
                                // limit reporting the wrong reason.
                            }
                        }
                    }
                    for (_, reason) in &reasons {
                        match reason {
                            Rejection::DegreeOverBudget { .. } => refusals.degree += 1,
                            Rejection::ProtectedPentagonDisturbed { .. } => refusals.pentagon += 1,
                            Rejection::NotInsertable(_) => refusals.not_insertable += 1,
                            // Counted with topology: like the others there, it
                            // is the change being too big to undo safely rather
                            // than the demand being unreachable.
                            Rejection::PatchTooLarge { .. } => refusals.topology += 1,
                            Rejection::SurfaceOpened { .. }
                            | Rejection::TopologyInvalid { .. }
                            | Rejection::CouldNotLegalize(_) => refusals.topology += 1,
                            Rejection::SliverTriangle { .. } => refusals.sliver += 1,
                            Rejection::NoImprovement { .. } => refusals.no_improvement += 1,
                            Rejection::Unmeasurable(_) => refusals.unmeasurable += 1,
                        }
                    }
                    unresolved_cells.push(site);
                }
                DemandOutcome::NotAttempted(_) => unresolved_cells.push(site),
            }
        }
        cycles += 1;
        if out_of_budget {
            stop_reason = StopReason::BudgetReached;
            break;
        }
        // A cycle that accepted nothing would read the same mesh next time and
        // build the same demands. Stopping here rather than running out the
        // cycle limit is what keeps "unmet" distinguishable from "out of
        // cycles" in the report.
        if accepted_this_cycle == 0 {
            stop_reason = StopReason::NoAcceptedTransactions;
            break;
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
            unbalanced_pairs_remaining,
            unresolved_count: unresolved_cells.len(),
            deterministic: true,
        },
        unresolved_cells,
    })
}

#[cfg(test)]
mod tests;
