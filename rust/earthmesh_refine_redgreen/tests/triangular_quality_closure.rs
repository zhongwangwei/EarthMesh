use earthmesh_mesh::{polygon_length_angle_metrics, TriangularMesh};
use earthmesh_refine_redgreen::{
    redgreen_mesh_from_triangular, refine_redgreen_round_inside, RedGreenSettings,
};

#[test]
fn triangular_closure_does_not_compound_green_slivers() {
    let base = TriangularMesh::from_icosahedron(12, 0, 1.0, 0.25).unwrap();
    let mut mesh = redgreen_mesh_from_triangular(&base, &base.m_neighbors).unwrap();
    let base_faces = mesh.triangle_count() - mesh.num_vertex;
    let settings = RedGreenSettings {
        protect_triangle_quality: true,
        min_triangle_angle_deg: 25.0,
        ..RedGreenSettings::default()
    };
    let source_floor = 0.5
        * mesh
            .cells_on_triangle
            .iter()
            .skip(mesh.num_vertex + 1)
            .flat_map(|c| {
                polygon_length_angle_metrics(&c.map(|v| mesh.cell_points[v]))
                    .unwrap()
                    .angles_degrees
            })
            .fold(180.0_f64, f64::min);
    let mut previous = None;
    let mut maximum_scale_ratio = 1.0_f64;
    for level in 1..=3 {
        let marking = mesh
            .triangle_points
            .iter()
            .enumerate()
            .map(|(i, p)| {
                i32::from(
                    i > mesh.num_vertex && p.lon_degrees.abs() < 35.0 && p.lat_degrees.abs() < 30.0,
                )
            })
            .collect::<Vec<_>>();
        let out =
            refine_redgreen_round_inside(&mesh, &marking, &settings, previous.as_deref()).unwrap();
        let mut range = (180.0_f64, 0.0_f64);
        for corners in out
            .mesh
            .cells_on_triangle
            .iter()
            .skip(out.mesh.num_vertex + 1)
        {
            let metrics =
                polygon_length_angle_metrics(&corners.map(|v| out.mesh.cell_points[v])).unwrap();
            for angle in metrics.angles_degrees {
                range.0 = range.0.min(angle);
                range.1 = range.1.max(angle);
            }
        }
        assert!(
            range.0 >= (source_floor - 0.05).max(settings.min_triangle_angle_deg)
                && range.1 <= 105.0,
            "level {level}: {range:?}"
        );
        let neighbors = earthmesh_mesh::triangle_neighbors_from_cell_membership_one_based(
            &out.mesh.cells_on_triangle,
            &out.mesh.triangles_on_cell,
            &out.mesh.n_triangles_on_cell,
        )
        .unwrap();
        assert!(neighbors
            .iter()
            .skip(out.mesh.num_vertex + 1)
            .all(|row| !row.contains(&0)));
        assert!(out.mesh.triangle_count() < base_faces * 4usize.pow(level));
        assert!(
            out.refined_triangle_count > 0,
            "level {level} must actually refine"
        );
        assert_eq!(
            out.mesh.refinement_levels.iter().copied().max(),
            Some(level as usize)
        );
        eprintln!(
            "level {level}: {} faces, {} green pairs, angles {range:?}",
            out.mesh.triangle_count() - out.mesh.num_vertex,
            out.mesh.green_parents.len()
        );
        let (ratio, warnings) = adjacent_scale_ratio(&out.mesh);
        maximum_scale_ratio = maximum_scale_ratio.max(ratio);
        eprintln!("level {level}: adjacent scale ratio={ratio}, warnings={warnings}");
        previous = Some(out.interior_marks);
        mesh = out.mesh;
    }
    assert!(
        maximum_scale_ratio <= 2.0,
        "physical 2:1 ratio: {maximum_scale_ratio}"
    );
}

#[test]
fn isolated_and_later_green_demand_is_not_silently_dropped() {
    let base = TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25).unwrap();
    let mesh = redgreen_mesh_from_triangular(&base, &base.m_neighbors).unwrap();
    let settings = RedGreenSettings {
        protect_triangle_quality: true,
        min_triangle_angle_deg: 25.0,
        ..RedGreenSettings::default()
    };
    let mut marking = vec![0; mesh.cells_on_triangle.len()];
    marking[10] = 1;
    let first = refine_redgreen_round_inside(&mesh, &marking, &settings, None).unwrap();
    assert_eq!(first.isolated_dropped_count, 0);
    assert!(first.refined_triangle_count > 0);
    let child = first.mesh.green_parents[0].1[0];
    let mut marking = vec![0; first.mesh.cells_on_triangle.len()];
    marking[child] = 1;
    let second = refine_redgreen_round_inside(&first.mesh, &marking, &settings, None).unwrap();
    assert_eq!(second.mesh.refinement_levels.iter().max(), Some(&2));
    assert!(second.interior_marks.contains(&1));
    let repeat = refine_redgreen_round_inside(&first.mesh, &marking, &settings, None).unwrap();
    assert_eq!(
        second, repeat,
        "hash randomization must not change numbering or geometry"
    );
}

fn adjacent_scale_ratio(mesh: &earthmesh_refine_redgreen::RedGreenMesh) -> (f64, usize) {
    let mut edges = std::collections::HashMap::new();
    let mut maximum = 1.0_f64;
    let mut warnings = 0;
    for &corners in mesh.cells_on_triangle.iter().skip(mesh.num_vertex + 1) {
        let area = earthmesh_mesh::spherical_triangle_area_unit(
            corners.map(|v| earthmesh_mesh::lonlat_degrees_to_unit_xyz(mesh.cell_points[v])),
        );
        assert!(area.is_finite() && area > 0.0);
        for e in 0..3 {
            let (a, b) = (corners[e], corners[(e + 1) % 3]);
            if let Some(other) = edges.remove(&(a.min(b), a.max(b))) {
                let ratio = (area.max(other) / area.min(other)).sqrt();
                maximum = maximum.max(ratio);
                warnings += usize::from(ratio > 2.0);
            } else {
                edges.insert((a.min(b), a.max(b)), area);
            }
        }
    }
    assert!(edges.is_empty(), "closed sphere must pair all edges");
    (maximum, warnings)
}
