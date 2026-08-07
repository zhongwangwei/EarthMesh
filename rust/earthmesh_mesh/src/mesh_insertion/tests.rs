use super::*;
use crate::{lonlat_degrees_to_unit_xyz, LonLatDegrees, TriangularMesh};

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
