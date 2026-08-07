//! A run through the public entry point, start to finish.
//!
//! The unit tests exercise each layer against its own contract. This asserts
//! the thing a caller actually depends on: hand in a mesh and a target, get
//! back a finer mesh where the target applied, a report that says what
//! happened, and a triangulation that still holds together.

use earthmesh_mesh::{
    lonlat_degrees_to_unit_xyz, LonLatDegrees, TriangularMesh, MESH_STATE_FIRST_ID,
};
use earthmesh_refine_harp_dv::{
    refine_harp_dv, AdaptiveMesh, CandidatePolicy, CellCriterion, CellView, HardGates,
    HarpDvConfig, HarpDvRequest, StopReason, TargetRegion, TargetScale, GRIDFILE_MAX_VERTEX_DEGREE,
};

fn base() -> AdaptiveMesh {
    let mesh = TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25, 0).expect("base mesh");
    AdaptiveMesh::from_triangular_mesh(&mesh).expect("adaptive mesh")
}

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

#[test]
fn a_regional_target_produces_a_finer_mesh_that_still_holds_together() {
    let mesh = base();
    let coarsest = coarsest_scale(&mesh);
    let centre = LonLatDegrees::new(105.0, 35.0);
    let before_sites = mesh.active_site_count();

    let criteria: Vec<Box<dyn CellCriterion>> = vec![Box::new(TargetScale {
        id: "regional-target".to_string(),
        target_scale_m: coarsest * 0.7,
        region: TargetRegion::Circle {
            centre,
            radius_m: 2_500_000.0,
        },
        source_resolution_m: None,
    })];

    let outcome = refine_harp_dv(
        mesh,
        &HarpDvRequest {
            config: HarpDvConfig::default(),
            criteria: &criteria,
            candidate_policy: CandidatePolicy::default(),
            gates: HardGates::default(),
        },
    )
    .expect("run");

    let report = &outcome.report;
    assert!(report.transactions_committed > 0, "{report:?}");
    assert_eq!(
        report.final_sites,
        before_sites + report.transactions_committed
    );
    assert!(
        matches!(
            report.stop_reason,
            StopReason::AllSatisfied | StopReason::NoAcceptedTransactions
        ),
        "{report:?}"
    );

    // The mesh is still one a gridfile could carry.
    let state = outcome.mesh.state();
    assert_eq!(state.open_edge_count(), 0);
    state.validate().expect("a triangulation");
    for site in MESH_STATE_FIRST_ID..state.vertices().len() {
        assert!(state.vertex_degree(site).expect("degree") <= GRIDFILE_MAX_VERTEX_DEGREE);
    }

    // Euler, which is what says the topology is right and not only the counts.
    let faces = state.triangle_count();
    let edges = faces * 3 / 2;
    let vertices = state.vertex_count();
    assert_eq!(vertices as isize - edges as isize + faces as isize, 2);

    // The refinement landed where it was asked for.
    let radius = state.sphere_radius();
    let unit_centre = lonlat_degrees_to_unit_xyz(centre);
    let added: Vec<_> = outcome
        .mesh
        .sites()
        .iter()
        .filter(|site| site.birth_cycle > 0)
        .collect();
    assert_eq!(added.len(), report.transactions_committed);
    for site in added {
        let unit = lonlat_degrees_to_unit_xyz(site.position);
        let dot = (unit.x * unit_centre.x + unit.y * unit_centre.y + unit.z * unit_centre.z)
            .clamp(-1.0, 1.0);
        assert!(
            dot.acos() * radius <= 2_500_000.0 + coarsest * 2.0,
            "a site was added at {:?}, well outside the region that asked",
            site.position
        );
    }
}

/// A request with no criteria is a run with nothing to do, not an error.
#[test]
fn a_run_with_no_criteria_returns_the_mesh_it_was_given() {
    let mesh = base();
    let before = mesh.state().clone();
    let outcome = refine_harp_dv(
        mesh,
        &HarpDvRequest {
            config: HarpDvConfig::default(),
            criteria: &[],
            candidate_policy: CandidatePolicy::default(),
            gates: HardGates::default(),
        },
    )
    .expect("run");
    assert_eq!(outcome.report.stop_reason, StopReason::AllSatisfied);
    assert_eq!(outcome.report.transactions_attempted, 0);
    assert_eq!(outcome.mesh.state(), &before);
}
