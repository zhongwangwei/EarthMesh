//! Point+radius refinement driven by real coastal demand, exercised directly
//! against `spawn_nest` rather than through a namelist.
//!
//! The h-field path expresses the same demand as a gradient-limited raster, and
//! its usable raster resolution turned out to sit in a window with failures at
//! both ends (see `docs/mesh_construction_technical_guide.md` section 8). Circles
//! are materializable by construction when the radius clears one rad3 footprint,
//! so this pins down whether the geometry path carries the same demand without
//! that window.

use earthmesh_mesh::{LonLatDegrees, MethodCMesh, RefinementRegion};

/// Coast blocks sampled at 1 degree from `landtype_igbp_update.nc` over
/// 108-120E / 18-26N, using the engine's own land/sea rule (`landtype != 0`).
const COAST: &[(f64, f64)] = &[
    (119.5000, 25.5000),
    (118.5000, 24.5000),
    (116.5000, 23.5000),
    (117.5000, 23.5000),
    (113.5000, 22.5000),
    (114.5000, 22.5000),
    (115.5000, 22.5000),
    (108.5000, 21.5000),
    (109.5000, 21.5000),
    (110.5000, 21.5000),
    (111.5000, 21.5000),
    (112.5000, 21.5000),
    (109.5000, 20.5000),
    (110.5000, 20.5000),
    (108.5000, 19.5000),
    (109.5000, 19.5000),
    (110.5000, 19.5000),
    (108.5000, 18.5000),
    (109.5000, 18.5000),
    (110.5000, 18.5000),
];

fn circles(radius_m: f64, level: usize) -> Vec<RefinementRegion> {
    COAST
        .iter()
        .map(|&(lon, lat)| RefinementRegion::Circle {
            center: LonLatDegrees::new(lon, lat),
            radius_meters: radius_m,
            level,
        })
        .collect()
}

/// One level of circles along the coast must refine, not abort.
#[test]
fn coastal_circle_chain_refines_at_one_level() {
    let mesh = MethodCMesh::from_icosahedron(21, 0, 1.0, 0.25, 0).expect("base Method-C mesh");
    let before = mesh.nwd;

    let refined = mesh
        .spawn_nest(&circles(150_000.0, 1), 1)
        .expect("a chain of coastal circles must be materializable");

    assert!(
        refined.nwd > before,
        "refinement must add faces: {before} -> {}",
        refined.nwd
    );
}

/// Two nested levels, the inner circles concentric with the outer ones. This is
/// the shape the h-field path could not carry at production resolution.
///
/// Unlike the raster's usable window — whose deciding variable is still unknown —
/// the constraint here is geometric and computable up front: the outer ring must
/// clear the inner one by more than one parent cell, or the child perimeter lands
/// on the parent boundary and Method-C rejects it.
#[test]
fn coastal_circle_chain_refines_at_two_levels() {
    let mesh = MethodCMesh::from_icosahedron(21, 0, 1.0, 0.25, 0).expect("base Method-C mesh");

    // The outer ring has to clear the inner one by more than a parent cell, or
    // the child perimeter lands on the parent boundary. NXP 21 base cells are
    // ~381 km, so 150 km of separation is not enough.
    let mut regions = circles(1_200_000.0, 1);
    regions.extend(circles(150_000.0, 2));

    let refined = mesh
        .spawn_nest(&regions, 2)
        .expect("nested coastal circles must be materializable");

    let deepest = refined
        .w_faces
        .iter()
        .skip(2)
        .map(|face| face.mrlw)
        .max()
        .unwrap_or(0);
    assert!(
        deepest >= 3,
        "two refinement levels must reach mrlw 3, got {deepest}"
    );
}

/// The failure mode when the rings are too close, kept so the constraint above
/// stays a measured fact rather than a comment.
#[test]
fn nested_rings_closer_than_a_parent_cell_are_rejected() {
    let mesh = MethodCMesh::from_icosahedron(21, 0, 1.0, 0.25, 0).expect("base Method-C mesh");

    // 150 km of separation against ~381 km base cells.
    let mut regions = circles(300_000.0, 1);
    regions.extend(circles(150_000.0, 2));

    let error = mesh
        .spawn_nest(&regions, 2)
        .expect_err("rings this close must be rejected, not silently mis-meshed");
    let message = error.to_string();
    // Which constraint gives out first is geometry, not policy. This used to
    // name the parent boundary, but that reading came from the walk starting on
    // ground the level-1 pass had just refined and so running a generation too
    // fine -- a misattribution, since a ring 150 km wide against 381 km cells
    // has a real problem of its own: there is not enough room across it to hold
    // a mask two faces thick, which is what the transition patch consumes.
    // Both sentences are the same rejection; the test is that it happens and
    // says something true about the geometry.
    assert!(
        message.contains("parent boundary")
            || message.contains("coarser grid boundary")
            || message.contains("one face thick")
            || message.contains("necks down"),
        "rejection must name the geometry that ran out; got {message}"
    );
}

/// Method-C documents a five-level ceiling, and point+radius reaches it.
///
/// Each ring must clear the next by more than that level's parent cell, and the
/// parent halves every level, so the radii can close up as they go in. At NXP 21
/// (base ~381 km) the required separations are 381 / 191 / 95 / 48 km, and the
/// innermost radius still has to clear the materializable floor.
///
/// Measured: 8821 faces -> 17341 for the full five levels, i.e. 381 km down to
/// 12 km for under twice the face count.
#[test]
fn nested_circles_reach_the_five_level_ceiling() {
    const RADII_KM: [f64; 5] = [2000.0, 1200.0, 700.0, 400.0, 200.0];
    let center = LonLatDegrees::new(114.0, 22.0);

    for depth in 1..=5usize {
        let mesh = MethodCMesh::from_icosahedron(21, 0, 1.0, 0.25, 0).expect("base Method-C mesh");
        let regions: Vec<_> = RADII_KM[..depth]
            .iter()
            .enumerate()
            .map(|(index, &radius_km)| RefinementRegion::Circle {
                center,
                radius_meters: radius_km * 1000.0,
                level: index + 1,
            })
            .collect();

        let refined = mesh
            .spawn_nest(&regions, depth)
            .unwrap_or_else(|error| panic!("depth {depth} must refine: {error}"));

        let deepest = refined
            .w_faces
            .iter()
            .skip(2)
            .map(|face| face.mrlw)
            .max()
            .unwrap_or(0);
        assert_eq!(
            deepest,
            depth + 1,
            "depth {depth} must reach mrlw {}",
            depth + 1
        );
    }
}
