use std::fs;

#[test]
fn mode4mesh_make_lambert_netcdf_writes_gridfile_mode4_schema() {
    let root = std::env::temp_dir().join(format!("earthmesh_cli_mode4mesh_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("gridfile")).expect("create gridfile dir");
    let source = root.join("lambert_mode.nc4");
    {
        let mut file = netcdf::create(&source).expect("create source");
        file.add_dimension("xi_vert", 2).expect("xi dim");
        file.add_dimension("eta_vert", 3).expect("eta dim");
        file.add_variable::<f64>("lon_vert", &["xi_vert", "eta_vert"])
            .expect("lon var")
            .put_values(&[181.0, 182.0, 183.0, 184.0, 185.0, 186.0], (.., ..))
            .expect("write lon");
        file.add_variable::<f64>("lat_vert", &["xi_vert", "eta_vert"])
            .expect("lat var")
            .put_values(&[10.0, 11.0, 12.0, 13.0, 14.0, 15.0], (.., ..))
            .expect("write lat");
    }

    let output = root.join("gridfile/gridfile_mode4_lambert.nc4");
    let report = earthmesh_cli::mode4mesh_make_netcdf(&source, "lambert", &output)
        .expect("mode4mesh_make lambert nc");

    assert_eq!(report.input, source);
    assert_eq!(report.grid_select, "lambert");
    assert_eq!(report.output, output);
    assert_eq!(report.bound_points, 7);
    assert_eq!(report.mode_points, 3);
    let file = netcdf::open(&report.output).expect("open output");
    assert_eq!(file.dimension("bound_points").unwrap().len(), 7);
    assert_eq!(file.dimension("mode_points").unwrap().len(), 3);
    assert_eq!(
        file.variable("lonlat_bound")
            .unwrap()
            .get_values::<f64, _>((.., ..))
            .unwrap(),
        vec![
            -999.0, -999.0, -179.0, 10.0, -178.0, 11.0, -177.0, 12.0, -176.0, 13.0, -175.0, 14.0,
            -174.0, 15.0,
        ]
    );
    assert_eq!(
        file.variable("ngr_bound")
            .unwrap()
            .get_values::<i32, _>((.., ..))
            .unwrap(),
        vec![1, 1, 1, 1, 2, 3, 5, 4, 4, 5, 7, 6]
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn mode4mesh_make_rejects_unsupported_grid_select_and_nml_lambert_like_fortran() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mode4mesh_reject_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create root");
    let nml = root.join("mode.nml");
    fs::write(&nml, "&dummy\n/\n").expect("write nml");
    let out = root.join("out.nc4");

    let err = earthmesh_cli::mode4mesh_make_netcdf(&nml, "lambert", &out)
        .expect_err("lambert nml is unsupported like Fortran");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);

    let err = earthmesh_cli::mode4mesh_make_netcdf(&nml, "cubical", &out)
        .expect_err("cubical unsupported");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);

    let _ = fs::remove_dir_all(&root);
}
