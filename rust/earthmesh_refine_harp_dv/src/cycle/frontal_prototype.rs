//! Prototype: does frontal point placement help, under demand-driven
//! scheduling?
//!
//! Test-only. Nothing here is reachable from `run_cycles`.
//!
//! # Naming
//!
//! What this builds is an **Eq. 6-7-corrected fan-wide frontal candidate**,
//! not the paper's Frontal-Delaunay. Five things differ, all of them forced by
//! HARP's contract rather than chosen:
//!
//! 1. candidates are generated for the whole fan of a demanded site, where the
//!    paper refines the globally worst bad triangle;
//! 2. frontal candidates are offered ahead of the shipped ladder at a fixed
//!    position, where the paper has no other ladder;
//! 3. the size field is the un-gradient-limited regional criterion, because the
//!    limited one does not exist yet during refinement;
//! 4. lengths are spherical arc lengths, where the paper uses local Euclidean
//!    lengths on the restricted surface;
//! 5. when only one of the two new-edge midpoints has a target value, that one
//!    alone sets the spacing.
//!
//! # What is borrowed, and from where
//!
//! The off-centre cascade of Engwirda 2017 (GMD 10:2117-2140, CC-BY),
//! Algorithm 1 and section 3.6. Implemented from the paper; the JIGSAW sources
//! were not read. For a bad triangle `f`:
//!
//! * the **shortest** edge `e0` is the frontal segment -- not the longest;
//! * both candidates lie on the Voronoi arc of `e0`, the geodesic through the
//!   edge midpoint and the triangle circumcentre;
//! * the size-optimal point sits at altitude `a_h` chosen so the two new edges
//!   reach the target spacing, and the shape-optimal point at
//!   `a_theta = |e0| / (2 tan(theta/2))` with `theta = asin(1/(2 rho_bar))`,
//!   which makes the new apex angle exactly `theta`;
//! * the winner is whichever of the two is **closer** to the midpoint while
//!   still at least `|e0|/2` from it, and the circumcentre is the fallback.
//!
//! `rho_bar` is the one parameter that points straight at the window: the
//! specification's lower bound is `rho <= 0.77786`, which is
//! `1 / (2 sin 40 deg)`, so asking the placement for `theta = 40` degrees and
//! asking the mesh for a 40 degree floor are the same request.
//!
//! # What is *not* borrowed, and why the result is not attributable
//!
//! The paper's quality is an emergent property of three things together: the
//! greedy priority schedule over the front, the frontal filtering, and the
//! point placement. HARP must serve physical demands first, so only the third
//! can be taken. What this measures is therefore a hybrid neither paper
//! describes, and a negative result rules out *frontal placement under
//! demand-driven scheduling*, not Frontal-Delaunay as published.
//!
//! # Size field
//!
//! Generation-stage only: `CellCriterion::target_scale_m_at`, which is the
//! unlimited criterion value. The gradient-limited field does not exist yet at
//! this point in the run -- `target_cell_scales` is built after refinement
//! finishes -- so the prototype cannot use it, and its numbers are not directly
//! comparable to an experiment that does. See
//! `docs/angle_window_40_80_experiment_spec.md` section 1.

use super::*;

use crate::candidate::{candidates_for_site, CandidatePolicy};

/// The radius-edge bound for a requested floor angle.
///
/// `rho_bar = 1 / (2 sin theta)`, so asking the placement for `theta` degrees
/// and asking the mesh for a `theta` degree floor are the same request. Forty
/// degrees gives `0.77786`, the number the specification writes as the
/// per-triangle bound.
pub(super) fn rho_bar_for(floor_angle_deg: f64) -> f64 {
    1.0 / (2.0 * floor_angle_deg.to_radians().sin())
}

/// One predictor-corrector pass for the size-optimal altitude, as the paper
/// prescribes for non-uniform spacing.
const SIZE_OPTIMAL_CORRECTIONS: usize = 2;

/// How a demand picks among the candidates it was offered.
///
/// The two arms of the experiment matrix. Keeping both here, rather than in two
/// drivers, is what makes the comparison single-variable: everything outside
/// this choice is shared code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Selection {
    /// Production: hard gates only, first survivor in ladder order wins.
    FirstSurvivor,
    /// Clone each candidate, keep those whose local cavity does not regress
    /// under `Better`, and rank the survivors by leximin on the sorted vector.
    BetterLeximin,
}

