use super::*;

use crate::{
    lonlat_degrees_to_unit_xyz, CartesianPoint, LonLatDegrees, TriangularMesh, MESH_STATE_FIRST_ID,
};

fn sphere(nxp: usize) -> MeshState {
    let mesh = TriangularMesh::from_icosahedron(nxp, 0, 1.0, 0.25).expect("base mesh");
    MeshState::from_triangular_mesh(&mesh).expect("neutral state")
}

fn delaunay_violations(state: &MeshState) -> usize {
    let mut violations = 0;
    for triangle in MESH_STATE_FIRST_ID..state.triangles().len() {
        for corner in 0..3 {
            if state.edge_is_illegal(triangle, corner).unwrap_or(false) {
                violations += 1;
            }
        }
    }
    violations
}

/// A flip turns the diagonal and leaves everything else alone.
#[test]
fn a_flip_replaces_one_edge_with_the_other_diagonal() {
    let mut state = sphere(6);
    let before = state.clone();
    let triangle = 10;
    let corner = 0;
    let neighbour = state.neighbours()[triangle][corner];
    let quad: BTreeSet<usize> = state.triangles()[triangle]
        .iter()
        .chain(state.triangles()[neighbour].iter())
        .copied()
        .collect();

    state.flip_edge(triangle, corner).expect("flip");

    let after: BTreeSet<usize> = state.triangles()[triangle]
        .iter()
        .chain(state.triangles()[neighbour].iter())
        .copied()
        .collect();
    assert_eq!(after, quad, "the same four corners, differently paired");
    assert_eq!(state.vertices(), before.vertices(), "no site moved");
    assert_eq!(state.triangle_count(), before.triangle_count());
    assert_eq!(state.open_edge_count(), 0);
    state.validate().expect("still a triangulation");
}

/// Flipping the same edge twice restores the mesh -- though not which slot
/// holds which triangle.
///
/// The pair swaps: after two flips the slot that held one triangle holds the
/// other. The mesh is the same mesh, and that is what "restores" can mean
/// here; asserting the rows come back identical would be asserting something
/// untrue, and this is the kind of test that quietly gets weakened to whatever
/// passes unless the difference is written down.
///
/// Still the strongest cheap check on the adjacency repair: a flip that
/// rebuilt neighbours wrongly would leave two valid triangles whose
/// neighbourhood no longer closes, and the edge set would not come back.
#[test]
fn flipping_twice_restores_the_mesh() {
    let edges = |state: &MeshState| {
        let mut set = BTreeSet::new();
        for triangle in MESH_STATE_FIRST_ID..state.triangles().len() {
            let corners = state.triangles()[triangle];
            for corner in 0..3 {
                let (a, b) = (corners[(corner + 1) % 3], corners[(corner + 2) % 3]);
                set.insert((a.min(b), a.max(b)));
            }
        }
        set
    };

    let mut state = sphere(6);
    let before = state.clone();
    let before_edges = edges(&before);
    state.flip_edge(10, 0).expect("flip");
    assert_ne!(edges(&state), before_edges, "one diagonal was replaced");

    let corner = (0..3)
        .find(|&corner| state.neighbours()[10][corner] == before.neighbours()[10][0])
        .expect("the two triangles are still neighbours");
    state.flip_edge(10, corner).expect("flip back");

    assert_eq!(edges(&state), before_edges, "and put back");
    assert_eq!(state.vertices(), before.vertices());
    assert_eq!(state.triangle_count(), before.triangle_count());
    assert_eq!(state.open_edge_count(), 0);
    state.validate().expect("still a triangulation");
}

/// An edge with nothing across it cannot be turned.
#[test]
fn an_edge_on_a_boundary_is_refused() {
    let mut state = MeshState::from_parts(
        vec![
            CartesianPoint::new(0.0, 0.0, 0.0),
            CartesianPoint::new(0.0, 0.0, 0.0),
            lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 0.0)),
            lonlat_degrees_to_unit_xyz(LonLatDegrees::new(10.0, 0.0)),
            lonlat_degrees_to_unit_xyz(LonLatDegrees::new(5.0, 10.0)),
        ],
        vec![[1, 1, 1], [1, 1, 1], [2, 3, 4]],
    )
    .expect("one triangle");
    let error = state
        .flip_edge(2, 0)
        .expect_err("a lone triangle has no quadrilateral");
    assert!(
        matches!(error, FlipError::EdgeIsOnTheBoundary { .. }),
        "{error}"
    );
}

