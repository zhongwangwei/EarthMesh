use std::io;

use super::netcdf::read_area_judge_grid_netcdf;
use super::types::{
    AreaJudgeExpandedGridReport, AreaJudgeGridPayload, AreaJudgeRestartGridRunConfig,
    AreaJudgeRestartGridRunReport,
};
use super::validate::validate_area_judge_grid_payload;

pub fn run_area_judge_restart_grid_one_based(
    config: AreaJudgeRestartGridRunConfig<'_>,
) -> io::Result<AreaJudgeRestartGridRunReport> {
    let payload = read_area_judge_grid_netcdf(config.input)?;
    let expanded = expand_area_judge_grid_payload_one_based(
        &payload,
        config.nlons_source,
        config.nlats_source,
    )?;
    Ok(AreaJudgeRestartGridRunReport {
        input: config.input.to_path_buf(),
        payload,
        expanded,
    })
}

pub fn expand_area_judge_grid_payload_one_based(
    payload: &AreaJudgeGridPayload,
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<AreaJudgeExpandedGridReport> {
    validate_area_judge_grid_payload(payload)?;
    let seaorland_select = payload.seaorland_select.as_ref().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Area_judge restart payload requires seaorland_select",
        )
    })?;
    if payload.bounds.maxlon_source > nlons_source || payload.bounds.minlat_source > nlats_source {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Area_judge restart bounds lon {}..{} lat {}..{} exceed source dimensions {}x{}",
                payload.bounds.minlon_source,
                payload.bounds.maxlon_source,
                payload.bounds.maxlat_source,
                payload.bounds.minlat_source,
                nlons_source,
                nlats_source
            ),
        ));
    }

    let mut is_in_domain = vec![vec![false; nlats_source + 1]; nlons_source + 1];
    let mut seaorland = vec![vec![false; nlats_source + 1]; nlons_source + 1];
    for (lon_offset, lon_index) in
        (payload.bounds.minlon_source..=payload.bounds.maxlon_source).enumerate()
    {
        for (lat_offset, lat_index) in
            (payload.bounds.maxlat_source..=payload.bounds.minlat_source).enumerate()
        {
            is_in_domain[lon_index][lat_index] =
                payload.is_in_area_select[lon_offset][lat_offset] != 0;
            seaorland[lon_index][lat_index] = seaorland_select[lon_offset][lat_offset] != 0;
        }
    }

    Ok(AreaJudgeExpandedGridReport {
        is_in_domain,
        seaorland,
        bounds: payload.bounds,
        nlons_select: payload.bounds.maxlon_source - payload.bounds.minlon_source + 1,
        nlats_select: payload.bounds.minlat_source - payload.bounds.maxlat_source + 1,
    })
}
