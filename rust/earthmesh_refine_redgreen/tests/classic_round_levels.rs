use earthmesh_mesh::{lonlat_degrees_to_unit_xyz, spherical_triangle_area_unit, TriangularMesh};
use earthmesh_refine_redgreen::{
    redgreen_mesh_from_triangular, refine_redgreen_round_inside, RedGreenMesh, RedGreenSettings,
};

fn face_areas(mesh: &RedGreenMesh) -> Vec<f64> {
    mesh.cells_on_triangle
        .iter()
        .map(|corners| {
            spherical_triangle_area_unit(
                corners.map(|v| lonlat_degrees_to_unit_xyz(mesh.cell_points[v])),
            )
        })
        .collect()
}

/// The classic (transition-row) route used to drop the per-face depth, so its
/// hex output carried no refinement levels. Each face's depth must now follow
/// its lineage -- and read as a scale: a depth-L face is about a 4^L-th of a
/// base face, give or take the transition halvings and the Lawson flips.
#[test]
fn classic_rounds_carry_each_faces_depth_as_its_scale() {
    let base = TriangularMesh::from_icosahedron(12, 0, 1.0, 0.25).unwrap();
    let mut mesh = redgreen_mesh_from_triangular(&base, &base.m_neighbors).unwrap();
    let base_areas = face_areas(&mesh);
    let real = mesh.num_vertex + 1;
    let (smallest, largest) = base_areas[real..]
        .iter()
        .fold((f64::INFINITY, 0.0_f64), |(lo, hi), &a| {
            (lo.min(a), hi.max(a))
        });
    let settings = RedGreenSettings::default();
    assert!(
        !settings.protect_triangle_quality,
        "this is the classic route"
    );

    let mut previous: Option<Vec<i32>> = None;
    for round in 1..=3usize {
        let marking = mesh
            .triangle_points
            .iter()
            .enumerate()
            .map(|(i, p)| {
                i32::from(
                    i > mesh.num_vertex && p.lon_degrees.abs() < 40.0 && p.lat_degrees.abs() < 30.0,
                )
            })
            .collect::<Vec<_>>();
        let out =
            refine_redgreen_round_inside(&mesh, &marking, &settings, previous.as_deref()).unwrap();
        let levels = &out.mesh.refinement_levels;
        assert_eq!(
            levels.len(),
            out.mesh.cells_on_triangle.len(),
            "round {round}: a depth for every face row"
        );
        let deepest = levels.iter().copied().max().unwrap();
        assert_eq!(deepest, round, "round {round}");
        for (face, &mark) in out.interior_marks.iter().enumerate() {
            if mark == 1 {
                assert_eq!(levels[face], round, "round {round}: red child {face}");
            }
        }
        let areas = face_areas(&out.mesh);
        for face in real..levels.len() {
            let scale = 4f64.powi(levels[face] as i32);
            let (area, lo, hi) = (areas[face], smallest / scale / 6.0, largest / scale * 2.0);
            assert!(
                (lo..=hi).contains(&area),
                "round {round}: face {face} at depth {} has area {area:e}, outside [{lo:e}, {hi:e}]",
                levels[face]
            );
        }
        previous = Some(out.interior_marks.clone());
        mesh = out.mesh;
    }
}
