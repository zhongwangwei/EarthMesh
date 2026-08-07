use super::*;

use earthmesh_mesh::{LonLatDegrees, TriangularMesh};

use crate::criteria::{TargetRegion, TargetScale};

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
        HardGates::default(),
        limits(8, 100_000),
    )
    .expect("run");

    assert_eq!(outcome.report.stop_reason, StopReason::AllSatisfied);
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
        HardGates::default(),
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
        HardGates::default(),
        limits(12, 100_000),
    )
    .expect("run");

    assert!(
        matches!(
            outcome.report.stop_reason,
            StopReason::AllSatisfied | StopReason::NoAcceptedTransactions
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
        HardGates::default(),
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
            ..HardGates::default()
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

/// The cycle limit is reported as itself.
#[test]
fn the_cycle_limit_is_reported_as_the_cycle_limit() {
    let mut mesh = sphere(6);
    let criteria = target(coarsest_scale(&mesh) * 0.5, TargetRegion::Global);

    let outcome = run_cycles(
        &mut mesh,
        &criteria,
        CandidatePolicy::default(),
        HardGates::default(),
        limits(1, 100_000),
    )
    .expect("run");

    assert_eq!(outcome.report.stop_reason, StopReason::MaximumCyclesReached);
    assert_eq!(outcome.report.cycles_completed, 1);
    assert!(outcome.report.transactions_committed > 0);
}

/// A cell already at the floor is not evaluated at all.
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
        HardGates::default(),
        CycleLimits {
            max_cycles: 8,
            max_sites: 100_000,
            minimum_cell_width_m: floor,
            max_neighbour_scale_ratio: 1.75,
        },
    )
    .expect("run");

    assert_eq!(outcome.report.stop_reason, StopReason::AllSatisfied);
    assert_eq!(outcome.report.transactions_attempted, 0);
    assert_eq!(mesh.state(), &before);
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
            HardGates::default(),
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

/// Balance closes most of the gap, and the run reports what it could not.
///
/// Measured on the same target with balance off: worst neighbour ratio 2.46,
/// 58 adjacent pairs past 1.75. With it on: 1.96 and 16, and the run stops
/// saying `NoAcceptedTransactions` rather than claiming to be finished.
///
/// It does not reach zero, and the reason is worth stating because it is not a
/// tuning problem. The degree bound and the scale bound pull against each
/// other: closing the last ratios needs cells the degree gate will not allow,
/// and insertion is the only move this backend has. Section 8.1 puts
/// r-adaptation -- moving sites -- ahead of h-adaptation for exactly this, and
/// it is not implemented. Until it is, the residual is reported rather than
/// hidden, because a mesh that quietly violates the bound it claims is the
/// failure class this backend exists to avoid.
#[test]
fn scale_balance_closes_most_of_the_gap_and_reports_the_rest() {
    let mut mesh = sphere(6);
    let criteria = steep_target(&mesh);
    let outcome = run_cycles(
        &mut mesh,
        &criteria,
        CandidatePolicy::default(),
        HardGates::default(),
        CycleLimits {
            max_cycles: 40,
            max_sites: 100_000,
            minimum_cell_width_m: 1.0,
            max_neighbour_scale_ratio: 1.75,
        },
    )
    .expect("run");

    let (worst, over) = ratio_survey(&mesh, 1.75);
    assert!(
        over <= 45 && worst < 2.2,
        "balance left {over} pairs past 1.75, worst {worst:.3}; without it the same target \
         leaves 58 and 2.46, so this is no better than doing nothing"
    );
    assert!(
        outcome.report.balance_transactions_committed > 0,
        "and it took work to do it"
    );
    assert_eq!(
        outcome.report.unbalanced_pairs_remaining, over,
        "the report says what is left rather than leaving a caller to find out"
    );
    assert_eq!(
        outcome.report.stop_reason,
        StopReason::NoAcceptedTransactions,
        "a run that could not finish balancing does not report having finished"
    );
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
        HardGates::default(),
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
        HardGates::default(),
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
