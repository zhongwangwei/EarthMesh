use super::*;

/// Port of `MOD_grid_preprocess:robust_spherical_area`.
///
/// Returns signed area on the unit sphere. The caller can multiply by radius²
/// when physical area is needed. The formula preserves Fortran's dateline-aware
/// `delta_lon` adjustment and does not take an absolute value.
pub fn robust_spherical_area_unit(points: &[LonLatDegrees]) -> Option<f64> {
    let num_inter = points.len();
    if num_inter < 3 {
        return None;
    }

    let mut area = 0.0;
    for i in 0..num_inter {
        let j = (i + 1) % num_inter;
        let lon_i = deg_to_rad(points[i].lon_degrees);
        let lon_j = deg_to_rad(points[j].lon_degrees);
        let lat_i = deg_to_rad(points[i].lat_degrees);
        let lat_j = deg_to_rad(points[j].lat_degrees);
        let mut delta_lon = lon_j - lon_i;
        if delta_lon > std::f64::consts::PI {
            delta_lon -= 2.0 * std::f64::consts::PI;
        } else if delta_lon < -std::f64::consts::PI {
            delta_lon += 2.0 * std::f64::consts::PI;
        }
        area += delta_lon * (2.0 + lat_i.sin() + lat_j.sin());
    }

    Some(area / 2.0)
}
