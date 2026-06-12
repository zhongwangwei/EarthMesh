use std::fs;

fn sample_ocean_source_mesh() -> earthmesh_cli::UnstructuredMesh {
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

fn mkgrd_ocean_config(base_dir: &str) -> earthmesh_core::EarthmeshConfig {
    earthmesh_core::EarthmeshConfig::from_mkgrd_namelist(&format!(
        "&mkgrd\n  NL%EXPNME='case_refine_final'\n  NL%base_dir='{base_dir}'\n  NL%NXP=9\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%output_format='FVCOM'\n  NL%refine=.true.\n/\n"
    ))
    .expect("parse mkgrd ocean config")
}

fn refine_config() -> earthmesh_core::RefineConfig {
    earthmesh_core::RefineConfig::from_mkrefine_namelist(
        "&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n/\n",
        "oceanmesh",
        "tri",
    )
    .expect("parse refine config")
}

#[test]
fn final_domain_handoff_copies_gridfile_and_runs_ocean_mask_postproc() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mkgrd_refine_final_handoff_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let base_dir = format!("{}/", root.display());
    let mkgrd = mkgrd_ocean_config(&base_dir);
    let refine = refine_config();
    let plan =
        earthmesh_cli::plan_mkgrd_refine_loop_io(&mkgrd, &refine).expect("plan refine loop io");

    fs::create_dir_all(plan.final_domain_gridfile.parent().unwrap()).expect("create gridfile dir");
    fs::create_dir_all(plan.final_domain_contain_output.parent().unwrap())
        .expect("create contain dir");
    earthmesh_cli::write_unstructured_mesh_netcdf(
        &plan.final_domain_gridfile,
        &sample_ocean_source_mesh(),
    )
    .expect("write final domain gridfile");
    earthmesh_cli::write_contain_netcdf(
        &plan.final_domain_contain_output,
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
    .expect("write final contain domain");

    let report = earthmesh_cli::run_mkgrd_refine_loop_final_domain_handoff(
        &plan,
        Some(earthmesh_cli::MkgrdFinalDomainPostprocOptions::Ocean(
            earthmesh_cli::MaskPostprocOceanRunOptions {
                mask_sea_ratio: 0.5,
                num_vertex: 1,
            },
        )),
    )
    .expect("run final handoff");

    assert_eq!(report.copied_result_gridfile, plan.final_result_gridfile);
    assert!(plan.final_result_gridfile.exists());
    let postproc = match report.postproc.expect("postproc report") {
        earthmesh_cli::MkgrdFinalDomainPostprocReport::Ocean(report) => report,
        other => panic!("expected ocean report, got {other:?}"),
    };
    let domain_plan = plan.final_mask_postproc_domain.as_ref().unwrap();
    assert_eq!(postproc.final_gridfile.output, domain_plan.result_gridfile);
    assert!(domain_plan.result_gridfile.exists());
    assert!(domain_plan.obc_output.as_ref().unwrap().exists());
    assert!(domain_plan.obcv2_output.as_ref().unwrap().exists());

    let _ = fs::remove_dir_all(&root);
}
