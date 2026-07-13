#[test]
fn dists_on_edge_writer_preserves_canonical_schema() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_dists_on_edge_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let output = root.join("distsOnEdge_NXP0009_02_global.nc4");

    let mesh = earthmesh_cli::mesh_metric_writers::DistsOnEdgeMesh {
        edge_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 10.0,
                lat: -5.0,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 20.0,
                lat: 15.0,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 30.0,
                lat: 25.0,
            },
        ],
        dists_on_edge: vec![100.0, 200.0, 300.0],
    };

    let report = earthmesh_cli::mesh_metric_writers::write_dists_on_edge_netcdf(&output, &mesh)
        .expect("write distsOnEdge file");

    assert_eq!(report.output, output);
    assert_eq!(report.num_edge, 3);

    let file = netcdf::open(&report.output).expect("open distsOnEdge file");
    assert_eq!(file.dimension("num_edge").expect("num_edge").len(), 3);
    assert_eq!(read_f64(&file, "lonv"), vec![10.0, 20.0, 30.0]);
    assert_eq!(read_f64(&file, "latv"), vec![-5.0, 15.0, 25.0]);
    assert_eq!(read_f64(&file, "distsOnEdge"), vec![100.0, 200.0, 300.0]);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn dists_on_edge_writer_rejects_length_mismatch() {
    let output = std::env::temp_dir().join("earthmesh_cli_bad_dists_on_edge.nc4");
    let mesh = earthmesh_cli::mesh_metric_writers::DistsOnEdgeMesh {
        edge_points: vec![earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 }],
        dists_on_edge: vec![1.0, 2.0],
    };

    let err = earthmesh_cli::mesh_metric_writers::write_dists_on_edge_netcdf(&output, &mesh)
        .expect_err("mismatched distsOnEdge rejected");
    assert!(err.to_string().contains("dists_on_edge length"));
}

fn read_f64(file: &netcdf::File, name: &str) -> Vec<f64> {
    file.variable(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
        .get_values::<f64, _>(..)
        .unwrap_or_else(|err| panic!("read {name}: {err}"))
}
