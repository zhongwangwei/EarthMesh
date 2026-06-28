use std::{fs, io, path::Path};

use earthmesh_mesh::AreaJudgeSourceBounds;

use crate::{
    i32_matrix_from_flat, netcdf_to_io_error, optional_values_i32_2d, required_dimension_len,
    required_scalar_usize_i32, required_values_f64, usize_to_i32, write_f64_1d,
    write_i32_matrix_rows, write_i32_scalar,
};

use super::types::AreaJudgeGridPayload;
use super::validate::validate_area_judge_grid_payload;

pub fn write_area_judge_grid_netcdf(
    output: impl AsRef<Path>,
    payload: &AreaJudgeGridPayload,
) -> io::Result<()> {
    validate_area_judge_grid_payload(payload)?;
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = crate::create_netcdf(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("nlons_select", payload.longitude.len())
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("nlats_select", payload.latitude.len())
        .map_err(netcdf_to_io_error)?;
    write_i32_scalar(
        &mut file,
        "minlon_DmArea",
        usize_to_i32("minlon_DmArea", payload.bounds.minlon_source)?,
    )?;
    write_i32_scalar(
        &mut file,
        "maxlon_DmArea",
        usize_to_i32("maxlon_DmArea", payload.bounds.maxlon_source)?,
    )?;
    write_i32_scalar(
        &mut file,
        "maxlat_DmArea",
        usize_to_i32("maxlat_DmArea", payload.bounds.maxlat_source)?,
    )?;
    write_i32_scalar(
        &mut file,
        "minlat_DmArea",
        usize_to_i32("minlat_DmArea", payload.bounds.minlat_source)?,
    )?;
    write_f64_1d(&mut file, "longitude", "nlons_select", &payload.longitude)?;
    write_f64_1d(&mut file, "latitude", "nlats_select", &payload.latitude)?;
    write_i32_matrix_rows(
        &mut file,
        "IsInArea_select",
        &["nlons_select", "nlats_select"],
        &payload.is_in_area_select,
    )?;
    write_i32_matrix_rows(
        &mut file,
        "IsInDmArea_select",
        &["nlons_select", "nlats_select"],
        &payload.is_in_area_select,
    )?;
    if let Some(seaorland) = payload.seaorland_select.as_ref() {
        write_i32_matrix_rows(
            &mut file,
            "seaorland_select",
            &["nlons_select", "nlats_select"],
            seaorland,
        )?;
    }
    Ok(())
}

pub fn read_area_judge_grid_netcdf(input: impl AsRef<Path>) -> io::Result<AreaJudgeGridPayload> {
    let file = crate::open_netcdf(input.as_ref()).map_err(netcdf_to_io_error)?;
    let nlons = required_dimension_len(&file, "nlons_select")?;
    let nlats = required_dimension_len(&file, "nlats_select")?;
    let bounds = AreaJudgeSourceBounds {
        minlon_source: required_scalar_usize_i32(&file, "minlon_DmArea")?,
        maxlon_source: required_scalar_usize_i32(&file, "maxlon_DmArea")?,
        maxlat_source: required_scalar_usize_i32(&file, "maxlat_DmArea")?,
        minlat_source: required_scalar_usize_i32(&file, "minlat_DmArea")?,
    };
    let longitude = required_values_f64(&file, "longitude")?;
    let latitude = required_values_f64(&file, "latitude")?;
    if longitude.len() != nlons {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "longitude length {} must match nlons_select {nlons}",
                longitude.len()
            ),
        ));
    }
    if latitude.len() != nlats {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "latitude length {} must match nlats_select {nlats}",
                latitude.len()
            ),
        ));
    }
    let area_values = if let Some(values) = optional_values_i32_2d(&file, "IsInDmArea_select")? {
        values
    } else if let Some(values) = optional_values_i32_2d(&file, "IsInArea_select")? {
        values
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing IsInDmArea_select or IsInArea_select variable",
        ));
    };
    let is_in_area_select = i32_matrix_from_flat("IsInArea_select", area_values, nlons, nlats)?;
    let seaorland_select = optional_values_i32_2d(&file, "seaorland_select")?
        .map(|values| i32_matrix_from_flat("seaorland_select", values, nlons, nlats))
        .transpose()?;

    let payload = AreaJudgeGridPayload {
        bounds,
        longitude,
        latitude,
        is_in_area_select,
        seaorland_select,
    };
    validate_area_judge_grid_payload(&payload).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid Area_judge grid payload: {err}"),
        )
    })?;
    Ok(payload)
}