/// The target cell scale at a point, as the generation stage can see it.
///
/// Minimum over the criteria that answer there, floored by the run's minimum
/// cell width. `None` where no criterion applies: there is no size constraint
/// to satisfy, so only the shape-optimal point is meaningful.
fn target_scale_at(
    criteria: &[Box<dyn CellCriterion>],
    point: CartesianPoint,
    radius_m: f64,
    minimum_cell_width_m: f64,
) -> Option<f64> {
    let lonlat = xyz_to_lonlat_degrees(point);
    criteria
        .iter()
        .filter_map(|criterion| criterion.target_scale_m_at(lonlat, radius_m))
        .filter(|value| value.is_finite() && *value > 0.0)
        .min_by(f64::total_cmp)
        .map(|value| value.max(minimum_cell_width_m))
}

/// Walk `distance` along the sphere from `here` in unit tangent direction.
fn geodesic_step(
    here: CartesianPoint,
    unit: CartesianPoint,
    distance: f64,
) -> Option<CartesianPoint> {
    let radius = magnitude(here);
    if !radius.is_finite() || radius <= 0.0 || !distance.is_finite() {
        return None;
    }
    let (sin, cos) = (distance / radius).sin_cos();
    let point = CartesianPoint::new(
        here.x * cos + unit.x * radius * sin,
        here.y * cos + unit.y * radius * sin,
        here.z * cos + unit.z * radius * sin,
    );
    point.x.is_finite().then_some(point)
}

/// The unit tangent at `here` along the geodesic towards `there`.
fn tangent_towards(here: CartesianPoint, there: CartesianPoint) -> Option<CartesianPoint> {
    let radius = magnitude(here);
    if !radius.is_finite() || radius <= 0.0 {
        return None;
    }
    let normal = CartesianPoint::new(here.x / radius, here.y / radius, here.z / radius);
    let dot = normal.x * there.x + normal.y * there.y + normal.z * there.z;
    let tangent = CartesianPoint::new(
        there.x - normal.x * dot,
        there.y - normal.y * dot,
        there.z - normal.z * dot,
    );
    let length = magnitude(tangent);
    (length.is_finite() && length > 0.0)
        .then(|| CartesianPoint::new(tangent.x / length, tangent.y / length, tangent.z / length))
}

/// Which arm of the cascade produced a point.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct FrontalStats {
    pub(super) committed: usize,
    /// The size-optimal altitude was the closer of the two.
    pub(super) size_chosen: usize,
    /// The shape-optimal altitude was the closer of the two.
    pub(super) shape_chosen: usize,
    /// Of those, the ones whose altitude exceeded the Voronoi segment and were
    /// clamped to the circumcentre. A clamped shape point is the circumcentre
    /// whatever `theta_bar` says, so only `shape_chosen - shape_clamped` are
    /// placements `theta_bar` can actually move.
    pub(super) shape_clamped: usize,
    /// Neither cleared the half-edge floor; the circumcentre was used.
    pub(super) fell_back: usize,
}

