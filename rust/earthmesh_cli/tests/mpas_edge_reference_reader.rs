#[test]
fn mpas_edge_reference_reader_matches_fortran_data_read_adjustments() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mpas_edge_reference_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let input = root.join("MPASOUT_NXP0007_global.nc4");
    write_mpas_edge_reference_fixture(&input);

    let reference =
        earthmesh_cli::read_mpas_edge_reference_netcdf(&input).expect("read edge reference");

    assert_eq!(
        reference.cells_on_edge_reference,
        vec![[1, 1], [2, 3], [3, 1], [1, 2]]
    );
    assert_eq!(reference.edge_points.len(), 4);
    assert_eq!(
        reference.edge_points[0],
        earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 }
    );
    assert_close(reference.edge_points[1].lon, 10.0);
    assert_close(reference.edge_points[1].lat, -5.0);
    assert_close(reference.edge_points[2].lon, -170.0);
    assert_close(reference.edge_points[2].lat, 15.0);
    assert_close(reference.edge_points[3].lon, -20.0);
    assert_close(reference.edge_points[3].lat, 25.0);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn mpas_edge_reference_reader_rejects_bad_two_dimension() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_bad_mpas_edge_reference_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let input = root.join("bad.nc4");
    let mut file = netcdf::create(&input).expect("create bad fixture");
    file.add_dimension("nEdges", 1).expect("nEdges");
    file.add_dimension("TWO", 3).expect("bad TWO");
    file.add_variable::<i32>("cellsOnEdge", &["nEdges", "TWO"])
        .expect("cells var")
        .put_values(&[1, 2, 3], (.., ..))
        .expect("cells values");
    file.add_variable::<f64>("lonEdge", &["nEdges"])
        .expect("lon var")
        .put_values(&[0.0], ..)
        .expect("lon values");
    file.add_variable::<f64>("latEdge", &["nEdges"])
        .expect("lat var")
        .put_values(&[0.0], ..)
        .expect("lat values");
    drop(file);

    let err = earthmesh_cli::read_mpas_edge_reference_netcdf(&input).expect_err("bad TWO rejected");
    assert!(err.to_string().contains("TWO"));

    let _ = std::fs::remove_dir_all(&root);
}

fn write_mpas_edge_reference_fixture(path: &std::path::Path) {
    let mut file = netcdf::create(path).expect("create fixture");
    file.add_dimension("nEdges", 3).expect("nEdges");
    file.add_dimension("TWO", 2).expect("TWO");
    file.add_variable::<i32>("cellsOnEdge", &["nEdges", "TWO"])
        .expect("cells var")
        .put_values(&[1, 2, 2, 0, 0, 1], (.., ..))
        .expect("cells values");
    file.add_variable::<f64>("lonEdge", &["nEdges"])
        .expect("lon var")
        .put_values(
            &[deg_to_rad(10.0), deg_to_rad(190.0), deg_to_rad(-20.0)],
            ..,
        )
        .expect("lon values");
    file.add_variable::<f64>("latEdge", &["nEdges"])
        .expect("lat var")
        .put_values(&[deg_to_rad(-5.0), deg_to_rad(15.0), deg_to_rad(25.0)], ..)
        .expect("lat values");
}

fn deg_to_rad(value: f64) -> f64 {
    value * std::f64::consts::PI / 180.0
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1.0e-12,
        "expected {expected}, got {actual}"
    );
}
