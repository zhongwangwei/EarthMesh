use std::io;

use earthmesh_core::EarthmeshRuntimeState;

use crate::refine_loop_executor::MkgrdRefineLoopExecutor;
use crate::{
    MkgrdFinalQualityCheckIoPlan, MkgrdRefineLoopStepIoPlan, MkgrdRefineLoopWorkingStateExecutor,
    MkgrdRefineSourceBranchExecutor, MkgrdRefineSourceBranchExecutorOptions,
    MkgrdRefineSourceBranchReport, MkgrdRefineSourceIoPlan,
};

/// Composite refine-loop executor that routes already-migrated source branches
/// to one executor and geometry/final-quality work to another.
///
/// This is the Rust replacement glue for the top-level `mkgrd.F90` refine loop:
/// callers can combine `MkgrdRefineSourceBranchExecutor` with
/// `MkgrdRefineLoopWorkingStateExecutor` without writing a bespoke adapter.
#[derive(Debug, Clone)]
pub struct MkgrdCompositeRefineLoopExecutor<SourceExecutor, RefineExecutor> {
    pub source_executor: SourceExecutor,
    pub refine_executor: RefineExecutor,
}

impl<SourceExecutor, RefineExecutor>
    MkgrdCompositeRefineLoopExecutor<SourceExecutor, RefineExecutor>
{
    pub fn new(source_executor: SourceExecutor, refine_executor: RefineExecutor) -> Self {
        Self {
            source_executor,
            refine_executor,
        }
    }

    pub fn into_parts(self) -> (SourceExecutor, RefineExecutor) {
        (self.source_executor, self.refine_executor)
    }
}

impl<SourceExecutor, RefineExecutor> MkgrdRefineLoopExecutor
    for MkgrdCompositeRefineLoopExecutor<SourceExecutor, RefineExecutor>
where
    SourceExecutor: MkgrdRefineLoopExecutor,
    RefineExecutor: MkgrdRefineLoopExecutor,
{
    fn run_source_branch(
        &mut self,
        step: &MkgrdRefineLoopStepIoPlan,
        source: &MkgrdRefineSourceIoPlan,
    ) -> io::Result<()> {
        let report_count = self.source_executor.source_branch_reports().len();
        self.source_executor.run_source_branch(step, source)?;
        if let Some(report) = self
            .source_executor
            .source_branch_reports()
            .get(report_count..)
            .and_then(|reports| reports.last())
        {
            self.refine_executor
                .accept_source_branch_report(step, source, report)
        } else {
            self.refine_executor
                .accept_source_branch_outputs(step, source)
        }
    }

    fn run_refine_loop_step(&mut self, step: &MkgrdRefineLoopStepIoPlan) -> io::Result<()> {
        self.refine_executor.run_refine_loop_step(step)?;
        if let Some((num_mp_step, num_wp_step)) =
            self.refine_executor.last_refine_step_post_counts()
        {
            self.source_executor.record_runtime_mesh_counts_for_step(
                step.step,
                num_mp_step,
                num_wp_step,
            )?;
        }
        Ok(())
    }

    fn run_final_quality_check(&mut self, plan: &MkgrdFinalQualityCheckIoPlan) -> io::Result<()> {
        self.refine_executor.run_final_quality_check(plan)
    }

    fn source_branch_reports(&self) -> &[MkgrdRefineSourceBranchReport] {
        self.source_executor.source_branch_reports()
    }

    fn runtime_state(&self) -> Option<&EarthmeshRuntimeState> {
        self.source_executor
            .runtime_state()
            .or_else(|| self.refine_executor.runtime_state())
    }
}

/// Standard migrated refine-loop executor shape: file-backed source-branch
/// kernels plus Rust working-state geometry/final-quality kernels.
pub type MkgrdMigratedRefineLoopExecutor<'a> = MkgrdCompositeRefineLoopExecutor<
    MkgrdRefineSourceBranchExecutor<'a>,
    MkgrdRefineLoopWorkingStateExecutor,
>;

impl<'a> MkgrdMigratedRefineLoopExecutor<'a> {
    /// Return source-branch reports recorded through the generic refine-loop
    /// executor path, including explicit `Get_Contain` runtime counter handoffs.
    pub fn source_branch_reports(&self) -> &[MkgrdRefineSourceBranchReport] {
        self.source_executor.source_branch_reports()
    }

    pub fn runtime_state(&self) -> Option<&EarthmeshRuntimeState> {
        self.source_executor
            .runtime_state()
            .or_else(|| self.refine_executor.runtime_state())
    }
}

/// Build the standard migrated refine-loop executor without exposing the generic
/// composite shape to adapters or Python/Rust runtime callers.
pub fn mkgrd_migrated_refine_loop_executor<'a>(
    source_options: MkgrdRefineSourceBranchExecutorOptions<'a>,
    refine_executor: MkgrdRefineLoopWorkingStateExecutor,
) -> MkgrdMigratedRefineLoopExecutor<'a> {
    MkgrdCompositeRefineLoopExecutor::new(
        MkgrdRefineSourceBranchExecutor::new(source_options),
        refine_executor,
    )
}

/// Build the standard migrated refine-loop executor and seed the source branch
/// side with the Rust-owned runtime state prepared from namelists/read_nl.
pub fn mkgrd_migrated_refine_loop_executor_with_runtime_state<'a>(
    source_options: MkgrdRefineSourceBranchExecutorOptions<'a>,
    refine_executor: MkgrdRefineLoopWorkingStateExecutor,
    runtime_state: EarthmeshRuntimeState,
) -> MkgrdMigratedRefineLoopExecutor<'a> {
    MkgrdCompositeRefineLoopExecutor::new(
        MkgrdRefineSourceBranchExecutor::new(source_options).with_runtime_state(runtime_state),
        refine_executor,
    )
}
