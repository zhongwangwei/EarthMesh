use super::*;

#[test]
fn method_c_projects_points_to_radius_before_neighbor_rebuild() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let selected = mesh
        .selected_region_faces(&region, 1, false)
        .expect("selected Method-C faces");
    let refined = mesh
        .spawn_nest_pass_with_max_mrows(&selected, 2, 7, true)
        .expect("Method-C pass");
    let radius = active_mesh_radius(&refined).expect("active mesh radius");
    let off_radius_points = (2..=refined.nmd)
        .filter(|&im| refined.m_metadata[im].mrlm_orig == 2)
        .filter(|&im| (magnitude(refined.m_points[im]) - radius).abs() > 1.0e-6)
        .collect::<Vec<_>>();

    assert!(
            off_radius_points.is_empty(),
            "Canonical spawn_nest projects all Method-C M coordinates back to Earth radius before tri_neighbors/perim_mrow/spring; off-radius M ids: {off_radius_points:?}"
        );
}

#[test]
fn method_c_refinement_level_is_not_grid_number() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let selected = mesh
        .selected_region_faces(&region, 1, false)
        .expect("selected Method-C faces");
    let child_grid_number = 4;
    let refined = mesh
        .spawn_nest_pass_with_max_mrows(&selected, child_grid_number, 7, true)
        .expect("Method-C pass with non-level grid number");

    let max_mrlm = refined
        .m_metadata
        .iter()
        .skip(2)
        .map(|metadata| metadata.mrlm)
        .max()
        .expect("M metadata");
    let max_mrlm_orig = refined
        .m_metadata
        .iter()
        .skip(2)
        .map(|metadata| metadata.mrlm_orig)
        .max()
        .expect("M original metadata");
    let max_mrlw = refined
        .w_faces
        .iter()
        .skip(2)
        .map(|face| face.mrlw)
        .max()
        .expect("W metadata");
    let max_mrlw_orig = refined
        .w_faces
        .iter()
        .skip(2)
        .map(|face| face.mrlw_orig)
        .max()
        .expect("W original metadata");

    assert!(
            max_mrlm <= 2 && max_mrlm_orig <= 2,
            "Canonical writes M refinement levels as parent mrlo + 1 independently of grid number; got max mrlm={max_mrlm}, max mrlm_orig={max_mrlm_orig}"
        );
    assert!(
            max_mrlw <= 2 && max_mrlw_orig <= 2,
            "Canonical writes W refinement levels as parent mrlo + 1 independently of grid number; got max mrlw={max_mrlw}, max mrlw_orig={max_mrlw_orig}"
        );
}

#[test]
fn method_c_keeps_canonical_linear_coordinates_before_projection() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let selected = mesh
        .selected_region_faces(&region, 1, false)
        .expect("selected Method-C faces");
    let refined = mesh
        .spawn_nest_pass_with_max_mrows(&selected, 2, 7, false)
        .expect("Method-C pass without final projection");
    let radius = active_mesh_radius(&mesh).expect("active mesh radius");
    let off_radius_points = (2..=refined.nmd)
        .filter(|&im| refined.m_metadata[im].mrlm_orig == 2)
        .filter(|&im| (magnitude(refined.m_points[im]) - radius).abs() > 1.0e-6)
        .collect::<Vec<_>>();

    assert!(
            !off_radius_points.is_empty(),
            "Canonical perim_fill3 writes ordinary linear M coordinates before the later spawn_nest radius projection"
        );
}

#[test]
fn method_c_projection_matches_canonical_radius_expansion() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let selected = mesh
        .selected_region_faces(&region, 1, false)
        .expect("selected Method-C faces");
    let linear = mesh
        .spawn_nest_pass_with_max_mrows(&selected, 2, 7, false)
        .expect("Method-C pass without final projection");
    let projected = mesh
        .spawn_nest_pass_with_max_mrows(&selected, 2, 7, true)
        .expect("Method-C pass with final projection");
    let radius = active_mesh_radius(&mesh).expect("active mesh radius");
    assert_eq!(linear.nmd, projected.nmd);

    for im in 2..=linear.nmd {
        let expected = normalize_cartesian_to_radius(linear.m_points[im], radius)
            .expect("Canonical radius expansion");
        let actual = projected.m_points[im];
        let delta = magnitude(CartesianPoint::new(
            actual.x - expected.x,
            actual.y - expected.y,
            actual.z - expected.z,
        ));
        assert!(
            delta < 1.0e-6,
            "Canonical spawn_nest projects M point {im} by radial expansion; delta={delta}"
        );
    }
}
