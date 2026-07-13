#[test]
fn contain_reader_writer_round_trip_compatibility_schema() {
    let root =
        std::env::temp_dir().join(format!("earthmesh_cli_contain_io_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let output = root.join("contain.nc4");

    let contain = earthmesh_cli::contain_io::ContainMesh {
        ustr_id: vec![vec![1, 1, 0], vec![2, 1, 2], vec![1, 3, 1]],
        ustr_ii: vec![vec![10, 20, 1, 7], vec![11, 20, 0, 8], vec![12, 21, 1, 9]],
        is_in_area_ustr: vec![0, 1, -1],
    };

    let report =
        earthmesh_cli::contain_io::write_contain_netcdf(&output, &contain).expect("write contain");
    assert_eq!(report.output, output);
    assert_eq!(report.num_ustr, 3);
    assert_eq!(report.num_ii, 3);
    assert_eq!(report.dim_a, 3);
    assert_eq!(report.dim_b, 4);

    let read_back = earthmesh_cli::contain_io::read_contain_netcdf(&output).expect("read contain");
    assert_eq!(read_back, contain);

    let file = netcdf::open(&output).expect("open contain");
    assert_eq!(file.dimension("num_ustr").expect("num_ustr").len(), 3);
    assert_eq!(file.dimension("num_ii").expect("num_ii").len(), 3);
    assert_eq!(file.dimension("dim_a").expect("dim_a").len(), 3);
    assert_eq!(file.dimension("dim_b").expect("dim_b").len(), 4);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn contain_writer_rejects_ragged_rows_and_mask_length_mismatch() {
    let output = std::env::temp_dir().join("earthmesh_cli_bad_contain.nc4");
    let ragged = earthmesh_cli::contain_io::ContainMesh {
        ustr_id: vec![vec![1, 1], vec![2]],
        ustr_ii: vec![vec![1, 2]],
        is_in_area_ustr: vec![1, 1],
    };
    let err = earthmesh_cli::contain_io::write_contain_netcdf(&output, &ragged)
        .expect_err("ragged rejected");
    assert!(err
        .to_string()
        .contains("ustr_id rows must have uniform width"));

    let bad_mask = earthmesh_cli::contain_io::ContainMesh {
        ustr_id: vec![vec![1, 1]],
        ustr_ii: vec![vec![1, 2]],
        is_in_area_ustr: vec![1, 1],
    };
    let err = earthmesh_cli::contain_io::write_contain_netcdf(&output, &bad_mask)
        .expect_err("mask mismatch");
    assert!(err.to_string().contains("IsInArea_ustr length"));
}