#[test]
fn retired_triangles_are_not_flipped_or_legalized() {
    let mut state = sphere(2);
    state.retire_triangle_in_region_for_test(2, &BTreeSet::from([2]));

    assert!(!state.edge_is_illegal(2, 0).expect("dead face is ignored"));
    assert!(matches!(
        state.flip_edge(2, 0),
        Err(FlipError::EdgeIsOnTheBoundary {
            triangle: 2,
            corner: 0
        })
    ));
    assert_eq!(state.legalize_around(&BTreeSet::from([2])).unwrap(), 0);
}

/// An icosahedral mesh is Delaunay, so legalizing it changes nothing.
#[test]
fn legalizing_an_already_delaunay_mesh_flips_nothing() {
    let mut state = sphere(6);
    let before = state.clone();
    let all: BTreeSet<usize> = (MESH_STATE_FIRST_ID..state.triangles().len()).collect();
    assert_eq!(state.legalize_around(&all).expect("legalize"), 0);
    assert_eq!(state, before);
}

/// Moving a site breaks the criterion, and legalizing restores it.
///
/// The reason this module exists: insertion reaches Delaunay by construction
/// and needs no flips, but moving a site leaves the triangles around it in
/// place and they may no longer be Delaunay.
#[test]
fn legalizing_after_a_move_restores_the_criterion() {
    let mut state = sphere(4);
    assert_eq!(delaunay_violations(&state), 0);

    // Drag a site a good way toward one of its neighbours, on the sphere.
    let site = 12;
    let fan = state.triangle_fan(site).expect("fan");
    let neighbour = state.triangles()[fan[0]]
        .iter()
        .copied()
        .find(|&corner| corner != site)
        .expect("a neighbour");
    let from = state.vertices()[site];
    let to = state.vertices()[neighbour];
    let radius = (from.x * from.x + from.y * from.y + from.z * from.z).sqrt();
    // Most of the way to the neighbour. A gentle nudge leaves the criterion
    // intact on a near-uniform mesh, which would make this test pass while
    // testing nothing.
    let blended = CartesianPoint::new(
        from.x * 0.12 + to.x * 0.88,
        from.y * 0.12 + to.y * 0.88,
        from.z * 0.12 + to.z * 0.88,
    );
    let length = (blended.x * blended.x + blended.y * blended.y + blended.z * blended.z).sqrt();
    state.move_vertex(
        site,
        CartesianPoint::new(
            blended.x / length * radius,
            blended.y / length * radius,
            blended.z / length * radius,
        ),
    );
    assert!(
        delaunay_violations(&state) > 0,
        "the move has to break the criterion or this test proves nothing"
    );

    let seed: BTreeSet<usize> = state.triangle_fan(site).expect("fan").into_iter().collect();
    let flips = state.legalize_around(&seed).expect("legalize");
    assert!(flips > 0);
    assert_eq!(delaunay_violations(&state), 0, "after {flips} flips");
    assert_eq!(state.open_edge_count(), 0);
    state.validate().expect("still a triangulation");

    // Euler, which is what says the topology survived and not only the counts.
    let faces = state.triangle_count();
    assert_eq!(
        state.vertex_count() as isize - (faces * 3 / 2) as isize + faces as isize,
        2
    );
}

/// Legalization is deterministic.
#[test]
fn legalizing_is_deterministic() {
    let build = || {
        let mut state = sphere(4);
        let site = 12;
        let from = state.vertices()[site];
        let radius = (from.x * from.x + from.y * from.y + from.z * from.z).sqrt();
        let moved = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(31.0, 12.0));
        state.move_vertex(
            site,
            CartesianPoint::new(moved.x * radius, moved.y * radius, moved.z * radius),
        );
        let seed: BTreeSet<usize> = state.triangle_fan(site).expect("fan").into_iter().collect();
        let flips = state.legalize_around(&seed).expect("legalize");
        (state, flips)
    };
    assert_eq!(build(), build());
}

/// Repeated flips keep the mesh a triangulation.
///
/// One flip is easy to get right and a sequence is not: each repair sees the
/// adjacency the last one left, and an error that cancels out over two
/// triangles does not over twenty.
#[test]
fn a_sequence_of_flips_keeps_the_adjacency_symmetric() {
    let mut state = sphere(6);
    let mut flipped = 0;
    for triangle in MESH_STATE_FIRST_ID..state.triangles().len() {
        for corner in 0..3 {
            if state.flip_edge(triangle, corner).is_ok() {
                flipped += 1;
                if let Err(errors) = state.validate() {
                    panic!("after {flipped} flips, at ({triangle}, {corner}): {errors:?}");
                }
                assert_eq!(
                    state.open_edge_count(),
                    0,
                    "after {flipped} flips, at ({triangle}, {corner})"
                );
            }
        }
        if flipped >= 40 {
            break;
        }
    }
    assert!(flipped > 10, "only {flipped} flips were legal");
}
