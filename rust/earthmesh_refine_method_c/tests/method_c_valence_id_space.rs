//! The valence repair reaches the point that actually overflowed.
//!
//! The error is raised while deriving the neighbours of the mesh being built,
//! so its M point names a point there; the repair ladder indexes the parent.
//! `emit_method_c_tables` translates between the two before raising. Guide 11.5
//! has the history and the counts.

use earthmesh_refine_method_c::{LonLatDegrees, MethodCMesh, RefinementRegion};

/// One circle at NXP 6 used to be refused, and is not.
///
/// This is the reproduction 11.5 was missing. It recorded the defect as needing
/// a 7,022-circle coastal band on a spring-relaxed base mesh; one circle on an
/// unrelaxed NXP 6 mesh raises the same valence error, in under two seconds.
/// Before the translation it reported M point 521 against a parent `nmd` of
/// 363 -- an id the parent does not have -- so the ladder's fixed-point rung was
/// skipped and the whole group was refused.
///
/// The assertions are about the mesh, not about the absence of an error: a run
/// that stops refusing is only good news if what it delivers is sound, and this
/// defect class produces meshes that are valid and not what was asked for.
#[test]
fn one_circle_at_nxp6_is_refined_rather_than_refused() {
    let mesh = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25).expect("base Method-C mesh");
    let regions = [RefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 900_000.0,
        level: 1,
    }];

    let refined = mesh
        .spawn_nest(&regions, 5)
        .expect("the valence repair now reaches the point that overflowed");

    assert!(
        refined.nwd > mesh.nwd,
        "refinement produced no new cells: {} faces before, {} after",
        mesh.nwd,
        refined.nwd
    );
    refined
        .validate_topology()
        .expect("the repaired nest is a valid Method-C mesh");
    for im in 2..=mesh.nmd {
        assert!(
            refined.m_neighbors[im].npoly <= 7,
            "parent M point {im} carries valence {} after nesting; the hexagonal dual \
             cannot represent more than seven",
            refined.m_neighbors[im].npoly
        );
    }
}

/// Every parent M point the repair is handed is one the parent actually has.
///
/// The guard this replaces was `im <= self.nmd`, which a child id satisfies by
/// coincidence whenever it happens to be small enough -- so it could not tell a
/// translated id from an untranslated one. Sweeping the configurations that used
/// to fail is the closest thing to a direct check available from outside the
/// crate: if any of them refuses again with a valence error, the translation has
/// stopped happening.
#[test]
fn configurations_that_overflowed_no_longer_refuse_for_valence() {
    for separation in [0.0f64, 1.0, 2.0, 4.0, 8.0] {
        let mesh = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25).expect("base Method-C mesh");
        let regions = [
            RefinementRegion::Circle {
                center: LonLatDegrees::new(115.0, 25.0),
                radius_meters: 900_000.0,
                level: 1,
            },
            RefinementRegion::Circle {
                center: LonLatDegrees::new(115.0 + separation, 25.0),
                radius_meters: 900_000.0,
                level: 1,
            },
        ];
        let refined = mesh
            .spawn_nest(&regions, 5)
            .unwrap_or_else(|error| panic!("separation {separation} refused: {error}"));
        refined
            .validate_topology()
            .unwrap_or_else(|error| panic!("separation {separation} invalid: {error}"));
    }
}
