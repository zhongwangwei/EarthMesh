#[test]
fn springjustment_regional_gridfile_adapter_returns_updated_mesh_and_writes_gridfile() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_spring_regional_gridfile_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("gridfile")).expect("create gridfile root");
    let input = root.join("gridfile/gridfile_NXP0009_03_hex.nc4");
    let output = root.join("gridfile/gridfile_NXP0009_04_hex.nc4");
    let mesh = spring_fixture_mesh();
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf(&input, &mesh)
        .expect("write gridfile");
    let move_mask = vec![false, false, true, false, false, false];

    let report = earthmesh_cli::grid_quality_pipeline::run_springjustment_regional_from_unstructured_gridfile(
        &input,
        earthmesh_cli::springjustment_gridfile_types::SpringjustmentRegionalRunOptions {
            move_mask: &move_mask,
            niter_refine: 0,
            radius: 1.0,
            diagnostic_every: 100,
        },
    )
    .expect("run springjustment regional gridfile adapter");

    assert_eq!(report.mesh.m_to_w, mesh.m_to_w);
    assert_eq!(report.mesh.w_to_m, mesh.w_to_m);
    assert_eq!(report.mesh.n_w_to_m, mesh.n_w_to_m);
    assert_eq!(report.mesh.m_points.len(), mesh.m_points.len());
    assert_eq!(report.mesh.w_points.len(), mesh.w_points.len());
    assert!(report.core.cells_on_edge.len() > 1);

    let write_report =
        earthmesh_cli::grid_quality_pipeline::write_springjustment_regional_gridfile(
            &output, &report,
        )
        .expect("write regional gridfile");
    assert_eq!(write_report.output, output);
    let round_trip =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&write_report.output)
            .expect("read regional gridfile");
    assert_eq!(round_trip.m_to_w, mesh.m_to_w);
    assert_eq!(round_trip.w_to_m, mesh.w_to_m);
    assert_eq!(round_trip.n_w_to_m, mesh.n_w_to_m);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn springjustment_regional_gridfile_adapter_rejects_bad_move_mask_length() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_spring_regional_gridfile_bad_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create root");
    let input = root.join("gridfile.nc4");
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf(
        &input,
        &spring_fixture_mesh(),
    )
    .expect("write gridfile");

    let err = earthmesh_cli::grid_quality_pipeline::run_springjustment_regional_from_unstructured_gridfile(
        &input,
        earthmesh_cli::springjustment_gridfile_types::SpringjustmentRegionalRunOptions {
            move_mask: &[false, true],
            niter_refine: 0,
            radius: 1.0,
            diagnostic_every: 100,
        },
    )
    .expect_err("bad move mask length rejected");

    assert!(err
        .to_string()
        .contains("failed to run Springjustment_regional_step core"));

    let _ = std::fs::remove_dir_all(&root);
}

fn spring_fixture_mesh() -> earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
    earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
        m_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.2, lat: 0.2 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.8, lat: 0.2 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.2, lat: 0.8 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.8, lat: 0.8 },
        ],
        w_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 1.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 1.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 1.0, lat: 1.0 },
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
