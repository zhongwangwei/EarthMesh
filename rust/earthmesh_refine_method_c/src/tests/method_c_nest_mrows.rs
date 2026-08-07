use super::super::*;

#[test]
fn method_c_nest_mrow_distance_multiplier_matches_canonical_transition_rows() {
    let cases = [
        ((-2, -2), 7.0 / 6.0),
        ((-1, -2), 8.0 / 6.0),
        ((-1, -1), 9.0 / 6.0),
        ((1, -1), 10.0 / 6.0),
        ((1, 1), 11.0 / 12.0),
        ((0, 0), 1.0),
        ((2, -3), 1.0),
    ];

    for ((mrow1, mrow2), expected) in cases {
        let actual = method_c_nest_mrow_distance_multiplier(mrow1, mrow2);
        assert!(
            (actual - expected).abs() <= f64::EPSILON,
            "mrow pair ({mrow1}, {mrow2}) expected {expected}, got {actual}"
        );
    }
}

#[test]
fn method_c_perim_mrow_preserves_existing_adjacent_rows_like_canonical() {
    let mesh = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let mut refined = mesh
        .spawn_nest_with_max_mrows(&[region], 1, MethodCMesh::MAX_MROWS_SURFACE)
        .expect("Method-C nest");
    let preserved_iw = (2..=refined.nwd)
        .find(|&iw| refined.w_faces[iw].mrow >= 2)
        .expect("transition row that Canonical may preserve");

    for iw in 2..=refined.nwd {
        refined.w_faces[iw].mrow = 0;
    }
    refined.w_faces[preserved_iw].mrow = 1;
    refined
        .apply_method_c_perimeter_mrows(2, MethodCMesh::MAX_MROWS_SURFACE)
        .expect("Canonical perim_mrow preserves old -2/-1/1 rows when not crossing");

    assert_eq!(
        refined.w_faces[preserved_iw].mrow, 1,
        "Canonical perim_mrow preserves existing -2, -1, and 1 rows unless they cross the new border"
    );
}

#[test]
fn method_c_perim_mrow_rejects_crossing_existing_border_like_canonical() {
    let mesh = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let mut refined = mesh
        .spawn_nest_with_max_mrows(&[region], 1, MethodCMesh::MAX_MROWS_SURFACE)
        .expect("Method-C nest");
    let crossing_iw = (2..=refined.nwd)
        .find(|&iw| refined.w_faces[iw].mrow == 1)
        .expect("current border row that should reject an existing adjacent row");

    for iw in 2..=refined.nwd {
        refined.w_faces[iw].mrow = 0;
    }
    refined.w_faces[crossing_iw].mrow = 1;

    let err = refined
        .apply_method_c_perimeter_mrows(2, MethodCMesh::MAX_MROWS_SURFACE)
        .expect_err("Canonical perim_mrow rejects crossing or too-close nested boundaries");
    assert!(
        err.to_string().contains("crosses the parent boundary"),
        "unexpected perim_mrow error: {err}"
    );
}

#[test]
fn method_c_perim_mrow_overwrites_old_outer_rows_below_minus_two_like_canonical() {
    let mesh = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let mut refined = mesh
        .spawn_nest_with_max_mrows(&[region], 1, MethodCMesh::MAX_MROWS_SURFACE)
        .expect("Method-C nest");
    let overwritten_iw = (2..=refined.nwd)
        .find(|&iw| refined.w_faces[iw].mrow >= 2)
        .expect("outer transition row that Canonical may overwrite");
    let expected_row = refined.w_faces[overwritten_iw].mrow;

    for iw in 2..=refined.nwd {
        refined.w_faces[iw].mrow = 0;
    }
    refined.w_faces[overwritten_iw].mrow = -3;
    refined
        .apply_method_c_perimeter_mrows(2, MethodCMesh::MAX_MROWS_SURFACE)
        .expect("Canonical perim_mrow overwrites old rows below -2");

    assert_eq!(
        refined.w_faces[overwritten_iw].mrow, expected_row,
        "Canonical perim_mrow overwrites existing mrow values below -2 with the new transition row"
    );
}

