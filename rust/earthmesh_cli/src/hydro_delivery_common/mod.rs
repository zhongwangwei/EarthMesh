use std::fs;
use std::io::{self, Read};
use std::path::Path;

use flate2::read::GzDecoder;

pub(crate) fn format_coupling_number(n: f64) -> String {
    if n.is_finite() && n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// Unit-sphere radius used by `util/hydro_mesh/earthmesh_intersection.py`.
pub(crate) const HYDRO_EARTH_RADIUS_M: f64 = earthmesh_core::EARTH_RADIUS_METERS;

pub(crate) fn read_text_maybe_gzip(path: impl AsRef<Path>) -> io::Result<String> {
    let path = path.as_ref();
    let bytes = fs::read(path)?;
    if path.extension().and_then(|s| s.to_str()) == Some("gz") {
        let mut text = String::new();
        GzDecoder::new(bytes.as_slice()).read_to_string(&mut text)?;
        return Ok(text);
    }
    String::from_utf8(bytes).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}
