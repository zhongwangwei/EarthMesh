use std::io;

use earthmesh_core::EarthmeshRuntimeState;

use crate::{
    MkgrdFinalQualityCheckIoPlan, MkgrdRefineLoopStepIoPlan, MkgrdRefineSourceBranchReport,
    MkgrdRefineSourceIoPlan,
};

/// Pluggable execution surface for the heavy kernels inside the top-level
/// `mkgrd.F90` refine loop.
///
/// The file schedule stays owned by `MkgrdRefineLoopIoPlan`; implementations
/// only execute the already-planned branch or geometry step. This keeps the
/// Fortran order testable while the remaining geometry kernels are replaced
/// incrementally.
pub trait MkgrdRefineLoopExecutor {
    fn run_source_branch(
        &mut self,
        step: &MkgrdRefineLoopStepIoPlan,
        source: &MkgrdRefineSourceIoPlan,
    ) -> io::Result<()>;

    fn run_refine_loop_step(&mut self, step: &MkgrdRefineLoopStepIoPlan) -> io::Result<()>;

    fn run_final_quality_check(&mut self, plan: &MkgrdFinalQualityCheckIoPlan) -> io::Result<()>;

    fn accept_source_branch_outputs(
        &mut self,
        _step: &MkgrdRefineLoopStepIoPlan,
        _source: &MkgrdRefineSourceIoPlan,
    ) -> io::Result<()> {
        Ok(())
    }

    fn accept_source_branch_report(
        &mut self,
        step: &MkgrdRefineLoopStepIoPlan,
        source: &MkgrdRefineSourceIoPlan,
        _report: &MkgrdRefineSourceBranchReport,
    ) -> io::Result<()> {
        self.accept_source_branch_outputs(step, source)
    }

    fn source_branch_reports(&self) -> &[MkgrdRefineSourceBranchReport] {
        &[]
    }

    fn runtime_state(&self) -> Option<&EarthmeshRuntimeState> {
        None
    }

    fn record_runtime_mesh_counts_for_step(
        &mut self,
        _step: usize,
        _num_mp_step: usize,
        _num_wp_step: usize,
    ) -> io::Result<()> {
        Ok(())
    }

    fn last_refine_step_post_counts(&self) -> Option<(usize, usize)> {
        None
    }
}

impl<T> MkgrdRefineLoopExecutor for &mut T
where
    T: MkgrdRefineLoopExecutor + ?Sized,
{
    fn run_source_branch(
        &mut self,
        step: &MkgrdRefineLoopStepIoPlan,
        source: &MkgrdRefineSourceIoPlan,
    ) -> io::Result<()> {
        (**self).run_source_branch(step, source)
    }

    fn run_refine_loop_step(&mut self, step: &MkgrdRefineLoopStepIoPlan) -> io::Result<()> {
        (**self).run_refine_loop_step(step)
    }

    fn run_final_quality_check(&mut self, plan: &MkgrdFinalQualityCheckIoPlan) -> io::Result<()> {
        (**self).run_final_quality_check(plan)
    }

    fn accept_source_branch_outputs(
        &mut self,
        step: &MkgrdRefineLoopStepIoPlan,
        source: &MkgrdRefineSourceIoPlan,
    ) -> io::Result<()> {
        (**self).accept_source_branch_outputs(step, source)
    }

    fn accept_source_branch_report(
        &mut self,
        step: &MkgrdRefineLoopStepIoPlan,
        source: &MkgrdRefineSourceIoPlan,
        report: &MkgrdRefineSourceBranchReport,
    ) -> io::Result<()> {
        (**self).accept_source_branch_report(step, source, report)
    }

    fn source_branch_reports(&self) -> &[MkgrdRefineSourceBranchReport] {
        (**self).source_branch_reports()
    }

    fn runtime_state(&self) -> Option<&EarthmeshRuntimeState> {
        (**self).runtime_state()
    }
}
