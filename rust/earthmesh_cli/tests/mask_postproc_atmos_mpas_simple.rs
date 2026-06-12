#[test]
fn atmos_mpas_simple_dispatch_uses_legacy_result_paths() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_atmos_mpas_simple_{}",
        std::process::id()
    ));
    let result_dir = root.join("result");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&result_dir).expect("create result dir");

    let mesh = earthmesh_cli::UnstructuredMesh {
        m_points: vec![
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint {
                lon: 90.0,
                lat: 0.0,
            },
        ],
        w_points: vec![
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint {
                lon: 180.0,
                lat: 0.0,
            },
        ],
        m_to_w: vec![[1, 1, 1], [1, 2, 1], [2, 1, 2]],
        w_to_m: vec![vec![1], vec![1, 2], vec![2, 1]],
        n_w_to_m: vec![1, 2, 2],
    };
    earthmesh_cli::write_unstructured_mesh_netcdf(
        result_dir.join("gridfile_NXP0009_tri.nc4"),
        &mesh,
    )
    .expect("write source gridfile");
    write_cellwidth_fixture(
        &result_dir.join("cellwidth_NXP0009_global.nc4"),
        &[12.0, 24.0, 48.0],
    );

    let report = earthmesh_cli::write_mask_postproc_atmos_mpas_simple_netcdf(
        &root,
        9,
        "tri",
        "atmosmesh",
        "MPAS-Simple",
    )
    .expect("write atmos MPAS-Simple output");

    let expected_output = result_dir.join("MPASOUT_NXP0009_global_Simple.nc4");
    assert_eq!(report.output, expected_output);
    assert_eq!(report.n_cells, 2);
    assert_eq!(report.n_vertices, 2);
    let file = netcdf::open(&expected_output).expect("open output");
    assert_eq!(read_f64(&file, "xCell"), vec![1.0, -1.0]);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn atmos_mpas_simple_dispatch_rejects_wrong_branch() {
    let err = earthmesh_cli::write_mask_postproc_atmos_mpas_simple_netcdf(
        std::env::temp_dir(),
        9,
        "tri",
        "earthmesh",
        "MPAS-Simple",
    )
    .expect_err("non-atmos rejected");
    assert!(err.to_string().contains("atmosmesh"));

    let err = earthmesh_cli::write_mask_postproc_atmos_mpas_simple_netcdf(
        std::env::temp_dir(),
        9,
        "tri",
        "atmosmesh",
        "MPAS",
    )
    .expect_err("full MPAS rejected");
    assert!(err.to_string().contains("MPAS-Simple"));
}

fn write_cellwidth_fixture(path: &std::path::Path, values: &[f64]) {
    let mut file = netcdf::create(path).expect("create cellwidth fixture");
    file.add_dimension("num_dbx", values.len())
        .expect("num_dbx dim");
    let mut var = file
        .add_variable::<f64>("cellwidth", &["num_dbx"])
        .expect("cellwidth var");
    var.put_values(values, ..).expect("cellwidth values");
}

fn read_f64(file: &netcdf::File, name: &str) -> Vec<f64> {
    file.variable(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
        .get_values::<f64, _>(..)
        .unwrap_or_else(|err| panic!("read {name}: {err}"))
}
