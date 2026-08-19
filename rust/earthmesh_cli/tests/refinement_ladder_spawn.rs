//! The derived radius ladder, run against Method-C rather than only checked
//! against the rule it was derived from.
//!
//! A ladder that satisfies "each level clears its parent" on paper is still
//! only a claim about the engine until the engine accepts it, so this nests to
//! every depth the ceiling allows and asks how deep the mesh actually got.

use earthmesh_cli::refinement_demand::ladder::nested_circle_radii_meters;
use earthmesh_mesh::{LonLatDegrees, RefinementRegion, TriangularMesh};
use earthmesh_refine_method_c::MethodCMesh;

const NXP: usize = 21;

fn base_cell_meters(nxp: usize) -> f64 {
    2.0 * std::f64::consts::PI * 6_371_229.0 / (5.0 * nxp as f64)
}

fn deepest_level(mesh: &TriangularMesh) -> usize {
    mesh.w_faces
        .iter()
        .skip(2)
        .map(|face| face.mrlw)
        .max()
        .unwrap_or(0)
}

fn nests_to_depth(nxp: usize, center: LonLatDegrees, depth: usize) {
    let radii = nested_circle_radii_meters(base_cell_meters(nxp), depth).expect("ladder");
    let regions: Vec<_> = radii
        .iter()
        .enumerate()
        .map(|(index, &radius_meters)| RefinementRegion::Circle {
            center,
            radius_meters,
            level: index + 1,
        })
        .collect();
    let mesh = MethodCMesh::from_icosahedron(nxp, 0, 1.0, 0.25).expect("base mesh");
    let refined = mesh
        .spawn_nest(&regions, depth)
        .unwrap_or_else(|error| panic!("nxp {nxp} depth {depth} radii {radii:?}: {error}"));
    assert_eq!(
        deepest_level(&refined),
        depth + 1,
        "nxp {nxp} depth {depth} radii {radii:?} did not reach the level it asked for"
    );
}

#[test]
fn the_derived_ladder_nests_to_every_depth() {
    for depth in 1..=5usize {
        nests_to_depth(NXP, LonLatDegrees::new(114.0, 22.0), depth);
    }
}

#[test]
fn the_ladder_holds_away_from_the_resolution_it_was_measured_at() {
    // The halo row count came from a search at NXP 21; a constant fitted there
    // and used everywhere is the failure this whole ladder replaced, so the
    // other resolutions are exercised rather than assumed.
    for nxp in [40usize, 81] {
        for depth in 1..=3usize {
            nests_to_depth(nxp, LonLatDegrees::new(114.0, 22.0), depth);
        }
    }
}

#[test]
fn the_ladder_holds_away_from_the_place_it_was_measured_at() {
    // Pentagons sit at fixed points of the icosahedron, so a chain that only
    // ever nests over the South China Sea says nothing about one over a
    // pentagon or a pole.
    for center in [
        LonLatDegrees::new(-30.0, 60.0),
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(0.0, 89.0),
    ] {
        for depth in 1..=3usize {
            nests_to_depth(NXP, center, depth);
        }
    }
}
