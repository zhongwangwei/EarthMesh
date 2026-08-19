//! Phase 1 covers four things: a config that refuses what it cannot honour,
//! ids that stay meaningful, a mesh that survives being wrapped, and an empty
//! run that is honest and repeatable.

use super::*;
use earthmesh_mesh::{TriangularMesh, MESH_STATE_FIRST_ID};

fn base_mesh() -> TriangularMesh {
    TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25).expect("base mesh")
}

/// Each field is checked, and the message names the field.
#[test]
fn a_config_that_cannot_be_honoured_is_refused_with_the_field_that_is_wrong() {
    HarpDvConfig::default()
        .validate()
        .expect("the default has to be runnable");

    let cases: [(HarpDvConfig, &str); 6] = [
        (
            HarpDvConfig {
                max_cycles: 0,
                ..HarpDvConfig::default()
            },
            "max_cycles",
        ),
        (
            HarpDvConfig {
                minimum_cell_width_m: f64::NAN,
                ..HarpDvConfig::default()
            },
            "minimum_cell_width_m",
        ),
        (
            HarpDvConfig {
                maximum_cells: 0,
                ..HarpDvConfig::default()
            },
            "maximum_cells",
        ),
        (
            HarpDvConfig {
                maximum_patch_cells: 0,
                ..HarpDvConfig::default()
            },
            "maximum_patch_cells",
        ),
        (
            HarpDvConfig {
                maximum_neighbor_scale_ratio: 1.0,
                ..HarpDvConfig::default()
            },
            "maximum_neighbor_scale_ratio",
        ),
        (
            HarpDvConfig {
                deterministic: false,
                ..HarpDvConfig::default()
            },
            "deterministic",
        ),
    ];
    for (config, field) in cases {
        let error = config
            .validate()
            .expect_err("this config describes a run that cannot be made");
        assert!(
            error.to_string().contains(field),
            "the message has to name {field}: {error}"
        );
    }
}

/// A patch larger than the mesh is a contradiction, not a large patch.
#[test]
fn a_patch_budget_larger_than_the_mesh_budget_is_refused() {
    let error = HarpDvConfig {
        maximum_cells: 100,
        maximum_patch_cells: 101,
        ..HarpDvConfig::default()
    }
    .validate()
    .expect_err("a patch cannot be larger than the mesh it sits in");
    assert!(
        error.to_string().contains("exceeds maximum_cells"),
        "{error}"
    );
}

/// Ids go forward and never come back.
///
/// A freed id handed out again would make an old lineage row, report row or
/// checkpoint point at a site that is not the one it meant.
#[test]
fn site_ids_are_monotonic_and_never_reissued() {
    let mut allocator = SiteIdAllocator::default();
    let issued: Vec<SiteId> = (0..5).map(|_| allocator.allocate()).collect();
    assert_eq!(
        issued,
        (0..5).map(SiteId).collect::<Vec<_>>(),
        "ids start at zero and step by one"
    );
    assert_eq!(allocator.peek(), SiteId(5));

    let mut resumed = SiteIdAllocator::starting_at(allocator.issued());
    assert_eq!(
        resumed.allocate(),
        SiteId(5),
        "a resumed allocator must not tread on what was already handed out"
    );
}

/// Wrapping a mesh gives every real M point an id, and nothing else one.
#[test]
fn wrapping_a_mesh_gives_every_site_an_identity() {
    let mesh = base_mesh();
    let expected = mesh.nmd - 1;
    let adaptive = AdaptiveMesh::from_triangular_mesh(&mesh).expect("wrap");

    assert_eq!(
        adaptive.sites().len(),
        expected,
        "slots 0 and 1 are canonical placeholders, not sites"
    );
    assert_eq!(adaptive.active_site_count(), expected);
    assert_eq!(adaptive.next_site_id(), SiteId(expected as u64));

    let mut ids: Vec<u64> = adaptive.sites().iter().map(|site| site.site_id.0).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), expected, "no id is handed out twice");

    for (offset, site) in adaptive.sites().iter().enumerate() {
        let vertex = MESH_STATE_FIRST_ID + offset;
        assert_eq!(adaptive.vertex_for_site_id(site.site_id), Some(vertex));
        assert_eq!(
            adaptive.site_for_vertex(vertex).map(|site| site.site_id),
            Some(site.site_id)
        );
        assert_eq!(
            site.position, site.origin_position,
            "an inherited site has not moved yet"
        );
        assert_eq!(site.cumulative_displacement_m, 0.0);
        assert_eq!(site.birth_cycle, 0, "it came in with the mesh");
        assert_eq!(site.parent_site_id, None, "inherited sites have no parent");
    }
}

