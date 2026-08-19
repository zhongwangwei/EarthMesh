use super::super::*;

/// Small Method-C refined fixture shared by the h-field spring tests: an
/// NXP-6 icosahedral base with one level-1 circular nest (same recipe as the
/// mrow tests).
fn refined_test_mesh() -> MethodCMesh {
    let mesh = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25).expect("base Method-C mesh");
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    mesh.spawn_nest_with_max_mrows(&[region], 1, MethodCMesh::MAX_MROWS_SURFACE)
        .expect("Method-C nest")
}

fn max_ngr(mesh: &TriangularMesh) -> usize {
    (2..=mesh.nmd)
        .map(|im| mesh.m_metadata[im].ngr)
        .max()
        .expect("active M points")
}

/// Compatibility-equivalent per-edge targets: `dist00 / 2^(mrlu - 1)`, optionally
/// folding in the mrow transition multiplier.
fn level_derived_targets(
    mesh: &TriangularMesh,
    edge_count: usize,
    dist00: f64,
    fold_mrow_multiplier: bool,
) -> Vec<f64> {
    (0..edge_count)
        .map(|iu| {
            if iu < 2 || iu > mesh.nud {
                return 0.0;
            }
            let edge = mesh.u_edges[iu];
            let level = edge.mrlu.max(1);
            let mut target = dist00 / 2.0_f64.powi(level as i32 - 1);
            if fold_mrow_multiplier {
                let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
                if iw1 > 1 && iw1 <= mesh.nwd && iw2 > 1 && iw2 <= mesh.nwd {
                    target *= method_c_nest_mrow_distance_multiplier(
                        mesh.w_faces[iw1].mrow,
                        mesh.w_faces[iw2].mrow,
                    );
                }
            }
            target
        })
        .collect()
}

#[test]
fn hfield_scratch_matches_compatibility_masks_floor_and_unshaped_targets_bitwise() {
    let refined = refined_test_mesh();
    let ngr = max_ngr(&refined);
    assert!(
        ngr >= 2,
        "fixture must contain a refined level, got ngr {ngr}"
    );
    let movable = crate::method_c_nest_spring::method_c_nest_movable_m_points(&refined, ngr, false)
        .expect("movable mask");
    assert!(
        movable.iter().skip(2).any(|&m| m),
        "fixture must expose movable transition points"
    );
    let topology = icosahedron_spring_topology_canonical(
        refined.nmd,
        &refined.u_edges,
        &refined.m_neighbors,
        0.035,
    )
    .expect("spring topology");
    let edge_count = topology.edge_m_points.len();

    let dist00 = 1234.5_f64;
    let compatibility = MethodCNestSpringScratch::new(&refined, &topology, &movable, dist00, true)
        .expect("compatibility scratch");
    let targets = level_derived_targets(&refined, edge_count, dist00, false);
    let hmode = MethodCNestSpringScratch::with_edge_target_lengths(
        &refined, &topology, &movable, &targets, true,
    )
    .expect("h-field scratch");

    assert_eq!(compatibility.moveu, hmode.moveu);
    assert_eq!(compatibility.compu, hmode.compu);
    assert_eq!(
        compatibility.min_area_squared.to_bits(),
        hmode.min_area_squared.to_bits(),
        "area floor must match bitwise (power-of-two scaling is exact)"
    );
    assert_eq!(compatibility.radius, hmode.radius);

    let mut unshaped = 0usize;
    let mut shaped = 0usize;
    for iu in 2..edge_count {
        if !compatibility.moveu[iu] {
            continue;
        }
        if compatibility.target_mrow_multiplier[iu] == 1.0 {
            unshaped += 1;
            assert_eq!(
                compatibility.target_level_base[iu].to_bits(),
                hmode.target_level_base[iu].to_bits(),
                "unshaped edge {iu} target base must match bitwise"
            );
        } else {
            shaped += 1;
        }
    }
    assert!(
        unshaped > 0,
        "fixture should have multiplier-1 movable edges"
    );
    assert!(
        shaped > 0,
        "fixture should include mrow-shaped transition edges"
    );
}

