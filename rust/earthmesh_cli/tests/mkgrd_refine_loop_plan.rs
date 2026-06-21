use std::{fs, path::PathBuf};

use earthmesh_cli::{
    infer_mkgrd_effective_final_step_from_gridfiles, plan_mkgrd_refine_loop,
    plan_mkgrd_refine_loop_io, write_unstructured_mesh_netcdf, LonLatPoint, MkgrdRefineSource,
    UnstructuredMesh,
};
use earthmesh_core::{EarthmeshConfig, RefineConfig};

fn mixed_refine_config() -> RefineConfig {
    RefineConfig::from_mkrefine_namelist(
        "&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=2\n  RL%max_iter_cal=3\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_cal_type='bbox'\n  RL%refine_num_landtypes=.true.\n  RL%set_dis_type='linear'\n/\n",
        "landmesh",
        "hex",
    )
    .expect("parse mixed refine config")
}

fn mkgrd_config() -> EarthmeshConfig {
    EarthmeshConfig::from_mkgrd_namelist(
        "&mkgrd\n  NL%EXPNME='case_refine'\n  NL%base_dir='/tmp/earthmesh/'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  NL%refine=.true.\n/\n",
    )
    .expect("parse mkgrd config")
}

#[test]
fn refine_loop_plan_follows_mixed_calculated_and_specified_windows() {
    let config = mixed_refine_config();

    let plan = plan_mkgrd_refine_loop(&config).expect("plan refine loop");

    assert_eq!(plan.max_iter, 3);
    assert_eq!(plan.steps.len(), 3);
    assert_eq!(plan.steps[0].max_transition_row, 1);
    assert_eq!(plan.steps[1].max_transition_row, 1);
    assert_eq!(plan.steps[2].max_transition_row, 1);
    assert_eq!(
        plan.steps[0].sources,
        vec![
            MkgrdRefineSource::CalculatedIterZero,
            MkgrdRefineSource::SpecifiedStep
        ]
    );
    assert_eq!(
        plan.steps[1].sources,
        vec![
            MkgrdRefineSource::CalculatedIterZero,
            MkgrdRefineSource::SpecifiedStep
        ]
    );
    assert_eq!(
        plan.steps[2].sources,
        vec![MkgrdRefineSource::CalculatedIterZero]
    );
    assert_eq!(plan.final_mask_postproc_step, 4);
}

#[test]
fn refine_loop_plan_ignores_disabled_calculated_window_like_real_atmos_example() {
    let config = RefineConfig::from_mkrefine_namelist(
        "&mkrefine\n RL%Istransition=.true.\n RL%SpringGlobal_type=1\n RL%SpringRegional_type=0\n RL%refine_spc=.true.\n RL%max_iter_spc=2\n RL%refine_cal=.false.\n RL%max_iter_cal=4\n RL%HALO=4,4,3\n RL%max_transition_row=4,4,3\n/\n",
        "atmosmesh",
        "hex",
    )
    .expect("parse real atmos refine controls");

    let plan = plan_mkgrd_refine_loop(&config)
        .expect("disabled calculated window should not require unused halo entries");

    assert_eq!(config.halo[..4], [0, 4, 4, 3]);
    assert_eq!(config.max_transition_row[..4], [0, 4, 4, 3]);
    assert_eq!(plan.max_iter, 2);
    assert_eq!(plan.steps.len(), 2);
    assert_eq!(
        plan.steps[0].sources,
        vec![MkgrdRefineSource::SpecifiedStep]
    );
    assert_eq!(
        plan.steps[1].sources,
        vec![MkgrdRefineSource::SpecifiedStep]
    );
}

