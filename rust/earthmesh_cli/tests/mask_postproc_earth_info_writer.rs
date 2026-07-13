fn sample_layout() -> earthmesh_cli::mask_postproc_types::MaskPostprocLayout {
    earthmesh_cli::mask_postproc_types::MaskPostprocLayout {
        ustr_points: 5,
        ustr_bounds: 8,
        center_points: vec![earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 }; 5],
        vertex_points: vec![earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 }; 8],
        center_neighbors: vec![
            vec![1, 1, 1],
            vec![1, 1, 1],
            vec![2, 3, 4],
            vec![5, 6, 7],
            vec![3, 4, 5],
        ],
        vertex_neighbors: vec![vec![1]; 8],
        center_neighbor_counts: vec![0, 0, 3, 3, 3],
        vertex_neighbor_counts: vec![0; 8],
    }
}

#[test]
fn earth_info_writer_uses_earth_plan_result_path_and_builder() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mask_postproc_earth_info_{}",
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
    let layout = sample_layout();

    let report = earthmesh_cli::mask_postproc_patchtypes::write_mask_postproc_earth_info_netcdf(
        &plan,
        &[3],
        5,
        &layout,
        &[0, 0, 1, -1, 1],
        &[0, 0, 1, 0, -1],
    )
    .expect("write earth info through plan");

    assert_eq!(report.output, root.join("result/earthmesh_info.nc4"));
    assert_eq!(report.num_step, 2);
    assert_eq!(report.num_ustr, 4);

    let file = netcdf::open(&report.output).expect("open earthmesh_info");
    assert_eq!(read_i32(&file, "num_step_f"), vec![3, 5]);
    assert_eq!(read_i32(&file, "seaorland_ustr_f"), vec![0, 0, 1, -1]);
    assert_eq!(read_i32(&file, "refine_degree_f"), vec![0, 0, 0, 0]);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn earth_info_writer_rejects_non_earth_plan() {
    let root = std::env::temp_dir().join("earthmesh_cli_land_has_no_earth_info");
    let plan = earthmesh_cli::mask_postproc_domain::plan_mask_postproc_domain_io(
        &root, 7, "tri", "landmesh", false,
    )
    .expect("land plan");
    let layout = sample_layout();

    let err = earthmesh_cli::mask_postproc_patchtypes::write_mask_postproc_earth_info_netcdf(
        &plan,
        &[3],
        5,
        &layout,
        &[0, 0, 1, -1, 1],
        &[0, 0, 1, 0, -1],
    )
    .expect_err("land plan rejected");

    assert!(err.to_string().contains("earthmesh_info"));
}

fn read_i32(file: &netcdf::File, name: &str) -> Vec<i32> {
    file.variable(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
        .get_values::<i32, _>(..)
        .unwrap_or_else(|err| panic!("read {name}: {err}"))
}
