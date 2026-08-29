use earthmesh_refine_certified::{
    requirement::{
        certify_final_cell_requirements, certify_final_cell_requirements_from_raster,
        graded_envelope, RasterLevelField, SourceLevelField, TargetLevelField,
    },
    BalanceCertificate, MotherGrid, PhysicalCertificate,
};

#[test]
fn identity_field_certifies_matching_levels() {
    let grid = MotherGrid::generate(1).unwrap();
    let n = grid.mesh.vertex_count();
    let source = SourceLevelField::from_active_voronoi_cells(&grid.mesh, vec![2; n]).unwrap();
    let target = TargetLevelField::from_active_voronoi_cells(&grid.mesh, vec![2; n]).unwrap();

    let cert =
        certify_final_cell_requirements(&grid.mesh, &source, &grid.mesh, &target, 0).unwrap();
    assert_eq!(cert.target_cells(), n);
    assert_eq!(cert.physical_residuals(), 0);
    assert_eq!(cert.balance_residuals(), 0);
    assert_eq!(cert.witnesses().len(), 0);
    assert_eq!(
        PhysicalCertificate::from_final_cells(&cert)
            .unwrap()
            .residuals(),
        0
    );
    assert_eq!(
        BalanceCertificate::from_final_cells(&cert)
            .unwrap()
            .residuals(),
        0
    );
}

#[test]
fn fine_source_to_coarse_target_takes_max_overlapping_requirement() {
    let fine = MotherGrid::generate(2).unwrap();
    let coarse = MotherGrid::generate(1).unwrap();
    let mut required = vec![1; fine.mesh.vertex_count()];
    required[0] = 4;
    let source = SourceLevelField::from_active_voronoi_cells(&fine.mesh, required).unwrap();
    let target = TargetLevelField::from_active_voronoi_cells(
        &coarse.mesh,
        vec![4; coarse.mesh.vertex_count()],
    )
    .unwrap();

    let cert =
        certify_final_cell_requirements(&fine.mesh, &source, &coarse.mesh, &target, 3).unwrap();
    assert_eq!(cert.physical_residuals(), 0);
    assert!(cert.required_levels().contains(&4));
}

#[test]
fn mixed_requirements_report_physical_witnesses() {
    let grid = MotherGrid::generate(2).unwrap();
    let mut required = vec![1; grid.mesh.vertex_count()];
    required[3] = 5;
    let source = SourceLevelField::from_active_voronoi_cells(&grid.mesh, required).unwrap();
    let target =
        TargetLevelField::from_active_voronoi_cells(&grid.mesh, vec![1; grid.mesh.vertex_count()])
            .unwrap();

    let err =
        certify_final_cell_requirements(&grid.mesh, &source, &grid.mesh, &target, 4).unwrap_err();
    assert_eq!(err.physical_residuals(), 1);
    assert!(err
        .witnesses()
        .iter()
        .any(|w| w.required_level == 5 && w.delivered_level == 1));
}

#[test]
fn pentagon_and_dateline_voronoi_cells_do_not_break_overlap() {
    let grid = MotherGrid::generate(1).unwrap();
    let n = grid.mesh.vertex_count();
    let source = SourceLevelField::from_active_voronoi_cells(&grid.mesh, vec![3; n]).unwrap();
    let target = TargetLevelField::from_active_voronoi_cells(&grid.mesh, vec![3; n]).unwrap();

    let cert =
        certify_final_cell_requirements(&grid.mesh, &source, &grid.mesh, &target, 0).unwrap();
    assert_eq!(cert.target_cells(), 12);
    assert_eq!(cert.physical_residuals() + cert.balance_residuals(), 0);
}

#[test]
fn adjacent_level_balance_reports_edge_witnesses() {
    let grid = MotherGrid::generate(1).unwrap();
    let n = grid.mesh.vertex_count();
    let source = SourceLevelField::from_active_voronoi_cells(&grid.mesh, vec![1; n]).unwrap();
    let mut delivered = vec![1; n];
    delivered[0] = 5;
    let target = TargetLevelField::from_active_voronoi_cells(&grid.mesh, delivered).unwrap();

    let err =
        certify_final_cell_requirements(&grid.mesh, &source, &grid.mesh, &target, 1).unwrap_err();
    assert!(err.balance_residuals() > 0);
    assert!(err.witnesses().iter().any(|w| w.kind == "balance"));
}

#[test]
fn level_fields_reject_length_mismatch() {
    let grid = MotherGrid::generate(1).unwrap();
    assert!(SourceLevelField::from_active_voronoi_cells(&grid.mesh, vec![1; 11]).is_err());
    assert!(TargetLevelField::from_active_voronoi_cells(&grid.mesh, vec![1; 13]).is_err());
}

