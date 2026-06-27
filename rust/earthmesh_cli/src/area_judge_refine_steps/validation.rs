use std::io;

use earthmesh_mesh::AreaJudgeSourceBounds;

use crate::grid_covers_area_judge_bounds_fortran_indexed;

/// Validate the `Area_judge`/`Area_judge_refine` containment rule.
pub fn validate_area_judge_refine_within_domain_fortran_indexed(
    is_in_refine: &[Vec<i32>],
    is_in_domain: &[Vec<i32>],
    bounds: AreaJudgeSourceBounds,
) -> io::Result<()> {
    grid_covers_area_judge_bounds_fortran_indexed("IsInRfArea_grid", is_in_refine, bounds)?;
    grid_covers_area_judge_bounds_fortran_indexed("IsInDmArea_grid", is_in_domain, bounds)?;

    for lat_index in bounds.maxlat_source..=bounds.minlat_source {
        for lon_index in bounds.minlon_source..=bounds.maxlon_source {
            if is_in_refine[lon_index][lat_index] != 0 && is_in_domain[lon_index][lat_index] == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("refine area exceeds domain area at lon {lon_index} lat {lat_index}"),
                ));
            }
        }
    }

    Ok(())
}
