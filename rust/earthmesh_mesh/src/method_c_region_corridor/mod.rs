use std::io;

use crate::method_c_region_validation::validate_method_c_radius;
use crate::spherical_projection::unproject_from_polar_stereographic_with_radius;
use crate::{
    lonlat_degrees_to_unit_xyz, method_c_corridor_segment_distance_meters,
    method_c_corridor_segment_pole, method_c_ll_ps_project, plane_segment_distance,
    validate_lonlat, xyz_to_lonlat_degrees, CartesianPoint, LonLatDegrees, PlanePoint, PoleBasis,
};

const MIN_FOOTPRINT_ERROR_METERS: f64 = 1.0e-7;
const MAX_FOOTPRINT_RECURSION_DEPTH: usize = 52;
const CAP_BASE_SUBDIVISIONS: usize = 8;

pub(crate) fn method_c_closed_corridor_contains_cartesian(
    point: CartesianPoint,
    points: &[LonLatDegrees],
    radius: f64,
    corridor_radius_meters: f64,
) -> bool {
    if points.len() < 2 {
        return false;
    }
    points.windows(2).any(|segment| {
        method_c_corridor_segment_distance_meters(point, segment[0], segment[1], radius).0
            < corridor_radius_meters
    }) || points
        .last()
        .zip(points.first())
        .is_some_and(|(&last, &first)| {
            method_c_corridor_segment_distance_meters(point, last, first, radius).0
                < corridor_radius_meters
        })
}

pub(crate) fn method_c_open_corridor_contains_cartesian(
    point: CartesianPoint,
    points: &[LonLatDegrees],
    radius: f64,
    corridor_radius_meters: f64,
) -> bool {
    if points.len() < 2 {
        return false;
    }
    points.windows(2).any(|segment| {
        method_c_corridor_segment_distance_meters(point, segment[0], segment[1], radius).0
            < corridor_radius_meters
    })
}

pub(crate) fn method_c_corridor_radius_at_segment(
    radius_meters: &[f64],
    idx: usize,
    t: f64,
) -> f64 {
    let start = radius_meters
        .get(idx)
        .copied()
        .or_else(|| radius_meters.last().copied())
        .unwrap_or(0.0);
    let end = radius_meters
        .get(idx + 1)
        .copied()
        .or_else(|| radius_meters.last().copied())
        .unwrap_or(start);
    (1.0 - t) * start + t * end
}

pub(crate) fn method_c_cartesian_xy_segment_distance(
    point: CartesianPoint,
    start: LonLatDegrees,
    end: LonLatDegrees,
) -> (f64, f64) {
    plane_segment_distance(
        PlanePoint::new(point.x, point.y),
        PlanePoint::new(start.lon_degrees, start.lat_degrees),
        PlanePoint::new(end.lon_degrees, end.lat_degrees),
    )
}

/// Build the spherical swept footprint used by Canonical Method-C corridor
/// selection.
///
/// Every consecutive point pair is projected with the same per-segment
/// stereographic pole as `ngr_area`. In that plane the two straight side
/// boundaries use the endpoint radii and the end caps are exact semicircles.
/// The boundary is then mapped back to the sphere. Returned rings are separate
/// union components, one per segment.
///
/// Curved spherical boundaries are split until their deviation from each
/// emitted minor-geodesic edge is at most `max_boundary_error_meters`; callers
/// therefore get an explicit geometric error bound rather than a fixed-angle,
/// resolution-dependent sampling.
pub fn method_c_corridor_swept_footprint(
    points: &[LonLatDegrees],
    radius_meters: &[f64],
    max_boundary_error_meters: f64,
) -> io::Result<Vec<Vec<LonLatDegrees>>> {
    if points.len() < 2 {
        return Err(invalid_footprint(
            "corridor footprint requires at least two points",
        ));
    }
    if radius_meters.len() != points.len() {
        return Err(invalid_footprint(
            "corridor footprint requires one radius per point",
        ));
    }
    if !max_boundary_error_meters.is_finite()
        || max_boundary_error_meters < MIN_FOOTPRINT_ERROR_METERS
    {
        return Err(invalid_footprint(format!(
            "corridor footprint boundary error must be finite and at least {MIN_FOOTPRINT_ERROR_METERS} meters"
        )));
    }
    for &point in points {
        validate_lonlat(point)?;
    }
    for &radius in radius_meters {
        validate_method_c_radius("corridor radius", radius)?;
    }

    points
        .windows(2)
        .enumerate()
        .map(|(index, segment)| {
            method_c_corridor_segment_footprint(
                segment[0],
                segment[1],
                radius_meters[index],
                radius_meters[index + 1],
                max_boundary_error_meters,
            )
        })
        .collect()
}

