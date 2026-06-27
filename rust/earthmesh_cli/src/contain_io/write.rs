use std::fs;
use std::io;
use std::path::Path;

use crate::{flatten_i32_rows, matrix_width, netcdf_to_io_error};

use super::types::{ContainMesh, ContainWriteReport, FlatContainMesh};
use super::validation::{validate_contain_mesh, validate_flat_contain_mesh};

/// Write the `contain_*.nc4` schema consumed by
/// `MOD_file_preprocess.F90:Contain_Read`.
pub fn write_contain_netcdf(
    output: impl AsRef<Path>,
    contain: &ContainMesh,
) -> io::Result<ContainWriteReport> {
    validate_contain_mesh(contain)?;
    write_flat_contain_netcdf(
        output,
        &FlatContainMesh {
            ustr_id_values: flatten_i32_rows(&contain.ustr_id),
            ustr_id_width: matrix_width("ustr_id", &contain.ustr_id)?,
            ustr_ii_values: flatten_i32_rows(&contain.ustr_ii),
            ustr_ii_width: matrix_width("ustr_ii", &contain.ustr_ii)?,
            is_in_area_ustr: contain.is_in_area_ustr.clone(),
        },
    )
}

pub fn write_flat_contain_netcdf(
    output: impl AsRef<Path>,
    contain: &FlatContainMesh,
) -> io::Result<ContainWriteReport> {
    validate_flat_contain_mesh(contain)?;
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let num_ustr = contain.num_ustr();
    let num_ii = contain.num_ii();
    let dim_a = contain.ustr_id_width;
    let dim_b = contain.ustr_ii_width;

    let mut file = netcdf::create(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("num_ustr", num_ustr)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("num_ii", num_ii)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("dim_a", dim_a)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("dim_b", dim_b)
        .map_err(netcdf_to_io_error)?;
    {
        let mut var = file
            .add_variable::<i32>("ustr_id", &["num_ustr", "dim_a"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&contain.ustr_id_values, (.., ..))
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<i32>("ustr_ii", &["num_ii", "dim_b"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&contain.ustr_ii_values, (.., ..))
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<i32>("IsInArea_ustr", &["num_ustr"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&contain.is_in_area_ustr, ..)
            .map_err(netcdf_to_io_error)?;
    }

    Ok(ContainWriteReport {
        output: output.to_path_buf(),
        num_ustr,
        num_ii,
        dim_a,
        dim_b,
    })
}
