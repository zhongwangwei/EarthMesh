use super::*;

use earthmesh_mesh::{LonLatDegrees, TriangularMesh};

use crate::criteria::{TargetRegion, TargetScale};

/// Gates with the sliver floor off.
///
/// These tests are about the cycle -- demands, stop reasons, balance, degree --
/// and the shipped 28-degree floor would refuse ordinary insertions and change
/// what they measure. The floor has its own coverage in `transaction`.
fn permissive() -> HardGates {
    HardGates {
        min_triangle_angle_deg: 0.0,
        ..HardGates::default()
    }
}

fn sphere(nxp: usize) -> AdaptiveMesh {
    let mesh = TriangularMesh::from_icosahedron(nxp, 0, 1.0, 0.25, 0).expect("base mesh");
    AdaptiveMesh::from_triangular_mesh(&mesh).expect("adaptive mesh")
}

fn limits(max_cycles: u32, max_sites: usize) -> CycleLimits {
    CycleLimits {
        max_cycles,
        max_sites,
        minimum_cell_width_m: 1.0,
        max_neighbour_scale_ratio: 1.75,
    }
}

/// The coarsest cell on an NXP 6 sphere, so a target can be set relative to it.
fn coarsest_scale(mesh: &AdaptiveMesh) -> f64 {
    let state = mesh.state();
    let radius = state.sphere_radius();
    (MESH_STATE_FIRST_ID..state.vertices().len())
        .filter_map(|site| {
            let cell = state.voronoi_cell(site).ok()?;
            CellView {
                site,
                cell: &cell,
                state,
                radius_m: radius,
            }
            .effective_scale_m()
        })
        .fold(0.0_f64, f64::max)
}

fn target(target_scale_m: f64, region: TargetRegion) -> Vec<Box<dyn CellCriterion>> {
    vec![Box::new(TargetScale {
        id: "target-scale".to_string(),
        target_scale_m,
        region,
        source_resolution_m: None,
    })]
}

#[test]
fn evaluation_retains_only_evidence_that_can_affect_the_demand() {
    let mesh = sphere(2);
    let coarsest = coarsest_scale(&mesh);
    let mut criteria: Vec<Box<dyn CellCriterion>> = (0..4_096)
        .map(|index| {
            Box::new(TargetScale {
                id: format!("satisfied-{index}"),
                target_scale_m: coarsest * 2.0,
                region: TargetRegion::Global,
                source_resolution_m: None,
            }) as Box<dyn CellCriterion>
        })
        .collect();
    criteria.extend(target(coarsest * 0.5, TargetRegion::Global));

    let (demands, _, _) = evaluate(&mesh, &criteria, limits(1, 100_000)).expect("evaluate");

    assert!(!demands.is_empty());
    assert!(demands.iter().all(|demand| demand.evidences.len() == 1));
}

/// A target every cell already meets ends the run in one look, having done
/// nothing.
#[test]
fn a_satisfied_target_stops_at_once_and_changes_nothing() {
    let mut mesh = sphere(6);
    let before = mesh.state().clone();
    let criteria = target(coarsest_scale(&mesh) * 2.0, TargetRegion::Global);

    let outcome = run_cycles(
        &mut mesh,
        &criteria,
        CandidatePolicy::default(),
        permissive(),
        limits(8, 100_000),
    )
    .expect("run");

    assert_eq!(outcome.report.stop_reason, StopReason::AllSatisfied);
    assert_eq!(outcome.report.unresolved_count, 0);
    assert!(outcome.unresolved_cells.is_empty());
    assert_eq!(outcome.report.cycles_completed, 0);
    assert_eq!(outcome.report.transactions_attempted, 0);
    assert_eq!(mesh.state(), &before);
}

