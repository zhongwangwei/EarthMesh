use std::io;

use crate::*;

use super::writer::write_area_judge_selected_grid_report;

/// Run one `Area_judge_refine(iter)` step and write its selected refine grid.
pub fn run_area_judge_refine_grid_fortran_indexed(
    config: AreaJudgeRefineGridRunConfig<'_>,
) -> io::Result<AreaJudgeRefineGridRunReport> {
    let refine_step = run_area_judge_refine_fortran_indexed(
        config.file_dir,
        config.iter,
        config.calculated_refine,
        config.mask_refine_spc_type,
        config.mask_refine_ndm,
        config.is_in_domain,
        config.lon_vertex,
        config.lat_vertex,
        config.lon_i,
        config.lat_i,
        config.gridnum_perdegree,
        config.nlons_source,
        config.nlats_source,
    )?;
    let refine_write = write_area_judge_selected_grid_report(
        config.refine_output,
        &refine_step.is_in_refine,
        None,
        config.lon_i,
        config.lat_i,
        refine_step.bounds,
    )?;
    Ok(AreaJudgeRefineGridRunReport {
        refine_step,
        refine_write,
    })
}
