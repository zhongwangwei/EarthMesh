use std::io;
use std::path::Path;

use crate::{first_existing_dimension_len, netcdf_to_io_error};

/// Derive `gridnum_perdegree` from a landcover file's own longitude dimension.
///
/// The landcover carve samples the landtype grid, and the sampler requires the
/// passed `gridnum_perdegree` to equal `lon_dim / 360`. `NL%gridnum_perdegree` is
/// a separate source-grid knob that need not match the landcover file's
/// resolution (e.g. an IGBP grid at 240/° vs a namelist default of 120), so read
/// the file's resolution instead of asserting the two are equal.
pub fn landtype_gridnum_perdegree(landtype_file: &Path) -> io::Result<usize> {
    let file = crate::open_netcdf(landtype_file).map_err(netcdf_to_io_error)?;
    let lon_dim = first_existing_dimension_len(&file, &["lon", "longitude"])?;
    if lon_dim == 0 || lon_dim % 360 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "landtype_file longitude dimension {lon_dim} is not a positive multiple of 360"
            ),
        ));
    }
    Ok(lon_dim / 360)
}