#[test]
fn certification_is_source_order_deterministic_after_max_merge() {
    let grid = MotherGrid::generate(2).unwrap();
    let n = grid.mesh.vertex_count();
    let mut a = vec![1; n];
    a[0] = 4;
    a[5] = 3;
    let mut b = vec![1; n];
    b[5] = 3;
    b[0] = 4;
    let target = TargetLevelField::from_active_voronoi_cells(&grid.mesh, vec![4; n]).unwrap();

    let ca = certify_final_cell_requirements(
        &grid.mesh,
        &SourceLevelField::from_active_voronoi_cells(&grid.mesh, a).unwrap(),
        &grid.mesh,
        &target,
        3,
    )
    .unwrap();
    let cb = certify_final_cell_requirements(
        &grid.mesh,
        &SourceLevelField::from_active_voronoi_cells(&grid.mesh, b).unwrap(),
        &grid.mesh,
        &target,
        3,
    )
    .unwrap();
    assert_eq!(ca.required_levels(), cb.required_levels());
    assert_eq!(ca.witnesses(), cb.witnesses());
}

#[test]
fn certification_rejects_field_mesh_id_mismatch() {
    let coarse = MotherGrid::generate(1).unwrap();
    let fine = MotherGrid::generate(2).unwrap();
    let source_for_coarse = SourceLevelField::from_active_voronoi_cells(
        &coarse.mesh,
        vec![1; coarse.mesh.vertex_count()],
    )
    .unwrap();
    let target =
        TargetLevelField::from_active_voronoi_cells(&fine.mesh, vec![1; fine.mesh.vertex_count()])
            .unwrap();

    let err =
        certify_final_cell_requirements(&fine.mesh, &source_for_coarse, &fine.mesh, &target, 1)
            .unwrap_err();
    assert!(matches!(
        err,
        earthmesh_refine_certified::FinalCellRequirementError::InvalidInput(_)
    ));
}

#[test]
fn raster_overlap_projects_seam_and_polar_requirements_to_final_voronoi_cells() {
    let grid = MotherGrid::generate(1).unwrap();
    let mut raster = vec![0; 8 * 4];
    raster[0] = 2;
    raster[7] = 3;
    raster[3 * 8 + 4] = 4;
    let raster = RasterLevelField::new(8, 4, raster).unwrap();
    let delivered =
        TargetLevelField::from_active_voronoi_cells(&grid.mesh, vec![4; grid.mesh.vertex_count()])
            .unwrap();

    let report =
        certify_final_cell_requirements_from_raster(&raster, &grid.mesh, &delivered, 0).unwrap();
    assert_eq!(report.physical_residuals(), 0);
    assert_eq!(report.balance_residuals(), 0);
    assert!(report.required_levels().contains(&2));
    assert!(report.required_levels().contains(&3));
    assert!(report.required_levels().contains(&4));
}

#[test]
fn raster_overlap_reports_source_cell_without_inventing_a_source_site() {
    let grid = MotherGrid::generate(1).unwrap();
    let mut raster = vec![0; 8 * 4];
    raster[9] = 2;
    let raster = RasterLevelField::new(8, 4, raster).unwrap();
    let delivered =
        TargetLevelField::from_active_voronoi_cells(&grid.mesh, vec![0; grid.mesh.vertex_count()])
            .unwrap();

    let error = certify_final_cell_requirements_from_raster(&raster, &grid.mesh, &delivered, 0)
        .unwrap_err();
    assert!(error
        .witnesses()
        .iter()
        .any(|witness| witness.source_cell == Some(9) && witness.source_site.is_none()));
}

#[test]
fn seam_hotspots_polar_noise_and_ring_requirements_grade_to_a_balanced_mixed_field() {
    let grid = MotherGrid::generate(4).unwrap();
    let (nlon, nlat) = (32, 16);
    let mut levels = vec![0; nlon * nlat];
    for (i, j, level) in [
        (0, 8, 3),
        (31, 8, 3),
        (1, 8, 2),
        (16, 15, 2),
        (8, 4, 1),
        (12, 4, 1),
        (16, 4, 1),
        (20, 4, 1),
    ] {
        levels[j * nlon + i] = levels[j * nlon + i].max(level);
    }
    let raster = RasterLevelField::new(nlon, nlat, levels).unwrap();
    let all_fine =
        TargetLevelField::from_active_voronoi_cells(&grid.mesh, vec![3; grid.mesh.vertex_count()])
            .unwrap();
    let required =
        certify_final_cell_requirements_from_raster(&raster, &grid.mesh, &all_fine, usize::MAX)
            .unwrap();
    let active_sites = grid.mesh.active_vertex_slots().collect::<Vec<_>>();
    let cells = active_sites
        .iter()
        .copied()
        .enumerate()
        .map(|(cell, site)| (site, cell))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut adjacency = vec![Vec::new(); active_sites.len()];
    for face in grid.mesh.active_triangle_slots() {
        let [a, b, c] = grid.mesh.triangles()[face];
        for (left, right) in [(a, b), (b, c), (c, a)] {
            let (left, right) = (cells[&left], cells[&right]);
            adjacency[left].push(right);
            adjacency[right].push(left);
        }
    }
    for row in &mut adjacency {
        row.sort_unstable();
        row.dedup();
    }
    let graded = graded_envelope(&adjacency, required.required_levels(), 1);
    assert!(graded.contains(&0));
    assert!(graded.contains(&3));
    let delivered = TargetLevelField::from_active_voronoi_cells(&grid.mesh, graded).unwrap();
    let certified =
        certify_final_cell_requirements_from_raster(&raster, &grid.mesh, &delivered, 1).unwrap();
    assert_eq!(certified.physical_residuals(), 0);
    assert_eq!(certified.balance_residuals(), 0);
}
