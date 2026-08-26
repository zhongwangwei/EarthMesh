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

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use earthmesh_mesh::{
    arc_length_unit_sphere, cross, lonlat_degrees_to_unit_xyz, magnitude, xyz_to_lonlat_degrees,
    CartesianPoint, Sign, MESH_STATE_FIRST_ID,
};
use earthmesh_refine::{
    order_demands, CriterionSemantics, DemandEvidence, RefinementCause, RefinementDemand,
};

use crate::candidate::CandidatePolicy;
use crate::criteria::{CellCriterion, CellView};
use crate::error::Result;
use crate::report::{AngleWindowVerdict, HarpDvRunReport, RejectionTally, StopReason};
use crate::state::{AdaptiveMesh, SiteId, SiteMobility};
use crate::transaction::{check, Acceptance, AffectedSites, DemandOutcome, HardGates, Rejection};
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
    /// Cells still demanding work after the final insertion or site move.
    pub unresolved_cells: Vec<usize>,
}

/// Every cell's effective scale, indexed by site. `None` where unreadable.
/// One triangle per site, for walks that would otherwise scan to find it.
///
/// `MeshState::triangle_fan` finds its seed with a linear `find` over the
/// active triangles, which is affordable once and quadratic for a full sweep
/// that does it per site -- the mesh's own doc comments say so and point at
/// `triangle_fan_from`. This builds the whole seed table in one pass instead.
///
/// The seed must be the *first* active triangle naming each site, because that
/// is what the scan returns: it fixes where the fan starts, and with it the
/// order the Voronoi corners are visited and the order their areas are summed.
/// Any other choice would still be a correct cell and would not be the same
/// float.
fn active_site_triangle_seeds(state: &MeshState) -> Vec<Option<usize>> {
    let mut seeds = vec![None; state.vertices().len()];
    for triangle in state.active_triangle_slots() {
        for corner in state.triangles()[triangle] {
            if seeds[corner].is_none() {
                seeds[corner] = Some(triangle);
            }
        }
    }
    seeds
}