fn method_c_corridor_segment_footprint(
    start: LonLatDegrees,
    end: LonLatDegrees,
    start_radius: f64,
    end_radius: f64,
    max_boundary_error_meters: f64,
) -> io::Result<Vec<LonLatDegrees>> {
    let earth_radius = earthmesh_core::EARTH_RADIUS_METERS;
    let pole = method_c_corridor_segment_pole(start, end);
    let a = method_c_ll_ps_project(start, pole, earth_radius);
    let b = method_c_ll_ps_project(end, pole, earth_radius);
    if !a.x.is_finite() || !a.y.is_finite() || !b.x.is_finite() || !b.y.is_finite() {
        return Err(invalid_footprint(
            "corridor segment cannot form its Canonical stereographic projection",
        ));
    }
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let length = dx.hypot(dy);
    if length == 0.0 {
        return method_c_corridor_disk_footprint(a, pole, start_radius, max_boundary_error_meters);
    }
    let tangent = PlanePoint::new(dx / length, dy / length);
    let normal = PlanePoint::new(-tangent.y, tangent.x);
    let radius_at = |t: f64| (1.0 - t) * start_radius + t * end_radius;
    let side = |t: f64, sign: f64| {
        let radius = radius_at(t);
        PlanePoint::new(
            a.x + t * dx + sign * radius * normal.x,
            a.y + t * dy + sign * radius * normal.y,
        )
    };
    let end_cap = |t: f64| {
        let angle = std::f64::consts::FRAC_PI_2 - std::f64::consts::PI * t;
        PlanePoint::new(
            b.x + end_radius * (angle.cos() * tangent.x + angle.sin() * normal.x),
            b.y + end_radius * (angle.cos() * tangent.y + angle.sin() * normal.y),
        )
    };
    let start_cap = |t: f64| {
        let angle = -std::f64::consts::FRAC_PI_2 - std::f64::consts::PI * t;
        PlanePoint::new(
            a.x + start_radius * (angle.cos() * tangent.x + angle.sin() * normal.x),
            a.y + start_radius * (angle.cos() * tangent.y + angle.sin() * normal.y),
        )
    };
    let lower_side = |t: f64| side(1.0 - t, -1.0);

    let mut ring = vec![unproject_method_c_plane(
        side(0.0, 1.0),
        pole,
        earth_radius,
    )?];
    append_bounded_curve(
        &mut ring,
        &|t| unproject_method_c_plane(side(t, 1.0), pole, earth_radius),
        1,
        max_boundary_error_meters,
    )?;
    append_bounded_curve(
        &mut ring,
        &|t| unproject_method_c_plane(end_cap(t), pole, earth_radius),
        CAP_BASE_SUBDIVISIONS,
        max_boundary_error_meters,
    )?;
    append_bounded_curve(
        &mut ring,
        &|t| unproject_method_c_plane(lower_side(t), pole, earth_radius),
        1,
        max_boundary_error_meters,
    )?;
    append_bounded_curve(
        &mut ring,
        &|t| unproject_method_c_plane(start_cap(t), pole, earth_radius),
        CAP_BASE_SUBDIVISIONS,
        max_boundary_error_meters,
    )?;
    remove_duplicate_closure(&mut ring);
    if ring.len() < 3 {
        return Err(invalid_footprint(
            "corridor segment footprint collapsed below three vertices",
        ));
    }
    Ok(ring)
}

fn method_c_corridor_disk_footprint(
    center: PlanePoint,
    pole: LonLatDegrees,
    radius: f64,
    max_boundary_error_meters: f64,
) -> io::Result<Vec<LonLatDegrees>> {
    let earth_radius = earthmesh_core::EARTH_RADIUS_METERS;
    let circle = |t: f64| {
        let angle = 2.0 * std::f64::consts::PI * t;
        unproject_method_c_plane(
            PlanePoint::new(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
            ),
            pole,
            earth_radius,
        )
    };
    let mut ring = vec![circle(0.0)?];
    append_bounded_curve(
        &mut ring,
        &circle,
        2 * CAP_BASE_SUBDIVISIONS,
        max_boundary_error_meters,
    )?;
    remove_duplicate_closure(&mut ring);
    Ok(ring)
}

fn unproject_method_c_plane(
    point: PlanePoint,
    pole: LonLatDegrees,
    earth_radius: f64,
) -> io::Result<LonLatDegrees> {
    let pole_basis = PoleBasis::from_lonlat_radians(
        pole.lon_degrees.to_radians(),
        pole.lat_degrees.to_radians(),
    );
    let displacement =
        unproject_from_polar_stereographic_with_radius(point, pole_basis, earth_radius);
    let pole_unit = lonlat_degrees_to_unit_xyz(pole);
    let absolute = CartesianPoint::new(
        displacement.x + earth_radius * pole_unit.x,
        displacement.y + earth_radius * pole_unit.y,
        displacement.z + earth_radius * pole_unit.z,
    );
    if !absolute.x.is_finite() || !absolute.y.is_finite() || !absolute.z.is_finite() {
        return Err(invalid_footprint(
            "corridor footprint cannot be unprojected from Canonical stereographic space",
        ));
    }
    Ok(xyz_to_lonlat_degrees(absolute))
}

