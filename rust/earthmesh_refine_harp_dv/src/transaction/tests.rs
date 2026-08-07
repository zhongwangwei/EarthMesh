use super::*;

use earthmesh_mesh::{lonlat_degrees_to_unit_xyz, LonLatDegrees, TriangularMesh};

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
    let gates = HardGates::default();
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
    let gates = HardGates::default();
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

    let outcome = mesh
        .propose_site(point, HardGates::default())
        .expect("propose");
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
                mesh.propose_site(point, HardGates::default())
                    .expect("propose")
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
        let outcome = mesh
            .propose_site(point, HardGates::default())
            .expect("propose");
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
            mesh.propose_site_near(point, hint, HardGates::default())
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