fn state_scales(state: &MeshState) -> Vec<Option<f64>> {
    let radius_m = state.sphere_radius();
    let seeds = active_site_triangle_seeds(state);
    let mut scales = vec![None; state.vertices().len()];
    for site in state.active_vertex_slots() {
        let Some(seed) = seeds[site] else {
            continue;
        };
        let Ok(cell) = state.voronoi_cell_from(site, seed) else {
            continue;
        };
        scales[site] = CellView {
            site,
            cell: &cell,
            state,
            radius_m,
        }
        .effective_scale_m();
    }
    scales
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
    for (site, corner) in directed_neighbour_pairs(state) {
        let (Some(here), Some(there)) = (scales[site], scales[corner]) else {
            continue;
        };
        if here <= limits.minimum_cell_width_m {
            continue;
        }
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
    balance_demands_from_worst(worst, limits)
}

/// Turn the worst ratio per cell into the demands the ladder consumes.
///
/// Split out so a reference implementation of the sweep above can produce
/// demands through exactly this code and differ only where it is meant to.
fn balance_demands_from_worst(
    worst: BTreeMap<usize, f64>,
    limits: CycleLimits,
) -> Vec<RefinementDemand> {
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
#[cfg(test)]
fn balance_survey(mesh: &AdaptiveMesh, limits: CycleLimits) -> (usize, f64) {
    balance_survey_state(mesh.state(), limits)
}

fn balance_survey_state(state: &MeshState, limits: CycleLimits) -> (usize, f64) {
    balance_survey_from_scales(state, &state_scales(state), limits)
}

/// The same survey against scales the caller already has.
fn balance_survey_from_scales(
    state: &MeshState,
    scales: &[Option<f64>],
    limits: CycleLimits,
) -> (usize, f64) {
    let mut over = 0;
    let mut worst = 1.0_f64;
    for (site, corner) in directed_neighbour_pairs(state) {
        let (Some(here), Some(there)) = (scales[site], scales[corner]) else {
            continue;
        };
        let ratio = here.max(there) / here.min(there);
        worst = worst.max(ratio);
        if ratio > limits.max_neighbour_scale_ratio {
            over += 1;
        }
    }
    (over, worst)
}

/// Every ordered corner pair of every active triangle.
///
/// Walking the triangles once replaces a scanned fan per site. The repeats are
/// deliberate and load-bearing: an undirected edge is visited by both incident
/// triangles and in both directions, and `over` has always counted it that many
/// times. De-duplicating here would quietly change what the balance survey
/// reports, so the traversal reproduces the old multiset exactly.
fn directed_neighbour_pairs(state: &MeshState) -> impl Iterator<Item = (usize, usize)> + '_ {
    state.active_triangle_slots().flat_map(|triangle| {
        let [a, b, c] = state.triangles()[triangle];
        [(a, b), (a, c), (b, a), (b, c), (c, a), (c, b)]
    })
}

/// The part of the global scale objective that a local move can change.
///
/// Every edge not incident to `sites` contributes the same value before and
/// after the move, so comparing this tuple is exactly the global objective
/// delta without rescanning the whole mesh for every candidate.
fn balance_objective(state: &MeshState, sites: &AffectedSites, bound: f64) -> Option<[f64; 3]> {
    let mut edges = BTreeSet::new();
    // A seed for the ring's own sites too: every triangle naming a corner is a
    // valid start for that corner's fan, and `voronoi_cell_from` pins the ring
    // to its lowest triangle, so which one is picked cannot reach the result.
    let mut seeds = sites.clone();
    let radius_m = state.sphere_radius();
    let mut cached = BTreeMap::new();
    for (&site, &seed) in sites {
        // The cell carries the fan that built it, so this loop reads each of
        // these sites once. Walking the ring here for the edges and again for
        // the scale below was the same walk twice, and the ring walk is what
        // the objective now spends nearly all of its time in.
        let cell = state.voronoi_cell_from(site, seed).ok()?;
        for &triangle in &cell.triangles {
            for corner in state.triangles()[triangle] {
                seeds.entry(corner).or_insert(triangle);
                if corner != site {
                    edges.insert((site.min(corner), site.max(corner)));
                }
            }
        }
        cached.insert(
            site,
            CellView {
                site,
                cell: &cell,
                state,
                radius_m,
            }
            .effective_scale_m()?,
        );
    }
    let mut violations = 0usize;
    let mut worst = 1.0_f64;
    let mut energy = 0.0;
    for (left, right) in edges {
        let mut scale = |site| {
            if let Some(value) = cached.get(&site) {
                return Some(*value);
            }
            let value = site_scale_from(state, site, *seeds.get(&site)?, radius_m)?;
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

/// The scale of one cell, at a radius the caller already has.
///
/// `MeshState::sphere_radius` averages over every active vertex, so asking it
/// per site inside a loop is a full pass over the mesh per iteration for a
/// value that cannot change while the loop runs.
#[cfg(test)]
fn site_scale(state: &MeshState, site: usize, radius_m: f64) -> Option<f64> {
    site_scale_from(state, site, state.triangle_fan(site).ok()?[0], radius_m)
}

/// The same, from a triangle the caller already knows names the site.
fn site_scale_from(state: &MeshState, site: usize, seed: usize, radius_m: f64) -> Option<f64> {
    let cell = state.voronoi_cell_from(site, seed).ok()?;
    CellView {
        site,
        cell: &cell,
        state,
        radius_m,
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

/// Paired positions that can cross a Delaunay flip boundary which neither
/// endpoint can cross alone under the strict-improvement gate.
fn degree_relief_pairs(
    state: &MeshState,
    site: usize,
) -> Vec<(usize, CartesianPoint, CartesianPoint)> {
    let here = state.vertices()[site];
    let mut candidates = Vec::new();
    for neighbour in neighbour_sites(state, site) {
        let there = state.vertices()[neighbour];
        let here_away = CartesianPoint::new(
            here.x + (here.x - there.x),
            here.y + (here.y - there.y),
            here.z + (here.z - there.z),
        );
        let there_away = CartesianPoint::new(
            there.x + (there.x - here.x),
            there.y + (there.y - here.y),
            there.z + (there.z - here.z),
        );
        for step in [0.25, 0.125] {
            let Some(first) = projected_step(here, here_away, step) else {
                continue;
            };
            let Some(second) = projected_step(there, there_away, step) else {
                continue;
            };
            candidates.push((neighbour, first, second));
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

fn collect_blockers(
    demand_site: usize,
    for_balance: bool,
    reasons: &[(crate::candidate::CandidateSource, Rejection)],
    degree_blocked_sites: &mut BTreeSet<usize>,
    pentagon_blocked_pairs: &mut BTreeSet<(usize, usize)>,
    balance_blocked_sites: &mut BTreeSet<usize>,
    stalled_demand_sites: &mut BTreeSet<usize>,
) -> bool {
    // The cell that asked and did not get served, whatever it asked for. The
    // move phase below is seeded from balance demands and from the vertices a
    // degree rejection named; a physical demand that neither balances nor
    // blocks anyone reaches no phase at all, so the only thing ever tried on
    // its behalf is the ladder that just refused it.
    stalled_demand_sites.insert(demand_site);
    degree_blocked_sites.extend(reasons.iter().filter_map(|(_, reason)| match reason {
        Rejection::DegreeOverBudget { site, .. } => Some(*site),
        _ => None,
    }));
    pentagon_blocked_pairs.extend(reasons.iter().filter_map(|(_, reason)| match reason {
        Rejection::ProtectedPentagonDisturbed { site: pentagon, .. } => {
            Some((*pentagon, demand_site))
        }
        _ => None,
    }));
    if for_balance {
        balance_blocked_sites.insert(demand_site);
    }
    !reasons.is_empty()
        && reasons
            .iter()
            .all(|(_, reason)| matches!(reason, Rejection::SliverTriangle { .. }))
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

/// How a neighbourhood scores, on every bound the recovery may not spend.
///
/// All numbers are lower-is-better and compared by Pareto dominance rather than
/// in a fixed order: the recovery is allowed to trade nothing. A move is an
/// improvement only when it leaves every one of these no worse and at least one
/// of them strictly better, which is what keeps a sweep from buying a smaller
/// residue with a thinner triangle or a seventh neighbour somewhere else.
///
/// Built only from finite values -- the constructor returns `None` otherwise --
/// so the `PartialOrd` below never sees a NaN, and the transaction layer's
/// `after < before` test cannot be satisfied by an unordered comparison.
#[derive(Clone, Copy, Debug, PartialEq)]
struct RegionScore {
    /// Unique sites carrying either a physical or a balance demand.
    pending: usize,
    /// Cells in the region a criterion still asks for.
    unresolved: usize,
    /// Adjacent pairs inside the region past the neighbour-scale bound.
    unbalanced: usize,
    /// Saturated sites: at the degree budget, so nothing can be inserted beside
    /// them.
    saturated: usize,
    /// The worst neighbour-scale ratio in the region.
    worst_ratio: f64,
    /// The smallest triangle angle in the region, negated so lower is better
    /// like the rest.
    negated_min_angle_deg: f64,
}

impl PartialOrd for RegionScore {
    /// Pareto dominance. `Less` means "no worse anywhere, better somewhere".
    ///
    /// Deliberately partial: two scores that each win on a different axis are
    /// incomparable, and `propose_move_cached` reads anything other than `Less`
    /// as no improvement. That is the whole acceptance rule the recovery needs,
    /// expressed in the ordering the existing transaction API already asks for.
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let axes = [
            (self.pending as f64, other.pending as f64),
            (self.unresolved as f64, other.unresolved as f64),
            (self.unbalanced as f64, other.unbalanced as f64),
            (self.saturated as f64, other.saturated as f64),
            (self.worst_ratio, other.worst_ratio),
            (self.negated_min_angle_deg, other.negated_min_angle_deg),
        ];
        let mut better = false;
        let mut worse = false;
        for (mine, theirs) in axes {
            if mine < theirs {
                better = true;
            } else if mine > theirs {
                worse = true;
            }
        }
        match (better, worse) {
            (true, false) => Some(std::cmp::Ordering::Less),
            (false, true) => Some(std::cmp::Ordering::Greater),
            (false, false) => Some(std::cmp::Ordering::Equal),
            (true, true) => None,
        }
    }
}

/// A site's triangle fan, reusing a seed from the last time it was read.
///
/// The fan walk is `O(degree)` from a triangle that names the site and linear in
/// the whole mesh without one. A move rewrites at most its own two rings, so
/// almost every cached seed survives almost every move; the scan is the
/// fallback, not the path.
fn fan_with_cached_seed(
    state: &MeshState,
    site: usize,
    seeds: &std::cell::RefCell<BTreeMap<usize, usize>>,
) -> Option<Vec<usize>> {
    let cached = seeds.borrow().get(&site).copied();
    if let Some(seed) = cached {
        if let Ok(fan) = state.triangle_fan_from(site, seed) {
            return Some(fan);
        }
    }
    let fan = state.triangle_fan(site).ok()?;
    seeds.borrow_mut().insert(site, *fan.first()?);
    Some(fan)
}

/// Score one region as it stands.
///
/// Reads the same criteria the driver reads and the same neighbour-scale bound
/// the balance demands are raised from, so "unresolved" here means what it means
/// in the report rather than a proxy for it.
/// One measured site's contribution to a region score.
type RegionCell = (std::rc::Rc<Vec<usize>>, f64, bool);

/// The region's cells as they stand before any candidate move.
///
/// A recovery scores the whole region once per candidate, and a candidate only
/// changes the cells around the site it moves. Everything else is the same
/// answer rebuilt -- and rebuilding it is the ring walk, which a sample puts at
/// 97% of the score.
type RegionCells = std::cell::RefCell<BTreeMap<usize, RegionCell>>;

/// `dirty` names the sites whose cells this state may have changed, which is
/// what the transaction hands the objective. They are measured fresh and not
/// stored, so a rejected candidate leaves the cache describing the base state
/// it was scored against; a committed one clears it.
#[allow(clippy::too_many_arguments)]
fn region_score(
    state: &MeshState,
    criteria: &[Box<dyn CellCriterion>],
    gates: HardGates,
    limits: CycleLimits,
    region: &BTreeSet<usize>,
    seeds: &std::cell::RefCell<BTreeMap<usize, usize>>,
    cells: &RegionCells,
    dirty: &AffectedSites,
) -> Option<RegionScore> {
    let radius_m = state.sphere_radius();
    // A fixed guard site's Voronoi cell can still change when a movable site
    // beside it moves. Measure the neighbour on the far side as well, or the
    // score misses exactly the ratio that a local recovery can push across its
    // boundary.
    // One entry per measured site, taken from the cache whenever this state did
    // not disturb it. Collecting the sites to measure needs the region's fans,
    // and so does the sweep below, so both come from the same lookup -- walking
    // the ring here and again there was the shape of the previous attempt, and
    // it left the cache reaching only the sites outside the region.
    let mut entries: BTreeMap<usize, RegionCell> = BTreeMap::new();
    let measure = |site: usize, entries: &mut BTreeMap<usize, RegionCell>| -> Option<()> {
        if entries.contains_key(&site) {
            return Some(());
        }
        let stale = dirty.contains_key(&site);
        if !stale {
            if let Some(entry) = cells.borrow().get(&site) {
                entries.insert(site, entry.clone());
                return Some(());
            }
        }
        let fan = fan_with_cached_seed(state, site, seeds)?;
        let cell = state.voronoi_cell_from(site, *fan.first()?).ok()?;
        let view = CellView {
            site,
            cell: &cell,
            state,
            radius_m,
        };
        let scale = view.effective_scale_m()?;
        if !scale.is_finite() || scale <= 0.0 {
            return None;
        }
        let mut demanding = false;
        if region.contains(&site) && scale > limits.minimum_cell_width_m {
            for criterion in criteria {
                if criterion.evaluate(&view).ok()?.demands_work() {
                    demanding = true;
                    break;
                }
            }
        }
        let entry = (std::rc::Rc::new(fan), scale, demanding);
        if !stale {
            cells.borrow_mut().insert(site, entry.clone());
        }
        entries.insert(site, entry);
        Some(())
    };

    let mut measured_sites = region.clone();
    for &site in region {
        measure(site, &mut entries)?;
        let fan = std::rc::Rc::clone(&entries.get(&site)?.0);
        measured_sites.extend(
            fan.iter()
                .flat_map(|&triangle| state.triangles()[triangle])
                .filter(|&corner| corner != site),
        );
    }

    let mut scales: BTreeMap<usize, f64> = BTreeMap::new();
    let mut fans: BTreeMap<usize, std::rc::Rc<Vec<usize>>> = BTreeMap::new();
    let mut unresolved = 0usize;
    let mut pending_sites = BTreeSet::new();
    let mut saturated = 0usize;
    for site in measured_sites {
        measure(site, &mut entries)?;
        let (fan, scale, demanding) = entries.get(&site)?.clone();
        if region.contains(&site) {
            if fan.len() >= gates.max_vertex_degree {
                saturated += 1;
            }
            if demanding {
                unresolved += 1;
                pending_sites.insert(site);
            }
            fans.insert(site, fan);
        }
        scales.insert(site, scale);
    }

    let mut unbalanced = 0usize;
    let mut worst_ratio = 1.0_f64;
    let mut smallest_angle = f64::MAX;
    let mut counted = BTreeSet::new();
    let mut counted_edges = BTreeSet::new();
    for (&site, fan) in &fans {
        for &triangle in fan.iter() {
            let corners = state.triangles()[triangle];
            if counted.insert(triangle) {
                let angle = crate::criteria::smallest_triangle_angle_deg([
                    state.vertices()[corners[0]],
                    state.vertices()[corners[1]],
                    state.vertices()[corners[2]],
                ]);
                if !angle.is_finite() {
                    return None;
                }
                smallest_angle = smallest_angle.min(angle);
            }
            for corner in corners {
                if corner == site {
                    continue;
                }
                // Each undirected pair once. ID ordering alone cannot dedupe:
                // an outside neighbour is never traversed through `fans`, so a
                // smaller outside ID would otherwise make the boundary edge
                // disappear from the score.
                let edge = (site.min(corner), site.max(corner));
                if !counted_edges.insert(edge) {
                    continue;
                }
                let (Some(&here), Some(&there)) = (scales.get(&site), scales.get(&corner)) else {
                    continue;
                };
                let ratio = here.max(there) / here.min(there);
                if !ratio.is_finite() {
                    return None;
                }
                worst_ratio = worst_ratio.max(ratio);
                if ratio > limits.max_neighbour_scale_ratio {
                    unbalanced += 1;
                    // Balance demands are raised against the coarser site.
                    pending_sites.insert(if here >= there { site } else { corner });
                }
            }
        }
    }
    if smallest_angle == f64::MAX {
        return None;
    }
    Some(RegionScore {
        pending: pending_sites.len(),
        unresolved,
        unbalanced,
        saturated,
        worst_ratio,
        negated_min_angle_deg: -smallest_angle,
    })
}

/// Breadth-first rings out from a set of sites, over mesh adjacency.
///
/// Rings of the triangulation, not rows of a lon/lat grid: what bounds a
/// recovery is how many edges away a site is from the cell that stalled, and on
/// a sphere those two are not the same shape anywhere.
fn topological_rings(
    state: &MeshState,
    centre: &BTreeSet<usize>,
    depth: usize,
) -> Vec<BTreeSet<usize>> {
    let mut rings = Vec::with_capacity(depth + 1);
    let mut seen: BTreeSet<usize> = centre.clone();
    rings.push(centre.clone());
    for _ in 0..depth {
        let mut next = BTreeSet::new();
        for &site in rings
            .last()
            .expect("the centre ring is always pushed first")
        {
            for neighbour in neighbour_sites(state, site) {
                if neighbour >= MESH_STATE_FIRST_ID && seen.insert(neighbour) {
                    next.insert(neighbour);
                }
            }
        }
        rings.push(next);
    }
    rings
}

fn union_find_root(parent: &mut [usize], mut index: usize) -> usize {
    while parent[index] != index {
        parent[index] = parent[parent[index]];
        index = parent[index];
    }
    index
}

/// Group stalled sites whose neighbourhoods overlap.
///
/// Two seeds within reach of one another describe one piece of geometry, and
/// sweeping it twice would let the second sweep undo what the first paid for.
/// Merging is by the rings themselves, so it follows the mesh rather than a
/// distance in metres.
fn residual_components(
    state: &MeshState,
    seeds: &BTreeSet<usize>,
    depth: usize,
) -> Vec<BTreeSet<usize>> {
    let mut owner: BTreeMap<usize, usize> = BTreeMap::new();
    let mut parent: Vec<usize> = Vec::new();
    let mut groups: Vec<BTreeSet<usize>> = Vec::new();
    for &seed in seeds {
        let index = groups.len();
        parent.push(index);
        groups.push([seed].into_iter().collect());
        let single = [seed].into_iter().collect();
        for site in topological_rings(state, &single, depth)
            .into_iter()
            .flatten()
        {
            match owner.get(&site).copied() {
                Some(other) => {
                    let (mine, theirs) = (
                        union_find_root(&mut parent, index),
                        union_find_root(&mut parent, other),
                    );
                    if mine != theirs {
                        parent[mine] = theirs;
                    }
                }
                None => {
                    owner.insert(site, index);
                }
            }
        }
    }
    let mut merged: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for (index, group) in groups.into_iter().enumerate() {
        let root = union_find_root(&mut parent, index);
        merged.entry(root).or_default().extend(group);
    }
    merged.into_values().collect()
}

/// Loosen the neighbourhoods that stalled, without spending any bound.
///
/// The shipped relief phases move one site: the vertex a degree rejection named,
/// or the cell a balance demand was raised against. Measured on the production
/// run, on the geometry it finished with, those phases commit nothing at all --
/// every single-site move around a stalled cell is either illegal or no
/// improvement. The room is one ring further out, and nothing ever looked there.
///
/// So this sweeps a few rings of the triangulation around each stalled region,
/// inward first to free degree headroom at the edge and then outward to even the
/// scales back out, and asks the *region* whether each move helped rather than
/// asking the one site being moved. Every proposal is an ordinary transaction:
/// same destinations, same hard gates, same Delaunay legalization, same rollback.
///
/// `movable_rings` is how far out sites may move; the ring beyond it is held
/// fixed, so a sweep can never push its own disturbance into a neighbourhood
/// nobody scored.
pub(crate) fn recover_stalled_regions(
    mesh: &mut AdaptiveMesh,
    criteria: &[Box<dyn CellCriterion>],
    gates: HardGates,
    limits: CycleLimits,
    seeds: &BTreeSet<usize>,
    movable_rings: usize,
) -> Result<usize> {
    // Ruppert's protected-segment runs keep their sites where the proof put
    // them, exactly as the single-site phases do.
    if seeds.is_empty() || !mesh.segments_are_empty() {
        return Ok(0);
    }
    let mut committed = 0usize;
    for centre in residual_components(mesh.state(), seeds, movable_rings) {
        committed += recover_one_region(mesh, criteria, gates, limits, &centre, movable_rings)?;
    }
    Ok(committed)
}

/// At most four sweeps over one region: outward-in, then inward-out.
fn recover_one_region(
    mesh: &mut AdaptiveMesh,
    criteria: &[Box<dyn CellCriterion>],
    gates: HardGates,
    limits: CycleLimits,
    centre: &BTreeSet<usize>,
    movable_rings: usize,
) -> Result<usize> {
    let rings = topological_rings(mesh.state(), centre, movable_rings + 1);
    // The guard ring is in the score but not in the sweep: what a move does to
    // the cells just outside it is still the recovery's business.
    let region: BTreeSet<usize> = rings.iter().flatten().copied().collect();
    let frozen = rings.last().cloned().unwrap_or_default();
    let seeds = std::cell::RefCell::new(BTreeMap::new());
    let cells: RegionCells = std::cell::RefCell::new(BTreeMap::new());
    let objective = |state: &MeshState, affected: &AffectedSites| {
        region_score(
            state, criteria, gates, limits, &region, &seeds, &cells, affected,
        )
    };

    let mut committed = 0usize;
    for sweep in 0..MAXIMUM_RECOVERY_SWEEPS {
        // Outward-in first: the degree headroom a stalled cell needs is held by
        // the ring outside it, so that ring has to move before the centre has
        // anywhere to go. Then back out, to spread the scales the first pass
        // pulled in.
        let order: Vec<usize> = if sweep % 2 == 0 {
            (0..=movable_rings).rev().collect()
        } else {
            (0..=movable_rings).collect()
        };
        let mut moved_this_sweep = 0usize;
        for ring in order {
            for &site in &rings[ring] {
                if !mesh.can_move_site(site) {
                    continue;
                }
                let Some(before) = objective(mesh.state(), &AffectedSites::new()) else {
                    continue;
                };
                let mut destinations = balance_destinations(mesh, site);
                destinations.extend(degree_relief_destinations(mesh.state(), site));
                let mut moved = false;
                for destination in destinations {
                    if let Acceptance::Committed(_) = mesh.propose_move_cached(
                        site,
                        destination,
                        gates,
                        &objective,
                        Some(&before),
                        true,
                    )? {
                        // The cache describes the state the candidates were
                        // scored against, and that state is gone.
                        cells.borrow_mut().clear();
                        moved = true;
                        moved_this_sweep += 1;
                        break;
                    }
                }
                if moved {
                    continue;
                }
                // Only once one site alone cannot help: two ends of an edge
                // moving together cross a flip boundary neither can cross under
                // the strict-improvement gate.
                for (neighbour, first, second) in degree_relief_pairs(mesh.state(), site) {
                    if frozen.contains(&neighbour)
                        || !region.contains(&neighbour)
                        || !mesh.can_move_site(neighbour)
                    {
                        continue;
                    }
                    if let Acceptance::Committed(_) = mesh.propose_pair_move_cached(
                        (site, first),
                        (neighbour, second),
                        gates,
                        &objective,
                        Some(&before),
                    )? {
                        cells.borrow_mut().clear();
                        moved_this_sweep += 1;
                        break;
                    }
                }
            }
        }
        committed += moved_this_sweep;
        // A sweep that improved nothing will improve nothing next time either:
        // the geometry it read is the geometry it leaves.
        if moved_this_sweep == 0 {
            break;
        }
    }
    Ok(committed)
}

/// How far out a stalled region may move sites, in rings of the triangulation.
///
/// Three, with the fourth held fixed. Two was measured not to reach the degree
/// headroom a saturated neighbourhood needs; the driver escalates to four once,
/// with a fifth guard ring, when three finds nothing.
const RECOVERY_MOVABLE_RINGS: usize = 3;

/// How many passes one stalled region is worth before the run moves on.
///
/// Two round trips: outward-in, out, in, out. A third round trip was measured to
/// commit nothing that the second had not already found.
const MAXIMUM_RECOVERY_SWEEPS: usize = 4;

const PREFERRED_MINIMUM_ANGLE_DEG: f64 = 40.0;
const PREFERRED_MAXIMUM_ANGLE_DEG: f64 = 80.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct AngleWindowSurvey {
    below: usize,
    inside_40_90: usize,
    above_90: usize,
    inside_40_80: usize,
    above_80: usize,
    unmeasurable: usize,
    count: usize,
    min_deg: f64,
    max_deg: f64,
    worst_deviation_deg: f64,
    legacy_penalty: f64,
    penalty: f64,
}

fn record_angle_window(survey: &mut AngleWindowSurvey, angle: f64) {
    if survey.count == 0 {
        survey.min_deg = angle;
        survey.max_deg = angle;
    } else {
        survey.min_deg = survey.min_deg.min(angle);
        survey.max_deg = survey.max_deg.max(angle);
    }
    survey.count += 1;

    if angle < PREFERRED_MINIMUM_ANGLE_DEG {
        survey.below += 1;
        survey.legacy_penalty += (PREFERRED_MINIMUM_ANGLE_DEG - angle).powi(2);
    } else if angle <= 90.0 {
        survey.inside_40_90 += 1;
    } else {
        survey.above_90 += 1;
        survey.legacy_penalty += (angle - 90.0).powi(2);
    }

    if (PREFERRED_MINIMUM_ANGLE_DEG..=PREFERRED_MAXIMUM_ANGLE_DEG).contains(&angle) {
        survey.inside_40_80 += 1;
    } else {
        if angle > PREFERRED_MAXIMUM_ANGLE_DEG {
            survey.above_80 += 1;
        }
        let deviation = if angle < PREFERRED_MINIMUM_ANGLE_DEG {
            PREFERRED_MINIMUM_ANGLE_DEG - angle
        } else {
            angle - PREFERRED_MAXIMUM_ANGLE_DEG
        };
        survey.worst_deviation_deg = survey.worst_deviation_deg.max(deviation);
        survey.penalty += deviation.powi(2);
    }
}

fn angle_window_survey(state: &MeshState) -> AngleWindowSurvey {
    let mut survey = AngleWindowSurvey::default();
    for triangle in state.active_triangle_slots() {
        let corners = state.triangles()[triangle];
        let Some(angles) = crate::criteria::triangle_angles_deg([
            state.vertices()[corners[0]],
            state.vertices()[corners[1]],
            state.vertices()[corners[2]],
        ]) else {
            survey.unmeasurable += 1;
            continue;
        };
        for angle in angles {
            record_angle_window(&mut survey, angle);
        }
    }
    survey
}

fn triangle_fan_ids(state: &MeshState, sites: &AffectedSites) -> Option<BTreeSet<usize>> {
    let mut triangles = BTreeSet::new();
    for (&site, &seed) in sites {
        triangles.extend(state.triangle_fan_from(site, seed).ok()?);
    }
    Some(triangles)
}

fn triangle_eta_value(state: &MeshState, triangle: usize) -> Option<f64> {
    let corners = state.triangles()[triangle];
    crate::criteria::triangle_eta([
        state.vertices()[corners[0]],
        state.vertices()[corners[1]],
        state.vertices()[corners[2]],
    ])
}

fn triangle_window_margin(state: &MeshState, triangle: usize) -> Option<f64> {
    let corners = state.triangles()[triangle];
    let angles = crate::criteria::triangle_angles_deg([
        state.vertices()[corners[0]],
        state.vertices()[corners[1]],
        state.vertices()[corners[2]],
    ])?;
    let min_angle = angles.into_iter().fold(f64::MAX, f64::min);
    let max_angle = angles.into_iter().fold(f64::MIN, f64::max);
    let margin =
        (min_angle - PREFERRED_MINIMUM_ANGLE_DEG).min(PREFERRED_MAXIMUM_ANGLE_DEG - max_angle);
    margin.is_finite().then_some(margin)
}

fn sorted_triangle_values(
    state: &MeshState,
    triangles: impl IntoIterator<Item = usize>,
    value: fn(&MeshState, usize) -> Option<f64>,
) -> Option<Vec<f64>> {
    let mut values = Vec::new();
    for triangle in triangles {
        values.push(value(state, triangle)?);
    }
    values.sort_by(f64::total_cmp);
    Some(values)
}

fn triangle_eta_values(state: &MeshState, sites: &AffectedSites) -> Option<Vec<f64>> {
    sorted_triangle_values(state, triangle_fan_ids(state, sites)?, triangle_eta_value)
}

fn triangle_window_margins(state: &MeshState, sites: &AffectedSites) -> Option<Vec<f64>> {
    sorted_triangle_values(
        state,
        triangle_fan_ids(state, sites)?,
        triangle_window_margin,
    )
}

fn all_triangle_eta_values(state: &MeshState) -> Option<Vec<f64>> {
    sorted_triangle_values(state, state.active_triangle_slots(), triangle_eta_value)
}

fn all_triangle_window_margins(state: &MeshState) -> Option<Vec<f64>> {
    sorted_triangle_values(state, state.active_triangle_slots(), triangle_window_margin)
}

/// Reverse lexicographic order: improving the worst value wins before any
/// change to a better value is considered.
fn worst_first_cmp(left: &[f64], right: &[f64]) -> Option<std::cmp::Ordering> {
    if left.len() != right.len() {
        return None;
    }
    for (&mine, &theirs) in left.iter().zip(right) {
        match mine.total_cmp(&theirs) {
            std::cmp::Ordering::Greater => return Some(std::cmp::Ordering::Less),
            std::cmp::Ordering::Less => return Some(std::cmp::Ordering::Greater),
            std::cmp::Ordering::Equal => {}
        }
    }
    Some(std::cmp::Ordering::Equal)
}

fn window_residue(margins: &[f64]) -> (usize, f64) {
    margins
        .iter()
        .filter(|margin| **margin < 0.0)
        .fold((0, 0.0), |(count, penalty), margin| {
            (count + 1, penalty + margin * margin)
        })
}

#[cfg(test)]
fn worst_first_eta_cmp(left: &[f64], right: &[f64]) -> Option<std::cmp::Ordering> {
    worst_first_cmp(left, right)
}

#[derive(Clone, Debug, PartialEq)]
struct QualityScore {
    unresolved: usize,
    scale_violations: usize,
    worst_scale_ratio: f64,
    window_first: bool,
    window_margin: Vec<f64>,
    triangle_eta: Vec<f64>,
}

impl QualityScore {
    fn hard_no_worse_than(&self, other: &Self) -> bool {
        self.unresolved <= other.unresolved
            && self.scale_violations <= other.scale_violations
            && (self.scale_violations == 0 || self.worst_scale_ratio <= other.worst_scale_ratio)
    }

    fn hard_no_better_than(&self, other: &Self) -> bool {
        other.hard_no_worse_than(self)
    }

    fn hard_equal(&self, other: &Self) -> bool {
        self.hard_no_worse_than(other) && other.hard_no_worse_than(self)
    }
}

impl PartialOrd for QualityScore {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self.window_first != other.window_first {
            return None;
        }
        let no_worse = self.hard_no_worse_than(other);
        let no_better = self.hard_no_better_than(other);
        let eta_cmp = worst_first_cmp(&self.triangle_eta, &other.triangle_eta)?;
        if !self.window_first {
            return match eta_cmp {
                std::cmp::Ordering::Less if no_worse => Some(std::cmp::Ordering::Less),
                std::cmp::Ordering::Greater if no_better => Some(std::cmp::Ordering::Greater),
                std::cmp::Ordering::Equal if self.hard_equal(other) => {
                    Some(std::cmp::Ordering::Equal)
                }
                _ => None,
            };
        }
        let eta_no_worse = eta_cmp != std::cmp::Ordering::Greater;
        let eta_no_better = eta_cmp != std::cmp::Ordering::Less;
        let mine = window_residue(&self.window_margin);
        let theirs = window_residue(&other.window_margin);
        let residue_no_worse = mine.0 <= theirs.0 && mine.1 <= theirs.1;
        let residue_no_better = mine.0 >= theirs.0 && mine.1 >= theirs.1;
        match worst_first_cmp(&self.window_margin, &other.window_margin)? {
            std::cmp::Ordering::Less if no_worse && eta_no_worse && residue_no_worse => {
                Some(std::cmp::Ordering::Less)
            }
            std::cmp::Ordering::Greater if no_better && eta_no_better && residue_no_better => {
                Some(std::cmp::Ordering::Greater)
            }
            std::cmp::Ordering::Equal => match eta_cmp {
                std::cmp::Ordering::Less if no_worse => Some(std::cmp::Ordering::Less),
                std::cmp::Ordering::Greater if no_better => Some(std::cmp::Ordering::Greater),
                std::cmp::Ordering::Equal if self.hard_equal(other) => {
                    Some(std::cmp::Ordering::Equal)
                }
                _ => None,
            },
            _ => None,
        }
    }
}

fn target_cell_scales(
    state: &MeshState,
    criteria: &[Box<dyn CellCriterion>],
    minimum_scale_m: f64,
    background_scale_m: f64,
) -> Option<Vec<f64>> {
    if background_scale_m <= 0.0 || !background_scale_m.is_finite() {
        return None;
    }

    let radius_m = state.sphere_radius();
    let mut queue = VecDeque::new();
    let mut queued = vec![false; state.vertices().len()];
    let mut target = vec![background_scale_m; state.vertices().len()];
    for site in state.active_vertex_slots() {
        let point = xyz_to_lonlat_degrees(state.vertices()[site]);
        if let Some(value) = criteria
            .iter()
            .filter_map(|criterion| criterion.target_scale_m_at(point, radius_m))
            .filter(|value| value.is_finite() && *value > 0.0)
            .min_by(f64::total_cmp)
        {
            target[site] = target[site].min(value.max(minimum_scale_m));
            queue.push_back(site);
            queued[site] = true;
        }
    }
    if queue.is_empty() {
        return None;
    }

    let mut edges = BTreeSet::new();
    for triangle in state.active_triangle_slots() {
        let [a, b, c] = state.triangles()[triangle];
        edges.extend([
            (a.min(b), a.max(b)),
            (b.min(c), b.max(c)),
            (c.min(a), c.max(a)),
        ]);
    }
    let mut adjacency = vec![Vec::new(); state.vertices().len()];
    for (left, right) in edges {
        let allowance = TARGET_SCALE_GRADIENT
            * arc_length_unit_sphere(state.vertices()[left], state.vertices()[right]);
        adjacency[left].push((right, allowance));
        adjacency[right].push((left, allowance));
    }
    while let Some(site) = queue.pop_front() {
        queued[site] = false;
        for &(neighbour, allowance) in &adjacency[site] {
            let candidate = target[site] + allowance;
            if candidate < target[neighbour] {
                target[neighbour] = candidate;
                if !queued[neighbour] {
                    queued[neighbour] = true;
                    queue.push_back(neighbour);
                }
            }
        }
    }
    Some(target)
}

fn target_angle_from_lengths(left: f64, right: f64, opposite: f64, radius_m: f64) -> Option<f64> {
    if !radius_m.is_finite()
        || radius_m <= 0.0
        || [left, right, opposite]
            .into_iter()
            .any(|length| !length.is_finite() || length <= 0.0)
        || left + right <= opposite
    {
        return None;
    }
    let (left, right, opposite) = (left / radius_m, right / radius_m, opposite / radius_m);
    let denominator = left.sin() * right.sin();
    if !denominator.is_finite() || denominator.abs() <= f64::EPSILON {
        return None;
    }
    let cosine = ((opposite.cos() - left.cos() * right.cos()) / denominator).clamp(-1.0, 1.0);
    let angle = cosine.acos().to_degrees();
    angle.is_finite().then_some(angle)
}

pub(crate) fn target_angle_window_survey(
    state: &MeshState,
    target_cell_scale: &[f64],
) -> AngleWindowSurvey {
    let mut survey = AngleWindowSurvey::default();
    for triangle in state.active_triangle_slots() {
        let [a, b, c] = state.triangles()[triangle];
        if [a, b, c]
            .into_iter()
            .any(|site| site >= target_cell_scale.len())
        {
            survey.unmeasurable += 1;
            continue;
        }
        let ab = CELL_SCALE_TO_EDGE_LENGTH * 0.5 * (target_cell_scale[a] + target_cell_scale[b]);
        let bc = CELL_SCALE_TO_EDGE_LENGTH * 0.5 * (target_cell_scale[b] + target_cell_scale[c]);
        let ca = CELL_SCALE_TO_EDGE_LENGTH * 0.5 * (target_cell_scale[c] + target_cell_scale[a]);
        let radius_m = state.sphere_radius();
        let Some(angles) = target_angle_from_lengths(ab, ca, bc, radius_m)
            .zip(target_angle_from_lengths(ab, bc, ca, radius_m))
            .zip(target_angle_from_lengths(bc, ca, ab, radius_m))
            .map(|((at_a, at_b), at_c)| [at_a, at_b, at_c])
        else {
            survey.unmeasurable += 1;
            continue;
        };
        for angle in angles {
            record_angle_window(&mut survey, angle);
        }
    }
    survey
}

fn vertices_below_degree_5(state: &MeshState) -> usize {
    vertices_below_degree_5_set(state).len()
}

/// Which sites are below degree five, from one pass over the triangles.
///
/// The retirement guards used to ask `vertex_degree` per site, which scans for
/// a fan seed and is linear in the mesh each time. `vertex_degrees` counts
/// incidences instead. On a closed triangulation the two agree; they are held
/// to that by `full_cell_sweeps_match_the_scanned_reference`, because a broken
/// fan makes `vertex_degree` return an error while the incidence count still
/// reports a degree.
fn vertices_below_degree_5_set(state: &MeshState) -> BTreeSet<usize> {
    let degrees = vertex_degrees(state);
    state
        .active_vertex_slots()
        .filter(|&site| degrees[site] < 5)
        .collect()
}

#[derive(Clone, Debug, Default, PartialEq)]
struct LeafLineageSurvey {
    active_adaptive_sites: usize,
    active_leaf_sites: usize,
    interior_leaf_sites: usize,
    lineage_unknown_adaptive_sites: usize,
    leaf_degree_4: usize,
    leaf_degree_5: usize,
    leaf_degree_6: usize,
    leaf_degree_7: usize,
    leaf_degree_other: usize,
    leaf_birth_cycle_min: u32,
    leaf_birth_cycle_max: u32,
    leaf_target_scale_measured: usize,
    leaf_target_scale_min_m: f64,
    leaf_target_scale_max_m: f64,
    angles_below_40_at_leaf_vertices: usize,
    angles_above_80_at_leaf_vertices: usize,
    angles_below_40_at_interior_leaf_vertices: usize,
    angles_above_80_at_interior_leaf_vertices: usize,
    violating_triangles_touching_leaf: usize,
    violating_triangles_touching_interior_leaf: usize,
    interior_leaf: Vec<bool>,
}

fn leaf_lineage_survey(
    mesh: &AdaptiveMesh,
    criteria: &[Box<dyn CellCriterion>],
) -> LeafLineageSurvey {
    let state = mesh.state();
    let sites = mesh.sites();
    let mut survey = LeafLineageSurvey::default();
    let active_ids: BTreeSet<SiteId> = sites
        .iter()
        .filter(|site| site.active)
        .map(|site| site.site_id)
        .collect();
    let active_parents: BTreeSet<SiteId> = sites
        .iter()
        .filter(|site| site.active)
        .filter_map(|site| site.parent_site_id)
        .filter(|parent| active_ids.contains(parent))
        .collect();
    let protected_sites: BTreeSet<usize> = mesh
        .segments
        .iter()
        .flat_map(|(tail, head)| [tail, head])
        .collect();
    let mut leaf = vec![false; state.vertices().len()];
    let mut interior_leaf = vec![false; state.vertices().len()];
    let radius_m = state.sphere_radius();

    for site in sites {
        if !site.active || site.birth_cycle == 0 {
            continue;
        }
        survey.active_adaptive_sites += 1;
        if site.parent_site_id.is_none() {
            survey.lineage_unknown_adaptive_sites += 1;
            continue;
        }
        if active_parents.contains(&site.site_id) {
            continue;
        }
        let Some(vertex) = mesh.vertex_for_site_id(site.site_id) else {
            continue;
        };
        leaf[vertex] = true;
        survey.active_leaf_sites += 1;
        if survey.active_leaf_sites == 1 {
            survey.leaf_birth_cycle_min = site.birth_cycle;
            survey.leaf_birth_cycle_max = site.birth_cycle;
        } else {
            survey.leaf_birth_cycle_min = survey.leaf_birth_cycle_min.min(site.birth_cycle);
            survey.leaf_birth_cycle_max = survey.leaf_birth_cycle_max.max(site.birth_cycle);
        }
        match state.vertex_degree(vertex) {
            Ok(4) => survey.leaf_degree_4 += 1,
            Ok(5) => survey.leaf_degree_5 += 1,
            Ok(6) => survey.leaf_degree_6 += 1,
            Ok(7) => survey.leaf_degree_7 += 1,
            _ => survey.leaf_degree_other += 1,
        }
        if site.mobility == SiteMobility::Interior && !protected_sites.contains(&vertex) {
            interior_leaf[vertex] = true;
            survey.interior_leaf_sites += 1;
        }
        let target = criteria
            .iter()
            .filter_map(|criterion| criterion.target_scale_m_at(site.position, radius_m))
            .filter(|scale| scale.is_finite() && *scale > 0.0)
            .min_by(f64::total_cmp);
        if let Some(target) = target {
            if survey.leaf_target_scale_measured == 0 {
                survey.leaf_target_scale_min_m = target;
                survey.leaf_target_scale_max_m = target;
            } else {
                survey.leaf_target_scale_min_m = survey.leaf_target_scale_min_m.min(target);
                survey.leaf_target_scale_max_m = survey.leaf_target_scale_max_m.max(target);
            }
            survey.leaf_target_scale_measured += 1;
        }
    }

    for triangle in state.active_triangle_slots() {
        let corners = state.triangles()[triangle];
        let Some(angles) = crate::criteria::triangle_angles_deg([
            state.vertices()[corners[0]],
            state.vertices()[corners[1]],
            state.vertices()[corners[2]],
        ]) else {
            continue;
        };
        let violates = angles.iter().any(|angle| {
            *angle < PREFERRED_MINIMUM_ANGLE_DEG || *angle > PREFERRED_MAXIMUM_ANGLE_DEG
        });
        if violates && corners.iter().any(|corner| leaf[*corner]) {
            survey.violating_triangles_touching_leaf += 1;
        }
        if violates && corners.iter().any(|corner| interior_leaf[*corner]) {
            survey.violating_triangles_touching_interior_leaf += 1;
        }
        for (corner, angle) in corners.into_iter().zip(angles) {
            if angle < PREFERRED_MINIMUM_ANGLE_DEG && leaf[corner] {
                survey.angles_below_40_at_leaf_vertices += 1;
                if interior_leaf[corner] {
                    survey.angles_below_40_at_interior_leaf_vertices += 1;
                }
            } else if angle > PREFERRED_MAXIMUM_ANGLE_DEG && leaf[corner] {
                survey.angles_above_80_at_leaf_vertices += 1;
                if interior_leaf[corner] {
                    survey.angles_above_80_at_interior_leaf_vertices += 1;
                }
            }
        }
    }
    survey.interior_leaf = interior_leaf;
    survey
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct DegreeFourRetirementAudit {
    candidates: usize,
    triangulations: usize,
    hard_gate_safe: usize,
    physical_safe: usize,
    balance_safe: usize,
    quality_improving: usize,
    fully_acceptable: usize,
    committed: usize,
}

struct RetirementPostcondition<'a> {
    criteria: &'a [Box<dyn CellCriterion>],
    gates: HardGates,
    pentagons: [usize; 12],
    limits: CycleLimits,
    before_demands: Option<usize>,
    before_balance: (usize, f64),
    before_angles: AngleWindowSurvey,
    before_vertices_below_degree_5: BTreeSet<usize>,
    before_eta: Option<f64>,
    before_margin: Option<f64>,
}

fn retirement_touched_triangles(report: &earthmesh_mesh::RetirementReport) -> BTreeSet<usize> {
    let mut touched = BTreeSet::new();
    touched.extend(report.reused_faces.iter().copied());
    touched.extend(report.retired_faces.iter().copied());
    if touched.is_empty() {
        touched.extend(report.fan.iter().copied());
    }
    touched
}

impl RetirementPostcondition<'_> {
    fn accepts(&self, state: &MeshState, report: &earthmesh_mesh::RetirementReport) -> bool {
        let touched = retirement_touched_triangles(report);
        if check(state, &touched, self.gates, &self.pentagons).is_err() {
            return false;
        }
        let (after_demands, after_scales) = demanded_cells_and_scales(state, self.criteria);
        if !self
            .before_demands
            .zip(after_demands)
            .is_some_and(|(before, after)| after <= before)
        {
            return false;
        }
        let after_balance = balance_survey_from_scales(state, &after_scales, self.limits);
        if after_balance.0 > self.before_balance.0
            || (after_balance.0 > 0 && after_balance.1 > self.before_balance.1)
        {
            return false;
        }
        let after_angles = angle_window_survey(state);
        let after_vertices_below_degree_5 = vertices_below_degree_5_set(state);
        let after_eta = all_triangle_eta_values(state).and_then(|values| values.first().copied());
        let after_margin =
            all_triangle_window_margins(state).and_then(|values| values.first().copied());
        after_angles.unmeasurable == 0
            && after_angles.below <= self.before_angles.below
            && after_angles.above_80 <= self.before_angles.above_80
            && after_angles.below + after_angles.above_80
                < self.before_angles.below + self.before_angles.above_80
            && after_angles.worst_deviation_deg <= self.before_angles.worst_deviation_deg
            && after_angles.penalty < self.before_angles.penalty
            && after_vertices_below_degree_5.is_subset(&self.before_vertices_below_degree_5)
            && self
                .before_eta
                .zip(after_eta)
                .is_some_and(|(before, after)| after >= before)
            && self
                .before_margin
                .zip(after_margin)
                .is_some_and(|(before, after)| after >= before)
    }
}

/// Which interior leaves are worth trying to retire, worst window margin first.
///
/// Separate from the retirement loop so the ordering can be held to a reference
/// implementation: the loop truncates this list, so a change that reorders it
/// silently retires a different set of sites while the commit count and the
/// runtime both look ordinary.
fn retirement_candidates(
    state: &MeshState,
    leaves: &LeafLineageSurvey,
    maximum_degree: usize,
) -> Vec<usize> {
    // Degrees first, from one pass over the triangles: asking `vertex_degree`
    // per site scans for a fan seed each time, and the old order evaluated it
    // before the cheap leaf lookup could rule the site out.
    let degrees = vertex_degrees(state);
    let seeds = active_site_triangle_seeds(state);
    let mut scored: Vec<(usize, f64)> = state
        .active_vertex_slots()
        .filter(|&site| {
            leaves.interior_leaf.get(site).copied().unwrap_or(false)
                && (4..=maximum_degree).contains(&degrees[site])
        })
        .filter_map(|site| {
            // The fan and its margins, once per surviving candidate: the old
            // comparator rebuilt both on every comparison.
            let fan = seeds[site].and_then(|seed| state.triangle_fan_from(site, seed).ok())?;
            let mut worst = f64::INFINITY;
            let mut breaches_the_window = false;
            for triangle in fan {
                let Some(margin) = triangle_window_margin(state, triangle) else {
                    continue;
                };
                worst = worst.min(margin);
                breaches_the_window |= margin < 0.0;
            }
            breaches_the_window.then_some((site, worst))
        })
        .collect();
    // `f64` is not `Ord`, so neither `sort_by_key` nor `sort_by_cached_key`
    // applies, and `to_bits` orders negatives backwards -- which is exactly the
    // half of the range that matters here.
    scored.sort_by(|left, right| {
        left.1
            .total_cmp(&right.1)
            .then_with(|| left.0.cmp(&right.0))
    });
    scored.into_iter().map(|(site, _)| site).collect()
}

fn retire_quality_leaf_sites(
    mesh: &mut AdaptiveMesh,
    criteria: &[Box<dyn CellCriterion>],
    gates: HardGates,
    limits: CycleLimits,
    leaves: &LeafLineageSurvey,
    maximum_degree: usize,
) -> (usize, usize) {
    if !mesh.segments_are_empty() {
        return (0, 0);
    }
    let started = std::time::Instant::now();
    let mut candidates = retirement_candidates(mesh.state(), leaves, maximum_degree);
    let found = candidates.len();
    // ponytail: each candidate still runs global postcondition scans; cap the
    // pass until those checks use local deltas.
    candidates.truncate(64);
    let read_baseline = |state: &MeshState| {
        let (demanded, scales) = demanded_cells_and_scales(state, criteria);
        (
            demanded,
            balance_survey_from_scales(state, &scales, limits),
            angle_window_survey(state),
            vertices_below_degree_5_set(state),
            all_triangle_eta_values(state).and_then(|values| values.first().copied()),
            all_triangle_window_margins(state).and_then(|values| values.first().copied()),
        )
    };
    let (
        mut before_demands,
        mut before_balance,
        mut before_angles,
        mut before_vertices_below_degree_5,
        mut before_eta,
        mut before_margin,
    ) = read_baseline(mesh.state());
    let mut committed = 0;
    let mut committed_d4 = 0;
    let attempted = candidates.len();
    for site in candidates {
        let degree = mesh.state().vertex_degree(site).unwrap_or(0);
        let postcondition = RetirementPostcondition {
            criteria,
            gates,
            pentagons: mesh.pentagon_ids(),
            limits,
            before_demands,
            before_balance,
            before_angles,
            before_vertices_below_degree_5: before_vertices_below_degree_5.clone(),
            before_eta,
            before_margin,
        };
        if mesh
            .retire_leaf_transactionally(site, |state, report| postcondition.accepts(state, report))
            .is_ok()
        {
            committed += 1;
            committed_d4 += usize::from(degree == 4);
            (
                before_demands,
                before_balance,
                before_angles,
                before_vertices_below_degree_5,
                before_eta,
                before_margin,
            ) = read_baseline(mesh.state());
        }
    }
    // This phase used to print nothing at all, which on a large mesh reads as a
    // hung run: it comes after the last cycle line and can outlast every other
    // phase put together.
    eprintln!(
        "harp_dv leaf retirement: {attempted} of {found} candidate(s) tried, {committed} retired \
         ({committed_d4} degree-four), {:.1}s",
        started.elapsed().as_secs_f64()
    );
    (committed, committed_d4)
}

/// Rebuild compact clones without one degree-four leaf.
///
/// This deliberately does not mutate `AdaptiveMesh`: compacting rows loses the
/// stable SiteId-to-row relation, so the result is evidence for (or against) a
/// future deletion transaction, not a deletion transaction itself.
fn clone_without_degree_four_site(state: &MeshState, site: usize) -> Vec<(MeshState, Vec<usize>)> {
    let Ok(fan) = state.triangle_fan(site) else {
        return Vec::new();
    };
    if fan.len() != 4 {
        return Vec::new();
    }
    let mut ring = Vec::with_capacity(4);
    for step in 0..4 {
        let here = state.triangles()[fan[step]];
        let next = state.triangles()[fan[(step + 1) % 4]];
        let shared: Vec<_> = here
            .into_iter()
            .filter(|corner| *corner != site && next.contains(corner))
            .collect();
        if shared.len() != 1 {
            return Vec::new();
        }
        ring.push(shared[0]);
    }
    if ring.iter().copied().collect::<BTreeSet<_>>().len() != 4 {
        return Vec::new();
    }

    let corners = state.triangles()[fan[0]];
    let Ok(expected) = earthmesh_mesh::orientation_on_sphere(
        state.vertices()[corners[0]],
        state.vertices()[corners[1]],
        state.vertices()[corners[2]],
    ) else {
        return Vec::new();
    };
    if expected == Sign::Zero {
        return Vec::new();
    }
    let alternatives = [
        [[ring[0], ring[1], ring[2]], [ring[0], ring[2], ring[3]]],
        [[ring[0], ring[1], ring[3]], [ring[1], ring[2], ring[3]]],
    ];
    let fan: BTreeSet<_> = fan.into_iter().collect();
    let mut rebuilt = Vec::new();
    for mut replacement in alternatives {
        let mut valid = true;
        for triangle in &mut replacement {
            match earthmesh_mesh::orientation_on_sphere(
                state.vertices()[triangle[0]],
                state.vertices()[triangle[1]],
                state.vertices()[triangle[2]],
            ) {
                Ok(sign) if sign == expected => {}
                Ok(Sign::Positive | Sign::Negative) => triangle.swap(1, 2),
                _ => valid = false,
            }
        }
        if !valid {
            continue;
        }

        let mut remap = vec![usize::MAX; state.vertices().len()];
        let mut vertices = state
            .vertices()
            .iter()
            .take(MESH_STATE_FIRST_ID)
            .copied()
            .collect::<Vec<_>>();
        for old in state.active_vertex_slots() {
            if old == site {
                continue;
            }
            remap[old] = vertices.len();
            vertices.push(state.vertices()[old]);
        }
        let mut triangles = state
            .triangles()
            .iter()
            .take(MESH_STATE_FIRST_ID)
            .copied()
            .collect::<Vec<_>>();
        triangles.extend(
            state
                .triangles()
                .iter()
                .enumerate()
                .filter(|(triangle, _)| {
                    state.is_triangle_live(*triangle) && !fan.contains(triangle)
                })
                .map(|(_, corners)| corners.map(|corner| remap[corner])),
        );
        let replacement_start = triangles.len();
        triangles.extend(replacement.map(|corners| corners.map(|corner| remap[corner])));
        let Ok(mut candidate) = MeshState::from_parts(vertices, triangles) else {
            continue;
        };
        let seeds: BTreeSet<_> = (replacement_start..candidate.triangles().len()).collect();
        if candidate.legalize_around(&seeds).is_err()
            || candidate.validate().is_err()
            || candidate.open_edge_count() != 0
            || candidate.active_triangle_slots().any(|triangle| {
                (0..3).any(|corner| candidate.edge_is_illegal(triangle, corner).unwrap_or(true))
            })
        {
            continue;
        }
        rebuilt.push((candidate, remap));
    }
    rebuilt
}

/// One cell sweep answering both retirement guards.
///
/// `accepts` used to count demanded cells and then survey balance, each
/// building every Voronoi cell again. The count keeps its all-or-nothing
/// contract -- `None` if any cell or criterion could not be read -- while the
/// scales keep `state_scales`'s: unreadable cells are simply absent.
fn demanded_cells_and_scales(
    state: &MeshState,
    criteria: &[Box<dyn CellCriterion>],
) -> (Option<usize>, Vec<Option<f64>>) {
    let radius_m = state.sphere_radius();
    let seeds = active_site_triangle_seeds(state);
    let mut scales = vec![None; state.vertices().len()];
    let mut demanded = Some(0usize);
    for site in state.active_vertex_slots() {
        let cell = seeds[site].and_then(|seed| state.voronoi_cell_from(site, seed).ok());
        let Some(cell) = cell else {
            demanded = None;
            continue;
        };
        let view = CellView {
            site,
            cell: &cell,
            state,
            radius_m,
        };
        scales[site] = view.effective_scale_m();
        let demands = criteria
            .iter()
            .filter(|criterion| criterion.semantics() != CriterionSemantics::MeshQuality)
            .try_fold(false, |demands, criterion| {
                criterion
                    .evaluate(&view)
                    .map(|evidence| demands || evidence.demands_work())
            });
        match demands {
            Ok(true) => demanded = demanded.map(|count| count + 1),
            Ok(false) => {}
            Err(_) => demanded = None,
        }
    }
    (demanded, scales)
}

fn demanded_cells_in_state(
    state: &MeshState,
    criteria: &[Box<dyn CellCriterion>],
) -> Option<usize> {
    let radius_m = state.sphere_radius();
    let seeds = active_site_triangle_seeds(state);
    let mut demanded = 0;
    for site in state.active_vertex_slots() {
        let cell = state.voronoi_cell_from(site, seeds[site]?).ok()?;
        let view = CellView {
            site,
            cell: &cell,
            state,
            radius_m,
        };
        if criteria
            .iter()
            .filter(|criterion| criterion.semantics() != CriterionSemantics::MeshQuality)
            .try_fold(false, |demands, criterion| {
                criterion
                    .evaluate(&view)
                    .map(|evidence| demands || evidence.demands_work())
            })
            .ok()?
        {
            demanded += 1;
        }
    }
    Some(demanded)
}

#[allow(clippy::too_many_arguments)]
fn degree_four_candidate_is_acceptable(
    mesh: &AdaptiveMesh,
    candidate: &MeshState,
    remap: &[usize],
    before_demands: Option<usize>,
    before_balance: (usize, f64),
    before_angles: &AngleWindowSurvey,
    before_eta: Option<f64>,
    before_margin: Option<f64>,
    criteria: &[Box<dyn CellCriterion>],
    gates: HardGates,
    limits: CycleLimits,
    audit: &mut DegreeFourRetirementAudit,
) -> bool {
    audit.triangulations += 1;
    let touched: BTreeSet<_> = candidate.active_triangle_slots().collect();
    let Some(pentagons) = mesh
        .pentagon_ids()
        .map(|pentagon| remap.get(pentagon).copied())
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .and_then(|vertices| vertices.try_into().ok())
    else {
        return false;
    };
    if check(candidate, &touched, gates, &pentagons).is_err() {
        return false;
    }
    audit.hard_gate_safe += 1;
    let after_demands = demanded_cells_in_state(candidate, criteria);
    let physical_safe = before_demands
        .zip(after_demands)
        .is_some_and(|(before, after)| after <= before);
    if physical_safe {
        audit.physical_safe += 1;
    }
    let after_balance = balance_survey_state(candidate, limits);
    let balance_safe = after_balance.0 <= before_balance.0
        && (after_balance.0 == 0 || after_balance.1 <= before_balance.1);
    if balance_safe {
        audit.balance_safe += 1;
    }
    let after_angles = angle_window_survey(candidate);
    let after_eta = all_triangle_eta_values(candidate).and_then(|values| values.first().copied());
    let after_global_margin =
        all_triangle_window_margins(candidate).and_then(|values| values.first().copied());
    let quality_improving = after_angles.unmeasurable == 0
        && after_angles.below <= before_angles.below
        && after_angles.above_80 <= before_angles.above_80
        && after_angles.below + after_angles.above_80
            < before_angles.below + before_angles.above_80
        && after_angles.worst_deviation_deg <= before_angles.worst_deviation_deg
        && after_angles.penalty < before_angles.penalty
        && before_eta
            .zip(after_eta)
            .is_some_and(|(before, after)| after >= before)
        && before_margin
            .zip(after_global_margin)
            .is_some_and(|(before, after)| after >= before);
    if quality_improving {
        audit.quality_improving += 1;
    }
    physical_safe && balance_safe && quality_improving
}

fn degree_four_retirement_audit(
    mesh: &AdaptiveMesh,
    criteria: &[Box<dyn CellCriterion>],
    gates: HardGates,
    limits: CycleLimits,
    leaves: &LeafLineageSurvey,
) -> DegreeFourRetirementAudit {
    let state = mesh.state();
    let before_demands = demanded_cells_in_state(state, criteria);
    let before_balance = balance_survey_state(state, limits);
    let before_angles = angle_window_survey(state);
    let before_eta = all_triangle_eta_values(state).and_then(|values| values.first().copied());
    let before_margin =
        all_triangle_window_margins(state).and_then(|values| values.first().copied());
    let mut audit = DegreeFourRetirementAudit::default();
    for site in state.active_vertex_slots() {
        if !leaves.interior_leaf.get(site).copied().unwrap_or(false)
            || state.vertex_degree(site).ok() != Some(4)
        {
            continue;
        }
        audit.candidates += 1;
        let mut site_acceptable = false;
        for (candidate, remap) in clone_without_degree_four_site(state, site) {
            site_acceptable |= degree_four_candidate_is_acceptable(
                mesh,
                &candidate,
                &remap,
                before_demands,
                before_balance,
                &before_angles,
                before_eta,
                before_margin,
                criteria,
                gates,
                limits,
                &mut audit,
            );
        }
        audit.fully_acceptable += usize::from(site_acceptable);
    }
    audit
}

fn vertex_degrees(state: &MeshState) -> Vec<usize> {
    let mut degrees = vec![0; state.vertices().len()];
    for triangle in state
        .active_triangle_slots()
        .map(|triangle| &state.triangles()[triangle])
    {
        for &site in triangle {
            degrees[site] += 1;
        }
    }
    degrees
}

fn median_cell_scale(state: &MeshState) -> Option<f64> {
    let cell_scales = state_scales(state);
    let mut scales = state
        .active_vertex_slots()
        .map(|site| cell_scales[site])
        .collect::<Option<Vec<_>>>()?;
    scales.sort_by(f64::total_cmp);
    scales.get(scales.len() / 2).copied()
}

fn natural_length_destination(
    state: &MeshState,
    target_cell_scale: &[f64],
    site: usize,
) -> Option<CartesianPoint> {
    let here = state.vertices()[site];
    let mut weighted = CartesianPoint::new(0.0, 0.0, 0.0);
    let mut total_weight = 0.0;
    for neighbour in neighbour_sites(state, site) {
        let there = state.vertices()[neighbour];
        let length = arc_length_unit_sphere(here, there);
        if !length.is_finite() || length <= 0.0 {
            return None;
        }
        let desired = CELL_SCALE_TO_EDGE_LENGTH
            * 0.5
            * (target_cell_scale[site] + target_cell_scale[neighbour]);
        let delta = (desired - length) / length;
        if !delta.is_finite() || delta.abs() < 1.0e-12 {
            continue;
        }
        let weight = delta * delta;
        weighted.x += weight * (here.x + delta * (here.x - there.x));
        weighted.y += weight * (here.y + delta * (here.y - there.y));
        weighted.z += weight * (here.z + delta * (here.z - there.z));
        total_weight += weight;
    }
    (total_weight > 0.0).then_some(CartesianPoint::new(
        weighted.x / total_weight,
        weighted.y / total_weight,
        weighted.z / total_weight,
    ))
}

fn worst_incident_triangle(state: &MeshState, site: usize) -> Option<(usize, f64)> {
    state
        .triangle_fan(site)
        .ok()?
        .into_iter()
        .filter_map(|triangle| {
            let corners = state.triangles()[triangle];
            let eta = crate::criteria::triangle_eta([
                state.vertices()[corners[0]],
                state.vertices()[corners[1]],
                state.vertices()[corners[2]],
            ])?;
            Some((triangle, eta))
        })
        .min_by(|left, right| left.1.total_cmp(&right.1))
}

fn triangle_ascent_destination(
    state: &MeshState,
    triangle: usize,
    site: usize,
) -> Option<CartesianPoint> {
    let corners = state.triangles()[triangle];
    corners.contains(&site).then_some(())?;
    let here = state.vertices()[site];
    let radius = magnitude(here);
    let neighbours = neighbour_sites(state, site);
    let mean_edge = neighbours
        .iter()
        .map(|&neighbour| arc_length_unit_sphere(here, state.vertices()[neighbour]))
        .sum::<f64>()
        / neighbours.len() as f64;
    if !radius.is_finite() || radius <= 0.0 || !mean_edge.is_finite() || mean_edge <= 0.0 {
        return None;
    }

    let normal = CartesianPoint::new(here.x / radius, here.y / radius, here.z / radius);
    let axis = if normal.x.abs() <= normal.y.abs() && normal.x.abs() <= normal.z.abs() {
        CartesianPoint::new(1.0, 0.0, 0.0)
    } else if normal.y.abs() <= normal.z.abs() {
        CartesianPoint::new(0.0, 1.0, 0.0)
    } else {
        CartesianPoint::new(0.0, 0.0, 1.0)
    };
    let first = cross(normal, axis);
    let first_length = magnitude(first);
    if first_length <= 0.0 || !first_length.is_finite() {
        return None;
    }
    let first = CartesianPoint::new(
        first.x / first_length,
        first.y / first_length,
        first.z / first_length,
    );
    let second = cross(normal, first);
    let probe = mean_edge * QUALITY_GRADIENT_PROBE_FRACTION;
    let eta_at = |position: CartesianPoint| {
        let mut points = [
            state.vertices()[corners[0]],
            state.vertices()[corners[1]],
            state.vertices()[corners[2]],
        ];
        points[corners.iter().position(|&corner| corner == site)?] = position;
        crate::criteria::triangle_eta(points)
    };
    let component = |direction: CartesianPoint| {
        let plus = projected_step(
            here,
            CartesianPoint::new(
                here.x + direction.x * probe,
                here.y + direction.y * probe,
                here.z + direction.z * probe,
            ),
            1.0,
        )?;
        let minus = projected_step(
            here,
            CartesianPoint::new(
                here.x - direction.x * probe,
                here.y - direction.y * probe,
                here.z - direction.z * probe,
            ),
            1.0,
        )?;
        Some((eta_at(plus)? - eta_at(minus)?) / (2.0 * probe))
    };
    let gradient = (component(first)?, component(second)?);
    let gradient_length = gradient.0.hypot(gradient.1);
    if !gradient_length.is_finite() || gradient_length <= f64::EPSILON {
        return None;
    }
    let direction = CartesianPoint::new(
        (first.x * gradient.0 + second.x * gradient.1) / gradient_length,
        (first.y * gradient.0 + second.y * gradient.1) / gradient_length,
        (first.z * gradient.0 + second.z * gradient.1) / gradient_length,
    );
    projected_step(
        here,
        CartesianPoint::new(
            here.x + direction.x * mean_edge * QUALITY_ASCENT_EDGE_FRACTION,
            here.y + direction.y * mean_edge * QUALITY_ASCENT_EDGE_FRACTION,
            here.z + direction.z * mean_edge * QUALITY_ASCENT_EDGE_FRACTION,
        ),
        1.0,
    )
}

fn worst_triangle_ascent_destination(state: &MeshState, site: usize) -> Option<CartesianPoint> {
    triangle_ascent_destination(state, worst_incident_triangle(state, site)?.0, site)
}

fn star_window_ascent_destination(state: &MeshState, site: usize) -> Option<CartesianPoint> {
    let here = state.vertices()[site];
    let radius = magnitude(here);
    let neighbours = neighbour_sites(state, site);
    let mean_edge = neighbours
        .iter()
        .map(|&neighbour| arc_length_unit_sphere(here, state.vertices()[neighbour]))
        .sum::<f64>()
        / neighbours.len() as f64;
    if !radius.is_finite() || radius <= 0.0 || !mean_edge.is_finite() || mean_edge <= 0.0 {
        return None;
    }
    let normal = CartesianPoint::new(here.x / radius, here.y / radius, here.z / radius);
    let axis = if normal.x.abs() <= normal.y.abs() && normal.x.abs() <= normal.z.abs() {
        CartesianPoint::new(1.0, 0.0, 0.0)
    } else if normal.y.abs() <= normal.z.abs() {
        CartesianPoint::new(0.0, 1.0, 0.0)
    } else {
        CartesianPoint::new(0.0, 0.0, 1.0)
    };
    let first = cross(normal, axis);
    let first_length = magnitude(first);
    if !first_length.is_finite() || first_length <= 0.0 {
        return None;
    }
    let first = CartesianPoint::new(
        first.x / first_length,
        first.y / first_length,
        first.z / first_length,
    );
    let second = cross(normal, first);
    let fan = state.triangle_fan(site).ok()?;
    let penalty_at = |position: CartesianPoint| {
        fan.iter().try_fold(0.0, |penalty, &triangle| {
            let corners = state.triangles()[triangle];
            let mut points = [
                state.vertices()[corners[0]],
                state.vertices()[corners[1]],
                state.vertices()[corners[2]],
            ];
            points[corners.iter().position(|&corner| corner == site)?] = position;
            let angles = crate::criteria::triangle_angles_deg(points)?;
            Some(
                penalty
                    + angles
                        .into_iter()
                        .map(|angle| {
                            if angle < PREFERRED_MINIMUM_ANGLE_DEG {
                                (PREFERRED_MINIMUM_ANGLE_DEG - angle).powi(2)
                            } else if angle > PREFERRED_MAXIMUM_ANGLE_DEG {
                                (angle - PREFERRED_MAXIMUM_ANGLE_DEG).powi(2)
                            } else {
                                0.0
                            }
                        })
                        .sum::<f64>(),
            )
        })
    };
    let probe = mean_edge * QUALITY_GRADIENT_PROBE_FRACTION;
    let component = |direction: CartesianPoint| {
        let plus = projected_step(
            here,
            CartesianPoint::new(
                here.x + direction.x * probe,
                here.y + direction.y * probe,
                here.z + direction.z * probe,
            ),
            1.0,
        )?;
        let minus = projected_step(
            here,
            CartesianPoint::new(
                here.x - direction.x * probe,
                here.y - direction.y * probe,
                here.z - direction.z * probe,
            ),
            1.0,
        )?;
        Some((penalty_at(minus)? - penalty_at(plus)?) / (2.0 * probe))
    };
    let gradient = (component(first)?, component(second)?);
    let length = gradient.0.hypot(gradient.1);
    if !length.is_finite() || length <= f64::EPSILON {
        return None;
    }
    projected_step(
        here,
        CartesianPoint::new(
            here.x + (first.x * gradient.0 + second.x * gradient.1) / length * mean_edge,
            here.y + (first.y * gradient.0 + second.y * gradient.1) / length * mean_edge,
            here.z + (first.z * gradient.0 + second.z * gradient.1) / length * mean_edge,
        ),
        QUALITY_ASCENT_EDGE_FRACTION,
    )
}

fn quality_problem_sites(
    mesh: &AdaptiveMesh,
    window_first: bool,
    excluded: &BTreeSet<usize>,
) -> (Vec<usize>, usize, usize) {
    let state = mesh.state();
    let mut worst: BTreeMap<usize, (f64, f64)> = BTreeMap::new();
    for triangle in state.active_triangle_slots() {
        let Some(margin) = triangle_window_margin(state, triangle) else {
            continue;
        };
        let Some(eta) = triangle_eta_value(state, triangle) else {
            continue;
        };
        if (window_first && margin >= 0.0) || (!window_first && eta >= QUALITY_ETA_TARGET) {
            continue;
        }
        for site in state.triangles()[triangle] {
            worst
                .entry(site)
                .and_modify(|current| {
                    current.0 = current.0.min(margin);
                    current.1 = current.1.min(eta);
                })
                .or_insert((margin, eta));
        }
    }
    let movable: Vec<_> = worst
        .into_iter()
        .filter(|(site, _)| mesh.can_move_site(*site))
        .collect();
    let found = movable.len();
    let mut sites: Vec<_> = movable
        .into_iter()
        .filter(|(site, _)| !excluded.contains(site))
        .collect();
    sites.sort_by(|left, right| {
        left.1
             .0
            .total_cmp(&right.1 .0)
            .then_with(|| left.1 .1.total_cmp(&right.1 .1))
            .then_with(|| left.0.cmp(&right.0))
    });
    if !window_first {
        sites.sort_by(|left, right| {
            left.1
                 .1
                .total_cmp(&right.1 .1)
                .then_with(|| left.1 .0.total_cmp(&right.1 .0))
                .then_with(|| left.0.cmp(&right.0))
        });
    }
    let eligible = sites.len();
    let sites = sites
        .into_iter()
        .take(MAXIMUM_QUALITY_SITES_PER_PASS)
        .map(|(site, _)| site)
        .collect();
    (sites, found, eligible)
}

#[derive(Clone, Debug)]
struct QualityGuardMetrics {
    pending: usize,
    unbalanced: usize,
    worst_scale_ratio: f64,
    angles: AngleWindowSurvey,
    eta: Vec<f64>,
    margins: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq)]
struct LowDegreeScore {
    unresolved: usize,
    scale_violations: usize,
    worst_scale_ratio: f64,
    deficit: usize,
    window_margin: Vec<f64>,
    triangle_eta: Vec<f64>,
}

impl LowDegreeScore {
    fn no_worse_than(&self, other: &Self) -> bool {
        let not_worse = |order| {
            matches!(
                order,
                Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
            )
        };
        self.unresolved <= other.unresolved
            && self.scale_violations <= other.scale_violations
            && (self.scale_violations == 0 || self.worst_scale_ratio <= other.worst_scale_ratio)
            && self.deficit <= other.deficit
            && not_worse(worst_first_cmp(&self.window_margin, &other.window_margin))
            && not_worse(worst_first_cmp(&self.triangle_eta, &other.triangle_eta))
    }
}

impl PartialOrd for LowDegreeScore {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let no_worse = self.no_worse_than(other);
        let no_better = other.no_worse_than(self);
        match (no_worse, no_better, self.deficit.cmp(&other.deficit)) {
            (true, _, std::cmp::Ordering::Less) => Some(std::cmp::Ordering::Less),
            (_, true, std::cmp::Ordering::Greater) => Some(std::cmp::Ordering::Greater),
            (true, true, std::cmp::Ordering::Equal) => Some(std::cmp::Ordering::Equal),
            _ => None,
        }
    }
}

fn low_degree_deficit(state: &MeshState, sites: &AffectedSites) -> Option<usize> {
    let mut degrees: BTreeMap<usize, usize> = sites.keys().map(|&site| (site, 0)).collect();
    for triangle in state
        .active_triangle_slots()
        .map(|triangle| &state.triangles()[triangle])
    {
        for site in triangle {
            if let Some(degree) = degrees.get_mut(site) {
                *degree += 1;
            }
        }
    }
    Some(
        degrees
            .into_values()
            .map(|degree| 5usize.saturating_sub(degree))
            .sum(),
    )
}

fn degree_gain_quads(state: &MeshState, centre: usize) -> Vec<(usize, usize, usize)> {
    let Ok(fan) = state.triangle_fan(centre) else {
        return Vec::new();
    };
    let mut candidates = BTreeSet::new();
    for triangle in fan {
        let Some(corner) = state.triangles()[triangle]
            .iter()
            .position(|&site| site == centre)
        else {
            continue;
        };
        let neighbour = state.neighbours()[triangle][corner];
        if neighbour < MESH_STATE_FIRST_ID {
            continue;
        }
        if let Some(outside) = state.triangles()[neighbour]
            .iter()
            .copied()
            .find(|site| !state.triangles()[triangle].contains(site))
        {
            let tail = state.triangles()[triangle][(corner + 1) % 3];
            let head = state.triangles()[triangle][(corner + 2) % 3];
            candidates.insert((outside, tail.min(head), tail.max(head)));
        }
    }
    candidates.into_iter().collect()
}

fn repair_low_degree_stars(
    mesh: &mut AdaptiveMesh,
    criteria: &[Box<dyn CellCriterion>],
    gates: HardGates,
    limits: CycleLimits,
) -> Result<usize> {
    if !mesh.segments_are_empty() {
        return Ok(0);
    }
    let mut committed = 0;
    for pass in 0..MAXIMUM_LOW_DEGREE_PASSES {
        let degrees = vertex_degrees(mesh.state());
        let mut sites: Vec<_> = mesh
            .state()
            .active_vertex_slots()
            .filter(|&site| degrees[site] < 5 && mesh.can_move_site(site))
            .collect();
        sites.sort_by_key(|&site| (degrees[site], site));
        if sites.is_empty() {
            break;
        }
        let before_count = sites.len();
        let mut committed_this_pass = 0;
        for centre in sites {
            if mesh
                .state()
                .vertex_degree(centre)
                .ok()
                .is_none_or(|degree| degree >= 5)
            {
                continue;
            }
            let objective = |state: &MeshState, affected: &AffectedSites| {
                let balance = balance_objective(state, affected, limits.max_neighbour_scale_ratio)?;
                let mut unresolved = 0;
                let radius_m = state.sphere_radius();
                'site: for (&site, &seed) in affected {
                    let cell = state.voronoi_cell_from(site, seed).ok()?;
                    let view = CellView {
                        site,
                        cell: &cell,
                        state,
                        radius_m,
                    };
                    for criterion in criteria {
                        if criterion.evaluate(&view).ok()?.demands_work() {
                            unresolved += 1;
                            continue 'site;
                        }
                    }
                }
                Some(LowDegreeScore {
                    unresolved,
                    scale_violations: balance[0] as usize,
                    worst_scale_ratio: balance[1],
                    deficit: low_degree_deficit(state, affected)?,
                    window_margin: triangle_window_margins(state, affected)?,
                    triangle_eta: triangle_eta_values(state, affected)?,
                })
            };
            let Some(before) = mesh.score_before_move(centre, &objective) else {
                continue;
            };
            let here = mesh.state().vertices()[centre];
            let quads = degree_gain_quads(mesh.state(), centre);
            let mut moved = false;
            for (opposite, tail, head) in quads {
                let there = mesh.state().vertices()[opposite];
                for step in LOW_DEGREE_LINE_SEARCH_STEPS {
                    let Some(destination) = projected_step(here, there, step) else {
                        continue;
                    };
                    if let Acceptance::Committed(_) = mesh.propose_move_cached(
                        centre,
                        destination,
                        gates,
                        &objective,
                        Some(&before),
                        true,
                    )? {
                        moved = true;
                        committed_this_pass += 1;
                        break;
                    }
                }
                if moved || !mesh.can_move_site(opposite) {
                    if moved {
                        break;
                    }
                    continue;
                }
                for step in LOW_DEGREE_PAIR_LINE_SEARCH_STEPS {
                    let (Some(first), Some(second)) = (
                        projected_step(here, there, step),
                        projected_step(there, here, step),
                    ) else {
                        continue;
                    };
                    if let Acceptance::Committed(_) = mesh.propose_pair_move_cached(
                        (centre, first),
                        (opposite, second),
                        gates,
                        &objective,
                        None,
                    )? {
                        moved = true;
                        committed_this_pass += 2;
                        break;
                    }
                }
                if moved {
                    break;
                }
                if !mesh.can_move_site(tail) || !mesh.can_move_site(head) {
                    continue;
                }
                let tail_here = mesh.state().vertices()[tail];
                let head_here = mesh.state().vertices()[head];
                let tail_away = CartesianPoint::new(
                    tail_here.x + (tail_here.x - head_here.x),
                    tail_here.y + (tail_here.y - head_here.y),
                    tail_here.z + (tail_here.z - head_here.z),
                );
                let head_away = CartesianPoint::new(
                    head_here.x + (head_here.x - tail_here.x),
                    head_here.y + (head_here.y - tail_here.y),
                    head_here.z + (head_here.z - tail_here.z),
                );
                for step in LOW_DEGREE_PAIR_LINE_SEARCH_STEPS {
                    let (Some(first), Some(second)) = (
                        projected_step(tail_here, tail_away, step),
                        projected_step(head_here, head_away, step),
                    ) else {
                        continue;
                    };
                    if let Acceptance::Committed(_) = mesh.propose_pair_move_cached(
                        (tail, first),
                        (head, second),
                        gates,
                        &objective,
                        None,
                    )? {
                        moved = true;
                        committed_this_pass += 2;
                        break;
                    }
                }
                if moved {
                    break;
                }
            }
        }
        committed += committed_this_pass;
        eprintln!(
            "harp_dv low-degree repair pass {}/{}: {} moved site(s), {} -> {} vertices below degree 5",
            pass + 1,
            MAXIMUM_LOW_DEGREE_PASSES,
            committed_this_pass,
            before_count,
            vertices_below_degree_5(mesh.state())
        );
        if committed_this_pass == 0 {
            break;
        }
    }
    Ok(committed)
}

/// Whether one cell is still asking, and how big it is.
///
/// Exactly the predicate `evaluate` applies -- built from the same evidences
/// and filtered by the same floor -- factored out so the pass guard can refresh
/// one site without sweeping the mesh.
fn guard_cell(
    state: &MeshState,
    criteria: &[Box<dyn CellCriterion>],
    limits: CycleLimits,
    site: usize,
    seed: usize,
    radius_m: f64,
) -> Result<(Option<f64>, bool)> {
    let Ok(cell) = state.voronoi_cell_from(site, seed) else {
        return Ok((None, false));
    };
    let view = CellView {
        site,
        cell: &cell,
        state,
        radius_m,
    };
    let scale = view.effective_scale_m();
    let at_floor = scale.is_some_and(|scale| scale <= limits.minimum_cell_width_m);
    let mut evidences = Vec::new();
    for criterion in criteria {
        let evidence = criterion.evaluate(&view)?;
        if evidence.demands_work() || !evidence.satisfiable {
            evidences.push(evidence);
        }
    }
    let demand =
        RefinementDemand::from_evidence(site as u64, evidences, RefinementCause::UserSpecified);
    Ok((scale, demand.demands_work() && !at_floor))
}

/// Every cell's scale, and which cells still ask, kept across quality passes.
///
/// The guard used to call `evaluate` once per pass, which builds a Voronoi cell
/// for every active site: 70,685 of them, 48 times, at NXP80. A sample put 805
/// of the guard's 813 samples in that sweep and 797 of those in the ring walk.
///
/// A pass only moves the neighbourhoods it commits to, so only those cells can
/// have changed. Refreshing them and keeping the rest is the same answer for
/// a fraction of the work -- and because a rejected pass restores the mesh, the
/// cache is snapshotted and restored with it.
#[derive(Clone)]
struct GuardCells {
    scales: Vec<Option<f64>>,
    demanding: BTreeSet<usize>,
}

impl GuardCells {
    fn full(
        state: &MeshState,
        criteria: &[Box<dyn CellCriterion>],
        limits: CycleLimits,
    ) -> Result<Self> {
        let mut cells = Self {
            scales: vec![None; state.vertices().len()],
            demanding: BTreeSet::new(),
        };
        let sites: BTreeSet<usize> = state.active_vertex_slots().collect();
        cells.refresh(state, criteria, limits, &sites)?;
        Ok(cells)
    }

    fn refresh(
        &mut self,
        state: &MeshState,
        criteria: &[Box<dyn CellCriterion>],
        limits: CycleLimits,
        sites: &BTreeSet<usize>,
    ) -> Result<()> {
        let radius_m = state.sphere_radius();
        let seeds = active_site_triangle_seeds(state);
        for &site in sites {
            let (scale, demanding) = match seeds.get(site).copied().flatten() {
                Some(seed) => guard_cell(state, criteria, limits, site, seed, radius_m)?,
                None => (None, false),
            };
            self.scales[site] = scale;
            if demanding {
                self.demanding.insert(site);
            } else {
                self.demanding.remove(&site);
            }
        }
        Ok(())
    }

    /// The cells a committed move can have changed, generously bounded.
    ///
    /// A move rewrites its own star and whatever legalising flipped around it,
    /// which is inside the two rings the transaction snapshots. Three rings is
    /// the same set with room to spare: over-invalidating costs a rebuild, and
    /// under-invalidating leaves a stale number in a guard that decides whether
    /// a whole pass is kept.
    fn dirty_around(state: &MeshState, moved: &BTreeSet<usize>) -> BTreeSet<usize> {
        topological_rings(state, moved, 3)
            .into_iter()
            .flatten()
            .collect()
    }
}

// Compare the incrementally kept cells against a fresh sweep. Off unless a test
// asks for it. The failure this guards against is silent: a dirty set that
// misses a cell leaves a stale scale in a number that decides whether a whole
// pass is kept, so the run stays valid and diverges.
#[cfg(test)]
thread_local! {
    /// How many more passes to check. Drift shows up as soon as a cell the
    /// refresh missed is read, and the early passes commit the most moves, so a
    /// bounded count catches it without running the comparison forty-eight
    /// times over -- which in a debug build is most of the test's runtime.
    static VERIFY_GUARD_CELLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn verify_guard_cells(
    state: &MeshState,
    criteria: &[Box<dyn CellCriterion>],
    limits: CycleLimits,
    cells: &GuardCells,
) -> Result<()> {
    let remaining = VERIFY_GUARD_CELLS.with(|verify| verify.get());
    if remaining == 0 {
        return Ok(());
    }
    VERIFY_GUARD_CELLS.with(|verify| verify.set(remaining - 1));
    let full = GuardCells::full(state, criteria, limits)?;
    for (site, (kept, fresh)) in cells.scales.iter().zip(&full.scales).enumerate() {
        assert_eq!(
            kept.map(f64::to_bits),
            fresh.map(f64::to_bits),
            "site {site} kept a stale scale"
        );
    }
    assert_eq!(
        cells.demanding, full.demanding,
        "the kept set of demanding cells drifted from a fresh sweep"
    );
    Ok(())
}

#[cfg(not(test))]
fn verify_guard_cells(
    _: &MeshState,
    _: &[Box<dyn CellCriterion>],
    _: CycleLimits,
    _: &GuardCells,
) -> Result<()> {
    Ok(())
}

impl QualityGuardMetrics {
    #[cfg(test)]
    fn read(
        mesh: &AdaptiveMesh,
        criteria: &[Box<dyn CellCriterion>],
        limits: CycleLimits,
    ) -> Result<Self> {
        let cells = GuardCells::full(mesh.state(), criteria, limits)?;
        Self::read_with(mesh, limits, &cells)
    }

    fn read_with(mesh: &AdaptiveMesh, limits: CycleLimits, cells: &GuardCells) -> Result<Self> {
        let scales = &cells.scales;
        let balance = balance_demands(mesh, scales, limits);
        let (unbalanced, worst_scale_ratio) =
            balance_survey_from_scales(mesh.state(), scales, limits);
        let pending: BTreeSet<u64> = cells
            .demanding
            .iter()
            .map(|&site| site as u64)
            .chain(balance.iter().map(|demand| demand.cell))
            .collect();
        Ok(Self {
            pending: pending.len(),
            unbalanced,
            worst_scale_ratio,
            angles: angle_window_survey(mesh.state()),
            eta: all_triangle_eta_values(mesh.state()).ok_or_else(|| {
                crate::error::HarpDvError::TopologyViolation(
                    "triangle area-length quality is not measurable".to_string(),
                )
            })?,
            margins: all_triangle_window_margins(mesh.state()).ok_or_else(|| {
                crate::error::HarpDvError::TopologyViolation(
                    "triangle angle-window margin is not measurable".to_string(),
                )
            })?,
        })
    }

    fn regression_from(&self, before: &Self, guard_window: bool) -> Option<&'static str> {
        if self.pending > before.pending {
            Some("pending demands increased")
        } else if self.unbalanced > before.unbalanced {
            Some("unbalanced neighbour pairs increased")
        } else if self.unbalanced > 0 && self.worst_scale_ratio > before.worst_scale_ratio {
            Some("worst neighbour scale ratio increased")
        } else if guard_window
            && self.angles.worst_deviation_deg > before.angles.worst_deviation_deg
        {
            Some("worst 40-80 degree deviation increased")
        } else if guard_window
            && self.angles.below + self.angles.above_80
                > before.angles.below + before.angles.above_80
        {
            Some("40-80 degree violations increased")
        } else if guard_window && self.angles.penalty > before.angles.penalty {
            Some("40-80 degree penalty increased")
        } else if guard_window
            && worst_first_cmp(&self.margins, &before.margins) == Some(std::cmp::Ordering::Greater)
        {
            Some("global angle-window margin vector regressed")
        } else if worst_first_cmp(&self.eta, &before.eta) == Some(std::cmp::Ordering::Greater) {
            Some("global area-length quality vector regressed")
        } else {
            None
        }
    }
}

/// Which destination a quality move came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MoveSource {
    Natural,
    Eta,
    Window,
}

/// Where the quality optimiser's moves came from.
///
/// Diagnostic only: it is not serialised, does not reach the run report, and
/// counts nothing outside the three-candidate loop -- the low-degree repair
/// moves are tallied separately. Generated and line-search counts describe all
/// work attempted; committed counts describe only moves retained after the
/// pass-level guard. It exists because a summary that only shows the finished
/// mesh cannot tell "the candidate never fired" from "the candidate fired and
/// was overruled", and those two call for opposite fixes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct QualityMoveAudit {
    natural_generated: usize,
    natural_line_search_attempts: usize,
    natural_committed: usize,
    eta_generated: usize,
    eta_line_search_attempts: usize,
    eta_committed: usize,
    window_generated: usize,
    window_line_search_attempts: usize,
    window_committed: usize,
    low_degree_committed: usize,
}

impl QualityMoveAudit {
    /// A destination was computed, whether or not the loop got as far as it.
    fn generated(&mut self, source: MoveSource) {
        match source {
            MoveSource::Natural => self.natural_generated += 1,
            MoveSource::Eta => self.eta_generated += 1,
            MoveSource::Window => self.window_generated += 1,
        }
    }

    /// A step along that destination reached the transaction.
    fn attempted(&mut self, source: MoveSource) {
        match source {
            MoveSource::Natural => self.natural_line_search_attempts += 1,
            MoveSource::Eta => self.eta_line_search_attempts += 1,
            MoveSource::Window => self.window_line_search_attempts += 1,
        }
    }

    fn committed(&mut self, source: MoveSource) {
        match source {
            MoveSource::Natural => self.natural_committed += 1,
            MoveSource::Eta => self.eta_committed += 1,
            MoveSource::Window => self.window_committed += 1,
        }
    }

    fn retained_commits(&self) -> [usize; 3] {
        [
            self.natural_committed,
            self.eta_committed,
            self.window_committed,
        ]
    }

    fn restore_retained_commits(&mut self, retained: [usize; 3]) {
        [
            self.natural_committed,
            self.eta_committed,
            self.window_committed,
        ] = retained;
    }

    fn retained_total(&self) -> usize {
        self.retained_commits().into_iter().sum::<usize>() + self.low_degree_committed
    }
}

fn optimise_mesh_quality(
    mesh: &mut AdaptiveMesh,
    criteria: &[Box<dyn CellCriterion>],
    gates: HardGates,
    limits: CycleLimits,
    background_scale_m: Option<f64>,
) -> Result<(usize, AngleWindowSurvey)> {
    let (committed, angles, _) = optimise_mesh_quality_with_natural_length(
        mesh,
        criteria,
        gates,
        limits,
        background_scale_m,
        true,
        NATURAL_LENGTH_PASSES,
    )?;
    Ok((committed, angles))
}

/// The optimiser with the natural-length candidate under test.
///
/// `natural_length_enabled` removes the candidate from every phase rather than
/// merely demoting it -- setting the priority passes to zero still leaves it in
/// the list behind the eta ascent, which is a different arm and a different
/// question. Neither knob is user-visible: `optimise_mesh_quality` is the only
/// production entry and it always ships the shipped semantics.
#[allow(clippy::too_many_arguments)]
fn optimise_mesh_quality_with_natural_length(
    mesh: &mut AdaptiveMesh,
    criteria: &[Box<dyn CellCriterion>],
    gates: HardGates,
    limits: CycleLimits,
    background_scale_m: Option<f64>,
    natural_length_enabled: bool,
    natural_length_priority_passes: usize,
) -> Result<(usize, AngleWindowSurvey, QualityMoveAudit)> {
    let audit = QualityMoveAudit::default();
    if !mesh.segments_are_empty() {
        eprintln!("harp_dv quality optimiser skipped: protected boundary segments are present");
        return Ok((0, AngleWindowSurvey::default(), audit));
    }
    let Some(background_scale_m) = background_scale_m else {
        eprintln!("harp_dv quality optimiser skipped: initial cell scale is not measurable");
        return Ok((0, AngleWindowSurvey::default(), audit));
    };
    let Some(target_cell_scale) = target_cell_scales(
        mesh.state(),
        criteria,
        limits.minimum_cell_width_m,
        background_scale_m,
    ) else {
        eprintln!("harp_dv quality optimiser skipped: no usable target-scale criterion");
        return Ok((0, AngleWindowSurvey::default(), audit));
    };
    let mut audit = audit;
    let target_angles = target_angle_window_survey(mesh.state(), &target_cell_scale);
    eprintln!(
        "harp_dv frozen target field angle diagnostic: below_40={}, in_40_80={}, above_80={}, unmeasurable={}, worst_deviation={:.3}",
        target_angles.below,
        target_angles.inside_40_80,
        target_angles.above_80,
        target_angles.unmeasurable,
        target_angles.worst_deviation_deg
    );
    // No vertices are inserted during this phase. Keep the sampled size field
    // fixed so every pass optimises the same objective instead of rebuilding it
    // from the geometry produced by the previous pass.
    let started = std::time::Instant::now();
    let initial_low_degree_moves = repair_low_degree_stars(mesh, criteria, gates, limits)?;
    audit.low_degree_committed = initial_low_degree_moves;
    let mut guard_cells = GuardCells::full(mesh.state(), criteria, limits)?;
    let initial = QualityGuardMetrics::read_with(mesh, limits, &guard_cells)?;
    let mut previous = initial.clone();
    let mut committed = initial_low_degree_moves;
    let mut eta_stopped = false;
    let mut excluded_sites = BTreeSet::new();
    let mut previous_phase = false;
    for pass in 0..MAXIMUM_QUALITY_PASSES {
        let window_first = pass >= ETA_QUALITY_PASSES;
        if window_first != previous_phase {
            excluded_sites.clear();
            previous_phase = window_first;
        }
        if !window_first && eta_stopped {
            continue;
        }
        let phase = if window_first { "window" } else { "eta" };
        let phase_pass = if window_first {
            pass + 1 - ETA_QUALITY_PASSES
        } else {
            pass + 1
        };
        let phase_passes = if window_first {
            WINDOW_QUALITY_PASSES
        } else {
            ETA_QUALITY_PASSES
        };
        if window_first && phase_pass == WINDOW_BREADTH_PASSES + 1 {
            excluded_sites.clear();
        }
        let pass_checkpoint = mesh.clone();
        let cells_checkpoint = guard_cells.clone();
        let pass_before = previous.clone();
        let retained_before = audit.retained_commits();
        let (sites, found, eligible) = quality_problem_sites(mesh, window_first, &excluded_sites);
        let breadth_sweep_exhausted =
            window_first && eligible <= MAXIMUM_QUALITY_SITES_PER_PASS && found > eligible;
        eprintln!(
            "harp_dv quality optimiser {phase} pass {phase_pass}/{phase_passes}: processing {} of {} eligible / {} current movable worst-first sites",
            sites.len(),
            eligible,
            found
        );
        if sites.is_empty() {
            if window_first && phase_pass <= WINDOW_BREADTH_PASSES {
                excluded_sites.clear();
                continue;
            }
            if window_first {
                break;
            }
            eta_stopped = true;
            continue;
        }
        let attempted_sites = sites.clone();
        let mut unproductive_sites: BTreeSet<_> = attempted_sites.iter().copied().collect();
        let mut moved_this_pass = BTreeSet::new();
        let mut committed_this_pass = 0usize;
        for site in sites {
            let objective = |state: &MeshState, affected: &AffectedSites| {
                let balance = balance_objective(state, affected, limits.max_neighbour_scale_ratio)?;
                let mut unresolved = 0usize;
                let radius_m = state.sphere_radius();
                for (&site, &seed) in affected {
                    let cell = state.voronoi_cell_from(site, seed).ok()?;
                    let view = CellView {
                        site,
                        cell: &cell,
                        state,
                        radius_m,
                    };
                    for criterion in criteria {
                        if criterion.evaluate(&view).ok()?.demands_work() {
                            unresolved += 1;
                            break;
                        }
                    }
                }
                Some(QualityScore {
                    unresolved,
                    scale_violations: balance[0] as usize,
                    worst_scale_ratio: balance[1],
                    window_first,
                    window_margin: triangle_window_margins(state, affected)?,
                    triangle_eta: triangle_eta_values(state, affected)?,
                })
            };
            let Some(before) = mesh.score_before_move(site, &objective) else {
                continue;
            };
            let here = mesh.state().vertices()[site];
            let natural = natural_length_enabled
                .then(|| natural_length_destination(mesh.state(), &target_cell_scale, site))
                .flatten();
            let eta_ascent = worst_triangle_ascent_destination(mesh.state(), site);
            let window_ascent = window_first
                .then(|| star_window_ascent_destination(mesh.state(), site))
                .flatten();
            let targets = if window_first {
                [
                    (MoveSource::Window, window_ascent),
                    (MoveSource::Eta, eta_ascent),
                    (MoveSource::Natural, natural),
                ]
            } else if pass < natural_length_priority_passes {
                [
                    (MoveSource::Natural, natural),
                    (MoveSource::Eta, eta_ascent),
                    (MoveSource::Window, None),
                ]
            } else {
                [
                    (MoveSource::Eta, eta_ascent),
                    (MoveSource::Natural, natural),
                    (MoveSource::Window, None),
                ]
            };
            // Counted here rather than inside the loop below: a destination that
            // exists but is never reached because an earlier candidate committed
            // has still been generated, and reading it as ungenerated would point
            // the diagnosis at the wiring instead of at the ordering.
            for (source, target) in targets {
                if target.is_some() {
                    audit.generated(source);
                }
            }
            'candidate: for (source, target) in targets {
                let Some(target) = target else { continue };
                for step in QUALITY_LINE_SEARCH_STEPS {
                    let Some(destination) = projected_step(here, target, step) else {
                        continue;
                    };
                    audit.attempted(source);
                    if let Acceptance::Committed(_) = mesh.propose_move_cached(
                        site,
                        destination,
                        gates,
                        &objective,
                        Some(&before),
                        true,
                    )? {
                        audit.committed(source);
                        committed_this_pass += 1;
                        moved_this_pass.insert(site);
                        unproductive_sites.remove(&site);
                        break 'candidate;
                    }
                }
            }
        }

        let dirty = GuardCells::dirty_around(mesh.state(), &moved_this_pass);
        if let Err(error) = guard_cells.refresh(mesh.state(), criteria, limits, &dirty) {
            *mesh = pass_checkpoint;
            return Err(error);
        }
        verify_guard_cells(mesh.state(), criteria, limits, &guard_cells)?;
        let after = match QualityGuardMetrics::read_with(mesh, limits, &guard_cells) {
            Ok(after) => after,
            Err(error) => {
                *mesh = pass_checkpoint;
                return Err(error);
            }
        };
        let eta_p1 = after.eta[(after.eta.len() / 100).min(after.eta.len().saturating_sub(1))];
        let below_eta_0_89 = after.eta.partition_point(|value| *value < 0.89);
        let retained_after = audit.retained_commits();
        let retained_this_pass = [
            retained_after[0] - retained_before[0],
            retained_after[1] - retained_before[1],
            retained_after[2] - retained_before[2],
        ];
        let regression = after.regression_from(&pass_before, window_first);
        let decision = if regression.is_some() {
            "rejected"
        } else {
            "retained"
        };
        eprintln!(
            "harp_dv quality optimiser {phase} pass {phase_pass}/{phase_passes}: {decision} {} tentative moves (natural/eta/window={}/{}/{}), outside {} -> {}, worst_deviation {:.6} -> {:.6}, margin_min={:.6}, eta_min={:.6} -> {:.6}, eta_p1={:.6}, triangles_below_eta_0_89={}",
            committed_this_pass,
            retained_this_pass[0],
            retained_this_pass[1],
            retained_this_pass[2],
            pass_before.angles.below + pass_before.angles.above_80,
            after.angles.below + after.angles.above_80,
            pass_before.angles.worst_deviation_deg,
            after.angles.worst_deviation_deg,
            after.margins[0],
            pass_before.eta[0],
            after.eta[0],
            eta_p1,
            below_eta_0_89
        );
        if let Some(guard) = regression {
            *mesh = pass_checkpoint;
            guard_cells = cells_checkpoint;
            audit.restore_retained_commits(retained_before);
            eprintln!(
                "harp_dv quality optimiser rejected {phase} pass {phase_pass} after {} move(s): {}",
                committed_this_pass, guard
            );
            excluded_sites.extend(attempted_sites);
            if eligible <= MAXIMUM_QUALITY_SITES_PER_PASS {
                if window_first {
                    break;
                }
                eta_stopped = true;
            }
            continue;
        }
        committed += committed_this_pass;
        previous = after;
        if committed_this_pass == 0 {
            excluded_sites.extend(attempted_sites);
            if window_first && found > eligible {
                excluded_sites.clear();
                continue;
            }
            if eligible <= MAXIMUM_QUALITY_SITES_PER_PASS {
                if window_first {
                    break;
                }
                eta_stopped = true;
            }
        } else if breadth_sweep_exhausted {
            excluded_sites.clear();
        } else {
            if window_first && phase_pass <= WINDOW_BREADTH_PASSES {
                // Finish a bounded breadth sweep before retrying failures. Once
                // every current candidate has been seen, the branch above clears
                // the set and reconsiders them on the changed mesh.
                excluded_sites.extend(unproductive_sites);
            } else {
                excluded_sites.clear();
            }
        }
    }
    let final_low_degree_moves = repair_low_degree_stars(mesh, criteria, gates, limits)?;
    committed += final_low_degree_moves;
    audit.low_degree_committed += final_low_degree_moves;
    if final_low_degree_moves > 0 {
        guard_cells = GuardCells::full(mesh.state(), criteria, limits)?;
        previous = QualityGuardMetrics::read_with(mesh, limits, &guard_cells)?;
    }
    eprintln!(
        "harp_dv quality optimiser complete: {} moves, margin_min {:.6} -> {:.6}, eta_min {:.6} -> {:.6}, angle-window violations {} -> {}, {:.1}s",
        committed,
        initial.margins[0],
        previous.margins[0],
        initial.eta[0],
        previous.eta[0],
        initial.angles.below + initial.angles.above_80,
        previous.angles.below + previous.angles.above_80,
        started.elapsed().as_secs_f64()
    );
    eprintln!(
        "harp_dv quality optimiser candidates (generated/line-search/retained): natural {}/{}/{}, eta {}/{}/{}, window {}/{}/{}, low-degree retained {}",
        audit.natural_generated,
        audit.natural_line_search_attempts,
        audit.natural_committed,
        audit.eta_generated,
        audit.eta_line_search_attempts,
        audit.eta_committed,
        audit.window_generated,
        audit.window_line_search_attempts,
        audit.window_committed,
        audit.low_degree_committed,
    );
    assert_eq!(
        audit.retained_total(),
        committed,
        "quality move audit must account for every retained move"
    );
    Ok((committed, target_angles, audit))
}

const QUALITY_ETA_TARGET: f64 = 0.9375;
const TARGET_SCALE_GRADIENT: f64 = 0.3;
// Regular triangular lattice: A_voronoi=sqrt(3)/2*l² and h=sqrt(A/pi).
const CELL_SCALE_TO_EDGE_LENGTH: f64 = 1.904_625_613_727_914_7;
const ETA_QUALITY_PASSES: usize = 16;
const WINDOW_QUALITY_PASSES: usize = 32;
const WINDOW_BREADTH_PASSES: usize = WINDOW_QUALITY_PASSES;
const MAXIMUM_QUALITY_PASSES: usize = ETA_QUALITY_PASSES + WINDOW_QUALITY_PASSES;
const NATURAL_LENGTH_PASSES: usize = 2;
const MAXIMUM_QUALITY_SITES_PER_PASS: usize = 1_024;
const QUALITY_LINE_SEARCH_STEPS: [f64; 5] = [0.5, 0.25, 0.125, 0.0625, 0.03125];
const QUALITY_GRADIENT_PROBE_FRACTION: f64 = 0.01;
const QUALITY_ASCENT_EDGE_FRACTION: f64 = 0.5;
const MAXIMUM_LOW_DEGREE_PASSES: usize = 4;
const LOW_DEGREE_LINE_SEARCH_STEPS: [f64; 4] = [0.25, 0.125, 0.0625, 0.03125];
const LOW_DEGREE_PAIR_LINE_SEARCH_STEPS: [f64; 5] = [0.5, 0.25, 0.125, 0.0625, 0.03125];

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
    let seeds = active_site_triangle_seeds(state);
    let mut demands = Vec::new();
    let mut tally = EvaluationTally::default();
    let mut scales = vec![None; state.vertices().len()];
    for site in state.active_vertex_slots() {
        // A cell that cannot be read is not a demand. Skipping it here keeps
        // evaluation total; the transaction layer reports the same cell as
        // `NotAttempted` if anything later asks for it.
        let Some(seed) = seeds[site] else {
            continue;
        };
        let Ok(cell) = state.voronoi_cell_from(site, seed) else {
            continue;
        };
        let view = CellView {
            site,
            cell: &cell,
            state,
            radius_m,
        };
        let scale = view.effective_scale_m();
        scales[site] = scale;
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
                if evidence.semantics == CriterionSemantics::MeshQuality {
                    RefinementCause::QualityRepair
                } else {
                    RefinementCause::PhysicalCriterion {
                        criterion_id: evidence.criterion_id.clone(),
                    }
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

// Test-only capture support for the exact production boundary before quality
// work. Thread-local state keeps parallel tests independent.
#[cfg(test)]
thread_local! {
    static CAPTURE_QUALITY_CHECKPOINT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static QUALITY_CHECKPOINT: std::cell::RefCell<Option<AdaptiveMesh>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn capture_quality_checkpoint(mesh: &AdaptiveMesh) -> bool {
    let requested = CAPTURE_QUALITY_CHECKPOINT.with(|capture| capture.get());
    if requested {
        QUALITY_CHECKPOINT.with(|checkpoint| {
            *checkpoint.borrow_mut() = Some(mesh.clone());
        });
    }
    requested
}

/// Build the same refinement/r-adaptation state production hands to the
/// quality optimiser, then return the captured state for controlled A/B tests.
#[cfg(test)]
fn production_quality_checkpoint(
    mesh: &mut AdaptiveMesh,
    criteria: &[Box<dyn CellCriterion>],
    policy: CandidatePolicy,
    gates: HardGates,
    limits: CycleLimits,
) -> AdaptiveMesh {
    production_quality_checkpoint_with_local_recovery(
        mesh,
        criteria,
        policy,
        gates,
        limits,
        LocalRecoveryPolicy::OFF,
    )
}

#[cfg(test)]
fn production_quality_checkpoint_with_local_recovery(
    mesh: &mut AdaptiveMesh,
    criteria: &[Box<dyn CellCriterion>],
    policy: CandidatePolicy,
    gates: HardGates,
    limits: CycleLimits,
    local_recovery: LocalRecoveryPolicy,
) -> AdaptiveMesh {
    CAPTURE_QUALITY_CHECKPOINT.with(|capture| {
        assert!(
            !capture.replace(true),
            "quality checkpoint capture is not re-entrant"
        );
    });
    QUALITY_CHECKPOINT.with(|checkpoint| {
        assert!(checkpoint.borrow_mut().take().is_none());
    });

    let run = run_cycles_with_local_recovery(mesh, criteria, policy, gates, limits, local_recovery);
    CAPTURE_QUALITY_CHECKPOINT.with(|capture| capture.set(false));
    run.expect("production refinement reaches the quality boundary");
    QUALITY_CHECKPOINT
        .with(|checkpoint| checkpoint.borrow_mut().take())
        .expect("production run did not reach the quality boundary")
}

/// Run cycles until something says to stop, and say which.
/// When a cycle may escalate a stall that is local rather than global.
///
/// Every escalation the loop has -- the broader ladder, pair relief, multi-ring
/// recovery -- triggers on "nothing was accepted anywhere this cycle". That
/// scalar is false whenever any corner of the mesh still accepts an insertion,
/// which on a production mesh is every cycle: measured zero firings in 11
/// cycles at NXP40 and in 100 at NXP80, while a degree-blocked site was waiting
/// in 10 and 100 of them (`.omx/plans/harp-dv-stall-escalation-diagnosis.md`).
///
/// Letting persistence alone open the gate is the thing the note above the
/// fallback ladder records as tried and rejected: it took the crate's own suite
/// from 32 seconds past 30 minutes without finishing, and says what was missing
/// -- "a bound on how much broadening a cycle may buy". Hence the seed cap.
///
/// Off in production until an A/B says otherwise.
#[derive(Clone, Copy, Debug, PartialEq)]
struct LocalRecoveryPolicy {
    /// Cycles a site must stay degree-blocked before it may seed a recovery.
    minimum_consecutive_cycles: u32,
    /// The most seeds one cycle may hand to `recover_stalled_regions`. Zero
    /// leaves the escalation exactly where it is.
    maximum_seeds_per_cycle: usize,
}

impl LocalRecoveryPolicy {
    /// Shipped behaviour: escalation stays behind the global scalar.
    const OFF: Self = Self {
        minimum_consecutive_cycles: 0,
        maximum_seeds_per_cycle: 0,
    };
}

pub fn run_cycles(
    mesh: &mut AdaptiveMesh,
    criteria: &[Box<dyn CellCriterion>],
    policy: CandidatePolicy,
    gates: HardGates,
    limits: CycleLimits,
) -> Result<CycleOutcome> {
    run_cycles_with_local_recovery(
        mesh,
        criteria,
        policy,
        gates,
        limits,
        LocalRecoveryPolicy::OFF,
    )
}

fn run_cycles_with_local_recovery(
    mesh: &mut AdaptiveMesh,
    criteria: &[Box<dyn CellCriterion>],
    policy: CandidatePolicy,
    gates: HardGates,
    limits: CycleLimits,
    local_recovery: LocalRecoveryPolicy,
) -> Result<CycleOutcome> {
    let initial_sites = mesh.active_site_count();
    // Freeze the input mesh's representative background scale. Recomputing it
    // from quality moves would make the optimiser chase its own output, while
    // a maximum lets one oversized cell set the target for the whole sphere.
    let background_scale_m = median_cell_scale(mesh.state());
    let mut attempted = 0usize;
    let mut committed = 0usize;
    let mut rolled_back = 0usize;
    let mut balanced = 0usize;
    let mut fallback_committed = 0usize;
    let mut refusals = RejectionTally::default();
    let mut relieved = 0usize;
    // Read-only stall-escalation audit. Diagnosis
    // `.omx/plans/harp-dv-stall-escalation-diagnosis.md` predicts that on a
    // production-scale mesh the multi-ring recovery never runs, because its
    // trigger is a global scalar -- "nothing was accepted anywhere this cycle"
    // -- while the stalls it exists to clear are local.
    let mut escalation_eligible_cycles = 0usize;
    let mut escalation_fired_cycles = 0usize;
    let mut local_escalation_cycles = 0usize;
    let mut local_escalation_seeds = 0usize;
    let mut consecutive_degree_blocked: BTreeMap<usize, u32> = BTreeMap::new();
    let mut degree_blocked_total = 0usize;
    let mut tier1_single_moves = 0usize;
    let mut tier1_pair_moves = 0usize;
    let mut r_adapted = 0usize;
    let mut pair_adapted = 0usize;
    let mut multi_ring_adapted = 0usize;
    let mut unresolved_cells = Vec::new();
    let mut quality_constrained_cells = BTreeSet::new();
    let mut cycles = 0u32;
    let mut stop_reason = StopReason::MaximumCyclesReached;
    let mut adaptation_probe = None;
    // The cells that asked and were refused last cycle. A demand refused
    // twice running is the one whose neighbourhood the ladder cannot serve;
    // a demand refused once is usually served next cycle by the geometry its
    // own neighbours just gained. Offering the move phase only the first kind
    // is what keeps the extra work proportional to the residue rather than to
    // every transient refusal -- measured, the difference is 66% of the run.
    let mut stalled_last_cycle: BTreeSet<usize> = BTreeSet::new();

    while cycles < limits.max_cycles {
        unresolved_cells.clear();
        quality_constrained_cells.clear();
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
        let mut stalled_demand_sites = BTreeSet::new();
        let mut stalled_demands = Vec::new();
        let mut retry_demands = Vec::new();
        let mut unattempted_cells = Vec::new();
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
                    if collect_blockers(
                        site,
                        for_balance,
                        &reasons,
                        &mut degree_blocked_sites,
                        &mut pentagon_blocked_pairs,
                        &mut balance_blocked_sites,
                        &mut stalled_demand_sites,
                    ) {
                        quality_constrained_cells.insert(site);
                    }
                    tally_refusals(&reasons, &mut refusals);
                    unresolved_cells.push(site);
                    stalled_demands.push((site, witness, for_balance, reasons));
                }
                DemandOutcome::NotAttempted(_) => {
                    unresolved_cells.push(site);
                    unattempted_cells.push(site);
                }
            }
        }
        // A productive cycle keeps the short candidate ladder. If no insertion
        // survived, retry only its unresolved demands with the broader stalled
        // ladder before spending another cycle on the same geometry.
        //
        // Tried and rejected: offering the broader ladder to every demand that
        // was also refused last cycle, regardless of what the rest of the globe
        // managed. It is the right shape -- one neighbourhood's stall is not
        // evidence about another's -- but the early cycles have thousands of
        // demands that stall twice and then resolve on their own, and giving
        // each of them the twenty-candidate ladder took the crate's own test
        // suite from 32 seconds past 30 minutes without finishing. Bounding it
        // by persistence is not enough; it would need a bound on how much
        // broadening a cycle may buy.
        if accepted_this_cycle == 0 && !stalled_demands.is_empty() && mesh.segments_are_empty() {
            unresolved_cells.clear();
            unresolved_cells.extend(unattempted_cells);
            quality_constrained_cells.clear();
            degree_blocked_sites.clear();
            pentagon_blocked_pairs.clear();
            balance_blocked_sites.clear();
            stalled_demand_sites.clear();
            for (site, witness, for_balance, mut reasons) in stalled_demands {
                if mesh.active_site_count() >= limits.max_sites {
                    out_of_budget = true;
                    break;
                }
                attempted += 1;
                match mesh.refine_cell_fallback(site, policy, gates)? {
                    DemandOutcome::Resolved { .. } => {
                        committed += 1;
                        accepted_this_cycle += 1;
                        fallback_committed += 1;
                        if for_balance {
                            balanced += 1;
                        }
                    }
                    DemandOutcome::Unresolved {
                        refusals: fallback, ..
                    } => {
                        rolled_back += fallback.len();
                        tally_refusals(&fallback, &mut refusals);
                        reasons.extend(fallback);
                        if collect_blockers(
                            site,
                            for_balance,
                            &reasons,
                            &mut degree_blocked_sites,
                            &mut pentagon_blocked_pairs,
                            &mut balance_blocked_sites,
                            &mut stalled_demand_sites,
                        ) {
                            quality_constrained_cells.insert(site);
                        }
                        unresolved_cells.push(site);
                        retry_demands.push((site, witness, for_balance));
                    }
                    DemandOutcome::NotAttempted(_) => {
                        if collect_blockers(
                            site,
                            for_balance,
                            &reasons,
                            &mut degree_blocked_sites,
                            &mut pentagon_blocked_pairs,
                            &mut balance_blocked_sites,
                            &mut stalled_demand_sites,
                        ) {
                            quality_constrained_cells.insert(site);
                        }
                        unresolved_cells.push(site);
                        retry_demands.push((site, witness, for_balance));
                    }
                }
            }
        }
        let use_pair_relief = accepted_this_cycle == 0;
        // Ruppert's protected-segment path has its own termination invariant:
        // accepted sites and split segments stay where the proof put them.
        // Moving even an unmarked interior site afterwards invalidates that
        // invariant, so r-adaptation is confined to unconstrained runs.
        if !mesh.segments_are_empty() {
            degree_blocked_sites.clear();
            pentagon_blocked_pairs.clear();
            balance_blocked_sites.clear();
            stalled_demand_sites.clear();
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
        // A cell whose demand the ladder has now refused twice running,
        // offered the same move the balance phase already runs. Nothing new is
        // proposed and nothing new is accepted -- the objective, the
        // destinations and the gates are the ones already here; the only
        // change is which sites reach them.
        let persistently_stalled: BTreeSet<usize> = stalled_demand_sites
            .intersection(&stalled_last_cycle)
            .copied()
            .collect();
        stalled_last_cycle = std::mem::take(&mut stalled_demand_sites);
        balance_blocked_sites.extend(persistently_stalled.iter().copied());
        if !degree_blocked_sites.is_empty() {
            escalation_eligible_cycles += 1;
        }
        degree_blocked_total += degree_blocked_sites.len();
        // How long each site has been blocked, counting this cycle. A site that
        // clears drops out, so the count is consecutive by construction.
        consecutive_degree_blocked = degree_blocked_sites
            .iter()
            .map(|&site| {
                (
                    site,
                    consecutive_degree_blocked.get(&site).copied().unwrap_or(0) + 1,
                )
            })
            .collect();
        for &blocked_site in &degree_blocked_sites {
            if !mesh.can_move_site(blocked_site)
                || mesh.state().vertex_degree(blocked_site).ok() < Some(gates.max_vertex_degree)
            {
                continue;
            }
            // This phase removes one hard writer blocker. Scale has its own
            // phase below; scoring it here was expensive and admitted moves
            // that balanced a neighbourhood without lowering this degree.
            let objective =
                |state: &MeshState, _: &AffectedSites| state.vertex_degree(blocked_site).ok();
            let Ok(before) = mesh.state().vertex_degree(blocked_site) else {
                continue;
            };
            let mut moved = false;
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
                    tier1_single_moves += 1;
                    r_adapted += 1;
                    adapted_this_cycle += 1;
                    moved = true;
                    break;
                }
            }
            if moved || !use_pair_relief {
                continue;
            }
            for (neighbour, first, second) in degree_relief_pairs(mesh.state(), blocked_site) {
                if !mesh.can_move_site(neighbour) {
                    continue;
                }
                if let Acceptance::Committed(_) = mesh.propose_pair_move_cached(
                    (blocked_site, first),
                    (neighbour, second),
                    gates,
                    &objective,
                    Some(&before),
                )? {
                    tier1_pair_moves += 1;
                    relieved += 1;
                    r_adapted += 2;
                    pair_adapted += 2;
                    adapted_this_cycle += 2;
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
                let objective = |state: &MeshState, _: &AffectedSites| {
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
                let Some(before) = objective(mesh.state(), &AffectedSites::new()) else {
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
        for &site in &balance_blocked_sites {
            if !mesh.can_move_site(site) {
                continue;
            }
            let objective = |state: &MeshState, affected: &AffectedSites| {
                balance_objective(state, affected, limits.max_neighbour_scale_ratio)
            };
            let Some(before) = mesh.score_before_move(site, &objective) else {
                continue;
            };
            if before[0] == 0.0
                && !pentagon_sites.contains(&site)
                && !persistently_stalled.contains(&site)
            {
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
        // Where every ladder and every single-site phase has stopped, widen
        // what may move rather than what may be inserted.
        //
        // Only on a cycle that placed nothing anywhere: this reads and rescores
        // a few rings per stalled region, which is worth doing when the
        // alternative is another identical cycle and is not worth doing while
        // the ordinary ladder is still productive.
        if accepted_this_cycle == 0 && mesh.segments_are_empty() {
            escalation_fired_cycles += 1;
            let mut recovery_seeds = persistently_stalled.clone();
            recovery_seeds.extend(degree_blocked_sites.iter().copied());
            recovery_seeds.extend(balance_blocked_sites.iter().copied());
            let mut recovered = recover_stalled_regions(
                mesh,
                criteria,
                gates,
                limits,
                &recovery_seeds,
                RECOVERY_MOVABLE_RINGS,
            )?;
            if recovered == 0 {
                // One escalation, once: a ring further out, and the guard ring
                // moves out with it. Unbounded widening is how a local repair
                // turns into the global relaxation that was measured to take
                // the worst angle from 25 degrees to 10.
                recovered = recover_stalled_regions(
                    mesh,
                    criteria,
                    gates,
                    limits,
                    &recovery_seeds,
                    RECOVERY_MOVABLE_RINGS + 1,
                )?;
            }
            r_adapted += recovered;
            adapted_this_cycle += recovered;
            multi_ring_adapted += recovered;
        } else if local_recovery.maximum_seeds_per_cycle > 0 && mesh.segments_are_empty() {
            // The same widening, offered to the sites that have been blocked
            // longest rather than to a globally idle cycle. Bounded twice: a
            // site must have persisted, and a cycle may seed only so many.
            let mut ranked: Vec<(u32, usize)> = consecutive_degree_blocked
                .iter()
                .filter(|(_, &cycles)| cycles >= local_recovery.minimum_consecutive_cycles)
                .map(|(&site, &cycles)| (cycles, site))
                .collect();
            // Longest-blocked first, then by site so the choice is the same on
            // every run.
            ranked.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
            let seeds: BTreeSet<usize> = ranked
                .into_iter()
                .take(local_recovery.maximum_seeds_per_cycle)
                .map(|(_, site)| site)
                .collect();
            if !seeds.is_empty() {
                local_escalation_cycles += 1;
                local_escalation_seeds += seeds.len();
                let recovered = recover_stalled_regions(
                    mesh,
                    criteria,
                    gates,
                    limits,
                    &seeds,
                    RECOVERY_MOVABLE_RINGS,
                )?;
                r_adapted += recovered;
                adapted_this_cycle += recovered;
                multi_ring_adapted += recovered;
            }
        }
        // Site motion is only useful if the demands it was meant to unlock are
        // retried against the moved geometry. Do that once as a batch, for
        // every stalled cycle -- not as a last-cycle exception and not once per
        // move. The latter was measured slower and harmed balance (§11.54).
        if accepted_this_cycle == 0
            && adapted_this_cycle > 0
            && !retry_demands.is_empty()
            && mesh.segments_are_empty()
        {
            unresolved_cells.clear();
            quality_constrained_cells.clear();
            for (site, witness, for_balance) in retry_demands {
                if mesh.active_site_count() >= limits.max_sites {
                    out_of_budget = true;
                    break;
                }
                attempted += 1;
                let mut reasons = match mesh.refine_cell(site, witness, policy, gates)? {
                    DemandOutcome::Resolved { .. } => {
                        committed += 1;
                        accepted_this_cycle += 1;
                        if for_balance {
                            balanced += 1;
                        }
                        continue;
                    }
                    DemandOutcome::Unresolved { refusals: ordinary } => {
                        rolled_back += ordinary.len();
                        tally_refusals(&ordinary, &mut refusals);
                        ordinary
                    }
                    DemandOutcome::NotAttempted(_) => {
                        unresolved_cells.push(site);
                        continue;
                    }
                };
                attempted += 1;
                match mesh.refine_cell_fallback(site, policy, gates)? {
                    DemandOutcome::Resolved { .. } => {
                        committed += 1;
                        accepted_this_cycle += 1;
                        fallback_committed += 1;
                        if for_balance {
                            balanced += 1;
                        }
                    }
                    DemandOutcome::Unresolved { refusals: fallback } => {
                        rolled_back += fallback.len();
                        tally_refusals(&fallback, &mut refusals);
                        reasons.extend(fallback);
                        if !reasons.is_empty()
                            && reasons.iter().all(|(_, reason)| {
                                matches!(reason, Rejection::SliverTriangle { .. })
                            })
                        {
                            quality_constrained_cells.insert(site);
                        }
                        unresolved_cells.push(site);
                    }
                    DemandOutcome::NotAttempted(_) => unresolved_cells.push(site),
                }
            }
        }
        cycles += 1;
        mesh.record_cycle_completed();
        eprintln!(
            "harp_dv cycle {cycles}/{}: {} insertions, {} r-adaptations, {} unresolved ({} \
             angle-constrained), {} active cells",
            limits.max_cycles,
            accepted_this_cycle,
            adapted_this_cycle,
            unresolved_cells.len(),
            quality_constrained_cells.len(),
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
                stop_reason = if quality_constrained_cells.len() == unresolved_cells.len()
                    && !quality_constrained_cells.is_empty()
                {
                    StopReason::QualityConstraintReached
                } else {
                    StopReason::NoAcceptedTransactions
                };
                break;
            }
            let signature = (
                physical_demand_count,
                balance_demand_count,
                unresolved_cells.len(),
                quality_constrained_cells.len(),
            );
            if adaptation_probe.is_some_and(|before: (usize, usize, usize, usize)| {
                signature.0 >= before.0
                    && signature.1 >= before.1
                    && signature.2 >= before.2
                    && signature.3 >= before.3
            }) {
                stop_reason = StopReason::NoProductiveAdaptation;
                break;
            }
            adaptation_probe = Some(signature);
        } else {
            adaptation_probe = None;
        }
    }

    // Read-only: what the stall-escalation diagnosis predicts. `fired` counts
    // cycles that reached `recover_stalled_regions`; `eligible` counts cycles
    // that had a degree-blocked site waiting for it.
    eprintln!(
        "harp_dv stall escalation audit: {cycles} cycle(s), escalation eligible in \
         {escalation_eligible_cycles}, fired in {escalation_fired_cycles}; \
         degree-blocked site-cycles {degree_blocked_total}; \
         tier-1 relief {tier1_single_moves} single / {tier1_pair_moves} pair move(s); \
         local escalation in {local_escalation_cycles} cycle(s) over \
         {local_escalation_seeds} seed(s)"
    );

    // HARP owns its final geometry.  Improve the preferred angle window with
    // the same transactional moves and hard gates used by its refinement;
    // downstream Method-C repair is neither needed nor allowed here.
    #[cfg(test)]
    if capture_quality_checkpoint(mesh) {
        // Checkpoint-only tests inspect the saved clone. Running optimisation
        // and leaf-retirement audits on the throwaway working mesh is pure
        // cost, and becomes dominant on large NXP fixtures.
        return Ok(CycleOutcome {
            report: HarpDvRunReport::empty(mesh.active_site_count(), stop_reason),
            unresolved_cells: Vec::new(),
        });
    }
    let (quality_optimiser_moves, target_angle_window) =
        optimise_mesh_quality(mesh, criteria, gates, limits, background_scale_m)?;
    r_adapted += quality_optimiser_moves;

    // Retirement changes both topology and cell geometry. Run it before the
    // final demand evaluation so every reported residual describes the mesh
    // that is actually returned rather than its pre-retirement predecessor.
    let pre_retirement_leaves = leaf_lineage_survey(mesh, criteria);
    let maximum_retirement_degree = if std::env::var_os("EARTHMESH_HARP_LEAF_RETIREMENT").is_some()
    {
        7
    } else {
        4
    };
    let (committed_retirements, committed_d4_retirements) = retire_quality_leaf_sites(
        mesh,
        criteria,
        gates,
        limits,
        &pre_retirement_leaves,
        maximum_retirement_degree,
    );

    // Re-read after the last insertion or move. The loop's attempted-demand
    // list predates its r-adaptation phase, so returning it here would make the
    // final count stale by construction whenever the last cycle moved sites.
    let (final_demands, final_tally, final_scales) = evaluate(mesh, criteria, limits)?;
    let final_balance = balance_demands(mesh, &final_scales, limits);
    let physical_demands_remaining = final_demands.len();
    let balance_demands_remaining = final_balance.len();
    let pending: BTreeSet<usize> = final_demands
        .iter()
        .chain(final_balance.iter())
        .map(|demand| demand.cell as usize)
        .collect();
    quality_constrained_cells.retain(|site| pending.contains(site));
    unresolved_cells = pending.into_iter().collect();
    if unresolved_cells.is_empty()
        && matches!(
            stop_reason,
            StopReason::MaximumCyclesReached
                | StopReason::NoAcceptedTransactions
                | StopReason::NoProductiveAdaptation
                | StopReason::QualityConstraintReached
        )
    {
        stop_reason = if final_tally.at_minimum_scale > 0 {
            StopReason::MinimumScaleReached
        } else if final_tally.unsatisfiable > 0 {
            StopReason::SourceResolutionReached
        } else {
            StopReason::AllSatisfied
        };
    }

    // Counted at the end rather than tracked through: it is what a caller has
    // to decide on, and a number carried through the loop would be the one
    // from whichever cycle last looked.
    let unbalanced_pairs_remaining =
        balance_survey_from_scales(mesh.state(), &final_scales, limits).0;

    let angle_window = angle_window_survey(mesh.state());
    let triangle_eta = all_triangle_eta_values(mesh.state()).ok_or_else(|| {
        crate::error::HarpDvError::TopologyViolation(
            "final triangle area-length quality is not measurable".to_string(),
        )
    })?;
    let triangle_eta_min = triangle_eta.first().copied().unwrap_or(0.0);
    let triangle_eta_p1 = triangle_eta
        .get((triangle_eta.len() / 100).min(triangle_eta.len().saturating_sub(1)))
        .copied()
        .unwrap_or(0.0);
    let triangles_below_eta_0_89 = triangle_eta.partition_point(|value| *value < 0.89);
    let leaf_lineage = leaf_lineage_survey(mesh, criteria);
    let d4_retirement = if std::env::var_os("EARTHMESH_HARP_D4_RETIREMENT_AUDIT").is_some()
        && mesh.segments_are_empty()
    {
        degree_four_retirement_audit(mesh, criteria, gates, limits, &leaf_lineage)
    } else {
        DegreeFourRetirementAudit::default()
    };
    let d4_retirement = DegreeFourRetirementAudit {
        committed: committed_d4_retirements,
        ..d4_retirement
    };
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
            fallback_transactions_committed: fallback_committed,
            refusals,
            degree_relieving_moves: relieved,
            r_adaptation_moves: r_adapted,
            paired_r_adaptation_moves: pair_adapted,
            multi_ring_r_adaptation_moves: multi_ring_adapted,
            angles_below_40_deg: angle_window.below,
            angles_in_40_90_deg: angle_window.inside_40_90,
            angles_above_90_deg: angle_window.above_90,
            angles_in_40_80_deg: angle_window.inside_40_80,
            angles_above_80_deg: angle_window.above_80,
            angle_min_deg: angle_window.min_deg,
            angle_max_deg: angle_window.max_deg,
            angle_window_40_80_verdict: if angle_window.below == 0
                && angle_window.above_80 == 0
                && angle_window.unmeasurable == 0
            {
                AngleWindowVerdict::Pass
            } else {
                AngleWindowVerdict::Fail
            },
            angle_window_unmeasurable_triangles: angle_window.unmeasurable,
            vertices_below_degree_5: vertices_below_degree_5(mesh.state()),
            active_adaptive_sites: leaf_lineage.active_adaptive_sites,
            active_leaf_sites: leaf_lineage.active_leaf_sites,
            interior_leaf_sites: leaf_lineage.interior_leaf_sites,
            lineage_unknown_adaptive_sites: leaf_lineage.lineage_unknown_adaptive_sites,
            leaf_degree_4: leaf_lineage.leaf_degree_4,
            leaf_degree_5: leaf_lineage.leaf_degree_5,
            leaf_degree_6: leaf_lineage.leaf_degree_6,
            leaf_degree_7: leaf_lineage.leaf_degree_7,
            leaf_degree_other: leaf_lineage.leaf_degree_other,
            leaf_birth_cycle_min: leaf_lineage.leaf_birth_cycle_min,
            leaf_birth_cycle_max: leaf_lineage.leaf_birth_cycle_max,
            leaf_target_scale_measured: leaf_lineage.leaf_target_scale_measured,
            leaf_target_scale_min_m: leaf_lineage.leaf_target_scale_min_m,
            leaf_target_scale_max_m: leaf_lineage.leaf_target_scale_max_m,
            angles_below_40_at_leaf_vertices: leaf_lineage.angles_below_40_at_leaf_vertices,
            angles_above_80_at_leaf_vertices: leaf_lineage.angles_above_80_at_leaf_vertices,
            angles_below_40_at_interior_leaf_vertices: leaf_lineage
                .angles_below_40_at_interior_leaf_vertices,
            angles_above_80_at_interior_leaf_vertices: leaf_lineage
                .angles_above_80_at_interior_leaf_vertices,
            violating_triangles_touching_leaf: leaf_lineage.violating_triangles_touching_leaf,
            violating_triangles_touching_interior_leaf: leaf_lineage
                .violating_triangles_touching_interior_leaf,
            d4_leaf_retirement_candidates: d4_retirement.candidates,
            d4_leaf_retirement_triangulations: d4_retirement.triangulations,
            d4_leaf_retirement_hard_gate_safe: d4_retirement.hard_gate_safe,
            d4_leaf_retirement_physical_safe: d4_retirement.physical_safe,
            d4_leaf_retirement_balance_safe: d4_retirement.balance_safe,
            d4_leaf_retirement_quality_improving: d4_retirement.quality_improving,
            d4_leaf_retirement_fully_acceptable: d4_retirement.fully_acceptable,
            d4_leaf_retirement_committed: d4_retirement.committed,
            quality_leaf_retirement_committed: committed_retirements,
            target_triangle_angles_below_40_deg: target_angle_window.below,
            target_triangle_angles_above_80_deg: target_angle_window.above_80,
            target_triangle_angle_count: target_angle_window.count,
            target_triangle_angle_min_deg: target_angle_window.min_deg,
            target_triangle_angle_max_deg: target_angle_window.max_deg,
            angle_window_penalty: angle_window.legacy_penalty,
            angle_window_40_80_penalty: angle_window.penalty,
            quality_optimiser_moves,
            triangle_eta_min,
            triangle_eta_p1,
            triangles_below_eta_0_89,
            unbalanced_pairs_remaining,
            unresolved_count: unresolved_cells.len(),
            physical_demands_remaining,
            balance_demands_remaining,
            quality_constrained_count: quality_constrained_cells.len(),
            deterministic: true,
        },
        unresolved_cells,
    })
}

/// Prototype: frontal point placement under demand-driven scheduling.
#[cfg(test)]
mod frontal_prototype;

#[cfg(test)]
mod tests;
