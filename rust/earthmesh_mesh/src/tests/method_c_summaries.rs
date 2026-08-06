use super::super::*;

#[test]
fn method_c_matches_reduced_canonical_nxp6_single_circle_summary() {
    let mesh = TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };

    let refined = mesh
        .spawn_nest_as_atmosmesh(&[region], 1)
        .expect("Method-C nest matching reduced Canonical probe");
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
        (435, 1297, 865),
        "reduced Canonical probe summary: nmd=435 nud=1297 nwd=865"
    );
    assert_eq!(
        ngr_counts,
        BTreeMap::from([(2, 864)]),
        "reduced Canonical probe summary: all 864 active W faces have ngr=2"
    );
    assert_eq!(
        (
            mrow_values.first().copied(),
            mrow_values.last().copied(),
            mrow_values.len()
        ),
        (Some(-6), Some(12), 864),
        "reduced Canonical probe summary: mrow min=-6 max=12 count=864"
    );
}

#[test]
fn method_c_matches_reduced_canonical_nxp7_single_circle_summary() {
    let mesh = TriangularMesh::from_icosahedron(7, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };

    let refined = mesh
        .spawn_nest_as_atmosmesh(&[region], 1)
        .expect("NXP7 Method-C nest matching reduced Canonical probe");
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
        (565, 1687, 1125),
        "reduced Canonical NXP7 probe summary: nmd=565 nud=1687 nwd=1125"
    );
    assert_eq!(
        ngr_counts,
        BTreeMap::from([(1, 57), (2, 1067)]),
        "reduced Canonical NXP7 probe summary: W-face ngr counts are ngr1=57 and ngr2=1067"
    );
    assert_eq!(
        (
            mrow_values.first().copied(),
            mrow_values.last().copied(),
            mrow_values.len()
        ),
        (Some(-6), Some(13), 1067),
        "reduced Canonical NXP7 probe summary: mrow min=-6 max=13 count=1067"
    );
}

#[test]
fn method_c_matches_reduced_canonical_nxp6_corridor_summary() {
    let mesh = TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let region = RefinementRegion::Corridor {
        points: vec![
            LonLatDegrees::new(115.0, 25.0),
            LonLatDegrees::new(130.0, 25.0),
        ],
        radius_meters: vec![2_500_000.0, 2_500_000.0],
        level: 1,
    };

    let refined = mesh
        .spawn_nest_as_atmosmesh(&[region], 1)
        .expect("Method-C corridor nest matching reduced Canonical probe");
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
        (474, 1414, 943),
        "reduced Canonical corridor probe summary: nmd=474 nud=1414 nwd=943"
    );
    assert_eq!(
        ngr_counts,
        BTreeMap::from([(2, 942)]),
        "reduced Canonical corridor probe summary: all 942 active W faces have ngr=2"
    );
    assert_eq!(
        (
            mrow_values.first().copied(),
            mrow_values.last().copied(),
            mrow_values.len()
        ),
        (Some(-6), Some(12), 942),
        "reduced Canonical corridor probe summary: mrow min=-6 max=12 count=942"
    );
}

#[test]
fn method_c_matches_reduced_canonical_nxp7_corridor_summary() {
    let mesh = TriangularMesh::from_icosahedron(7, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let region = RefinementRegion::Corridor {
        points: vec![
            LonLatDegrees::new(115.0, 25.0),
            LonLatDegrees::new(130.0, 25.0),
        ],
        radius_meters: vec![2_500_000.0, 2_500_000.0],
        level: 1,
    };

    let refined = mesh
        .spawn_nest_as_atmosmesh(&[region], 1)
        .expect("NXP7 Method-C corridor nest matching reduced Canonical probe");
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
        (643, 1921, 1281),
        "reduced Canonical NXP7 corridor probe summary: nmd=643 nud=1921 nwd=1281"
    );
    assert_eq!(
        ngr_counts,
        BTreeMap::from([(1, 25), (2, 1255)]),
        "reduced Canonical NXP7 corridor probe summary: W-face ngr counts are ngr1=25 and ngr2=1255"
    );
    assert_eq!(
        (
            mrow_values.first().copied(),
            mrow_values.last().copied(),
            mrow_values.len()
        ),
        (Some(-8), Some(13), 1255),
        "reduced Canonical NXP7 corridor probe summary: mrow min=-8 max=13 count=1255"
    );
}

