use super::*;

use earthmesh_mesh::{lonlat_degrees_to_unit_xyz, LonLatDegrees, TriangularMesh};

/// Gates with the sliver floor off.
///
/// Most tests here are about insertion, rollback and accounting, not about
/// mesh quality. Leaving the shipped 28-degree floor on would have them refuse
/// perfectly ordinary insertions and assert nothing they claim to.
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

fn on(mesh: &AdaptiveMesh, lon: f64, lat: f64) -> CartesianPoint {
    let unit = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(lon, lat));
    let radius = mesh.state().sphere_radius();
    CartesianPoint::new(unit.x * radius, unit.y * radius, unit.z * radius)
}

/// A spread of candidates, deterministic and independent of the mesh.
fn candidates(count: usize) -> Vec<(f64, f64)> {
    (0..count)
        .map(|step| {
            (
                -180.0 + (step as f64) * 6.1,
                -70.0 + ((step * 37) % 140) as f64,
            )
        })
        .collect()
}

fn worst_degree(mesh: &AdaptiveMesh) -> usize {
    let state = mesh.state();
    (MESH_STATE_FIRST_ID..state.vertices().len())
        .filter_map(|site| state.vertex_degree(site).ok())
        .max()
        .unwrap_or(0)
}

/// The gate holds the bound the gridfile needs, over a run long enough to
/// break it several times over.
///
/// Without it this same sequence reaches degree eight inside ten insertions --
/// measured in `earthmesh_mesh`, and the reason this gate is not optional.
#[test]
fn the_degree_gate_keeps_the_mesh_writable() {
    let mut mesh = sphere(6);
    let gates = permissive();
    let mut outcomes = Vec::new();
    for (lon, lat) in candidates(40) {
        let point = on(&mesh, lon, lat);
        outcomes.push(mesh.propose_site(point, gates).expect("propose"));
    }

    let kept = outcomes.iter().filter(|o| o.committed().is_some()).count();
    let refused = outcomes.len() - kept;
    assert!(kept > 0, "some candidates were kept");
    assert!(
        refused > 0,
        "and some were refused -- otherwise this run never tested the gate"
    );
    assert!(
        worst_degree(&mesh) <= GRIDFILE_MAX_VERTEX_DEGREE,
        "the gate let a site through at degree {}",
        worst_degree(&mesh)
    );
    assert_eq!(mesh.state().open_edge_count(), 0);
    mesh.state().validate().expect("still a triangulation");
}

/// Every refusal is for a reason the run can name.
#[test]
fn a_refusal_says_which_site_and_by_how_much() {
    let mut mesh = sphere(6);
    let gates = permissive();
    let mut degree_refusals = 0;
    for (lon, lat) in candidates(40) {
        let point = on(&mesh, lon, lat);
        if let Some(rejection) = mesh
            .propose_site(point, gates)
            .expect("propose")
            .rejection()
        {
            match rejection {
                Rejection::DegreeOverBudget {
                    site,
                    degree,
                    budget,
                } => {
                    degree_refusals += 1;
                    assert!(*site >= MESH_STATE_FIRST_ID);
                    assert!(degree > budget, "{degree} is not over {budget}");
                }
                // The twelve pentagons have to stay pentagons, so a candidate
                // beside one is refused for that instead of for the general
                // degree bound.
                Rejection::ProtectedPentagonDisturbed { site, degree } => {
                    assert!(*site >= MESH_STATE_FIRST_ID);
                    assert_ne!(*degree, 5);
                }
                // The writer's own admissibility test, refused here rather
                // than at output.
                Rejection::SliverTriangle { angle_deg, .. } => {
                    assert!(*angle_deg >= 0.0);
                }
                other => panic!("unexpected refusal: {other}"),
            }
        }
    }
    assert!(degree_refusals > 0);
}

/// A rolled-back proposal leaves nothing at all.
///
/// Compared against the whole prior triangulation and the whole site table,
/// because a rollback that restores the mesh and keeps the id would leave a
/// report naming a site that does not exist.
#[test]
fn a_rejected_proposal_leaves_the_mesh_and_the_site_table_untouched() {
    let mut mesh = sphere(6);
    let gates = HardGates {
        // Nothing can pass: an icosahedral mesh already has sites of degree
        // six, and an insertion only raises the ones it touches.
        max_vertex_degree: 5,
        ..HardGates::default()
    };
    let before_state = mesh.state().clone();
    let before_sites = mesh.sites().to_vec();
    let before_next_id = mesh.next_site_id();

    for (lon, lat) in candidates(12) {
        let point = on(&mesh, lon, lat);
        let outcome = mesh.propose_site(point, gates).expect("propose");
        assert!(outcome.committed().is_none(), "nothing passes at degree 5");
    }

    assert_eq!(
        mesh.state(),
        &before_state,
        "the triangulation is unchanged"
    );
    assert_eq!(mesh.sites(), before_sites.as_slice());
    assert_eq!(
        mesh.next_site_id(),
        before_next_id,
        "no id was spent on a site that was rolled back"
    );
}

