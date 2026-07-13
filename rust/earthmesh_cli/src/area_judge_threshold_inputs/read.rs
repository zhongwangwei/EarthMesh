use std::io;

use earthmesh_mesh::AreaJudgeSourceBounds;

use super::{
    groups::{
        read_area_judge_threshold_2d_group_one_based,
        read_area_judge_threshold_2layer_group_one_based,
    },
    AREA_JUDGE_ATMOS_ONELAYER_NAMES, AREA_JUDGE_LAND_ONELAYER_NAMES,
    AREA_JUDGE_LAND_TWOLAYER_NAMES, AREA_JUDGE_OCEAN_ONELAYER_NAMES,
};
use crate::{
    grid_covers_area_judge_bounds_one_based, AreaJudgeThresholdInputsReport,
    AreaJudgeThresholdReadConfig,
};

fn crop_area_judge_landtypes_one_based(
    landtypes_global: &[Vec<i32>],
    bounds: AreaJudgeSourceBounds,
) -> io::Result<Vec<Vec<i32>>> {
    grid_covers_area_judge_bounds_one_based("landtypes_global", landtypes_global, bounds)?;
    let nlons_select = bounds.maxlon_source - bounds.minlon_source + 1;
    let nlats_select = bounds.minlat_source - bounds.maxlat_source + 1;
    let mut landtypes = vec![vec![0; nlats_select + 1]; nlons_select + 1];
    for lon_offset in 0..nlons_select {
        for lat_offset in 0..nlats_select {
            landtypes[lon_offset + 1][lat_offset + 1] = landtypes_global
                [bounds.minlon_source + lon_offset][bounds.maxlat_source + lat_offset];
        }
    }
    Ok(landtypes)
}

/// Read and crop threshold inputs after calculated `Area_judge` refine bounds are known.
pub fn read_area_judge_threshold_inputs_one_based(
    config: AreaJudgeThresholdReadConfig<'_>,
    landtypes_global: &[Vec<i32>],
    bounds: AreaJudgeSourceBounds,
) -> io::Result<AreaJudgeThresholdInputsReport> {
    if bounds.maxlon_source < bounds.minlon_source || bounds.minlat_source < bounds.maxlat_source {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "invalid Area_judge threshold bounds lon {}..{} lat {}..{}",
                bounds.minlon_source,
                bounds.maxlon_source,
                bounds.maxlat_source,
                bounds.minlat_source
            ),
        ));
    }
    let nlons_select = bounds.maxlon_source - bounds.minlon_source + 1;
    let nlats_select = bounds.minlat_source - bounds.maxlat_source + 1;
    let landtypes = crop_area_judge_landtypes_one_based(landtypes_global, bounds)?;

    let mut land_onelayer = Vec::new();
    let mut land_twolayer = Vec::new();
    let mut ocean_onelayer = Vec::new();
    let mut atmos_onelayer = Vec::new();

    match config.mesh_type {
        "landmesh" => {
            land_onelayer = read_area_judge_threshold_2d_group_one_based(
                config.threshold_dir,
                &AREA_JUDGE_LAND_ONELAYER_NAMES,
                config.refine_onelayer_lnd,
                bounds,
            )?;
            land_twolayer = read_area_judge_threshold_2layer_group_one_based(
                config.threshold_dir,
                &AREA_JUDGE_LAND_TWOLAYER_NAMES,
                config.refine_twolayer_lnd,
                bounds,
            )?;
        }
        "oceanmesh" => {
            ocean_onelayer = read_area_judge_threshold_2d_group_one_based(
                config.threshold_dir,
                &AREA_JUDGE_OCEAN_ONELAYER_NAMES,
                config.refine_onelayer_ocn,
                bounds,
            )?;
        }
        "atmos" | "atmosmesh" => {
            atmos_onelayer = read_area_judge_threshold_2d_group_one_based(
                config.threshold_dir,
                &AREA_JUDGE_ATMOS_ONELAYER_NAMES,
                config.refine_onelayer_atmos,
                bounds,
            )?;
        }
        "LOCmesh" | "earthmesh" => {
            land_onelayer = read_area_judge_threshold_2d_group_one_based(
                config.threshold_dir,
                &AREA_JUDGE_LAND_ONELAYER_NAMES,
                config.refine_onelayer_lnd,
                bounds,
            )?;
            land_twolayer = read_area_judge_threshold_2layer_group_one_based(
                config.threshold_dir,
                &AREA_JUDGE_LAND_TWOLAYER_NAMES,
                config.refine_twolayer_lnd,
                bounds,
            )?;
            ocean_onelayer = read_area_judge_threshold_2d_group_one_based(
                config.threshold_dir,
                &AREA_JUDGE_OCEAN_ONELAYER_NAMES,
                config.refine_onelayer_ocn,
                bounds,
            )?;
            atmos_onelayer = read_area_judge_threshold_2d_group_one_based(
                config.threshold_dir,
                &AREA_JUDGE_ATMOS_ONELAYER_NAMES,
                config.refine_onelayer_atmos,
                bounds,
            )?;
        }
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unsupported Area_judge threshold mesh_type {other}"),
            ));
        }
    }

    Ok(AreaJudgeThresholdInputsReport {
        bounds,
        nlons_select,
        nlats_select,
        landtypes,
        land_onelayer,
        land_twolayer,
        ocean_onelayer,
        atmos_onelayer,
    })
}
