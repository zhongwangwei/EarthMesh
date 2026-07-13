#[test]
fn patchtype_writer_preserves_patchid_save_schema() {
    let root = std::env::temp_dir().join(format!("earthmesh_cli_patchtype_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let output = root.join("patchtype.nc4");

    let patch = earthmesh_cli::mask_postproc_writers::PatchIdMesh {
        elmindex: vec![vec![2, 3], vec![4, 5], vec![6, 7]],
        lon_w: vec![100.0, 101.0, 102.0],
        lon_e: vec![101.0, 102.0, 103.0],
        lat_n: vec![30.0, 29.0],
        lat_s: vec![29.0, 28.0],
        longitude: vec![100.5, 101.5, 102.5],
        latitude: vec![29.5, 28.5],
    };

    let report = earthmesh_cli::mask_postproc_writers::write_patchid_netcdf(&output, &patch)
        .expect("write patchid");
    assert_eq!(report.output, output);
    assert_eq!(report.nlon, 3);
    assert_eq!(report.nlat, 2);

    let file = netcdf::open(&output).expect("open patchtype");
    assert_eq!(file.dimension("nlon").expect("nlon").len(), 3);
    assert_eq!(file.dimension("nlat").expect("nlat").len(), 2);
    assert_eq!(read_i32(&file, "elmindex"), vec![2, 3, 4, 5, 6, 7]);
    assert_eq!(read_f64(&file, "lon_w"), patch.lon_w);
    assert_eq!(read_f64(&file, "lon_e"), patch.lon_e);
    assert_eq!(read_f64(&file, "lat_n"), patch.lat_n);
    assert_eq!(read_f64(&file, "lat_s"), patch.lat_s);
    assert_eq!(read_f64(&file, "longitude"), patch.longitude);
    assert_eq!(read_f64(&file, "latitude"), patch.latitude);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn patchtype_writer_rejects_dimension_mismatches() {
    let output = std::env::temp_dir().join("earthmesh_cli_bad_patchtype.nc4");
    let bad = earthmesh_cli::mask_postproc_writers::PatchIdMesh {
        elmindex: vec![vec![1, 2], vec![3]],
        lon_w: vec![0.0, 1.0],
        lon_e: vec![1.0, 2.0],
        lat_n: vec![1.0, 0.0],
        lat_s: vec![0.0, -1.0],
        longitude: vec![0.5, 1.5],
        latitude: vec![0.5, -0.5],
    };
    let err = earthmesh_cli::mask_postproc_writers::write_patchid_netcdf(&output, &bad)
        .expect_err("ragged rejected");
    assert!(err
        .to_string()
        .contains("elmindex rows must have uniform width"));
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
