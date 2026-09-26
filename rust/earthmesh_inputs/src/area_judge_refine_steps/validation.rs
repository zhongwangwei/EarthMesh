use std::io;

use earthmesh_mesh::AreaJudgeSourceBounds;

use crate::grid_covers_area_judge_bounds_one_based;

/// Validate the `Area_judge`/`Area_judge_refine` containment rule.
pub fn validate_area_judge_refine_within_domain_one_based<R, D>(
    is_in_refine: &[Vec<R>],
    is_in_domain: &[Vec<D>],
    bounds: AreaJudgeSourceBounds,
) -> io::Result<()>
where
    R: Copy + Into<i32>,
    D: Copy + Into<i32>,
{
    grid_covers_area_judge_bounds_one_based("IsInRfArea_grid", is_in_refine, bounds)?;
    grid_covers_area_judge_bounds_one_based("IsInDmArea_grid", is_in_domain, bounds)?;

    for lat_index in bounds.maxlat_source..=bounds.minlat_source {
        for lon_index in bounds.minlon_source..=bounds.maxlon_source {
            if is_in_refine[lon_index][lat_index].into() != 0
                && is_in_domain[lon_index][lat_index].into() == 0
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("refine area exceeds domain area at lon {lon_index} lat {lat_index}"),
                ));
            }
        }
    }

    Ok(())
}
