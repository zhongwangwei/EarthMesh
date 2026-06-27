use crate::{hydro_close_geometry::ring_bbox, HydroCloseMaskSpec};

pub(crate) fn is_close_mask_ring_too_close(
    spec: &HydroCloseMaskSpec,
    emitted_specs: &[HydroCloseMaskSpec],
    min_ring_separation_deg: f64,
) -> bool {
    let spec_bbox = ring_bbox(&spec.coordinates);
    emitted_specs.iter().any(|emitted| {
        bbox_distance_deg(spec_bbox, ring_bbox(&emitted.coordinates)) < min_ring_separation_deg
    })
}

pub(crate) fn bbox_distance_deg(left: (f64, f64, f64, f64), right: (f64, f64, f64, f64)) -> f64 {
    let (left_min_lon, left_min_lat, left_max_lon, left_max_lat) = left;
    let (right_min_lon, right_min_lat, right_max_lon, right_max_lat) = right;
    let lon_gap = 0.0_f64
        .max(right_min_lon - left_max_lon)
        .max(left_min_lon - right_max_lon);
    let lat_gap = 0.0_f64
        .max(right_min_lat - left_max_lat)
        .max(left_min_lat - right_max_lat);
    (lon_gap * lon_gap + lat_gap * lat_gap).sqrt()
}
