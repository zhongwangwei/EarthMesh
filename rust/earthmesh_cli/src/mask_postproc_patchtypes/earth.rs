use std::io;

use crate::{
    matrix_width, patchtype_indices, usize_from_i32_nonnegative, usize_from_i32_positive,
    usize_to_i32, validate_contain_mesh, validate_mask_postproc_layout,
    write_earthmesh_info_netcdf, ContainMesh, EarthPatchtypes, EarthmeshInfo,
    EarthmeshInfoWriteReport, MaskPostprocDomainIoPlan, MaskPostprocLayout,
};

/// Pure-data port of the `MOD_mask_postproc.F90:mask_postproc_Earth`
/// `patchtypes_make` loop.
///
/// Rust row `0` corresponds to Canonical row `1`; output `patchtypes_select`
/// is row-major by selected longitude index, then selected latitude index.
pub fn build_earth_patchtypes_one_based(
    contain: &ContainMesh,
    mask_sea_ratio: f64,
    minlon_dm_area: i32,
    maxlat_dm_area: i32,
    nlons_dm_select: usize,
    nlats_dm_select: usize,
) -> io::Result<EarthPatchtypes> {
    validate_contain_mesh(contain)?;
    let dim_a = matrix_width("ustr_id", &contain.ustr_id)?;
    let dim_b = matrix_width("ustr_ii", &contain.ustr_ii)?;
    if dim_a < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "earth patchtype construction requires ustr_id rows with at least two columns",
        ));
    }
    if dim_b < 3 {
        let active_cell_canonicals_pixels = contain
            .ustr_id
            .iter()
            .zip(contain.is_in_area_ustr.iter())
            .any(|(row, &active)| active == 1 && row.first().copied().unwrap_or(0) != 0);
        if contain.ustr_ii.is_empty() && !active_cell_canonicals_pixels {
            let seaorland_ustr = vec![0_i32; contain.ustr_id.len()];
            let patchtypes_select = vec![vec![0_i32; nlats_dm_select]; nlons_dm_select];
            return Ok(EarthPatchtypes {
                seaorland_ustr,
                patchtypes_select,
                sum_land_ustr: 0,
                sum_sea_ustr: 0,
            });
        }
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "earth patchtype construction requires ustr_ii rows with at least three columns",
        ));
    }

    let mut seaorland_ustr = vec![0_i32; contain.ustr_id.len()];
    let mut patchtypes_select = vec![vec![0_i32; nlats_dm_select]; nlons_dm_select];
    let mut sum_land_ustr = 0usize;
    let mut sum_sea_ustr = 0usize;

    for canonical_cell_id in 2..=contain.ustr_id.len() {
        let cell_idx = canonical_cell_id - 1;
        if contain.is_in_area_ustr[cell_idx] != 1 {
            continue;
        }
        let pixel_count =
            usize_from_i32_nonnegative(contain.ustr_id[cell_idx][0], "ustr_id(:,1) pixel count")?;
        let first_pixel_id =
            usize_from_i32_positive(contain.ustr_id[cell_idx][1], "ustr_id(:,2) first pixel id")?;
        if pixel_count == 0 {
            continue;
        }
        let last_pixel_id = first_pixel_id + pixel_count - 1;
        if last_pixel_id > contain.ustr_ii.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "cell {canonical_cell_id} canonicals pixel id {last_pixel_id}, outside 1..={}",
                    contain.ustr_ii.len()
                ),
            ));
        }

        let mut land_pixels = 0_i32;
        for canonical_pixel_id in first_pixel_id..=last_pixel_id {
            land_pixels += contain.ustr_ii[canonical_pixel_id - 1][2];
        }

        if f64::from(land_pixels) / pixel_count as f64 > mask_sea_ratio {
            seaorland_ustr[cell_idx] = 1;
            sum_land_ustr += 1;
            for canonical_pixel_id in first_pixel_id..=last_pixel_id {
                let pixel = &contain.ustr_ii[canonical_pixel_id - 1];
                if pixel[2] == 0 {
                    continue;
                }
                let (lon_idx, lat_idx) = patchtype_indices(
                    pixel[0],
                    pixel[1],
                    minlon_dm_area,
                    maxlat_dm_area,
                    nlons_dm_select,
                    nlats_dm_select,
                )?;
                patchtypes_select[lon_idx][lat_idx] =
                    i32::try_from(canonical_cell_id).map_err(|_| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("cell id {canonical_cell_id} does not fit i32"),
                        )
                    })?;
            }
        } else {
            seaorland_ustr[cell_idx] = -1;
            sum_sea_ustr += 1;
        }
    }

    Ok(EarthPatchtypes {
        seaorland_ustr,
        patchtypes_select,
        sum_land_ustr,
        sum_sea_ustr,
    })
}

