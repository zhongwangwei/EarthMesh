#[test]
fn get_edge_adapter_builds_production_edges_from_unstructured_gridfile() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_get_edge_adapter_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let gridfile = root.join("gridfile.nc4");

    let mesh = tetrahedron_like_unstructured_mesh();
    earthmesh_cli::write_unstructured_mesh_netcdf(&gridfile, &mesh).expect("write gridfile");

    let output = earthmesh_cli::get_edge_from_unstructured_gridfile(&gridfile)
        .expect("build GetEdge production output");

    assert_eq!(output.cells_on_edge[2], [2, 3]);
    assert_eq!(output.cells_on_edge[3], [1, 3]);
    assert_eq!(output.cells_on_edge[4], [1, 2]);
    assert_eq!(output.edges_on_vertex[2], [2, 3, 4]);
    assert_eq!(output.cells_on_vertex[2], [3, 3, 2]);
    approx_eq(output.edge_points[2].lon_degrees, 1.0, 1.0e-3);
    approx_eq(output.edge_points[2].lat_degrees, 1.0, 1.0e-3);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn get_edge_adapter_rejects_invalid_connectivity_ids() {
    let mut mesh = tetrahedron_like_unstructured_mesh();
    mesh.m_to_w[1] = [10, 2, 3];

    let err = earthmesh_cli::get_edge_from_unstructured_mesh(&mesh)
        .expect_err("invalid triangle-to-cell id rejected");
    assert!(err
        .to_string()
        .contains("m_to_w row 1 references cell id 10"));
}

fn tetrahedron_like_unstructured_mesh() -> earthmesh_cli::UnstructuredMesh {
    earthmesh_cli::UnstructuredMesh {
        m_points: vec![
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint { lon: 1.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 1.0 },
            earthmesh_cli::LonLatPoint { lon: 1.0, lat: 1.0 },
        ],
        w_points: vec![
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint { lon: 2.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 2.0 },
            earthmesh_cli::LonLatPoint { lon: 2.0, lat: 2.0 },
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

fn approx_eq(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {actual} ~= {expected} within {tolerance}"
    );
}
