use std::io;
use std::path::Path;

use crate::netcdf_to_io_error;

use super::LambertVertices;

/// Read `xi_vert`/`eta_vert`, `lon_vert`, and `lat_vert` from a Lambert source.
pub fn read_lambert_vertices_netcdf(inputfile: impl AsRef<Path>) -> io::Result<LambertVertices> {
    let file = netcdf::open(inputfile.as_ref()).map_err(netcdf_to_io_error)?;
    let xi_vert = file
        .dimension("xi_vert")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing xi_vert dimension"))?
        .len();
    let eta_vert = file
        .dimension("eta_vert")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing eta_vert dimension"))?
        .len();
    let lon_vert = file
        .variable("lon_vert")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing lon_vert variable"))?
        .get_values::<f64, _>((.., ..))
        .map_err(netcdf_to_io_error)?;
    let lat_vert = file
        .variable("lat_vert")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing lat_vert variable"))?
        .get_values::<f64, _>((.., ..))
        .map_err(netcdf_to_io_error)?;
    Ok(LambertVertices {
        xi_vert,
        eta_vert,
        lon_vert,
        lat_vert,
    })
}