/// Engwirda's off-centre point for one triangle, and which arm produced it.
pub(super) fn frontal_offcentre(
    state: &MeshState,
    criteria: &[Box<dyn CellCriterion>],
    minimum_cell_width_m: f64,
    rho_bar: f64,
    triangle: usize,
    stats: &mut FrontalStats,
) -> Option<CartesianPoint> {
    let corners = state.triangles()[triangle];
    let radius_m = state.sphere_radius();

    // The frontal segment is the *shortest* edge. Advancing from the short side
    // is what makes the new element well shaped; the longest edge is where the
    // shipped ladder looks, and it is a different rule.
    let (left, right) = (0..3)
        .map(|corner| (corners[(corner + 1) % 3], corners[(corner + 2) % 3]))
        .min_by(|&(a, b), &(c, d)| {
            arc_length_unit_sphere(state.vertices()[a], state.vertices()[b])
                .total_cmp(&arc_length_unit_sphere(
                    state.vertices()[c],
                    state.vertices()[d],
                ))
                .then_with(|| (a.min(b), a.max(b)).cmp(&(c.min(d), c.max(d))))
        })?;
    let (a, b) = (state.vertices()[left], state.vertices()[right]);
    let edge = arc_length_unit_sphere(a, b);
    if !edge.is_finite() || edge <= 0.0 {
        return None;
    }

    let sum = CartesianPoint::new(a.x + b.x, a.y + b.y, a.z + b.z);
    let scale = magnitude(sum);
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let midpoint = CartesianPoint::new(
        sum.x / scale * radius_m,
        sum.y / scale * radius_m,
        sum.z / scale * radius_m,
    );

    // The Voronoi arc of the frontal edge runs through the midpoint and the
    // circumcentre; the admissible altitudes are the half-edge at one end and
    // the circumcentre at the other.
    let circumcentre = state.circumcentre(triangle).ok()?;
    let ceiling = arc_length_unit_sphere(midpoint, circumcentre);
    let floor = 0.5 * edge;
    if !ceiling.is_finite() || ceiling < floor {
        stats.fell_back += 1;
        return Some(circumcentre);
    }
    let direction = tangent_towards(midpoint, circumcentre)?;

    // Shape-optimal: the altitude that makes the new apex angle exactly theta.
    let theta = (1.0 / (2.0 * rho_bar)).clamp(-1.0, 1.0).asin();
    let shape = (2.0 * (0.5 * theta).tan() > 0.0).then(|| edge / (2.0 * (0.5 * theta).tan()));

    // Size-optimal, Engwirda 2017 Eq. 6-7: the target is sampled at the
    // midpoints of the two edges the insertion would create, averaged, and the
    // altitude is capped at the equilateral height for that spacing. The cap
    // binds whenever the target is longer than the frontal edge, which is the
    // usual case when refining a cell that is too coarse; without it the new
    // triangle is elongated rather than equilateral.
    //
    // The target depends on where the apex lands and the apex depends on the
    // target, so the paper solves it by predictor-corrector. The predictor
    // samples at the frontal edge midpoint; each corrector re-samples at the
    // *new edge* midpoints the previous altitude implies.
    let mut wanted = target_scale_at(criteria, midpoint, radius_m, minimum_cell_width_m)
        .map(|target| CELL_SCALE_TO_EDGE_LENGTH * target);
    let mut size = None;
    for _ in 0..SIZE_OPTIMAL_CORRECTIONS {
        let Some(spacing) = wanted else {
            break;
        };
        let squared = spacing * spacing - floor * floor;
        if squared <= 0.0 {
            size = None;
            break;
        }
        let altitude = squared.sqrt().min(0.5 * 3.0_f64.sqrt() * spacing);
        size = Some(altitude);
        let Some(apex) = geodesic_step(midpoint, direction, altitude.min(ceiling)) else {
            break;
        };
        // Re-sample where the paper says to: the midpoints of the two edges
        // this apex would create with the frontal edge's endpoints.
        let mut next = Vec::with_capacity(2);
        for end in [a, b] {
            let sum = CartesianPoint::new(apex.x + end.x, apex.y + end.y, apex.z + end.z);
            let scale = magnitude(sum);
            if !scale.is_finite() || scale <= 0.0 {
                continue;
            }
            let new_midpoint = CartesianPoint::new(
                sum.x / scale * radius_m,
                sum.y / scale * radius_m,
                sum.z / scale * radius_m,
            );
            if let Some(target) =
                target_scale_at(criteria, new_midpoint, radius_m, minimum_cell_width_m)
            {
                next.push(CELL_SCALE_TO_EDGE_LENGTH * target);
            }
        }
        if next.is_empty() {
            break;
        }
        wanted = Some(next.iter().sum::<f64>() / next.len() as f64);
    }

    // Closest admissible of the two, circumcentre otherwise. Which arm wins is
    // counted, because "theta_bar is inert" is only a claim about the final
    // numbers until the shape arm is shown never to be selected.
    let admissible =
        |altitude: Option<f64>| altitude.filter(|value| value.is_finite() && *value >= floor);
    let (size, shape) = (admissible(size), admissible(shape));
    match (size, shape) {
        (Some(size_altitude), Some(shape_altitude)) => {
            if size_altitude <= shape_altitude {
                stats.size_chosen += 1;
            } else {
                stats.shape_chosen += 1;
                stats.shape_clamped += usize::from(shape_altitude >= ceiling);
            }
            geodesic_step(
                midpoint,
                direction,
                size_altitude.min(shape_altitude).min(ceiling),
            )
        }
        (Some(altitude), None) => {
            stats.size_chosen += 1;
            geodesic_step(midpoint, direction, altitude.min(ceiling))
        }
        (None, Some(altitude)) => {
            stats.shape_chosen += 1;
            stats.shape_clamped += usize::from(altitude >= ceiling);
            geodesic_step(midpoint, direction, altitude.min(ceiling))
        }
        (None, None) => {
            stats.fell_back += 1;
            Some(circumcentre)
        }
    }
}