/// Build the `earthmesh_info.nc4` payload from the final
/// `MOD_mask_postproc.F90:mask_postproc_Earth` role/refinement loop.
pub fn build_earthmesh_info_one_based(
    mode_grid: &str,
    num_mp_step: &[usize],
    sjx_points: usize,
    layout: &MaskPostprocLayout,
    is_in_domain_ustr: &[i32],
    seaorland_ustr: &[i32],
) -> io::Result<EarthmeshInfo> {
    validate_mask_postproc_layout(layout)?;
    let mode_grid = mode_grid.trim();
    let role_points = match mode_grid {
        "tri" => {
            if is_in_domain_ustr.len() < layout.ustr_points {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "IsInDmArea_ustr length {} must cover ustr_points {}",
                        is_in_domain_ustr.len(),
                        layout.ustr_points
                    ),
                ));
            }
            layout.ustr_points
        }
        "hex" => is_in_domain_ustr.len().min(layout.ustr_points),
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("earthmesh_info supports tri or hex mode_grid only, got {other}"),
            ));
        }
    };
    if seaorland_ustr.len() < role_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "seaorland_ustr length {} must cover role points {}",
                seaorland_ustr.len(),
                role_points
            ),
        ));
    }

    let mut num_step_f = num_mp_step
        .iter()
        .map(|&value| usize_to_i32("num_mp_step", value))
        .collect::<io::Result<Vec<_>>>()?;
    num_step_f.push(usize_to_i32("sjx_points", sjx_points)?);

    let active_count = is_in_domain_ustr
        .iter()
        .take(role_points)
        .skip(2)
        .filter(|&&value| value == 1)
        .count();
    let mut seaorland_ustr_f = vec![0_i32; active_count + 2];
    let mut refine_degree_f = vec![0_i32; active_count + 2];

    let mut compact_id = 1_usize;
    match mode_grid {
        "tri" => {
            let mut step_idx = 1_usize;
            for source_id in 2..role_points {
                if step_idx < num_step_f.len()
                    && usize::try_from(num_step_f[step_idx]).unwrap_or(usize::MAX) <= source_id
                {
                    num_step_f[step_idx] = usize_to_i32("num_step_f compact id", compact_id)?;
                    step_idx += 1;
                }
                if is_in_domain_ustr[source_id] != 1 {
                    continue;
                }
                compact_id += 1;
                seaorland_ustr_f[compact_id] = seaorland_ustr[source_id];
                refine_degree_f[compact_id] =
                    usize_to_i32("refine_degree_f", step_idx.saturating_sub(1))?;
            }
        }
        "hex" => {
            for source_id in 2..role_points {
                let max_center_vertex = layout.center_neighbors[source_id]
                    .iter()
                    .copied()
                    .max()
                    .unwrap_or(0);
                let mut step_idx = 1_usize;
                while step_idx < num_step_f.len()
                    && usize::try_from(num_step_f[step_idx]).unwrap_or(usize::MAX)
                        < max_center_vertex
                {
                    step_idx += 1;
                }
                if is_in_domain_ustr[source_id] != 1 {
                    continue;
                }
                compact_id += 1;
                seaorland_ustr_f[compact_id] = seaorland_ustr[source_id];
                refine_degree_f[compact_id] =
                    usize_to_i32("refine_degree_f", step_idx.saturating_sub(1))?;
            }
        }
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("earthmesh_info supports tri or hex mode_grid only, got {other}"),
            ));
        }
    }

    Ok(EarthmeshInfo {
        num_step_f,
        refine_degree_f,
        seaorland_ustr_f,
    })
}

/// Compose the Earth branch role/refinement payload with the compatibility
/// `result/earthmesh_info.nc4` output path.
pub fn write_mask_postproc_earth_info_netcdf(
    plan: &MaskPostprocDomainIoPlan,
    num_mp_step: &[usize],
    sjx_points: usize,
    layout: &MaskPostprocLayout,
    is_in_domain_ustr: &[i32],
    seaorland_ustr: &[i32],
) -> io::Result<EarthmeshInfoWriteReport> {
    if plan.mesh_type != "earthmesh" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "earthmesh_info output is only produced for earthmesh plans, got {}",
                plan.mesh_type
            ),
        ));
    }
    let info = build_earthmesh_info_one_based(
        &plan.mode_grid,
        num_mp_step,
        sjx_points,
        layout,
        is_in_domain_ustr,
        seaorland_ustr,
    )?;
    write_earthmesh_info_netcdf(plan.file_dir.join("result/earthmesh_info.nc4"), &info)
}
