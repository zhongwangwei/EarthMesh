use std::io;

use earthmesh_mesh::AreaJudgeSourceBounds;

use crate::*;

/// Copy calculated refine state into the active refine state for
/// `MOD_Area_judge.F90:Area_judge_refine(iter == 0)`.
pub fn activate_area_judge_calculated_refine_fortran_indexed(
    is_in_refine_calculated: &[Vec<i32>],
    bounds: AreaJudgeSourceBounds,
) -> io::Result<AreaJudgeRefineActivationReport> {
    if bounds.maxlon_source < bounds.minlon_source || bounds.minlat_source < bounds.maxlat_source {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "invalid Area_judge refine bounds lon {}..{} lat {}..{}",
                bounds.minlon_source,
                bounds.maxlon_source,
                bounds.maxlat_source,
                bounds.minlat_source
            ),
        ));
    }
    grid_covers_area_judge_bounds_fortran_indexed(
        "IsInRfArea_cal_grid",
        is_in_refine_calculated,
        bounds,
    )?;

    let nlons_select = bounds.maxlon_source - bounds.minlon_source + 1;
    let nlats_select = bounds.minlat_source - bounds.maxlat_source + 1;
    let selected_cells = (bounds.maxlat_source..=bounds.minlat_source)
        .flat_map(|lat_index| {
            (bounds.minlon_source..=bounds.maxlon_source)
                .map(move |lon_index| (lon_index, lat_index))
        })
        .filter(|(lon_index, lat_index)| is_in_refine_calculated[*lon_index][*lat_index] != 0)
        .count();

    Ok(AreaJudgeRefineActivationReport {
        is_in_refine: is_in_refine_calculated.to_vec(),
        bounds,
        nlons_select,
        nlats_select,
        selected_cells,
    })
}
