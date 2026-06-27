use std::io;

use crate::getref_threshold_aggregation::empty_getref_threshold_aggregation_fortran_indexed;
use crate::*;

use super::helpers::{has_getref_onelayer_thresholds, has_getref_twolayer_thresholds};

/// Split mixed LOC containment, calculate enabled land/ocean/atmos threshold
/// reports, and aggregate them into the final top-level GetRef threshold
/// matrix.
#[allow(clippy::too_many_arguments)]
pub fn calculate_getref_loc_threshold_reports_fortran_indexed(
    is_in_refine_sjx: &[i32],
    loc_id: &[Vec<i32>],
    loc_ii: &[Vec<i32>],
    landtypes: &[Vec<i32>],
    land_basic_config: GetRefLandBasicConfig,
    land_onelayer_inputs: &[GetRefOneLayerThresholdInput<'_>],
    land_twolayer_inputs: &[GetRefTwoLayerThresholdInput<'_>],
    ocean_config: GetRefOceanThresholdConfig,
    ocean_onelayer_inputs: &[GetRefOneLayerThresholdInput<'_>],
    atmos_config: GetRefAtmosThresholdConfig,
    atmos_onelayer_inputs: &[GetRefOneLayerThresholdInput<'_>],
) -> io::Result<GetRefLocThresholdReports> {
    let num_vertex = land_basic_config.num_vertex;
    if ocean_config.num_vertex != num_vertex {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "ocean num_vertex {} must match land num_vertex {num_vertex}",
                ocean_config.num_vertex
            ),
        ));
    }
    if atmos_config.num_vertex != num_vertex {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "atmos num_vertex {} must match land num_vertex {num_vertex}",
                atmos_config.num_vertex
            ),
        ));
    }
    if ocean_config.maxlc != land_basic_config.maxlc {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "ocean maxlc {} must match land maxlc {}",
                ocean_config.maxlc, land_basic_config.maxlc
            ),
        ));
    }
    if atmos_config.maxlc != land_basic_config.maxlc {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "atmos maxlc {} must match land maxlc {}",
                atmos_config.maxlc, land_basic_config.maxlc
            ),
        ));
    }
    let split = split_getref_loc_containment_fortran_indexed(loc_id, loc_ii, num_vertex)?;

    let land_enabled = land_basic_config.refine_num_landtypes
        || land_basic_config.refine_area_mainland
        || has_getref_onelayer_thresholds(land_onelayer_inputs)
        || has_getref_twolayer_thresholds(land_twolayer_inputs);
    let ocean_enabled =
        ocean_config.refine_sea_ratio || has_getref_onelayer_thresholds(ocean_onelayer_inputs);
    let atmos_enabled = has_getref_onelayer_thresholds(atmos_onelayer_inputs);

    let land = if land_enabled {
        Some(calculate_getref_land_threshold_report_fortran_indexed(
            is_in_refine_sjx,
            &split.land.mp_id,
            &split.land.mp_ii,
            landtypes,
            land_basic_config,
            land_onelayer_inputs,
            land_twolayer_inputs,
        )?)
    } else {
        None
    };
    let ocean = if ocean_enabled {
        Some(calculate_getref_ocean_threshold_report_fortran_indexed(
            is_in_refine_sjx,
            &split.ocean.mp_id,
            &split.ocean.mp_ii,
            landtypes,
            ocean_config,
            ocean_onelayer_inputs,
        )?)
    } else {
        None
    };
    let atmos = if atmos_enabled {
        Some(calculate_getref_atmos_threshold_report_fortran_indexed(
            is_in_refine_sjx,
            &split.atmos.mp_id,
            &split.atmos.mp_ii,
            landtypes,
            atmos_config,
            atmos_onelayer_inputs,
        )?)
    } else {
        None
    };
    let aggregate = if land.is_none() && ocean.is_none() && atmos.is_none() {
        empty_getref_threshold_aggregation_fortran_indexed(num_vertex, split.sjx_points)?
    } else {
        aggregate_getref_threshold_reports_fortran_indexed(
            num_vertex,
            land.as_ref(),
            ocean.as_ref(),
            atmos.as_ref(),
        )?
    };

    Ok(GetRefLocThresholdReports {
        split,
        land,
        ocean,
        atmos,
        aggregate,
    })
}