/// A committed site is in the table, and its id resolves to where it went.
#[test]
fn a_committed_site_is_recorded_where_the_report_says() {
    let mut mesh = sphere(6);
    let before_sites = mesh.sites().len();
    let point = on(&mesh, 41.0, 19.0);

    let outcome = mesh.propose_site(point, permissive()).expect("propose");
    let report = outcome.committed().expect("this one passes");

    assert_eq!(mesh.sites().len(), before_sites + 1);
    let site = mesh.sites().last().expect("the new row");
    assert_eq!(site.site_id, report.site_id);
    assert!(site.active);
    assert_eq!(site.birth_cycle, 1, "created by adaptation, not inherited");
    assert_eq!(report.triangles_created, report.triangles_removed + 2);
    assert!(report.max_degree_touched <= GRIDFILE_MAX_VERTEX_DEGREE);
}

/// The same proposals in the same order give the same mesh and the same ids.
#[test]
fn proposing_is_deterministic() {
    let build = || {
        let mut mesh = sphere(6);
        let outcomes: Vec<Acceptance> = candidates(20)
            .into_iter()
            .map(|(lon, lat)| {
                let point = on(&mesh, lon, lat);
                mesh.propose_site(point, permissive()).expect("propose")
            })
            .collect();
        (mesh.state().clone(), committed_site_ids(&outcomes))
    };
    let (first_state, first_ids) = build();
    let (second_state, second_ids) = build();
    assert_eq!(first_state, second_state);
    assert_eq!(first_ids, second_ids);
    assert!(!first_ids.is_empty());
}

/// A point off the mesh's sphere is refused as a proposal, not as a panic.
#[test]
fn a_candidate_off_the_sphere_is_refused() {
    let mut mesh = sphere(6);
    let unit = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(23.0, 17.0));
    let outcome = mesh
        .propose_site(unit, HardGates::default())
        .expect("propose");
    match outcome.rejection().expect("refused") {
        Rejection::NotInsertable(error) => {
            assert!(error.to_string().contains("radius"), "{error}");
        }
        other => panic!("expected an insertability refusal, got {other}"),
    }
    assert_eq!(mesh.sites().len(), mesh.state().vertex_count());
}

/// A proposal reads a neighbourhood, not a mesh.
///
/// The property behind the cost measurement in `propose_site_near`'s docs, and
/// checkable without a clock: every gate is run over the triangles the change
/// touched and the ring around them. When this held only by accident the
/// gates called `open_edge_count` and `validate`, which walk everything, and
/// one proposal into a 737k-triangle mesh cost 3 milliseconds instead of 275
/// microseconds -- growth that is invisible on any fixture small enough to be
/// a unit test.
///
/// Checked by counting how much of the mesh a proposal can see: the same
/// proposal into a mesh sixty-four times larger must not read sixty-four times
/// as much. Approximated by the triangles it reports touching, which is what
/// every gate is handed.
#[test]
fn a_proposal_touches_a_neighbourhood_whatever_the_mesh_size() {
    let mut touched = Vec::new();
    for nxp in [6usize, 24, 48] {
        let mut mesh = sphere(nxp);
        let radius = mesh.state().sphere_radius();
        let unit = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(41.0, 19.0));
        let point = CartesianPoint::new(unit.x * radius, unit.y * radius, unit.z * radius);
        let outcome = mesh.propose_site(point, permissive()).expect("propose");
        let report = outcome.committed().expect("this one passes");
        touched.push(report.triangles_created + report.triangles_removed);
    }
    let largest = *touched.iter().max().expect("measured");
    assert!(
        largest <= 16,
        "a proposal touched {largest} triangles; a Delaunay cavity is a handful whatever the \
         mesh, so this is a gate reading past the change: {touched:?}"
    );
}

/// A hint changes what the walk costs, not what it finds.
#[test]
fn a_location_hint_does_not_change_the_outcome() {
    let build = |hint: Option<usize>| {
        let mut mesh = sphere(12);
        let radius = mesh.state().sphere_radius();
        for (lon, lat) in candidates(12) {
            let unit = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(lon, lat));
            let point = CartesianPoint::new(unit.x * radius, unit.y * radius, unit.z * radius);
            mesh.propose_site_near(point, hint, permissive())
                .expect("propose");
        }
        mesh.state().clone()
    };
    let without = build(None);
    assert_eq!(build(Some(MESH_STATE_FIRST_ID)), without);
    assert_eq!(build(Some(200)), without);
    assert_eq!(
        build(Some(usize::MAX)),
        without,
        "an out-of-range hint is ignored"
    );
}

