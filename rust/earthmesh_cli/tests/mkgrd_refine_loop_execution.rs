use std::fs;
use std::io;

use earthmesh_cli::{
    MkgrdRefineLoopExecutor, MkgrdRefineLoopStepIoPlan, MkgrdRefineSource, MkgrdRefineSourceIoPlan,
};
use earthmesh_core::{EarthmeshConfig, RefineConfig};

#[derive(Default)]
struct RecordingExecutor {
    events: Vec<String>,
}

impl MkgrdRefineLoopExecutor for RecordingExecutor {
    fn run_source_branch(
        &mut self,
        step: &MkgrdRefineLoopStepIoPlan,
        source: &MkgrdRefineSourceIoPlan,
    ) -> io::Result<()> {
        self.events.push(format!(
            "source:{}:{:?}:{}:{}:{}",
            step.step,
            source.source,
            source.area_judge_iter,
            source.get_contain_iter,
            source.getref_iter
        ));
        Ok(())
    }

    fn run_refine_loop_step(&mut self, step: &MkgrdRefineLoopStepIoPlan) -> io::Result<()> {
        self.events.push(format!("refine:{}", step.step));
        if let Some(parent) = step.refine_loop_output_gridfile.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(
            &step.refine_loop_output_gridfile,
            format!("gridfile after step {}", step.step),
        )?;
        Ok(())
    }

    fn run_final_quality_check(
        &mut self,
        _plan: &earthmesh_cli::MkgrdFinalQualityCheckIoPlan,
    ) -> io::Result<()> {
        self.events.push("final-quality".to_string());
        Ok(())
    }
}

fn mkgrd_config(base_dir: &str) -> EarthmeshConfig {
    EarthmeshConfig::from_mkgrd_namelist(&format!(
        "&mkgrd\n  NL%EXPNME='case_refine_exec'\n  NL%base_dir='{base_dir}'\n  NL%NXP=8\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  NL%refine=.true.\n/\n"
    ))
    .expect("parse mkgrd config")
}

fn refine_config() -> RefineConfig {
    RefineConfig::from_mkrefine_namelist(
        "&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=2\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_cal_type='bbox'\n  RL%refine_num_landtypes=.true.\n/\n",
        "landmesh",
        "hex",
    )
    .expect("parse refine config")
}

#[test]
fn refine_loop_execution_dispatches_sources_steps_and_final_handoff_in_fortran_order() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mkgrd_refine_loop_execution_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let base_dir = format!("{}/", root.display());
    let mkgrd = mkgrd_config(&base_dir);
    let refine = refine_config();
    let plan =
        earthmesh_cli::plan_mkgrd_refine_loop_io(&mkgrd, &refine).expect("plan refine loop io");

    fs::create_dir_all(plan.steps[0].refine_loop_input_gridfile.parent().unwrap())
        .expect("create gridfile dir");
    fs::write(
        &plan.steps[0].refine_loop_input_gridfile,
        "initial gridfile",
    )
    .expect("write initial gridfile");

    let mut executor = RecordingExecutor::default();
    let report = earthmesh_cli::run_mkgrd_refine_loop_execution(&plan, &mut executor, None)
        .expect("run refine loop execution");

    assert_eq!(
        executor.events,
        vec![
            format!("source:1:{:?}:0:0:0", MkgrdRefineSource::CalculatedIterZero),
            format!("source:1:{:?}:1:1:1", MkgrdRefineSource::SpecifiedStep),
            "refine:1".to_string(),
            format!("source:2:{:?}:0:0:0", MkgrdRefineSource::CalculatedIterZero),
            "refine:2".to_string(),
        ]
    );
    assert_eq!(report.executed_sources, 3);
    assert_eq!(report.executed_refine_steps, 2);
    assert!(!report.ran_final_quality_check);
    assert_eq!(
        report.final_handoff.copied_result_gridfile,
        plan.final_result_gridfile
    );
    assert_eq!(
        fs::read_to_string(&plan.final_result_gridfile).expect("read final result gridfile"),
        "gridfile after step 2"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn refine_loop_execution_runs_final_quality_before_final_handoff_when_enabled() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mkgrd_refine_loop_final_quality_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let base_dir = format!("{}/", root.display());
    let mkgrd = mkgrd_config(&base_dir);
    let mut refine = refine_config();
    refine.spring_global_type = 1;
    let plan =
        earthmesh_cli::plan_mkgrd_refine_loop_io(&mkgrd, &refine).expect("plan refine loop io");

    fs::create_dir_all(plan.steps[0].refine_loop_input_gridfile.parent().unwrap())
        .expect("create gridfile dir");
    fs::write(
        &plan.steps[0].refine_loop_input_gridfile,
        "initial gridfile",
    )
    .expect("write initial gridfile");

    let mut executor = RecordingExecutor::default();
    let report = earthmesh_cli::run_mkgrd_refine_loop_execution(&plan, &mut executor, None)
        .expect("run refine loop execution");

    assert!(report.ran_final_quality_check);
    assert_eq!(executor.events.last(), Some(&"final-quality".to_string()));
    assert_eq!(
        fs::read_to_string(&plan.final_result_gridfile).expect("read final result gridfile"),
        "gridfile after step 2"
    );

    let _ = fs::remove_dir_all(&root);
}
