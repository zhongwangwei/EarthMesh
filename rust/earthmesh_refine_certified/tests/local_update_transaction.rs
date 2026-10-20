use std::io;

use earthmesh_mesh::MeshState;
use earthmesh_refine_certified::{
    construction::{
        build_mixed_certified_construction, CertifiedConstruction, CertifiedRunOptions,
        LocalUpdate, LocalUpdateDecision,
    },
    RasterLevelField,
};

fn construct(update: Option<&LocalUpdate<'_, usize>>) -> io::Result<CertifiedConstruction<usize>> {
    // Keep all 42 sites at level one: the optional update is the only mutation.
    let raster = RasterLevelField::new(8, 4, vec![1; 32]).unwrap();
    build_mixed_certified_construction(1, 1, &CertifiedRunOptions::default(), &raster, 100, update)
}

fn collapse_edge(mesh: &mut MeshState) {
    let face = mesh.active_triangle_slots().next().unwrap();
    let [a, b, _] = mesh.triangles()[face];
    mesh.move_vertex(a, mesh.vertices()[b]);
}

fn assert_same_control(
    actual: &CertifiedConstruction<usize>,
    control: &CertifiedConstruction<usize>,
) {
    assert_eq!(actual.geometry.primal(), control.geometry.primal());
    assert_eq!(
        actual.geometry.certificate(),
        control.geometry.certificate()
    );
    assert_eq!(actual.remap, control.remap);
    assert_eq!(actual.remap_certificate, control.remap_certificate);
    assert_eq!(
        actual.final_cell_requirements,
        control.final_cell_requirements
    );
    assert_eq!(actual.delivered_levels, control.delivered_levels);
    assert_eq!(actual.pentagons, control.pentagons);
    // Includes epoch/component search states and all retry/elastic/interval counts.
    assert_eq!(actual.elastic_report, control.elastic_report);
    assert_eq!(actual.attempted_patches, control.attempted_patches);
    assert_eq!(actual.accepted_patches, control.accepted_patches);
    assert_eq!(actual.removed_vertices, control.removed_vertices);
    assert_eq!(actual.removed_faces, control.removed_faces);
    assert_eq!(actual.search_complete, control.search_complete);
    assert_eq!(
        actual.search_budget_exhausted,
        control.search_budget_exhausted
    );
}

#[test]
fn successful_callback_with_invalid_geometry_retains_certified_control() {
    let control = construct(None).unwrap();
    let update = |mesh: &mut MeshState, _: &[usize], _: &[usize; 12], _| {
        collapse_edge(mesh);
        Ok(7)
    };
    let actual = construct(Some(&update)).unwrap_or_else(|error| {
        panic!("optional local update discarded certified control: {error}")
    });
    assert_same_control(&actual, &control);
    assert!(
        matches!(actual.local_update, Some(LocalUpdateDecision::Control(reason)) if !reason.is_empty())
    );
}

#[test]
fn invalid_data_callback_cannot_mutate_retained_control() {
    let control = construct(None).unwrap();
    let update = |mesh: &mut MeshState, _: &[usize], _: &[usize; 12], _| {
        collapse_edge(mesh);
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "rejected local trial",
        ))
    };
    let actual = construct(Some(&update)).unwrap();
    assert_same_control(&actual, &control);
    assert_eq!(
        actual.local_update,
        Some(LocalUpdateDecision::Control("rejected local trial".into()))
    );
}

#[test]
fn other_callback_errors_still_abort() {
    let update = |mesh: &mut MeshState, _: &[usize], _: &[usize; 12], _| {
        collapse_edge(mesh);
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "cannot read local request",
        ))
    };
    let error = construct(Some(&update))
        .err()
        .expect("callback error must propagate");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(error.to_string(), "cannot read local request");
}

#[test]
fn accepted_noop_retains_candidate_report() {
    let control = construct(None).unwrap();
    let update = |_: &mut MeshState, _: &[usize], _: &[usize; 12], _| Ok(7);
    let actual = construct(Some(&update)).unwrap();
    assert_same_control(&actual, &control);
    assert_eq!(actual.local_update, Some(LocalUpdateDecision::Candidate(7)));
}