#[test]
fn refine_loop_plan_accepts_three_fortran_prefix_halo_values_like_delta_cases() {
    let config = RefineConfig::from_mkrefine_namelist(
        "&mkrefine\n RL%Istransition=.true.\n RL%SpringGlobal_type=1\n RL%SpringRegional_type=0\n RL%refine_spc=.true.\n RL%max_iter_spc=3\n RL%refine_cal=.false.\n RL%max_iter_cal=4\n RL%HALO=4,4,3\n RL%max_transition_row=4,4,3\n/\n",
        "atmosmesh",
        "hex",
    )
    .expect("parse real delta refine controls");

    let plan =
        plan_mkgrd_refine_loop(&config).expect("three specified refine steps should use halo(1:3)");

    assert_eq!(config.halo[..4], [0, 4, 4, 3]);
    assert_eq!(plan.max_iter, 3);
    assert_eq!(plan.steps.len(), 3);
}

#[test]
fn refine_loop_plan_rejects_non_positive_max_iter_like_fortran() {
    let mut config = mixed_refine_config();
    config.max_iter_spc = 0;
    config.max_iter_cal = 0;

    let err = plan_mkgrd_refine_loop(&config).expect_err("max_iter must be positive");

    assert!(err.to_string().contains("max_iter must be more than zero"));
}

#[test]
fn refine_loop_plan_models_dynamic_all_steps_exit_before_incrementing_step() {
    let mut config = mixed_refine_config();
    config.exit_loop_step[1] = true;
    config.exit_loop_step[2] = true;
    config.exit_loop_step[3] = true;

    let plan = plan_mkgrd_refine_loop(&config).expect("plan refine loop exits");

    assert!(!plan.steps[0].stop_after_step);
    assert!(!plan.steps[1].stop_after_step);
    assert!(plan.steps[2].stop_after_step);
    assert_eq!(plan.final_mask_postproc_step, 3);
}

#[test]
fn refine_loop_io_plan_maps_fortran_step_files_and_final_postproc_inputs() {
    let mkgrd = mkgrd_config();
    let refine = mixed_refine_config();

    let plan = plan_mkgrd_refine_loop_io(&mkgrd, &refine).expect("plan refine loop io");

    let root = PathBuf::from("/tmp/earthmesh/case_refine");
    assert_eq!(plan.file_dir, root);
    assert_eq!(plan.nxp, 16);
    assert_eq!(plan.max_iter, 3);
    assert_eq!(plan.final_mask_postproc_step, 4);
    assert_eq!(plan.final_get_contain_iter, 0);
    assert_eq!(
        plan.final_domain_contain_output,
        PathBuf::from("/tmp/earthmesh/case_refine/contain/contain_landmesh_domain_NXP0016_hex.nc4")
    );
    assert_eq!(
        plan.final_result_gridfile,
        PathBuf::from("/tmp/earthmesh/case_refine/result/gridfile_NXP0016_hex.nc4")
    );
    assert!(!plan.final_quality_check.run_quality_check);
    assert_eq!(plan.final_quality_check.step, 4);
    let postproc = plan
        .final_mask_postproc_domain
        .as_ref()
        .expect("landmesh uses domain mask_postproc");
    assert_eq!(postproc.source_gridfile, plan.final_result_gridfile);
    assert_eq!(postproc.contain_domain, plan.final_domain_contain_output);
    assert_eq!(
        postproc.result_gridfile,
        PathBuf::from("/tmp/earthmesh/case_refine/result/gridfile_NXP0016_hex_landmesh.nc4")
    );

    let step1 = &plan.steps[0];
    assert_eq!(step1.step, 1);
    assert_eq!(step1.max_transition_row, 1);
    assert_eq!(
        step1.refine_loop_input_gridfile,
        root.join("gridfile/gridfile_NXP0016_01_hex.nc4")
    );
    assert_eq!(
        step1.refine_loop_original_tmpfile,
        root.join("tmpfile/gridfile_NXP0016_01_ori.nc4")
    );
    assert_eq!(
        step1.refine_loop_stage2_tmpfile,
        root.join("tmpfile/gridfile_NXP0016_01_2.nc4")
    );
    assert_eq!(
        step1.refine_loop_stage5_tmpfile,
        root.join("tmpfile/gridfile_NXP0016_01_5.nc4")
    );
    assert_eq!(
        step1.refine_loop_output_gridfile,
        root.join("gridfile/gridfile_NXP0016_02_hex.nc4")
    );

    let calculated = &step1.sources[0];
    assert_eq!(calculated.source, MkgrdRefineSource::CalculatedIterZero);
    assert_eq!(calculated.area_judge_iter, 0);
    assert_eq!(calculated.get_contain_iter, 0);
    assert_eq!(calculated.getref_iter, 0);
    assert_eq!(
        calculated.contain_output,
        root.join("contain/contain_landmesh_refine_cal_NXP0016_01_tri.nc4")
    );
    assert_eq!(
        calculated.threshold_outputs,
        vec![root.join("threshold/threshold_calculate_land_NXP0016_01.nc4")]
    );
    assert_eq!(calculated.specified_threshold_output, None);

    let specified = &step1.sources[1];
    assert_eq!(specified.source, MkgrdRefineSource::SpecifiedStep);
    assert_eq!(specified.area_judge_iter, 1);
    assert_eq!(specified.get_contain_iter, 1);
    assert_eq!(specified.getref_iter, 1);
    assert_eq!(
        specified.contain_output,
        root.join("contain/contain_landmesh_refine_spc_NXP0016_01_tri.nc4")
    );
    assert_eq!(specified.threshold_outputs, Vec::<PathBuf>::new());
    assert_eq!(
        specified.specified_threshold_output,
        Some(root.join("threshold/threshold_specified_NXP0016_01.nc4"))
    );
}

