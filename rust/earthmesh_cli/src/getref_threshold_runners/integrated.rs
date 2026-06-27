use std::io;

use crate::*;

use super::{
    loc::run_getref_loc_mesh_threshold_files_fortran_indexed,
    single::run_getref_single_mesh_threshold_files_fortran_indexed,
};

/// Run the integrated calculated-threshold GetRef path from Area_judge
/// threshold NetCDF files and legacy contain NetCDF input through legacy
/// threshold NetCDF outputs.
pub fn run_getref_integrated_threshold_files_fortran_indexed(
    config: GetRefIntegratedFileRunConfig<'_>,
) -> io::Result<GetRefIntegratedFileRunReport> {
    let threshold_inputs = read_area_judge_threshold_inputs_fortran_indexed(
        AreaJudgeThresholdReadConfig {
            threshold_dir: config.threshold_dir,
            mesh_type: config.mesh_type,
            refine_onelayer_lnd: config.refine_onelayer_lnd,
            refine_twolayer_lnd: config.refine_twolayer_lnd,
            refine_onelayer_ocn: config.refine_onelayer_ocn,
            refine_onelayer_atmos: config.refine_onelayer_atmos,
        },
        config.landtypes_global,
        config.threshold_bounds,
    )?;

    let (single_mesh, loc_mesh) = {
        let land_onelayer_inputs = build_getref_onelayer_threshold_inputs(
            &threshold_inputs.land_onelayer,
            config.refine_onelayer_lnd,
            config.th_onelayer_lnd,
        )?;
        let land_twolayer_inputs = build_getref_twolayer_threshold_inputs(
            &threshold_inputs.land_twolayer,
            config.refine_twolayer_lnd,
            config.th_twolayer_lnd,
        )?;
        let ocean_onelayer_inputs = build_getref_onelayer_threshold_inputs(
            &threshold_inputs.ocean_onelayer,
            config.refine_onelayer_ocn,
            config.th_onelayer_ocn,
        )?;
        let atmos_onelayer_inputs = build_getref_onelayer_threshold_inputs(
            &threshold_inputs.atmos_onelayer,
            config.refine_onelayer_atmos,
            config.th_onelayer_atmos,
        )?;

        if matches!(config.mesh_type, "LOCmesh" | "earthmesh") {
            (
                None,
                Some(run_getref_loc_mesh_threshold_files_fortran_indexed(
                    GetRefLocMeshFileRunConfig {
                        contain_file: config.contain_file,
                        land_threshold_output: config.land_threshold_output,
                        ocean_threshold_output: config.ocean_threshold_output,
                        atmos_threshold_output: config.atmos_threshold_output,
                        is_in_refine_sjx: config.is_in_refine_sjx,
                        landtypes: &threshold_inputs.landtypes,
                        land_basic_config: config.land_basic_config,
                        land_onelayer_inputs: &land_onelayer_inputs,
                        land_twolayer_inputs: &land_twolayer_inputs,
                        ocean_config: config.ocean_config,
                        ocean_onelayer_inputs: &ocean_onelayer_inputs,
                        atmos_config: config.atmos_config,
                        atmos_onelayer_inputs: &atmos_onelayer_inputs,
                    },
                )?),
            )
        } else {
            (
                Some(run_getref_single_mesh_threshold_files_fortran_indexed(
                    GetRefSingleMeshFileRunConfig {
                        mesh_type: config.mesh_type,
                        contain_file: config.contain_file,
                        land_threshold_output: config.land_threshold_output,
                        ocean_threshold_output: config.ocean_threshold_output,
                        atmos_threshold_output: config.atmos_threshold_output,
                        is_in_refine_sjx: config.is_in_refine_sjx,
                        landtypes: &threshold_inputs.landtypes,
                        land_basic_config: config.land_basic_config,
                        land_onelayer_inputs: &land_onelayer_inputs,
                        land_twolayer_inputs: &land_twolayer_inputs,
                        ocean_config: config.ocean_config,
                        ocean_onelayer_inputs: &ocean_onelayer_inputs,
                        atmos_config: config.atmos_config,
                        atmos_onelayer_inputs: &atmos_onelayer_inputs,
                    },
                )?),
                None,
            )
        }
    };

    Ok(GetRefIntegratedFileRunReport {
        threshold_inputs,
        single_mesh,
        loc_mesh,
    })
}
