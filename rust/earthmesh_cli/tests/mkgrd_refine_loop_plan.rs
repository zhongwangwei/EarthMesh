use std::path::PathBuf;

use earthmesh_cli::{plan_mkgrd_refine_loop, plan_mkgrd_refine_loop_io, MkgrdRefineSource};
use earthmesh_core::{EarthmeshConfig, RefineConfig};

fn mixed_refine_config() -> RefineConfig {
    RefineConfig::from_mkrefine_namelist(
        "&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=2\n  RL%max_iter_cal=3\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_cal_type='bbox'\n  RL%refine_num_landtypes=.true.\n/\n",
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
