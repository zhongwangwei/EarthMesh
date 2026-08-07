use super::*;

use crate::mesh_state::MeshState;

fn base(nxp: usize) -> TriangularMesh {
    TriangularMesh::from_icosahedron(nxp, 0, 1.0, 0.25, 0).expect("base mesh")
}

/// A mesh survives the round trip through the neutral type.
///
/// The property that makes the neutral type usable at the output boundary at
/// all: a backend can take a mesh apart, work on the part it understands, and
/// hand back something the writers accept.
#[test]
fn a_mesh_round_trips_through_the_neutral_type() {
    let mesh = base(6);
    let state = MeshState::from_triangular_mesh(&mesh).expect("neutral state");
    let rebuilt = state
        .to_triangular_mesh(mesh.impent, None)
        .expect("three tables");

    assert_eq!(rebuilt.nmd, mesh.nmd);
    assert_eq!(rebuilt.nwd, mesh.nwd);
    assert_eq!(rebuilt.nud, mesh.nud);
    rebuilt.validate_topology().expect("a valid mesh");

    // Faces keep their ids, which is what every report and lineage depends on.
    for iw in MESH_STATE_FIRST_ID..=mesh.nwd {
        let mut here = rebuilt.w_faces[iw].im;
        let mut there = mesh.w_faces[iw].im;
        here.sort_unstable();
        there.sort_unstable();
        assert_eq!(here, there, "face {iw}");
    }

    // And taking it apart again gives the state it came from.
    let again = MeshState::from_triangular_mesh(&rebuilt).expect("neutral state");
    assert_eq!(again, state);
}

/// A refined mesh round-trips too, which is the case that matters.
#[test]
fn a_mesh_with_an_inserted_site_round_trips() {
    let mesh = base(6);
    let mut state = MeshState::from_triangular_mesh(&mesh).expect("neutral state");
    let radius = state.sphere_radius();
    let unit = crate::lonlat_degrees_to_unit_xyz(crate::LonLatDegrees::new(41.0, 19.0));
    state
        .insert_site(crate::CartesianPoint::new(
            unit.x * radius,
            unit.y * radius,
            unit.z * radius,
        ))
        .expect("insert");

    let rebuilt = state
        .to_triangular_mesh(mesh.impent, None)
        .expect("three tables");
    assert_eq!(rebuilt.nmd, mesh.nmd + 1);
    assert_eq!(rebuilt.nwd, mesh.nwd + 2);
    rebuilt.validate_topology().expect("a valid mesh");
    assert_eq!(
        MeshState::from_triangular_mesh(&rebuilt).expect("neutral state"),
        state
    );
}

/// Per-face generations land where the writers read them.
#[test]
fn face_levels_are_carried_into_the_tables() {
    let mesh = base(6);
    let state = MeshState::from_triangular_mesh(&mesh).expect("neutral state");
    let mut levels = vec![1usize; state.triangles().len()];
    for level in levels.iter_mut().skip(10).take(5) {
        *level = 3;
    }

    let rebuilt = state
        .to_triangular_mesh(mesh.impent, Some(&levels))
        .expect("three tables");
    for iw in 10..15 {
        assert_eq!(rebuilt.w_faces[iw].mrlw, 3, "face {iw}");
    }
    assert_eq!(rebuilt.w_faces[9].mrlw, 1);
}

/// A pentagon id that is not a site of the mesh is refused.
#[test]
fn a_pentagon_id_the_mesh_does_not_carry_is_refused() {
    let mesh = base(6);
    let state = MeshState::from_triangular_mesh(&mesh).expect("neutral state");
    let mut impent = mesh.impent;
    impent[3] = state.vertices().len();
    let error = state
        .to_triangular_mesh(impent, None)
        .expect_err("that id names nothing");
    assert!(error.to_string().contains("not a site"), "{error}");
}

/// Refinement makes more than twelve degree-5 sites, which is why the twelve
/// are carried rather than found.
///
/// Euler gives `#degree-5 - #degree-7 = 12` on a sphere. One insertion is
/// enough to make a degree-7 site and so a thirteenth and fourteenth degree-5
/// one; a derivation from degree would put a different twelve in the file
/// every time the mesh changed.
#[test]
fn a_refined_mesh_has_more_than_twelve_degree_five_sites() {
    let mesh = base(6);
    let mut state = MeshState::from_triangular_mesh(&mesh).expect("neutral state");
    let count = |state: &MeshState| {
        (MESH_STATE_FIRST_ID..state.vertices().len())
            .filter(|&site| state.vertex_degree(site) == Ok(5))
            .count()
    };
    assert_eq!(
        count(&state),
        12,
        "an unrefined icosahedral mesh has twelve"
    );

    let radius = state.sphere_radius();
    let unit = crate::lonlat_degrees_to_unit_xyz(crate::LonLatDegrees::new(41.0, 19.0));
    state
        .insert_site(crate::CartesianPoint::new(
            unit.x * radius,
            unit.y * radius,
            unit.z * radius,
        ))
        .expect("insert");
    assert!(
        count(&state) > 12,
        "one insertion left {} degree-5 sites",
        count(&state)
    );
    // And the carried twelve still name sites, so the mesh still writes.
    state
        .to_triangular_mesh(mesh.impent, None)
        .expect("three tables");
}