/// A target inside a circle refines there and leaves the rest of the sphere as
/// it was.
///
/// The property the whole backend is for: demand is served where it is, not by
/// refining everything to the finest asked anywhere.
#[test]
fn a_regional_target_refines_where_it_applies_and_nowhere_else() {
    let mut mesh = sphere(6);
    let coarsest = coarsest_scale(&mesh);
    let centre = LonLatDegrees::new(0.0, 0.0);
    let criteria = target(
        coarsest * 0.8,
        TargetRegion::Circle {
            centre,
            radius_m: 2_000_000.0,
        },
    );
    let before_sites = mesh.active_site_count();

    let outcome = run_cycles(
        &mut mesh,
        &criteria,
        CandidatePolicy::default(),
        permissive(),
        limits(6, 100_000),
    )
    .expect("run");

    assert!(outcome.report.transactions_committed > 0, "it did work");
    assert_eq!(
        mesh.active_site_count(),
        before_sites + outcome.report.transactions_committed
    );
    assert_eq!(mesh.state().open_edge_count(), 0);
    mesh.state().validate().expect("still a triangulation");

    // Every site added is inside the circle it was asked for, within a cell's
    // width -- a cavity reaches a little past the site that triggered it.
    let radius = mesh.state().sphere_radius();
    let unit_centre = earthmesh_mesh::lonlat_degrees_to_unit_xyz(centre);
    for site in mesh.sites().iter().filter(|site| site.birth_cycle > 0) {
        let unit = earthmesh_mesh::lonlat_degrees_to_unit_xyz(site.position);
        let dot = (unit.x * unit_centre.x + unit.y * unit_centre.y + unit.z * unit_centre.z)
            .clamp(-1.0, 1.0);
        assert!(
            dot.acos() * radius <= 2_000_000.0 + coarsest * 2.0,
            "a site was added at {:?}, outside the region that asked",
            site.position
        );
    }
}

/// Cells stop asking as they are refined, so the run converges rather than
/// stopping on a limit.
#[test]
fn refining_makes_the_demand_go_away() {
    let mut mesh = sphere(6);
    let criteria = target(
        coarsest_scale(&mesh) * 0.9,
        TargetRegion::Circle {
            centre: LonLatDegrees::new(120.0, 30.0),
            radius_m: 1_500_000.0,
        },
    );

    let outcome = run_cycles(
        &mut mesh,
        &criteria,
        CandidatePolicy::default(),
        permissive(),
        limits(12, 100_000),
    )
    .expect("run");

    assert!(
        matches!(
            outcome.report.stop_reason,
            StopReason::AllSatisfied
                | StopReason::NoAcceptedTransactions
                | StopReason::NoProductiveAdaptation
        ),
        "the run should converge or say it could not, not run out of cycles: {:?}",
        outcome.report.stop_reason
    );
    assert!(outcome.report.cycles_completed < 12);
}

/// The site budget stops the run and says so.
#[test]
fn the_site_budget_stops_the_run_under_its_own_name() {
    let mut mesh = sphere(6);
    let before = mesh.active_site_count();
    let criteria = target(coarsest_scale(&mesh) * 0.5, TargetRegion::Global);

    let outcome = run_cycles(
        &mut mesh,
        &criteria,
        CandidatePolicy::default(),
        permissive(),
        limits(20, before + 25),
    )
    .expect("run");

    assert_eq!(outcome.report.stop_reason, StopReason::BudgetReached);
    assert!(mesh.active_site_count() <= before + 25);
    assert_eq!(mesh.state().open_edge_count(), 0);
}

/// A run that can propose nothing legal says that, rather than reporting
/// success or exhausting its cycles.
///
/// The distinction the report exists to preserve: "everything was satisfied"
/// and "nothing I tried was allowed" produce the same mesh and mean opposite
/// things.
#[test]
fn a_run_that_cannot_place_anything_says_so() {
    let mut mesh = sphere(6);
    let before = mesh.state().clone();
    let criteria = target(coarsest_scale(&mesh) * 0.5, TargetRegion::Global);

    let outcome = run_cycles(
        &mut mesh,
        &criteria,
        CandidatePolicy::default(),
        HardGates {
            max_vertex_degree: 5,
            ..permissive()
        },
        limits(20, 100_000),
    )
    .expect("run");

    assert_eq!(
        outcome.report.stop_reason,
        StopReason::NoAcceptedTransactions
    );
    assert_eq!(outcome.report.cycles_completed, 1, "it did not keep trying");
    assert_eq!(outcome.report.transactions_committed, 0);
    assert!(outcome.report.unresolved_count > 0);
    assert_eq!(mesh.state(), &before, "and left the mesh alone");
}

