use super::*;

#[test]
fn olam_method_c_olamin_style_multilevel_corridor_table_outputs_closed_mesh() {
    let mesh = OlamDelaunayMesh::from_icosahedron(33, 5000, 1.25, 0.035, 100)
        .expect("base OLAM mesh")
        .expand_by_factor(2)
        .expect("Fortran expand_global2 base OLAM mesh");
    let path = vec![
        LonLatDegrees::new(-94.0, 25.0),
        LonLatDegrees::new(-95.0, 26.0),
    ];
    let regions = [
        OlamRefinementRegion::Corridor {
            points: path.clone(),
            radius_meters: vec![3_000_000.0, 3_000_000.0],
            level: 1,
        },
        OlamRefinementRegion::Corridor {
            points: path.clone(),
            radius_meters: vec![1_800_000.0, 1_800_000.0],
            level: 2,
        },
        OlamRefinementRegion::Corridor {
            points: path,
            radius_meters: vec![1_200_000.0, 1_200_000.0],
            level: 3,
        },
    ];

    let refined = mesh
        .spawn_nest_as_atmosmesh(&regions, 3)
        .expect("OLAMIN-style atmosphere Method-C corridor table nest");
    assert_eq!(
            (refined.nmd, refined.nud, refined.nwd),
            (84_099, 252_289, 168_193),
            "OLAMIN-style atmosphere Method-C corridor table output should match the Fortran Method-C HDF5 M/U/W table sizes"
        );
    let mut ngr_counts = BTreeMap::<usize, usize>::new();
    for face in refined.w_faces.iter().skip(2) {
        *ngr_counts.entry(face.ngr).or_insert(0) += 1;
    }
    assert_eq!(
            ngr_counts,
            BTreeMap::from([(1, 76_426), (2, 11_468), (3, 15_114), (4, 65_184)]),
            "OLAMIN-style atmosphere Method-C corridor table output should match the Fortran Method-C per-grid W-face counts"
        );
    let mut mrows = refined
        .w_faces
        .iter()
        .skip(2)
        .filter_map(|face| (face.mrow != 0).then_some(face.mrow))
        .collect::<Vec<_>>();
    mrows.sort_unstable();
    assert_eq!(
            (mrows.first().copied(), mrows.last().copied(), mrows.len()),
            (Some(-13), Some(13), 50_069),
            "OLAMIN-style atmosphere Method-C corridor table output should match the Fortran Method-C atmosphere mrow envelope"
        );
    refined
        .validate_topology()
        .expect("OLAMIN-style Method-C table topology");
}

