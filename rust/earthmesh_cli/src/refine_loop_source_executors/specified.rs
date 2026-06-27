use std::io;

use super::types::{
    MkgrdSpecifiedRefineSourceBranchReport, MkgrdSpecifiedRefineSourceExecutorOptions,
};
use crate::refine_loop_executor::MkgrdRefineLoopExecutor;
use crate::{
    getcontain_mesh_kind_from_mesh_type, read_contain_netcdf,
    run_area_judge_refine_grid_fortran_indexed, run_getcontain_refine_file_fortran_indexed,
    run_mkgrd_final_quality_check, write_getref_specified_threshold_netcdf,
    AreaJudgeRefineGridRunConfig, GetContainRefineFileRunConfig, MkgrdFinalQualityCheckIoPlan,
    MkgrdRefineLoopStepIoPlan, MkgrdRefineSource, MkgrdRefineSourceIoPlan,
};

#[derive(Debug, Clone, Copy)]
pub struct MkgrdSpecifiedRefineSourceExecutor<'a> {
    options: MkgrdSpecifiedRefineSourceExecutorOptions<'a>,
}

impl<'a> MkgrdSpecifiedRefineSourceExecutor<'a> {
    pub fn new(options: MkgrdSpecifiedRefineSourceExecutorOptions<'a>) -> Self {
        Self { options }
    }

    pub fn run_source_branch_report(
        &self,
        step: &MkgrdRefineLoopStepIoPlan,
        source: &MkgrdRefineSourceIoPlan,
    ) -> io::Result<MkgrdSpecifiedRefineSourceBranchReport> {
        if source.source != MkgrdRefineSource::SpecifiedStep {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "MkgrdSpecifiedRefineSourceExecutor only supports specified refine sources",
            ));
        }
        if source.area_judge_iter != step.step
            || source.get_contain_iter != step.step
            || source.getref_iter != step.step
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "specified refine source iter mismatch for step {}: area {} contain {} getref {}",
                    step.step, source.area_judge_iter, source.get_contain_iter, source.getref_iter
                ),
            ));
        }

        let mask_refine_ndm = *self
            .options
            .mask_refine_ndm_by_iter
            .get(source.area_judge_iter)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "specified refine iter {} must fit mask_refine_ndm 0:9",
                        source.area_judge_iter
                    ),
                )
            })?;
        if mask_refine_ndm == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "mask_refine_ndm({}) must be larger than zero",
                    source.area_judge_iter
                ),
            ));
        }

        let mesh_kind = getcontain_mesh_kind_from_mesh_type(self.options.mesh_type)?;
        let area = run_area_judge_refine_grid_fortran_indexed(AreaJudgeRefineGridRunConfig {
            file_dir: self.options.file_dir,
            iter: source.area_judge_iter,
            calculated_refine: None,
            mask_refine_spc_type: self.options.mask_refine_spc_type,
            mask_refine_ndm,
            is_in_domain: self.options.is_in_domain,
            lon_vertex: self.options.lon_vertex,
            lat_vertex: self.options.lat_vertex,
            lon_i: self.options.lon_i,
            lat_i: self.options.lat_i,
            gridnum_perdegree: self.options.gridnum_perdegree,
            nlons_source: self.options.nlons_source,
            nlats_source: self.options.nlats_source,
            refine_output: &source.area_judge_output,
        })?;
        let contain = run_getcontain_refine_file_fortran_indexed(GetContainRefineFileRunConfig {
            gridfile: &step.refine_loop_input_gridfile,
            area_grid_file: &source.area_judge_output,
            output: &source.contain_output,
            mesh_kind,
            seaorland: self.options.seaorland,
            lon_vertex: self.options.lon_vertex,
            lat_vertex: self.options.lat_vertex,
            lon_i: self.options.lon_i,
            lat_i: self.options.lat_i,
            num_vertex: self.options.num_vertex,
        })?;
        let contain_payload = read_contain_netcdf(&source.contain_output)?;
        let threshold_output = source
            .specified_threshold_output
            .as_deref()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "specified refine source requires specified threshold output path",
                )
            })?;
        let specified_threshold = write_getref_specified_threshold_netcdf(
            threshold_output,
            &contain_payload.is_in_area_ustr,
        )?;

        Ok(MkgrdSpecifiedRefineSourceBranchReport {
            area,
            contain,
            specified_threshold,
        })
    }
}

impl MkgrdRefineLoopExecutor for MkgrdSpecifiedRefineSourceExecutor<'_> {
    fn run_source_branch(
        &mut self,
        step: &MkgrdRefineLoopStepIoPlan,
        source: &MkgrdRefineSourceIoPlan,
    ) -> io::Result<()> {
        self.run_source_branch_report(step, source).map(|_| ())
    }

    fn run_refine_loop_step(&mut self, _step: &MkgrdRefineLoopStepIoPlan) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "refine_loop geometry step is not implemented by MkgrdSpecifiedRefineSourceExecutor",
        ))
    }

    fn run_final_quality_check(&mut self, plan: &MkgrdFinalQualityCheckIoPlan) -> io::Result<()> {
        run_mkgrd_final_quality_check(plan)
    }
}
