use std::io;
use std::path::Path;

use crate::{flatten_i32_rows, matrix_width, netcdf_to_io_error};

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
    crate::ensure_parent_dir(output)?;
    let nlon = patch.elmindex.len();
    let nlat = matrix_width("elmindex", &patch.elmindex)?;

    let mut file = crate::create_netcdf(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("nlon", nlon)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("nlat", nlat)
        .map_err(netcdf_to_io_error)?;
    {
        let mut var = file
            .add_variable::<i32>("elmindex", &["nlon", "nlat"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&flatten_i32_rows(&patch.elmindex), (.., ..))
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<f64>("lon_w", &["nlon"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&patch.lon_w, ..)
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<f64>("lon_e", &["nlon"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&patch.lon_e, ..)
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<f64>("lat_n", &["nlat"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&patch.lat_n, ..)
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<f64>("lat_s", &["nlat"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&patch.lat_s, ..)
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

    Ok(PatchIdWriteReport {
        output: output.to_path_buf(),
        nlon,
        nlat,
    })
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