/// A pure angle wall is named instead of being mistaken for more work.
#[test]
fn a_run_blocked_only_by_the_angle_floor_names_the_constraint() {
    let mut mesh = sphere(6);
    let criteria = target(coarsest_scale(&mesh) * 0.5, TargetRegion::Global);

    let outcome = run_cycles(
        &mut mesh,
        &criteria,
        CandidatePolicy::default(),
        HardGates {
            min_triangle_angle_deg: 59.0,
            ..permissive()
        },
        limits(20, 100_000),
    )
    .expect("run");

    assert_eq!(
        outcome.report.stop_reason,
        StopReason::QualityConstraintReached
    );
    assert_eq!(outcome.report.cycles_completed, 1);
    assert_eq!(outcome.report.transactions_committed, 0);
    assert_eq!(
        outcome.report.quality_constrained_count,
        outcome.report.unresolved_count
    );
    assert!(outcome.report.refusals.sliver > 0);
}

/// The cycle limit is reported as itself.
#[test]
fn the_cycle_limit_is_reported_as_the_cycle_limit() {
    let mut mesh = sphere(6);
    let criteria = target(coarsest_scale(&mesh) * 0.5, TargetRegion::Global);

    let outcome = run_cycles(
        &mut mesh,
        &criteria,
        CandidatePolicy::default(),
        permissive(),
        limits(1, 100_000),
    )
    .expect("run");

    assert_eq!(outcome.report.stop_reason, StopReason::MaximumCyclesReached);
    assert_eq!(outcome.report.cycles_completed, 1);
    assert!(outcome.report.transactions_committed > 0);
}

/// A cell already at the floor is not evaluated, and the run says so.
///
/// This asserted `AllSatisfied`, which pinned the defect rather than the
/// behaviour: a mesh every cell of which is parked at `minimum_cell_width_m`
/// has not satisfied a target of one metre -- it has stopped short of it. The
/// two endings need different answers from a caller, and `MinimumScaleReached`
/// existed as a variant with nothing ever assigning it.
///
/// The skip is whole-cell, so it also stops the *quality* criteria being asked
/// about that cell, not only the scale ones. That is worth knowing and is why
/// the reason is reported rather than folded away.
#[test]
fn a_cell_at_the_minimum_width_stops_asking() {
    let mut mesh = sphere(6);
    let before = mesh.state().clone();
    let criteria = target(1.0, TargetRegion::Global);
    // Coarser than every cell, so the floor covers the whole mesh.
    let floor = coarsest_scale(&mesh) * 2.0;

    let outcome = run_cycles(
        &mut mesh,
        &criteria,
        CandidatePolicy::default(),
        permissive(),
        CycleLimits {
            max_cycles: 8,
            max_sites: 100_000,
            minimum_cell_width_m: floor,
            max_neighbour_scale_ratio: 1.75,
        },
    )
    .expect("run");

    assert_eq!(
        outcome.report.stop_reason,
        StopReason::MinimumScaleReached,
        "every cell is at the floor, which is stopping short and not satisfying"
    );
    assert_eq!(outcome.report.transactions_attempted, 0);
    assert_eq!(mesh.state(), &before);
}

/// A mesh that genuinely meets its target still reports `AllSatisfied`.
///
/// The other side of the change above: telling the endings apart is only worth
/// anything if the satisfied one still says satisfied.
#[test]
fn a_mesh_that_meets_its_target_still_reports_all_satisfied() {
    let mut mesh = sphere(6);
    // Coarser than every cell, so every cell already meets it.
    let target_scale = coarsest_scale(&mesh) * 2.0;
    let criteria = target(target_scale, TargetRegion::Global);

    let outcome = run_cycles(
        &mut mesh,
        &criteria,
        CandidatePolicy::default(),
        permissive(),
        CycleLimits {
            max_cycles: 8,
            max_sites: 100_000,
            // Far below every cell, so nothing is parked at the floor.
            minimum_cell_width_m: 1.0,
            max_neighbour_scale_ratio: 1.75,
        },
    )
    .expect("run");

    assert_eq!(outcome.report.stop_reason, StopReason::AllSatisfied);
    assert_eq!(outcome.report.transactions_attempted, 0);
}

