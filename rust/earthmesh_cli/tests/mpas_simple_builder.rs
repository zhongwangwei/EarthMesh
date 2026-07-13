fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1.0e-12,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn mpas_simple_builder_ports_cal_simple_payload_semantics() {
    let mesh = earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
        m_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 90.0,
                lat: 0.0,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 0.0,
                lat: 90.0,
            },
        ],
        w_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 180.0,
                lat: 0.0,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 90.0,
                lat: 0.0,
            },
        ],
        m_to_w: vec![[1, 1, 1], [1, 2, 3], [2, 3, 1], [3, 1, 2]],
        w_to_m: vec![vec![1], vec![1, 2, 3], vec![2, 3, 1], vec![3, 1, 2]],
        n_w_to_m: vec![1, 3, 3, 3],
    };
    let cellwidth = vec![10.0, 20.0, 40.0, 80.0];

    let simple =
        earthmesh_cli::mpas_unstructured_mesh_builders::build_mpas_simple_mesh_from_unstructured_one_based(&mesh, &cellwidth)
            .expect("build MPAS Simple payload");

    assert_eq!(
        simple.cells_on_vertex,
        vec![vec![0, 0, 0], vec![0, 1, 2], vec![1, 2, 0], vec![2, 0, 1]]
    );
    assert_eq!(simple.x_cell.len(), mesh.w_points.len());
    assert_eq!(simple.x_vertex.len(), mesh.m_points.len());

    assert_close(simple.x_cell[1], 1.0);
    assert_close(simple.y_cell[1], 0.0);
    assert_close(simple.z_cell[1], 0.0);
    assert_close(simple.x_cell[2], -1.0);
    assert_close(simple.y_cell[2], 0.0);
    assert_close(simple.z_cell[2], 0.0);
    assert_close(simple.x_cell[3], 0.0);
    assert_close(simple.y_cell[3], 1.0);
    assert_close(simple.z_cell[3], 0.0);

    assert_close(simple.x_vertex[1], 1.0);
    assert_close(simple.y_vertex[1], 0.0);
    assert_close(simple.z_vertex[1], 0.0);
    assert_close(simple.x_vertex[2], 0.0);
    assert_close(simple.y_vertex[2], 1.0);
    assert_close(simple.z_vertex[2], 0.0);
    assert_close(simple.x_vertex[3], 0.0);
    assert_close(simple.y_vertex[3], 0.0);
    assert_close(simple.z_vertex[3], 1.0);

    assert_eq!(simple.mesh_density[0], 1.0);
    assert_close(simple.mesh_density[1], (10.0_f64 / 20.0).powi(4));
    assert_close(simple.mesh_density[2], (10.0_f64 / 40.0).powi(4));
    assert_close(simple.mesh_density[3], (10.0_f64 / 80.0).powi(4));
}

#[test]
fn mpas_simple_builder_rejects_invalid_cellwidth_inputs() {
    let mesh = earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
        m_points: vec![earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 }],
        w_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 1.0, lat: 1.0 },
        ],
        m_to_w: vec![[1, 1, 1]],
        w_to_m: vec![vec![1], vec![1]],
        n_w_to_m: vec![1, 1],
    };

    let short =
        earthmesh_cli::mpas_unstructured_mesh_builders::build_mpas_simple_mesh_from_unstructured_one_based(&mesh, &[1.0])
            .expect_err("short cellwidth rejected");
    assert!(short.to_string().contains("cellwidth length"));

    let zero =
        earthmesh_cli::mpas_unstructured_mesh_builders::build_mpas_simple_mesh_from_unstructured_one_based(&mesh, &[1.0, 0.0])
            .expect_err("zero cellwidth rejected");
    assert!(zero.to_string().contains("positive"));
}

#[test]
fn mpas_simple_file_pipeline_reads_inputs_and_writes_simple_output() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mpas_simple_pipeline_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let gridfile = root.join("gridfile_NXP0004_tri.nc4");
    let cellwidth_file = root.join("cellwidth_NXP0004_global.nc4");
    let output = root.join("MPASOUT_NXP0004_global_Simple.nc4");

    let mesh = earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
        m_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 90.0,
                lat: 0.0,
            },
        ],
        w_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 180.0,
                lat: 0.0,
            },
        ],
        m_to_w: vec![[1, 1, 1], [1, 2, 1], [2, 1, 2]],
        w_to_m: vec![vec![1], vec![1, 2], vec![2, 1]],
        n_w_to_m: vec![1, 2, 2],
    };
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf(&gridfile, &mesh)
        .expect("write gridfile");
    write_cellwidth_fixture(&cellwidth_file, &[12.0, 24.0, 48.0]);

    let cellwidth = earthmesh_cli::mesh_metric_writers::read_cellwidth_netcdf(&cellwidth_file)
        .expect("read cellwidth file");
    assert_eq!(cellwidth, vec![12.0, 24.0, 48.0]);

    let report = earthmesh_cli::gridfile_output_writers::write_mpas_simple_mesh_from_netcdf_inputs(
        &gridfile,
        &cellwidth_file,
        &output,
    )
    .expect("write MPAS Simple from files");
    assert_eq!(report.n_cells, 2);
    assert_eq!(report.n_vertices, 2);

    let file = netcdf::open(&output).expect("open simple output");
    assert_eq!(read_f64(&file, "xCell"), vec![1.0, -1.0]);
    assert_eq!(
        read_f64(&file, "meshDensity"),
        vec![(12.0_f64 / 24.0).powi(4), (12.0_f64 / 48.0).powi(4)]
    );
    assert_eq!(read_i32(&file, "cellsOnVertex"), vec![0, 1, 0, 1, 0, 1]);

    let _ = std::fs::remove_dir_all(&root);
}

fn write_cellwidth_fixture(path: &std::path::Path, values: &[f64]) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create cellwidth fixture");
    file.add_dimension("num_dbx", values.len())
        .expect("num_dbx dim");
    let mut var = file
        .add_variable::<f64>("cellwidth", &["num_dbx"])
        .expect("cellwidth var");
    var.put_values(values, ..).expect("cellwidth values");
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
