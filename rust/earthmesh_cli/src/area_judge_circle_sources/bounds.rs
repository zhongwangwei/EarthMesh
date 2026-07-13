use std::io;

use crate::LonLatPoint;

pub(super) fn area_judge_circle_scan_bounds_canonical(
    center: LonLatPoint,
    radius_km: f64,
) -> io::Result<(f64, f64, f64, f64)> {
    if !center.lon.is_finite()
        || !center.lat.is_finite()
        || !radius_km.is_finite()
        || radius_km < 0.0
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "circle center coordinates and radius must be finite, with non-negative radius",
        ));
    }
    let temp = std::f64::consts::PI / 180.0 * earthmesh_core::EARTH_RADIUS_METERS / 1000.0;
    let cos_lat = center.lat.to_radians().cos();
    let mut edgew_temp = center.lon - (radius_km / (temp * cos_lat)) * 1.2;
    let mut edgee_temp = center.lon + (radius_km / (temp * cos_lat)) * 1.2;
    let mut edgen_temp = center.lat + (radius_km / temp) * 1.2;
    let mut edges_temp = center.lat - (radius_km / temp) * 1.2;

    if edgee_temp > 180.0 || edgew_temp < -180.0 || !(-90.0..=90.0).contains(&edgen_temp) {
        edgew_temp = -180.0;
        edgee_temp = 180.0;
    }
    if edgen_temp > 90.0 {
        edges_temp = edges_temp.min(edgen_temp);
        edgen_temp = 90.0;
    } else if edges_temp < -90.0 {
        edgen_temp = edges_temp.max(edgen_temp);
        edges_temp = -90.0;
    }
    Ok((edgew_temp, edgee_temp, edgen_temp, edges_temp))
}