#[test]
fn refine_loop_io_plan_uses_early_exit_step_for_final_domain_postproc() {
    let mkgrd = mkgrd_config();
    let mut refine = mixed_refine_config();
    refine.exit_loop_step[1] = true;
    refine.exit_loop_step[2] = true;
    refine.exit_loop_step[3] = true;

    let plan = plan_mkgrd_refine_loop_io(&mkgrd, &refine).expect("plan refine loop io");

    assert_eq!(plan.steps.len(), 3);
    assert!(plan.steps[2].stop_after_step);
    assert_eq!(plan.final_mask_postproc_step, 3);
    assert_eq!(
        plan.final_domain_gridfile,
        PathBuf::from("/tmp/earthmesh/case_refine/gridfile/gridfile_NXP0016_03_hex.nc4")
    );
    assert_eq!(plan.final_quality_check.step, 3);
}

#[test]
fn refine_loop_io_plan_orders_locmesh_calculated_threshold_outputs_like_fortran_getref() {
    let mkgrd = EarthmeshConfig::from_mkgrd_namelist(
        "&mkgrd\n  NL%EXPNME='case_loc_refine'\n  NL%base_dir='/tmp/earthmesh/'\n  NL%NXP=16\n  NL%mesh_type='LOCmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  NL%refine=.true.\n/\n",
    )
    .expect("parse LOCmesh mkgrd config");
    let refine = RefineConfig::from_mkrefine_namelist(
        "&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=3\n  RL%max_transition_row=1\n  RL%mask_refine_cal_type='bbox'\n  RL%refine_num_landtypes=.true.\n  RL%set_dis_type='linear'\n/\n",
        "LOCmesh",
        "hex",
    )
    .expect("parse LOCmesh calculated refine config");

    let plan = plan_mkgrd_refine_loop_io(&mkgrd, &refine).expect("plan LOCmesh refine loop io");
    let source = &plan.steps[0].sources[0];

    assert_eq!(source.source, MkgrdRefineSource::CalculatedIterZero);
    assert_eq!(
        source.threshold_outputs,
        vec![
            PathBuf::from(
                "/tmp/earthmesh/case_loc_refine/threshold/threshold_calculate_land_NXP0016_01.nc4"
            ),
            PathBuf::from(
                "/tmp/earthmesh/case_loc_refine/threshold/threshold_calculate_ocean_NXP0016_01.nc4"
            ),
            PathBuf::from(
                "/tmp/earthmesh/case_loc_refine/threshold/threshold_calculate_atmos_NXP0016_01.nc4"
            ),
        ],
        "Fortran LOCmesh calculated GetRef handoff uses land, ocean, atmosphere threshold outputs in that order"
    );
}

