use super::*;
use crate::{
    lonlat_degrees_to_unit_xyz, normalize_cartesian_to_radius, LonLatDegrees, TriangularMesh,
};

fn sphere(nxp: usize) -> MeshState {
    let mesh = TriangularMesh::from_icosahedron(nxp, 0, 1.0, 0.25, 0).expect("base mesh");
    MeshState::from_triangular_mesh(&mesh).expect("neutral state")
}

/// A point on the same sphere the mesh lives on.
///
/// The meshes here are in metres. A unit vector sits, as far as the predicates
/// are concerned, at the centre of that sphere, and inserting one produces a
/// mesh that closes and is not Delaunay -- which is how this helper came to
/// take the state rather than just a longitude and a latitude.
fn on(state: &MeshState, lon: f64, lat: f64) -> CartesianPoint {
    let unit = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(lon, lat));
    let radius = state.sphere_radius();
    CartesianPoint::new(unit.x * radius, unit.y * radius, unit.z * radius)
}

/// Every triangle's circumcircle is empty of every other site: the definition,
/// checked directly rather than trusted from the construction. Quadratic, so
/// only run on small meshes.
fn delaunay_violations(state: &MeshState) -> usize {
    let mut violations = 0;
    for triangle in MESH_STATE_FIRST_ID..state.triangles().len() {
        let corners = state.triangles()[triangle];
        for vertex in MESH_STATE_FIRST_ID..state.vertices().len() {
            if corners.contains(&vertex) {
                continue;
            }
            let inside = in_circle_on_sphere(
                state.vertices()[corners[0]],
                state.vertices()[corners[1]],
                state.vertices()[corners[2]],
                state.vertices()[vertex],
            );
            if inside == Ok(Sign::Positive) {
                violations += 1;
            }
        }
    }
    violations
}

/// The walk finds a triangle that really contains the point.
#[test]
fn the_walk_settles_on_a_triangle_that_contains_the_point() {
    let state = sphere(6);
    for (lon, lat) in [(0.0, 0.0), (115.0, 25.0), (-60.0, -70.0), (179.0, 5.0)] {
        let point = on(&state, lon, lat);
        let triangle = state
            .locate_triangle(point, None)
            .unwrap_or_else(|error| panic!("locate {lon},{lat}: {error}"));

        // Containment verified independently of the walk: the point is on the
        // same side of all three edges as the triangle's own winding.
        let corners = state.triangles()[triangle];
        let winding = orientation_on_sphere(
            state.vertices()[corners[0]],
            state.vertices()[corners[1]],
            state.vertices()[corners[2]],
        )
        .expect("clear");
        for corner in 0..3 {
            let side = orientation_on_sphere(
                state.vertices()[corners[(corner + 1) % 3]],
                state.vertices()[corners[(corner + 2) % 3]],
                point,
            )
            .expect("clear");
            assert!(
                !matches!(
                    (winding, side),
                    (Sign::Positive, Sign::Negative) | (Sign::Negative, Sign::Positive)
                ),
                "the point is outside edge {corner} of the triangle the walk chose"
            );
        }
    }
}

/// A hint changes the route, not the destination.
#[test]
fn the_walk_reaches_the_same_triangle_from_any_hint() {
    let state = sphere(6);
    let point = on(&state, 45.0, 30.0);
    let expected = state.locate_triangle(point, None).expect("locate");
    for hint in [MESH_STATE_FIRST_ID, 20, 100, state.triangles().len() - 1] {
        assert_eq!(
            state.locate_triangle(point, Some(hint)).expect("locate"),
            expected
        );
    }
}

/// One insertion: the sphere stays closed and gains exactly two triangles.
///
/// A cavity of k triangles leaves k + 2 boundary edges and the fan makes one
/// triangle per edge, so the count rises by two however large the cavity was.
#[test]
fn inserting_a_site_keeps_the_sphere_closed_and_adds_two_triangles() {
    let mut state = sphere(6);
    let before_triangles = state.triangle_count();
    let before_vertices = state.vertex_count();
    let point = on(&state, 37.0, 21.0);

    let report = state.insert_site(point).expect("insert");

    assert_eq!(state.vertex_count(), before_vertices + 1);
    assert_eq!(state.triangle_count(), before_triangles + 2);
    assert_eq!(report.created.len(), report.removed.len() + 2);
    assert_eq!(
        state.open_edge_count(),
        0,
        "the sphere is still closed after the cavity was refilled"
    );
    state.validate().expect("adjacency is symmetric again");
}

