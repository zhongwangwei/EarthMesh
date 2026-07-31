use std::fs;

fn read_i32(file: &netcdf::File, name: &str) -> Vec<i32> {
    file.variable(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
        .get_values::<i32, _>(..)
        .unwrap_or_else(|err| panic!("read {name}: {err}"))
}

fn sample_ocean_source_mesh() -> earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
    let mut m_points = vec![earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 }; 8];
    for (idx, point) in m_points.iter_mut().enumerate() {
        point.lon = idx as f64;
        point.lat = idx as f64 * 0.5;
    }
    let mut w_points =
        vec![earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 }; 15];
    for (idx, point) in w_points.iter_mut().enumerate() {
        point.lon = 100.0 + idx as f64;
        point.lat = 40.0 + idx as f64 * 0.25;
    }

    let mut m_to_w = vec![[1, 1, 1]; 8];
    m_to_w[2] = [10, 11, 14];
    m_to_w[3] = [11, 12, 14];
    m_to_w[4] = [12, 13, 14];
    m_to_w[5] = [13, 10, 14];

    let mut w_to_m = vec![vec![1; 7]; 15];
    w_to_m[10] = vec![2, 5, 6, 7, 1, 1, 1];
    w_to_m[11] = vec![2, 3, 6, 7, 1, 1, 1];
    w_to_m[12] = vec![3, 4, 6, 7, 1, 1, 1];
    w_to_m[13] = vec![4, 5, 6, 7, 1, 1, 1];
    w_to_m[14] = vec![2, 3, 4, 5, 1, 1, 1];
    let mut n_w_to_m = vec![0; 15];
    n_w_to_m[10] = 5;
    n_w_to_m[11] = 5;
    n_w_to_m[12] = 5;
    n_w_to_m[13] = 5;
    n_w_to_m[14] = 4;

    earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
        m_points,
        w_points,
        m_to_w,
        w_to_m,
        n_w_to_m,
    }
}

fn isolated_ocean_source_mesh() -> earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
    let point = earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 };
    earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
        m_points: vec![point; 5],
        w_points: vec![point; 5],
        m_to_w: vec![[1, 1, 1], [1, 1, 1], [2, 3, 4], [2, 3, 4], [2, 3, 4]],
        w_to_m: vec![
            vec![1, 1, 1],
            vec![1, 1, 1],
            vec![2, 3, 4],
            vec![2, 3, 4],
            vec![2, 3, 4],
        ],
        n_w_to_m: vec![0, 0, 3, 3, 3],
    }
}

