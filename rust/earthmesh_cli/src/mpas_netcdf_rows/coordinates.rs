use crate::LonLatDegrees;

pub(crate) fn mpas_lat_lon_radians(points: &[LonLatDegrees]) -> (Vec<f64>, Vec<f64>) {
    let mut lat = Vec::with_capacity(points.len());
    let mut lon = Vec::with_capacity(points.len());
    for point in points {
        lat.push(earthmesh_core::deg_to_rad(point.lat_degrees));
        let mut lon_degrees = point.lon_degrees;
        if lon_degrees < 0.0 {
            lon_degrees += 360.0;
        }
        lon.push(earthmesh_core::deg_to_rad(lon_degrees));
    }
    (lat, lon)
}
