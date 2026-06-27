use std::io;
use std::path::Path;

use crate::{netcdf_to_io_error, required_values_f64, required_values_i8, ColmSurfaceClassPoint};

/// Read the surface-class points from a CoLM coupling NetCDF for GUI preview
/// coloring and lightweight inspection.
pub fn read_colm_surface_class_points_netcdf(
    input_netcdf: impl AsRef<Path>,
) -> io::Result<Vec<ColmSurfaceClassPoint>> {
    let file = netcdf::open(input_netcdf.as_ref()).map_err(netcdf_to_io_error)?;
    let lon = required_values_f64(&file, "center_lon")?;
    let lat = required_values_f64(&file, "center_lat")?;
    let code = required_values_i8(&file, "surface_class_code")?;
    if lon.len() != lat.len() || lon.len() != code.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "CoLM coupling coordinate/class lengths differ: lon={} lat={} code={}",
                lon.len(),
                lat.len(),
                code.len()
            ),
        ));
    }
    Ok(lon
        .into_iter()
        .zip(lat)
        .zip(code)
        .map(|((lon, lat), code)| ColmSurfaceClassPoint { lon, lat, code })
        .collect())
}