#[test]
fn ocean_runner_reads_inputs_writes_final_gridfile_and_tri_boundaries() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mask_postproc_ocean_runner_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("result")).expect("create result dir");
    fs::create_dir_all(root.join("contain")).expect("create contain dir");

    let plan = earthmesh_cli::mask_postproc_domain::plan_mask_postproc_domain_io(
        &root,
        9,
        "tri",
        "oceanmesh",
        false,
    )
    .expect("ocean plan");
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf_with_refine_levels(
        &plan.source_gridfile,
        &sample_ocean_source_mesh(),
        Some(&[0, 1, 2, 3, 4, 5, 6, 7]),
        Some(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]),
    )
    .expect("write source mesh");
    let contain = earthmesh_cli::contain_io::ContainMesh {
        ustr_id: vec![
            vec![0, 0, 1],
            vec![0, 0, 1],
            vec![1, 0, 1],
            vec![1, 0, 1],
            vec![1, 0, 1],
            vec![1, 0, 1],
            vec![0, 0, 1],
            vec![0, 0, 1],
        ],
        ustr_ii: vec![vec![0, 0, 0]],
        is_in_area_ustr: vec![0, -1, 1, 1, 1, 1, -1, -1],
    };
    earthmesh_cli::contain_io::write_contain_netcdf(&plan.contain_domain, &contain)
        .expect("write contain domain");

    let report = earthmesh_cli::mask_postproc_domain::run_mask_postproc_ocean_domain(
        &plan,
        earthmesh_cli::mask_postproc_types::MaskPostprocOceanRunOptions {
            mask_sea_ratio: 0.5,
            num_vertex: 1,
        },
    )
    .expect("run ocean mask_postproc domain");

    assert_eq!(report.final_gridfile.output, plan.result_gridfile);
    assert_eq!(report.final_gridfile.sjx_points, 6);
    assert_eq!(
        report.obc.as_ref().expect("obc report").output,
        plan.obc_output.clone().unwrap()
    );
    assert_eq!(
        report.obcv2.as_ref().expect("obcv2 report").output,
        plan.obcv2_output.clone().unwrap()
    );
    assert_eq!(report.renewal.renewed.points_next, 5);
    assert_eq!(report.finalization.vertex_reindex.vertex_mapping[10], 2);

    let final_mesh = earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(
        &report.final_gridfile.output,
    )
    .expect("read final gridfile");
    assert_eq!(final_mesh.m_to_w[2], [2, 3, 6]);
    assert_eq!(final_mesh.m_to_w[5], [5, 2, 6]);
    let final_points = earthmesh_cli::grid_quality_pipeline::read_gridfile_mesh_points(
        &report.final_gridfile.output,
    )
    .expect("read final points");
    assert_eq!(final_points.m_refine_level.len(), final_mesh.m_points.len());
    assert_eq!(final_points.w_refine_level.len(), final_mesh.w_points.len());
    assert!(final_points.m_refine_level.contains(&5));
    assert!(final_points.w_refine_level.contains(&13));

    let obc_file = netcdf::open(report.obc.unwrap().output).expect("open obc");
    assert_eq!(read_i32(&obc_file, "bdy_order"), vec![1, 2, 3, 4, 5]);
    assert_eq!(read_i32(&obc_file, "obc_order"), vec![1, 1, 1, 1, 1]);
    assert_eq!(read_i32(&obc_file, "ibc_order"), vec![1, 2, 3, 4, 5]);

    let obcv2_file = netcdf::open(report.obcv2.unwrap().output).expect("open obcv2");
    assert_eq!(read_i32(&obcv2_file, "n_close_curve"), vec![4]);
    assert_eq!(
        read_i32(&obcv2_file, "close_curve"),
        vec![10, 11, 12, 13, 1]
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn ocean_runner_rejects_non_ocean_plan_before_writing_outputs() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mask_postproc_ocean_runner_reject_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let plan = earthmesh_cli::mask_postproc_domain::plan_mask_postproc_domain_io(
        &root, 9, "tri", "landmesh", false,
    )
    .expect("land plan");

    let err = earthmesh_cli::mask_postproc_domain::run_mask_postproc_ocean_domain(
        &plan,
        earthmesh_cli::mask_postproc_types::MaskPostprocOceanRunOptions {
            mask_sea_ratio: 0.5,
            num_vertex: 1,
        },
    )
    .expect_err("non-ocean plan rejected");

    assert!(err.to_string().contains("oceanmesh"));
    assert!(!plan.result_gridfile.exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn ocean_runner_does_not_promote_orphan_source_demand() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mask_postproc_ocean_runner_hard_demand_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("result")).expect("create result dir");
    fs::create_dir_all(root.join("contain")).expect("create contain dir");

    let plan = earthmesh_cli::mask_postproc_domain::plan_mask_postproc_domain_io(
        &root,
        9,
        "tri",
        "oceanmesh",
        false,
    )
    .expect("ocean plan");
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf_with_refine_levels(
        &plan.source_gridfile,
        &isolated_ocean_source_mesh(),
        Some(&[0, 0, 2, 0, 0]),
        Some(&[0; 5]),
    )
    .expect("write source mesh");
    earthmesh_cli::contain_io::write_contain_netcdf(
        &plan.contain_domain,
        &earthmesh_cli::contain_io::ContainMesh {
            ustr_id: vec![vec![0, 0, 1]; 5],
            ustr_ii: vec![vec![0, 0, 0]],
            is_in_area_ustr: vec![0, -1, 1, -1, -1],
        },
    )
    .expect("write contain domain");

    let err = earthmesh_cli::mask_postproc_domain::run_mask_postproc_ocean_domain_with_hard_demand(
        &plan,
        earthmesh_cli::mask_postproc_types::MaskPostprocOceanRunOptions {
            mask_sea_ratio: 0.5,
            num_vertex: 1,
        },
        &[true, false, false],
    )
    .expect_err("an orphan-only ocean product must remain empty");

    let failure = earthmesh_cli::masked_topology_cleanup::domain_topology_failure(&err)
        .expect("typed domain-topology failure");
    assert_eq!(
        failure.kind(),
        earthmesh_cli::masked_topology_cleanup::DomainTopologyFailureKind::NoRetainedCells
    );
    assert_eq!(failure.center_id(), None);
    assert!(!plan.result_gridfile.exists());

    let _ = fs::remove_dir_all(&root);
}
