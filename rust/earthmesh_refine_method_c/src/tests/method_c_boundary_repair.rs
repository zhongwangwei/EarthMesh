use super::super::*;

#[test]
fn method_c_rejects_reduced_canonical_nxp6_two_level_corridor_too_close_boundary() {
    let mesh = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25).expect("base Method-C mesh");
    let regions = [
        RefinementRegion::Corridor {
            points: vec![
                LonLatDegrees::new(115.0, 25.0),
                LonLatDegrees::new(130.0, 25.0),
            ],
            radius_meters: vec![6_000_000.0, 6_000_000.0],
            level: 1,
        },
        RefinementRegion::Corridor {
            points: vec![
                LonLatDegrees::new(115.0, 25.0),
                LonLatDegrees::new(130.0, 25.0),
            ],
            radius_meters: vec![1_000_000.0, 1_000_000.0],
            level: 2,
        },
    ];

    let error = mesh
            .spawn_nest_as_atmosmesh(&regions, 2)
            .expect_err("reduced Canonical probe rejects this same-length two-level corridor as too close to the parent boundary");
    let message = error.to_string();
    assert!(
            message.contains("crosses")
                || message.contains("too close")
                || message.contains("parent boundary")
                || message.contains("next coarser grid boundary"),
            "Rust should reject the same invalid two-level corridor as the reduced Canonical probe; got {error}"
        );
    assert!(
            !message.contains("cannot be grouped into transition triples"),
            "Rust should reject this invalid two-level corridor before Method-C perimeter triple grouping; got {error}"
        );
}

#[test]
fn method_c_matches_reduced_canonical_nxp6_two_circle_summary() {
    let mesh = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25).expect("base Method-C mesh");
    let regions = [
        RefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 4_000_000.0,
            level: 1,
        },
        RefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 1_000_000.0,
            level: 2,
        },
    ];

    let refined = mesh
        .spawn_nest_as_atmosmesh(&regions, 2)
        .expect("two-level Method-C nest matching reduced Canonical probe");
    let mut ngr_counts = BTreeMap::<usize, usize>::new();
    let mut mrow_values = Vec::new();
    for iw in 2..=refined.nwd {
        let face = refined.w_faces[iw];
        *ngr_counts.entry(face.ngr).or_insert(0) += 1;
        if face.mrow != 0 {
            mrow_values.push(face.mrow);
        }
    }
    mrow_values.sort_unstable();

    assert_eq!(
        (refined.nmd, refined.nud, refined.nwd),
        (624, 1864, 1243),
        "reduced Canonical probe summary: nmd=624 nud=1864 nwd=1243"
    );
    assert_eq!(
        ngr_counts,
        BTreeMap::from([(2, 154), (3, 1088)]),
        "reduced Canonical probe summary: W-face ngr counts are ngr2=154 and ngr3=1088"
    );
    assert_eq!(
        (
            mrow_values.first().copied(),
            mrow_values.last().copied(),
            mrow_values.len()
        ),
        (Some(-6), Some(11), 1242),
        "reduced Canonical probe summary: mrow min=-6 max=11 count=1242"
    );
}

#[test]
fn method_c_matches_reduced_canonical_nxp7_two_circle_summary() {
    let mesh = MethodCMesh::from_icosahedron(7, 0, 1.0, 0.25).expect("base Method-C mesh");
    let regions = [
        RefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 3_000_000.0,
            level: 1,
        },
        RefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 1_000_000.0,
            level: 2,
        },
    ];

    let refined = mesh
        .spawn_nest_as_atmosmesh(&regions, 2)
        .expect("NXP7 two-level Method-C circle nest matching reduced Canonical probe");
    let mut ngr_counts = BTreeMap::<usize, usize>::new();
    let mut mrow_values = Vec::new();
    for iw in 2..=refined.nwd {
        let face = refined.w_faces[iw];
        *ngr_counts.entry(face.ngr).or_insert(0) += 1;
        if face.mrow != 0 {
            mrow_values.push(face.mrow);
        }
    }
    mrow_values.sort_unstable();

    assert_eq!(
        (refined.nmd, refined.nud, refined.nwd),
        (754, 2254, 1503),
        "reduced Canonical NXP7 two-circle probe summary: nmd=754 nud=2254 nwd=1503"
    );
    assert_eq!(
            ngr_counts,
            BTreeMap::from([(1, 3), (2, 335), (3, 1164)]),
            "reduced Canonical NXP7 two-circle probe summary: W-face ngr counts are ngr1=3, ngr2=335, ngr3=1164"
        );
    assert_eq!(
        (
            mrow_values.first().copied(),
            mrow_values.last().copied(),
            mrow_values.len()
        ),
        (Some(-6), Some(13), 1499),
        "reduced Canonical NXP7 two-circle probe summary: mrow min=-6 max=13 count=1499"
    );
}