#[test]
fn hfield_spring_with_level_derived_targets_tracks_compatibility_and_is_deterministic() {
    let refined = refined_test_mesh();
    let ngr = max_ngr(&refined);
    let niter = 40usize;

    let compatibility = refined
        .spring_nest(6, niter, ngr, false)
        .expect("compatibility nest spring");

    let radius = earthmesh_mesh::active_mesh_radius(&refined).expect("mesh radius");
    let dist00 = canonical_global_dist00(1.0, radius, 6);
    let topology = icosahedron_spring_topology_canonical(
        refined.nmd,
        &refined.u_edges,
        &refined.m_neighbors,
        0.035,
    )
    .expect("spring topology");
    let targets = level_derived_targets(&refined, topology.edge_m_points.len(), dist00, true);

    let hmode = refined
        .spring_nest_with_edge_targets(niter, ngr, false, true, &targets)
        .expect("h-field nest spring");
    let hmode_again = refined
        .spring_nest_with_edge_targets(niter, ngr, false, true, &targets)
        .expect("h-field nest spring rerun");

    assert_eq!(hmode.nmd, compatibility.nmd);
    assert_eq!(hmode.nud, compatibility.nud);
    assert_eq!(hmode.nwd, compatibility.nwd);

    let movable = crate::method_c_nest_spring::method_c_nest_movable_m_points(&refined, ngr, false)
        .expect("movable mask");
    let mut max_diff = 0.0_f64;
    for im in 2..=refined.nmd {
        let a = compatibility.m_points[im];
        let b = hmode.m_points[im];
        let d = ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt();
        if movable[im] {
            if d > max_diff {
                max_diff = d;
            }
        } else {
            // Pinned points only pass through the trailing default-real
            // rounding, identically on both paths.
            assert_eq!(a.x.to_bits(), b.x.to_bits(), "pinned point {im} x");
            assert_eq!(a.y.to_bits(), b.y.to_bits(), "pinned point {im} y");
            assert_eq!(a.z.to_bits(), b.z.to_bits(), "pinned point {im} z");
        }
        let c = hmode_again.m_points[im];
        assert_eq!(b.x.to_bits(), c.x.to_bits(), "determinism {im} x");
        assert_eq!(b.y.to_bits(), c.y.to_bits(), "determinism {im} y");
        assert_eq!(b.z.to_bits(), c.z.to_bits(), "determinism {im} z");
    }
    // Folding the mrow multiplier into the field changes f32 rounding order by
    // ULPs on transition edges only; forty damped iterations must stay tiny.
    assert!(
        max_diff <= 1e-5 * radius,
        "h-field spring drifted from compatibility: {max_diff} (radius {radius})"
    );
}

#[test]
fn edge_targets_sample_at_midpoints_and_move_only_transition_points() {
    let refined = refined_test_mesh();
    let ngr = max_ngr(&refined);
    let radius = earthmesh_mesh::active_mesh_radius(&refined).expect("mesh radius");
    let dist00 = canonical_global_dist00(1.0, radius, 6);

    let targets = method_c_edge_target_lengths_from_field(&refined, |_lon, _lat| dist00)
        .expect("uniform field targets");
    assert_eq!(targets.len(), refined.nud + 1);
    for iu in 2..=refined.nud {
        let edge = refined.u_edges[iu];
        let [im1, im2] = edge.im;
        if im1 > 1 && im2 > 1 && im1 <= refined.nmd && im2 <= refined.nmd {
            assert!(
                (targets[iu] - dist00).abs() <= 1e-12 * dist00,
                "active edge {iu} should sample the uniform field"
            );
        }
    }

    let out = refined
        .spring_nest_with_edge_targets(10, ngr, false, true, &targets)
        .expect("uniform h-field spring");
    let movable = crate::method_c_nest_spring::method_c_nest_movable_m_points(&refined, ngr, false)
        .expect("movable mask");
    let rounded = |v: f64| (v as f32) as f64;
    let mut any_moved = false;
    for im in 2..=refined.nmd {
        let before = refined.m_points[im];
        let after = out.m_points[im];
        if movable[im] {
            if after.x.to_bits() != rounded(before.x).to_bits()
                || after.y.to_bits() != rounded(before.y).to_bits()
                || after.z.to_bits() != rounded(before.z).to_bits()
            {
                any_moved = true;
            }
        } else {
            assert_eq!(
                after.x.to_bits(),
                rounded(before.x).to_bits(),
                "pinned {im} x"
            );
            assert_eq!(
                after.y.to_bits(),
                rounded(before.y).to_bits(),
                "pinned {im} y"
            );
            assert_eq!(
                after.z.to_bits(),
                rounded(before.z).to_bits(),
                "pinned {im} z"
            );
        }
    }
    assert!(
        any_moved,
        "uniform targets should still relax transition points"
    );
}