#[test]
fn refine_loop_plan_rejects_invalid_halo_transition_controls_like_fortran() {
    let mut too_small_halo = mixed_refine_config();
    too_small_halo.halo[1] = 1;
    too_small_halo.max_transition_row[1] = 2;
    let err = plan_mkgrd_refine_loop(&too_small_halo).expect_err("halo must cover transition rows");
    assert!(err
        .to_string()
        .contains("halo(1) must be larger than or equal to max_transition_row(1)"));

    let mut non_positive_halo = mixed_refine_config();
    non_positive_halo.halo[2] = 0;
    non_positive_halo.max_transition_row[2] = -1;
    let err = plan_mkgrd_refine_loop(&non_positive_halo).expect_err("halo must be positive");
    assert!(err.to_string().contains("halo(2) must be more than zero"));

    let mut non_positive_transition = mixed_refine_config();
    non_positive_transition.halo[3] = 1;
    non_positive_transition.max_transition_row[3] = 0;
    let err = plan_mkgrd_refine_loop(&non_positive_transition)
        .expect_err("transition rows must be positive");
    assert!(err
        .to_string()
        .contains("max_transition_row(3) must be more than zero"));
}

#[test]
fn final_quality_check_io_plan_matches_global_spring_paths() {
    let mkgrd = mkgrd_config();
    let mut refine = mixed_refine_config();
    refine.spring_global_type = 1;
    refine.spring_regional_type = 0;

    let plan = earthmesh_cli::plan_mkgrd_final_quality_check_io(&mkgrd, &refine, 4)
        .expect("plan final quality check");

    let root = PathBuf::from("/tmp/earthmesh/case_refine");
    assert!(plan.run_quality_check);
    assert_eq!(
        plan.spring_mode,
        earthmesh_cli::MkgrdFinalQualitySpringMode::Global
    );
    assert_eq!(plan.step, 4);
    assert_eq!(
        plan.input_gridfile,
        root.join("gridfile/gridfile_NXP0016_04_hex.nc4")
    );
    assert_eq!(
        plan.original_gridfile,
        Some(root.join("gridfile/gridfile_NXP0016_04_hex_orial.nc4"))
    );
    assert_eq!(
        plan.quality_before_spring,
        Some(root.join("result/quality_NXP0016_04_global_beforeSpring.nc4"))
    );
    assert_eq!(
        plan.quality_after_spring,
        Some(root.join("result/quality_NXP0016_04_global.nc4"))
    );
    assert_eq!(
        plan.output_gridfile,
        Some(root.join("gridfile/gridfile_NXP0016_04_hex.nc4"))
    );
    assert_eq!(plan.regional_set_dis, None);
}

#[test]
fn effective_final_step_uses_planned_gridfile_when_it_exists_even_if_counts_match() {
    let root = temp_root("earthmesh_cli_effective_final_step");
    let mut mkgrd = mkgrd_config();
    mkgrd.base_dir = format!("{}/", root.display());
    let mut refine = mixed_refine_config();
    refine.refine_setting = "specified".to_string();
    refine.refine_spc = true;
    refine.refine_cal = false;
    refine.max_iter_spc = 3;
    refine.max_iter_cal = 0;
    refine.spring_global_type = 1;
    refine.spring_regional_type = 0;
    let plan = plan_mkgrd_refine_loop_io(&mkgrd, &refine).expect("plan refine loop io");

    write_unstructured_mesh_netcdf(
        root.join("case_refine/gridfile/gridfile_NXP0016_03_hex.nc4"),
        &small_mesh(2),
    )
    .expect("write step 3 gridfile");
    write_unstructured_mesh_netcdf(
        root.join("case_refine/gridfile/gridfile_NXP0016_04_hex.nc4"),
        &small_mesh(2),
    )
    .expect("write unchanged step 4 gridfile");

    let effective =
        infer_mkgrd_effective_final_step_from_gridfiles(&plan).expect("infer effective step");

    assert_eq!(effective, 4);
}

