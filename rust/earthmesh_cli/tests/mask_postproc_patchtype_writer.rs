#[test]
fn mask_postproc_patchtype_writer_uses_plan_path_and_coordinate_builder() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mask_postproc_patchtype_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let plan = earthmesh_cli::mask_postproc_domain::plan_mask_postproc_domain_io(
        &root,
        7,
        "tri",
        "earthmesh",
        false,
    )
    .expect("earth plan");
    let output = plan.patchtype_output.clone().expect("patchtype output");

    let report = earthmesh_cli::mask_postproc_patchtypes::write_mask_postproc_patchtype_netcdf(
        &plan,
        vec![vec![2, 0], vec![3, 4]],
        1,
        3,
        &[10.0, 11.0, 12.0, 13.0],
        &[50.0, 49.0, 48.0, 47.0, 46.0],
        &[10.5, 11.5, 12.5],
        &[49.5, 48.5, 47.5, 46.5],
    )
    .expect("write patchtype through plan");

    assert_eq!(report.output, output);
    assert_eq!(report.nlon, 2);
    assert_eq!(report.nlat, 2);

    let file = netcdf::open(&report.output).expect("open patchtype");
    assert_eq!(read_i32(&file, "elmindex"), vec![2, 0, 3, 4]);
    assert_eq!(read_f64(&file, "lon_w"), vec![11.0, 12.0]);
    assert_eq!(read_f64(&file, "lon_e"), vec![12.0, 13.0]);
    assert_eq!(read_f64(&file, "lat_n"), vec![47.0, 48.0]);
    assert_eq!(read_f64(&file, "lat_s"), vec![46.0, 47.0]);
    assert_eq!(read_f64(&file, "longitude"), vec![11.5, 12.5]);
    assert_eq!(read_f64(&file, "latitude"), vec![46.5, 47.5]);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn mask_postproc_patchtype_writer_accepts_full_domain_north_latitude_start() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mask_postproc_patchtype_full_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let plan = earthmesh_cli::mask_postproc_domain::plan_mask_postproc_domain_io(
        &root,
        7,
        "tri",
        "earthmesh",
        false,
    )
    .expect("earth plan");

    let report = earthmesh_cli::mask_postproc_patchtypes::write_mask_postproc_patchtype_netcdf(
        &plan,
        vec![vec![2, 0], vec![3, 4]],
        1,
        1,
        &[10.0, 11.0, 12.0, 13.0],
        &[50.0, 49.0, 48.0, 47.0],
        &[10.5, 11.5, 12.5],
        &[49.5, 48.5, 47.5],
    )
    .expect("write full-domain north-start patchtype");

    let file = netcdf::open(&report.output).expect("open patchtype");
    assert_eq!(read_f64(&file, "lat_n"), vec![49.0, 48.0]);
    assert_eq!(read_f64(&file, "lat_s"), vec![48.0, 47.0]);
    assert_eq!(read_f64(&file, "latitude"), vec![48.5, 47.5]);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn mask_postproc_patchtype_writer_rejects_plans_without_patchtype_output() {
    let root = std::env::temp_dir().join("earthmesh_cli_no_patchtype_plan");
    let plan = earthmesh_cli::mask_postproc_domain::plan_mask_postproc_domain_io(
        &root,
        7,
        "tri",
        "oceanmesh",
        false,
    )
    .expect("ocean plan");

    let err = earthmesh_cli::mask_postproc_patchtypes::write_mask_postproc_patchtype_netcdf(
        &plan,
        vec![vec![0]],
        0,
        0,
        &[0.0, 1.0],
        &[1.0, 0.0],
        &[0.5],
        &[0.5],
    )
    .expect_err("ocean plan has no patchtype output");

    assert!(err.to_string().contains("patchtype_output"));
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
