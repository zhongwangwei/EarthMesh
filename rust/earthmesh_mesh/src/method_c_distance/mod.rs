use earthmesh_core::deg_to_rad;

use super::{CartesianPoint, LonLatDegrees, PlanePoint};

pub(crate) fn method_c_ec_ps_distance_meters(
    point: CartesianPoint,
    pole: LonLatDegrees,
    radius: f64,
) -> f64 {
    let projected = method_c_ec_ps_project_canonical_real(point, pole, radius);
    projected.x.hypot(projected.y)
}

fn method_c_ec_ps_project_canonical_real(
    point: CartesianPoint,
    pole: LonLatDegrees,
    radius: f64,
) -> PlanePoint {
    let radius = radius as f32;
    let point_radius =
        ((point.x as f32).powi(2) + (point.y as f32).powi(2) + (point.z as f32).powi(2)).sqrt();
    if point_radius == 0.0 {
        return PlanePoint::new(f64::INFINITY, f64::INFINITY);
    }
    let scale = radius / point_radius;
    let xeq = point.x as f32 * scale;
    let yeq = point.y as f32 * scale;
    let zeq = point.z as f32 * scale;
    let pole_lat = deg_to_rad(pole.lat_degrees) as f32;
    let pole_lon = deg_to_rad(pole.lon_degrees) as f32;
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
    let t = earth_diameter / (earth_diameter + zq).max(1.0);

    PlanePoint::new((xq * t) as f64, (yq * t) as f64)
}

fn method_c_ll_ps_project_canonical_real(
    point: LonLatDegrees,
    pole: LonLatDegrees,
    radius: f64,
) -> PlanePoint {
    let radius = radius as f32;
    let qlat = deg_to_rad(point.lat_degrees) as f32;
    let qlon = deg_to_rad(point.lon_degrees) as f32;
    let cartesian = CartesianPoint::new(
        (radius * qlat.cos() * qlon.cos()) as f64,
        (radius * qlat.cos() * qlon.sin()) as f64,
        (radius * qlat.sin()) as f64,
    );
    method_c_ec_ps_project_canonical_real(cartesian, pole, radius as f64)
}

fn plane_segment_distance_canonical_real(
    point: PlanePoint,
    start: PlanePoint,
    end: PlanePoint,
) -> (f64, f64) {
    let x0 = point.x as f32;
    let y0 = point.y as f32;
    let x1 = start.x as f32;
    let y1 = start.y as f32;
    let x2 = end.x as f32;
    let y2 = end.y as f32;
    let dx = x2 - x1;
    let dy = y2 - y1;
    let xp = x0 - x1;
    let yp = y0 - y1;
    let denom = dx * dx + dy * dy;
    let t = if denom <= f32::EPSILON {
        0.0
    } else {
        ((xp * dx + yp * dy) / denom).clamp(0.0, 1.0)
    };
    let dist = ((xp - t * dx).powi(2) + (yp - t * dy).powi(2)).sqrt();
    (dist as f64, t as f64)
}

pub(crate) fn method_c_corridor_segment_distance_meters(
    point: CartesianPoint,
    start: LonLatDegrees,
    end: LonLatDegrees,
    radius: f64,
) -> (f64, f64) {
    let mut segment_lon = 0.5 * (start.lon_degrees + end.lon_degrees);
    if (start.lon_degrees - end.lon_degrees).abs() > 180.0 {
        if segment_lon <= 0.0 {
            segment_lon += 180.0;
        } else {
            segment_lon -= 180.0;
        }
    }
    let pole = LonLatDegrees::new(segment_lon, 0.5 * (start.lat_degrees + end.lat_degrees));
    let a = method_c_ll_ps_project_canonical_real(start, pole, radius);
    let b = method_c_ll_ps_project_canonical_real(end, pole, radius);
    let p = method_c_ec_ps_project_canonical_real(point, pole, radius);
    plane_segment_distance_canonical_real(p, a, b)
}

pub(crate) fn plane_segment_distance(
    point: PlanePoint,
    start: PlanePoint,
    end: PlanePoint,
) -> (f64, f64) {
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
