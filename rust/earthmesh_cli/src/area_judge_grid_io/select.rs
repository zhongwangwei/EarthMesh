use std::io;

use earthmesh_mesh::AreaJudgeSourceBounds;

use crate::require_len;

use super::types::AreaJudgeGridPayload;
use super::validate::{
    grid_covers_area_judge_bounds_fortran_indexed, validate_area_judge_grid_payload,
};

pub fn select_area_judge_grid_fortran_indexed(
    is_in_area: &[Vec<i32>],
    seaorland: Option<&[Vec<i32>]>,
    lon_i: &[f64],
    lat_i: &[f64],
    bounds: AreaJudgeSourceBounds,
) -> io::Result<AreaJudgeGridPayload> {
    if bounds.maxlon_source < bounds.minlon_source || bounds.minlat_source < bounds.maxlat_source {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "invalid Area_judge source bounds lon {}..{} lat {}..{}",
                bounds.minlon_source,
                bounds.maxlon_source,
                bounds.maxlat_source,
                bounds.minlat_source
            ),
        ));
    }
    grid_covers_area_judge_bounds_fortran_indexed("IsInArea", is_in_area, bounds)?;
    if let Some(seaorland) = seaorland {
        grid_covers_area_judge_bounds_fortran_indexed("seaorland", seaorland, bounds)?;
    }
    require_len("longitude source", lon_i.len(), bounds.maxlon_source + 1)?;
    require_len("latitude source", lat_i.len(), bounds.minlat_source + 1)?;

    let longitude = (bounds.minlon_source..=bounds.maxlon_source)
        .map(|lon_index| lon_i[lon_index])
        .collect::<Vec<_>>();
    let latitude = (bounds.maxlat_source..=bounds.minlat_source)
        .map(|lat_index| lat_i[lat_index])
        .collect::<Vec<_>>();
    let is_in_area_select = select_i32_matrix_fortran_indexed(is_in_area, bounds);
    let seaorland_select =
        seaorland.map(|values| select_i32_matrix_fortran_indexed(values, bounds));

    let payload = AreaJudgeGridPayload {
        bounds,
        longitude,
        latitude,
        is_in_area_select,
        seaorland_select,
    };
    validate_area_judge_grid_payload(&payload)?;
    Ok(payload)
}

fn select_i32_matrix_fortran_indexed(
    values: &[Vec<i32>],
    bounds: AreaJudgeSourceBounds,
) -> Vec<Vec<i32>> {
    (bounds.minlon_source..=bounds.maxlon_source)
        .map(|lon_index| {
            (bounds.maxlat_source..=bounds.minlat_source)
                .map(|lat_index| values[lon_index][lat_index])
                .collect::<Vec<_>>()
        })
        .collect()
}