/// Euler survives, which is what says the topology is right and not only the
/// counts.
#[test]
fn euler_still_holds_after_several_insertions() {
    let mut state = sphere(6);
    for (lon, lat) in [
        (10.0, 10.0),
        (-30.0, 40.0),
        (100.0, -20.0),
        (170.0, 60.0),
        (-150.0, -55.0),
    ] {
        let point = on(&state, lon, lat);
        state.insert_site(point).expect("insert");
    }
    let faces = state.triangle_count();
    let edges = faces * 3 / 2;
    let vertices = state.vertex_count();
    assert_eq!(
        vertices as isize - edges as isize + faces as isize,
        2,
        "V {vertices} - E {edges} + F {faces}"
    );
    assert_eq!(state.open_edge_count(), 0);
    state.validate().expect("valid");
}

/// The result is Delaunay, checked against the definition rather than taken
/// from the algorithm's reputation.
#[test]
fn the_triangulation_is_delaunay_before_and_after_inserting() {
    let mut state = sphere(4);
    assert_eq!(
        delaunay_violations(&state),
        0,
        "the icosahedral mesh is already Delaunay"
    );
    let first = on(&state, 23.0, 17.0);
    state.insert_site(first).expect("insert");
    assert_eq!(
        delaunay_violations(&state),
        0,
        "cavity insertion leaves nothing for a legalization pass to do"
    );
    let second = on(&state, -88.0, -33.0);
    state.insert_site(second).expect("insert");
    assert_eq!(delaunay_violations(&state), 0);
}

/// Everything outside the cavity is untouched.
///
/// The property the whole local approach exists for. Without it an insertion is
/// a global rebuild wearing a local interface.
#[test]
fn nothing_outside_the_cavity_changes() {
    let mut state = sphere(6);
    let before = state.clone();
    let point = on(&state, 60.0, 12.0);
    let report = state.insert_site(point).expect("insert");

    let disturbed: BTreeSet<usize> = report
        .removed
        .iter()
        .chain(report.created.iter())
        .copied()
        .collect();
    for triangle in MESH_STATE_FIRST_ID..before.triangles().len() {
        if disturbed.contains(&triangle) {
            continue;
        }
        assert_eq!(
            state.triangles()[triangle],
            before.triangles()[triangle],
            "triangle {triangle} is outside the cavity and its corners moved"
        );
    }
    for vertex in MESH_STATE_FIRST_ID..before.vertices().len() {
        assert_eq!(
            state.vertices()[vertex],
            before.vertices()[vertex],
            "vertex {vertex} moved, and an insertion moves nothing"
        );
    }
}

/// A point already carried by the mesh is refused.
#[test]
fn inserting_a_site_that_is_already_there_is_refused() {
    let mut state = sphere(6);
    let existing = state.vertices()[MESH_STATE_FIRST_ID + 3];
    let error = state
        .insert_site(existing)
        .expect_err("a duplicate site is not an insertion");
    assert!(matches!(error, InsertionError::Duplicate { .. }), "{error}");
}

/// A point off the mesh's sphere is refused rather than inserted.
///
/// This is the mistake that found the guard: a unit vector into a mesh in
/// metres. It closed, its winding was consistent, Euler held, and every new
/// triangle's circumcircle held half the mesh. Nothing except a direct Delaunay
/// check would have said so.
#[test]
fn a_point_off_the_meshs_sphere_is_refused_rather_than_quietly_wrong() {
    let mut state = sphere(6);
    let unit = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(23.0, 17.0));
    let error = state
        .insert_site(unit)
        .expect_err("a unit vector is not a point on a mesh measured in metres");
    match error {
        InsertionError::OffSphere {
            candidate_radius,
            mesh_radius,
        } => {
            assert!((candidate_radius - 1.0).abs() < 1e-9);
            assert!(mesh_radius > 1.0e6);
        }
        other => panic!("expected an off-sphere refusal, got {other}"),
    }
}