#[test]
fn method_c_matches_reduced_canonical_nxp6_variable_radius_corridor_summary() {
    let mesh = TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let region = RefinementRegion::Corridor {
        points: vec![
            LonLatDegrees::new(115.0, 25.0),
            LonLatDegrees::new(130.0, 25.0),
        ],
        radius_meters: vec![2_500_000.0, 1_250_000.0],
        level: 1,
    };

    let refined = mesh
        .spawn_nest_as_atmosmesh(&[region], 1)
        .expect("Method-C variable-radius corridor nest matching reduced Canonical probe");
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
        (435, 1297, 865),
        "reduced Canonical variable-radius corridor probe summary: nmd=435 nud=1297 nwd=865"
    );
    assert_eq!(
        ngr_counts,
        BTreeMap::from([(2, 864)]),
        "reduced Canonical variable-radius corridor probe summary: all 864 active W faces have ngr=2"
    );
    assert_eq!(
        (
            mrow_values.first().copied(),
            mrow_values.last().copied(),
            mrow_values.len()
        ),
        (Some(-6), Some(12), 864),
        "reduced Canonical variable-radius corridor probe summary: mrow min=-6 max=12 count=864"
    );
}

#[test]
fn method_c_matches_reduced_canonical_nxp6_three_point_corridor_summary() {
    let mesh = TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let region = RefinementRegion::Corridor {
        points: vec![
            LonLatDegrees::new(115.0, 25.0),
            LonLatDegrees::new(130.0, 25.0),
            LonLatDegrees::new(150.0, 0.0),
        ],
        radius_meters: vec![2_500_000.0, 2_500_000.0, 2_500_000.0],
        level: 1,
    };

    let refined = mesh
        .spawn_nest_as_atmosmesh(&[region], 1)
        .expect("Method-C three-point corridor nest matching reduced Canonical probe");
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
        (552, 1648, 1099),
        "reduced Canonical three-point corridor probe summary: nmd=552 nud=1648 nwd=1099"
    );
    assert_eq!(
        ngr_counts,
        BTreeMap::from([(2, 1098)]),
        "reduced Canonical three-point corridor probe summary: all 1098 active W faces have ngr=2"
    );
    assert_eq!(
        (
            mrow_values.first().copied(),
            mrow_values.last().copied(),
            mrow_values.len()
        ),
        (Some(-9), Some(12), 1098),
        "reduced Canonical three-point corridor probe summary: mrow min=-9 max=12 count=1098"
    );
}

#[test]
fn method_c_matches_reduced_canonical_nxp6_two_level_corridor_summary() {
    let mesh = TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
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
                LonLatDegrees::new(120.0, 25.0),
                LonLatDegrees::new(125.0, 25.0),
            ],
            radius_meters: vec![1_000_000.0, 1_000_000.0],
            level: 2,
        },
    ];

    let refined = mesh
        .spawn_nest_as_atmosmesh(&regions, 2)
        .expect("two-level Method-C corridor nest matching reduced Canonical probe");
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
        (783, 2341, 1561),
        "reduced Canonical two-level corridor probe summary: nmd=783 nud=2341 nwd=1561"
    );
    assert_eq!(
            ngr_counts,
            BTreeMap::from([(2, 294), (3, 1266)]),
            "reduced Canonical two-level corridor probe summary: W-face ngr counts are ngr2=294 and ngr3=1266"
        );
    assert_eq!(
        (
            mrow_values.first().copied(),
            mrow_values.last().copied(),
            mrow_values.len()
        ),
        (Some(-6), Some(11), 1560),
        "reduced Canonical two-level corridor probe summary: mrow min=-6 max=11 count=1560"
    );
}

#[test]
fn method_c_matches_reduced_canonical_nxp7_two_level_corridor_summary() {
    let mesh = TriangularMesh::from_icosahedron(7, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let regions = [
        RefinementRegion::Corridor {
            points: vec![
                LonLatDegrees::new(115.0, 25.0),
                LonLatDegrees::new(130.0, 25.0),
            ],
            radius_meters: vec![2_500_000.0, 2_500_000.0],
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
        .expect("NXP7 two-level Method-C corridor nest matching reduced Canonical probe");
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
        (715, 2137, 1425),
        "reduced Canonical NXP7 two-level corridor probe summary: nmd=715 nud=2137 nwd=1425"
    );
    assert_eq!(
            ngr_counts,
            BTreeMap::from([(1, 25), (2, 287), (3, 1112)]),
            "reduced Canonical NXP7 two-level corridor probe summary: W-face ngr counts are ngr1=25, ngr2=287, ngr3=1112"
        );
    assert_eq!(
        (
            mrow_values.first().copied(),
            mrow_values.last().copied(),
            mrow_values.len()
        ),
        (Some(-6), Some(13), 1399),
        "reduced Canonical NXP7 two-level corridor probe summary: mrow min=-6 max=13 count=1399"
    );
}