#[test]
fn method_c_perim_mrow_uses_canonical_half_step_row_growth() {
    let mesh = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let refined = mesh
        .spawn_nest_with_max_mrows(&[region], 1, 3)
        .expect("Method-C nest with explicit mrow width");
    let max_abs_mrow = refined
        .w_faces
        .iter()
        .skip(2)
        .map(|face| face.mrow.unsigned_abs())
        .max()
        .expect("mrow values");

    assert_eq!(
            max_abs_mrow, 3,
            "Canonical perim_mrow propagates through 2*max_mrows passes but only increments row magnitude on alternating passes"
        );
}

#[test]
fn method_c_nest_movable_points_match_canonical_transition_rule() {
    let mesh = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let refined = mesh.spawn_nest(&[region], 1).expect("local circle nest");
    let actual = method_c_nest_movable_m_points(&refined, 2, false).expect("movable M point mask");
    let mut expected = vec![false; refined.nmd + 1];

    for im in 2..=refined.nmd {
        if refined.m_metadata[im].ngr != 2 {
            continue;
        }
        let neighbors = refined.m_neighbors[im];
        expected[im] = neighbors
            .iw
            .iter()
            .take(neighbors.npoly)
            .any(|&iw| refined.w_faces[iw].mrow != 0);
    }

    let mismatched = (2..=refined.nmd)
        .filter(|&im| actual[im] != expected[im])
        .collect::<Vec<_>>();
    assert!(
            mismatched.is_empty(),
            "Canonical spring_dynamics_nest only moves M points on ngr that touch mrow != 0; mismatched M ids: {mismatched:?}"
        );
}

#[test]
fn method_c_nest_movable_points_use_mrow_not_boundary_row_cache() {
    let mesh = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let mut refined = mesh.spawn_nest(&[region], 1).expect("local circle nest");
    refined.boundary_rows.clear();

    let actual = method_c_nest_movable_m_points(&refined, 2, false).expect("movable M point mask");
    let mut expected = vec![false; refined.nmd + 1];

    for im in 2..=refined.nmd {
        if refined.m_metadata[im].ngr != 2 {
            continue;
        }
        let neighbors = refined.m_neighbors[im];
        expected[im] = neighbors
            .iw
            .iter()
            .take(neighbors.npoly)
            .any(|&iw| refined.w_faces[iw].mrow != 0);
    }

    let missed = (2..=refined.nmd)
        .filter(|&im| expected[im] && !actual[im])
        .collect::<Vec<_>>();
    assert!(
            missed.is_empty(),
            "Canonical spring_dynamics_nest reads itab_wd%mrow directly, not a cached boundary-row list; missed M ids: {missed:?}"
        );
}

#[test]
fn method_c_nest_move_interior_keeps_parent_grid_m_points_stationary() {
    let mesh = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let refined = mesh.spawn_nest(&[region], 1).expect("local circle nest");
    let actual = method_c_nest_movable_m_points(&refined, 2, true).expect("movable M point mask");
    let mismatched = (2..=refined.nmd)
        .filter(|&im| actual[im] != (refined.m_metadata[im].ngr == 2))
        .collect::<Vec<_>>();

    assert!(
            mismatched.is_empty(),
            "Canonical moveint=1 moves all and only M points whose itab_md%ngr equals the current nest ngr; mismatched M ids: {mismatched:?}"
        );
}

#[test]
fn method_c_nest_transition_movement_filters_parent_grid_m_points() {
    let mut mesh = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let boundary_iw = 2;
    mesh.boundary_rows = vec![boundary_iw];
    mesh.w_faces[boundary_iw].mrow = 1;
    mesh.w_faces[boundary_iw].ngr = 2;

    let actual = method_c_nest_movable_m_points(&mesh, 2, false).expect("movable M point mask");
    let moved_parent_points = mesh.w_faces[boundary_iw]
        .im
        .iter()
        .copied()
        .filter(|&im| mesh.m_metadata[im].ngr != 2 && actual[im])
        .collect::<Vec<_>>();

    assert!(
            moved_parent_points.is_empty(),
            "Canonical spring_dynamics_nest skips transition-row M points whose itab_md%ngr is not the current ngr; moved parent-grid M ids: {moved_parent_points:?}"
        );
}