/// A mesh with no triangulation in it is refused rather than wrapped.
#[test]
fn a_mesh_with_no_sites_is_refused() {
    let mut mesh = base_mesh();
    mesh.nmd = 1;
    let error = AdaptiveMesh::from_triangular_mesh(&mesh)
        .expect_err("a mesh with no sites carries nothing to adapt");
    assert!(error.to_string().contains("no triangulation"), "{error}");
}

/// The wrapper takes the triangulation and leaves Method-C's bookkeeping.
///
/// This is what makes HARP-DV a sibling of the other backends rather than
/// something built on one of them.
#[test]
fn wrapping_keeps_the_triangulation_and_drops_the_generations() {
    let mesh = base_mesh();
    let adaptive = AdaptiveMesh::from_triangular_mesh(&mesh).expect("wrap");
    let state = adaptive.state();
    assert_eq!(state.vertex_count(), mesh.nmd - 1);
    assert_eq!(state.triangle_count(), mesh.nwd - 1);
    assert_eq!(
        state.open_edge_count(),
        0,
        "the triangulation still closes after the generations are dropped"
    );
}

/// Phase 1's contract: nothing asked for, nothing changed, and it says so.
#[test]
fn a_request_with_nothing_to_do_returns_the_mesh_it_was_given() {
    let mesh = base_mesh();
    let faces_before = mesh.nwd;
    let points_before = mesh.nmd;
    let adaptive = AdaptiveMesh::from_triangular_mesh(&mesh).expect("wrap");
    let sites_before = adaptive.active_site_count();

    let outcome = refine_harp_dv(
        adaptive,
        &HarpDvRequest {
            config: HarpDvConfig::default(),
            criteria: &[],
            candidate_policy: crate::CandidatePolicy::default(),
            gates: crate::HardGates::default(),
        },
    )
    .expect("an empty request is a run with nothing to do, not an error");

    assert_eq!(outcome.report.stop_reason, StopReason::AllSatisfied);
    assert_eq!(outcome.report.cycles_completed, 0);
    assert_eq!(outcome.report.initial_sites, sites_before);
    assert_eq!(outcome.report.final_sites, sites_before);
    assert_eq!(outcome.report.transactions_attempted, 0);
    assert_eq!(outcome.report.unresolved_count, 0);
    assert_eq!(
        outcome.report.angle_window_40_80_verdict,
        AngleWindowVerdict::NotEvaluated,
        "an empty request must not claim that it measured mesh quality"
    );
    assert!(outcome.report.deterministic);
    assert_eq!(
        outcome.report.schema_version,
        HarpDvRunReport::SCHEMA_VERSION
    );

    let returned = outcome.mesh.into_state();
    assert_eq!(
        returned.triangle_count(),
        faces_before - 1,
        "the mesh came back untouched"
    );
    assert_eq!(returned.vertex_count(), points_before - 1);
}

/// The same request twice gives the same answer twice.
#[test]
fn two_identical_empty_runs_agree() {
    let run = || {
        let adaptive = AdaptiveMesh::from_triangular_mesh(&base_mesh()).expect("wrap");
        refine_harp_dv(
            adaptive,
            &HarpDvRequest {
                config: HarpDvConfig::default(),
                criteria: &[],
                candidate_policy: crate::CandidatePolicy::default(),
                gates: crate::HardGates::default(),
            },
        )
        .expect("empty run")
    };
    let first = run();
    let second = run();
    assert_eq!(first.report, second.report);
    assert_eq!(
        first.mesh.sites(),
        second.mesh.sites(),
        "identity is part of the answer, so it has to be reproducible"
    );
}

/// An invalid config is refused before any work, not partway through.
#[test]
fn a_run_validates_its_config_before_touching_the_mesh() {
    let adaptive = AdaptiveMesh::from_triangular_mesh(&base_mesh()).expect("wrap");
    let error = refine_harp_dv(
        adaptive,
        &HarpDvRequest {
            config: HarpDvConfig {
                max_cycles: 0,
                ..HarpDvConfig::default()
            },
            criteria: &[],
            candidate_policy: crate::CandidatePolicy::default(),
            gates: crate::HardGates::default(),
        },
    )
    .expect_err("a run that cannot be made must not start");
    assert!(matches!(error, HarpDvError::InvalidConfig(_)), "{error}");
}

/// Evidence that asks for nothing does not ask for anything.
#[test]
fn satisfied_evidence_demands_no_work() {
    let evidence = DemandEvidence::satisfied("point_radius", CriterionSemantics::TargetScale);
    assert!(!evidence.demands_work());
    assert_eq!(
        evidence.stop_reason,
        Some(EvidenceStopReason::AlreadySatisfied)
    );
}