#[test]
fn effective_final_step_uses_previous_gridfile_when_planned_noop_output_is_absent() {
    let root = temp_root("earthmesh_cli_effective_final_step_missing_planned");
    let mut mkgrd = mkgrd_config();
    mkgrd.base_dir = format!("{}/", root.display());
    let mut refine = mixed_refine_config();
    refine.refine_setting = "specified".to_string();
    refine.refine_spc = true;
    refine.refine_cal = false;
    refine.max_iter_spc = 1;
    refine.max_iter_cal = 0;
    let plan = plan_mkgrd_refine_loop_io(&mkgrd, &refine).expect("plan refine loop io");

    write_unstructured_mesh_netcdf(
        root.join("case_refine/gridfile/gridfile_NXP0016_01_hex.nc4"),
        &small_mesh(2),
    )
    .expect("write step 1 gridfile");

    let effective =
        infer_mkgrd_effective_final_step_from_gridfiles(&plan).expect("infer effective step");

    assert_eq!(effective, 1);
}

#[test]
fn final_quality_global_spring_plan_uses_namelist_runtime_controls() {
    let mut mkgrd = mkgrd_config();
    mkgrd.mesh_type = "atmosmesh".to_string();
    mkgrd.output_format = "MPAS-Simple".to_string();
    mkgrd.beta = 2.4;
    mkgrd.relax = 0.125;
    let mut refine = mixed_refine_config();
    refine.spring_global_type = 1;
    refine.spring_regional_type = 0;
    refine.num_rc = 2;
    refine.set_dis_type = "nonlinear2".to_string();
    refine.niter_refine = 1234;

    let plan = earthmesh_cli::plan_mkgrd_final_quality_check_io(&mkgrd, &refine, 4)
        .expect("plan final quality global spring controls");
    let spring = plan.global_spring.expect("global spring controls");

    let expected_base =
        f64::from(mkgrd.beta) * std::f64::consts::PI * 2.0 * earthmesh_core::EARTH_RADIUS_METERS
            / (5.0 * f64::from(mkgrd.nxp));
    assert!((spring.base_dists_on_edge - expected_base).abs() < 1.0e-6);
    assert_eq!(spring.base_cellwidth, Some(7680.0 / f64::from(mkgrd.nxp)));
    assert_eq!(spring.distance_num_rc, 2);
    assert_eq!(
        spring.distance_spacing,
        earthmesh_mesh::DistanceLayerSpacing::Exponential
    );
    assert_eq!(spring.niter_refine, 1234);
    assert_eq!(spring.relax, f64::from(mkgrd.relax));
    assert_eq!(spring.radius, earthmesh_core::EARTH_RADIUS_METERS);
}

#[test]
fn final_quality_global_spring_plan_uses_fortran_integer_cellwidth_base() {
    let mut mkgrd = mkgrd_config();
    mkgrd.nxp = 112;
    mkgrd.mesh_type = "atmosmesh".to_string();
    mkgrd.output_format = "MPAS".to_string();
    let mut refine = mixed_refine_config();
    refine.spring_global_type = 1;
    refine.spring_regional_type = 0;

    let plan = earthmesh_cli::plan_mkgrd_final_quality_check_io(&mkgrd, &refine, 4)
        .expect("plan final quality global spring controls");
    let spring = plan.global_spring.expect("global spring controls");

    assert_eq!(spring.base_cellwidth, Some(68.0));
}