/// The same insertions in the same order give the same mesh.
#[test]
fn insertion_is_deterministic() {
    let build = || {
        let mut state = sphere(6);
        for (lon, lat) in [(5.0, 5.0), (95.0, -15.0), (-140.0, 44.0)] {
            let point = on(&state, lon, lat);
            state.insert_site(point).expect("insert");
        }
        state
    };
    assert_eq!(build(), build());
}

/// Insertion breaks the degree-7 cap almost at once.
///
/// Method-C holds vertex degree to {5, 6, 7} with its transition rows and
/// red-green holds it to 7 with its judge chain, because the gridfile cannot
/// carry more: `ItabW`'s `im`/`iv`/`iw` are `[i32; 7]`, and the ring walk in
/// `icosahedron_m_neighbors` refuses a valence above seven -- loudly, as a
/// repairable error, not by truncating.
///
/// Plain Delaunay insertion promises nothing about degree, and this measures
/// how little room that leaves: ten sites into an NXP 6 mesh is enough. So a
/// backend that inserts has to gate on degree itself; there is no depth of
/// insertion at which the question can be deferred.
#[test]
fn inserting_sites_passes_the_degree_the_gridfile_can_carry() {
    let mut state = sphere(6);
    let worst = |state: &MeshState| {
        (MESH_STATE_FIRST_ID..state.vertices().len())
            .filter_map(|site| state.vertex_degree(site).ok())
            .max()
            .unwrap_or(0)
    };
    assert_eq!(
        worst(&state),
        6,
        "an unrefined icosahedral mesh has none over 6"
    );

    let mut inserted = 0;
    let mut first_over_seven = None;
    for step in 0..40 {
        let lon = -180.0 + f64::from(step) * 6.1;
        let lat = -70.0 + f64::from((step * 37) % 140);
        if state.insert_site(on(&state, lon, lat)).is_ok() {
            inserted += 1;
            if first_over_seven.is_none() && worst(&state) > 7 {
                first_over_seven = Some(inserted);
            }
        }
    }
    let broke_at = first_over_seven.expect("the cap does get broken");
    assert!(
        broke_at <= 15,
        "the cap survived {broke_at} insertions; it was measured to break by ten"
    );
}

#[test]
fn insertion_reuse_invalidates_old_face_id_but_keeps_old_vertex_ids() {
    let mut state = sphere(6);
    let point = on(&state, 37.0, 21.0);
    let containing = state.locate_triangle(point, None).expect("locate");
    let old_face = state.face_id(containing).expect("face id");
    let old_vertex = state.vertex_id(MESH_STATE_FIRST_ID).expect("vertex id");

    let report = state.insert_site(point).expect("insert");

    assert!(report.removed.contains(&containing));
    assert!(report.removed_ids.contains(&old_face));
    assert!(state.contains_vertex_id(report.site_id));
    assert!(report
        .created_ids
        .iter()
        .all(|&face| state.contains_face_id(face)));
    assert!(!state.contains_face_id(old_face));
    assert!(state.contains_vertex_id(old_vertex));
}

#[test]
fn transaction_reject_restores_topology_and_active_ids() {
    let mut state = sphere(6);
    let before = state.clone();
    let face = state.face_id(10).expect("face id");
    let vertex = state.vertex_id(10).expect("vertex id");

    let error = state
        .insert_site_transactionally(on(&state, 41.0, 19.0), |_, _| false)
        .expect_err("forced rejection rolls back");

    assert!(matches!(error, InsertionTransactionError::Rejected));
    assert_eq!(state, before);
    assert!(state.contains_face_id(face));
    assert!(state.contains_vertex_id(vertex));
    state.validate().expect("valid after rollback");
}