#[test]
fn method_c_nest_spring_ignores_mrlu_outside_moving_stencil() {
    let mesh = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let mut refined = mesh.spawn_nest(&[region], 1).expect("local circle nest");
    let movable = method_c_nest_movable_m_points(&refined, 2, false).expect("movable M point mask");
    let transition_face_id = *refined
        .boundary_rows()
        .first()
        .expect("transition row face should be recorded");
    let transition_point_id = refined.w_faces[transition_face_id]
        .im
        .iter()
        .copied()
        .find(|&im| movable[im])
        .expect("transition face has a movable M point");
    let transition_neighbors = refined.m_neighbors[transition_point_id];
    let neighbor_edge_id = transition_neighbors.iu[0];
    let neighbor_point_id = if refined.u_edges[neighbor_edge_id].im[0] == transition_point_id {
        refined.u_edges[neighbor_edge_id].im[1]
    } else {
        refined.u_edges[neighbor_edge_id].im[0]
    };
    let squeezed = CartesianPoint::new(
        refined.m_points[neighbor_point_id].x * 0.999
            + refined.m_points[transition_point_id].x * 0.001,
        refined.m_points[neighbor_point_id].y * 0.999
            + refined.m_points[transition_point_id].y * 0.001,
        refined.m_points[neighbor_point_id].z * 0.999
            + refined.m_points[transition_point_id].z * 0.001,
    );
    let scale = earthmesh_core::EARTH_RADIUS_METERS / magnitude(squeezed);
    refined.m_points[transition_point_id] =
        CartesianPoint::new(squeezed.x * scale, squeezed.y * scale, squeezed.z * scale);
    let mut with_remote_level = refined.clone();
    let remote_edge_id = (2..=with_remote_level.nud)
        .find(|&iu| {
            let [im1, im2] = with_remote_level.u_edges[iu].im;
            !movable[im1] && !movable[im2]
        })
        .expect("remote non-moving U edge");
    with_remote_level.u_edges[remote_edge_id].mrlu = 16;

    let baseline = refined
        .spring_nest(6, 1, 2, false)
        .expect("baseline nest spring");
    let remote_changed = with_remote_level
        .spring_nest(6, 1, 2, false)
        .expect("remote-level nest spring");

    for im in 2..=baseline.nmd {
        let diff = magnitude(vector_between(
            baseline.m_points[im],
            remote_changed.m_points[im],
        ));
        assert!(
                diff <= 1.0e-7,
                "Canonical spring_dynamics_nest computes mrlmax only from nmoveu edges; remote edge {remote_edge_id} changed M point {im} by {diff}"
            );
    }
}

#[test]
fn method_c_nest_spring_ignores_degenerate_edge_outside_compu_stencil() {
    let mesh = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let mut refined = mesh.spawn_nest(&[region], 1).expect("local circle nest");
    let movable = method_c_nest_movable_m_points(&refined, 2, false).expect("movable M point mask");
    let topology = icosahedron_spring_topology_canonical(
        refined.nmd,
        &refined.u_edges,
        &refined.m_neighbors,
        0.035,
    )
    .expect("spring topology");
    let mut compu = vec![false; refined.nud + 1];
    for edge_id in 2..=refined.nud {
        let [im1, im2] = refined.u_edges[edge_id].im;
        let [iu1, _, iu3, _] = topology.edge_neighbor_u[edge_id];
        let [iu1_im1, iu1_im2] = refined.u_edges[iu1].im;
        let im3 = if iu1_im1 == im1 { iu1_im2 } else { iu1_im1 };
        let [iu3_im1, iu3_im2] = refined.u_edges[iu3].im;
        let im4 = if iu3_im1 == im1 { iu3_im2 } else { iu3_im1 };
        compu[edge_id] = movable[im1] || movable[im2] || movable[im3] || movable[im4];
    }
    let remote_edge_id = (2..=refined.nud)
        .find(|&edge_id| !compu[edge_id])
        .expect("non-computational remote U edge");
    let [remote_im1, remote_im2] = refined.u_edges[remote_edge_id].im;
    refined.m_points[remote_im2] = refined.m_points[remote_im1];

    refined
        .spring_nest(6, 1, 2, false)
        .expect("Canonical spring_dynamics_nest should ignore non-compu remote edges");
}