/// The same run twice gives the same mesh and the same report.
#[test]
fn a_run_is_deterministic() {
    let build = || {
        let mut mesh = sphere(6);
        let criteria = target(
            coarsest_scale(&mesh) * 0.85,
            TargetRegion::Circle {
                centre: LonLatDegrees::new(-60.0, 20.0),
                radius_m: 1_800_000.0,
            },
        );
        let outcome = run_cycles(
            &mut mesh,
            &criteria,
            CandidatePolicy::default(),
            permissive(),
            limits(8, 100_000),
        )
        .expect("run");
        (mesh.state().clone(), outcome.report)
    };
    let (first_state, first_report) = build();
    let (second_state, second_report) = build();
    assert_eq!(first_state, second_state);
    assert_eq!(first_report, second_report);
    assert!(first_report.transactions_committed > 0);
}

/// The worst neighbour scale ratio a run leaves, and how many pairs are over.
fn ratio_survey(mesh: &AdaptiveMesh, bound: f64) -> (f64, usize) {
    let state = mesh.state();
    let radius = state.sphere_radius();
    let scale = |site: usize| {
        let cell = state.voronoi_cell(site).ok()?;
        CellView {
            site,
            cell: &cell,
            state,
            radius_m: radius,
        }
        .effective_scale_m()
    };
    let mut worst = 1.0_f64;
    let mut over = 0usize;
    for site in MESH_STATE_FIRST_ID..state.vertices().len() {
        let Some(here) = scale(site) else { continue };
        for triangle in state.triangle_fan(site).expect("fan") {
            for corner in state.triangles()[triangle] {
                if corner == site {
                    continue;
                }
                let Some(there) = scale(corner) else { continue };
                let ratio = here.max(there) / here.min(there);
                worst = worst.max(ratio);
                if ratio > bound {
                    over += 1;
                }
            }
        }
    }
    (worst, over)
}

fn steep_target(mesh: &AdaptiveMesh) -> Vec<Box<dyn CellCriterion>> {
    target(
        coarsest_scale(mesh) * 0.3,
        TargetRegion::Circle {
            centre: LonLatDegrees::new(105.0, 35.0),
            radius_m: 1_200_000.0,
        },
    )
}

fn worst_degree(mesh: &AdaptiveMesh) -> usize {
    let state = mesh.state();
    (MESH_STATE_FIRST_ID..state.vertices().len())
        .filter_map(|site| state.vertex_degree(site).ok())
        .max()
        .unwrap_or(0)
}

fn worst_triangle_floor(mesh: &AdaptiveMesh) -> f64 {
    let state = mesh.state();
    (MESH_STATE_FIRST_ID..state.triangles().len())
        .map(|triangle| {
            let corners = state.triangles()[triangle];
            crate::criteria::smallest_angle_deg_for_test([
                state.vertices()[corners[0]],
                state.vertices()[corners[1]],
                state.vertices()[corners[2]],
            ])
        })
        .fold(f64::MAX, f64::min)
}

/// A degree wall is not only counted; it is relieved without breaking the mesh.
///
/// This is deliberately the same small deterministic target as the balance
/// tests. Before the r-adaptation hook, the run stopped with degree refusals
/// and a larger balance residue. The regression is relational enough to leave
/// room for harmless geometry tie changes, but concrete enough to fail if the
/// relief move is deleted or stops passing through the ordinary gates.
#[test]
fn degree_relief_moves_reduce_the_wall_without_breaking_quality_gates() {
    let mut mesh = sphere(6);
    let criteria = steep_target(&mesh);
    let gates = HardGates {
        min_triangle_angle_deg: 20.0,
        ..HardGates::default()
    };
    let outcome = run_cycles(
        &mut mesh,
        &criteria,
        CandidatePolicy::default(),
        gates,
        limits(40, 100_000),
    )
    .expect("run");

    let (worst_ratio, unbalanced) = ratio_survey(&mesh, 1.75);
    assert!(
        outcome.report.degree_relieving_moves > 0,
        "the fixture must exercise r-adaptation, not only count a degree wall: {:?}",
        outcome.report
    );
    assert!(
        outcome.report.unresolved_count <= 2,
        "r-adaptation should remove most of the degree-wall residue; protected pentagons may \
         still refuse a local insertion: {:?}",
        outcome.report
    );
    assert!(
        unbalanced <= 8 && worst_ratio < 2.1,
        "r-adaptation should keep the balance residue bounded, got {unbalanced} pairs and \
         worst ratio {worst_ratio:.3}"
    );
    assert_eq!(outcome.report.unbalanced_pairs_remaining, unbalanced);
    assert!(
        worst_degree(&mesh) <= gates.max_vertex_degree,
        "degree gate leaked: worst degree {}",
        worst_degree(&mesh)
    );
    assert!(
        worst_triangle_floor(&mesh) >= gates.min_triangle_angle_deg - 1.0e-9,
        "angle gate leaked: worst angle {:.6}",
        worst_triangle_floor(&mesh)
    );
    assert_eq!(mesh.state().open_edge_count(), 0);
    mesh.state().validate().expect("still a triangulation");
}

