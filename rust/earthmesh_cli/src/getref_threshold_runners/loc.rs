use std::io;

use crate::*;

use super::support::{
    contain_rows_to_fortran_indexed, contain_rows_to_fortran_indexed_with_empty_width,
    required_getref_output_path,
};

/// Run the calculated-threshold GetRef path for LOCmesh from legacy mixed
/// contain NetCDF input through legacy component threshold NetCDF outputs.
pub fn run_getref_loc_mesh_threshold_files_fortran_indexed(
    config: GetRefLocMeshFileRunConfig<'_>,
) -> io::Result<GetRefLocMeshFileRunReport> {
    let contain = read_contain_netcdf(config.contain_file)?;
    let loc_id = contain_rows_to_fortran_indexed("LOC ustr_id", &contain.ustr_id)?;
    let loc_ii =
        contain_rows_to_fortran_indexed_with_empty_width("LOC ustr_ii", &contain.ustr_ii, 3)?;
    let threshold = calculate_getref_loc_threshold_reports_fortran_indexed(
        config.is_in_refine_sjx,
        &loc_id,
        &loc_ii,
        config.landtypes,
        config.land_basic_config,
        config.land_onelayer_inputs,
        config.land_twolayer_inputs,
        config.ocean_config,
        config.ocean_onelayer_inputs,
        config.atmos_config,
        config.atmos_onelayer_inputs,
    )?;

    let writes = GetRefThresholdFileWrites {
        land: threshold
            .land
            .as_ref()
            .map(|report| {
                write_getref_land_threshold_netcdf(
                    required_getref_output_path(config.land_threshold_output, "land")?,
                    report,
                )
            })
            .transpose()?,
        ocean: threshold
            .ocean
            .as_ref()
            .map(|report| {
                write_getref_ocean_threshold_netcdf(
                    required_getref_output_path(config.ocean_threshold_output, "ocean")?,
                    report,
                )
            })
            .transpose()?,
        atmos: threshold
            .atmos
            .as_ref()
            .map(|report| {
                write_getref_atmos_threshold_netcdf(
                    required_getref_output_path(config.atmos_threshold_output, "atmos")?,
                    report,
                )
            })
            .transpose()?,
    };

    Ok(GetRefLocMeshFileRunReport {
        contain,
        threshold,
        writes,
    })
}
