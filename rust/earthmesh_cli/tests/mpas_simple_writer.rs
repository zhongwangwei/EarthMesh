#[test]
fn mpas_simple_writer_preserves_canonical_schema_and_placeholder_slices() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mpas_simple_writer_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let output = root.join("MPASOUT_NXP0007_global_Simple.nc4");

    let mesh = earthmesh_cli::mpas_simple_writer::MpasSimpleMesh {
        x_cell: vec![0.0, 10.0, 20.0],
        y_cell: vec![0.0, 11.0, 21.0],
        z_cell: vec![0.0, 12.0, 22.0],
        x_vertex: vec![0.0, 30.0, 40.0, 50.0],
        y_vertex: vec![0.0, 31.0, 41.0, 51.0],
        z_vertex: vec![0.0, 32.0, 42.0, 52.0],
        cells_on_vertex: vec![vec![0, 0, 0], vec![0, 1, 2], vec![1, 0, 2], vec![2, 1, 0]],
        mesh_density: vec![0.0, 1.0, 0.25],
    };

    let report = earthmesh_cli::mpas_simple_writer::write_mpas_simple_mesh_netcdf(&output, &mesh)
        .expect("write MPAS simple mesh");

    assert_eq!(report.output, output);
    assert_eq!(report.n_cells, 2);
    assert_eq!(report.n_vertices, 3);

    let file = netcdf::open(&report.output).expect("open MPAS simple mesh");
    assert_eq!(file.dimension("nCells").expect("nCells").len(), 2);
    assert_eq!(file.dimension("nVertices").expect("nVertices").len(), 3);
    assert_eq!(
        file.dimension("vertexDegree").expect("vertexDegree").len(),
        3
    );
    assert_eq!(read_f64(&file, "xCell"), vec![10.0, 20.0]);
    assert_eq!(read_f64(&file, "yCell"), vec![11.0, 21.0]);
    assert_eq!(read_f64(&file, "zCell"), vec![12.0, 22.0]);
    assert_eq!(read_f64(&file, "xVertex"), vec![30.0, 40.0, 50.0]);
    assert_eq!(read_f64(&file, "yVertex"), vec![31.0, 41.0, 51.0]);
    assert_eq!(read_f64(&file, "zVertex"), vec![32.0, 42.0, 52.0]);
    assert_eq!(
        read_i32(&file, "cellsOnVertex"),
        vec![0, 1, 2, 1, 0, 2, 2, 1, 0]
    );
    assert_eq!(read_f64(&file, "meshDensity"), vec![1.0, 0.25]);

    let on_sphere = file.attribute("on_a_sphere").expect("on_a_sphere");
    let on_sphere_value: String = on_sphere
        .value()
        .expect("read on_a_sphere")
        .try_into()
        .expect("string attr");
    assert_eq!(on_sphere_value, "YES");

    let radius = file.attribute("sphere_radius").expect("sphere_radius");
    let radius_value: f64 = radius
        .value()
        .expect("read sphere_radius")
        .try_into()
        .expect("f64 attr");
    assert_eq!(radius_value, 1.0);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn mpas_simple_writer_rejects_dimension_mismatches() {
    let output = std::env::temp_dir().join("earthmesh_cli_bad_mpas_simple.nc4");
    let bad = earthmesh_cli::mpas_simple_writer::MpasSimpleMesh {
        x_cell: vec![0.0, 1.0],
        y_cell: vec![0.0],
        z_cell: vec![0.0, 1.0],
        x_vertex: vec![0.0, 1.0],
        y_vertex: vec![0.0, 1.0],
        z_vertex: vec![0.0, 1.0],
        cells_on_vertex: vec![vec![0, 0, 0], vec![1, 2]],
        mesh_density: vec![0.0, 1.0],
    };

    let err = earthmesh_cli::mpas_simple_writer::write_mpas_simple_mesh_netcdf(&output, &bad)
        .expect_err("bad simple mesh rejected");
    assert!(err.to_string().contains("y_cell length"));
}

fn read_i32(file: &netcdf::File, name: &str) -> Vec<i32> {
    file.variable(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
        .get_values::<i32, _>(..)
        .unwrap_or_else(|err| panic!("read {name}: {err}"))
}

fn read_f64(file: &netcdf::File, name: &str) -> Vec<f64> {
    file.variable(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
        .get_values::<f64, _>(..)
        .unwrap_or_else(|err| panic!("read {name}: {err}"))
}