#[test]
fn final_unresolved_cells_are_reevaluated_after_the_last_adaptation() {
    let mut mesh = sphere(6);
    let criteria = steep_target(&mesh);
    let limits = limits(1, 100_000);
    let outcome = run_cycles(
        &mut mesh,
        &criteria,
        CandidatePolicy::default(),
        permissive(),
        limits,
    )
    .expect("run");
    let (physical, _, scales) = evaluate(&mesh, &criteria, limits).expect("final evaluation");
    let pending: BTreeSet<usize> = physical
        .iter()
        .chain(balance_demands(&mesh, &scales, limits).iter())
        .map(|demand| demand.cell as usize)
        .collect();

    assert_eq!(
        outcome.unresolved_cells,
        pending.into_iter().collect::<Vec<_>>()
    );
    assert_eq!(
        outcome.report.unresolved_count,
        outcome.unresolved_cells.len()
    );
}

/// Balance plus r-adaptation closes the neighbour-scale gap.
///
/// Measured on the same target with balance off: worst neighbour ratio 2.46,
/// 58 adjacent pairs past 1.75. Insertion-only balance left 1.96 and 16;
/// r-adaptation closes the scale remainder. A protected pentagon can still
/// leave one explicit size demand; hiding that would be worse than reporting
/// the hard topology constraint.
#[test]
fn scale_balance_and_r_adaptation_close_the_gap() {
    let mut mesh = sphere(6);
    let criteria = steep_target(&mesh);
    let outcome = run_cycles(
        &mut mesh,
        &criteria,
        CandidatePolicy::default(),
        permissive(),
        CycleLimits {
            max_cycles: 40,
            max_sites: 100_000,
            minimum_cell_width_m: 1.0,
            max_neighbour_scale_ratio: 1.75,
        },
    )
    .expect("run");

    let (worst, over) = ratio_survey(&mesh, 1.75);
    assert_eq!(over, 0, "balance left {over} pairs past 1.75");
    assert!(worst <= 1.75, "worst neighbour ratio is {worst:.3}");
    assert!(
        outcome.report.balance_transactions_committed > 0,
        "and it took work to do it"
    );
    assert_eq!(
        outcome.report.unbalanced_pairs_remaining, over,
        "the report says what is left rather than leaving a caller to find out"
    );
    assert!(matches!(
        outcome.report.stop_reason,
        StopReason::AllSatisfied | StopReason::NoAcceptedTransactions
    ));
    assert_eq!(
        outcome.report.unresolved_count,
        outcome.unresolved_cells.len()
    );
    assert!(
        outcome.report.unresolved_count <= 1,
        "only a protected-pentagon demand may remain: {:?}",
        outcome.report
    );
    if outcome.report.unresolved_count == 1 {
        assert!(outcome.report.refusals.pentagon > 0);
    }
    assert_eq!(mesh.state().open_edge_count(), 0);
    mesh.state().validate().expect("still a triangulation");
}

