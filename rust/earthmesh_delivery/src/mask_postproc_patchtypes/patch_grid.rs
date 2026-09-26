use std::io;

use crate::{
    lookup_f64, matrix_width, usize_from_i32_nonnegative, write_patchid_netcdf,
    MaskPostprocDomainIoPlan, PatchIdMesh, PatchIdWriteReport,
};

/// Build the `PatchID_Save` payload from a selected-domain patch index grid and
/// the `MOD_Area_judge` lon/lat lookup arrays.
pub fn patchid_mesh_from_selected_domain(
    patchtypes_select: Vec<Vec<i32>>,
    minlon_dm_area: i32,
    maxlat_dm_area: i32,
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    lon_i: &[f64],
    lat_i: &[f64],
) -> io::Result<PatchIdMesh> {
    let nlon = patchtypes_select.len();
    let nlat = matrix_width("patchtypes_select", &patchtypes_select)?;

    let mut lon_w = Vec::with_capacity(nlon);
    let mut lon_e = Vec::with_capacity(nlon);
    let mut longitude = Vec::with_capacity(nlon);
    for lon_offset in 0..nlon {
        let source_lon = usize_from_i32_nonnegative(
            minlon_dm_area
                + i32::try_from(lon_offset).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("longitude offset {lon_offset} does not fit i32"),
                    )
                })?,
            "Dmlons_source",
        )?;
        lon_w.push(lookup_f64(lon_vertex, source_lon, "lon_vertex")?);
        lon_e.push(lookup_f64(lon_vertex, source_lon + 1, "lon_vertex")?);
        longitude.push(lookup_f64(lon_i, source_lon, "lon_i")?);
    }

    let mut lat_n = Vec::with_capacity(nlat);
    let mut lat_s = Vec::with_capacity(nlat);
    let mut latitude = Vec::with_capacity(nlat);
    let nlat_i32 = i32::try_from(nlat).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("latitude count {nlat} does not fit i32"),
        )
    })?;
    let can_descend_from_maxlat = maxlat_dm_area - nlat_i32 + 1 >= 1;
    let can_ascend_from_maxlat = maxlat_dm_area >= 0
        && usize::try_from(maxlat_dm_area + nlat_i32)
            .map(|end| end < lat_vertex.len() && end <= lat_i.len())
            .unwrap_or(false);
    if !can_descend_from_maxlat && !can_ascend_from_maxlat {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "patchtype latitude window maxlat {maxlat_dm_area} cannot cover {nlat} rows in source order"
            ),
        ));
    }
    for lat_offset in 0..nlat {
        let lat_offset_i32 = i32::try_from(lat_offset).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("latitude offset {lat_offset} does not fit i32"),
            )
        })?;
        let source_lat_i32 = if can_descend_from_maxlat {
            maxlat_dm_area - lat_offset_i32
        } else {
            maxlat_dm_area + lat_offset_i32
        };
        let source_lat = usize_from_i32_nonnegative(source_lat_i32, "Dmlats_source")?;
        lat_n.push(lookup_f64(lat_vertex, source_lat, "lat_vertex")?);
        lat_s.push(lookup_f64(lat_vertex, source_lat + 1, "lat_vertex")?);
        latitude.push(lookup_f64(lat_i, source_lat, "lat_i")?);
    }

    Ok(PatchIdMesh {
        elmindex: patchtypes_select,
        lon_w,
        lon_e,
        lat_n,
        lat_s,
        longitude,
        latitude,
    })
}

/// Compose `PatchID_Save` coordinate construction with the compatibility patchtype
/// output path selected by `plan_mask_postproc_domain_io`.
pub fn write_mask_postproc_patchtype_netcdf(
    plan: &MaskPostprocDomainIoPlan,
    patchtypes_select: Vec<Vec<i32>>,
    minlon_dm_area: i32,
    maxlat_dm_area: i32,
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    lon_i: &[f64],
    lat_i: &[f64],
) -> io::Result<PatchIdWriteReport> {
    let output = plan.patchtype_output.as_ref().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "mask_postproc plan for {} has no patchtype_output",
                plan.mesh_type
            ),
        )
    })?;
    let patch = patchid_mesh_from_selected_domain(
        patchtypes_select,
        minlon_dm_area,
        maxlat_dm_area,
        lon_vertex,
        lat_vertex,
        lon_i,
        lat_i,
    )?;
    write_patchid_netcdf(output, &patch)
}
