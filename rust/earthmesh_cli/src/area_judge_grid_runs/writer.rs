use crate::select_area_judge_grid_one_based;
use crate::write_area_judge_grid_netcdf;
use crate::AreaJudgeGridWriteReport;
use std::io;
use std::path::Path;

use earthmesh_mesh::AreaJudgeSourceBounds;

pub(crate) fn write_area_judge_selected_grid_report(
    output: &Path,
    is_in_area: &[Vec<i32>],
    seaorland: Option<&[Vec<i32>]>,
    lon_i: &[f64],
    lat_i: &[f64],
    bounds: AreaJudgeSourceBounds,
) -> io::Result<AreaJudgeGridWriteReport> {
    let payload = select_area_judge_grid_one_based(is_in_area, seaorland, lon_i, lat_i, bounds)?;
    write_area_judge_grid_netcdf(output, &payload)?;
    Ok(AreaJudgeGridWriteReport {
        output: output.to_path_buf(),
        bounds: payload.bounds,
        nlons_select: payload.longitude.len(),
        nlats_select: payload.latitude.len(),
        selected_cells: count_selected_cells_zero_based(&payload.is_in_area_select),
        has_seaorland: payload.seaorland_select.is_some(),
    })
}

fn count_selected_cells_zero_based(grid: &[Vec<i32>]) -> usize {
    grid.iter()
        .flat_map(|row| row.iter())
        .filter(|value| **value != 0)
        .count()
}