/// A demand nothing can satisfy is unresolved, and the mesh is untouched.
///
/// Section 13.4 in one assertion: the last rung of the ladder is not committed
/// because it is the last. A mesh that kept a point every gate refused cannot
/// be told apart from a mesh that was refined correctly, which is the whole
/// class of failure this backend is supposed to avoid.
#[test]
fn a_demand_the_ladder_cannot_satisfy_leaves_the_mesh_alone() {
    let mut mesh = sphere(6);
    let before = mesh.state().clone();
    let before_sites = mesh.sites().len();
    let gates = HardGates {
        max_vertex_degree: 5,
        ..HardGates::default()
    };

    let outcome = mesh
        .refine_cell(40, None, CandidatePolicy::default(), gates)
        .expect("refine");
    match outcome {
        DemandOutcome::Unresolved { refusals } => {
            assert_eq!(refusals.len(), 3, "every rung was tried: {refusals:?}");
            assert_eq!(
                refusals
                    .iter()
                    .map(|(source, _)| *source)
                    .collect::<Vec<_>>(),
                vec![
                    CandidateSource::FarthestPoint,
                    CandidateSource::OffCentre,
                    CandidateSource::LongestEdgeMidpoint,
                ]
            );
        }
        other => panic!("expected the ladder to run out, got {other:?}"),
    }
    assert_eq!(mesh.state(), &before, "and left nothing behind");
    assert_eq!(mesh.sites().len(), before_sites);
}

/// A demand the ladder can satisfy stops at the first rung that passes.
#[test]
fn refining_a_cell_keeps_the_first_candidate_that_survives() {
    let mut mesh = sphere(6);
    let outcome = mesh
        .refine_cell(40, None, CandidatePolicy::default(), permissive())
        .expect("refine");
    match outcome {
        DemandOutcome::Resolved { source, report } => {
            // Not necessarily the first rung: this cell's farthest corner sits
            // beside a pentagon, and a pentagon has to stay one. Which rung
            // wins is the ladder doing its job; that a rung won is the claim.
            assert_ne!(source, CandidateSource::Witness, "none was supplied");
            assert!(report.max_degree_touched <= GRIDFILE_MAX_VERTEX_DEGREE);
        }
        other => panic!("expected a commit, got {other:?}"),
    }
    assert_eq!(mesh.state().open_edge_count(), 0);
    mesh.state().validate().expect("still a triangulation");
}

/// A failed rung is rolled back before the next is tried.
///
/// Otherwise the ladder would stack attempts: rung two would be proposed into
/// a mesh already carrying rung one, and a "first candidate that passes" would
/// have committed several.
#[test]
fn each_rung_is_undone_before_the_next_is_tried() {
    let mut mesh = sphere(6);
    let before_triangles = mesh.state().triangle_count();
    let before_sites = mesh.sites().len();

    // Degree 6 lets some rungs through and refuses others, so at least one
    // refusal precedes whatever is finally kept somewhere in this sweep.
    let gates = HardGates {
        max_vertex_degree: 6,
        ..permissive()
    };
    let mut resolved = 0;
    for site in [7usize, 40, 120, 300] {
        if mesh
            .refine_cell(site, None, CandidatePolicy::default(), gates)
            .expect("refine")
            .resolved()
            .is_some()
        {
            resolved += 1;
        }
    }
    assert_eq!(
        mesh.state().triangle_count(),
        before_triangles + resolved * 2,
        "each resolved demand added one site's worth of triangles and no more"
    );
    assert_eq!(mesh.sites().len(), before_sites + resolved);
    assert_eq!(mesh.state().open_edge_count(), 0);
}

