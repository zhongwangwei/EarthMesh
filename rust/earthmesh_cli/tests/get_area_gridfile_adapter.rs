#[test]
fn get_area_adapter_builds_area_payload_from_unstructured_gridfile() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_get_area_adapter_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let gridfile = root.join("gridfile.nc4");
    let mesh = area_fixture_mesh();
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf(&gridfile, &mesh)
        .expect("write gridfile");

    let output =
        earthmesh_cli::grid_quality_pipeline::get_area_from_unstructured_gridfile(&gridfile)
            .expect("build GetArea production output");

    assert_eq!(output.unit.area_triangle.len(), mesh.m_points.len());
    assert_eq!(output.unit.area_cell.len(), mesh.w_points.len());
    assert!(output.unit.area_triangle[2] > 0.0);
    assert!(output.unit.area_cell[2] > 0.0);
    assert!(output.reconstruction_error.max_relative.is_finite());

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn get_area_adapter_rejects_invalid_vertices_on_cell_ids() {
    let mut mesh = area_fixture_mesh();
    mesh.w_to_m[2] = vec![2, 99, 4];

    let err = earthmesh_cli::grid_quality_pipeline::get_area_from_unstructured_mesh(&mesh)
        .expect_err("invalid verticesOnCell id rejected");
    assert!(err
        .to_string()
        .contains("w_to_m row 2 canonicals triangle id 99"));
}

fn area_fixture_mesh() -> earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
    earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
        m_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 1.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 1.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 1.0, lat: 1.0 },
        ],
        w_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 2.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 2.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 2.0, lat: 2.0 },
        ],
        m_to_w: vec![
            [1, 1, 1],
            [1, 1, 1],
            [1, 2, 3],
            [1, 2, 4],
            [1, 3, 4],
            [2, 3, 4],
        ],
        w_to_m: vec![
            vec![1],
            vec![2, 3, 4],
            vec![2, 3, 5],
            vec![2, 4, 5],
            vec![3, 4, 5],
        ],
        n_w_to_m: vec![1, 3, 3, 3, 3],
    }
}