#[test]
fn rollback_does_not_reissue_transient_ids() {
    let mut state = sphere(6);
    let first = on(&state, 41.0, 19.0);
    let containing = state.locate_triangle(first, None).expect("locate");
    let cavity = state.delaunay_cavity(first, containing).expect("cavity");
    let patch = state.snapshot_around(&cavity);
    let transient = state.insert_site(first).expect("insert");
    let transient_site = state.vertex_id(transient.site).expect("transient site");
    let transient_faces: Vec<_> = transient
        .created
        .iter()
        .map(|&slot| state.face_id(slot).expect("transient face"))
        .collect();

    state.restore_patch(patch).expect("rollback");
    let second = state.insert_site(on(&state, -35.0, 44.0)).expect("second");
    let second_site = state.vertex_id(second.site).expect("second site");
    assert_eq!(second_site.slot, transient_site.slot);
    assert_ne!(second_site, transient_site);

    for face in transient_faces {
        if let Some(new_face) = state.face_id(face.slot) {
            assert_ne!(new_face, face, "transient face id was reissued");
        }
    }
}

#[test]
fn degree_forecast_is_exact_for_an_on_edge_split() {
    let mut state = sphere(6);
    let [tail, head, _] = state.triangles()[20];
    let a = state.vertices()[tail];
    let b = state.vertices()[head];
    let point = normalize_cartesian_to_radius(
        CartesianPoint::new(a.x + b.x, a.y + b.y, a.z + b.z),
        state.sphere_radius(),
    )
    .expect("edge midpoint");
    let tail_before = state.vertex_degree(tail).expect("tail degree");
    let head_before = state.vertex_degree(head).expect("head degree");
    let forecast = state.forecast_degrees(point, Some(20)).expect("forecast");

    let report = state.insert_site(point).expect("split edge");
    let changed: BTreeSet<_> = report.created.iter().copied().collect();
    let actual_worst = state
        .sites_touching(&changed)
        .into_iter()
        .filter(|&(site, _)| site != report.site)
        .map(|(site, seed)| state.vertex_degree_from(site, seed).expect("degree"))
        .max()
        .expect("ring");

    assert_eq!(forecast.new_site, state.vertex_degree(report.site).unwrap());
    assert_eq!(forecast.worst_neighbour, actual_worst);
    assert_eq!(state.vertex_degree(tail), Ok(tail_before));
    assert_eq!(state.vertex_degree(head), Ok(head_before));
}

#[test]
fn boundary_edge_insertion_splits_open_edge_without_degenerate_fan() {
    let mut state = MeshState::from_parts(
        vec![
            CartesianPoint::new(0.0, 0.0, 0.0),
            CartesianPoint::new(0.0, 0.0, 0.0),
            p(0.0, 0.0),
            p(90.0, 0.0),
            p(0.0, 90.0),
        ],
        vec![[1, 1, 1], [1, 1, 1], [2, 3, 4]],
    )
    .expect("open triangle");
    let point = normalize_cartesian_to_radius(
        CartesianPoint::new(
            state.vertices()[2].x + state.vertices()[3].x,
            state.vertices()[2].y + state.vertices()[3].y,
            state.vertices()[2].z + state.vertices()[3].z,
        ),
        state.sphere_radius(),
    )
    .expect("midpoint");

    let report = state
        .insert_site_on_boundary_edge_transactionally(point, 2, 3, |_, _| true)
        .expect("boundary insert");

    assert_eq!(state.vertex_count(), 4);
    assert_eq!(state.triangle_count(), 2);
    assert_eq!(state.open_edge_count(), 4);
    state.validate().expect("valid open mesh");
    assert!(state.triangles()[MESH_STATE_FIRST_ID..]
        .iter()
        .all(|corners| !(corners.contains(&2) && corners.contains(&3))));
    assert!(state.triangles()[MESH_STATE_FIRST_ID..]
        .iter()
        .any(|corners| corners.contains(&2) && corners.contains(&report.site)));
    assert!(state.triangles()[MESH_STATE_FIRST_ID..]
        .iter()
        .any(|corners| corners.contains(&3) && corners.contains(&report.site)));
}

fn p(lon: f64, lat: f64) -> CartesianPoint {
    lonlat_degrees_to_unit_xyz(LonLatDegrees::new(lon, lat))
}