#[test]
fn geometry_valid_update_that_misses_raster_demand_retains_control() {
    use earthmesh_mesh::CartesianPoint;
    use earthmesh_refine_certified::{
        certify_final_cell_requirements_from_raster, Certificate, TargetLevelField,
    };

    let mut levels = vec![0; 32 * 16];
    levels[8 * 32 + 16] = 2;
    let raster = RasterLevelField::new(32, 16, levels).unwrap();
    let options = CertifiedRunOptions {
        gradation_rings_per_level: 1,
        search_budget: 1_000,
        ..CertifiedRunOptions::default()
    };
    let control =
        build_mixed_certified_construction::<usize>(1, 2, &options, &raster, 400, None).unwrap();
    assert!(control.delivered_levels.iter().any(|&level| level < 2));
    let update = |mesh: &mut MeshState, delivered: &[usize], _: &[usize; 12], angle| {
        // A rigid half-turn preserves geometry but moves the refined area away
        // from the fixed geographic raster demand.
        let sites = mesh.active_vertex_slots().collect::<Vec<_>>();
        for site in sites {
            let point = mesh.vertices()[site];
            mesh.move_vertex(
                site,
                CartesianPoint {
                    x: -point.x,
                    y: -point.y,
                    z: point.z,
                },
            );
        }
        Certificate::final_delivery_for(angle)
            .verify_geometry(mesh)
            .unwrap();
        let delivered =
            TargetLevelField::from_active_voronoi_cells(mesh, delivered.to_vec()).unwrap();
        let failure = certify_final_cell_requirements_from_raster(&raster, mesh, &delivered, 1)
            .expect_err("rotated coarse cells must miss the fixed raster demand");
        assert!(failure.physical_residuals() > 0);
        Ok(7)
    };
    let actual =
        build_mixed_certified_construction(1, 2, &options, &raster, 400, Some(&update)).unwrap();
    assert_same_control(&actual, &control);
    assert!(
        matches!(actual.local_update, Some(LocalUpdateDecision::Control(reason))
        if reason.contains("final-cell certification"))
    );
}

#[test]
fn accepted_motion_has_fresh_remap_and_complete_delivery_evidence() {
    use earthmesh_mesh::CartesianPoint;
    use earthmesh_refine_certified::{
        finalize_geometry_certified_mother, FinalCertificationEvidence,
    };

    let control = construct(None).unwrap();
    let update = |mesh: &mut MeshState, _: &[usize], anchors: &[usize; 12], _| {
        let face = mesh
            .active_triangle_slots()
            .find(|&face| {
                mesh.triangles()[face]
                    .iter()
                    .any(|site| !anchors.contains(site))
            })
            .unwrap();
        let triangle = mesh.triangles()[face];
        let site = *triangle
            .iter()
            .find(|site| !anchors.contains(site))
            .unwrap();
        let neighbor = *triangle.iter().find(|&&other| other != site).unwrap();
        let point = mesh.vertices()[site];
        let toward = mesh.vertices()[neighbor];
        let moved = CartesianPoint {
            x: point.x + 1e-4 * (toward.x - point.x),
            y: point.y + 1e-4 * (toward.y - point.y),
            z: point.z + 1e-4 * (toward.z - point.z),
        };
        let norm = |p: CartesianPoint| (p.x * p.x + p.y * p.y + p.z * p.z).sqrt();
        let scale = norm(point) / norm(moved);
        mesh.move_vertex(
            site,
            CartesianPoint {
                x: moved.x * scale,
                y: moved.y * scale,
                z: moved.z * scale,
            },
        );
        Ok(11)
    };
    let actual = construct(Some(&update)).unwrap();
    assert_eq!(
        actual.local_update,
        Some(LocalUpdateDecision::Candidate(11))
    );
    assert_ne!(actual.geometry.primal(), control.geometry.primal());
    assert_ne!(actual.remap, control.remap);
    assert!(actual
        .remap
        .joins(control.geometry.primal(), actual.geometry.primal()));
    assert_eq!(actual.elastic_report, control.elastic_report);
    assert_eq!(actual.delivered_levels, control.delivered_levels);
    let evidence = FinalCertificationEvidence::from_final_cells(
        actual.final_cell_requirements.as_ref().unwrap(),
        actual.remap_certificate.clone(),
    )
    .unwrap();
    finalize_geometry_certified_mother((*actual.geometry).clone(), evidence).unwrap();
}
