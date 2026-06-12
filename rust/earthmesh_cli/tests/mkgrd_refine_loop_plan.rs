use earthmesh_cli::{plan_mkgrd_refine_loop, MkgrdRefineSource};
use earthmesh_core::RefineConfig;

fn mixed_refine_config() -> RefineConfig {
    RefineConfig::from_mkrefine_namelist(
        "&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=2\n  RL%max_iter_cal=3\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_cal_type='bbox'\n  RL%refine_num_landtypes=.true.\n/\n",
        "landmesh",
        "hex",
    )
    .expect("parse mixed refine config")
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