/// Turning balance off reproduces the measurement it was built for.
///
/// Without this the previous test could pass because the target happens not to
/// produce a steep gradient, and nobody would know.
#[test]
fn without_balance_the_same_target_breaks_the_bound() {
    let mut mesh = sphere(6);
    let criteria = steep_target(&mesh);
    run_cycles(
        &mut mesh,
        &criteria,
        CandidatePolicy::default(),
        permissive(),
        CycleLimits {
            max_cycles: 30,
            max_sites: 100_000,
            minimum_cell_width_m: 1.0,
            // Beyond anything a run can reach, so no balance demand is raised.
            max_neighbour_scale_ratio: 1.0e9,
        },
    )
    .expect("run");

    let (worst, over) = ratio_survey(&mesh, 1.75);
    assert!(
        over > 0 && worst > 1.75,
        "this target was chosen because it breaks the bound; it no longer does \
         ({over} over, worst {worst:.3}), so the balance test above proves nothing"
    );
}

/// The report tells physical refinement from balance refinement.
///
/// Section 14 asks for the distinction by name: a reader has to be able to see
/// what the run was asked for and what that cost in cells nobody requested.
#[test]
fn the_report_separates_balance_from_what_was_asked_for() {
    let mut mesh = sphere(6);
    let criteria = steep_target(&mesh);
    let outcome = run_cycles(
        &mut mesh,
        &criteria,
        CandidatePolicy::default(),
        permissive(),
        CycleLimits {
            max_cycles: 30,
            max_sites: 100_000,
            minimum_cell_width_m: 1.0,
            max_neighbour_scale_ratio: 1.75,
        },
    )
    .expect("run");

    let report = &outcome.report;
    assert!(report.balance_transactions_committed > 0);
    assert!(
        report.balance_transactions_committed < report.transactions_committed,
        "balance should be a cost of the refinement, not the whole of it: {report:?}"
    );
}

/// The refusals are counted by kind, and one kind dominates.
///
/// Measured through the CLI at NXP 21, two levels: 786 refusals on the degree
/// bound against 30 on the pentagons and zero everywhere else. That number is
/// what says the remaining work is site motion rather than a better candidate
/// ladder -- candidate generation never failed once.
///
/// Asserted as a shape rather than as those figures: that the tally adds up to
/// what the run rolled back, and that degree is the largest kind. Pinning 786
/// would make the next person to improve this edit the test first.
#[test]
fn the_refusals_are_counted_by_kind() {
    let mut mesh = sphere(6);
    let criteria = steep_target(&mesh);
    let outcome = run_cycles(
        &mut mesh,
        &criteria,
        CandidatePolicy::default(),
        permissive(),
        limits(20, 100_000),
    )
    .expect("run");

    let refusals = outcome.report.refusals;
    assert_eq!(
        refusals.total(),
        outcome.report.transactions_rolled_back,
        "every rollback is accounted for by kind: {refusals:?}"
    );
    assert!(refusals.total() > 0);
    assert!(
        refusals.degree >= refusals.pentagon
            && refusals.degree >= refusals.not_insertable
            && refusals.degree >= refusals.topology,
        "the degree bound is the wall this backend runs into: {refusals:?}"
    );
    assert_eq!(
        refusals.not_insertable, 0,
        "the ladder always found somewhere legal to try: {refusals:?}"
    );
}

