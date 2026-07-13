#[test]
fn mpas_graph_info_writer_matches_canonical_placeholder_and_interior_edge_rules() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mpas_graph_info_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let output = root.join("graph.info");

    let report = earthmesh_cli::mpas_graph_info_writer::write_mpas_graph_info(
        &output,
        4,
        &[
            vec![0, 0, 0, 0],
            vec![2, 3, 0, 0],
            vec![1, 3, 4, 0],
            vec![1, 2, 0, 0],
        ],
        &[[0, 0], [1, 2], [2, 0], [3, 4], [0, 4]],
        &[0, 2, 3, 2],
    )
    .expect("write graph.info");

    assert_eq!(report.output, output);
    assert_eq!(report.n_cells_written, 3);
    assert_eq!(report.interior_edges, 2);
    assert_eq!(report.cells_with_boundary_edges, 0);

    let contents = std::fs::read_to_string(&report.output).expect("read graph.info");
    assert_eq!(
        contents,
        "         3         2\n         2         3\n         1         3         4\n         1         2\n"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn mpas_graph_info_writer_reports_cells_with_missing_neighbors_and_rejects_bad_width() {
    let output = std::env::temp_dir().join("earthmesh_cli_bad_graph.info");
    let report = earthmesh_cli::mpas_graph_info_writer::write_mpas_graph_info(
        &output,
        3,
        &[vec![0, 0, 0], vec![2, 0, 0]],
        &[[0, 0], [1, 0]],
        &[0, 2],
    )
    .expect("write graph with boundary cell");
    assert_eq!(report.cells_with_boundary_edges, 1);
    assert_eq!(
        std::fs::read_to_string(&output).expect("read"),
        "         1         0\n         2\n"
    );
    let _ = std::fs::remove_file(&output);

    let bad = earthmesh_cli::mpas_graph_info_writer::write_mpas_graph_info(
        &output,
        3,
        &[vec![0, 0], vec![1, 2]],
        &[[0, 0], [1, 2]],
        &[0, 2],
    )
    .expect_err("bad maxEdges rejected");
    assert!(bad.to_string().contains("max_edges"));
}
