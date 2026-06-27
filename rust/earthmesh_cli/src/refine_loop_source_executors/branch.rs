use std::io;

use earthmesh_core::EarthmeshRuntimeState;

use super::calculated::MkgrdCalculatedRefineSourceExecutor;
use super::specified::MkgrdSpecifiedRefineSourceExecutor;
use super::types::{MkgrdRefineSourceBranchExecutorOptions, MkgrdRefineSourceBranchReport};
use crate::refine_loop_executor::MkgrdRefineLoopExecutor;
use crate::{
    run_mkgrd_final_quality_check, MkgrdFinalQualityCheckIoPlan, MkgrdRefineLoopStepIoPlan,
    MkgrdRefineSource, MkgrdRefineSourceIoPlan,
};

#[derive(Debug, Clone)]
pub struct MkgrdRefineSourceBranchExecutor<'a> {
    options: MkgrdRefineSourceBranchExecutorOptions<'a>,
    source_branch_reports: Vec<MkgrdRefineSourceBranchReport>,
    runtime_state: Option<EarthmeshRuntimeState>,
}

impl<'a> MkgrdRefineSourceBranchExecutor<'a> {
    pub fn new(options: MkgrdRefineSourceBranchExecutorOptions<'a>) -> Self {
        Self {
            options,
            source_branch_reports: Vec::new(),
            runtime_state: None,
        }
    }

    pub fn with_runtime_state(mut self, runtime_state: EarthmeshRuntimeState) -> Self {
        self.runtime_state = Some(runtime_state);
        self
    }

    pub fn source_branch_reports(&self) -> &[MkgrdRefineSourceBranchReport] {
        &self.source_branch_reports
    }

    pub fn runtime_state(&self) -> Option<&EarthmeshRuntimeState> {
        self.runtime_state.as_ref()
    }

    pub fn run_source_branch_report(
        &self,
        step: &MkgrdRefineLoopStepIoPlan,
        source: &MkgrdRefineSourceIoPlan,
    ) -> io::Result<MkgrdRefineSourceBranchReport> {
        match source.source {
            MkgrdRefineSource::CalculatedIterZero => {
                let options = self.options.calculated.ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "calculated refine source branch requested but no calculated executor options were provided",
                    )
                })?;
                MkgrdCalculatedRefineSourceExecutor::new(options)
                    .run_source_branch_report(step, source)
                    .map(MkgrdRefineSourceBranchReport::Calculated)
            }
            MkgrdRefineSource::SpecifiedStep => {
                let options = self.options.specified.ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "specified refine source branch requested but no specified executor options were provided",
                    )
                })?;
                MkgrdSpecifiedRefineSourceExecutor::new(options)
                    .run_source_branch_report(step, source)
                    .map(MkgrdRefineSourceBranchReport::Specified)
            }
        }
    }
}

impl MkgrdRefineLoopExecutor for MkgrdRefineSourceBranchExecutor<'_> {
    fn run_source_branch(
        &mut self,
        step: &MkgrdRefineLoopStepIoPlan,
        source: &MkgrdRefineSourceIoPlan,
    ) -> io::Result<()> {
        let report = self.run_source_branch_report(step, source)?;
        if let Some(runtime_state) = self.runtime_state.as_mut() {
            let counts = report.contain_runtime_counts();
            runtime_state
                .record_mesh_counts_for_step(
                    step.step,
                    counts.current_num_mp_step,
                    counts.current_num_wp_step,
                )
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
            if counts.previous_num_vertex > 0 {
                runtime_state
                    .record_num_vertex(counts.previous_num_vertex)
                    .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
            }
        }
        self.source_branch_reports.push(report);
        Ok(())
    }

    fn run_refine_loop_step(&mut self, _step: &MkgrdRefineLoopStepIoPlan) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "refine_loop geometry step is not implemented by MkgrdRefineSourceBranchExecutor",
        ))
    }

    fn run_final_quality_check(&mut self, plan: &MkgrdFinalQualityCheckIoPlan) -> io::Result<()> {
        run_mkgrd_final_quality_check(plan)
    }

    fn source_branch_reports(&self) -> &[MkgrdRefineSourceBranchReport] {
        &self.source_branch_reports
    }

    fn runtime_state(&self) -> Option<&EarthmeshRuntimeState> {
        self.runtime_state.as_ref()
    }

    fn record_runtime_mesh_counts_for_step(
        &mut self,
        step: usize,
        num_mp_step: usize,
        num_wp_step: usize,
    ) -> io::Result<()> {
        if let Some(runtime_state) = self.runtime_state.as_mut() {
            runtime_state
                .record_mesh_counts_for_step(step, num_mp_step, num_wp_step)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
        }
        Ok(())
    }
}
