use earthmesh_mesh::{normalize_cartesian_to_radius, CartesianPoint};
use earthmesh_refine_certified::requirement::{
    graded_envelope, merge_sources, one_ring_adjacency, RequirementSource,
};
use earthmesh_refine_certified::{
    mother_grid::analytic_counts, safe_mother_only, Certificate, CertifiedConfig,
    CertifiedMeshOutcome, MotherGrid, VertexAddress,
};
use std::collections::{BTreeSet, VecDeque};

#[test]
fn mother_grid_counts_and_topology_for_supported_levels() {
    for n in [1, 2, 3, 4, 6, 8, 12] {
        let grid = MotherGrid::generate(n).unwrap();
        let report = Certificate::final_delivery()
            .verify_mother_grid(&grid)
            .unwrap();
        assert_eq!(
            analytic_counts(n).unwrap(),
            (report.vertices, report.edges, report.faces)
        );
        assert_eq!(report.euler, 2);
        assert_eq!(report.charge, 12);
        assert_eq!(report.open_edges, 0);
        assert_eq!(report.delaunay_violations, 0);
        assert_eq!(report.voronoi_invalid_cells, 0);
        assert_eq!(report.voronoi_reciprocal_errors, 0);
    }
}

#[test]
fn mother_grid_has_stable_unique_addresses() {
    let grid = MotherGrid::generate(4).unwrap();
    let addresses: Vec<_> = grid
        .addresses
        .iter()
        .skip(2)
        .map(|a| a.as_ref().unwrap())
        .collect();
    let unique: BTreeSet<_> = addresses.iter().copied().collect();
    assert_eq!(addresses.len(), unique.len());
    assert_eq!(
        addresses
            .iter()
            .filter(|a| matches!(a, VertexAddress::IcosahedronVertex(_)))
            .count(),
        12
    );
}

