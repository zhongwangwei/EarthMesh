use std::io;

use crate::{
    matrix_width, patchtype_indices, usize_from_i32_nonnegative, usize_from_i32_positive,
    validate_contain_mesh, ContainMesh, LandPatchtypes,
};

/// Pure-data port of the `MOD_mask_postproc.F90:mask_postproc_Lnd`
/// `patchtypes_make` loop.
///
/// Rust row `0` corresponds to Fortran row `1`.  `seaorland` is the selected
/// domain land mask in the same row-major layout as `patchtypes_select`.
pub fn build_land_patchtypes_fortran_indexed(
    contain: &ContainMesh,
    seaorland: &[Vec<i32>],
    minlon_dm_area: i32,
    maxlat_dm_area: i32,
    nlons_dm_select: usize,
    nlats_dm_select: usize,
) -> io::Result<LandPatchtypes> {
    validate_contain_mesh(contain)?;
    let dim_a = matrix_width("ustr_id", &contain.ustr_id)?;
    let dim_b = matrix_width("ustr_ii", &contain.ustr_ii)?;
    let seaorland_width = matrix_width("seaorland", seaorland)?;
    if dim_a < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "land patchtype construction requires ustr_id rows with at least two columns",
        ));
    }
    if dim_b < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "land patchtype construction requires ustr_ii rows with at least two columns",
        ));
    }
    if seaorland.len() != nlons_dm_select || seaorland_width != nlats_dm_select {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "seaorland shape {}x{} must match patchtype grid {nlons_dm_select}x{nlats_dm_select}",
                seaorland.len(),
                seaorland_width
            ),
        ));
    }

    let mut seaorland = seaorland.to_vec();
    let mut patchtypes_select = vec![vec![0_i32; nlats_dm_select]; nlons_dm_select];

    for fortran_cell_id in 2..=contain.ustr_id.len() {
        let cell_idx = fortran_cell_id - 1;
        if contain.is_in_area_ustr[cell_idx] == 0 {
            continue;
        }
        let pixel_count =
            usize_from_i32_nonnegative(contain.ustr_id[cell_idx][0], "ustr_id(:,1) pixel count")?;
        if pixel_count == 0 {
            continue;
        }
        let first_pixel_id =
            usize_from_i32_positive(contain.ustr_id[cell_idx][1], "ustr_id(:,2) first pixel id")?;
        let last_pixel_id = first_pixel_id + pixel_count - 1;
        if last_pixel_id > contain.ustr_ii.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "cell {fortran_cell_id} references pixel id {last_pixel_id}, outside 1..={}",
                    contain.ustr_ii.len()
                ),
            ));
        }

        for fortran_pixel_id in first_pixel_id..=last_pixel_id {
            let pixel = &contain.ustr_ii[fortran_pixel_id - 1];
            let (lon_idx, lat_idx) = patchtype_indices(
                pixel[0],
                pixel[1],
                minlon_dm_area,
                maxlat_dm_area,
                nlons_dm_select,
                nlats_dm_select,
            )?;
            seaorland[lon_idx][lat_idx] = 0;
            patchtypes_select[lon_idx][lat_idx] = i32::try_from(fortran_cell_id).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("cell id {fortran_cell_id} does not fit i32"),
                )
            })?;
        }
    }

    let mut filled_ignored_land_pixels = 0usize;
    for lat_idx in 0..nlats_dm_select {
        for lon_idx in 0..nlons_dm_select {
            if seaorland[lon_idx][lat_idx] == 0 {
                continue;
            }
            let mut inherited_patch = if lat_idx > 0 {
                patchtypes_select[lon_idx][lat_idx - 1]
            } else {
                0
            };
            if inherited_patch == 0 {
                inherited_patch = (0..nlats_dm_select)
                    .map(|candidate_lat| patchtypes_select[lon_idx][candidate_lat])
                    .find(|patch| *patch != 0)
                    .unwrap_or(0);
            }
            if inherited_patch == 0 {
                inherited_patch = patchtypes_select
                    .iter()
                    .flat_map(|row| row.iter().copied())
                    .find(|patch| *patch != 0)
                    .unwrap_or(0);
            }
            if inherited_patch == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "ignored land pixel has no neighboring patch id",
                ));
            }
            patchtypes_select[lon_idx][lat_idx] = inherited_patch;
            seaorland[lon_idx][lat_idx] = 0;
            filled_ignored_land_pixels += 1;
        }
    }

    Ok(LandPatchtypes {
        seaorland,
        patchtypes_select,
        filled_ignored_land_pixels,
    })
}
