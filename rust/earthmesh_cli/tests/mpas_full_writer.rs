fn sample_full_mpas_mesh() -> earthmesh_cli::mpas_mesh_types::MpasMesh {
    earthmesh_cli::mpas_mesh_types::MpasMesh {
        lat_cell: vec![0.0, 0.11, 0.12],
        lon_cell: vec![0.0, 1.11, 1.12],
        x_cell: vec![0.0, 10.0, 20.0],
        y_cell: vec![0.0, 11.0, 21.0],
        z_cell: vec![0.0, 12.0, 22.0],
        lat_vertex: vec![0.0, 0.21, 0.22],
        lon_vertex: vec![0.0, 1.21, 1.22],
        x_vertex: vec![0.0, 30.0, 40.0],
        y_vertex: vec![0.0, 31.0, 41.0],
        z_vertex: vec![0.0, 32.0, 42.0],
        lat_edge: vec![0.0, 0.31, 0.32, 0.33],
        lon_edge: vec![0.0, 1.31, 1.32, 1.33],
        x_edge: vec![0.0, 50.0, 60.0, 70.0],
        y_edge: vec![0.0, 51.0, 61.0, 71.0],
        z_edge: vec![0.0, 52.0, 62.0, 72.0],
        n_edges_on_cell: vec![0, 3, 2],
        cells_on_cell: vec![seq_i32(0, 10), seq_i32(101, 10), seq_i32(201, 10)],
        vertices_on_cell: vec![seq_i32(0, 10), seq_i32(301, 10), seq_i32(401, 10)],
        edges_on_cell: vec![seq_i32(0, 10), seq_i32(501, 10), seq_i32(601, 10)],
        cells_on_vertex: vec![vec![0, 0, 0], vec![1, 2, 0], vec![2, 1, 0]],
        edges_on_vertex: vec![vec![0, 0, 0], vec![1, 2, 3], vec![3, 2, 1]],
        cells_on_edge: vec![[0, 0], [1, 2], [2, 0], [0, 1]],
        vertices_on_edge: vec![[0, 0], [1, 2], [2, 1], [1, 1]],
        n_edges_on_edge: vec![0, 4, 5, 6],
        edges_on_edge: vec![
            seq_i32(0, 20),
            seq_i32(701, 20),
            seq_i32(801, 20),
            seq_i32(901, 20),
        ],
        area_cell: vec![0.0, 1000.0, 2000.0],
        area_triangle: vec![0.0, 3000.0, 4000.0],
        kite_areas_on_vertex: vec![
            vec![0.0, 0.0, 0.0],
            vec![1.1, 1.2, 1.3],
            vec![2.1, 2.2, 2.3],
        ],
        dv_edge: vec![0.0, 5.1, 5.2, 5.3],
        dc_edge: vec![0.0, 6.1, 6.2, 6.3],
        angle_edge: vec![0.0, 7.1, 7.2, 7.3],
        weights_on_edge: vec![
            seq_f64(0.0, 20),
            seq_f64(10.0, 20),
            seq_f64(20.0, 20),
            seq_f64(30.0, 20),
        ],
        mesh_density: vec![0.0, 0.5, 0.25],
        nominal_min_dc: 12345.0,
        error_segment: vec![0.0, 0.01, 0.02, 0.03],
    }
}

