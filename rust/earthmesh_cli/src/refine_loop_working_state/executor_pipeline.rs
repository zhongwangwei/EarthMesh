use std::io;
use std::path::PathBuf;

use super::run_mkgrd_refine_loop_working_state_prologue;
use crate::refine_loop_executor::MkgrdRefineLoopExecutor;
use crate::refine_loop_source_executors::MkgrdRefineSourceBranchReport;
use crate::*;

mod transition_pipeline;

/// File-backed refine-loop geometry executor backed by `RefineLoopWorkingState`.
///
/// By default it performs the verified prologue/read/write boundary and exports
/// the current state unchanged. Supplying one-into-four markers enables the
/// migrated one-into-four -> Array_length -> NGR path for focused parity slices.
#[derive(Debug, Default, Clone)]
pub struct MkgrdRefineLoopWorkingStateExecutor {
    one_into_four_ref_sjx: Option<Vec<i32>>,
    one_into_two_ref_sjx: Option<Vec<i32>>,
    one_into_two_is_reverse: bool,
    one_into_two_mrl_new: Option<Vec<i32>>,
    one_into_two_triangle_neighbors: Option<Vec<Vec<usize>>>,
    specified_threshold_file: Option<PathBuf>,
    calculated_threshold_files: Vec<PathBuf>,
    num_vertex: usize,
    set_dis_in: usize,
    last_post_refine_counts: Option<(usize, usize)>,
}

impl MkgrdRefineLoopWorkingStateExecutor {
    pub fn with_one_into_four_ref_sjx(
        ref_sjx: Vec<i32>,
        num_vertex: usize,
        set_dis_in: usize,
    ) -> Self {
        Self {
            one_into_four_ref_sjx: Some(ref_sjx),
            one_into_two_ref_sjx: None,
            one_into_two_is_reverse: false,
            one_into_two_mrl_new: None,
            one_into_two_triangle_neighbors: None,
            specified_threshold_file: None,
            calculated_threshold_files: Vec::new(),
            num_vertex,
            set_dis_in,
            last_post_refine_counts: None,
        }
    }

    pub fn with_one_into_two_ref_sjx(
        ref_sjx: Vec<i32>,
        is_reverse: bool,
        num_vertex: usize,
        mrl_new: Vec<i32>,
    ) -> Self {
        Self {
            one_into_four_ref_sjx: None,
            one_into_two_ref_sjx: Some(ref_sjx),
            one_into_two_is_reverse: is_reverse,
            one_into_two_mrl_new: Some(mrl_new),
            one_into_two_triangle_neighbors: None,
            specified_threshold_file: None,
            calculated_threshold_files: Vec::new(),
            num_vertex,
            set_dis_in: 0,
            last_post_refine_counts: None,
        }
    }

    pub fn with_one_into_two_triangle_neighbors(
        mut self,
        triangle_neighbors: Vec<Vec<usize>>,
    ) -> Self {
        self.one_into_two_triangle_neighbors = Some(triangle_neighbors);
        self
    }

    pub fn with_specified_threshold_file(
        threshold_file: impl Into<PathBuf>,
        num_vertex: usize,
        set_dis_in: usize,
    ) -> Self {
        Self {
            one_into_four_ref_sjx: None,
            one_into_two_ref_sjx: None,
            one_into_two_is_reverse: false,
            one_into_two_mrl_new: None,
            one_into_two_triangle_neighbors: None,
            specified_threshold_file: Some(threshold_file.into()),
            calculated_threshold_files: Vec::new(),
            num_vertex,
            set_dis_in,
            last_post_refine_counts: None,
        }
    }

    pub fn with_calculated_threshold_files(
        threshold_files: impl IntoIterator<Item = PathBuf>,
        num_vertex: usize,
        set_dis_in: usize,
    ) -> Self {
        Self {
            one_into_four_ref_sjx: None,
            one_into_two_ref_sjx: None,
            one_into_two_is_reverse: false,
            one_into_two_mrl_new: None,
            one_into_two_triangle_neighbors: None,
            specified_threshold_file: None,
            calculated_threshold_files: threshold_files.into_iter().collect(),
            num_vertex,
            set_dis_in,
            last_post_refine_counts: None,
        }
    }

