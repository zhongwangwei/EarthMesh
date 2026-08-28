use super::*;

use earthmesh_mesh::{LonLatDegrees, MeshState, TriangularMesh};

use crate::certifier::TargetGradientBin;
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
    let mesh = TriangularMesh::from_icosahedron(nxp, 0, 1.0, 0.25).expect("base mesh");
    AdaptiveMesh::from_triangular_mesh(&mesh).expect("adaptive mesh")
}

fn on(mesh: &AdaptiveMesh, lon: f64, lat: f64) -> CartesianPoint {
    let unit = earthmesh_mesh::lonlat_degrees_to_unit_xyz(LonLatDegrees::new(lon, lat));
    let radius = mesh.state().sphere_radius();
    CartesianPoint::new(unit.x * radius, unit.y * radius, unit.z * radius)
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
    state
        .active_vertex_slots()
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

fn edge_count(state: &MeshState) -> usize {
    let mut edges = BTreeSet::new();
    for [a, b, c] in state
        .active_triangle_slots()
        .map(|triangle| state.triangles()[triangle])
    {
        edges.extend([
            (a.min(b), a.max(b)),
            (b.min(c), b.max(c)),
            (c.min(a), c.max(a)),
        ]);
    }
    edges.len()
}

fn demanded_cells(state: &MeshState, criteria: &[Box<dyn CellCriterion>]) -> usize {
    let radius_m = state.sphere_radius();
    state
        .active_vertex_slots()
        .filter(|&site| {
            let Ok(cell) = state.voronoi_cell(site) else {
                return true;
            };
            let view = CellView {
                site,
                cell: &cell,
                state,
                radius_m,
            };
            criteria.iter().any(|criterion| {
                criterion
                    .evaluate(&view)
                    .map_or(true, |evidence| evidence.demands_work())
            })
        })
        .count()
}

#[test]
fn production_degree_four_retirement_has_a_quality_improving_candidate() {
    let mesh = sphere(6);
    let source = mesh.state().clone();
    let parent = 20;
    let parent_id = mesh.sites()[parent - MESH_STATE_FIRST_ID].site_id;
    let mut inserted = None;
    'search: for lon in (-160..=160).step_by(20) {
        for lat in (-60..=60).step_by(20) {
            let mut trial = sphere(6);
            if let Acceptance::Committed(report) = trial
                .propose_site_for(
                    on(&trial, lon as f64, lat as f64),
                    None,
                    permissive(),
                    parent,
                )
                .expect("proposal")
            {
                let site = report.vertex;
                if trial.state().vertex_degree(site).ok() == Some(4)
                    && !trial.pentagon_ids().contains(&site)
                {
                    inserted = Some((trial, site));
                    break 'search;
                }
            }
        }
    }
    let (trial, leaf) = inserted.expect("the fixture has a degree-four inserted site");
    let adaptive = &trial.sites()[leaf - MESH_STATE_FIRST_ID];
    assert_eq!(adaptive.parent_site_id, Some(parent_id));
    assert_eq!(adaptive.mobility, SiteMobility::Interior);
    assert!(adaptive.birth_cycle > 0);

    let before = trial.state().clone();
    let before_margin = trial
        .state()
        .triangle_fan(leaf)
        .expect("leaf fan")
        .into_iter()
        .filter_map(|triangle| triangle_window_margin(trial.state(), triangle))
        .fold(f64::MAX, f64::min);
    let criteria = target(coarsest_scale(&trial) * 2.0, TargetRegion::Global);
    let before_demands = demanded_cells(trial.state(), &criteria);
    let mut retired = trial.state().clone();
    retired
        .retire_degree_four_vertex_transactionally(leaf, |candidate, _| {
            let after_margin = all_triangle_window_margins(candidate)
                .and_then(|margins| margins.first().copied())
                .unwrap_or(f64::MIN);
            let degrees_ok = candidate.active_vertex_slots().all(|site| {
                candidate
                    .vertex_degree(site)
                    .is_ok_and(|degree| degree <= 7)
            });
            let angle_floor_ok = candidate.active_triangle_slots().all(|triangle| {
                let corners = candidate.triangles()[triangle];
                crate::criteria::smallest_triangle_angle_deg([
                    candidate.vertices()[corners[0]],
                    candidate.vertices()[corners[1]],
                    candidate.vertices()[corners[2]],
                ]) >= 25.0
            });
            after_margin > before_margin
                && degrees_ok
                && angle_floor_ok
                && demanded_cells(candidate, &criteria) <= before_demands
        })
        .expect("one diagonal improves quality without breaking the hard gates");

    assert_eq!(
        trial.state(),
        &before,
        "the spike must not mutate its source"
    );
    assert_ne!(trial.state(), &source, "the known-parent leaf was inserted");
    assert_eq!(retired.vertex_count() + 1, trial.state().vertex_count());
    assert_eq!(retired.triangle_count() + 2, trial.state().triangle_count());
    assert_eq!(
        retired.vertex_count() as isize - edge_count(&retired) as isize
            + retired.triangle_count() as isize,
        trial.state().vertex_count() as isize - edge_count(trial.state()) as isize
            + trial.state().triangle_count() as isize
    );
}

#[test]
fn retirement_audit_reads_production_trials_without_mutating_the_mesh() {
    let parent = 20;
    let mut fixture = None;
    'search: for lon in (-160..=160).step_by(20) {
        for lat in (-60..=60).step_by(20) {
            let mut trial = sphere(6);
            if let Acceptance::Committed(report) = trial
                .propose_site_for(
                    on(&trial, lon as f64, lat as f64),
                    None,
                    permissive(),
                    parent,
                )
                .expect("proposal")
            {
                let site = report.vertex;
                if trial.state().vertex_degree(site).ok() == Some(4)
                    && !trial.pentagon_ids().contains(&site)
                {
                    fixture = Some(trial);
                    break 'search;
                }
            }
        }
    }
    let trial = fixture.expect("the fixture has a degree-four inserted site");
    let before = trial.state().clone();
    let criteria = target(coarsest_scale(&trial) * 2.0, TargetRegion::Global);
    let leaves = leaf_lineage_survey(&trial, &criteria);
    let audit = degree_four_retirement_audit(
        &trial,
        &criteria,
        permissive(),
        limits(1, 100_000),
        &leaves,
        4,
    )
    .expect("audit");

    assert!(audit.summary.evaluated);
    assert_eq!(
        audit.summary.sites_total,
        audit.summary.sites_not_leaf + audit.summary.sites_eligible
    );
    assert_eq!(
        audit.summary.sites_eligible,
        audit.summary.sites_without_window_violation + audit.summary.sites_audited
    );
    assert_eq!(audit.summary.sites_audited, 1);
    assert_eq!(audit.sites.len(), audit.summary.sites_total);
    assert_eq!(audit.trials.len(), audit.summary.trials_total);
    assert_eq!(audit.summary.trials_total, 2);
    assert_eq!(audit.sites[0].trial_count, 2);
    assert!(audit.summary.checks.geometry.pass > 0);
    assert!(audit.summary.checks.hard_gate.pass > 0);
    assert_eq!(
        audit.summary.sites_with_any_valid_trial,
        audit
            .sites
            .iter()
            .filter(|site| site.any_valid_trial)
            .count()
    );
    assert_eq!(
        audit.summary.checks.geometry.pass,
        audit
            .trials
            .iter()
            .filter(|trial| trial.geometry == DegreeFourCheckStatus::Pass)
            .count()
    );
    assert!(audit
        .trials
        .windows(2)
        .all(|pair| pair[0].site_id < pair[1].site_id
            || (pair[0].site_id == pair[1].site_id && pair[0].trial_index < pair[1].trial_index)));
    for counts in [
        &audit.summary.checks.geometry,
        &audit.summary.checks.hard_gate,
        &audit.summary.checks.physical_demand,
        &audit.summary.checks.scale_balance,
        &audit.summary.checks.no_new_low_degree,
        &audit.summary.checks.angle_count,
        &audit.summary.checks.worst_deviation,
        &audit.summary.checks.penalty,
        &audit.summary.checks.eta,
        &audit.summary.checks.margin,
        &audit.summary.checks.conservative_remap,
    ] {
        assert_eq!(
            counts.pass + counts.fail + counts.not_evaluated,
            audit.summary.trials_total
        );
    }
    assert_eq!(trial.state(), &before);
}