#[test]
#[ignore = "runs three 5000-iteration atmosphere spring passes; use the table-only OLAMIN corridor test for default Method-C count/topology coverage"]
fn olam_method_c_olamin_style_multilevel_corridor_outputs_closed_mesh() {
    let mesh = OlamDelaunayMesh::from_icosahedron(33, 5000, 1.25, 0.035, 100)
        .expect("base OLAM mesh")
        .expand_by_factor(2)
        .expect("Fortran expand_global2 base OLAM mesh");
    let path = vec![
        LonLatDegrees::new(-94.0, 25.0),
        LonLatDegrees::new(-95.0, 26.0),
    ];
    let regions = [
        OlamRefinementRegion::Corridor {
            points: path.clone(),
            radius_meters: vec![3_000_000.0, 3_000_000.0],
            level: 1,
        },
        OlamRefinementRegion::Corridor {
            points: path.clone(),
            radius_meters: vec![1_800_000.0, 1_800_000.0],
            level: 2,
        },
        OlamRefinementRegion::Corridor {
            points: path,
            radius_meters: vec![1_200_000.0, 1_200_000.0],
            level: 3,
        },
    ];

    let (refined, spring_passes) = mesh
        .spawn_nest_with_spring_as_atmosmesh(&regions, 3, 66, 5000)
        .expect("OLAMIN-style atmosphere Method-C corridor nest");
    assert_eq!(
        spring_passes, 3,
        "Fortran MAKEGRID runs one atmosphere spring pass after each active Method-C nest"
    );
    assert_eq!(
            (refined.nmd, refined.nud, refined.nwd),
            (84_099, 252_289, 168_193),
            "OLAMIN-style atmosphere Method-C corridor output should match the Fortran Method-C HDF5 M/U/W table sizes"
        );
    let mut ngr_counts = BTreeMap::<usize, usize>::new();
    for face in refined.w_faces.iter().skip(2) {
        *ngr_counts.entry(face.ngr).or_insert(0) += 1;
    }
    assert_eq!(
            ngr_counts,
            BTreeMap::from([(1, 76_426), (2, 11_468), (3, 15_114), (4, 65_184)]),
            "OLAMIN-style atmosphere Method-C corridor output should match the Fortran Method-C per-grid W-face counts"
        );
    let mut mrows = refined
        .w_faces
        .iter()
        .skip(2)
        .filter_map(|face| (face.mrow != 0).then_some(face.mrow))
        .collect::<Vec<_>>();
    mrows.sort_unstable();
    assert_eq!(
            (mrows.first().copied(), mrows.last().copied(), mrows.len()),
            (Some(-13), Some(13), 50_069),
            "OLAMIN-style atmosphere Method-C corridor output should match the Fortran Method-C atmosphere mrow envelope"
        );

    refined
        .validate_topology()
        .expect("OLAMIN-style Method-C topology");
    let grid_numbers = refined
        .w_faces
        .iter()
        .skip(2)
        .filter_map(|face| (face.ngr > 1).then_some(face.ngr))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        grid_numbers,
        BTreeSet::from([2, 3, 4]),
        "Fortran Method-C spawns one grid number per OLAMIN corridor refinement level"
    );
    for iu in 2..=refined.nud {
        assert!(
            refined.u_edges[iu].im.iter().all(|&im| im > 1),
            "U edge {iu} should not contain placeholder M endpoint"
        );
        assert!(
            refined.u_edges[iu].iw.iter().take(2).all(|&iw| iw > 1),
            "U edge {iu} should not contain placeholder adjacent W face"
        );
    }
    for iw in 2..=refined.nwd {
        assert!(
            refined.w_faces[iw].im.iter().all(|&im| im > 1),
            "W face {iw} should not contain placeholder M vertex"
        );
        assert!(
            refined.w_faces[iw].iu.iter().all(|&iu| iu > 1),
            "W face {iw} should not contain placeholder U edge"
        );
    }
    for im in 2..=mesh.nmd {
        assert!(
            refined.m_neighbors[im].npoly <= 7,
            "old M point {im} exceeds OLAM-supported valence after OLAMIN-style Method-C nesting"
        );
    }

    let adapted = voronoi_grid_from_olam_delaunay_mesh(
        &refined,
        active_mesh_radius(&refined).expect("active mesh radius"),
    )
    .expect("OLAMIN-style Method-C Voronoi handoff");
    assert_eq!(adapted.grid.nma, refined.nwd);
    assert_eq!(adapted.grid.nua, refined.nud);
    assert_eq!(adapted.grid.nwa, refined.nmd);
    for iw in 2..=refined.nwd {
        assert_eq!(
            adapted.tabs.m[iw].npoly as usize, refined.w_faces[iw].npoly,
            "Voronoi handoff should preserve Method-C W-face npoly for face {iw}"
        );
        assert_eq!(
            adapted.tabs.m[iw].ngr as usize, refined.w_faces[iw].ngr,
            "Voronoi handoff should preserve Method-C W-face grid number for face {iw}"
        );
    }
    for im in 2..=refined.nmd {
        assert_eq!(
            adapted.tabs.w[im].npoly as usize, refined.m_neighbors[im].npoly,
            "Voronoi handoff should preserve Method-C M-neighbor npoly for point {im}"
        );
    }
}