    pub fn run_refine_loop_step_report(
        &self,
        step: &MkgrdRefineLoopStepIoPlan,
    ) -> io::Result<MkgrdRefineLoopWorkingStateStepReport> {
        let prologue = run_mkgrd_refine_loop_working_state_prologue(step)?;
        let mut state = prologue.state.clone();
        let mut loaded_ref_sjx = None;
        let mut onedivide_four_connection = None;
        let mut array_length = None;
        let mut onedivide_four_renew = None;
        let mut onedivide_two = None;
        let mut ngr_renew = None;
        let mut post_refine_counts = None;

        let ref_sjx = if let Some(ref_sjx) = &self.one_into_four_ref_sjx {
            Some(ref_sjx.clone())
        } else if let Some(threshold_file) = &self.specified_threshold_file {
            Some(read_getref_specified_ref_sjx_netcdf(threshold_file)?)
        } else if !self.calculated_threshold_files.is_empty() {
            Some(read_getref_calculated_ref_sjx_netcdf(
                &self.calculated_threshold_files,
                self.num_vertex,
            )?)
        } else {
            None
        };

        if let Some(ref_sjx) = ref_sjx {
            loaded_ref_sjx = Some(ref_sjx.clone());
            self.run_configured_one_into_four_pipeline(step, &mut state, &ref_sjx)
                .map(|(connection, length, renew, ngr, counts)| {
                    onedivide_four_connection = Some(connection);
                    array_length = Some(length);
                    onedivide_four_renew = Some(renew);
                    ngr_renew = Some(ngr);
                    post_refine_counts = counts;
                })?;
        } else if let Some(ref_sjx) = &self.one_into_two_ref_sjx {
            loaded_ref_sjx = Some(ref_sjx.clone());
            let mrl_new = self.one_into_two_mrl_new.as_ref().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "one-into-two transition requires mrl_new markers",
                )
            })?;
            onedivide_two = Some(
                self.run_configured_one_into_two_transition_pipeline(&mut state, ref_sjx, mrl_new)?,
            );
        }

        let output_mesh = state.to_unstructured_mesh()?;
        write_unstructured_mesh_netcdf(&step.refine_loop_output_gridfile, &output_mesh)?;
        Ok(MkgrdRefineLoopWorkingStateStepReport {
            prologue,
            state,
            output_gridfile: step.refine_loop_output_gridfile.clone(),
            loaded_ref_sjx,
            onedivide_four_connection,
            array_length,
            onedivide_four_renew,
            onedivide_two,
            ngr_renew,
            post_refine_counts,
        })
    }
}

impl MkgrdRefineLoopExecutor for MkgrdRefineLoopWorkingStateExecutor {
    fn run_source_branch(
        &mut self,
        _step: &MkgrdRefineLoopStepIoPlan,
        _source: &MkgrdRefineSourceIoPlan,
    ) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "refine source branches are not implemented by MkgrdRefineLoopWorkingStateExecutor",
        ))
    }

    fn run_refine_loop_step(&mut self, step: &MkgrdRefineLoopStepIoPlan) -> io::Result<()> {
        let report = self.run_refine_loop_step_report(step)?;
        self.last_post_refine_counts = report.post_refine_counts;
        Ok(())
    }

    fn run_final_quality_check(&mut self, plan: &MkgrdFinalQualityCheckIoPlan) -> io::Result<()> {
        run_mkgrd_final_quality_check(plan)
    }

    fn accept_source_branch_outputs(
        &mut self,
        _step: &MkgrdRefineLoopStepIoPlan,
        source: &MkgrdRefineSourceIoPlan,
    ) -> io::Result<()> {
        self.one_into_four_ref_sjx = None;
        match source.source {
            MkgrdRefineSource::SpecifiedStep => {
                let threshold_file =
                    source.specified_threshold_output.as_ref().ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "specified refine source handoff requires a threshold output path",
                        )
                    })?;
                self.specified_threshold_file = Some(threshold_file.clone());
                self.calculated_threshold_files.clear();
            }
            MkgrdRefineSource::CalculatedIterZero => {
                if source.threshold_outputs.is_empty() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "calculated refine source handoff requires threshold output paths",
                    ));
                }
                self.specified_threshold_file = None;
                self.calculated_threshold_files = source.threshold_outputs.clone();
            }
        }
        Ok(())
    }

    fn accept_source_branch_report(
        &mut self,
        step: &MkgrdRefineLoopStepIoPlan,
        source: &MkgrdRefineSourceIoPlan,
        report: &MkgrdRefineSourceBranchReport,
    ) -> io::Result<()> {
        let counts = report.contain_runtime_counts();
        if counts.previous_num_vertex > 0 {
            self.num_vertex = counts.previous_num_vertex;
        }
        if let MkgrdRefineSourceBranchReport::Calculated(report) = report {
            let threshold_files = report.getref.written_threshold_outputs();
            if !threshold_files.is_empty() {
                self.specified_threshold_file = None;
                self.calculated_threshold_files = threshold_files;
                return Ok(());
            }
        }
        self.accept_source_branch_outputs(step, source)
    }

    fn last_refine_step_post_counts(&self) -> Option<(usize, usize)> {
        self.last_post_refine_counts
    }
}
