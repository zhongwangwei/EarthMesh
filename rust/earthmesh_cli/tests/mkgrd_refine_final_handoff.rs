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

fn mkgrd_land_config(base_dir: &str) -> earthmesh_core::EarthmeshConfig {
    earthmesh_core::EarthmeshConfig::from_mkgrd_namelist(&format!(
        "&mkgrd\n  NL%EXPNME='case_refine_final_land'\n  NL%base_dir='{base_dir}'\n  NL%NXP=4\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%refine=.true.\n/\n"
    ))
    .expect("parse mkgrd land config")
}

fn mkgrd_atmos_config(base_dir: &str) -> earthmesh_core::EarthmeshConfig {
    earthmesh_core::EarthmeshConfig::from_mkgrd_namelist(&format!(
        "&mkgrd\n  NL%EXPNME='case_refine_final_atmos'\n  NL%base_dir='{base_dir}'\n  NL%NXP=9\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='tri'\n  NL%output_format='MPAS-Simple'\n  NL%refine=.true.\n/\n"
    ))
    .expect("parse mkgrd atmos config")
}

fn mkgrd_atmos_full_mpas_config(base_dir: &str) -> earthmesh_core::EarthmeshConfig {
    earthmesh_core::EarthmeshConfig::from_mkgrd_namelist(&format!(
        "&mkgrd\n  NL%EXPNME='case_refine_final_atmos_mpas'\n  NL%base_dir='{base_dir}'\n  NL%NXP=9\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%output_format='MPAS'\n  NL%refine=.true.\n/\n"
    ))
    .expect("parse mkgrd atmos full MPAS config")
}

fn refine_config() -> earthmesh_core::RefineConfig {
    earthmesh_core::RefineConfig::from_mkrefine_namelist(
        "&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=4,4,3\n  RL%max_transition_row=4,4,3\n  RL%mask_refine_spc_type='bbox'\n/\n",
        "oceanmesh",
        "tri",
    )
    .expect("parse refine config")
}

fn sample_atmos_simple_source_mesh() -> earthmesh_cli::UnstructuredMesh {
    earthmesh_cli::UnstructuredMesh {
        m_points: vec![
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint {
                lon: 90.0,
                lat: 0.0,
            },
        ],
        w_points: vec![
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint {
                lon: 180.0,
                lat: 0.0,
            },
        ],
        m_to_w: vec![[1, 1, 1], [1, 2, 1], [2, 1, 2]],
        w_to_m: vec![vec![1], vec![1, 2], vec![2, 1]],
        n_w_to_m: vec![1, 2, 2],
    }
}

