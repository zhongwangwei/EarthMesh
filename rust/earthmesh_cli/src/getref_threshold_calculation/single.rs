use std::io;

use crate::*;

/// Calculate and aggregate the top-level GetRef report for non-LOC mesh types.
#[allow(clippy::too_many_arguments)]
pub fn calculate_getref_single_mesh_threshold_reports_fortran_indexed(
    mesh_type: &str,
    is_in_refine_sjx: &[i32],
    mp_id: &[Vec<i32>],
    mp_ii: &[Vec<i32>],
    landtypes: &[Vec<i32>],
    land_basic_config: GetRefLandBasicConfig,
    land_onelayer_inputs: &[GetRefOneLayerThresholdInput<'_>],
    land_twolayer_inputs: &[GetRefTwoLayerThresholdInput<'_>],
    ocean_config: GetRefOceanThresholdConfig,
    ocean_onelayer_inputs: &[GetRefOneLayerThresholdInput<'_>],
    atmos_config: GetRefAtmosThresholdConfig,
    atmos_onelayer_inputs: &[GetRefOneLayerThresholdInput<'_>],
) -> io::Result<GetRefSingleMeshThresholdReports> {
    let (land, ocean, atmos) = match mesh_type {
        "landmesh" => (
            Some(calculate_getref_land_threshold_report_fortran_indexed(
                is_in_refine_sjx,
                mp_id,
                mp_ii,
                landtypes,
                land_basic_config,
                land_onelayer_inputs,
                land_twolayer_inputs,
            )?),
            None,
            None,
        ),
        "oceanmesh" => (
            None,
            Some(calculate_getref_ocean_threshold_report_fortran_indexed(
                is_in_refine_sjx,
                mp_id,
                mp_ii,
                landtypes,
                ocean_config,
                ocean_onelayer_inputs,
            )?),
            None,
        ),
        "atmos" | "atmosmesh" => (
            None,
            None,
            Some(calculate_getref_atmos_threshold_report_fortran_indexed(
                is_in_refine_sjx,
                mp_id,
                mp_ii,
                landtypes,
                atmos_config,
                atmos_onelayer_inputs,
            )?),
        ),
        "LOCmesh" => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "LOCmesh requires calculate_getref_loc_threshold_reports_fortran_indexed",
            ));
        }
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unsupported GetRef mesh_type {other}"),
            ));
        }
    };
    let aggregate_num_vertex = match mesh_type {
        "landmesh" => land_basic_config.num_vertex,
        "oceanmesh" => ocean_config.num_vertex,
        "atmosmesh" => atmos_config.num_vertex,
        _ => unreachable!("validated non-LOC mesh_type"),
    };
    let aggregate = aggregate_getref_threshold_reports_fortran_indexed(
        aggregate_num_vertex,
        land.as_ref(),
        ocean.as_ref(),
        atmos.as_ref(),
    )?;
    Ok(GetRefSingleMeshThresholdReports {
        mesh_type: mesh_type.to_string(),
        land,
        ocean,
        atmos,
        aggregate,
    })
}
