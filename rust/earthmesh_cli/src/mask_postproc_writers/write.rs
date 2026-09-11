use std::io;
use std::path::Path;

use crate::{matrix_width, netcdf_to_io_error};

use super::types::{EarthmeshInfo, EarthmeshInfoWriteReport, PatchIdMesh, PatchIdWriteReport};
use super::validation::{validate_earthmesh_info, validate_patchid_mesh};

/// Write the `patchtype_NXP*.nc4` schema produced by
/// `MOD_mask_postproc.F90:PatchID_Save`.
pub fn write_patchid_netcdf(
    output: impl AsRef<Path>,
    patch: &PatchIdMesh,
) -> io::Result<PatchIdWriteReport> {
    validate_patchid_mesh(patch)?;
    let output = output.as_ref();
    let nlon = patch.elmindex.len();
    let nlat = matrix_width("elmindex", &patch.elmindex)?;

    let mut file = create_patchid_file(
        output,
        &patch.lon_w,
        &patch.lon_e,
        &patch.lat_s,
        &patch.lat_n,
    )?;
    // Rust's in-memory map is [lon][lat]; CoLM's Fortran reader expects
    // elmindex(nlat,nlon) on disk. Transpose a row, not a second full raster.
    let mut row = vec![0_i32; nlon];
    for j in 0..nlat {
        for (i, value) in row.iter_mut().enumerate() {
            *value = patch.elmindex[i][j];
        }
        file.variable_mut("elmindex")
            .expect("defined elmindex")
            .put_values(&row, (j, ..))
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<f64>("longitude", &["nlon"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&patch.longitude, ..)
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<f64>("latitude", &["nlat"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&patch.latitude, ..)
            .map_err(netcdf_to_io_error)?;
    }

    file.close().map_err(netcdf_to_io_error)?;
    Ok(PatchIdWriteReport {
        output: output.to_path_buf(),
        nlon,
        nlat,
    })
}

/// Shared disk schema for dense legacy maps and the streaming polygon adapter.
/// NetCDF's Fortran API reverses dimension order: val(x,y) reads (nlat,nlon).
pub(crate) fn create_patchid_file(
    output: &Path,
    lon_w: &[f64],
    lon_e: &[f64],
    lat_s: &[f64],
    lat_n: &[f64],
) -> io::Result<netcdf::FileMut> {
    if lon_w.is_empty()
        || lat_s.is_empty()
        || lon_w.len() != lon_e.len()
        || lat_s.len() != lat_n.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "patchid coordinates must have matching nonzero longitude/latitude lengths",
        ));
    }
    crate::ensure_parent_dir(output)?;
    let mut file = crate::create_netcdf(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("nlon", lon_w.len())
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("nlat", lat_s.len())
        .map_err(netcdf_to_io_error)?;
    {
        let mut variable = file
            .add_variable::<i32>("elmindex", &["nlat", "nlon"])
            .map_err(netcdf_to_io_error)?;
        variable
            .set_chunking(&[1, lon_w.len().min(16384)])
            .map_err(netcdf_to_io_error)?;
        variable
            .set_compression(1, true)
            .map_err(netcdf_to_io_error)?;
    }
    for (name, dimension, values) in [
        ("lon_w", "nlon", lon_w),
        ("lon_e", "nlon", lon_e),
        ("lat_s", "nlat", lat_s),
        ("lat_n", "nlat", lat_n),
    ] {
        file.add_variable::<f64>(name, &[dimension])
            .map_err(netcdf_to_io_error)?
            .put_values(values, ..)
            .map_err(netcdf_to_io_error)?;
    }
    Ok(file)
}

/// Write the `earthmesh_info.nc4` schema produced by
/// `MOD_file_preprocess.F90:LOCmesh_info_save` in the Earth postprocess branch.
pub fn write_earthmesh_info_netcdf(
    output: impl AsRef<Path>,
    info: &EarthmeshInfo,
) -> io::Result<EarthmeshInfoWriteReport> {
    validate_earthmesh_info(info)?;
    let output = output.as_ref();
    crate::ensure_parent_dir(output)?;

    let num_step = info.num_step_f.len();
    let num_ustr = info.refine_degree_f.len();

    let mut file = crate::create_netcdf(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("num_step", num_step)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("num_ustr", num_ustr)
        .map_err(netcdf_to_io_error)?;
    {
        let mut var = file
            .add_variable::<i32>("num_step_f", &["num_step"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&info.num_step_f, ..)
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<i32>("refine_degree_f", &["num_ustr"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&info.refine_degree_f, ..)
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<i32>("seaorland_ustr_f", &["num_ustr"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&info.seaorland_ustr_f, ..)
            .map_err(netcdf_to_io_error)?;
    }

    Ok(EarthmeshInfoWriteReport {
        output: output.to_path_buf(),
        num_step,
        num_ustr,
    })
}