/// Raising the degree budget buys nothing once the bound stops binding.
///
/// The measurement that decides whether widening `ItabW` -- the gridfile's
/// `[i32; 7]` rows -- is worth the risk to the Fortran comparison path. It is
/// not: this backend saturates, and removing the bound entirely adds a few
/// percent of cells and still stops as `NoAcceptedTransactions`. Guide 11.17.
///
/// # The saturation degree is platform-dependent, and was asserted anyway
///
/// This asserted `worst_nine == 9`, passed on macOS for months, and turned CI
/// red the first time it ran on Linux, where the same run reaches **8**.
/// Nothing was wrong with the mesh: budget 9 and budget 16 still produced the
/// identical one. What differed is a discrete value -- the largest vertex
/// degree -- read off geometry that is continuous, where a last-bit difference
/// in one predicate moves an insertion and takes a degree with it.
///
/// `worst < budget` was the next guess and is also wrong: on macOS the worst
/// degree *equals* the budget of nine, and raising the budget to sixteen still
/// changes nothing. So the bound being reached does not mean the bound is what
/// stopped the run.
///
/// **The equality below is the whole claim.** Two budgets, one mesh: raising
/// the budget buys nothing. It needs no degree at all, and the exact figure
/// belongs in the guide as a measurement rather than in an assertion as a
/// constant -- a constant read off continuous geometry is an assertion about
/// the machine that measured it.
#[test]
fn the_degree_budget_saturates() {
    let run = |budget: usize| {
        let mut mesh = sphere(6);
        let criteria = steep_target(&mesh);
        run_cycles(
            &mut mesh,
            &criteria,
            CandidatePolicy::default(),
            HardGates {
                max_vertex_degree: budget,
                ..permissive()
            },
            limits(40, 100_000),
        )
        .expect("run");
        let state = mesh.state();
        let worst = (MESH_STATE_FIRST_ID..state.vertices().len())
            .filter_map(|site| state.vertex_degree(site).ok())
            .max()
            .unwrap_or(0);
        (mesh.active_site_count(), worst)
    };
    let (cells_nine, worst_nine) = run(9);
    let (cells_sixteen, worst_sixteen) = run(16);
    assert_eq!(
        (cells_nine, worst_nine),
        (cells_sixteen, worst_sixteen),
        "a budget past nine changed the mesh, so the saturation this rests on is gone"
    );
    // No assertion on `worst_nine` itself: measured 9 on macOS and 8 on Linux,
    // and the claim does not rest on which.

    let (cells_seven, _worst_seven) = run(7);
    assert!(
        (cells_nine as f64) < cells_seven as f64 * 1.10,
        "removing the degree bound bought {cells_seven} -> {cells_nine} cells; if that is now a \
         large gain, widening the gridfile rows is worth reconsidering"
    );
}

/// With degree out of the way, every remaining refusal is a pentagon.
///
/// The second wall, and it has a fix that costs no code: guide 11.14 measured
/// that relaxing the base mesh (`NL%niter`) takes pentagon refusals to zero,
/// because a relaxed mesh separates the twelve enough that candidates stop
/// landing beside one.
#[test]
fn the_wall_behind_degree_is_the_pentagons() {
    let mut mesh = sphere(6);
    let criteria = steep_target(&mesh);
    let outcome = run_cycles(
        &mut mesh,
        &criteria,
        CandidatePolicy::default(),
        HardGates {
            max_vertex_degree: 16,
            ..permissive()
        },
        limits(40, 100_000),
    )
    .expect("run");

    let refusals = outcome.report.refusals;
    assert_eq!(
        refusals.degree, 0,
        "the budget was meant to be out of reach"
    );
    assert!(refusals.pentagon > 0);
    assert_eq!(
        refusals.pentagon,
        refusals.total(),
        "with degree lifted, the pentagons are the whole of what is left: {refusals:?}"
    );
}

