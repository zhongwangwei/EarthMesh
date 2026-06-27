use std::io;

use crate::*;

use super::writer::write_area_judge_selected_grid_report;

/// Compose the restart `Area_judge` branch, activate iter-zero calculated
/// refine when available, and write selected grid payloads for restart-style
/// downstream consumers.
pub fn run_area_judge_restart_grids_fortran_indexed(
    config: AreaJudgeRestartGridsRunConfig<'_>,
) -> io::Result<AreaJudgeRestartGridsRunReport> {
    let area = build_area_judge_restart_fortran_indexed(
        config.file_dir,
        config.restart_input,
        config.mask_patch,
        config.refine,
        config.calculated_refine,
        config.lon_vertex,
        config.lat_vertex,
        config.lon_i,
        config.lat_i,
        config.gridnum_perdegree,
        config.nlons_source,
        config.nlats_source,
    )?;

    let domain_write = config
        .domain_output
        .map(|output| {
            write_area_judge_selected_grid_report(
                output,
                &area.domain.is_in_domain,
                Some(&area.seaorland.seaorland),
                config.lon_i,
                config.lat_i,
                area.domain.bounds,
            )
        })
        .transpose()?;

    let refine_step = if config.refine {
        area.calculated_refine
            .as_ref()
            .map(|calculated| {
                run_area_judge_refine_fortran_indexed(
                    config.file_dir,
                    0,
                    Some((&calculated.is_in_area, calculated.bounds)),
                    "",
                    0,
                    &area.domain.is_in_domain,
                    config.lon_vertex,
                    config.lat_vertex,
                    config.lon_i,
                    config.lat_i,
                    config.gridnum_perdegree,
                    config.nlons_source,
                    config.nlats_source,
                )
            })
            .transpose()?
    } else {
        None
    };

    let refine_write = match (config.refine_output, refine_step.as_ref()) {
        (Some(output), Some(refine)) => Some(write_area_judge_selected_grid_report(
            output,
            &refine.is_in_refine,
            None,
            config.lon_i,
            config.lat_i,
            refine.bounds,
        )?),
        (Some(_), None) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "restart Area_judge refine output requires calculated iter-zero refine state",
            ));
        }
        (None, _) => None,
    };

    Ok(AreaJudgeRestartGridsRunReport {
        area,
        refine_step,
        domain_write,
        refine_write,
    })
}
