use std::fs;

#[test]
fn iap_mesh_reader_preserves_canonical_placeholder_and_degree_conversion() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_iap_mesh_read_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let input = root.join("iap.nc4");
    write_iap_fixture(&input);

    let payload = earthmesh_cli::mode_file_io::read_iap_mesh_netcdf(&input).expect("read iap mesh");

    assert_eq!(payload.w_points.len(), 4);
    assert_eq!(payload.w_points[0].lon, 0.0);
    assert_eq!(payload.w_points[0].lat, 0.0);
    assert_close(payload.w_points[1].lon, -170.0);
    assert_close(payload.w_points[1].lat, 10.0);
    assert_close(payload.w_points[2].lon, 170.0);
    assert_close(payload.w_points[2].lat, -20.0);
    assert_close(payload.w_points[3].lon, 45.0);
    assert_close(payload.w_points[3].lat, 0.0);

    assert_eq!(
        payload.triangle_neighbors,
        vec![[1, 1, 1], [1, 2, 3], [3, 2, 1]]
    );
    assert_eq!(
        payload.triangle_vertices,
        vec![[1, 1, 1], [2, 3, 4], [4, 3, 2]]
    );

    let _ = fs::remove_dir_all(&root);
}

fn write_iap_fixture(path: &std::path::Path) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create fixture");
    file.add_dimension("sjx_points", 2).expect("sjx dim");
    file.add_dimension("lbx_points", 3).expect("lbx dim");
    file.add_dimension("dimb", 3).expect("dimb dim");
    file.add_variable::<f64>("GLONW", &["lbx_points"])
        .expect("GLONW var")
        .put_values(
            &[
                190.0_f64.to_radians(),
                (-190.0_f64).to_radians(),
                45.0_f64.to_radians(),
            ],
            ..,
        )
        .expect("GLONW values");
    file.add_variable::<f64>("GLATW", &["lbx_points"])
        .expect("GLATW var")
        .put_values(
            &[10.0_f64.to_radians(), (-20.0_f64).to_radians(), 0.0_f64],
            ..,
        )
        .expect("GLATW values");
    file.add_variable::<i32>("itab_m%im", &["sjx_points", "dimb"])
        .expect("itab_m%im var")
        .put_values(&[0, 1, 2, 2, 1, 0], (.., ..))
        .expect("itab_m%im values");
    file.add_variable::<i32>("itab_m%iw", &["sjx_points", "dimb"])
        .expect("itab_m%iw var")
        .put_values(&[1, 2, 3, 3, 2, 1], (.., ..))
        .expect("itab_m%iw values");
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1.0e-10,
        "actual {actual} expected {expected}"
    );
}