/// Refine to a stall, under one cell of the experiment matrix.
///
/// # Single variable
///
/// With `use_frontal = false` and `Selection::FirstSurvivor` this is exactly
/// the shipped loop -- the same `refine_cell` and `refine_cell_fallback`, in
/// the same order -- so that arm reproduces the production figures by
/// construction rather than by resemblance, and the test asserts it.
///
/// The two axes are independent: which candidates exist, and how one is chosen
/// among those that survive the hard gates. An earlier version moved both at
/// once and could not attribute its own result.
///
/// Fan triangles are offered worst-radius-edge first, the local reading of the
/// paper's "refine the element with the worst ratio at each iteration".
pub(super) fn refine_with_frontal(
    mesh: &mut AdaptiveMesh,
    criteria: &[Box<dyn CellCriterion>],
    gates: HardGates,
    limits: CycleLimits,
    use_frontal: bool,
    selection: Selection,
    rho_bar: f64,
) -> FrontalStats {
    let policy = CandidatePolicy::default();
    let mut stats = FrontalStats::default();
    for _ in 0..limits.max_cycles {
        let (mut demands, _, scales) = evaluate(mesh, criteria, limits).expect("evaluate");
        demands.extend(balance_demands(mesh, &scales, limits));
        if demands.is_empty() {
            break;
        }
        order_demands(&mut demands);
        let mut accepted = 0usize;
        for demand in &demands {
            let site = demand.cell as usize;
            let frontal = if use_frontal {
                frontal_points(
                    mesh.state(),
                    criteria,
                    limits.minimum_cell_width_m,
                    rho_bar,
                    site,
                    &mut stats,
                )
            } else {
                Vec::new()
            };

            let mut served = match selection {
                Selection::FirstSurvivor => frontal.iter().any(|&(point, hint)| {
                    matches!(
                        mesh.propose_site_for(point, Some(hint), gates, site),
                        Ok(Acceptance::Committed(_))
                    )
                }),
                Selection::BetterLeximin => {
                    let mut offered: Vec<(CartesianPoint, usize)> = frontal;
                    if let Ok(ladder) = candidates_for_site(mesh.state(), site, None, policy) {
                        offered.extend(
                            ladder
                                .into_iter()
                                .map(|candidate| (candidate.point, candidate.hint)),
                        );
                    }
                    let mut ranked: Vec<(Vec<f64>, usize, CartesianPoint, usize)> = offered
                        .into_iter()
                        .enumerate()
                        .filter_map(|(order, (point, hint))| {
                            let after = surviving_cavity_quality(mesh, point, hint, gates)?;
                            Some((after, order, point, hint))
                        })
                        .collect();
                    ranked.sort_by(|left, right| {
                        leximin(&left.0, &right.0).then_with(|| left.1.cmp(&right.1))
                    });
                    ranked.iter().any(|&(_, _, point, hint)| {
                        matches!(
                            mesh.propose_site_for(point, Some(hint), gates, site),
                            Ok(Acceptance::Committed(_))
                        )
                    })
                }
            };

            if !served {
                served = mesh
                    .refine_cell(site, None, policy, gates)
                    .expect("refine")
                    .resolved()
                    .is_some()
                    || mesh
                        .refine_cell_fallback(site, policy, gates)
                        .expect("fallback")
                        .resolved()
                        .is_some();
            }
            if served {
                accepted += 1;
                stats.committed += 1;
            }
        }
        if accepted == 0 {
            break;
        }
    }
    stats
}

