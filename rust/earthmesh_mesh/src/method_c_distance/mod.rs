use earthmesh_core::deg_to_rad;

use super::{magnitude, CartesianPoint, LonLatDegrees, PlanePoint};

pub(crate) fn method_c_ec_ps_distance_meters(
    point: CartesianPoint,
    pole: LonLatDegrees,
    radius: f64,
) -> f64 {
    let projected = method_c_ec_ps_project(point, pole, radius);
    projected.x.hypot(projected.y)
}

pub(crate) fn method_c_ec_ps_project(
    point: CartesianPoint,
    pole: LonLatDegrees,
    radius: f64,
) -> PlanePoint {
    let point_radius = magnitude(point);
    if point_radius == 0.0 {
        return PlanePoint::new(f64::INFINITY, f64::INFINITY);
    }
    let scale = radius / point_radius;
    let xeq = point.x * scale;
    let yeq = point.y * scale;
    let zeq = point.z * scale;
    let pole_lat = deg_to_rad(pole.lat_degrees);
    let pole_lon = deg_to_rad(pole.lon_degrees);
    let sinplat = pole_lat.sin();
    let cosplat = pole_lat.cos();
    let sinplon = pole_lon.sin();
    let cosplon = pole_lon.cos();

    let xep = radius * cosplat * cosplon;
    let yep = radius * cosplat * sinplon;
    let zep = radius * sinplat;
    let dxe = xeq - xep;
    let dye = yeq - yep;
    let dze = zeq - zep;

    let xq = -sinplon * dxe + cosplon * dye;
    let yq = cosplat * dze - sinplat * (cosplon * dxe + sinplon * dye);
    let zq = sinplat * dze + cosplat * (cosplon * dxe + sinplon * dye);
    let earth_diameter = 2.0 * radius;
    let denominator = earth_diameter + zq;
    if !denominator.is_finite() || denominator <= 1.0 {
        // Stereographic distance diverges at the antipode. Clamping the
        // denominator to one metre turns the 0/0 limit at the exact antipode
        // into a near-zero projected distance, which can select the opposite
        // side of the globe for a local refinement region. Treat the guarded
        // near-singular cap as infinitely far instead.
        return PlanePoint::new(f64::INFINITY, f64::INFINITY);
    }
    let t = earth_diameter / denominator;

    PlanePoint::new(xq * t, yq * t)
}

pub(crate) fn method_c_ll_ps_project(
    point: LonLatDegrees,
    pole: LonLatDegrees,
    radius: f64,
) -> PlanePoint {
    let qlat = deg_to_rad(point.lat_degrees);
    let qlon = deg_to_rad(point.lon_degrees);
    let cartesian = CartesianPoint::new(
        radius * qlat.cos() * qlon.cos(),
        radius * qlat.cos() * qlon.sin(),
        radius * qlat.sin(),
    );
    method_c_ec_ps_project(cartesian, pole, radius)
}

pub(crate) fn method_c_corridor_segment_pole(
    start: LonLatDegrees,
    end: LonLatDegrees,
) -> LonLatDegrees {
    let mut segment_lon = 0.5 * (start.lon_degrees + end.lon_degrees);
    if (start.lon_degrees - end.lon_degrees).abs() > 180.0 {
        if segment_lon <= 0.0 {
            segment_lon += 180.0;
        } else {
            segment_lon -= 180.0;
        }
    }
    LonLatDegrees::new(segment_lon, 0.5 * (start.lat_degrees + end.lat_degrees))
}

pub(crate) fn method_c_corridor_segment_distance_meters(
    point: CartesianPoint,
    start: LonLatDegrees,
    end: LonLatDegrees,
    radius: f64,
) -> (f64, f64) {
    let pole = method_c_corridor_segment_pole(start, end);
    let a = method_c_ll_ps_project(start, pole, radius);
    let b = method_c_ll_ps_project(end, pole, radius);
    let p = method_c_ec_ps_project(point, pole, radius);
    plane_segment_distance(p, a, b)
}

pub(crate) fn plane_segment_distance(
    point: PlanePoint,
    start: PlanePoint,
    end: PlanePoint,
) -> (f64, f64) {
    if !point.x.is_finite()
        || !point.y.is_finite()
        || !start.x.is_finite()
        || !start.y.is_finite()
        || !end.x.is_finite()
        || !end.y.is_finite()
    {
        return (f64::INFINITY, 0.0);
    }
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let denom = dx * dx + dy * dy;
    let t = if denom == 0.0 {
        0.0
    } else {
        (((point.x - start.x) * dx + (point.y - start.y) * dy) / denom).clamp(0.0, 1.0)
    };
    let closest_x = start.x + t * dx;
    let closest_y = start.y + t * dy;
    ((point.x - closest_x).hypot(point.y - closest_y), t)
}
