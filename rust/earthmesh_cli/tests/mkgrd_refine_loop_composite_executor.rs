use std::fs;
use std::io;

use earthmesh_cli::{
    MkgrdCompositeRefineLoopExecutor, MkgrdFinalQualityCheckIoPlan, MkgrdRefineLoopExecutor,
    MkgrdRefineLoopStepIoPlan, MkgrdRefineSourceIoPlan,
};
use earthmesh_core::{EarthmeshConfig, RefineConfig};

#[derive(Default)]
struct RecordingSourceExecutor {
    events: Vec<String>,
}

impl MkgrdRefineLoopExecutor for RecordingSourceExecutor {
    fn run_source_branch(
        &mut self,
        step: &MkgrdRefineLoopStepIoPlan,
        source: &MkgrdRefineSourceIoPlan,
    ) -> io::Result<()> {
        self.events
            .push(format!("source:{}:{:?}", step.step, source.source));
        Ok(())
    }

    fn run_refine_loop_step(&mut self, _step: &MkgrdRefineLoopStepIoPlan) -> io::Result<()> {
        panic!("composite executor must not dispatch refine steps to source executor");
    }

    fn run_final_quality_check(&mut self, _plan: &MkgrdFinalQualityCheckIoPlan) -> io::Result<()> {
        panic!("composite executor must not dispatch final quality to source executor");
    }
}

#[derive(Default)]
struct RecordingGeometryExecutor {
    events: Vec<String>,
}

impl MkgrdRefineLoopExecutor for RecordingGeometryExecutor {
    fn run_source_branch(
        &mut self,
        _step: &MkgrdRefineLoopStepIoPlan,
        _source: &MkgrdRefineSourceIoPlan,
    ) -> io::Result<()> {
        panic!("composite executor must not dispatch source branches to geometry executor");
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

    fn run_final_quality_check(&mut self, _plan: &MkgrdFinalQualityCheckIoPlan) -> io::Result<()> {
        self.events.push("final-quality".to_string());
        Ok(())
    }
}

fn mkgrd_config(base_dir: &str) -> EarthmeshConfig {
    EarthmeshConfig::from_mkgrd_namelist(&format!(
        "&mkgrd\n  NL%EXPNME='case_composite_exec'\n  NL%base_dir='{base_dir}'\n  NL%NXP=8\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  NL%refine=.true.\n/\n"
    ))
    .expect("parse mkgrd config")
}

fn refine_config() -> RefineConfig {
    RefineConfig::from_mkrefine_namelist(
        "&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n/\n",
        "landmesh",
        "hex",
    )
    .expect("parse refine config")
}

#[test]
fn composite_executor_routes_sources_to_source_executor_and_geometry_to_working_executor() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mkgrd_refine_loop_composite_executor_{}",
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

    let mut executor = MkgrdCompositeRefineLoopExecutor::new(
        RecordingSourceExecutor::default(),
        RecordingGeometryExecutor::default(),
    );
    let report = earthmesh_cli::run_mkgrd_refine_loop_execution(&plan, &mut executor, None)
        .expect("run refine loop through composite executor");

    assert_eq!(
        executor.source_executor.events,
        vec!["source:1:SpecifiedStep"]
    );
    assert_eq!(
        executor.refine_executor.events,
        vec!["refine:1", "final-quality"]
    );
    assert_eq!(report.executed_sources, 1);
    assert!(report.source_branch_reports.is_empty());
    assert_eq!(report.executed_refine_steps, 1);
    assert!(report.ran_final_quality_check);
    assert_eq!(
        fs::read_to_string(&plan.final_result_gridfile).expect("read final result gridfile"),
        "gridfile after step 1"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn migrated_refine_loop_executor_constructor_hides_composite_generic_shape() {
    let executor: earthmesh_cli::MkgrdMigratedRefineLoopExecutor<'_> =
        earthmesh_cli::mkgrd_migrated_refine_loop_executor(
            earthmesh_cli::MkgrdRefineSourceBranchExecutorOptions::default(),
            earthmesh_cli::MkgrdRefineLoopWorkingStateExecutor::default(),
        );
    assert!(executor.source_branch_reports().is_empty());
    let (_source_executor, _refine_executor) = executor.into_parts();
}
