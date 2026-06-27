use std::io;

use crate::*;

use super::support::{
    contain_rows_to_fortran_indexed, contain_rows_to_fortran_indexed_with_empty_width,
    getref_ustr_ii_width_for_mesh_type, required_getref_output_path,
};

/// Run the calculated-threshold GetRef path for a non-LOC mesh from legacy
/// contain NetCDF input through legacy threshold NetCDF output.
pub fn run_getref_single_mesh_threshold_files_fortran_indexed(
    config: GetRefSingleMeshFileRunConfig<'_>,
) -> io::Result<GetRefSingleMeshFileRunReport> {
    let contain = read_contain_netcdf(config.contain_file)?;
    let mp_id = contain_rows_to_fortran_indexed("ustr_id", &contain.ustr_id)?;
    let mp_ii = contain_rows_to_fortran_indexed_with_empty_width(
        "ustr_ii",
        &contain.ustr_ii,
        getref_ustr_ii_width_for_mesh_type(config.mesh_type)?,
    )?;
    let threshold = calculate_getref_single_mesh_threshold_reports_fortran_indexed(
        config.mesh_type,
        config.is_in_refine_sjx,
        &mp_id,
        &mp_ii,
        config.landtypes,
        config.land_basic_config,
        config.land_onelayer_inputs,
        config.land_twolayer_inputs,
        config.ocean_config,
        config.ocean_onelayer_inputs,
        config.atmos_config,
        config.atmos_onelayer_inputs,
    )?;

    let writes = match config.mesh_type {
        "landmesh" => GetRefThresholdFileWrites {
            land: Some(write_getref_land_threshold_netcdf(
                required_getref_output_path(config.land_threshold_output, "land")?,
                threshold.land.as_ref().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "landmesh run missing land report",
                    )
                })?,
            )?),
            ocean: None,
            atmos: None,
        },
        "oceanmesh" => GetRefThresholdFileWrites {
            land: None,
            ocean: Some(write_getref_ocean_threshold_netcdf(
                required_getref_output_path(config.ocean_threshold_output, "ocean")?,
                threshold.ocean.as_ref().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "oceanmesh run missing ocean report",
                    )
                })?,
            )?),
            atmos: None,
        },
        "atmosmesh" => GetRefThresholdFileWrites {
            land: None,
            ocean: None,
            atmos: Some(write_getref_atmos_threshold_netcdf(
                required_getref_output_path(config.atmos_threshold_output, "atmos")?,
                threshold.atmos.as_ref().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "atmosmesh run missing atmosphere report",
                    )
                })?,
            )?),
        },
        "LOCmesh" => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "LOCmesh requires LOC-specific GetRef file orchestration",
            ));
        }
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unsupported GetRef mesh_type {other}"),
            ));
        }
    };

    Ok(GetRefSingleMeshFileRunReport {
        contain,
        threshold,
        writes,
    })
}
