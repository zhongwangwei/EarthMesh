#[test]
fn earthmesh_info_writer_preserves_locmesh_info_schema() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_earthmesh_info_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let output = root.join("earthmesh_info.nc4");

    let info = earthmesh_cli::EarthmeshInfo {
        num_step_f: vec![1, 4, 7],
        refine_degree_f: vec![0, 0, 1, 1, 2],
        seaorland_ustr_f: vec![0, 1, -1, 1, -1],
    };

    let report =
        earthmesh_cli::write_earthmesh_info_netcdf(&output, &info).expect("write earthmesh info");
    assert_eq!(report.output, output);
    assert_eq!(report.num_step, 3);
    assert_eq!(report.num_ustr, 5);

    let file = netcdf::open(&output).expect("open earthmesh_info");
    assert_eq!(file.dimension("num_step").expect("num_step").len(), 3);
    assert_eq!(file.dimension("num_ustr").expect("num_ustr").len(), 5);
    assert_eq!(read_i32(&file, "num_step_f"), info.num_step_f);
    assert_eq!(read_i32(&file, "refine_degree_f"), info.refine_degree_f);
    assert_eq!(read_i32(&file, "seaorland_ustr_f"), info.seaorland_ustr_f);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn earthmesh_info_writer_rejects_ustr_vector_length_mismatches() {
    let output = std::env::temp_dir().join("earthmesh_cli_bad_earthmesh_info.nc4");
    let bad = earthmesh_cli::EarthmeshInfo {
        num_step_f: vec![1, 4],
        refine_degree_f: vec![0, 1, 2],
        seaorland_ustr_f: vec![1, -1],
    };

    let err =
        earthmesh_cli::write_earthmesh_info_netcdf(&output, &bad).expect_err("mismatch rejected");
    assert!(err
        .to_string()
        .contains("refine_degree_f and seaorland_ustr_f must have matching length"));
}

fn read_i32(file: &netcdf::File, name: &str) -> Vec<i32> {
    file.variable(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
        .get_values::<i32, _>(..)
        .unwrap_or_else(|err| panic!("read {name}: {err}"))
}
