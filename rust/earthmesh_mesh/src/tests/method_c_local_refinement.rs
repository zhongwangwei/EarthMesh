use super::*;

#[test]
fn method_c_refines_locally_and_caps_old_m_valence() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };

    let refined = mesh.spawn_nest(&[region], 1).expect("Method-C nest");
    let global_doubled = mesh.expand_global2().expect("global factor-2 expansion");

    assert!(refined.nmd > mesh.nmd);
    assert!(refined.nud > mesh.nud);
    assert!(refined.nwd > mesh.nwd);
    assert!(
        refined.nwd < global_doubled.nwd,
        "specified-region Method-C spawn should remain local, not refine the whole globe"
    );
    refined
        .validate_topology()
        .expect("valid Method-C refinement topology");
    for im in 2..=mesh.nmd {
        assert!(
            refined.m_neighbors[im].npoly <= 7,
            "old M point {im} exceeds Method-C-supported valence after Method-C closure"
        );
    }
}

#[test]
fn spawn_nest_rejects_all_active_selection_instead_of_global_fallback() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let mut selected = vec![false; mesh.nwd + 1];
    for item in selected.iter_mut().take(mesh.nwd + 1).skip(2) {
        *item = true;
    }

    let err = mesh
        .spawn_nest_pass_with_max_mrows(
            &selected,
            2,
            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
            true,
        )
        .expect_err("Method-C should not replace all-active selection with global expansion");

    assert!(
        err.to_string().contains("no nwdiv == 2 convex start point"),
        "unexpected error: {err}"
    );
}