/// Frontal points for a demanded site's fan, worst radius-edge first.
fn frontal_points(
    state: &MeshState,
    criteria: &[Box<dyn CellCriterion>],
    minimum_cell_width_m: f64,
    rho_bar: f64,
    site: usize,
    stats: &mut FrontalStats,
) -> Vec<(CartesianPoint, usize)> {
    let mut fan: Vec<usize> = state.triangle_fan(site).unwrap_or_default();
    fan.sort_by(|&left, &right| {
        radius_edge_ratio(state, right)
            .unwrap_or(0.0)
            .total_cmp(&radius_edge_ratio(state, left).unwrap_or(0.0))
            .then_with(|| left.cmp(&right))
    });
    fan.into_iter()
        .filter_map(|triangle| {
            frontal_offcentre(
                state,
                criteria,
                minimum_cell_width_m,
                rho_bar,
                triangle,
                stats,
            )
            .map(|point| (point, triangle))
        })
        .collect()
}

/// The cavity quality a candidate would leave, if it clears the hard gates and
/// does not regress the local sorted vector. `None` when it does neither.
fn surviving_cavity_quality(
    mesh: &AdaptiveMesh,
    point: CartesianPoint,
    hint: usize,
    gates: HardGates,
) -> Option<Vec<f64>> {
    let state = mesh.state();
    let containing = state.locate_triangle(point, Some(hint)).ok()?;
    let cavity = state.delaunay_cavity(point, containing).ok()?;
    let before = sorted_eta(state, cavity.iter().copied());
    let mut clone = state.clone();
    let report = clone
        .insert_site_with_cavity(point, containing, &cavity)
        .ok()?;
    let after = sorted_eta(&clone, report.created.iter().copied());
    let touched: BTreeSet<_> = report.created.iter().copied().collect();
    (check(&clone, &touched, gates, &mesh.pentagon_ids()).is_ok() && better(&after, &before))
        .then_some(after)
}

/// `R / e_min` from chord lengths, the planar comparison triangle's ratio.
fn radius_edge_ratio(state: &MeshState, triangle: usize) -> Option<f64> {
    let corners = state.triangles()[triangle];
    let points = [
        state.vertices()[corners[0]],
        state.vertices()[corners[1]],
        state.vertices()[corners[2]],
    ];
    let lengths = [
        magnitude(CartesianPoint::new(
            points[1].x - points[2].x,
            points[1].y - points[2].y,
            points[1].z - points[2].z,
        )),
        magnitude(CartesianPoint::new(
            points[2].x - points[0].x,
            points[2].y - points[0].y,
            points[2].z - points[0].z,
        )),
        magnitude(CartesianPoint::new(
            points[0].x - points[1].x,
            points[0].y - points[1].y,
            points[0].z - points[1].z,
        )),
    ];
    let semiperimeter = lengths.iter().sum::<f64>() * 0.5;
    let area = (semiperimeter
        * (semiperimeter - lengths[0])
        * (semiperimeter - lengths[1])
        * (semiperimeter - lengths[2]))
        .max(0.0)
        .sqrt();
    let shortest = lengths.iter().copied().fold(f64::INFINITY, f64::min);
    (area > 0.0 && shortest > 0.0)
        .then(|| lengths.iter().product::<f64>() / (4.0 * area) / shortest)
        .filter(|ratio| ratio.is_finite())
}

/// Area-length of the named triangles, worst first.
fn sorted_eta(state: &MeshState, triangles: impl Iterator<Item = usize>) -> Vec<f64> {
    let mut values: Vec<f64> = triangles
        .filter_map(|triangle| triangle_eta_value(state, triangle))
        .collect();
    values.sort_by(f64::total_cmp);
    values
}

/// `Better(after, before)`: sorted, and no compared position may fall.
///
/// Unequal lengths are compared over the worst `min(len)` entries -- the fan
/// makes more triangles than the cavity destroyed, and the question is whether
/// the worst of them hold up. The paper does not specify the alignment.
fn better(after: &[f64], before: &[f64]) -> bool {
    let compared = after.len().min(before.len());
    compared > 0
        && after[..compared]
            .iter()
            .zip(&before[..compared])
            .all(|(after, before)| after >= before)
}

/// Rank by the sorted local vector, worst entry first; larger sorts earlier.
///
/// A total order, unlike `Better`, which is a predicate and leaves pairs
/// incomparable. Sorting needs the former.
fn leximin(left: &[f64], right: &[f64]) -> std::cmp::Ordering {
    for (mine, theirs) in left.iter().zip(right) {
        match mine.total_cmp(theirs) {
            std::cmp::Ordering::Equal => {}
            other => return other.reverse(),
        }
    }
    std::cmp::Ordering::Equal
}
