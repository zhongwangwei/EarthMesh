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

use std::collections::BTreeMap;

use earthmesh_mesh::{lonlat_degrees_to_unit_xyz, magnitude, CartesianPoint, MESH_STATE_FIRST_ID};
use earthmesh_refine::{
    order_demands, CriterionSemantics, DemandEvidence, RefinementCause, RefinementDemand,
};

use crate::candidate::CandidatePolicy;
use crate::criteria::{CellCriterion, CellView};
use crate::error::Result;
use crate::report::{HarpDvRunReport, StopReason};
use crate::state::AdaptiveMesh;
use crate::transaction::{DemandOutcome, HardGates};

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
            attempted += 1;
            let for_balance = matches!(demand.cause, RefinementCause::ScaleBalance { .. });
            match mesh.refine_cell(site, witness, policy, gates)? {
                DemandOutcome::Resolved { .. } => {
                    committed += 1;
                    accepted_this_cycle += 1;
                    if for_balance {
                        balanced += 1;
                    }
                }
                DemandOutcome::Unresolved { refusals } => {
                    rolled_back += refusals.len();
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
            unbalanced_pairs_remaining,
            unresolved_count: unresolved_cells.len(),
            deterministic: true,
        },
        unresolved_cells,
    })
}

#[cfg(test)]
mod tests;