#[test]
fn geometry_failure_leaves_downstream_checks_not_evaluated() {
    let trial = unevaluated_degree_four_trial(
        DegreeFourTrialIdentity {
            index: 0,
            ring_site_ids: None,
            diagonal_site_ids: None,
            diagonal_vertices: None,
        },
        SiteId(7),
        11,
    );
    assert_eq!(trial.geometry, DegreeFourCheckStatus::Fail);
    assert!([
        trial.hard_gate,
        trial.physical_demand,
        trial.scale_balance,
        trial.no_new_low_degree,
        trial.angle_count,
        trial.worst_deviation,
        trial.penalty,
        trial.eta,
        trial.margin,
        trial.conservative_remap,
    ]
    .into_iter()
    .all(|check| check == DegreeFourCheckStatus::NotEvaluated));
    assert!(!trial.fully_acceptable);
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

    assert_eq!(outcome.report.cycles_completed, 2, "fixture changed");
    assert_eq!(
        mesh.sites().iter().map(|site| site.birth_cycle).max(),
        Some(2),
        "sites inserted in the second cycle must say so"
    );

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
    // A patch, not the globe. What this asserts is about the cycle loop's
    // report, and `run_cycles` goes on to spend forty-eight quality passes on
    // whatever the loop produced -- so a global target makes this test pay for
    // optimising a mesh it never looks at.
    let criteria = target(
        coarsest_scale(&mesh) * 0.5,
        TargetRegion::Circle {
            centre: LonLatDegrees::new(105.0, 35.0),
            radius_m: 600_000.0,
        },
    );

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
    for site in state.active_vertex_slots() {
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

/// The same shape as `steep_target` over a smaller patch.
///
/// For tests whose claim is about the cycle loop's report rather than about the
/// size of the refined region. `run_cycles` spends forty-eight quality passes on
/// whatever the loop leaves behind, so the patch is what those tests are really
/// paying for.
fn small_target(mesh: &AdaptiveMesh) -> Vec<Box<dyn CellCriterion>> {
    target(
        coarsest_scale(mesh) * 0.3,
        TargetRegion::Circle {
            centre: LonLatDegrees::new(105.0, 35.0),
            radius_m: 600_000.0,
        },
    )
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
    state
        .active_vertex_slots()
        .filter_map(|site| state.vertex_degree(site).ok())
        .max()
        .unwrap_or(0)
}

fn worst_triangle_floor(mesh: &AdaptiveMesh) -> f64 {
    let state = mesh.state();
    state
        .active_triangle_slots()
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

#[test]
fn angle_window_survey_counts_every_triangle_corner() {
    let mesh = sphere(3);
    let survey = angle_window_survey(mesh.state());
    let corners = mesh.state().triangle_count() * 3;
    assert_eq!(
        survey.below + survey.inside_40_80 + survey.above_80,
        corners
    );
    assert_eq!(
        survey.below + survey.inside_40_90 + survey.above_90,
        corners
    );
    assert_eq!(survey.count, corners);
    assert_eq!(survey.unmeasurable, 0);
    assert!(survey.min_deg.is_finite());
    assert!(survey.max_deg >= survey.min_deg);
    assert!(survey.penalty.is_finite());
    assert!(survey.penalty >= 0.0);
}

#[test]
fn traced_quality_stages_split_eta_from_window_boundary() {
    let gates = HardGates::default();
    let limits = limits(40, 200_000);
    let mut mesh = sphere(6);
    let background_scale_m = median_cell_scale(mesh.state());
    let criteria = steep_target(&mesh);
    stalled_by_insertion_alone(&mut mesh, &criteria, gates, limits);

    let mut events = Vec::new();
    let mut sink = |event| {
        events.push(event);
        Ok(())
    };
    let mut trace = TraceEmitter::on(&mut sink, WindowBudgetAuditMode::Off);
    let (_, _, audit) = optimise_mesh_quality_with_natural_length(
        &mut mesh,
        &criteria,
        gates,
        limits,
        background_scale_m,
        true,
        NATURAL_LENGTH_PASSES,
        &mut trace,
    )
    .expect("optimise with trace");
    assert!(
        audit.window_committed > 0,
        "fixture must exercise the window phase"
    );

    let summaries = events
        .iter()
        .filter_map(|event| match event {
            HarpTraceEvent::StageSummary {
                stage,
                certification,
            } => Some((*stage, certification)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let post_eta = summaries
        .iter()
        .position(|(stage, _)| *stage == HarpTraceStage::PostEta)
        .expect("post_eta summary");
    let post_window = summaries
        .iter()
        .position(|(stage, _)| *stage == HarpTraceStage::PostWindow)
        .expect("post_window summary");
    assert!(post_eta < post_window);
    assert_ne!(summaries[post_eta].1, summaries[post_window].1);
    for stage in [
        HarpTraceStage::PostInitialLowDegree,
        HarpTraceStage::PostEta,
        HarpTraceStage::PostWindow,
        HarpTraceStage::PostFinalLowDegree,
    ] {
        let (_, certification) = summaries
            .iter()
            .find(|(seen, _)| *seen == stage)
            .expect("quality stage summary");
        assert!(certification
            .triangle_context_angle_exposure
            .keys()
            .any(|key| {
                key.frozen_gradated_target_gradient_bin != TargetGradientBin::Unavailable
            }));
    }
}

#[test]
fn window_budget_audit_is_default_off_for_traced_quality() {
    let gates = HardGates::default();
    let limits = limits(40, 200_000);
    let mut mesh = sphere(6);
    let background_scale_m = median_cell_scale(mesh.state());
    let criteria = small_target(&mesh);
    stalled_by_insertion_alone(&mut mesh, &criteria, gates, limits);

    let mut events = Vec::new();
    let mut sink = |event| {
        events.push(event);
        Ok(())
    };
    let mut trace = TraceEmitter::on(&mut sink, WindowBudgetAuditMode::Off);
    optimise_mesh_quality_with_natural_length(
        &mut mesh,
        &criteria,
        gates,
        limits,
        background_scale_m,
        true,
        NATURAL_LENGTH_PASSES,
        &mut trace,
    )
    .expect("optimise with ordinary trace");

    assert!(events.iter().all(|event| !matches!(
        event,
        HarpTraceEvent::WindowBudgetPassSummary(_) | HarpTraceEvent::WindowBudgetArmSummary(_)
    )));
}

#[test]
fn window_budget_audit_uses_fixed_s3_and_closes_the_cohort() {
    let gates = HardGates::default();
    let limits = limits(40, 200_000);
    let mut mesh = sphere(6);
    let background_scale_m = median_cell_scale(mesh.state());
    let criteria = small_target(&mesh);
    stalled_by_insertion_alone(&mut mesh, &criteria, gates, limits);

    let mut events = Vec::new();
    let mut sink = |event| {
        events.push(event);
        Ok(())
    };
    let mut trace = TraceEmitter::on(&mut sink, WindowBudgetAuditMode::Off);
    trace.enable_window_budget_audit();
    optimise_mesh_quality_with_natural_length(
        &mut mesh,
        &criteria,
        gates,
        limits,
        background_scale_m,
        true,
        NATURAL_LENGTH_PASSES,
        &mut trace,
    )
    .expect("optimise with window-budget audit");

    let arm_summaries = events
        .iter()
        .filter_map(|event| match event {
            HarpTraceEvent::WindowBudgetArmSummary(summary) => Some(summary),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(arm_summaries.len(), 3);
    assert_eq!(arm_summaries[0].arm, WindowBudgetArm::W32);
    assert_eq!(arm_summaries[1].arm, WindowBudgetArm::W64);
    assert_eq!(arm_summaries[2].arm, WindowBudgetArm::W96);

    let w32 = arm_summaries[0];
    let post_window = events
        .iter()
        .find_map(|event| match event {
            HarpTraceEvent::StageSummary {
                stage: HarpTraceStage::PostWindow,
                certification,
            } => Some(certification),
            _ => None,
        })
        .expect("production post-window summary");
    assert_eq!(
        w32.s4_total_violation_count,
        post_window.below_40_count + post_window.above_80_count,
        "W32 audit arm must match delivered S4 statistics"
    );

    for summary in &arm_summaries {
        assert_eq!(summary.s3_violation_key_count, w32.s3_violation_key_count);
        assert_eq!(
            summary.s4_resolved_s3_cohort_key_count + summary.s4_persisted_s3_cohort_key_count,
            summary.s3_violation_key_count,
            "{} S4 cohort denominator drifted",
            summary.arm.name()
        );
        assert_eq!(
            summary.s6_resolved_s3_cohort_key_count + summary.s6_persisted_s3_cohort_key_count,
            summary.s3_violation_key_count,
            "{} S6 cohort denominator drifted",
            summary.arm.name()
        );

        let passes = events
            .iter()
            .filter_map(|event| match event {
                HarpTraceEvent::WindowBudgetPassSummary(pass) if pass.arm == summary.arm => {
                    Some(pass)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let last = passes.last().expect("each arm must emit a pass summary");
        assert_eq!(summary.final_pass_found_sites, last.found_sites);
        assert_eq!(summary.final_pass_eligible_sites, last.eligible_sites);
        assert_eq!(summary.unique_sites_seen, last.unique_sites_seen);
        assert_eq!(
            summary.processed_site_slots,
            passes.iter().map(|pass| pass.processed_sites).sum()
        );
        assert_eq!(
            summary.total_line_search_attempt_count,
            passes
                .iter()
                .map(|pass| pass.line_search_attempt_count)
                .sum()
        );
        assert_eq!(
            summary.mean_processed_site_slots_per_unique_site,
            (summary.unique_sites_seen != 0)
                .then_some(summary.processed_site_slots as f64 / summary.unique_sites_seen as f64)
        );
        assert!(summary.s4_found_sites_never_processed_count <= summary.s4_found_sites);
    }

    let pass_summaries = events
        .iter()
        .filter_map(|event| match event {
            HarpTraceEvent::WindowBudgetPassSummary(summary) => Some(summary),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        !pass_summaries.is_empty(),
        "fixture must exercise window passes"
    );
    for summary in pass_summaries {
        assert_eq!(
            summary.resolved_s3_cohort_key_count + summary.persisted_s3_cohort_key_count,
            w32.s3_violation_key_count,
            "{} pass {} cohort denominator drifted",
            summary.arm.name(),
            summary.pass_index
        );
    }
}

#[test]
fn window_budget_prefix_guard_checks_pass_32_and_64() {
    let shared = state_fingerprint(&sphere(3));
    let divergent = state_fingerprint(&sphere(4));
    let result = |pass_count, pass_32_fingerprint, pass_64_fingerprint, s4_fingerprint| {
        WindowBudgetArmResult {
            pass_count,
            pass_32_fingerprint,
            pass_64_fingerprint,
            s4_fingerprint,
            s6_fingerprint: shared.clone(),
        }
    };
    let w32 = result(32, Some(shared.clone()), None, shared.clone());
    let w64 = result(
        64,
        Some(shared.clone()),
        Some(shared.clone()),
        shared.clone(),
    );
    let w96 = result(
        96,
        Some(shared.clone()),
        Some(shared.clone()),
        shared.clone(),
    );

    verify_window_budget_prefix(WindowBudgetArm::W32, &w32, 32, WindowBudgetArm::W64, &w64)
        .expect("W64 must reproduce W32 through pass 32");
    verify_window_budget_prefix(WindowBudgetArm::W64, &w64, 64, WindowBudgetArm::W96, &w96)
        .expect("W96 must reproduce W64 through pass 64");

    let divergent_w96 = result(96, Some(shared.clone()), Some(divergent), shared.clone());
    assert!(verify_window_budget_prefix(
        WindowBudgetArm::W64,
        &w64,
        64,
        WindowBudgetArm::W96,
        &divergent_w96,
    )
    .is_err());
}

#[test]
fn trace_emitter_caches_frozen_field_only_when_enabled() {
    let target = [1.0, 2.0, 3.0];
    let mut off = TraceEmitter::off();
    off.set_frozen_target_scales(&target);
    assert!(off.frozen_target_scales.is_none());

    let mut events = Vec::new();
    let mut sink = |event| {
        events.push(event);
        Ok(())
    };
    let mut on = TraceEmitter::on(&mut sink, WindowBudgetAuditMode::Off);
    on.set_frozen_target_scales(&target);
    assert_eq!(on.frozen_target_scales.as_deref(), Some(target.as_slice()));
}

#[test]
fn angle_window_penalties_keep_legacy_and_delivery_semantics_separate() {
    let mut survey = AngleWindowSurvey::default();
    record_angle_window(&mut survey, 30.0);
    record_angle_window(&mut survey, 100.0);

    assert_eq!(survey.legacy_penalty, 200.0);
    assert_eq!(survey.penalty, 500.0);
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
    // Kept at `steep_target`'s reach: shrinking it was measured to make this
    // test slower, 38s to 67s, not faster.
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
    assert!(outcome.report.quality_optimiser_moves > 0);
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
    let criteria = small_target(&mesh);
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
    let criteria = small_target(&mesh);
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
    let criteria = small_target(&mesh);
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
    let criteria = small_target(&mesh);
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
        let criteria = small_target(&mesh);
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
        let worst = state
            .active_vertex_slots()
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
    // Non-vacuity: the tighter budget has to actually cost cells. If all three
    // runs agree the equality above is true of a mesh that never reached any
    // bound, and the test asserts nothing.
    assert!(
        cells_seven < cells_nine,
        "a budget of seven cost nothing ({cells_seven} against {cells_nine}), so this fixture \
         never reaches the degree wall the claim is about"
    );
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
    // Kept at `steep_target`'s reach. A smaller patch never touches one of the
    // twelve pentagons, so `refusals.pentagon > 0` below stops holding -- the
    // wall this is about is simply not met, and the test would pass by never
    // reaching what it names.
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
///
/// The cycle ceiling is 80 against a measured 36. It used to be 40, which
/// sounds like a bound on a runaway but was really a 10 percent margin: the
/// unprotected arm converges in 36 cycles on macOS -- measured identical on
/// this commit and on 12078c4, so nothing in the refinement work moved it --
/// and exceeded 40 on Linux, failing the very assertion below. What separates
/// the two platforms is float detail, `acos` and FMA contraction, tipping
/// individual angle comparisons; it is not a difference in what the algorithm
/// does. A margin that thin cannot tell "converged" from "hit the ceiling",
/// which is the one thing this arm exists to check. 80 leaves that judgement
/// intact while staying a real bound: a platform needing more than twice the
/// measured count has diverged in behaviour, not in rounding, and should fail
/// here rather than be absorbed. The assertions carry the cycle count so the
/// next such failure reports how far off it was.
#[test]
fn protected_segments_make_a_quality_target_terminate() {
    use crate::criteria::MinAngle;
    const ANGLE: f64 = 20.0;

    let run = |protect: bool| {
        let base = TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25).expect("base");
        let mut mesh = AdaptiveMesh::from_triangular_mesh(&base).expect("adaptive");
        let coarsest = coarsest_scale(&mesh);
        let centre = LonLatDegrees::new(105.0, 35.0);
        // Left at 1,200 km deliberately: shrinking it was measured to make this
        // test *slower*, 73s to 113s. The work here is driven by the `MinAngle`
        // criterion below, which carries no region and so refines the whole
        // sphere; the protected circle's size does not reach it.
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
            for triangle in state.active_triangle_slots() {
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
            limits(80, 200_000),
        )
        .expect("run");
        let state = mesh.state();
        let worst = state
            .active_triangle_slots()
            .map(|triangle| {
                let c = state.triangles()[triangle];
                crate::criteria::smallest_angle_deg_for_test([
                    state.vertices()[c[0]],
                    state.vertices()[c[1]],
                    state.vertices()[c[2]],
                ])
            })
            .fold(f64::MAX, f64::min);
        (
            mesh.active_site_count(),
            worst,
            outcome.report.stop_reason,
            outcome.report.cycles_completed,
        )
    };

    let (sites, worst, stop, cycles) = run(true);
    assert!(
        sites < 2_000,
        "{sites} sites; it should converge, not run away"
    );
    assert!(
        worst >= ANGLE,
        "min triangle angle {worst:.2}; requested {ANGLE:.2}"
    );
    assert_eq!(
        stop,
        StopReason::NoAcceptedTransactions,
        "protected arm stopped after {cycles} of 80 cycles"
    );

    let (unprotected_sites, unprotected_worst, unprotected_stop, unprotected_cycles) = run(false);
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
        "unexpected unconstrained stop: {unprotected_stop:?} after \
         {unprotected_cycles} of 80 cycles (36 measured on macOS)"
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

/// Drive a mesh to where insertion alone has nothing left to place.
///
/// Deliberately without the r-adaptation phases: this is the state the recovery
/// pass exists to take further, and building it from the ladders alone keeps the
/// fixture from depending on the very phase under test.
fn stalled_by_insertion_alone(
    mesh: &mut AdaptiveMesh,
    criteria: &[Box<dyn CellCriterion>],
    gates: HardGates,
    limits: CycleLimits,
) -> BTreeSet<usize> {
    let policy = CandidatePolicy::default();
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
            if mesh
                .refine_cell(site, None, policy, gates)
                .expect("refine")
                .resolved()
                .is_some()
                || mesh
                    .refine_cell_fallback(site, policy, gates)
                    .expect("fallback")
                    .resolved()
                    .is_some()
            {
                accepted += 1;
            }
        }
        if accepted == 0 {
            break;
        }
    }
    pending_union(mesh, criteria, limits)
}

/// Every cell still asking, physical and balance together.
fn pending_union(
    mesh: &AdaptiveMesh,
    criteria: &[Box<dyn CellCriterion>],
    limits: CycleLimits,
) -> BTreeSet<usize> {
    let (demands, _, scales) = evaluate(mesh, criteria, limits).expect("evaluate");
    demands
        .iter()
        .chain(balance_demands(mesh, &scales, limits).iter())
        .map(|demand| demand.cell as usize)
        .collect()
}

#[test]
fn multi_ring_recovery_serves_a_residue_one_ring_cannot() {
    // Pinned, not defaulted. What is under test is that three rings of reach
    // recover a residue one ring cannot, and that needs a residue to exist:
    // with the shipped floor at zero this fixture serves every demand from the
    // insertion ladder alone and there is nothing left to recover. Twenty-five
    // is the floor that used to be the default and still makes it stall -- the
    // number is the fixture's, not a claim about what runs should use. It also
    // puts the `worst_triangle_floor` bound below back to work, which a zero
    // floor makes vacuous.
    let gates = HardGates {
        min_triangle_angle_deg: 25.0,
        ..HardGates::default()
    };
    let limits = limits(40, 200_000);
    let mut stalled = sphere(6);
    let criteria = steep_target(&stalled);
    let seeds = stalled_by_insertion_alone(&mut stalled, &criteria, gates, limits);
    assert!(
        !seeds.is_empty(),
        "the fixture has to actually stall, or there is nothing to recover"
    );

    // One ring reaches exactly as far as the shipped relief phases do: the cell
    // that asked and the neighbours it shares an edge with. Three rings is the
    // change under test, and the comparison is what says the extra reach is
    // what did the work rather than the sweep order.
    let mut one_ring = stalled.clone();
    let mut three_rings = stalled.clone();
    recover_stalled_regions(&mut one_ring, &criteria, gates, limits, &seeds, 1).expect("one ring");
    recover_stalled_regions(&mut three_rings, &criteria, gates, limits, &seeds, 3)
        .expect("three rings");

    let after_one = pending_union(&one_ring, &criteria, limits).len();
    let after_three = pending_union(&three_rings, &criteria, limits).len();
    assert!(
        after_three < seeds.len(),
        "three-ring recovery has to reduce the residue: {} -> {}",
        seeds.len(),
        after_three
    );
    assert!(after_three < after_one);

    // The bounds the recovery is not allowed to buy progress with.
    assert!(worst_degree(&three_rings) <= gates.max_vertex_degree);
    assert!(worst_triangle_floor(&three_rings) >= gates.min_triangle_angle_deg - 1e-9);
    assert_eq!(three_rings.state().open_edge_count(), 0);
    three_rings
        .state()
        .validate()
        .expect("still a triangulation");

    // Same input, same output.
    let mut again = stalled.clone();
    recover_stalled_regions(&mut again, &criteria, gates, limits, &seeds, 3).expect("again");
    assert_eq!(
        again.state(),
        three_rings.state(),
        "the recovery has to be deterministic"
    );
}

#[test]
fn target_scale_optimizer_improves_eta_tail_without_spending_harp_gates() {
    let gates = HardGates::default();
    let limits = limits(40, 200_000);
    let mut mesh = sphere(6);
    let background_scale_m = median_cell_scale(mesh.state());
    let criteria = small_target(&mesh);
    stalled_by_insertion_alone(&mut mesh, &criteria, gates, limits);
    let before = all_triangle_eta_values(mesh.state()).expect("eta");
    let angles_before = angle_window_survey(mesh.state());
    let pending_before = pending_union(&mesh, &criteria, limits).len();
    let unbalanced_before = balance_survey(&mesh, limits).0;
    let empty = BTreeSet::new();
    let (first_batch, found, eligible, _) = quality_problem_sites(&mesh, false, &empty, false);
    assert_eq!(found, eligible);
    let excluded: BTreeSet<_> = first_batch.iter().copied().collect();
    let (next_batch, found_again, eligible_after_skip, _) =
        quality_problem_sites(&mesh, false, &excluded, false);
    assert_eq!(found_again, found);
    assert_eq!(eligible_after_skip, found - excluded.len());
    assert!(next_batch.iter().all(|site| !excluded.contains(site)));

    let (moves, target_angles) =
        optimise_mesh_quality(&mut mesh, &criteria, gates, limits, background_scale_m)
            .expect("optimise");
    assert_eq!(target_angles.unmeasurable, 0);
    assert_eq!(target_angles.count, mesh.state().triangle_count() * 3);
    let after = all_triangle_eta_values(mesh.state()).expect("eta");
    let angles_after = angle_window_survey(mesh.state());

    assert!(
        moves > 0,
        "the fixture must exercise HARP's quality optimiser"
    );
    assert!(
        worst_first_eta_cmp(&after, &before) == Some(std::cmp::Ordering::Less),
        "quality optimisation must improve the lexicographic eta tail, got {:.6} -> {:.6}",
        before[0],
        after[0]
    );
    assert!(pending_union(&mesh, &criteria, limits).len() <= pending_before);
    assert!(balance_survey(&mesh, limits).0 <= unbalanced_before);
    assert!(
        angles_after.below + angles_after.above_80 < angles_before.below + angles_before.above_80,
        "quality optimisation must reduce 40-80 degree window violations: {} -> {}",
        angles_before.below + angles_before.above_80,
        angles_after.below + angles_after.above_80
    );
    assert!(
        angles_after.below + angles_after.above_80 <= 70,
        "the star-window direction must beat the former 72-violation plateau"
    );
    assert!(worst_degree(&mesh) <= gates.max_vertex_degree);
    assert!(worst_triangle_floor(&mesh) >= gates.min_triangle_angle_deg - 1e-9);
    assert_eq!(mesh.state().open_edge_count(), 0);
    mesh.state().validate().expect("still a triangulation");
}

#[test]
fn worst_first_eta_accepts_a_better_worst_triangle_despite_a_worse_average() {
    let before = [0.50, 0.95, 0.96];
    let after = [0.60, 0.90, 0.96];
    assert_eq!(
        worst_first_eta_cmp(&after, &before),
        Some(std::cmp::Ordering::Less)
    );
}

#[test]
fn target_angle_survey_measures_the_frozen_size_field() {
    let mesh = sphere(3);
    let criteria = target(coarsest_scale(&mesh) * 0.5, TargetRegion::Global);
    let target = target_cell_scales(mesh.state(), &criteria, 1.0, coarsest_scale(&mesh))
        .expect("target field");
    let survey = target_angle_window_survey(mesh.state(), &target);

    assert_eq!(survey.count, mesh.state().triangle_count() * 3);
    assert_eq!(survey.unmeasurable, 0);
    assert_eq!(survey.below, 0);
    assert_eq!(survey.above_80, 0);
    assert!((survey.max_deg - survey.min_deg).abs() < 1.0e-10);
    assert!(
        survey.min_deg > 60.0,
        "spherical equilateral angles exceed planar 60 degrees"
    );
}

#[test]
fn quality_score_prefers_window_margin_when_eta_is_no_worse() {
    let before = QualityScore {
        unresolved: 0,
        scale_violations: 0,
        worst_scale_ratio: 1.0,
        window_first: true,
        window_margin: vec![-10.0, 5.0],
        triangle_eta: vec![0.90, 0.95],
    };
    let after = QualityScore {
        unresolved: 0,
        scale_violations: 0,
        worst_scale_ratio: 1.0,
        window_first: true,
        window_margin: vec![-5.0, 5.0],
        triangle_eta: vec![0.90, 0.95],
    };

    assert_eq!(after.partial_cmp(&before), Some(std::cmp::Ordering::Less));
}

#[test]
fn quality_score_does_not_buy_eta_regression_with_window_margin() {
    let before = QualityScore {
        unresolved: 0,
        scale_violations: 0,
        worst_scale_ratio: 1.0,
        window_first: true,
        window_margin: vec![-10.0, 5.0],
        triangle_eta: vec![0.90, 0.95],
    };
    let after = QualityScore {
        unresolved: 0,
        scale_violations: 0,
        worst_scale_ratio: 1.0,
        window_first: true,
        window_margin: vec![-5.0, 5.0],
        triangle_eta: vec![0.80, 0.95],
    };

    assert_eq!(after.partial_cmp(&before), None);
}

#[test]
fn quality_score_rejects_a_better_worst_margin_that_increases_local_penalty() {
    let before = QualityScore {
        unresolved: 0,
        scale_violations: 0,
        worst_scale_ratio: 1.0,
        window_first: true,
        window_margin: vec![-10.0, -2.0, 5.0],
        triangle_eta: vec![0.80, 0.90, 0.95],
    };
    let after = QualityScore {
        unresolved: 0,
        scale_violations: 0,
        worst_scale_ratio: 1.0,
        window_first: true,
        window_margin: vec![-9.0, -8.0, 5.0],
        triangle_eta: vec![0.81, 0.90, 0.95],
    };

    assert_eq!(after.partial_cmp(&before), None);
}

#[test]
fn eta_phase_restores_the_existing_eta_first_semantics() {
    let before = QualityScore {
        unresolved: 0,
        scale_violations: 0,
        worst_scale_ratio: 1.0,
        window_first: false,
        window_margin: vec![-5.0],
        triangle_eta: vec![0.80],
    };
    let after = QualityScore {
        unresolved: 0,
        scale_violations: 0,
        worst_scale_ratio: 1.0,
        window_first: false,
        window_margin: vec![-6.0],
        triangle_eta: vec![0.90],
    };

    assert_eq!(after.partial_cmp(&before), Some(std::cmp::Ordering::Less));
}

#[test]
fn quality_score_keeps_unresolved_and_scale_as_vetoes() {
    let before = QualityScore {
        unresolved: 0,
        scale_violations: 0,
        worst_scale_ratio: 1.0,
        window_first: true,
        window_margin: vec![-10.0],
        triangle_eta: vec![0.90],
    };
    let after = QualityScore {
        unresolved: 1,
        scale_violations: 0,
        worst_scale_ratio: 1.0,
        window_first: true,
        window_margin: vec![10.0],
        triangle_eta: vec![1.0],
    };

    assert_eq!(after.partial_cmp(&before), None);
}

#[test]
fn quality_score_does_not_trade_fewer_scale_violations_for_a_worse_ratio() {
    let before = QualityScore {
        unresolved: 0,
        scale_violations: 2,
        worst_scale_ratio: 1.8,
        window_first: true,
        window_margin: vec![-10.0],
        triangle_eta: vec![0.90],
    };
    let after = QualityScore {
        unresolved: 0,
        scale_violations: 1,
        worst_scale_ratio: 1.9,
        window_first: true,
        window_margin: vec![10.0],
        triangle_eta: vec![1.0],
    };

    assert_eq!(after.partial_cmp(&before), None);
}

#[test]
fn quality_score_ignores_ratio_changes_inside_the_satisfied_bound() {
    let before = QualityScore {
        unresolved: 0,
        scale_violations: 0,
        worst_scale_ratio: 1.2,
        window_first: true,
        window_margin: vec![-10.0],
        triangle_eta: vec![0.90],
    };
    let after = QualityScore {
        unresolved: 0,
        scale_violations: 0,
        worst_scale_ratio: 1.3,
        window_first: true,
        window_margin: vec![-5.0],
        triangle_eta: vec![0.91],
    };

    assert_eq!(after.partial_cmp(&before), Some(std::cmp::Ordering::Less));
}

#[test]
fn low_degree_score_requires_a_real_deficit_reduction_without_quality_regression() {
    let before = LowDegreeScore {
        unresolved: 0,
        scale_violations: 0,
        worst_scale_ratio: 1.2,
        deficit: 1,
        window_margin: vec![-10.0, -2.0],
        triangle_eta: vec![0.80, 0.90],
    };
    let mut after = before.clone();
    after.deficit = 0;
    after.window_margin = vec![-9.0, -2.0];
    after.triangle_eta = vec![0.81, 0.90];
    assert_eq!(after.partial_cmp(&before), Some(std::cmp::Ordering::Less));

    after.triangle_eta[0] = 0.79;
    assert_eq!(after.partial_cmp(&before), None);
}

#[test]
fn degree_gain_candidates_are_the_vertices_across_a_low_degree_star() {
    let mesh = sphere(3);
    let centre = MESH_STATE_FIRST_ID;
    let neighbours = neighbour_sites(mesh.state(), centre);
    let candidates = degree_gain_quads(mesh.state(), centre);

    assert!(!candidates.is_empty());
    assert!(candidates
        .iter()
        .all(|(outside, tail, head)| !neighbours.contains(outside)
            && neighbours.contains(tail)
            && neighbours.contains(head)));
    assert!(candidates.iter().all(|&(outside, _, _)| outside != centre));
}

#[test]
fn target_field_is_continuous_and_reaches_every_site() {
    let mesh = sphere(6);
    let fine = coarsest_scale(&mesh) * 0.3;
    let centre = xyz_to_lonlat_degrees(mesh.state().vertices()[40]);
    let criteria = target(
        fine,
        TargetRegion::Circle {
            centre,
            radius_m: 1.0,
        },
    );
    let target = target_cell_scales(mesh.state(), &criteria, 1.0, coarsest_scale(&mesh))
        .expect("target field");

    assert!((target[40] - fine).abs() <= 1.0e-12 * fine);
    let target_angles = target_angle_window_survey(mesh.state(), &target);
    assert_eq!(target_angles.unmeasurable, 0);
    assert_eq!(target_angles.count, mesh.state().triangle_count() * 3);
    for site in mesh.state().active_vertex_slots() {
        for neighbour in neighbour_sites(mesh.state(), site) {
            let allowance = TARGET_SCALE_GRADIENT_LIMIT
                * earthmesh_mesh::arc_length_unit_sphere(
                    mesh.state().vertices()[site],
                    mesh.state().vertices()[neighbour],
                );
            let excess = (target[site] - target[neighbour]).abs() - allowance;
            assert!(
                excess <= 1.0e-7 * target[site].max(target[neighbour]),
                "edge {site}-{neighbour}: h=({:.17e},{:.17e}) allowance={allowance:.17e} exceeds by {excess:e}",
                target[site], target[neighbour]
            );
        }
    }
}

#[test]
fn target_field_and_natural_length_use_metres_once() {
    let mesh = sphere(6);
    let state = mesh.state();
    let site = 40;
    let neighbour = *neighbour_sites(state, site).first().expect("neighbour");
    let edge_m =
        earthmesh_mesh::arc_length_unit_sphere(state.vertices()[site], state.vertices()[neighbour]);
    assert!(
        edge_m > 100_000.0,
        "mesh arc lengths are already metres; multiplying by the radius again would be wrong"
    );

    let current = site_scale(state, site, state.sphere_radius()).expect("scale");
    let target = vec![current; state.vertices().len()];
    assert!(natural_length_destination(state, &target, site).is_some());
}

/// One natural-length step and what it did to the edges it was asked to fix.
///
/// # Why the plain mean edge length is not the direction to read
///
/// A single vertex on a fixed-radius sphere cannot move all of its incident
/// edges the same way: the isotropic part of the demand is purely radial, and
/// `projected_step` removes it exactly -- the radius comes back bit for bit.
/// What survives is tangential, and it is driven by the *spread* of the edge
/// errors rather than their common sign. Under a uniform stretch the largest
/// term is the shortest edge, so the site moves away from it; under a uniform
/// compression it is the longest edge, so the site moves toward it. On an
/// ordinary star those are nearly the same direction, and the unweighted mean
/// edge length falls in both cases. It carries no directional information.
///
/// The quantities that do are the relative length error the update descends and
/// the edge the field is most unhappy about.
struct NaturalLengthProbe {
    site: usize,
    desired_m: f64,
    mean_edge_before_m: f64,
    mean_edge_after_m: f64,
    error_before: f64,
    error_after: f64,
    /// The edge the field is most unhappy about, and what the step did to it.
    worst_edge_before_m: f64,
    worst_edge_after_m: f64,
    /// Edge lengths weighted by how far each one is from what the field asked.
    deficit_weighted_mean_before_m: f64,
    deficit_weighted_mean_after_m: f64,
    destination_chord_m: f64,
    step_chord_m: f64,
    radius_before_m: f64,
    radius_after_m: f64,
}

impl NaturalLengthProbe {
    /// Everything needed to tell a wrong direction from a wrong step size.
    fn report(&self) -> String {
        format!(
            "site {}; desired {:.3} m; mean edge {:.3} -> {:.3}; worst edge {:.3} -> {:.3}; \
             deficit-weighted mean {:.3} -> {:.3}; relative error {:.9} -> {:.9}; \
             destination {:.3} m away, projected step {:.3} m; radius {:.6} -> {:.6}",
            self.site,
            self.desired_m,
            self.mean_edge_before_m,
            self.mean_edge_after_m,
            self.worst_edge_before_m,
            self.worst_edge_after_m,
            self.deficit_weighted_mean_before_m,
            self.deficit_weighted_mean_after_m,
            self.error_before,
            self.error_after,
            self.destination_chord_m,
            self.step_chord_m,
            self.radius_before_m,
            self.radius_after_m,
        )
    }
}

fn chord_m(a: CartesianPoint, b: CartesianPoint) -> f64 {
    magnitude(CartesianPoint::new(a.x - b.x, a.y - b.y, a.z - b.z))
}

/// Take one small natural-length step under a constant target field.
///
/// `factor` is what the field asks every incident edge to become as a multiple
/// of its current mean: above one is a stretch, below one a compression. The
/// site is an ordinary six-neighbour vertex, so no base pentagon distorts the
/// star.
fn natural_length_probe(factor: f64) -> NaturalLengthProbe {
    let mesh = sphere(6);
    let state = mesh.state();
    let site = state
        .active_vertex_slots()
        .find(|&site| state.vertex_degree(site).ok() == Some(6))
        .expect("an ordinary six-neighbour site");
    let neighbours: Vec<usize> = neighbour_sites(state, site).into_iter().collect();
    let here = state.vertices()[site];
    let edges_from = |point: CartesianPoint| -> Vec<f64> {
        neighbours
            .iter()
            .map(|&neighbour| {
                earthmesh_mesh::arc_length_unit_sphere(point, state.vertices()[neighbour])
            })
            .collect()
    };

    let before = edges_from(here);
    let mean_edge_before_m = before.iter().sum::<f64>() / before.len() as f64;
    let desired_m = factor * mean_edge_before_m;
    let target = vec![desired_m / CELL_SCALE_TO_EDGE_LENGTH; state.vertices().len()];

    let destination =
        natural_length_destination(state, &target, site).expect("a natural-length destination");
    let stepped = projected_step(here, destination, 0.031_25).expect("a projected step");
    let after = edges_from(stepped);
    let relative_error = |lengths: &[f64]| -> f64 {
        lengths
            .iter()
            .map(|length| ((length - desired_m) / desired_m).powi(2))
            .sum()
    };

    // The relative error is a deficit-weighted objective: an edge already close
    // to what the field asked contributes almost nothing, and an edge far from
    // it dominates. Track the same weighting so the direction can be read off
    // the quantity the update actually descends.
    let weights: Vec<f64> = before
        .iter()
        .map(|length| (length - desired_m).abs())
        .collect();
    let weight_total = weights.iter().sum::<f64>();
    let deficit_weighted_mean = |lengths: &[f64]| -> f64 {
        lengths
            .iter()
            .zip(&weights)
            .map(|(length, weight)| length * weight)
            .sum::<f64>()
            / weight_total
    };
    let worst = weights
        .iter()
        .enumerate()
        .max_by(|left, right| left.1.total_cmp(right.1))
        .map(|(index, _)| index)
        .expect("an incident edge");

    NaturalLengthProbe {
        site,
        desired_m,
        mean_edge_before_m,
        mean_edge_after_m: after.iter().sum::<f64>() / after.len() as f64,
        error_before: relative_error(&before),
        error_after: relative_error(&after),
        worst_edge_before_m: before[worst],
        worst_edge_after_m: after[worst],
        deficit_weighted_mean_before_m: deficit_weighted_mean(&before),
        deficit_weighted_mean_after_m: deficit_weighted_mean(&after),
        destination_chord_m: chord_m(here, destination),
        step_chord_m: chord_m(here, stepped),
        radius_before_m: magnitude(here),
        radius_after_m: magnitude(stepped),
    }
}

#[test]
fn a_natural_length_step_lengthens_edges_the_field_wants_longer() {
    let probe = natural_length_probe(1.10);
    let report = probe.report();

    assert!(
        probe.desired_m > probe.mean_edge_before_m,
        "the stretch fixture did not ask for longer edges: {report}"
    );
    assert!(
        probe.worst_edge_after_m > probe.worst_edge_before_m,
        "the field asked for longer edges and the step shortened the shortest one: {report}"
    );
    assert!(
        probe.deficit_weighted_mean_after_m > probe.deficit_weighted_mean_before_m,
        "the step did not lengthen the edges the field is most unhappy about: {report}"
    );
    assert_natural_length_step_is_well_posed(&probe, &report);
}

#[test]
fn a_natural_length_step_shortens_edges_the_field_wants_shorter() {
    let probe = natural_length_probe(0.90);
    let report = probe.report();

    assert!(
        probe.desired_m < probe.mean_edge_before_m,
        "the compression fixture did not ask for shorter edges: {report}"
    );
    assert!(
        probe.worst_edge_after_m < probe.worst_edge_before_m,
        "the field asked for shorter edges and the step lengthened the longest one: {report}"
    );
    assert!(
        probe.deficit_weighted_mean_after_m < probe.deficit_weighted_mean_before_m,
        "the step did not shorten the edges the field is most unhappy about: {report}"
    );
    assert_natural_length_step_is_well_posed(&probe, &report);
}

/// What must hold whichever way the field asks the star to move.
fn assert_natural_length_step_is_well_posed(probe: &NaturalLengthProbe, report: &str) {
    assert!(
        probe.error_after < probe.error_before,
        "the step did not reduce the relative length error: {report}"
    );
    assert!(
        probe.step_chord_m < probe.destination_chord_m,
        "the line-search fraction did not shorten the move: {report}"
    );
    // The isotropic part of the demand is radial and the projection removes it,
    // so the star's overall size barely responds while the worst edge moves a
    // full step. Reading direction off the plain mean would read noise.
    assert!(
        (probe.mean_edge_after_m - probe.mean_edge_before_m).abs()
            < (probe.worst_edge_after_m - probe.worst_edge_before_m).abs(),
        "the step moved the star's overall size more than the edge it targeted: {report}"
    );
    assert!(
        (probe.radius_after_m - probe.radius_before_m).abs() <= 1.0e-12 * probe.radius_before_m,
        "the step left the sphere: {report}"
    );
}

#[test]
fn leaf_lineage_survey_keeps_strict_angle_ownership_separate_from_touching() {
    let mut mesh = sphere(6);
    let gates = permissive();
    let parent_vertex = 40;
    let parent_id = mesh.sites()[parent_vertex - MESH_STATE_FIRST_ID].site_id;
    let point = on(&mesh, 41.0, 19.0);
    let report = mesh
        .propose_site_for(point, None, gates, parent_vertex)
        .expect("proposal")
        .committed()
        .expect("the insertion commits")
        .clone();
    let target_scale = coarsest_scale(&mesh) * 2.0;
    let criteria = target(target_scale, TargetRegion::Global);

    let survey = leaf_lineage_survey(&mesh, &criteria);
    let child = &mesh.sites()[report.vertex - MESH_STATE_FIRST_ID];
    assert_eq!(child.parent_site_id, Some(parent_id));
    assert_eq!(survey.active_adaptive_sites, 1);
    assert_eq!(survey.active_leaf_sites, 1);
    assert_eq!(survey.interior_leaf_sites, 1);
    assert_eq!(survey.lineage_unknown_adaptive_sites, 0);
    assert_eq!(
        survey.leaf_degree_4
            + survey.leaf_degree_5
            + survey.leaf_degree_6
            + survey.leaf_degree_7
            + survey.leaf_degree_other,
        1
    );
    assert_eq!(survey.leaf_birth_cycle_min, 1);
    assert_eq!(survey.leaf_birth_cycle_max, 1);
    assert_eq!(survey.leaf_target_scale_measured, 1);
    assert!((survey.leaf_target_scale_min_m - target_scale).abs() <= 1.0e-12 * target_scale);
    assert!((survey.leaf_target_scale_max_m - target_scale).abs() <= 1.0e-12 * target_scale);
    let strict = survey.angles_below_40_at_leaf_vertices + survey.angles_above_80_at_leaf_vertices;
    if strict > 0 {
        assert!(survey.violating_triangles_touching_leaf > 0);
    }
}

#[test]
fn adaptive_sites_without_lineage_are_not_retirement_leaves() {
    let mut mesh = sphere(6);
    let point = on(&mesh, 41.0, 19.0);
    mesh.propose_site(point, permissive())
        .expect("proposal")
        .committed()
        .expect("the insertion commits");

    let survey = leaf_lineage_survey(&mesh, &[]);
    assert_eq!(survey.active_adaptive_sites, 1);
    assert_eq!(survey.lineage_unknown_adaptive_sites, 1);
    assert_eq!(survey.active_leaf_sites, 0);
    assert_eq!(survey.interior_leaf_sites, 0);
}

#[test]
fn a_recovery_that_improves_nothing_leaves_the_mesh_exactly_as_it_was() {
    let gates = HardGates::default();
    let limits = limits(40, 200_000);
    let mut mesh = sphere(4);
    let criteria = target(coarsest_scale(&mesh) * 2.0, TargetRegion::Global);
    let seeds: BTreeSet<usize> = (MESH_STATE_FIRST_ID..MESH_STATE_FIRST_ID + 6).collect();

    // Run it to convergence first. What is being tested is the state after the
    // sweeps have found everything they can find: from there every proposal is
    // legal and none of them dominates, so every one has to be rolled back.
    let mut rounds = 0;
    loop {
        let committed = recover_stalled_regions(&mut mesh, &criteria, gates, limits, &seeds, 3)
            .expect("recover");
        if committed == 0 {
            break;
        }
        rounds += 1;
        assert!(rounds < 32, "the recovery has to converge, not churn");
    }

    let converged = mesh.clone();
    let committed =
        recover_stalled_regions(&mut mesh, &criteria, gates, limits, &seeds, 3).expect("recover");
    assert_eq!(
        committed, 0,
        "a converged region has nothing left to commit"
    );
    assert_eq!(
        mesh.state(),
        converged.state(),
        "every proposal in that pass was rolled back, so the triangulation must compare equal"
    );
    assert!(worst_degree(&mesh) <= gates.max_vertex_degree);
    assert!(worst_triangle_floor(&mesh) >= gates.min_triangle_angle_deg - 1e-9);
    assert_eq!(mesh.state().open_edge_count(), 0);
    mesh.state().validate().expect("still a triangulation");
}

#[test]
fn a_region_score_includes_scale_ratios_crossing_its_boundary() {
    let mesh = sphere(6);
    let state = mesh.state();
    let limits = limits(1, 100_000);
    let criteria: Vec<Box<dyn CellCriterion>> = Vec::new();
    let gates = HardGates::default();
    let radius_m = state.sphere_radius();
    let scale = |site| {
        let cell = state.voronoi_cell(site).expect("cell");
        CellView {
            site,
            cell: &cell,
            state,
            radius_m,
        }
        .effective_scale_m()
        .expect("scale")
    };

    let (site, expected) = state
        .active_vertex_slots()
        .filter_map(|site| {
            let here = scale(site);
            let smaller: Vec<_> = neighbour_sites(state, site)
                .into_iter()
                .filter(|&neighbour| neighbour < site)
                .collect();
            (!smaller.is_empty()).then(|| {
                let worst = smaller
                    .into_iter()
                    .map(|neighbour| {
                        let there = scale(neighbour);
                        here.max(there) / here.min(there)
                    })
                    .fold(1.0_f64, f64::max);
                (site, worst)
            })
        })
        .max_by(|left, right| left.1.total_cmp(&right.1))
        .expect("physical sites");
    assert!(
        expected > 1.0,
        "the fixture needs a non-uniform edge to a smaller outside ID"
    );

    let region = [site].into_iter().collect();
    let score = region_score(
        state,
        &criteria,
        gates,
        limits,
        &region,
        &std::cell::RefCell::new(BTreeMap::new()),
        &std::cell::RefCell::new(BTreeMap::new()),
        &AffectedSites::new(),
    )
    .expect("score");
    assert!(
        (score.worst_ratio - expected).abs() < 1.0e-6,
        "the score reported {} but the edges crossing the region boundary reach {expected}",
        score.worst_ratio
    );
}

#[test]
fn a_region_score_cannot_buy_progress_with_a_worse_boundary_ratio() {
    let before = RegionScore {
        pending: 1,
        unresolved: 1,
        unbalanced: 0,
        saturated: 0,
        worst_ratio: 1.2,
        negated_min_angle_deg: -30.0,
    };
    let after = RegionScore {
        pending: 0,
        unresolved: 0,
        unbalanced: 0,
        saturated: 0,
        worst_ratio: 1.3,
        negated_min_angle_deg: -30.0,
    };

    assert_ne!(
        after.partial_cmp(&before),
        Some(std::cmp::Ordering::Less),
        "a recovery may not trade a worse boundary ratio for fewer demands"
    );
}

/// Where the surviving window violations come from.
///
/// The optimiser inserts nothing, so a site keeps its identity across it and
/// the two violation sets can be compared directly. Which way the comparison
/// falls decides whether a generation-stage change is worth prototyping: a
/// residue inherited from refinement is one that better point placement could
/// prevent, and a residue the optimiser manufactured is one it would not.
///
/// An owner is the corner the offending angle actually sits at, paired with
/// which side of the window it fell off. Counting every corner of a triangle
/// that holds one bad angle would measure something else -- whether the
/// residue stays in the same neighbourhood -- and would report more owners
/// than there are violating angles, which is how the first version of this
/// gave 100 owners for 65 angles. The pairing with the side matters too: a
/// site that owned a low angle and now owns a high one has not inherited
/// anything, it has swapped one defect for another.
#[test]
fn the_surviving_residue_is_attributed_to_refinement_or_to_optimisation() {
    fn violation_owners(mesh: &AdaptiveMesh) -> BTreeSet<(usize, bool)> {
        let state = mesh.state();
        let mut owners = BTreeSet::new();
        for triangle in state.active_triangle_slots() {
            let corners = state.triangles()[triangle];
            let Some(angles) = crate::criteria::triangle_angles_deg([
                state.vertices()[corners[0]],
                state.vertices()[corners[1]],
                state.vertices()[corners[2]],
            ]) else {
                continue;
            };
            for (corner, angle) in corners.into_iter().zip(angles) {
                if angle < PREFERRED_MINIMUM_ANGLE_DEG {
                    owners.insert((corner, false));
                } else if angle > PREFERRED_MAXIMUM_ANGLE_DEG {
                    owners.insert((corner, true));
                }
            }
        }
        owners
    }

    let gates = HardGates::default();
    let limits = limits(40, 200_000);
    let mut mesh = sphere(6);
    let background_scale_m = median_cell_scale(mesh.state());
    let criteria = small_target(&mesh);
    stalled_by_insertion_alone(&mut mesh, &criteria, gates, limits);

    let after_refinement = angle_window_survey(mesh.state());
    let owners_after_refinement = violation_owners(&mesh);
    let vertices_before = mesh.state().vertex_count();
    let sites_before = mesh.active_site_count();

    optimise_mesh_quality(&mut mesh, &criteria, gates, limits, background_scale_m)
        .expect("optimise");

    // The premise the comparison rests on. Relocation moves sites; it must not
    // create or retire them, or the two owner sets name different things.
    assert_eq!(mesh.state().vertex_count(), vertices_before);
    assert_eq!(mesh.active_site_count(), sites_before);

    let after_optimisation = angle_window_survey(mesh.state());
    let owners_after_optimisation = violation_owners(&mesh);
    let violations = after_optimisation.below + after_optimisation.above_80;
    assert!(
        owners_after_optimisation.len() <= violations,
        "an owner is one corner of one violating angle, so owners ({}) cannot exceed angles ({violations})",
        owners_after_optimisation.len()
    );

    let inherited: BTreeSet<_> = owners_after_optimisation
        .intersection(&owners_after_refinement)
        .copied()
        .collect();
    let manufactured: BTreeSet<_> = owners_after_optimisation
        .difference(&owners_after_refinement)
        .copied()
        .collect();

    eprintln!(
        "\n=== residue provenance ===\nangles: refinement {} -> optimisation {violations}",
        after_refinement.below + after_refinement.above_80
    );
    eprintln!(
        "violation owners (site, is_high): refinement {}, optimisation {}",
        owners_after_refinement.len(),
        owners_after_optimisation.len()
    );
    eprintln!(
        "    inherited from refinement: {} ({:.1}% of the survivors)",
        inherited.len(),
        100.0 * inherited.len() as f64 / owners_after_optimisation.len().max(1) as f64
    );
    eprintln!(
        "    manufactured by the optimiser: {} ({:.1}%)",
        manufactured.len(),
        100.0 * manufactured.len() as f64 / owners_after_optimisation.len().max(1) as f64
    );

    assert!(
        !owners_after_optimisation.is_empty(),
        "the fixture must still have a residue to attribute"
    );
    // The claim a generation-stage prototype would rest on. If the optimiser
    // is the one manufacturing the residue, placing points better cannot
    // prevent it, and this has to fail rather than be read past.
    assert!(
        inherited.len() > manufactured.len(),
        "the residue must be mostly inherited for refinement-stage placement to be the lever: \
         inherited {}, manufactured {}",
        inherited.len(),
        manufactured.len()
    );
}

/// Export the frozen target-scale field so an offline experiment can read the
/// same numbers the quality optimiser was given.
///
/// `#[ignore]` because it writes a file and answers nothing on its own.
///
/// # Why the positions have to travel with the values
///
/// `target_cell_scales` returns an array indexed by site, and it is computed
/// once, from the geometry as it stands when the optimiser starts. Sites move
/// afterwards; the value stays with the site. Reading the field back by pairing
/// post-optimisation positions with those values would advect the whole field,
/// so the export writes the position each value was computed at.
///
/// # Why the raw criterion is not a substitute
///
/// `TargetScale::target_scale_m_at` answers at any point on the sphere, but it
/// answers with the unlimited criterion value. The gradient limiting that makes
/// the field usable is a queue relaxation over the edges of *this* mesh
/// (`target_cell_scales` collects them from `active_triangle_slots`), so it
/// cannot be reproduced away from the mesh it was built on. An offline
/// generator that called the criterion directly would be working against a
/// steeper field than HARP ever saw, and the comparison would be worthless.
///
/// The consequence for how a result may be read: this field was gradient
/// limited on one particular mesh graph. A verdict obtained against it is a
/// verdict about *this field*, not about the criteria that produced it.
#[test]
#[ignore = "writes a CSV for the offline experiment; run explicitly"]
fn export_the_frozen_target_scale_field() {
    use std::io::Write;

    let gates = HardGates::default();
    let limits = limits(40, 200_000);
    let mut mesh = sphere(6);
    // Before refinement, exactly as the shipped fixture takes it. Section 11.64
    // records that re-reading the background after refinement gives a different
    // field and may not be mixed with production numbers.
    let background_scale_m = median_cell_scale(mesh.state()).expect("background scale");
    let criteria = steep_target(&mesh);
    stalled_by_insertion_alone(&mut mesh, &criteria, gates, limits);

    let state = mesh.state();
    let target = target_cell_scales(
        state,
        &criteria,
        limits.minimum_cell_width_m,
        background_scale_m,
    )
    .expect("frozen target field");

    let path = std::env::var("EARTHMESH_FROZEN_FIELD_CSV")
        .unwrap_or_else(|_| "frozen_target_field_nxp6.csv".to_string());
    let mut file = std::fs::File::create(&path).expect("create csv");
    writeln!(
        file,
        "# frozen target-scale field, captured where the quality optimiser reads it"
    )
    .expect("write");
    writeln!(
        file,
        "# sphere_radius_m={:.17e} background_scale_m={:.17e} gradient={}",
        state.sphere_radius(),
        background_scale_m,
        TARGET_SCALE_GRADIENT_LIMIT
    )
    .expect("write");
    writeln!(
        file,
        "# cell_scale_to_edge_length={CELL_SCALE_TO_EDGE_LENGTH:.17e}"
    )
    .expect("write");
    writeln!(file, "site_id,x,y,z,target_cell_scale_m").expect("write");

    let mut written = 0usize;
    for slot in state.active_vertex_slots() {
        let Some(site) = slot
            .checked_sub(MESH_STATE_FIRST_ID)
            .and_then(|row| mesh.sites().get(row))
        else {
            continue;
        };
        let point = state.vertices()[slot];
        writeln!(
            file,
            "{},{:.17e},{:.17e},{:.17e},{:.17e}",
            site.site_id.0, point.x, point.y, point.z, target[slot]
        )
        .expect("write");
        written += 1;
    }

    let survey = angle_window_survey(state);
    eprintln!(
        "wrote {written} rows to {path}; radius {:.6e}, window violations at capture {}",
        state.sphere_radius(),
        survey.below + survey.above_80
    );
    assert_eq!(written, mesh.active_site_count());
}

/// The full 2x2: candidate set against selection rule, plus the production
/// reference the control cell must reproduce.
///
/// Two axes move independently. An earlier version moved both at once, reported
/// that the frontal candidates won on every axis, and was wrong: the selection
/// rule was doing the work. Whether the two interact is a hypothesis this
/// matrix can test and a three-number comparison cannot.
///
/// The control cell is *asserted* equal to production, not merely printed
/// beside it. Without that, a later change to the shipped ladder would let the
/// control drift while the test still passed.
#[test]
#[ignore = "bounded frontal placement A/B; run explicitly"]
fn frontal_placement_ab_on_the_nxp6_proxy() {
    use super::frontal_prototype::{refine_with_frontal, rho_bar_for, Selection};

    struct Arm {
        sites: usize,
        refined_violations: usize,
        final_violations: usize,
        below: usize,
        above: usize,
        owners: usize,
        eta_min: f64,
        max_degree: usize,
        low_degree: usize,
        pending: usize,
        unbalanced: usize,
        mesh: AdaptiveMesh,
    }

    fn run(
        use_frontal: bool,
        selection: Selection,
        floor_angle_deg: f64,
        gates: HardGates,
        limits: CycleLimits,
    ) -> Arm {
        let mut mesh = sphere(6);
        let background_scale_m = median_cell_scale(mesh.state());
        let criteria = steep_target(&mesh);
        refine_with_frontal(
            &mut mesh,
            &criteria,
            gates,
            limits,
            use_frontal,
            selection,
            rho_bar_for(floor_angle_deg),
        );
        let refined = angle_window_survey(mesh.state());
        optimise_mesh_quality(&mut mesh, &criteria, gates, limits, background_scale_m)
            .expect("optimise");
        let after = angle_window_survey(mesh.state());
        let degrees: Vec<usize> = mesh
            .state()
            .active_vertex_slots()
            .filter_map(|site| mesh.state().vertex_degree(site).ok())
            .collect();
        Arm {
            sites: mesh.active_site_count(),
            refined_violations: refined.below + refined.above_80,
            final_violations: after.below + after.above_80,
            below: after.below,
            above: after.above_80,
            owners: violating_owner_count(&mesh),
            eta_min: all_triangle_eta_values(mesh.state()).expect("eta")[0],
            max_degree: degrees.iter().copied().max().unwrap_or(0),
            low_degree: degrees.iter().filter(|&&degree| degree < 5).count(),
            pending: pending_union(&mesh, &criteria, limits).len(),
            unbalanced: balance_survey(&mesh, limits).0,
            mesh,
        }
    }

    let gates = HardGates::default();
    let limits = limits(40, 200_000);

    let mut production = sphere(6);
    let production_background = median_cell_scale(production.state());
    let production_criteria = steep_target(&production);
    stalled_by_insertion_alone(&mut production, &production_criteria, gates, limits);
    let production_refined = angle_window_survey(production.state());
    optimise_mesh_quality(
        &mut production,
        &production_criteria,
        gates,
        limits,
        production_background,
    )
    .expect("optimise");
    let production_after = angle_window_survey(production.state());

    let cells = [
        (
            "first-survivor, shipped only ",
            run(false, Selection::FirstSurvivor, 40.0, gates, limits),
        ),
        (
            "first-survivor, + frontal    ",
            run(true, Selection::FirstSurvivor, 40.0, gates, limits),
        ),
        (
            "Better+leximin, shipped only ",
            run(false, Selection::BetterLeximin, 40.0, gates, limits),
        ),
        (
            "Better+leximin, + frontal    ",
            run(true, Selection::BetterLeximin, 40.0, gates, limits),
        ),
    ];

    eprintln!(
        "\nproduction reference: sites {}, refined {}, final {} (below {} above {})",
        production.active_site_count(),
        production_refined.below + production_refined.above_80,
        production_after.below + production_after.above_80,
        production_after.below,
        production_after.above_80
    );
    eprintln!("cell                            sites  refined  final  below  above  owners  eta_min   maxdeg  deg<5  pending  unbal");
    for (name, arm) in &cells {
        eprintln!(
            "{name}   {:5}  {:7}  {:5}  {:5}  {:5}  {:6}  {:.6}  {:6}  {:5}  {:7}  {:5}",
            arm.sites,
            arm.refined_violations,
            arm.final_violations,
            arm.below,
            arm.above,
            arm.owners,
            arm.eta_min,
            arm.max_degree,
            arm.low_degree,
            arm.pending,
            arm.unbalanced
        );
    }

    // The control cell is the shipped loop, so it must be the shipped result.
    let control = &cells[0].1;
    assert_eq!(control.sites, production.active_site_count());
    assert_eq!(
        control.refined_violations,
        production_refined.below + production_refined.above_80
    );
    assert_eq!(
        control.final_violations,
        production_after.below + production_after.above_80
    );
    assert_eq!(
        control.mesh.state(),
        production.state(),
        "the control cell must be the production mesh, not merely score like it"
    );

    for (name, arm) in &cells {
        arm.mesh.state().validate().expect("still a triangulation");
        assert_eq!(arm.mesh.state().open_edge_count(), 0, "{name}");
        assert!(worst_degree(&arm.mesh) <= gates.max_vertex_degree, "{name}");
        assert!(
            worst_triangle_floor(&arm.mesh) >= gates.min_triangle_angle_deg - 1e-9,
            "{name}"
        );
    }
}

/// How many `(site, is_high)` owners of out-of-window angles the mesh has.
fn violating_owner_count(mesh: &AdaptiveMesh) -> usize {
    let state = mesh.state();
    let mut owners = BTreeSet::new();
    for triangle in state.active_triangle_slots() {
        let corners = state.triangles()[triangle];
        let Some(angles) = crate::criteria::triangle_angles_deg([
            state.vertices()[corners[0]],
            state.vertices()[corners[1]],
            state.vertices()[corners[2]],
        ]) else {
            continue;
        };
        for (corner, angle) in corners.into_iter().zip(angles) {
            if angle < PREFERRED_MINIMUM_ANGLE_DEG {
                owners.insert((corner, false));
            } else if angle > PREFERRED_MAXIMUM_ANGLE_DEG {
                owners.insert((corner, true));
            }
        }
    }
    owners.len()
}

/// Is the requested floor angle a lever, in the one cell where frontal helps?
///
/// Single variable: only `theta_bar` moves. The interpretation was locked
/// before the run, so a number in the middle cannot be talked into a Go:
///
/// | result | verdict |
/// |---|---|
/// | any hard gate, pending or unbalanced regresses | that step is void |
/// | violations <= 32 | prototype Go |
/// | 33..=49 | directional gain, still No-Go; stop scanning, go to the oracle |
/// | >= 50 | theta_bar is not a lever; stop tuning |
///
/// Ties break on owners, then sites, then eta_min. Site growth must stay
/// within ten per cent either way.
#[test]
#[ignore = "parameter scan; run explicitly after section 11.66"]
fn the_floor_angle_scan_in_the_cell_where_frontal_helps() {
    use super::frontal_prototype::{refine_with_frontal, rho_bar_for, Selection};

    let gates = HardGates::default();
    let limits = limits(40, 200_000);
    let baseline_sites = 469usize;

    eprintln!("\ntheta_bar  sites  refined  final  below  above  owners  eta_min   deg<5  pending  unbal  growth");
    for floor_angle_deg in [40.0_f64, 45.0, 50.0] {
        let mut mesh = sphere(6);
        let background_scale_m = median_cell_scale(mesh.state());
        let criteria = steep_target(&mesh);
        let stats = refine_with_frontal(
            &mut mesh,
            &criteria,
            gates,
            limits,
            true,
            Selection::BetterLeximin,
            rho_bar_for(floor_angle_deg),
        );
        // "theta_bar is inert" is only a claim about the summary numbers until
        // the shape arm is shown never to win. It is the only path theta_bar
        // can influence.
        assert_eq!(
            stats.shape_chosen,
            stats.shape_clamped,
            "theta_bar {floor_angle_deg}: {} shape placements were not clamped to the \
             circumcentre, so theta_bar could have moved them",
            stats.shape_chosen - stats.shape_clamped
        );
        let refined = angle_window_survey(mesh.state());
        optimise_mesh_quality(&mut mesh, &criteria, gates, limits, background_scale_m)
            .expect("optimise");
        let after = angle_window_survey(mesh.state());
        let degrees: Vec<usize> = mesh
            .state()
            .active_vertex_slots()
            .filter_map(|site| mesh.state().vertex_degree(site).ok())
            .collect();
        eprintln!(
            "{floor_angle_deg:8.0}  {:5}  {:7}  {:5}  {:5}  {:5}  {:6}  {:.6}  {:5}  {:7}  {:5}  {:+.2}%",
            mesh.active_site_count(),
            refined.below + refined.above_80,
            after.below + after.above_80,
            after.below,
            after.above_80,
            violating_owner_count(&mesh),
            all_triangle_eta_values(mesh.state()).expect("eta")[0],
            degrees.iter().filter(|&&degree| degree < 5).count(),
            pending_union(&mesh, &criteria, limits).len(),
            balance_survey(&mesh, limits).0,
            100.0 * (mesh.active_site_count() as f64 / baseline_sites as f64 - 1.0)
        );
        eprintln!(
            "          cascade arms: size {} shape {} (clamped {}) fallback {}",
            stats.size_chosen, stats.shape_chosen, stats.shape_clamped, stats.fell_back
        );
        mesh.state().validate().expect("still a triangulation");
        assert_eq!(mesh.state().open_edge_count(), 0);
        assert!(worst_degree(&mesh) <= gates.max_vertex_degree);
        assert!(worst_triangle_floor(&mesh) >= gates.min_triangle_angle_deg - 1e-9);
    }
}

/// Diagnostic only: raising the transaction floor is not the same as asking
/// the final product to reach the delivery window.
#[test]
#[ignore = "bounded NXP6 comparison; run explicitly"]
fn transaction_floor_0_vs_25_vs_40_on_the_nxp6_proxy() {
    eprintln!("\nfloor  sites  committed  rolled_back  unresolved  quality_blocked  sliver_refusals  below40  above80  min_angle  max_angle  stop");

    for floor in [0.0_f64, 25.0, 40.0] {
        let mut mesh = sphere(6);
        let criteria = steep_target(&mesh);
        let outcome = run_cycles(
            &mut mesh,
            &criteria,
            CandidatePolicy::default(),
            HardGates {
                min_triangle_angle_deg: floor,
                ..HardGates::default()
            },
            limits(40, 200_000),
        )
        .expect("bounded run");
        let report = outcome.report;

        eprintln!(
            "{floor:5.0}  {:5}  {:9}  {:11}  {:10}  {:15}  {:15}  {:7}  {:7}  {:9.4}  {:9.4}  {:?}",
            report.final_sites,
            report.transactions_committed,
            report.transactions_rolled_back,
            report.unresolved_count,
            report.quality_constrained_count,
            report.refusals.sliver,
            report.angles_below_40_deg,
            report.angles_above_80_deg,
            report.angle_min_deg,
            report.angle_max_deg,
            report.stop_reason,
        );

        mesh.state().validate().expect("still a triangulation");
        assert_eq!(mesh.state().open_edge_count(), 0);
        assert!(worst_degree(&mesh) <= HardGates::default().max_vertex_degree);
    }
}

/// One arm of the natural-length A/B.
///
/// `enabled` removes the candidate outright rather than demoting it. Setting
/// only the priority passes to zero leaves it in the list behind the eta
/// ascent, which answers a different question -- whether the candidate is worth
/// anything at all once it stops pre-empting -- so both arms are needed.
#[derive(Clone, Copy, Debug)]
struct NaturalLengthArm {
    name: &'static str,
    enabled: bool,
    priority_passes: usize,
}

const NATURAL_LENGTH_ARMS: [NaturalLengthArm; 3] = [
    NaturalLengthArm {
        name: "OFF",
        enabled: false,
        priority_passes: 0,
    },
    NaturalLengthArm {
        name: "FALLBACK",
        enabled: true,
        priority_passes: 0,
    },
    NaturalLengthArm {
        name: "CURRENT",
        enabled: true,
        priority_passes: NATURAL_LENGTH_PASSES,
    },
];

#[test]
fn a_rejected_quality_pass_does_not_claim_retained_moves() {
    let mut audit = QualityMoveAudit::default();
    audit.generated(MoveSource::Natural);
    audit.attempted(MoveSource::Natural);
    let retained_before = audit.retained_commits();
    audit.committed(MoveSource::Natural);

    audit.restore_retained_commits(retained_before);

    assert_eq!(audit.natural_generated, 1);
    assert_eq!(audit.natural_line_search_attempts, 1);
    assert_eq!(audit.natural_committed, 0);
    assert_eq!(audit.retained_total(), 0);
}

/// Bit-for-bit identity of the geometry and topology an arm produced.
#[derive(Clone, Debug, PartialEq, Eq)]
struct MeshFingerprint {
    vertices: Vec<[u64; 3]>,
    triangles: Vec<[usize; 3]>,
    active_vertices: Vec<usize>,
    active_triangles: Vec<usize>,
}

fn mesh_fingerprint(state: &MeshState) -> MeshFingerprint {
    MeshFingerprint {
        vertices: state
            .vertices()
            .iter()
            .map(|point| [point.x.to_bits(), point.y.to_bits(), point.z.to_bits()])
            .collect(),
        triangles: state.triangles().to_vec(),
        active_vertices: state.active_vertex_slots().collect(),
        active_triangles: state.active_triangle_slots().collect(),
    }
}

/// What one arm finished with, in the fields the pre-registered read-out needs.
#[derive(Clone, Debug)]
struct NaturalLengthArmResult {
    arm: &'static str,
    sites: usize,
    moves: usize,
    audit: QualityMoveAudit,
    pending: usize,
    unbalanced: usize,
    worst_scale_ratio: f64,
    below_40: usize,
    above_80: usize,
    unmeasurable: usize,
    min_angle_deg: f64,
    max_angle_deg: f64,
    worst_deviation_deg: f64,
    eta_min: f64,
    eta_p1: f64,
    margin_min: f64,
    runtime_s: f64,
    open_edges: usize,
    max_degree: usize,
    fingerprint: MeshFingerprint,
}

impl NaturalLengthArmResult {
    /// The pre-registered primary metric.
    fn outside(&self) -> usize {
        self.below_40 + self.above_80
    }
}

/// Run one arm from a shared starting mesh and measure what it produced.
fn run_natural_length_arm(
    arm: NaturalLengthArm,
    start: &AdaptiveMesh,
    criteria: &[Box<dyn CellCriterion>],
    gates: HardGates,
    limits: CycleLimits,
    background_scale_m: Option<f64>,
) -> NaturalLengthArmResult {
    let mut mesh = start.clone();
    let started = std::time::Instant::now();
    let (moves, _, audit) = optimise_mesh_quality_with_natural_length(
        &mut mesh,
        criteria,
        gates,
        limits,
        background_scale_m,
        arm.enabled,
        arm.priority_passes,
        &mut TraceEmitter::off(),
    )
    .expect("the optimiser runs");
    let runtime_s = started.elapsed().as_secs_f64();

    mesh.state()
        .validate()
        .unwrap_or_else(|error| panic!("{} left an invalid triangulation: {error:?}", arm.name));
    let metrics = QualityGuardMetrics::read(&mesh, criteria, limits).expect("final metrics");
    let eta_p1 = metrics.eta[(metrics.eta.len() / 100).min(metrics.eta.len().saturating_sub(1))];
    NaturalLengthArmResult {
        arm: arm.name,
        sites: mesh.active_site_count(),
        moves,
        audit,
        pending: metrics.pending,
        unbalanced: metrics.unbalanced,
        worst_scale_ratio: metrics.worst_scale_ratio,
        below_40: metrics.angles.below,
        above_80: metrics.angles.above_80,
        unmeasurable: metrics.angles.unmeasurable,
        min_angle_deg: metrics.angles.min_deg,
        max_angle_deg: metrics.angles.max_deg,
        worst_deviation_deg: metrics.angles.worst_deviation_deg,
        eta_min: metrics.eta[0],
        eta_p1,
        margin_min: metrics.margins[0],
        runtime_s,
        open_edges: mesh.state().open_edge_count(),
        max_degree: worst_degree(&mesh),
        fingerprint: mesh_fingerprint(mesh.state()),
    }
}

fn audit_pending_checkpoint(
    mesh: &AdaptiveMesh,
    criteria: &[Box<dyn CellCriterion>],
    policy: CandidatePolicy,
    gates: HardGates,
    limits: CycleLimits,
) {
    let (physical, _, scales) = evaluate(mesh, criteria, limits).expect("physical demands");
    let balance = balance_demands(mesh, &scales, limits);
    let pending_sites: BTreeSet<usize> = physical
        .iter()
        .chain(balance.iter())
        .map(|demand| demand.cell as usize)
        .collect();
    eprintln!(
        "\nPENDING AUDIT: {} physical demands, {} balance demands, {} unique sites",
        physical.len(),
        balance.len(),
        pending_sites.len()
    );

    let mut ordinary_resolved = 0usize;
    let mut fallback_resolved = 0usize;
    let mut blocked = 0usize;
    let mut refusal_tally = RejectionTally::default();
    for demand in physical.iter().chain(balance.iter()) {
        let site = demand.cell as usize;
        let point = xyz_to_lonlat_degrees(mesh.state().vertices()[site]);
        let degree = mesh.state().vertex_degree(site).unwrap_or(0);
        let witness = demand.preferred_witness.map(|witness| {
            let unit = lonlat_degrees_to_unit_xyz(witness);
            let radius = magnitude(mesh.state().vertices()[site]);
            CartesianPoint::new(unit.x * radius, unit.y * radius, unit.z * radius)
        });
        let evidence = demand
            .evidences
            .iter()
            .filter(|evidence| evidence.demands_work())
            .map(|evidence| {
                format!(
                    "{}:{:.6}>{:.6} ({:+.3}%)",
                    evidence.criterion_id,
                    evidence.measured_value,
                    evidence.threshold,
                    evidence.normalized_violation * 100.0
                )
            })
            .collect::<Vec<_>>()
            .join(";");

        let mut probe = mesh.clone();
        let (outcome, reasons) = match probe
            .refine_cell(site, witness, policy, gates)
            .expect("ordinary diagnostic transaction")
        {
            DemandOutcome::Resolved { .. } => {
                ordinary_resolved += 1;
                ("ordinary-resolves", Vec::new())
            }
            DemandOutcome::Unresolved { refusals, .. } => {
                match probe
                    .refine_cell_fallback(site, policy, gates)
                    .expect("fallback diagnostic transaction")
                {
                    DemandOutcome::Resolved { .. } => {
                        fallback_resolved += 1;
                        ("fallback-resolves", refusals)
                    }
                    DemandOutcome::Unresolved {
                        refusals: fallback, ..
                    } => {
                        let mut combined = refusals;
                        combined.extend(fallback);
                        blocked += 1;
                        ("blocked", combined)
                    }
                    DemandOutcome::NotAttempted(_) => {
                        blocked += 1;
                        ("fallback-not-attempted", refusals)
                    }
                }
            }
            DemandOutcome::NotAttempted(_) => {
                blocked += 1;
                ("ordinary-not-attempted", Vec::new())
            }
        };
        let mut local_tally = RejectionTally::default();
        tally_refusals(&reasons, &mut local_tally);
        tally_refusals(&reasons, &mut refusal_tally);
        eprintln!(
            "site {site:6} lon {:+10.5} lat {:+9.5} degree {degree} cause {:?} => {outcome}; \
             evidence [{evidence}]; refusals {:?}",
            point.lon_degrees, point.lat_degrees, demand.cause, local_tally
        );
    }
    eprintln!(
        "PENDING AUDIT SUMMARY: ordinary_resolved={ordinary_resolved} \
         fallback_resolved={fallback_resolved} blocked={blocked} refusals={refusal_tally:?}"
    );
}

/// Does the natural-length candidate earn its place in the 40-80 window?
///
/// Same production pre-optimiser checkpoint, same gates, same limits, same
/// frozen background scale, same line search: the only thing that differs
/// between the arms is whether the candidate is generated and how early it is
/// tried. The read-out is pre-registered in
/// `.omx/plans/harp-natural-length-ab.md`.
#[test]
#[ignore = "bounded natural-length A/B; run explicitly"]
fn natural_length_ab_on_the_nxp_proxy() {
    let nxp = std::env::var("EARTHMESH_TEST_NXP")
        .ok()
        .map(|value| {
            value
                .parse::<usize>()
                .expect("EARTHMESH_TEST_NXP must be an integer")
        })
        .unwrap_or(12);
    assert!(nxp >= 6, "EARTHMESH_TEST_NXP must be at least 6, got {nxp}");

    let gates = HardGates::default();
    let max_cycles = std::env::var("EARTHMESH_TEST_MAX_CYCLES")
        .ok()
        .map(|value| {
            value
                .parse::<u32>()
                .expect("EARTHMESH_TEST_MAX_CYCLES must be an integer")
        })
        .unwrap_or(40);
    let limits = limits(max_cycles, 200_000);
    let mut production = sphere(nxp);
    // Frozen on the input mesh, exactly as `run_cycles` does it. The checkpoint
    // itself is captured after production refinement and r-adaptation, at the
    // line where production would call the quality optimiser.
    let background_scale_m = median_cell_scale(production.state());
    let criteria = steep_target(&production);
    let start = production_quality_checkpoint(
        &mut production,
        &criteria,
        CandidatePolicy::default(),
        gates,
        limits,
    );
    let before = QualityGuardMetrics::read(&start, &criteria, limits).expect("starting metrics");
    start
        .state()
        .validate()
        .expect("production checkpoint is a triangulation");
    assert_eq!(start.state().open_edge_count(), 0);
    assert!(worst_degree(&start) <= gates.max_vertex_degree);
    if std::env::var_os("EARTHMESH_TEST_LOCATE_PENDING").is_some() {
        audit_pending_checkpoint(&start, &criteria, CandidatePolicy::default(), gates, limits);
        return;
    }
    assert_eq!(
        before.pending, 0,
        "A/B requires a completed production refinement checkpoint"
    );
    assert_eq!(
        before.unbalanced, 0,
        "A/B requires a balanced production refinement checkpoint"
    );
    eprintln!(
        "\nnxp {nxp}: production pre-optimiser checkpoint has {} sites, {} pending, {} unbalanced, \
         below40 {}, above80 {}",
        start.active_site_count(),
        before.pending,
        before.unbalanced,
        before.angles.below,
        before.angles.above_80
    );

    let requested_arm = std::env::var("EARTHMESH_TEST_ARM").ok();
    if let Some(requested) = &requested_arm {
        assert!(
            NATURAL_LENGTH_ARMS.iter().any(|arm| arm.name == requested),
            "EARTHMESH_TEST_ARM must be OFF, FALLBACK, or CURRENT"
        );
    }
    let selected_arms: Vec<NaturalLengthArm> = NATURAL_LENGTH_ARMS
        .into_iter()
        .filter(|arm| {
            requested_arm
                .as_deref()
                .is_none_or(|requested| arm.name == requested)
        })
        .collect();
    let results: Vec<NaturalLengthArmResult> = selected_arms
        .iter()
        .copied()
        .map(|arm| {
            run_natural_length_arm(arm, &start, &criteria, gates, limits, background_scale_m)
        })
        .collect();

    eprintln!(
        "\nnxp arm       sites moves nat_c eta_c win_c pending unbal worst_ratio below40 above80 \
         outside min_angle max_angle worst_dev  eta_min   eta_p1 margin_min runtime_s"
    );
    for result in &results {
        eprintln!(
            "{nxp:3} {:<9} {:5} {:5} {:5} {:5} {:5} {:7} {:5} {:11.5} {:7} {:7} {:7} {:9.4} \
             {:9.4} {:9.4} {:8.6} {:8.6} {:10.6} {:9.1}",
            result.arm,
            result.sites,
            result.moves,
            result.audit.natural_committed,
            result.audit.eta_committed,
            result.audit.window_committed,
            result.pending,
            result.unbalanced,
            result.worst_scale_ratio,
            result.below_40,
            result.above_80,
            result.outside(),
            result.min_angle_deg,
            result.max_angle_deg,
            result.worst_deviation_deg,
            result.eta_min,
            result.eta_p1,
            result.margin_min,
            result.runtime_s
        );
    }
    eprintln!("\narm       candidate generated/line-search/committed");
    for result in &results {
        eprintln!(
            "{:<9} natural {}/{}/{}  eta {}/{}/{}  window {}/{}/{}",
            result.arm,
            result.audit.natural_generated,
            result.audit.natural_line_search_attempts,
            result.audit.natural_committed,
            result.audit.eta_generated,
            result.audit.eta_line_search_attempts,
            result.audit.eta_committed,
            result.audit.window_generated,
            result.audit.window_line_search_attempts,
            result.audit.window_committed
        );
    }

    for result in &results {
        let arm = result.arm;
        assert_eq!(
            result.audit.retained_total(),
            result.moves,
            "{arm} audit does not match the moves retained in the mesh"
        );
        assert_eq!(result.open_edges, 0, "{arm} left the surface open");
        assert!(
            result.max_degree <= gates.max_vertex_degree,
            "{arm} left a vertex of degree {}",
            result.max_degree
        );
        assert_eq!(
            result.unmeasurable, 0,
            "{arm} left {} unmeasurable triangles",
            result.unmeasurable
        );
        assert!(
            result.pending <= before.pending,
            "{arm} left {} pending demands, up from {}",
            result.pending,
            before.pending
        );
        assert!(
            result.unbalanced <= before.unbalanced,
            "{arm} left {} unbalanced pairs, up from {}",
            result.unbalanced,
            before.unbalanced
        );
    }

    // The arms are only comparable if each one is a function of its input.
    // The high-cost proxy may skip the duplicate timing run after this property
    // has been locked on NXP12; the A/B itself is unchanged.
    if std::env::var_os("EARTHMESH_TEST_SKIP_REPEAT").is_none() {
        for arm in selected_arms {
            let repeat =
                run_natural_length_arm(arm, &start, &criteria, gates, limits, background_scale_m);
            let first = results
                .iter()
                .find(|result| result.arm == arm.name)
                .expect("the arm ran");
            assert!(
                repeat.fingerprint == first.fingerprint,
                "{} is not deterministic: the repeat run produced a different mesh",
                arm.name
            );
            assert_eq!(
                repeat.audit, first.audit,
                "{} is not deterministic: the repeat run committed different candidates",
                arm.name
            );
        }
    }
}

/// The arms must differ in the natural-length candidate and nothing else.
///
/// The A/B above is bounded and ignored by default, so this is what keeps the
/// wiring honest: OFF has to remove the candidate from every phase rather than
/// merely demote it, and CURRENT has to actually generate one. A silently
/// disabled candidate would leave both arms identical and every later reading
/// of the experiment wrong.
#[test]
fn the_natural_length_arms_differ_only_in_the_natural_candidate() {
    let gates = HardGates::default();
    let limits = limits(40, 200_000);
    let mut production = sphere(6);
    let background_scale_m = median_cell_scale(production.state());
    // A narrower patch than `steep_target`'s. What this pins is the wiring --
    // that OFF removes the candidate from every phase and CURRENT still makes
    // one -- and a smaller refined region reaches a converged checkpoint for a
    // fraction of the debug-build runtime.
    let criteria = target(
        coarsest_scale(&production) * 0.4,
        TargetRegion::Circle {
            centre: LonLatDegrees::new(105.0, 35.0),
            radius_m: 500_000.0,
        },
    );
    let start = production_quality_checkpoint(
        &mut production,
        &criteria,
        CandidatePolicy::default(),
        gates,
        limits,
    );
    let checkpoint = QualityGuardMetrics::read(&start, &criteria, limits).expect("checkpoint");
    assert_eq!(checkpoint.pending, 0);
    assert_eq!(checkpoint.unbalanced, 0);

    let off = run_natural_length_arm(
        NATURAL_LENGTH_ARMS[0],
        &start,
        &criteria,
        gates,
        limits,
        background_scale_m,
    );
    let current = run_natural_length_arm(
        NATURAL_LENGTH_ARMS[2],
        &start,
        &criteria,
        gates,
        limits,
        background_scale_m,
    );

    assert_eq!(
        (
            off.audit.natural_generated,
            off.audit.natural_line_search_attempts,
            off.audit.natural_committed
        ),
        (0, 0, 0),
        "OFF still reached the natural-length candidate: {:?}",
        off.audit
    );
    assert!(
        current.audit.natural_generated > 0,
        "CURRENT never generated a natural-length candidate: {:?}",
        current.audit
    );
    assert!(
        current.audit.natural_line_search_attempts > 0,
        "CURRENT generated natural-length destinations but never stepped along one: {:?}",
        current.audit
    );
    // Both arms must still be doing the rest of the work, or "OFF is worse" is
    // a statement about a broken optimiser rather than about the candidate.
    assert!(off.audit.eta_generated > 0 && current.audit.eta_generated > 0);
    assert!(off.moves > 0 && current.moves > 0);
}

// ---------------------------------------------------------------------------
// Reference implementations for the full-cell-sweep performance work.
//
// These are verbatim copies of the per-site logic the optimisation replaces.
// They exist because the mesh's own tests all stayed green across an earlier
// rewrite that would have changed results (guide 11.3): passing tests are not
// equivalence, and only a cell-for-cell comparison against the old code can
// tell a faster implementation from a different one.
//
// Nothing in production may call these.
// ---------------------------------------------------------------------------

/// The pre-optimisation `state_scales`: one scanned Voronoi cell per site.
fn reference_state_scales(state: &MeshState) -> Vec<Option<f64>> {
    let radius_m = state.sphere_radius();
    let mut scales = vec![None; state.vertices().len()];
    for site in state.active_vertex_slots() {
        let Ok(cell) = state.voronoi_cell(site) else {
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

/// The pre-optimisation `balance_demands`: a scanned fan per site.
fn reference_balance_demands(
    state: &MeshState,
    scales: &[Option<f64>],
    limits: CycleLimits,
) -> Vec<RefinementDemand> {
    let mut worst: BTreeMap<usize, f64> = BTreeMap::new();
    for site in state.active_vertex_slots() {
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
    balance_demands_from_worst(worst, limits)
}

/// The pre-optimisation `balance_survey_state`, duplicate counting included.
fn reference_balance_survey(state: &MeshState, limits: CycleLimits) -> (usize, f64) {
    let scales = reference_state_scales(state);
    let mut over = 0;
    let mut worst = 1.0_f64;
    for site in state.active_vertex_slots() {
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
                let ratio = here.max(there) / here.min(there);
                worst = worst.max(ratio);
                if ratio > limits.max_neighbour_scale_ratio {
                    over += 1;
                }
            }
        }
    }
    (over, worst)
}

/// The pre-optimisation retirement candidate list, order included.
fn reference_retirement_candidates(
    state: &MeshState,
    leaves: &LeafLineageSurvey,
    maximum_degree: usize,
) -> Vec<usize> {
    let mut candidates = state
        .active_vertex_slots()
        .filter(|&site| {
            let degree = state.vertex_degree(site).ok();
            leaves.interior_leaf.get(site).copied().unwrap_or(false)
                && degree.is_some_and(|degree| (3..=maximum_degree).contains(&degree))
                && state.triangle_fan(site).is_ok_and(|fan| {
                    fan.into_iter().any(|triangle| {
                        triangle_window_margin(state, triangle).is_some_and(|margin| margin < 0.0)
                    })
                })
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|&left, &right| {
        let worst = |site| {
            state
                .triangle_fan(site)
                .ok()
                .into_iter()
                .flatten()
                .filter_map(|triangle| triangle_window_margin(state, triangle))
                .fold(f64::INFINITY, f64::min)
        };
        worst(left)
            .total_cmp(&worst(right))
            .then_with(|| left.cmp(&right))
    });
    candidates
}

/// The pre-optimisation `demanded_cells_in_state`: a scanned cell per site.
fn reference_demanded_cells_in_state(
    state: &MeshState,
    criteria: &[Box<dyn CellCriterion>],
) -> Option<usize> {
    let radius_m = state.sphere_radius();
    let mut demanded = 0;
    for site in state.active_vertex_slots() {
        let cell = state.voronoi_cell(site).ok()?;
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

/// The pre-optimisation degree-below-five set: a scanned fan per site.
fn reference_vertices_below_degree_5(state: &MeshState) -> BTreeSet<usize> {
    state
        .active_vertex_slots()
        .filter(|&site| state.vertex_degree(site).is_ok_and(|degree| degree < 5))
        .collect()
}

/// Meshes the sweeps have to agree on, including the awkward ones.
///
/// A tombstone is in the list because every sweep indexes by vertex slot: a
/// retirement leaves a hole in that table, and an implementation that walks
/// triangles rather than slots can silently shift every index after it.
fn full_sweep_fixtures() -> Vec<(&'static str, AdaptiveMesh)> {
    let mut fixtures = Vec::new();

    fixtures.push(("base sphere(6)", sphere(6)));

    let mut inserted = sphere(6);
    let criteria = steep_target(&inserted);
    let gates = permissive();
    let limits = limits(4, 100_000);
    stalled_by_insertion_alone(&mut inserted, &criteria, gates, limits);
    fixtures.push(("after insertions", inserted.clone()));

    let mut moved = inserted.clone();
    let objective = |_: &MeshState, _: &AffectedSites| Some(0usize);
    let site = moved
        .state()
        .active_vertex_slots()
        .find(|&site| moved.can_move_site(site))
        .expect("a movable site");
    let here = moved.state().vertices()[site];
    let neighbour = *neighbour_sites(moved.state(), site)
        .first()
        .expect("a neighbour");
    let there = moved.state().vertices()[neighbour];
    if let Some(destination) = projected_step(here, there, 0.125) {
        let _ = moved.propose_move_cached(site, destination, gates, &objective, None, false);
    }
    fixtures.push(("after a move and legalize", moved));

    // Any leaf will do; which one retires depends on whether the ring it
    // leaves behind can be retriangulated at all, so try them in turn.
    let mut retired = inserted;
    let leaves: Vec<usize> = retired
        .state()
        .active_vertex_slots()
        .filter(|&site| retired.is_retirable_leaf(site))
        .collect();
    let tombstoned = leaves.into_iter().any(|leaf| {
        retired
            .retire_leaf_transactionally(leaf, |_, _| true)
            .is_ok()
    });
    assert!(
        tombstoned,
        "the tombstone fixture needs at least one leaf that retires"
    );
    fixtures.push(("after a retirement leaves a tombstone", retired));

    fixtures
}

#[test]
fn full_cell_sweeps_match_the_scanned_reference() {
    let limits = limits(4, 100_000);
    for (name, mesh) in full_sweep_fixtures() {
        let state = mesh.state();

        let reference = reference_state_scales(state);
        let produced = state_scales(state);
        assert_eq!(
            produced.len(),
            reference.len(),
            "{name}: the scale table changed length"
        );
        for (site, (produced, reference)) in produced.iter().zip(&reference).enumerate() {
            assert_eq!(
                produced.map(f64::to_bits),
                reference.map(f64::to_bits),
                "{name}: site {site} scale differs from the scanned reference"
            );
        }

        let produced = balance_demands(&mesh, &reference, limits);
        let expected = reference_balance_demands(state, &reference, limits);
        assert_eq!(
            produced.len(),
            expected.len(),
            "{name}: balance demand count differs"
        );
        for (produced, expected) in produced.iter().zip(&expected) {
            assert_eq!(
                produced.cell, expected.cell,
                "{name}: balance demand order or site differs"
            );
        }

        let produced = balance_survey_state(state, limits);
        let expected = reference_balance_survey(state, limits);
        assert_eq!(
            (produced.0, produced.1.to_bits()),
            (expected.0, expected.1.to_bits()),
            "{name}: balance survey differs from the scanned reference"
        );

        assert_eq!(
            vertices_below_degree_5_set(state),
            reference_vertices_below_degree_5(state),
            "{name}: the degree-below-five set differs from the scanned reference"
        );

        // The seeded scale, with the radius passed in rather than recomputed
        // per site, against the scanned original.
        let seeds = active_site_triangle_seeds(state);
        let radius_m = state.sphere_radius();
        for site in state.active_vertex_slots() {
            let Some(seed) = seeds[site] else { continue };
            assert_eq!(
                site_scale_from(state, site, seed, radius_m).map(f64::to_bits),
                reference_site_scale(state, site).map(f64::to_bits),
                "{name}: site {site} seeded scale differs from the scanned reference"
            );
        }

        // The merged retirement sweep has to answer both guards exactly as the
        // two separate sweeps did.
        let criteria = steep_target(&mesh);
        let (merged_count, merged_scales) = demanded_cells_and_scales(state, &criteria);
        assert_eq!(
            merged_count,
            demanded_cells_in_state(state, &criteria),
            "{name}: the merged sweep counts different demanded cells"
        );
        for (site, (merged, reference)) in merged_scales.iter().zip(&reference).enumerate() {
            assert_eq!(
                merged.map(f64::to_bits),
                reference.map(f64::to_bits),
                "{name}: site {site} scale from the merged sweep differs"
            );
        }
    }
}

#[test]
fn retirement_candidate_order_matches_the_scanned_reference() {
    let criteria = |mesh: &AdaptiveMesh| steep_target(mesh);
    let mut longest = 0usize;
    for (name, mesh) in full_sweep_fixtures() {
        let leaves = leaf_lineage_survey(&mesh, &criteria(&mesh));
        for maximum_degree in [4usize, 7] {
            let produced = retirement_candidates(mesh.state(), &leaves, maximum_degree);
            let expected = reference_retirement_candidates(mesh.state(), &leaves, maximum_degree);
            assert_eq!(
                produced, expected,
                "{name}, maximum degree {maximum_degree}: candidate list or order differs"
            );
            longest = longest.max(produced.len());
        }
    }
    // Ordering is the thing this test exists to pin, and two equal empty lists
    // would pin nothing.
    assert!(
        longest > 1,
        "the fixtures produced no candidate list long enough to have an order"
    );
}

#[test]
fn quality_leaf_retirement_schedules_and_commits_a_degree_three_leaf() {
    let mut mesh = sphere(4);
    let report = mesh
        .propose_site_for(on(&mesh, -170.0, -5.0), None, permissive(), 20)
        .expect("degree-three proposal")
        .committed()
        .expect("degree-three proposal commits")
        .clone();
    let leaf = report.vertex;
    let site_id = report.site_id;
    assert_eq!(mesh.state().vertex_degree(leaf), Ok(3));
    let certification = crate::certifier::certify_mesh(&mesh, &[]);
    let owned_violations = certification
        .violations
        .iter()
        .filter(|violation| violation.corner_vertex == leaf)
        .collect::<Vec<_>>();
    assert_eq!(owned_violations.len(), 3);
    assert!(owned_violations
        .iter()
        .all(|violation| violation.triangle_degree_triplet == [3, 7, 7]));

    let criteria = Vec::<Box<dyn CellCriterion>>::new();
    let leaves = leaf_lineage_survey(&mesh, &criteria);
    assert!(leaves.interior_leaf[leaf]);
    assert_eq!(retirement_candidates(mesh.state(), &leaves, 3), [leaf]);
    let before_low_degree = vertices_below_degree_5_set(mesh.state());
    let before_angles = angle_window_survey(mesh.state());

    let counts = retire_quality_leaf_sites(
        &mut mesh,
        &criteria,
        HardGates::default(),
        limits(1, 100_000),
        &leaves,
        3,
    );

    assert_eq!((counts.0, counts.1), (1, 0));
    assert!(counts.2.is_empty());
    assert!(vertices_below_degree_5_set(mesh.state()).len() < before_low_degree.len());
    let after_angles = angle_window_survey(mesh.state());
    assert!(
        after_angles.below + after_angles.above_80 < before_angles.below + before_angles.above_80
    );
    assert!(mesh.vertex_for_site_id(site_id).is_none());
    assert!(mesh
        .conservative_remap()
        .iter()
        .any(|weight| weight.old_site_id == site_id));
}

/// Read `EARTHMESH_TEST_NXP` / `EARTHMESH_TEST_MAX_CYCLES`, with defaults.
fn nxp_and_cycles(default_nxp: usize, default_cycles: u32) -> (usize, u32) {
    let nxp = std::env::var("EARTHMESH_TEST_NXP")
        .ok()
        .map(|value| {
            value
                .parse::<usize>()
                .expect("EARTHMESH_TEST_NXP must be an integer")
        })
        .unwrap_or(default_nxp);
    assert!(nxp >= 6, "EARTHMESH_TEST_NXP must be at least 6, got {nxp}");
    let cycles = std::env::var("EARTHMESH_TEST_MAX_CYCLES")
        .ok()
        .map(|value| {
            value
                .parse::<u32>()
                .expect("EARTHMESH_TEST_MAX_CYCLES must be an integer")
        })
        .unwrap_or(default_cycles);
    (nxp, cycles)
}

/// What leaf retirement costs, and that it still retires the same leaves.
///
/// `natural_length_ab_on_the_nxp_proxy` cannot measure this: it goes through
/// `production_quality_checkpoint`, and a capturing run returns before the
/// retirement phase. So the input here is the captured checkpoint and the
/// phase is driven directly -- a fixed, reproducible state, though not the
/// post-optimiser mesh production would hand it.
#[test]
#[ignore = "retirement performance gate; run explicitly"]
fn leaf_retirement_on_the_production_checkpoint() {
    let (nxp, max_cycles) = nxp_and_cycles(40, 20);
    let gates = HardGates::default();
    let limits = limits(max_cycles, 200_000);
    let mut production = sphere(nxp);
    let criteria = steep_target(&production);
    let start = production_quality_checkpoint(
        &mut production,
        &criteria,
        CandidatePolicy::default(),
        gates,
        limits,
    );

    let leaves = leaf_lineage_survey(&start, &criteria);
    let maximum_degree = 4;
    let candidates = retirement_candidates(start.state(), &leaves, maximum_degree);

    let mut mesh = start.clone();
    let started = std::time::Instant::now();
    let (committed, committed_d4, _) =
        retire_quality_leaf_sites(&mut mesh, &criteria, gates, limits, &leaves, maximum_degree);
    let runtime_s = started.elapsed().as_secs_f64();

    eprintln!(
        "\nnxp {nxp}: retirement over {} sites, {} candidate(s), {committed} retired \
         ({committed_d4} degree-four), {runtime_s:.2}s",
        start.active_site_count(),
        candidates.len()
    );
    eprintln!(
        "first candidates: {:?}",
        &candidates[..candidates.len().min(8)]
    );

    // The phase's cost is candidate collection plus the guards, run once per
    // postcondition call. Timing each against the reference implementation
    // isolates what this work changed from what it did not.
    let state = start.state();
    let clock = |work: &mut dyn FnMut()| {
        let started = std::time::Instant::now();
        work();
        started.elapsed().as_secs_f64()
    };
    let candidates_now = clock(&mut || {
        retirement_candidates(state, &leaves, maximum_degree);
    });
    let candidates_before = clock(&mut || {
        reference_retirement_candidates(state, &leaves, maximum_degree);
    });
    let guards_now = clock(&mut || {
        let (_, scales) = demanded_cells_and_scales(state, &criteria);
        balance_survey_from_scales(state, &scales, limits);
        vertices_below_degree_5_set(state);
    });
    let guards_before = clock(&mut || {
        reference_demanded_cells_in_state(state, &criteria);
        reference_balance_survey(state, limits);
        reference_vertices_below_degree_5(state);
    });
    eprintln!(
        "candidate collection {candidates_before:.4}s -> {candidates_now:.4}s ({:.0}x)",
        candidates_before / candidates_now.max(f64::MIN_POSITIVE)
    );
    eprintln!(
        "one guard sweep      {guards_before:.4}s -> {guards_now:.4}s ({:.0}x)",
        guards_before / guards_now.max(f64::MIN_POSITIVE)
    );

    mesh.state()
        .validate()
        .unwrap_or_else(|error| panic!("retirement left an invalid triangulation: {error:?}"));
    assert_eq!(mesh.state().open_edge_count(), 0);
    assert!(worst_degree(&mesh) <= gates.max_vertex_degree);

    // Ordering decides which 64 candidates are tried, so a reordering shows up
    // as a different mesh rather than a different count.
    let mut repeat = start.clone();
    let repeat_counts = retire_quality_leaf_sites(
        &mut repeat,
        &criteria,
        gates,
        limits,
        &leaves,
        maximum_degree,
    );
    assert_eq!(
        (committed, committed_d4),
        (repeat_counts.0, repeat_counts.1),
        "retirement is not deterministic"
    );
    assert!(
        mesh_fingerprint(mesh.state()) == mesh_fingerprint(repeat.state()),
        "retirement is not deterministic: the repeat run produced a different mesh"
    );
}

/// The whole production path, with each phase's cost named.
///
/// The A/B proxy stops at the quality boundary, so nothing else measures what a
/// caller of `run_cycles` actually waits for.
#[test]
#[ignore = "full production path timing; run explicitly"]
fn the_full_production_path_on_the_nxp_proxy() {
    let (nxp, max_cycles) = nxp_and_cycles(40, 20);
    let gates = HardGates::default();
    let limits = limits(max_cycles, 200_000);
    let mut mesh = sphere(nxp);
    let criteria = steep_target(&mesh);

    let started = std::time::Instant::now();
    let outcome = run_cycles(
        &mut mesh,
        &criteria,
        CandidatePolicy::default(),
        gates,
        limits,
    )
    .expect("the production path runs");
    let runtime_s = started.elapsed().as_secs_f64();
    let report = outcome.report;

    // The phase-by-phase costs are on stderr above this line: the cycle lines,
    // "quality optimiser complete", and "leaf retirement".
    eprintln!(
        "\nnxp {nxp} full path: {runtime_s:.1}s total, {} sites, {} cycles, stop {:?}",
        report.final_sites, report.cycles_completed, report.stop_reason
    );
    eprintln!(
        "  below40 {} above80 {} min {:.4} max {:.4} unresolved {} unbalanced {}",
        report.angles_below_40_deg,
        report.angles_above_80_deg,
        report.angle_min_deg,
        report.angle_max_deg,
        report.unresolved_count,
        report.unbalanced_pairs_remaining
    );

    // Where the tail actually is. "Outside the window" counts a 39-degree
    // corner and a 0.2-degree one alike, and those mean very different things
    // to whatever consumes the mesh.
    let mut corners = Vec::new();
    let mut worst_triangles = Vec::new();
    for triangle in mesh.state().active_triangle_slots() {
        let [a, b, c] = mesh.state().triangles()[triangle];
        let Some(angles) = crate::criteria::triangle_angles_deg([
            mesh.state().vertices()[a],
            mesh.state().vertices()[b],
            mesh.state().vertices()[c],
        ]) else {
            continue;
        };
        worst_triangles.push(angles.iter().copied().fold(f64::MAX, f64::min));
        corners.extend(angles);
    }
    worst_triangles.sort_by(f64::total_cmp);
    let below = |limit: f64| worst_triangles.partition_point(|angle| *angle < limit);
    eprintln!(
        "triangles by smallest angle: <1 {}, <5 {}, <15 {}, <25 {}, <30 {}, <40 {} of {}",
        below(1.0),
        below(5.0),
        below(15.0),
        below(25.0),
        below(30.0),
        below(40.0),
        worst_triangles.len()
    );
    corners.sort_by(|left, right| right.total_cmp(left));
    eprintln!(
        "widest corners: {:?}",
        corners
            .iter()
            .take(8)
            .map(|angle| (angle * 100.0).round() / 100.0)
            .collect::<Vec<_>>()
    );

    mesh.state().validate().unwrap_or_else(|error| {
        panic!("the production path left an invalid triangulation: {error:?}")
    });
    assert_eq!(mesh.state().open_edge_count(), 0);
    assert!(worst_degree(&mesh) <= gates.max_vertex_degree);
}

// ---------------------------------------------------------------------------
// Reference implementations for the seeded-objective work.
//
// The transaction objective still finds a fan seed by scanning, once per
// affected site per line-search step. These pin what it produces today so the
// seeded version can be shown to produce the same thing.
// ---------------------------------------------------------------------------

/// The pre-optimisation cell read: the seed is found by scanning the mesh.
fn reference_voronoi_cell_scanned(
    state: &MeshState,
    site: usize,
) -> Option<earthmesh_mesh::VoronoiCell> {
    state.voronoi_cell(site).ok()
}

/// The pre-optimisation `triangle_fan_ids`: a scanned fan per site.
fn reference_triangle_fan_ids(
    state: &MeshState,
    sites: &BTreeSet<usize>,
) -> Option<BTreeSet<usize>> {
    let mut triangles = BTreeSet::new();
    for &site in sites {
        triangles.extend(state.triangle_fan(site).ok()?);
    }
    Some(triangles)
}

/// The pre-optimisation `site_scale`, with the radius recomputed inside.
fn reference_site_scale(state: &MeshState, site: usize) -> Option<f64> {
    let cell = state.voronoi_cell(site).ok()?;
    CellView {
        site,
        cell: &cell,
        state,
        radius_m: state.sphere_radius(),
    }
    .effective_scale_m()
}

/// Any incident triangle must seed the cell the scan produces, bit for bit.
///
/// This is the precondition for feeding the objective from `sites_touching`,
/// whose seed is the lowest triangle *within the change's reach* rather than
/// within the whole mesh. `triangle_fan_from` starts the fan at whatever seed
/// it is given, so two seeds give the same ring rotated -- the same cell, and a
/// different order to sum its corner areas in.
///
/// **This test is expected to fail until `voronoi_cell_from` pins the fan to
/// its lowest triangle.** A version of it that passes before then is a version
/// that never tried a non-minimal seed, which is why the count is asserted.
#[test]
fn every_incident_seed_builds_the_same_cell_as_the_scan() {
    let mut checked = 0usize;
    let mut non_minimal = 0usize;
    for (name, mesh) in full_sweep_fixtures() {
        let state = mesh.state();
        let radius_m = state.sphere_radius();
        for site in state.active_vertex_slots() {
            let Some(expected) = reference_voronoi_cell_scanned(state, site) else {
                continue;
            };
            let expected_scale = CellView {
                site,
                cell: &expected,
                state,
                radius_m,
            }
            .effective_scale_m();
            // The scan takes the first active triangle naming the site, so the
            // fan it returns starts at the lowest-numbered incident one.
            let minimal = expected.triangles[0];
            for &seed in &expected.triangles {
                non_minimal += usize::from(seed != minimal);
                let produced = state
                    .voronoi_cell_from(site, seed)
                    .expect("an incident triangle seeds a cell");
                assert_eq!(
                    produced.triangles, expected.triangles,
                    "{name}: site {site} seeded from {seed} produced a differently ordered fan"
                );
                let produced_scale = CellView {
                    site,
                    cell: &produced,
                    state,
                    radius_m,
                }
                .effective_scale_m();
                assert_eq!(
                    produced_scale.map(f64::to_bits),
                    expected_scale.map(f64::to_bits),
                    "{name}: site {site} seeded from {seed} produced a different scale"
                );
                checked += 1;
            }
        }
    }
    assert!(
        non_minimal > 0,
        "no fixture exercised a non-minimal seed, so this test proves nothing"
    );
    eprintln!("checked {checked} (site, seed) pairs, {non_minimal} with a non-minimal seed");
}

/// `triangle_fan_ids` must not care which incident triangle seeded each fan.
///
/// Unlike the cell read above this one needs no normalisation: the ids go into
/// a `BTreeSet` and the values `sorted_triangle_values` derives from them are
/// sorted, so the ring's starting point cannot reach the result.
#[test]
fn triangle_fan_ids_are_independent_of_the_seed() {
    for (name, mesh) in full_sweep_fixtures() {
        let state = mesh.state();
        let seeds = active_site_triangle_seeds(state);
        for site in state.active_vertex_slots() {
            let Ok(scanned) = state.triangle_fan(site) else {
                continue;
            };
            let expected: BTreeSet<usize> = scanned.iter().copied().collect();
            for &seed in &scanned {
                let produced: BTreeSet<usize> = state
                    .triangle_fan_from(site, seed)
                    .expect("an incident triangle seeds a fan")
                    .into_iter()
                    .collect();
                assert_eq!(
                    produced, expected,
                    "{name}: site {site} seeded from {seed} yielded a different triangle set"
                );
            }
            assert_eq!(
                seeds[site],
                Some(scanned[0]),
                "{name}: the seed table disagrees with the scan at site {site}"
            );
        }
        let seeded: AffectedSites = state
            .active_vertex_slots()
            .filter_map(|site| Some((site, seeds[site]?)))
            .collect();
        let sites: BTreeSet<usize> = seeded.keys().copied().collect();
        assert_eq!(
            triangle_fan_ids(state, &seeded),
            reference_triangle_fan_ids(state, &sites),
            "{name}: the fan-id collection differs from the scanned reference"
        );
    }
}

/// Does a bounded, local stall escalation clear what the global one never sees?
///
/// The escalations all trigger on "nothing was accepted anywhere this cycle",
/// which measured zero firings at both NXP40 and NXP80 while degree-blocked
/// sites waited in almost every cycle. This offers the same widening to the
/// sites that have been blocked longest instead, under the bound the
/// tried-and-rejected note asks for: a persistence floor and a per-cycle seed
/// cap. Read-out is the refinement residual, so the checkpoint path is enough
/// -- the optimiser moves sites but does not resolve demands.
#[test]
#[ignore = "bounded local-recovery A/B; run explicitly"]
fn local_recovery_ab_on_the_nxp_proxy() {
    let (nxp, max_cycles) = nxp_and_cycles(40, 20);
    let gates = HardGates::default();
    let limits = limits(max_cycles, 200_000);
    let arms = [
        ("OFF", LocalRecoveryPolicy::OFF),
        (
            "LOCAL-2/16",
            LocalRecoveryPolicy {
                minimum_consecutive_cycles: 2,
                maximum_seeds_per_cycle: 16,
            },
        ),
    ];

    eprintln!(
        "\nnxp arm         sites pending unbal below40 above80 min_angle max_angle runtime_s"
    );
    let mut results = Vec::new();
    for (name, local_recovery) in arms {
        let mut production = sphere(nxp);
        let criteria = steep_target(&production);
        let started = std::time::Instant::now();
        let start = production_quality_checkpoint_with_local_recovery(
            &mut production,
            &criteria,
            CandidatePolicy::default(),
            gates,
            limits,
            local_recovery,
        );
        let runtime_s = started.elapsed().as_secs_f64();
        let metrics = QualityGuardMetrics::read(&start, &criteria, limits).expect("metrics");
        eprintln!(
            "{nxp:3} {name:<11} {:5} {:7} {:5} {:7} {:7} {:9.4} {:9.4} {runtime_s:9.1}",
            start.active_site_count(),
            metrics.pending,
            metrics.unbalanced,
            metrics.angles.below,
            metrics.angles.above_80,
            metrics.angles.min_deg,
            metrics.angles.max_deg
        );
        start
            .state()
            .validate()
            .unwrap_or_else(|error| panic!("{name} left an invalid triangulation: {error:?}"));
        assert_eq!(start.state().open_edge_count(), 0);
        assert!(worst_degree(&start) <= gates.max_vertex_degree);
        results.push((name, metrics.pending, metrics.unbalanced, runtime_s));
    }

    let (_, off_pending, off_unbalanced, off_runtime) = results[0];
    let (_, local_pending, local_unbalanced, local_runtime) = results[1];
    eprintln!(
        "\npending {off_pending} -> {local_pending}, unbalanced {off_unbalanced} -> \
         {local_unbalanced}, runtime {off_runtime:.1}s -> {local_runtime:.1}s"
    );
}

/// How far apart are a fan's triangles in the arrays the walk indexes?
///
/// The ring walk is six steps, and its cost still grows with mesh size --
/// 1.05ms a line-search attempt at NXP40 against 4.69ms at NXP80, with the two
/// profiles the same shape. That is the memory hierarchy, not different work.
///
/// # What it measured, and why triangle renumbering was not done
///
/// At NXP40, 71% of the steps between consecutive fan members land within four
/// rows of each other -- one cache line -- and the median step is one. Fans are
/// already packed. Renumbering could only reach the other 29%, of a walk that
/// is about two thirds of the optimiser, so its ceiling is under a fifth even
/// assuming every non-local step is a full miss and every local one is free.
///
/// The misses come from somewhere renumbering cannot reach: `quality_problem_sites`
/// orders a pass worst-margin-first, which is spatially arbitrary, so
/// consecutive sites in a pass sit nowhere near each other and each arrives
/// cold. NXP40's triangles, neighbours and vertices come to about 2.6MB and
/// mostly stay resident; NXP80's come to 8.5MB and do not. Fixing that means
/// changing the order sites are processed in, which changes which moves the
/// greedy pass finds -- a different mesh, not a faster one.
#[test]
#[ignore = "locality probe; run explicitly"]
fn how_scattered_is_a_fan_in_the_triangle_arrays() {
    let (nxp, max_cycles) = nxp_and_cycles(40, 20);
    let gates = HardGates::default();
    let limits = limits(max_cycles, 200_000);
    let mut production = sphere(nxp);
    let criteria = steep_target(&production);
    let start = production_quality_checkpoint(
        &mut production,
        &criteria,
        CandidatePolicy::default(),
        gates,
        limits,
    );
    let state = start.state();
    let seeds = active_site_triangle_seeds(state);

    let mut spreads = Vec::new();
    let mut steps = Vec::new();
    for site in state.active_vertex_slots() {
        let Some(seed) = seeds[site] else { continue };
        let Ok(fan) = state.triangle_fan_from(site, seed) else {
            continue;
        };
        let low = fan.iter().copied().min().expect("a fan");
        let high = fan.iter().copied().max().expect("a fan");
        spreads.push(high - low);
        for pair in fan.windows(2) {
            steps.push(pair[1].abs_diff(pair[0]));
        }
    }
    spreads.sort_unstable();
    steps.sort_unstable();
    let at = |values: &[usize], percent: usize| values[values.len() * percent / 100];
    eprintln!(
        "\nnxp {nxp}: {} triangles, {} fans",
        state.triangle_count(),
        spreads.len()
    );
    eprintln!(
        "fan index spread  p50 {}  p90 {}  p99 {}  max {}",
        at(&spreads, 50),
        at(&spreads, 90),
        at(&spreads, 99),
        spreads.last().expect("a fan")
    );
    eprintln!(
        "step between consecutive fan members  p50 {}  p90 {}  p99 {}",
        at(&steps, 50),
        at(&steps, 90),
        at(&steps, 99)
    );
    // A cache line holds four `[usize; 3]` rows. Anything much past that is a
    // miss per step.
    let within_a_line = steps.iter().filter(|&&step| step <= 4).count();
    eprintln!(
        "steps landing within four rows: {within_a_line} of {} ({:.1}%)",
        steps.len(),
        100.0 * within_a_line as f64 / steps.len() as f64
    );
}

/// The kept cell survey must equal a fresh one after every pass.
///
/// `GuardCells` refreshes only the neighbourhoods a pass moved, which is the
/// whole point and also the whole risk: a dirty set that misses a cell leaves a
/// stale scale in the guard that decides whether a pass is kept, and the run
/// carries on producing a valid mesh that is not the one it would have made.
/// This turns that comparison on and runs the optimiser through it.
#[test]
fn the_kept_cell_survey_never_drifts_from_a_fresh_sweep() {
    let gates = HardGates::default();
    // Three refinement cycles, not forty: the comparison is per pass and does
    // not need a large mesh to find a cell the refresh missed.
    let limits = limits(3, 100_000);
    let mut mesh = sphere(6);
    let background_scale_m = median_cell_scale(mesh.state());
    let criteria = steep_target(&mesh);
    stalled_by_insertion_alone(&mut mesh, &criteria, gates, limits);

    const CHECKED_PASSES: usize = 8;
    VERIFY_GUARD_CELLS.with(|verify| verify.set(CHECKED_PASSES));
    let outcome = optimise_mesh_quality(&mut mesh, &criteria, gates, limits, background_scale_m);
    let unused = VERIFY_GUARD_CELLS.with(|verify| verify.replace(0));
    let (moves, _) = outcome.expect("the optimiser runs");

    assert!(
        moves > 0,
        "the fixture never committed a move, so nothing was verified"
    );
    assert!(
        unused < CHECKED_PASSES,
        "no pass was compared against a fresh sweep"
    );
}

/// Attribute the synthetic steep-target degree wall at NXP80.
///
/// This is `sphere(80)` plus the synthetic `steep_target` criterion. It is not
/// the real IGBP NXP80 production run and must not be cited as one.
///
/// Read-only. The triangle is already there at the quality boundary, so the
/// checkpoint path is enough and no production code changes.
#[test]
#[ignore = "degenerate-triangle attribution; run explicitly"]
fn synthetic_steep_target_nxp80_degree_wall_attribution() {
    let (nxp, max_cycles) = nxp_and_cycles(80, 100);
    let gates = HardGates::default();
    let limits = limits(max_cycles, 200_000);
    let mut production = sphere(nxp);
    let criteria = steep_target(&production);
    let start = production_quality_checkpoint(
        &mut production,
        &criteria,
        CandidatePolicy::default(),
        gates,
        limits,
    );
    let state = start.state();

    let worst = state
        .active_triangle_slots()
        .filter_map(|triangle| Some((triangle, triangle_window_margin(state, triangle)?)))
        .min_by(|left, right| left.1.total_cmp(&right.1))
        .expect("a measurable triangle");
    let (triangle, margin) = worst;
    let corners = state.triangles()[triangle];
    let angles = crate::criteria::triangle_angles_deg([
        state.vertices()[corners[0]],
        state.vertices()[corners[1]],
        state.vertices()[corners[2]],
    ]);
    let mut degree_triplet = corners.map(|corner| state.vertex_degree(corner).expect("degree"));
    degree_triplet.sort_unstable();
    if nxp == 80 && max_cycles == 100 {
        assert_eq!(degree_triplet, [3, 7, 7]);
    }
    let criterion_refs = criteria
        .iter()
        .map(|criterion| criterion.as_ref())
        .collect::<Vec<_>>();
    let certification = crate::certifier::certify_mesh(&start, &criterion_refs);

    eprintln!("\nfixture_kind=synthetic");
    eprintln!("source_mesh=sphere({nxp})");
    eprintln!("criterion=steep_target");
    eprintln!("worst triangle {triangle}, window margin {margin:.6}");
    eprintln!("angles {angles:?}");
    eprintln!("degree_triplet={degree_triplet:?}");
    eprintln!(
        "certifier angles={} min={:?} p1={:?} p99={:?} max={:?} below_40={} above_80={}",
        certification.measurable_angle_count,
        certification.min_angle_deg,
        certification.p1_angle_deg,
        certification.p99_angle_deg,
        certification.max_angle_deg,
        certification.below_40_count,
        certification.above_80_count
    );
    eprintln!("degree_histogram={:?}", certification.degree_histogram);
    eprintln!("eta {:?}", triangle_eta_value(state, triangle));
    eprintln!(
        "\n{:>8} {:>9} {:>6} {:>6} {:>6} {:>10} {:>12} {:>10}",
        "vertex", "degree", "birth", "depth", "parent", "mobility", "movable", "displaced"
    );
    for corner in corners {
        let site = start.site_for_vertex(corner);
        eprintln!(
            "{corner:>8} {:>9} {:>6} {:>6} {:>6} {:>10} {:>12} {:>10.1}",
            state
                .vertex_degree(corner)
                .map_or("-".to_string(), |degree| degree.to_string()),
            site.map_or("-".to_string(), |site| site.birth_cycle.to_string()),
            site.map_or("-".to_string(), |site| site.depth.to_string()),
            site.map_or("-".to_string(), |site| site
                .parent_site_id
                .map_or("none".to_string(), |parent| parent.0.to_string())),
            site.map_or("-".to_string(), |site| format!("{:?}", site.mobility)),
            start.can_move_site(corner),
            site.map_or(f64::NAN, |site| site.cumulative_displacement_m)
        );
        let point = xyz_to_lonlat_degrees(state.vertices()[corner]);
        eprintln!(
            "         lon {:+.5} lat {:+.5}",
            point.lon_degrees, point.lat_degrees
        );
    }

    // Does the optimiser even look at it? A pass takes the worst-first movable
    // sites, capped; a triangle whose corners never reach that list is not
    // being optimised badly, it is not being optimised.
    for window_first in [false, true] {
        let (selected, found, eligible, _) =
            quality_problem_sites(&start, window_first, &BTreeSet::new(), false);
        let picked: Vec<usize> = corners
            .into_iter()
            .filter(|corner| selected.contains(corner))
            .collect();
        eprintln!(
            "\n{} pass: {} of {} eligible / {} movable selected; corners in the list: {:?}",
            if window_first { "window" } else { "eta" },
            selected.len(),
            eligible,
            found,
            picked
        );
        for corner in corners {
            let rank = selected.iter().position(|&site| site == corner);
            eprintln!("  vertex {corner}: rank {rank:?}");
        }
    }
}