fn append_bounded_curve<F>(
    ring: &mut Vec<LonLatDegrees>,
    curve: &F,
    base_subdivisions: usize,
    max_boundary_error_meters: f64,
) -> io::Result<()>
where
    F: Fn(f64) -> io::Result<LonLatDegrees>,
{
    for part in 0..base_subdivisions {
        let t0 = part as f64 / base_subdivisions as f64;
        let t1 = (part + 1) as f64 / base_subdivisions as f64;
        let start = if part == 0 {
            *ring
                .last()
                .ok_or_else(|| invalid_footprint("corridor footprint ring is empty"))?
        } else {
            curve(t0)?
        };
        let end = curve(t1)?;
        append_bounded_curve_interval(
            ring,
            curve,
            t0,
            t1,
            start,
            end,
            max_boundary_error_meters / earthmesh_core::EARTH_RADIUS_METERS,
            0,
        )?;
    }
    Ok(())
}

fn append_bounded_curve_interval<F>(
    ring: &mut Vec<LonLatDegrees>,
    curve: &F,
    t0: f64,
    t1: f64,
    start: LonLatDegrees,
    end: LonLatDegrees,
    max_error_radians: f64,
    depth: usize,
) -> io::Result<()>
where
    F: Fn(f64) -> io::Result<LonLatDegrees>,
{
    let midpoint_t = 0.5 * (t0 + t1);
    let midpoint = curve(midpoint_t)?;
    let deviation = distance_to_minor_arc_radians(midpoint, start, end);
    if deviation <= max_error_radians {
        push_distinct(ring, end);
        return Ok(());
    }
    if depth >= MAX_FOOTPRINT_RECURSION_DEPTH || midpoint_t == t0 || midpoint_t == t1 {
        return Err(invalid_footprint(format!(
            "corridor footprint could not meet the requested {max_error_radians:.3e}-radian boundary error"
        )));
    }
    append_bounded_curve_interval(
        ring,
        curve,
        t0,
        midpoint_t,
        start,
        midpoint,
        max_error_radians,
        depth + 1,
    )?;
    append_bounded_curve_interval(
        ring,
        curve,
        midpoint_t,
        t1,
        midpoint,
        end,
        max_error_radians,
        depth + 1,
    )
}

fn distance_to_minor_arc_radians(
    point: LonLatDegrees,
    start: LonLatDegrees,
    end: LonLatDegrees,
) -> f64 {
    let point = unit(point);
    let start = unit(start);
    let end = unit(end);
    let arc = angle(start, end);
    if arc <= 64.0 * f64::EPSILON {
        return angle(point, start);
    }
    let cross = cross(start, end);
    let cross_norm = dot(cross, cross).sqrt();
    if cross_norm <= 64.0 * f64::EPSILON {
        return angle(point, start).min(angle(point, end));
    }
    let normal = [
        cross[0] / cross_norm,
        cross[1] / cross_norm,
        cross[2] / cross_norm,
    ];
    let signed = dot(point, normal);
    let projected = [
        point[0] - signed * normal[0],
        point[1] - signed * normal[1],
        point[2] - signed * normal[2],
    ];
    let projected_norm = dot(projected, projected).sqrt();
    if projected_norm <= 64.0 * f64::EPSILON {
        return std::f64::consts::FRAC_PI_2;
    }
    let projected = [
        projected[0] / projected_norm,
        projected[1] / projected_norm,
        projected[2] / projected_norm,
    ];
    let along = angle(start, projected) + angle(projected, end);
    if along <= arc + 1.0e-10 {
        signed.abs().clamp(0.0, 1.0).asin()
    } else {
        angle(point, start).min(angle(point, end))
    }
}

fn unit(point: LonLatDegrees) -> [f64; 3] {
    let point = lonlat_degrees_to_unit_xyz(point);
    [point.x, point.y, point.z]
}

fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn cross(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn angle(left: [f64; 3], right: [f64; 3]) -> f64 {
    dot(left, right).clamp(-1.0, 1.0).acos()
}

fn push_distinct(ring: &mut Vec<LonLatDegrees>, point: LonLatDegrees) {
    let duplicate = ring
        .last()
        .is_some_and(|last| angle(unit(*last), unit(point)) <= 64.0 * f64::EPSILON);
    if !duplicate {
        ring.push(point);
    }
}

fn remove_duplicate_closure(ring: &mut Vec<LonLatDegrees>) {
    if ring.len() > 1
        && angle(unit(ring[0]), unit(*ring.last().expect("non-empty ring"))) <= 64.0 * f64::EPSILON
    {
        ring.pop();
    }
}

fn invalid_footprint(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}
