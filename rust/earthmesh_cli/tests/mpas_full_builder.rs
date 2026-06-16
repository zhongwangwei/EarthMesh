#[test]
fn mpas_full_builder_composes_geometry_payload_and_writer() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mpas_full_builder_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let mesh = closed_fixture_mesh();
    let cellwidth = vec![100.0; mesh.w_points.len()];

    let mpas =
        earthmesh_cli::build_mpas_mesh_from_unstructured_fortran_indexed(&mesh, &cellwidth, 9, 3)
            .expect("build full MPAS mesh payload");

    assert_eq!(mpas.x_cell.len(), mesh.w_points.len());
    assert_eq!(mpas.x_vertex.len(), mesh.m_points.len());
    assert!(mpas.x_edge.len() > 2);
    assert_eq!(mpas.n_edges_on_cell[2], 3);
    assert_eq!(mpas.vertices_on_cell[2].len(), 10);
    assert!(mpas.vertices_on_cell[2][0] >= 1);
    assert!(mpas.cells_on_vertex[2].iter().all(|id| *id >= 0));
    assert!(mpas.area_cell[2] > 0.0);
    assert!(mpas.area_triangle[2] > 0.0);
    assert_eq!(mpas.kite_areas_on_vertex[2].len(), 3);
    assert_eq!(mpas.edges_on_edge[2].len(), 20);
    assert_eq!(mpas.weights_on_edge[2].len(), 20);
    assert!(mpas.nominal_min_dc > 0.0);

    let output = root.join("MPASOUT_NXP0009_global.nc4");
    let report =
        earthmesh_cli::write_mpas_mesh_netcdf(&output, &mpas).expect("write full MPAS mesh");
    assert_eq!(report.n_cells, mesh.w_points.len() - 1);
    assert_eq!(report.n_vertices, mesh.m_points.len() - 1);
    assert_eq!(report.output, output);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn mpas_full_builder_restores_fortran_single_placeholder_payload_shape() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mpas_full_fortran_placeholder_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let mesh = fortran_single_placeholder_fixture_mesh();
    let cellwidth = vec![100.0; mesh.w_points.len()];

    let mpas =
        earthmesh_cli::build_mpas_mesh_from_unstructured_fortran_indexed(&mesh, &cellwidth, 9, 3)
            .expect("build MPAS from Fortran single-placeholder mesh");
    assert_eq!(mpas.x_cell.len(), mesh.w_points.len());
    assert_eq!(mpas.x_vertex.len(), mesh.m_points.len());

    let output = root.join("MPASOUT_NXP0009_global.nc4");
    let report =
        earthmesh_cli::write_mpas_mesh_netcdf(&output, &mpas).expect("write full MPAS mesh");
    assert_eq!(report.n_cells, mesh.w_points.len() - 1);
    assert_eq!(report.n_vertices, mesh.m_points.len() - 1);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn mpas_full_builder_rejects_bad_cellwidth_length() {
    let mesh = closed_fixture_mesh();
    let err =
        earthmesh_cli::build_mpas_mesh_from_unstructured_fortran_indexed(&mesh, &[100.0], 9, 3)
            .expect_err("bad cellwidth rejected");
    assert!(err.to_string().contains("cellwidth length"));
}

fn fortran_single_placeholder_fixture_mesh() -> earthmesh_cli::UnstructuredMesh {
    let legacy = closed_fixture_mesh();
    earthmesh_cli::UnstructuredMesh {
        m_points: std::iter::once(legacy.m_points[0])
            .chain(legacy.m_points[2..].iter().copied())
            .collect(),
        w_points: std::iter::once(legacy.w_points[0])
            .chain(legacy.w_points[2..].iter().copied())
            .collect(),
        m_to_w: std::iter::once([1, 1, 1])
            .chain(legacy.m_to_w[2..].iter().copied())
            .collect(),
        w_to_m: std::iter::once(vec![1])
            .chain(legacy.w_to_m[2..].iter().cloned())
            .collect(),
        n_w_to_m: std::iter::once(0)
            .chain(legacy.n_w_to_m[2..].iter().copied())
            .collect(),
    }
}

fn closed_fixture_mesh() -> earthmesh_cli::UnstructuredMesh {
    earthmesh_cli::UnstructuredMesh {
        m_points: vec![
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint { lon: 0.2, lat: 0.2 },
            earthmesh_cli::LonLatPoint { lon: 0.8, lat: 0.2 },
            earthmesh_cli::LonLatPoint { lon: 0.2, lat: 0.8 },
            earthmesh_cli::LonLatPoint { lon: 0.8, lat: 0.8 },
        ],
        w_points: vec![
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint { lon: 1.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 1.0 },
            earthmesh_cli::LonLatPoint { lon: 1.0, lat: 1.0 },
        ],
        m_to_w: vec![
            [1, 1, 1],
            [1, 1, 1],
            [2, 3, 4],
            [2, 3, 5],
            [2, 4, 5],
            [3, 4, 5],
        ],
        w_to_m: vec![
            vec![1],
            vec![1],
            vec![2, 3, 4],
            vec![2, 3, 5],
            vec![2, 4, 5],
            vec![3, 4, 5],
        ],
        n_w_to_m: vec![1, 1, 3, 3, 3, 3],
    }
}

#[test]
fn mpas_full_file_pipeline_reads_inputs_and_writes_mesh_plus_graph() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mpas_full_pipeline_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("result")).expect("create result root");
    let gridfile = root.join("result/gridfile_NXP0009_hex.nc4");
    let cellwidth_file = root.join("result/cellwidth_NXP0009_global.nc4");
    let mesh_output = root.join("result/MPASOUT_NXP0009_global.nc4");
    let graph_output = root.join("result/MPASOUT_NXP0009_global.graph.info");
    let mesh = closed_fixture_mesh();
    earthmesh_cli::write_unstructured_mesh_netcdf(&gridfile, &mesh).expect("write gridfile");
    earthmesh_cli::write_cellwidth_netcdf(
        &cellwidth_file,
        &earthmesh_cli::CellwidthMesh {
            cell_points: mesh.w_points.clone(),
            cellwidth: vec![100.0; mesh.w_points.len()],
        },
    )
    .expect("write cellwidth");

    let report = earthmesh_cli::write_mpas_mesh_from_netcdf_inputs(
        &gridfile,
        &cellwidth_file,
        &mesh_output,
        &graph_output,
        9,
        3,
    )
    .expect("write full MPAS from file inputs");

    assert_eq!(report.mesh.output, mesh_output);
    assert_eq!(report.graph_info.output, graph_output);
    assert!(report.graph_info.n_cells_written > 0);
    assert!(std::fs::read_to_string(&report.graph_info.output)
        .expect("read graph")
        .starts_with("         "));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn mpas_full_builder_nominal_min_dc_uses_fortran_integer_nxp_division() {
    let mesh = closed_fixture_mesh();
    let cellwidth = vec![100.0; mesh.w_points.len()];

    let mpas =
        earthmesh_cli::build_mpas_mesh_from_unstructured_fortran_indexed(&mesh, &cellwidth, 112, 4)
            .expect("build MPAS with non-divisible NXP");

    let expected =
        (7680 / 112 / 2_usize.pow(3)) as f64 / earthmesh_core::EARTH_RADIUS_METERS * 1000.0;
    assert_eq!(mpas.nominal_min_dc, expected);
}