/// A witness the criterion supplied is tried before anything this module made
/// up.
#[test]
fn a_witness_is_tried_first() {
    let mut mesh = sphere(6);
    let site = 40;
    // A point just off the site, inside its own cell: legal, and nothing the
    // geometric rungs would have produced.
    let centre = mesh.state().vertices()[site];
    let neighbour = mesh.state().vertices()[mesh.state().triangles()
        [mesh.state().triangle_fan(site).expect("fan")[0]]
        .iter()
        .copied()
        .find(|&corner| corner != site)
        .expect("a neighbour")];
    let witness = CartesianPoint::new(
        centre.x * 0.8 + neighbour.x * 0.2,
        centre.y * 0.8 + neighbour.y * 0.2,
        centre.z * 0.8 + neighbour.z * 0.2,
    );
    let radius = earthmesh_mesh::magnitude(centre);
    let scale = radius / earthmesh_mesh::magnitude(witness);
    let witness = CartesianPoint::new(witness.x * scale, witness.y * scale, witness.z * scale);

    let outcome = mesh
        .refine_cell(
            site,
            Some(witness),
            CandidatePolicy::default(),
            permissive(),
        )
        .expect("refine");
    match outcome {
        DemandOutcome::Resolved { source, report } => {
            assert_eq!(source, CandidateSource::Witness);
            assert_eq!(mesh.state().vertices()[report.vertex], witness);
        }
        other => panic!("expected the witness to be kept, got {other:?}"),
    }
}

/// The sliver floor is the lever on mesh quality, not just a degeneracy guard.
///
/// Measured through the CLI at NXP 21 (guide 11.33, 11.34): a 5-degree floor
/// leaves a worst angle of 17.07 and 7723 cells; 30 leaves 30.00 and 7132.
/// Refusing an insertion that would make a thin triangle sends the ladder to a
/// later and more conservative rung, and the finished mesh tracks the floor.
///
/// Asserted as the relation rather than those figures: a higher floor gives a
/// better worst angle and costs cells. Pinning 28.12 would make the next person
/// to improve this edit the test first.
#[test]
fn a_higher_sliver_floor_buys_a_better_worst_angle() {
    let worst_angle = |mesh: &AdaptiveMesh| {
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
    };

    let run = |floor: f64| {
        let mut mesh = sphere(6);
        let gates = HardGates {
            min_triangle_angle_deg: floor,
            ..HardGates::default()
        };
        let radius = mesh.state().sphere_radius();
        for (lon, lat) in candidates(40) {
            let unit = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(lon, lat));
            let point = CartesianPoint::new(unit.x * radius, unit.y * radius, unit.z * radius);
            mesh.propose_site(point, gates).expect("propose");
        }
        (mesh.state().vertex_count(), worst_angle(&mesh))
    };

    let (loose_sites, loose_angle) = run(1.0);
    let (tight_sites, tight_angle) = run(30.0);

    assert!(
        tight_angle > loose_angle,
        "a 30-degree floor left {tight_angle:.2} and a 1-degree floor {loose_angle:.2}"
    );
    assert!(
        tight_angle >= 30.0,
        "the floor is a floor: {tight_angle:.2} is under it"
    );
    assert!(
        tight_sites <= loose_sites,
        "refusing thin triangles cannot add sites: {tight_sites} against {loose_sites}"
    );
}

/// A patch bigger than the run allows is refused, not snapshotted anyway.
///
/// `maximum_patch_cells` was declared, validated, and passed to nothing --
/// `api.rs` built `CycleLimits` without it and no transaction ever looked at a
/// patch's size. A bound accepted and ignored is what this crate's own config
/// note about `deterministic` says a flag must never be.
///
/// One is the tightest possible budget: an insertion's cavity plus its ring is
/// always more than one triangle, so this must refuse, and refuse by name
/// rather than by running out of something else.
#[test]
fn a_patch_over_budget_is_refused_by_name() {
    let mut mesh = sphere(6);
    let site = 40;
    let point = mesh.state().vertices()[site];
    let gates = HardGates {
        max_patch_triangles: 1,
        ..HardGates::default()
    };
    let target = crate::candidate::candidates_for_site(
        mesh.state(),
        site,
        None,
        crate::CandidatePolicy::default(),
    )
    .expect("ladder")
    .first()
    .map(|candidate| candidate.point)
    .unwrap_or(point);

    match mesh
        .propose_site_near(target, None, gates)
        .expect("proposal")
    {
        Acceptance::RolledBack(Rejection::PatchTooLarge { triangles, allowed }) => {
            assert_eq!(allowed, 1);
            assert!(triangles > 1, "the patch really was larger: {triangles}");
        }
        other => panic!("expected the patch bound to refuse, got {other:?}"),
    }

    // And with the bound generous the same change is refused for some other
    // reason or not at all -- either way not by the patch bound, so the refusal
    // above is that bound and not the change being impossible.
    let mut mesh = sphere(6);
    let generous = HardGates {
        max_patch_triangles: 10_000,
        ..HardGates::default()
    };
    assert!(
        !matches!(
            mesh.propose_site_near(target, None, generous)
                .expect("proposal"),
            Acceptance::RolledBack(Rejection::PatchTooLarge { .. })
        ),
        "a generous bound must not be what refuses it"
    );
}