#[test]
fn method_c_repairs_nxp7_circle_parent_radius_that_canonical_overruns_perimeter() {
    let mesh = MethodCMesh::from_icosahedron(7, 0, 1.0, 0.25).expect("base Method-C mesh");
    let regions = [RefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 4_000_000.0,
        level: 1,
    }];

    let refined = mesh
            .spawn_nest_as_atmosmesh(&regions, 1)
            .expect("Rust should repair the non-triplet parent perimeter instead of reproducing Canonical's perim_fill3 overrun");
    refined
        .validate_topology()
        .expect("repaired nxp7 circle topology");
    for im in 2..=refined.nmd {
        assert!(
            refined.m_neighbors[im].npoly <= 7,
            "repaired nxp7 circle M point {im} exceeds Method-C-supported valence"
        );
    }
}

#[test]
fn method_c_repairs_nxp7_corridor_parent_radius_that_canonical_overruns_perimeter() {
    let mesh = MethodCMesh::from_icosahedron(7, 0, 1.0, 0.25).expect("base Method-C mesh");
    let regions = [RefinementRegion::Corridor {
        points: vec![
            LonLatDegrees::new(115.0, 25.0),
            LonLatDegrees::new(130.0, 25.0),
        ],
        radius_meters: vec![4_000_000.0, 4_000_000.0],
        level: 1,
    }];

    let refined = mesh
            .spawn_nest_as_atmosmesh(&regions, 1)
            .expect("Rust should repair the non-triplet corridor parent perimeter instead of reproducing Canonical's perim_fill3 overrun");
    refined
        .validate_topology()
        .expect("repaired nxp7 corridor topology");
    for im in 2..=refined.nmd {
        assert!(
            refined.m_neighbors[im].npoly <= 7,
            "repaired nxp7 corridor M point {im} exceeds Method-C-supported valence"
        );
    }
}

#[test]
fn method_c_anneals_nxp7_two_circle_after_repaired_parent() {
    let mesh = MethodCMesh::from_icosahedron(7, 0, 1.0, 0.25).expect("base Method-C mesh");
    let regions = [
        RefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 4_000_000.0,
            level: 1,
        },
        RefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 1_000_000.0,
            level: 2,
        },
    ];

    let refined = mesh
        .spawn_nest_as_atmosmesh(&regions, 2)
        .expect("child mask should anneal after repaired nxp7 parent circle");
    refined
        .validate_topology()
        .expect("annealed nxp7 two-circle topology");
    for im in 2..=refined.nmd {
        assert!(
            refined.m_neighbors[im].npoly <= 7,
            "annealed nxp7 two-circle M point {im} exceeds Method-C-supported valence"
        );
    }
}

#[test]
fn method_c_anneals_nxp7_two_corridor_after_repaired_parent() {
    let mesh = MethodCMesh::from_icosahedron(7, 0, 1.0, 0.25).expect("base Method-C mesh");
    let regions = [
        RefinementRegion::Corridor {
            points: vec![
                LonLatDegrees::new(115.0, 25.0),
                LonLatDegrees::new(130.0, 25.0),
            ],
            radius_meters: vec![4_000_000.0, 4_000_000.0],
            level: 1,
        },
        RefinementRegion::Corridor {
            points: vec![
                LonLatDegrees::new(120.0, 25.0),
                LonLatDegrees::new(125.0, 25.0),
            ],
            radius_meters: vec![500_000.0, 500_000.0],
            level: 2,
        },
    ];

    let refined = mesh
        .spawn_nest_as_atmosmesh(&regions, 2)
        .expect("child mask should anneal after repaired nxp7 parent corridor");
    refined
        .validate_topology()
        .expect("annealed nxp7 two-corridor topology");
    for im in 2..=refined.nmd {
        assert!(
            refined.m_neighbors[im].npoly <= 7,
            "annealed nxp7 two-corridor M point {im} exceeds Method-C-supported valence"
        );
    }
}

#[test]
fn method_c_moves_the_parent_halo_where_the_reduced_canonical_probe_gives_up() {
    // This case used to be pinned as a refusal, matching the reduced Canonical
    // probe: the level-2 circle sits too close to the level-1 boundary and the
    // pass reports "crosses the parent boundary".
    //
    // Growing the parent is the remedy for exactly that complaint -- it is the
    // boundary the child is too close to, and moving it away is what the error
    // asks for. Measured over sixty three-level cases at NXP 21, a larger parent
    // rescued all fourteen refusals and a smaller parent rescued none, so the
    // retry sweeps upward now and reaches cases whose parent built cleanly.
    //
    // Only upward. Shrinking the parent also rescues some configurations and
    // refines less than the run asked for while doing it, which is the failure
    // that leaves a valid mesh nobody wanted.
    let mesh = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25).expect("base Method-C mesh");
    let regions = [
        RefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        },
        RefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 1_000_000.0,
            level: 2,
        },
    ];

    let refined = mesh
        .spawn_nest_as_atmosmesh(&regions, 2)
        .expect("a larger parent halo builds what the probe gives up on");

    assert!(refined.nwd > mesh.nwd);
    let deepest = (2..=refined.nwd)
        .map(|iw| refined.w_faces[iw].mrlw)
        .max()
        .expect("faces");
    assert_eq!(
        deepest, 3,
        "both levels have to land; a rescue that drops the inner one is not a rescue"
    );
    refined
        .validate_topology()
        .expect("the rescued mesh is a mesh");
}