/// Protected segments are what let a quality target reach Ruppert's bound.
///
/// A 20-degree angle target with explicit protected segments converges and
/// every triangle clears the requested 20 degrees.  The unconstrained path is
/// checked too: r-adaptation may now clear this particular fixture without
/// segments, but that empirical result is not Ruppert's boundary guarantee.
///
/// 20 degrees, not 25. Guide 11.29: the sound segment list diverges at 25,
/// which is what the theory says -- Ruppert's proof reaches about 20.7 and no
/// further, and 30 needs Chew's variant. An earlier version appeared to
/// converge at 25 because it approximated segments with a set of boundary
/// sites, diverted nearly every candidate into splitting spurious ones, and so
/// did almost no quality refinement at all.
#[test]
fn protected_segments_make_a_quality_target_terminate() {
    use crate::criteria::MinAngle;
    const ANGLE: f64 = 20.0;

    let run = |protect: bool| {
        let base = TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25, 0).expect("base");
        let mut mesh = AdaptiveMesh::from_triangular_mesh(&base).expect("adaptive");
        let coarsest = coarsest_scale(&mesh);
        let centre = LonLatDegrees::new(105.0, 35.0);
        let reach = 1_200_000.0;
        if protect {
            // The boundary as segments: mesh edges that straddle the circle,
            // one endpoint inside and one outside. That is a discretisation of
            // the curve, which a set of nearby sites is not.
            let state = mesh.state();
            let radius = state.sphere_radius();
            let unit_centre = earthmesh_mesh::lonlat_degrees_to_unit_xyz(centre);
            let inside = |site: usize| {
                let point = state.vertices()[site];
                let length = earthmesh_mesh::magnitude(point);
                if length <= 0.0 {
                    return false;
                }
                let dot =
                    (point.x * unit_centre.x + point.y * unit_centre.y + point.z * unit_centre.z)
                        / length;
                dot.clamp(-1.0, 1.0).acos() * radius <= reach
            };
            let mut segments = std::collections::BTreeSet::new();
            for triangle in MESH_STATE_FIRST_ID..state.triangles().len() {
                let corners = state.triangles()[triangle];
                for corner in 0..3 {
                    let (a, b) = (corners[(corner + 1) % 3], corners[(corner + 2) % 3]);
                    if inside(a) != inside(b) {
                        segments.insert((a.min(b), a.max(b)));
                    }
                }
            }
            mesh.protect_segments(segments);
        }
        let mut criteria = target(
            coarsest / 4.0,
            TargetRegion::Circle {
                centre,
                radius_m: reach,
            },
        );
        criteria.push(Box::new(MinAngle {
            id: "min-angle".to_string(),
            min_angle_deg: ANGLE,
        }));
        let outcome = run_cycles(
            &mut mesh,
            &criteria,
            CandidatePolicy::default(),
            permissive(),
            limits(40, 200_000),
        )
        .expect("run");
        let state = mesh.state();
        let worst = (MESH_STATE_FIRST_ID..state.triangles().len())
            .map(|triangle| {
                let c = state.triangles()[triangle];
                crate::criteria::smallest_angle_deg_for_test([
                    state.vertices()[c[0]],
                    state.vertices()[c[1]],
                    state.vertices()[c[2]],
                ])
            })
            .fold(f64::MAX, f64::min);
        (mesh.active_site_count(), worst, outcome.report.stop_reason)
    };

    let (sites, worst, stop) = run(true);
    assert!(
        sites < 2_000,
        "{sites} sites; it should converge, not run away"
    );
    assert!(
        worst >= ANGLE,
        "min triangle angle {worst:.2}; requested {ANGLE:.2}"
    );
    assert_eq!(stop, StopReason::NoAcceptedTransactions);

    let (unprotected_sites, unprotected_worst, unprotected_stop) = run(false);
    assert!(
        unprotected_sites < 2_000,
        "the unconstrained comparison must stay bounded, got {unprotected_sites} sites"
    );
    assert!(
        unprotected_worst >= ANGLE,
        "the unconstrained comparison regressed below {ANGLE:.2}: {unprotected_worst:.2}"
    );
    assert!(
        matches!(
            unprotected_stop,
            StopReason::AllSatisfied | StopReason::NoAcceptedTransactions
        ),
        "unexpected unconstrained stop: {unprotected_stop:?}"
    );
}

/// A criterion covering nowhere does not read as "stopped at the floor".
///
/// The tally that tells the three endings apart was first counted *before* the
/// criteria were read, so any cell below `minimum_cell_width_m` counted --
/// including every cell outside the region, which wants nothing at all. A run
/// with an empty region and no demand anywhere came back
/// `MinimumScaleReached`, which is the same kind of wrong answer the tally was
/// added to stop: a run that asked for nothing is satisfied, not thwarted.
#[test]
fn a_region_that_covers_no_cell_reports_satisfied_not_thwarted() {
    let mut mesh = sphere(6);
    // A circle of ten metres somewhere in the Pacific: no cell's centre is in
    // it, so no criterion demands anything of any cell.
    let criteria = target(
        1.0,
        TargetRegion::Circle {
            centre: earthmesh_mesh::LonLatDegrees::new(-140.0, -30.0),
            radius_m: 10.0,
        },
    );
    // And a floor above every cell, so the old tally would have counted them all.
    let floor = coarsest_scale(&mesh) * 2.0;

    let outcome = run_cycles(
        &mut mesh,
        &criteria,
        CandidatePolicy::default(),
        permissive(),
        CycleLimits {
            max_cycles: 8,
            max_sites: 100_000,
            minimum_cell_width_m: floor,
            max_neighbour_scale_ratio: 1.75,
        },
    )
    .expect("run");

    assert_eq!(
        outcome.report.stop_reason,
        StopReason::AllSatisfied,
        "nothing was ever asked, so nothing was left unmet"
    );
    assert_eq!(outcome.report.transactions_attempted, 0);
}