fn sample_atmos_full_mpas_source_mesh() -> earthmesh_cli::UnstructuredMesh {
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

#[test]
fn final_domain_handoff_can_generate_getcontain_domain_before_postproc() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mkgrd_refine_final_handoff_contain_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let base_dir = format!("{}/", root.display());
    let mkgrd = mkgrd_land_config(&base_dir);
    let refine = refine_config();
    let plan =
        earthmesh_cli::plan_mkgrd_refine_loop_io(&mkgrd, &refine).expect("plan refine loop io");

    fs::create_dir_all(plan.final_domain_gridfile.parent().unwrap()).expect("create gridfile dir");
    earthmesh_cli::write_unstructured_mesh_netcdf(
        &plan.final_domain_gridfile,
        &earthmesh_cli::UnstructuredMesh {
            m_points: vec![earthmesh_cli::LonLatPoint { lon: 1.0, lat: 1.0 }],
            w_points: vec![
                earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
                earthmesh_cli::LonLatPoint { lon: 2.0, lat: 0.0 },
                earthmesh_cli::LonLatPoint { lon: 0.0, lat: 2.0 },
            ],
            m_to_w: vec![[1, 2, 3]],
            w_to_m: vec![vec![1], vec![1], vec![1]],
            n_w_to_m: vec![1, 1, 1],
        },
    )
    .expect("write final domain gridfile");

    let lon_i = vec![f64::NAN, 0.5, 1.5, 2.5];
    let lat_i = vec![f64::NAN, 1.5, 0.5];
    let lon_vertex = vec![f64::NAN, 0.0, 2.0, 3.0];
    let lat_vertex = vec![f64::NAN, 2.0, 0.0];
    let mut domain_grid = vec![vec![0; lat_i.len()]; lon_i.len()];
    domain_grid[1][1] = 1;
    domain_grid[1][2] = 1;
    domain_grid[2][1] = 1;
    domain_grid[2][2] = 1;
    let payload = earthmesh_cli::select_area_judge_grid_fortran_indexed(
        &domain_grid,
        None,
        &lon_i,
        &lat_i,
        earthmesh_mesh::AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: 2,
            maxlat_source: 1,
            minlat_source: 2,
        },
    )
    .expect("select final domain grid");
    let domain_area = plan.file_dir.join("result/IsInDmArea_grid.nc4");
    earthmesh_cli::write_area_judge_grid_netcdf(&domain_area, &payload)
        .expect("write final domain area grid");

    let mut seaorland = vec![vec![0; lat_i.len()]; lon_i.len()];
    seaorland[1][1] = 1;
    seaorland[1][2] = 1;
    seaorland[2][1] = 0;
    seaorland[2][2] = 1;

    let report = earthmesh_cli::run_mkgrd_refine_loop_final_domain_handoff_with_domain_contain(
        &plan,
        Some(earthmesh_cli::MkgrdFinalDomainContainOptions {
            area_grid_file: &domain_area,
            mesh_kind: earthmesh_cli::GetContainMeshKind::Land,
            seaorland: &seaorland,
            lon_vertex: &lon_vertex,
            lat_vertex: &lat_vertex,
            lon_i: &lon_i,
            lat_i: &lat_i,
            num_vertex: 0,
        }),
        None,
    )
    .expect("run final handoff with generated Get_Contain(0) domain");

    assert_eq!(report.copied_result_gridfile, plan.final_result_gridfile);
    assert!(plan.final_result_gridfile.exists());
    assert_eq!(report.contain_domain, plan.final_domain_contain_output);
    let contain = earthmesh_cli::read_contain_netcdf(&plan.final_domain_contain_output)
        .expect("read contain");
    assert_eq!(contain.is_in_area_ustr, vec![0, 1]);
    assert_eq!(contain.ustr_id, vec![vec![0, 0], vec![3, 1]]);
    assert_eq!(contain.ustr_ii, vec![vec![1, 1], vec![1, 2], vec![2, 2]]);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn final_domain_handoff_runs_atmos_mpas_simple_without_domain_postproc_plan() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mkgrd_refine_final_handoff_atmos_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let base_dir = format!("{}/", root.display());
    let mkgrd = mkgrd_atmos_config(&base_dir);
    let refine = refine_config();
    let plan =
        earthmesh_cli::plan_mkgrd_refine_loop_io(&mkgrd, &refine).expect("plan refine loop io");
    assert!(
        plan.final_mask_postproc_domain.is_none(),
        "atmosmesh uses the MPAS branch instead of a land/ocean domain postproc plan"
    );

    fs::create_dir_all(plan.final_domain_gridfile.parent().unwrap()).expect("create gridfile dir");
    fs::create_dir_all(root.join("case_refine_final_atmos/result")).expect("create result dir");
    let mesh = sample_atmos_simple_source_mesh();
    earthmesh_cli::write_unstructured_mesh_netcdf(&plan.final_domain_gridfile, &mesh)
        .expect("write final domain gridfile");
    earthmesh_cli::write_cellwidth_netcdf(
        root.join("case_refine_final_atmos/result/cellwidth_NXP0009_global.nc4"),
        &earthmesh_cli::CellwidthMesh {
            cell_points: mesh.w_points.clone(),
            cellwidth: vec![12.0, 24.0, 48.0],
        },
    )
    .expect("write cellwidth");

    let report = earthmesh_cli::run_mkgrd_refine_loop_final_domain_handoff(
        &plan,
        Some(earthmesh_cli::MkgrdFinalDomainPostprocOptions::Atmos {
            output_format: "MPAS-Simple",
        }),
    )
    .expect("run atmos final handoff");

    assert_eq!(report.copied_result_gridfile, plan.final_result_gridfile);
    assert!(plan.final_result_gridfile.exists());
    match report.postproc.expect("postproc report") {
        earthmesh_cli::MkgrdFinalDomainPostprocReport::Atmos(postproc) => {
            assert_eq!(
                postproc.output,
                root.join("case_refine_final_atmos/result/MPASOUT_NXP0009_global_Simple.nc4")
            );
            assert!(postproc.output.exists());
        }
        other => panic!("expected atmos MPAS-Simple report, got {other:?}"),
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn final_domain_handoff_runs_atmos_full_mpas_without_domain_postproc_plan() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mkgrd_refine_final_handoff_atmos_full_mpas_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let base_dir = format!("{}/", root.display());
    let mkgrd = mkgrd_atmos_full_mpas_config(&base_dir);
    let refine = refine_config();
    let plan =
        earthmesh_cli::plan_mkgrd_refine_loop_io(&mkgrd, &refine).expect("plan refine loop io");
    assert!(
        plan.final_mask_postproc_domain.is_none(),
        "atmosmesh full MPAS uses the MPAS branch instead of a land/ocean domain postproc plan"
    );

    fs::create_dir_all(plan.final_domain_gridfile.parent().unwrap()).expect("create gridfile dir");
    fs::create_dir_all(root.join("case_refine_final_atmos_mpas/result"))
        .expect("create result dir");
    let mesh = sample_atmos_full_mpas_source_mesh();
    earthmesh_cli::write_unstructured_mesh_netcdf(&plan.final_domain_gridfile, &mesh)
        .expect("write final domain gridfile");
    earthmesh_cli::write_cellwidth_netcdf(
        root.join("case_refine_final_atmos_mpas/result/cellwidth_NXP0009_global.nc4"),
        &earthmesh_cli::CellwidthMesh {
            cell_points: mesh.w_points.clone(),
            cellwidth: vec![100.0; mesh.w_points.len()],
        },
    )
    .expect("write cellwidth");

    let report = earthmesh_cli::run_mkgrd_refine_loop_final_domain_handoff(
        &plan,
        Some(earthmesh_cli::MkgrdFinalDomainPostprocOptions::Atmos {
            output_format: "MPAS",
        }),
    )
    .expect("run atmos full MPAS final handoff");

    assert_eq!(report.copied_result_gridfile, plan.final_result_gridfile);
    assert!(plan.final_result_gridfile.exists());
    match report.postproc.expect("postproc report") {
        earthmesh_cli::MkgrdFinalDomainPostprocReport::AtmosFull(postproc) => {
            assert_eq!(
                postproc.mesh.output,
                root.join("case_refine_final_atmos_mpas/result/MPASOUT_NXP0009_global.nc4")
            );
            assert_eq!(
                postproc.graph_info.output,
                root.join("case_refine_final_atmos_mpas/result/MPASOUT_NXP0009_global.graph.info")
            );
            assert!(postproc.mesh.output.exists());
            assert!(postproc.graph_info.output.exists());
        }
        other => panic!("expected atmos full MPAS report, got {other:?}"),
    }

    let _ = fs::remove_dir_all(&root);
}
