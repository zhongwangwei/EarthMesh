use std::io;

use earthmesh_mesh::AreaJudgeSourceBounds;

use crate::require_len;

use super::types::AreaJudgeGridPayload;
use super::validate::{grid_covers_area_judge_bounds_one_based, validate_area_judge_grid_payload};

pub fn select_area_judge_grid_one_based<T>(
    is_in_area: &[Vec<T>],
    seaorland: Option<&[Vec<bool>]>,
    lon_i: &[f64],
    lat_i: &[f64],
    bounds: AreaJudgeSourceBounds,
) -> io::Result<AreaJudgeGridPayload>
where
    T: Copy + Into<i32>,
{
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
    grid_covers_area_judge_bounds_one_based("IsInArea", is_in_area, bounds)?;
    if let Some(seaorland) = seaorland {
        grid_covers_area_judge_bounds_one_based("seaorland", seaorland, bounds)?;
    }
    require_len("longitude source", lon_i.len(), bounds.maxlon_source + 1)?;
    require_len("latitude source", lat_i.len(), bounds.minlat_source + 1)?;

    let longitude = (bounds.minlon_source..=bounds.maxlon_source)
        .map(|lon_index| lon_i[lon_index])
        .collect::<Vec<_>>();
    let latitude = (bounds.maxlat_source..=bounds.minlat_source)
        .map(|lat_index| lat_i[lat_index])
        .collect::<Vec<_>>();
    let is_in_area_select = select_binary_compatible_matrix_one_based(is_in_area, bounds);
    let seaorland_select =
        seaorland.map(|values| select_bool_matrix_as_i32_one_based(values, bounds));

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

fn select_binary_compatible_matrix_one_based<T>(
    values: &[Vec<T>],
    bounds: AreaJudgeSourceBounds,
) -> Vec<Vec<i32>>
where
    T: Copy + Into<i32>,
{
    (bounds.minlon_source..=bounds.maxlon_source)
        .map(|lon_index| {
            (bounds.maxlat_source..=bounds.minlat_source)
                .map(|lat_index| values[lon_index][lat_index].into())
                .collect::<Vec<_>>()
        })
        .collect()
}

fn select_bool_matrix_as_i32_one_based(
    values: &[Vec<bool>],
    bounds: AreaJudgeSourceBounds,
) -> Vec<Vec<i32>> {
    (bounds.minlon_source..=bounds.maxlon_source)
        .map(|lon_index| {
            (bounds.maxlat_source..=bounds.minlat_source)
                .map(|lat_index| i32::from(values[lon_index][lat_index]))
                .collect::<Vec<_>>()
        })
        .collect()
}
