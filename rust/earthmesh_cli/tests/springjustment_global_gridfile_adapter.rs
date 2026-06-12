#[test]
fn springjustment_global_gridfile_adapter_reads_mesh_writes_persistence_and_returns_updated_mesh() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_spring_global_gridfile_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("gridfile")).expect("create gridfile root");
    let gridfile = root.join("gridfile/gridfile_NXP0009_03_hex.nc4");
    let mesh = spring_fixture_mesh();
    earthmesh_cli::write_unstructured_mesh_netcdf(&gridfile, &mesh).expect("write gridfile");

    let report = earthmesh_cli::run_springjustment_global_from_unstructured_gridfile(
        &gridfile,
        &root,
        9,
        3,
        earthmesh_cli::SpringjustmentGlobalRunOptions {
            base_dists_on_edge: 100.0,
            base_cellwidth: Some(200.0),
            distance_num_rc: 0,
            distance_spacing: earthmesh_mesh::DistanceLayerSpacing::Linear,
            distance_steps: &[],
            niter_refine: 0,
            relax: 0.25,
            radius: 1.0,
            diagnostic_every: 100,
        },
    )
    .expect("run springjustment global gridfile adapter");

    assert_eq!(report.mesh.m_to_w, mesh.m_to_w);
    assert_eq!(report.mesh.w_to_m, mesh.w_to_m);
    assert_eq!(report.mesh.n_w_to_m, mesh.n_w_to_m);
    assert_eq!(report.mesh.m_points.len(), mesh.m_points.len());
    assert_eq!(report.mesh.w_points.len(), mesh.w_points.len());
    assert_eq!(
        report.persistence.dists_on_edge.output,
        root.join("result/distsOnEdge_NXP0009_03_global.nc4")
    );
    assert_eq!(
        report
            .persistence
            .cellwidth
            .as_ref()
            .expect("cellwidth")
            .output,
        root.join("result/cellwidth_NXP0009_global.nc4")
    );

    let cellwidth =
        netcdf::open(root.join("result/cellwidth_NXP0009_global.nc4")).expect("open cellwidth");
    assert_eq!(
        read_f64(&cellwidth, "cellwidth"),
        vec![200.0; mesh.w_points.len()]
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn springjustment_global_gridfile_adapter_rejects_invalid_connectivity_before_writing() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_spring_global_gridfile_bad_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create root");
    let mut mesh = spring_fixture_mesh();
    mesh.m_to_w[2] = [1, 2, 99];
    let gridfile = root.join("bad_gridfile.nc4");
    earthmesh_cli::write_unstructured_mesh_netcdf(&gridfile, &mesh).expect("write gridfile");

    let err = earthmesh_cli::run_springjustment_global_from_unstructured_gridfile(
        &gridfile,
        &root,
        9,
        3,
        earthmesh_cli::SpringjustmentGlobalRunOptions {
            base_dists_on_edge: 100.0,
            base_cellwidth: None,
            distance_num_rc: 0,
            distance_spacing: earthmesh_mesh::DistanceLayerSpacing::Linear,
            distance_steps: &[],
            niter_refine: 0,
            relax: 0.25,
            radius: 1.0,
            diagnostic_every: 100,
        },
    )
    .expect_err("invalid gridfile connectivity rejected");

    assert!(err
        .to_string()
        .contains("m_to_w row 2 references cell id 99"));
    assert!(!root.join("result").exists());

    let _ = std::fs::remove_dir_all(&root);
}

fn spring_fixture_mesh() -> earthmesh_cli::UnstructuredMesh {
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

fn read_f64(file: &netcdf::File, name: &str) -> Vec<f64> {
    file.variable(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
        .get_values::<f64, _>(..)
        .unwrap_or_else(|err| panic!("read {name}: {err}"))
}
