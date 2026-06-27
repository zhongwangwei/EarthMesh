use std::io;

use super::helpers::{getref_is_in_refine_from_contain, mkgrd_calculated_getref_output_refs};
use super::types::{
    MkgrdCalculatedRefineSourceBranchReport, MkgrdCalculatedRefineSourceExecutorOptions,
};
use crate::refine_loop_executor::MkgrdRefineLoopExecutor;
use crate::{
    getcontain_mesh_kind_from_mesh_type, read_contain_netcdf,
    run_area_judge_refine_grid_fortran_indexed, run_getcontain_refine_file_fortran_indexed,
    run_getref_integrated_threshold_files_fortran_indexed, run_mkgrd_final_quality_check,
    AreaJudgeRefineGridRunConfig, GetContainRefineFileRunConfig, GetRefIntegratedFileRunConfig,
    MkgrdFinalQualityCheckIoPlan, MkgrdRefineLoopStepIoPlan, MkgrdRefineSource,
    MkgrdRefineSourceIoPlan,
};

#[derive(Debug, Clone, Copy)]
pub struct MkgrdCalculatedRefineSourceExecutor<'a> {
    options: MkgrdCalculatedRefineSourceExecutorOptions<'a>,
}

impl<'a> MkgrdCalculatedRefineSourceExecutor<'a> {
    pub fn new(options: MkgrdCalculatedRefineSourceExecutorOptions<'a>) -> Self {
        Self { options }
    }

    pub fn run_source_branch_report(
        &self,
        step: &MkgrdRefineLoopStepIoPlan,
        source: &MkgrdRefineSourceIoPlan,
    ) -> io::Result<MkgrdCalculatedRefineSourceBranchReport> {
        if source.source != MkgrdRefineSource::CalculatedIterZero {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "MkgrdCalculatedRefineSourceExecutor only supports calculated iter-zero sources",
            ));
        }
        if source.area_judge_iter != 0 || source.get_contain_iter != 0 || source.getref_iter != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "calculated refine source must use iter zero; got area {} contain {} getref {}",
                    source.area_judge_iter, source.get_contain_iter, source.getref_iter
                ),
            ));
        }

        let mesh_kind = getcontain_mesh_kind_from_mesh_type(self.options.mesh_type)?;
        let area = run_area_judge_refine_grid_fortran_indexed(AreaJudgeRefineGridRunConfig {
            file_dir: self.options.file_dir,
            iter: source.area_judge_iter,
            calculated_refine: Some(self.options.calculated_refine),
            mask_refine_spc_type: "",
            mask_refine_ndm: 0,
            is_in_domain: &[],
            lon_vertex: self.options.lon_vertex,
            lat_vertex: self.options.lat_vertex,
            lon_i: self.options.lon_i,
            lat_i: self.options.lat_i,
            gridnum_perdegree: 0,
            nlons_source: self.options.lon_i.len().saturating_sub(1),
            nlats_source: self.options.lat_i.len().saturating_sub(1),
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
        let outputs =
            mkgrd_calculated_getref_output_refs(self.options.mesh_type, &source.threshold_outputs)?;
        let is_in_refine_sjx = getref_is_in_refine_from_contain(&contain_payload.is_in_area_ustr);
        let getref =
            run_getref_integrated_threshold_files_fortran_indexed(GetRefIntegratedFileRunConfig {
                mesh_type: self.options.mesh_type,
                threshold_dir: self.options.threshold_dir,
                contain_file: &source.contain_output,
                land_threshold_output: outputs.0,
                ocean_threshold_output: outputs.1,
                atmos_threshold_output: outputs.2,
                landtypes_global: self.options.landtypes_global,
                threshold_bounds: area.refine_step.bounds,
                is_in_refine_sjx: &is_in_refine_sjx,
                refine_onelayer_lnd: self.options.refine_onelayer_lnd,
                th_onelayer_lnd: self.options.th_onelayer_lnd,
                refine_twolayer_lnd: self.options.refine_twolayer_lnd,
                th_twolayer_lnd: self.options.th_twolayer_lnd,
                refine_onelayer_ocn: self.options.refine_onelayer_ocn,
                th_onelayer_ocn: self.options.th_onelayer_ocn,
                refine_onelayer_atmos: self.options.refine_onelayer_atmos,
                th_onelayer_atmos: self.options.th_onelayer_atmos,
                land_basic_config: self.options.land_basic_config,
                ocean_config: self.options.ocean_config,
                atmos_config: self.options.atmos_config,
            })?;

        Ok(MkgrdCalculatedRefineSourceBranchReport {
            area,
            contain,
            getref,
        })
    }
}

impl MkgrdRefineLoopExecutor for MkgrdCalculatedRefineSourceExecutor<'_> {
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
            "refine_loop geometry step is not implemented by MkgrdCalculatedRefineSourceExecutor",
        ))
    }

    fn run_final_quality_check(&mut self, plan: &MkgrdFinalQualityCheckIoPlan) -> io::Result<()> {
        run_mkgrd_final_quality_check(plan)
    }
}
