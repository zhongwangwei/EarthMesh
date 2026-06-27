use std::io;
use std::path::Path;

use crate::{netcdf_to_io_error, require_len, required_dimension_len, required_values_i32};

/// Read `obc_order` from the `obc.nc4`/`obc_patch.nc4` boundary file used by
/// `MOD_file_preprocess.F90:FVCOM_Mesh_Save`.
pub fn read_obc_order_netcdf(input: impl AsRef<Path>) -> io::Result<Vec<usize>> {
    let file = netcdf::open(input.as_ref()).map_err(netcdf_to_io_error)?;
    let bdy_num = required_dimension_len(&file, "bdy_num")?;
    let values = required_values_i32(&file, "obc_order")?;
    require_len("obc_order", values.len(), bdy_num)?;
    values
        .into_iter()
        .map(|value| {
            usize::try_from(value).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("obc_order contains negative value {value}"),
                )
            })
        })
        .collect()
}