#[test]
fn final_quality_global_spring_plan_rejects_invalid_distance_controls() {
    let mkgrd = mkgrd_config();
    let mut refine = mixed_refine_config();
    refine.spring_global_type = 1;
    refine.spring_regional_type = 0;
    refine.num_rc = 1;
    refine.set_dis_type = "unknown".to_string();

    let err = earthmesh_cli::plan_mkgrd_final_quality_check_io(&mkgrd, &refine, 4)
        .expect_err("unknown set_dis_type must be rejected");

    assert!(err.to_string().contains("set_dis_type"));
}

#[test]
fn final_quality_regional_spring_plan_uses_namelist_runtime_controls() {
    let mkgrd = mkgrd_config();
    let mut refine = mixed_refine_config();
    refine.spring_global_type = 0;
    refine.spring_regional_type = 2;
    refine.halo[1] = 7;
    refine.niter_refine = 456;

    let plan = earthmesh_cli::plan_mkgrd_final_quality_check_io(&mkgrd, &refine, 4)
        .expect("plan final regional spring controls");
    let spring = plan.regional_spring.expect("regional spring controls");

    assert_eq!(plan.global_spring, None);
    assert_eq!(plan.regional_source_mask, None);
    assert_eq!(plan.regional_set_dis, Some(7));
    assert_eq!(spring.niter_refine, 456);
    assert_eq!(spring.radius, earthmesh_core::EARTH_RADIUS_METERS);
}

#[test]
fn final_quality_check_io_plan_preserves_fortran_skip_and_regional_final_modes() {
    let mkgrd = mkgrd_config();
    let mut refine = mixed_refine_config();

    let skipped = earthmesh_cli::plan_mkgrd_final_quality_check_io(&mkgrd, &refine, 4)
        .expect("plan skipped final quality check");
    assert!(!skipped.run_quality_check);
    assert_eq!(
        skipped.spring_mode,
        earthmesh_cli::MkgrdFinalQualitySpringMode::SkippedBothDisabled
    );
    assert_eq!(skipped.original_gridfile, None);

    refine.spring_global_type = 0;
    refine.spring_regional_type = 2;
    refine.halo[1] = 7;
    let regional = earthmesh_cli::plan_mkgrd_final_quality_check_io(&mkgrd, &refine, 4)
        .expect("plan regional final quality check");
    assert!(regional.run_quality_check);
    assert_eq!(
        regional.spring_mode,
        earthmesh_cli::MkgrdFinalQualitySpringMode::RegionalFinal
    );
    assert_eq!(regional.regional_set_dis, Some(7));

    refine.spring_global_type = 1;
    refine.spring_regional_type = 1;
    let skipped_regional_each_step =
        earthmesh_cli::plan_mkgrd_final_quality_check_io(&mkgrd, &refine, 4)
            .expect("plan springregional each-step skip");
    assert!(!skipped_regional_each_step.run_quality_check);
    assert_eq!(
        skipped_regional_each_step.spring_mode,
        earthmesh_cli::MkgrdFinalQualitySpringMode::SkippedRegionalEachStep
    );
}

fn temp_root(prefix: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("{prefix}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("case_refine/gridfile")).expect("create gridfile dir");
    root
}

fn small_mesh(extra_triangle: usize) -> UnstructuredMesh {
    let mut m_points = vec![
        LonLatPoint { lon: 0.0, lat: 0.0 },
        LonLatPoint { lon: 0.0, lat: 0.0 },
        LonLatPoint { lon: 1.0, lat: 0.0 },
    ];
    let mut m_to_w = vec![[1, 1, 1], [1, 2, 3], [1, 2, 3]];
    for _ in 0..extra_triangle {
        m_points.push(LonLatPoint { lon: 0.5, lat: 0.5 });
        m_to_w.push([1, 2, 3]);
    }
    UnstructuredMesh {
        m_points,
        w_points: vec![
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 1.0, lat: 0.0 },
            LonLatPoint { lon: 0.0, lat: 1.0 },
        ],
        m_to_w,
        w_to_m: vec![vec![1], vec![1], vec![1], vec![1]],
        n_w_to_m: vec![1, 1, 1, 1],
    }
}
