#[test]
fn springjustment_global_persistence_writes_compatibility_result_files() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_springjustment_global_persistence_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("result")).expect("create result root");

    let output = spring_output(Some(vec![12.0, 24.0, 48.0]));
    let report = earthmesh_cli::grid_quality_pipeline::write_springjustment_global_persistence(
        &root,
        9,
        3,
        &[
            earthmesh_mesh::LonLatDegrees {
                lon_degrees: 110.0,
                lat_degrees: -10.0,
            },
            earthmesh_mesh::LonLatDegrees {
                lon_degrees: 120.0,
                lat_degrees: 0.0,
            },
            earthmesh_mesh::LonLatDegrees {
                lon_degrees: 130.0,
                lat_degrees: 10.0,
            },
        ],
        &output,
    )
    .expect("write springjustment persistence");

    assert_eq!(
        report.dists_on_edge.output,
        root.join("result/distsOnEdge_NXP0009_03_global.nc4")
    );
    assert_eq!(
        report.cellwidth.as_ref().expect("cellwidth report").output,
        root.join("result/cellwidth_NXP0009_global.nc4")
    );

    let dists = netcdf::open(&report.dists_on_edge.output).expect("open dists");
    assert_eq!(read_f64(&dists, "lonv"), vec![10.0, 20.0]);
    assert_eq!(read_f64(&dists, "latv"), vec![1.0, 2.0]);
    assert_eq!(read_f64(&dists, "distsOnEdge"), vec![100.0, 200.0]);

    let cellwidth =
        netcdf::open(&report.cellwidth.expect("cellwidth report").output).expect("open cellwidth");
    assert_eq!(read_f64(&cellwidth, "lonw"), vec![110.0, 120.0, 130.0]);
    assert_eq!(read_f64(&cellwidth, "latw"), vec![-10.0, 0.0, 10.0]);
    assert_eq!(read_f64(&cellwidth, "cellwidth"), vec![12.0, 24.0, 48.0]);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn springjustment_global_persistence_skips_cellwidth_when_absent() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_springjustment_global_no_cellwidth_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("result")).expect("create result root");

    let report = earthmesh_cli::grid_quality_pipeline::write_springjustment_global_persistence(
        &root,
        10,
        4,
        &[],
        &spring_output(None),
    )
    .expect("write springjustment persistence");

    assert!(report.cellwidth.is_none());
    assert!(root
        .join("result/distsOnEdge_NXP0010_04_global.nc4")
        .exists());
    assert!(!root.join("result/cellwidth_NXP0010_global.nc4").exists());

    let _ = std::fs::remove_dir_all(&root);
}

fn spring_output(cellwidth: Option<Vec<f64>>) -> earthmesh_mesh::SpringjustmentGlobalCoreOutput {
    earthmesh_mesh::SpringjustmentGlobalCoreOutput {
        updated_triangle_lonlat: Vec::new(),
        updated_cell_lonlat: Vec::new(),
        triangle_neighbors: Vec::new(),
        cells_on_edge: Vec::new(),
        vertices_on_edge: Vec::new(),
        edges_on_vertex: Vec::new(),
        cells_on_vertex: Vec::new(),
        edges_on_cell: Vec::new(),
        cells_on_cell: Vec::new(),
        edges_on_edge_tri: Vec::new(),
        dists_on_edge: vec![100.0, 200.0],
        cellwidth,
        edge_lonlat: vec![
            earthmesh_mesh::LonLatDegrees {
                lon_degrees: 10.0,
                lat_degrees: 1.0,
            },
            earthmesh_mesh::LonLatDegrees {
                lon_degrees: 20.0,
                lat_degrees: 2.0,
            },
        ],
        spring: earthmesh_mesh::SpringDynamicsGlobalOutput {
            updated_cell_points: Vec::new(),
            last_edge_displacements: Vec::new(),
            last_frac_change_squared: Vec::new(),
            diagnostic_max_displacements: Vec::new(),
        },
    }
}

fn read_f64(file: &netcdf::File, name: &str) -> Vec<f64> {
    file.variable(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
        .get_values::<f64, _>(..)
        .unwrap_or_else(|err| panic!("read {name}: {err}"))
}
