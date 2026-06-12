use std::fs;

#[test]
fn mask_restart_ocean_without_patch_plans_remask_postproc_without_gridinit() {
    let root =
        std::env::temp_dir().join(format!("earthmesh_cli_mask_restart_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");

    let namelist = root.join("mkgrd_restart.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%output_format='FVCOM'\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.false.\n/\n"
        ),
    )
    .expect("write namelist");

    let report =
        earthmesh_cli::plan_mkgrd_mask_restart_namelist(&namelist, &root, 7).expect("restart plan");

    assert!(report.config.mask_restart);
    assert_eq!(report.remask.mesh_type, "oceanmesh");
    assert_eq!(report.remask.file_dir, root.join("case_restart/"));
    assert_eq!(report.remask.step, 8);
    assert!(!report.remask.refine);
    assert_eq!(
        report.remask.action,
        earthmesh_cli::MaskRestartAction::RunMaskPostproc
    );
    assert!(!report.workspace_plan.remove_existing_file_dir);
    assert!(report.workspace_plan.directories_to_create.is_empty());
    assert!(report.workspace_plan.mask_operations.is_empty());

    let _ = fs::remove_dir_all(&root);
}

fn restart_ocean_source_mesh() -> earthmesh_cli::UnstructuredMesh {
    let mut m_points = vec![earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 }; 8];
    for (idx, point) in m_points.iter_mut().enumerate() {
        point.lon = idx as f64;
        point.lat = idx as f64 * 0.5;
    }
    let mut w_points = vec![earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 }; 14];
    for (idx, point) in w_points.iter_mut().enumerate() {
        point.lon = 100.0 + idx as f64;
        point.lat = 40.0 + idx as f64 * 0.25;
    }
    let mut m_to_w = vec![[1, 1, 1]; 8];
    m_to_w[2] = [10, 11, 2];
    m_to_w[3] = [11, 12, 3];
    m_to_w[4] = [12, 13, 4];
    m_to_w[5] = [13, 10, 5];
    let mut w_to_m = vec![vec![1; 7]; 14];
    w_to_m[2] = vec![2, 1, 1, 1, 1, 1, 1];
    w_to_m[3] = vec![3, 1, 1, 1, 1, 1, 1];
    w_to_m[4] = vec![4, 1, 1, 1, 1, 1, 1];
    w_to_m[5] = vec![5, 1, 1, 1, 1, 1, 1];
    w_to_m[10] = vec![2, 5, 6, 7, 1, 1, 1];
    w_to_m[11] = vec![2, 3, 6, 7, 1, 1, 1];
    w_to_m[12] = vec![3, 4, 6, 7, 1, 1, 1];
    w_to_m[13] = vec![4, 5, 6, 7, 1, 1, 1];
    let mut n_w_to_m = vec![0; 14];
    n_w_to_m[2] = 1;
    n_w_to_m[3] = 1;
    n_w_to_m[4] = 1;
    n_w_to_m[5] = 1;
    n_w_to_m[10] = 5;
    n_w_to_m[11] = 5;
    n_w_to_m[12] = 5;
    n_w_to_m[13] = 5;
    earthmesh_cli::UnstructuredMesh {
        m_points,
        w_points,
        m_to_w,
        w_to_m,
        n_w_to_m,
    }
}

#[test]
fn run_mask_restart_ocean_namelist_executes_postproc_outputs() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mask_restart_run_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let case_dir = root.join("case_restart");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(case_dir.join("contain")).expect("create contain dir");

    let namelist = root.join("mkgrd_restart_run.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%output_format='FVCOM'\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.false.\n/\n"
        ),
    )
    .expect("write namelist");

    let io_plan =
        earthmesh_cli::plan_mask_postproc_domain_io(&case_dir, 16, "tri", "oceanmesh", false)
            .expect("postproc io plan");
    earthmesh_cli::write_unstructured_mesh_netcdf(
        &io_plan.source_gridfile,
        &restart_ocean_source_mesh(),
    )
    .expect("write source gridfile");
    earthmesh_cli::write_contain_netcdf(
        &io_plan.contain_domain,
        &earthmesh_cli::ContainMesh {
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
        },
    )
    .expect("write contain domain");

    let report = earthmesh_cli::run_mkgrd_mask_restart_ocean_namelist(
        &namelist,
        &root,
        7,
        earthmesh_cli::MaskPostprocOceanRunOptions {
            mask_sea_ratio: 0.5,
            num_vertex: 1,
        },
    )
    .expect("run mask_restart ocean postproc");

    assert_eq!(
        report.plan.remask.action,
        earthmesh_cli::MaskRestartAction::RunMaskPostproc
    );
    assert_eq!(
        report.postproc.final_gridfile.output,
        io_plan.result_gridfile
    );
    assert!(io_plan.result_gridfile.exists());
    assert!(io_plan.obc_output.unwrap().exists());
    assert!(io_plan.obcv2_output.unwrap().exists());

    let _ = fs::remove_dir_all(&root);
}