#[test]
fn safe_mother_only_reports_budget_or_geometry_certified_mesh() {
    match safe_mother_only(4, 10) {
        CertifiedMeshOutcome::CellBudgetInsufficient {
            required_cells,
            budget,
        } => {
            assert_eq!(required_cells, 320);
            assert_eq!(budget, 10);
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
    match safe_mother_only(2, 80) {
        CertifiedMeshOutcome::GeometryCertified(mesh) => assert_eq!(mesh.certificate().faces, 80),
        other => panic!("unexpected outcome: {other:?}"),
    }
}

#[test]
fn typed_config_api_delivers_geometry_certified_outcome() {
    let config = CertifiedConfig::mother_only(3);
    match earthmesh_refine_certified::generate_certified_mother_grid(&config) {
        CertifiedMeshOutcome::GeometryCertified(mesh) => {
            assert_eq!(
                (
                    mesh.certificate().vertices,
                    mesh.certificate().edges,
                    mesh.certificate().faces
                ),
                (92, 270, 180)
            );
            assert_eq!(mesh.primal().triangle_count(), 180);
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
}

#[test]
fn graded_envelope_is_max_order_invariant_and_matches_bruteforce() {
    let grid = MotherGrid::generate(2).unwrap();
    let adjacency = one_ring_adjacency(grid.mesh.triangles(), grid.mesh.vertices().len());
    let mut sources = vec![
        RequirementSource {
            vertex: 2,
            level: 4,
        },
        RequirementSource {
            vertex: 9,
            level: 3,
        },
        RequirementSource {
            vertex: 2,
            level: 5,
        },
    ];
    let a = graded_envelope(
        &adjacency,
        &merge_sources(grid.mesh.vertices().len(), &sources),
        2,
    );
    sources.reverse();
    let b = graded_envelope(
        &adjacency,
        &merge_sources(grid.mesh.vertices().len(), &sources),
        2,
    );
    assert_eq!(a, b);
    assert_eq!(
        a,
        brute_force_envelope(
            &adjacency,
            &merge_sources(grid.mesh.vertices().len(), &sources),
            2
        )
    );
    assert!(a
        .iter()
        .zip(merge_sources(grid.mesh.vertices().len(), &sources))
        .all(|(&g, r)| g >= r));
    for v in 2..a.len() {
        for &u in &adjacency[v] {
            assert!(a[v].abs_diff(a[u]) <= 1 || a[v].min(a[u]) >= 4);
        }
    }
}

#[test]
fn large_n_support_table_can_be_count_only() {
    for n in [16, 32] {
        let grid = MotherGrid::generate(n).unwrap();
        assert_eq!(analytic_counts(n).unwrap().0, grid.mesh.vertex_count());
        assert_eq!(analytic_counts(n).unwrap().2, grid.mesh.triangle_count());
    }
}

fn brute_force_envelope(
    adjacency: &[Vec<usize>],
    required: &[usize],
    ring_width: usize,
) -> Vec<usize> {
    let width = ring_width.max(1);
    let mut out = required.to_vec();
    for source in 0..required.len() {
        let level = required[source];
        if level == 0 {
            continue;
        }
        let mut distance = vec![usize::MAX; required.len()];
        let mut queue = VecDeque::from([source]);
        distance[source] = 0;
        while let Some(v) = queue.pop_front() {
            out[v] = out[v].max(level.saturating_sub(distance[v] / width));
            for &u in adjacency.get(v).into_iter().flatten() {
                if u < distance.len() && distance[u] == usize::MAX {
                    distance[u] = distance[v] + 1;
                    queue.push_back(u);
                }
            }
        }
    }
    out
}

#[test]
fn mother_angle_gate_uses_outward_interval_threshold_proof() {
    let grid = MotherGrid::generate(4).unwrap();
    let report = Certificate::final_delivery()
        .verify_mother_grid(&grid)
        .unwrap();
    let gate = report.angle_gate.unwrap();
    assert_eq!(gate.supported_subdivision, 4);
    assert_eq!(gate.observed_min_degrees, report.min_angle_degrees);
    assert_eq!(gate.observed_max_degrees, report.max_angle_degrees);
    assert_eq!(
        gate.proof_method,
        "runtime outward interval threshold proof"
    );

    let unsupported = MotherGrid::generate(5).unwrap();
    assert!(Certificate::final_delivery()
        .verify_mother_grid(&unsupported)
        .is_err());
}

#[test]
fn geometry_gate_fields_are_zero_and_enforced() {
    let grid = MotherGrid::generate(2).unwrap();
    let report = Certificate::final_delivery()
        .verify_mother_grid(&grid)
        .unwrap();
    assert_eq!(report.topology_errors, 0);
    assert_eq!(report.degree_outside_window, 0);
    assert!(report.require_geometry_gates().is_ok());
}

#[test]
fn hierarchy_rebuild_is_stable_budgeted_and_geometry_certified() {
    let fine = MotherGrid::generate(2).unwrap();
    let candidates =
        earthmesh_refine_certified::coarsen::complete_four_child_patch_candidates(&fine);
    assert_eq!(candidates.len(), 20);
    assert!(candidates
        .windows(2)
        .all(|w| w[0].parent_face < w[1].parent_face));

    match earthmesh_refine_certified::coarsen::rebuild_one_level_from_complete_mother_patches(
        fine.clone(),
        19,
    ) {
        earthmesh_refine_certified::coarsen::HierarchyRebuildOutcome::SearchBudgetExhausted {
            attempted_patches,
            snapshot_unchanged,
            mesh,
        } => {
            assert_eq!(attempted_patches, 19);
            assert!(snapshot_unchanged);
            assert_eq!(mesh, fine);
        }
        other => panic!("unexpected outcome: {other:?}"),
    }

    match earthmesh_refine_certified::coarsen::rebuild_one_level_from_complete_mother_patches(
        fine, 20,
    ) {
        earthmesh_refine_certified::coarsen::HierarchyRebuildOutcome::Rebuilt {
            mesh,
            removed_vertices,
            removed_faces,
            candidates,
            remap,
            remap_certificate,
        } => {
            assert_eq!(removed_vertices, 30);
            assert_eq!(removed_faces, 60);
            assert_eq!(candidates.len(), 20);
            assert_eq!(remap.rows().len(), 20);
            assert!(remap.rows().iter().all(|row| row.sources.len() == 4));
            assert_eq!(remap_certificate.bad_lineage_rows(), 0);
            assert_eq!(mesh.certificate().faces, 20);
            assert_eq!(mesh.certificate().topology_errors, 0);
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
}

#[test]
fn reverse_coarsening_is_atomic_finite_and_requirement_protected() {
    use earthmesh_refine_certified::coarsen::{reverse_coarsen_mother_grid, ReverseCoarsenOutcome};

    let grid = MotherGrid::generate(2).unwrap();
    let original = grid.mesh.clone();
    let required = vec![0; grid.mesh.vertices().len()];
    let exhausted = reverse_coarsen_mother_grid(grid, &required, 0, 0);
    let ReverseCoarsenOutcome::SearchBudgetExhausted(report) = exhausted else {
        panic!("zero search budget must preserve the certified mother");
    };
    assert_eq!(report.attempted_vertices, 0);
    assert_eq!(report.committed_vertices, 0);
    assert_eq!(report.mesh.primal(), &original);

    let protected_grid = MotherGrid::generate(2).unwrap();
    let protected = vec![1; protected_grid.mesh.vertices().len()];
    let ReverseCoarsenOutcome::Completed(report) =
        reverse_coarsen_mother_grid(protected_grid, &protected, 0, usize::MAX)
    else {
        panic!("protected finite candidate set must complete");
    };
    assert_eq!(report.attempted_vertices, 0);
    assert_eq!(report.committed_vertices, 0);
    assert!(report.protected_vertices > 0);
    report.mesh.certificate().require_geometry_gates().unwrap();

    let open_grid = MotherGrid::generate(2).unwrap();
    let original = open_grid.mesh.clone();
    let open = vec![0; open_grid.mesh.vertices().len()];
    let outcome = reverse_coarsen_mother_grid(open_grid, &open, 0, usize::MAX);
    let report = match outcome {
        ReverseCoarsenOutcome::Completed(report)
        | ReverseCoarsenOutcome::SearchBudgetExhausted(report) => report,
        other => panic!("unexpected reverse-coarsening outcome: {other:?}"),
    };
    assert_eq!(
        report.attempted_vertices,
        report.committed_vertices + report.rejected_vertices
    );
    assert!(report.final_vertices <= report.initial_vertices);
    if report.committed_vertices == 0 {
        assert_eq!(report.mesh.primal(), &original);
    }
    report.mesh.certificate().require_geometry_gates().unwrap();
}

#[test]
fn remap_identity_and_hierarchy_average_certify_row_sum_nonnegative_constant_closure() {
    let identity = earthmesh_refine_certified::remap::ConservativeRemap::identity(4);
    let id_cert = identity.certify_identity(4);
    assert_eq!(id_cert.rows(), 4);
    assert_eq!(id_cert.negative_weights(), 0);
    assert_eq!(id_cert.bad_row_sums(), 0);
    assert_eq!(id_cert.bad_lineage_rows(), 0);
    assert_eq!(id_cert.constant_closure_error(), 0.0);
    assert_eq!(id_cert.global_area_closure_error(), 0.0);

    let coarse = MotherGrid::generate(2).unwrap();
    let fine = MotherGrid::generate(4).unwrap();
    let remap = earthmesh_refine_certified::remap::ConservativeRemap::hierarchy_2_to_1_average(
        &coarse, &fine,
    )
    .unwrap();
    let cert = remap.certify_hierarchy_2_to_1_average(&coarse, &fine);
    assert_eq!(cert.rows(), coarse.mesh.triangle_count());
    assert_eq!(cert.negative_weights(), 0);
    assert_eq!(cert.bad_row_sums(), 0);
    assert_eq!(cert.bad_lineage_rows(), 0);
    assert!(cert.constant_closure_error() <= 1.0e-12);
    assert!(cert.global_area_closure_error() <= 1.0e-12);
    assert!(remap.rows().iter().all(|row| row.sources.len() == 4));
    assert!(remap
        .rows()
        .iter()
        .flat_map(|row| &row.sources)
        .any(|&(_, weight)| (weight - 0.25).abs() > 1.0e-6));
}

#[test]
fn spherical_overlap_remap_handles_fine_coarse_dateline_and_polar_cells() {
    let midpoint = |a: (f64, f64), b: (f64, f64)| {
        let xyz = |(lon, lat): (f64, f64)| {
            let lon = lon.to_radians();
            let lat = lat.to_radians();
            [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
        };
        let (a, b) = (xyz(a), xyz(b));
        let sum = [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
        let norm = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt();
        let point = [sum[0] / norm, sum[1] / norm, sum[2] / norm];
        (
            point[1].atan2(point[0]).to_degrees(),
            point[2].asin().to_degrees(),
        )
    };
    let split_triangle = |a, b, c| {
        let ab = midpoint(a, b);
        let bc = midpoint(b, c);
        let ca = midpoint(c, a);
        (
            vec![
                vec![a, ab, ca],
                vec![ab, b, bc],
                vec![ca, bc, c],
                vec![ab, bc, ca],
            ],
            vec![vec![a, b, c]],
        )
    };
    for (fine, coarse) in [
        split_triangle((-4.0, 0.0), (4.0, 0.0), (0.0, 6.0)),
        split_triangle((170.0, 0.0), (-170.0, 0.0), (180.0, 10.0)),
        split_triangle((-30.0, 80.0), (30.0, 80.0), (0.0, 89.0)),
    ] {
        let fine_to_coarse =
            earthmesh_refine_certified::remap::ConservativeRemap::spherical_overlap(&fine, &coarse)
                .unwrap();
        let certificate = fine_to_coarse.certify_spherical_overlap(fine.len(), coarse.len());
        assert_eq!(certificate.negative_weights(), 0);
        assert_eq!(certificate.bad_row_sums(), 0);
        assert_eq!(certificate.bad_lineage_rows(), 0);
        assert!(certificate.constant_closure_error() <= 1.0e-12);
        assert!(
            certificate.global_area_closure_error() <= 1.0e-12,
            "fine-to-coarse coverage error {}",
            certificate.global_area_closure_error()
        );

        let coarse_to_fine =
            earthmesh_refine_certified::remap::ConservativeRemap::spherical_overlap(&coarse, &fine)
                .unwrap();
        let certificate = coarse_to_fine.certify_spherical_overlap(coarse.len(), fine.len());
        assert_eq!(certificate.bad_row_sums(), 0);
        assert_eq!(certificate.bad_lineage_rows(), 0);
        assert!(
            certificate.global_area_closure_error() <= 1.0e-12,
            "coarse-to-fine coverage error {}",
            certificate.global_area_closure_error()
        );
    }
}

#[test]
fn spherical_overlap_remap_covers_real_voronoi_pentagons() {
    let coarse = MotherGrid::generate(1).unwrap();
    let fine = MotherGrid::generate(2).unwrap();
    let remap = earthmesh_refine_certified::remap::ConservativeRemap::between_voronoi_meshes(
        &fine.mesh,
        &coarse.mesh,
    )
    .unwrap();
    let certificate =
        remap.certify_spherical_overlap(fine.mesh.vertex_count(), coarse.mesh.vertex_count());
    assert_eq!(certificate.negative_weights(), 0);
    assert_eq!(certificate.bad_row_sums(), 0);
    assert_eq!(certificate.bad_lineage_rows(), 0);
    assert!(certificate.constant_closure_error() <= 1.0e-12);
    assert!(certificate.global_area_closure_error() <= 1.0e-12);
}

#[test]
fn counts_and_max_level_do_not_overflow_or_floor_requested_level() {
    assert_eq!(
        earthmesh_refine_certified::mother_grid::analytic_counts(0),
        None
    );
    assert_eq!(
        earthmesh_refine_certified::mother_grid::analytic_counts(usize::MAX),
        None
    );

    let mut config = CertifiedConfig::mother_only(5);
    config.max_level = 2;
    match earthmesh_refine_certified::generate_certified_mother_grid(&config) {
        CertifiedMeshOutcome::MaximumLevelReached {
            requested_level,
            max_level,
        } => {
            assert_eq!(requested_level, 3);
            assert_eq!(max_level, 2);
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
}

#[test]
fn final_certified_mesh_requires_audited_geometry_promotion() {
    let wrong = earthmesh_refine_certified::geometry_certified_mother_grid(2);
    let CertifiedMeshOutcome::GeometryCertified(wrong) = wrong else {
        panic!("expected geometry-certified mother grid")
    };
    let other = MotherGrid::generate(1).unwrap();
    let evidence =
        earthmesh_refine_certified::safe_mother_final_evidence(&[], 0, &other.mesh).unwrap();
    assert!(
        earthmesh_refine_certified::finalize_geometry_certified_mother(*wrong, evidence)
            .unwrap_err()
            .to_string()
            .contains("target rows")
    );

    let mismatch = earthmesh_refine_certified::geometry_certified_mother_grid(2);
    let CertifiedMeshOutcome::GeometryCertified(mismatch) = mismatch else {
        panic!("expected geometry-certified mother grid")
    };
    let mut different_mesh = mismatch.primal().clone();
    let site = different_mesh.active_vertex_slots().next().unwrap();
    let point = different_mesh.vertices()[site];
    different_mesh.move_vertex(
        site,
        normalize_cartesian_to_radius(CartesianPoint::new(point.x + 1.0e-6, point.y, point.z), 1.0)
            .unwrap(),
    );
    let evidence =
        earthmesh_refine_certified::safe_mother_final_evidence(&[], 0, &different_mesh).unwrap();
    assert!(
        earthmesh_refine_certified::finalize_geometry_certified_mother(*mismatch, evidence)
            .unwrap_err()
            .to_string()
            .contains("different mesh")
    );

    let outcome = earthmesh_refine_certified::geometry_certified_mother_grid(2);
    let CertifiedMeshOutcome::GeometryCertified(geometry) = outcome else {
        panic!("expected geometry-certified mother grid")
    };
    let evidence =
        earthmesh_refine_certified::safe_mother_final_evidence(&[], 0, geometry.primal()).unwrap();
    let ok = earthmesh_refine_certified::finalize_geometry_certified_mother(*geometry, evidence)
        .unwrap();
    assert_eq!(ok.certificate().physical_residuals, 0);

    let arbitrary = MotherGrid::generate(2).unwrap().mesh;
    let f64_only_geometry = Certificate::final_delivery()
        .verify_geometry(&arbitrary)
        .unwrap();
    assert!(f64_only_geometry.angle_gate.is_none());
    assert!(earthmesh_refine_certified::safe_mother_final_evidence(&[2], 1, &arbitrary,).is_err());
}

#[test]
fn unsupported_mother_maps_to_noncertifiable_outcome() {
    match earthmesh_refine_certified::generate_certified_mother_grid(&CertifiedConfig::mother_only(
        5,
    )) {
        CertifiedMeshOutcome::CriterionNotCertifiable { reason } => {
            assert!(reason.contains("support table"));
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
}

#[test]
fn supported_large_levels_include_n80_interval_certification() {
    for n in [20, 40] {
        let grid = MotherGrid::generate(n).unwrap();
        let report = Certificate::final_delivery()
            .verify_mother_grid(&grid)
            .unwrap();
        assert_eq!(report.angle_gate.unwrap().supported_subdivision, n);
        assert_eq!(
            analytic_counts(n).unwrap(),
            (report.vertices, report.edges, report.faces)
        );
    }

    let strict80 = MotherGrid::generate(80).unwrap();
    let report80 = Certificate::final_delivery()
        .verify_mother_grid(&strict80)
        .unwrap();
    assert_eq!(report80.angle_gate.unwrap().supported_subdivision, 80);
    assert_eq!(
        analytic_counts(80).unwrap(),
        (report80.vertices, report80.edges, report80.faces)
    );

    let grid160 = MotherGrid::generate(160).unwrap();
    assert_eq!(analytic_counts(160).unwrap().0, grid160.mesh.vertex_count());
    assert_eq!(
        analytic_counts(160).unwrap().2,
        grid160.mesh.triangle_count()
    );
    match earthmesh_refine_certified::generate_certified_mother_grid(&CertifiedConfig {
        mother_subdivision: 160,
        delivery: earthmesh_refine_certified::DeliveryMode::Coupled,
        max_cells: Some(20 * 160 * 160),
        max_level: 8,
        grading_ring_width: 1,
    }) {
        CertifiedMeshOutcome::GeometryCertified(mesh) => {
            assert_eq!(
                mesh.certificate()
                    .angle_gate
                    .as_ref()
                    .unwrap()
                    .supported_subdivision,
                160
            );
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
}
