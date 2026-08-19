use super::*;
use crate::{lonlat_degrees_to_unit_xyz, CartesianPoint, LonLatDegrees, TriangularMesh};

fn sphere(nxp: usize) -> MeshState {
    let mesh = TriangularMesh::from_icosahedron(nxp, 0, 1.0, 0.25).expect("base mesh");
    MeshState::from_triangular_mesh(&mesh).expect("neutral state")
}

fn on(state: &MeshState, lon: f64, lat: f64) -> CartesianPoint {
    let unit = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(lon, lat));
    let radius = state.sphere_radius();
    CartesianPoint::new(unit.x * radius, unit.y * radius, unit.z * radius)
}

/// The point of the whole module: an insertion, undone, leaves nothing.
///
/// Compared against the entire prior state rather than against a count or a
/// checksum, because a rollback that restores the right number of triangles
/// with the wrong adjacency is exactly the failure this exists to prevent.
#[test]
fn restoring_a_patch_undoes_an_insertion_completely() {
    let mut state = sphere(6);
    let before = state.clone();

    let point = on(&state, 41.0, 19.0);
    let containing = state.locate_triangle(point, None).expect("locate");
    let cavity = state.delaunay_cavity(point, containing).expect("cavity");
    let patch = state.snapshot_around(&cavity);

    state.insert_site(point).expect("insert");
    assert_ne!(state, before, "the insertion did change something");

    state.restore_patch(patch).expect("restore");
    assert_eq!(state, before, "and the restore changed it all back");
    state.validate().expect("valid");
    assert_eq!(state.open_edge_count(), 0);
}

/// The ring outside the change has to be in the patch.
///
/// Restoring only the triangles that were overwritten leaves their neighbours
/// naming the triangles the insertion made -- a mesh that is neither the old
/// one nor the new one. This test pins that `snapshot_around` widens the seed
/// rather than trusting the caller to.
#[test]
fn a_patch_covers_the_ring_and_not_only_the_seed() {
    let state = sphere(6);
    let seed: BTreeSet<usize> = [10usize].into_iter().collect();
    let patch = state.snapshot_around(&seed);

    let covered: BTreeSet<usize> = patch.triangles().collect();
    assert!(covered.contains(&10));
    for neighbour in state.neighbours()[10] {
        assert!(
            covered.contains(&neighbour),
            "triangle {neighbour} is across an edge of the seed and is not in the patch"
        );
    }
    assert_eq!(patch.len(), 4, "the seed and its three neighbours");
}

/// Rolling back one insertion out of several restores that one.
#[test]
fn a_patch_restores_the_last_change_and_leaves_the_earlier_ones() {
    let mut state = sphere(6);
    state.insert_site(on(&state, 10.0, 10.0)).expect("first");
    state.insert_site(on(&state, 80.0, -20.0)).expect("second");
    let after_two = state.clone();

    let point = on(&state, -35.0, 44.0);
    let containing = state.locate_triangle(point, None).expect("locate");
    let cavity = state.delaunay_cavity(point, containing).expect("cavity");
    let patch = state.snapshot_around(&cavity);
    state.insert_site(point).expect("third");

    state.restore_patch(patch).expect("restore");
    assert_eq!(
        state, after_two,
        "the third insertion is gone and the first two are not"
    );
}

/// A patch is an undo, so it refuses a mesh that has shrunk under it.
#[test]
fn a_patch_refuses_a_mesh_that_lost_what_it_recorded() {
    let mut state = sphere(6);
    let patch = state.snapshot_around(&[8usize, 9].into_iter().collect());
    let mut smaller = sphere(4);

    let error = smaller
        .restore_patch(patch)
        .expect_err("these rows do not fit in this mesh");
    assert!(
        matches!(error, PatchError::MeshShrankBelowThePatch { .. }),
        "{error}"
    );

    // And the refusal is a refusal: nothing was half-applied on the way out.
    assert_eq!(smaller, sphere(4));
    let _ = &mut state;
}

/// Ids outside the patch keep their meaning across a restore.
///
/// A rollback that renumbered would invalidate every id the layer above is
/// holding -- demands, lineage, the cells it is still deciding about.
#[test]
fn a_restore_renumbers_nothing() {
    let mut state = sphere(6);
    let before = state.clone();
    let point = on(&state, 120.0, -8.0);
    let containing = state.locate_triangle(point, None).expect("locate");
    let cavity = state.delaunay_cavity(point, containing).expect("cavity");
    let patch = state.snapshot_around(&cavity);
    state.insert_site(point).expect("insert");
    state.restore_patch(patch).expect("restore");

    for triangle in MESH_STATE_FIRST_ID..before.triangles().len() {
        assert_eq!(state.triangles()[triangle], before.triangles()[triangle]);
    }
    assert_eq!(state.vertex_count(), before.vertex_count());
}
