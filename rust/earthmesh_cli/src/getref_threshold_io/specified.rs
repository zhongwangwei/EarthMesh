use std::fs;
use std::io;
use std::path::Path;

use crate::{netcdf_to_io_error, write_i32_1d};

use super::types::GetRefSpecifiedThresholdWriteReport;

/// Read the specified-refinement marker file produced by
/// `MOD_GetRef:GetRef(iter /= 0)` and restore the Fortran placeholder element.
pub fn read_getref_specified_ref_sjx_netcdf(input: impl AsRef<Path>) -> io::Result<Vec<i32>> {
    let file = crate::open_netcdf(input.as_ref()).map_err(netcdf_to_io_error)?;
    let variable = file.variable("IsInRfArea_sjx_specified").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "specified threshold file is missing IsInRfArea_sjx_specified",
        )
    })?;
    let mut values = Vec::with_capacity(variable.len() + 1);
    values.push(0);
    values.extend(
        variable
            .get_values::<i32, _>(..)
            .map_err(netcdf_to_io_error)?,
    );
    Ok(values)
}

/// Write the specified-refinement target file produced by
/// `MOD_GetRef:GetRef(iter /= 0)`.
pub fn write_getref_specified_threshold_netcdf(
    output: impl AsRef<Path>,
    is_in_refine_sjx: &[i32],
) -> io::Result<GetRefSpecifiedThresholdWriteReport> {
    let sjx_points = is_in_refine_sjx.len().checked_sub(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "IsInRfArea_sjx must include a Fortran placeholder element",
        )
    })?;
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = crate::create_netcdf(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("sjx_points", sjx_points)
        .map_err(netcdf_to_io_error)?;
    write_i32_1d(
        &mut file,
        "IsInRfArea_sjx_specified",
        "sjx_points",
        &is_in_refine_sjx[1..],
    )?;

    Ok(GetRefSpecifiedThresholdWriteReport {
        output: output.to_path_buf(),
        sjx_points,
    })
}
