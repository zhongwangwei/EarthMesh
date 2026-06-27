use std::io;

use earthmesh_mesh::AreaJudgeSourceBounds;

use crate::*;

/// Build `seaorland` from `IsInDmArea_grid` and `landtypes_global`.
pub fn build_area_judge_seaorland_fortran_indexed(
    is_in_domain: &[Vec<i32>],
    landtypes_global: &[Vec<i32>],
    bounds: AreaJudgeSourceBounds,
    mesh_type: &str,
    refine: bool,
) -> io::Result<AreaJudgeSeaOrLandReport> {
    grid_covers_area_judge_bounds_fortran_indexed("IsInDmArea_grid", is_in_domain, bounds)?;
    grid_covers_area_judge_bounds_fortran_indexed("landtypes_global", landtypes_global, bounds)?;

    let nlons_source = is_in_domain.len().saturating_sub(1);
    let nlats_source = is_in_domain
        .get(1)
        .map(|row| row.len().saturating_sub(1))
        .unwrap_or(0);
    let mut seaorland = vec![vec![0_i32; nlats_source + 1]; nlons_source + 1];

    if matches!(mesh_type, "atmos" | "atmosmesh") && !refine {
        return Ok(AreaJudgeSeaOrLandReport {
            seaorland,
            sum_land_grid: 0,
        });
    }

    let mut sum_land_grid = 0_i32;
    for lon_index in bounds.minlon_source..=bounds.maxlon_source {
        let domain_row = &is_in_domain[lon_index];
        let landtype_row = &landtypes_global[lon_index];
        let seaorland_row = &mut seaorland[lon_index];
        for lat_index in bounds.maxlat_source..=bounds.minlat_source {
            if domain_row[lat_index] != 0 && landtype_row[lat_index] != 0 {
                seaorland_row[lat_index] = 1;
                sum_land_grid += 1;
            }
        }
    }

    Ok(AreaJudgeSeaOrLandReport {
        seaorland,
        sum_land_grid,
    })
}