#[test]
fn mpas_full_writer_preserves_canonical_schema_and_placeholder_slices() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mpas_full_writer_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let output = root.join("MPASOUT_NXP0007_global.nc4");

    let report = earthmesh_cli::write_mpas_mesh_netcdf(&output, &sample_full_mpas_mesh())
        .expect("write full MPAS mesh");

    assert_eq!(report.output, output);
    assert_eq!(report.n_cells, 2);
    assert_eq!(report.n_vertices, 2);
    assert_eq!(report.n_edges, 3);

    let file = netcdf::open(&report.output).expect("open full MPAS mesh");
    assert_eq!(file.dimension("nCells").expect("nCells").len(), 2);
    assert_eq!(file.dimension("nVertices").expect("nVertices").len(), 2);
    assert_eq!(file.dimension("nEdges").expect("nEdges").len(), 3);
    assert_eq!(file.dimension("maxEdges").expect("maxEdges").len(), 10);
    assert_eq!(file.dimension("maxEdges2").expect("maxEdges2").len(), 20);
    assert_eq!(file.dimension("TWO").expect("TWO").len(), 2);
    assert_eq!(
        file.dimension("vertexDegree").expect("vertexDegree").len(),
        3
    );

    assert_eq!(read_i32(&file, "indexToCellID"), vec![1, 2]);
    assert_eq!(read_i32(&file, "indexToVertexID"), vec![1, 2]);
    assert_eq!(read_i32(&file, "indexToEdgeID"), vec![1, 2, 3]);
    assert_eq!(read_f64(&file, "latCell"), vec![0.11, 0.12]);
    assert_eq!(read_f64(&file, "xVertex"), vec![30.0, 40.0]);
    assert_eq!(read_f64(&file, "zEdge"), vec![52.0, 62.0, 72.0]);
    assert_eq!(read_i32(&file, "nEdgesOnCell"), vec![3, 2]);
    assert_eq!(
        read_i32(&file, "cellsOnCell")[0..12],
        [101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 201, 202]
    );
    assert_eq!(read_i32(&file, "cellsOnVertex"), vec![1, 2, 0, 2, 1, 0]);
    assert_eq!(read_i32(&file, "boundaryVertex"), vec![0, 0]);
    assert_eq!(read_i32(&file, "cellsOnEdge"), vec![1, 2, 2, 0, 0, 1]);
    assert_eq!(read_i32(&file, "edgesOnEdge")[0..4], [701, 702, 703, 704]);
    assert_eq!(read_f64(&file, "areaCell"), vec![1000.0, 2000.0]);
    assert_eq!(
        read_f64(&file, "kiteAreasOnVertex"),
        vec![1.1, 1.2, 1.3, 2.1, 2.2, 2.3]
    );
    assert_eq!(read_f64(&file, "weightsOnEdge")[0..3], [10.0, 11.0, 12.0]);
    assert_eq!(read_f64(&file, "meshDensity"), vec![0.5, 0.25]);
    assert_eq!(read_f64(&file, "error_segment"), vec![0.01, 0.02, 0.03]);
    assert_eq!(read_f64(&file, "nominalMinDc"), vec![12345.0]);

    let mesh_spec: String = file
        .attribute("mesh_spec")
        .expect("mesh_spec")
        .value()
        .expect("read mesh_spec")
        .try_into()
        .expect("string attr");
    assert_eq!(mesh_spec, "1.0");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn mpas_ocean_writer_scales_physical_metrics_and_marks_boundary_vertices() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mpas_ocean_writer_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let output = root.join("MPAS-Ocean.nc4");
    let mut mesh = sample_full_mpas_mesh();
    mesh.cells_on_vertex[2] = vec![2, 0, 0];
    mesh.edges_on_cell[1] = vec![1, 2, 3, 0, 0, 0, 0, 0, 0, 0];
    mesh.edges_on_cell[2] = vec![2, 3, 0, 0, 0, 0, 0, 0, 0, 0];
    // This ordering makes angle2 the largest angle, so the expected value
    // distinguishes MPAS-Tools' historical b*c term from the cosine law's b*b.
    mesh.dc_edge = vec![0.0, 3.0, 5.0, 4.0];

    earthmesh_cli::write_mpas_ocean_mesh_netcdf(&output, &mesh).expect("write MPAS-Ocean mesh");
    let file = netcdf::open(&output).expect("open MPAS-Ocean mesh");
    let radius = earthmesh_cli::MPAS_OCEAN_SPHERE_RADIUS_METERS;
    let radius_value: f64 = file
        .attribute("sphere_radius")
        .expect("sphere_radius")
        .value()
        .expect("read sphere_radius")
        .try_into()
        .expect("f64 attr");
    assert_eq!(radius_value, radius);
    assert_eq!(read_i32(&file, "boundaryVertex"), vec![0, 1]);
    assert_eq!(read_f64(&file, "xCell"), vec![10.0 * radius, 20.0 * radius]);
    assert_eq!(
        read_f64(&file, "areaCell"),
        vec![1000.0 * radius * radius, 2000.0 * radius * radius]
    );
    assert_eq!(read_f64(&file, "dvEdge")[0], 5.1 * radius);
    assert_eq!(read_f64(&file, "angleEdge")[0], 7.1);
    assert_eq!(read_f64(&file, "weightsOnEdge")[0], 10.0);
    assert_eq!(read_f64(&file, "cellQuality"), vec![5.1 / 5.3, 5.2 / 5.3]);
    assert_f64_close(&read_f64(&file, "gridSpacing"), &[4.0, 4.5]);
    assert_eq!(read_f64(&file, "triangleQuality"), vec![3.0 / 5.0; 2]);
    assert_f64_close(
        &read_f64(&file, "triangleAngleQuality"),
        &[0.4728407233878185, 0.5639696094406146],
    );
    assert_eq!(read_i32(&file, "obtuseTriangle"), vec![0, 0]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn mpas_full_writer_rejects_dimension_mismatches() {
    let output = std::env::temp_dir().join("earthmesh_cli_bad_mpas_full.nc4");
    let mut bad = sample_full_mpas_mesh();
    bad.cells_on_cell[1].pop();
    let err = earthmesh_cli::write_mpas_mesh_netcdf(&output, &bad)
        .expect_err("bad full MPAS mesh rejected");
    assert!(err.to_string().contains("cells_on_cell"));
}

fn seq_i32(start: i32, len: usize) -> Vec<i32> {
    (0..len).map(|idx| start + idx as i32).collect()
}

fn seq_f64(start: f64, len: usize) -> Vec<f64> {
    (0..len).map(|idx| start + idx as f64).collect()
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

fn assert_f64_close(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len());
    for (&actual, &expected) in actual.iter().zip(expected) {
        assert!((actual - expected).abs() <= expected.abs().max(1.0) * 1.0e-12);
    }
}
