use std::collections::{BTreeMap, HashSet};
use std::f64::consts::PI;

use earthmesh_geometry::{
    area_judge_first_self_intersection_one_based, polygon_area, spherical_polygon_area_km2, Point,
    EARTH_RADIUS_KM,
};
use serde::{Deserialize, Serialize};

use crate::GeometryPoint;

const DEFAULT_CHAIKIN_ITERATIONS: u8 = 2;
const DEFAULT_MAX_SEGMENT_ANGLE_DEG: f64 = 0.25;
const DEFAULT_CAP_MAX_RADIUS_DEG: f64 = 80.0;
const VECTOR_EPS: f64 = 1.0e-12;
const ANTIPODAL_EPS_RAD: f64 = 1.0e-9;
const MAX_CLOSE_BOUNDARY_POINTS: usize = 20_000;

fn default_chaikin_iterations() -> u8 {
    DEFAULT_CHAIKIN_ITERATIONS
}

fn default_max_segment_angle_deg() -> f64 {
    DEFAULT_MAX_SEGMENT_ANGLE_DEG
}

fn default_cap_max_radius_deg() -> f64 {
    DEFAULT_CAP_MAX_RADIUS_DEG
}

/// How an input close ring is represented after it is read.
///
/// `Polyline` is the compatibility default and preserves the original points.
/// The other modes validate the input as a simple local ring before applying a
/// spherical transformation.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum CloseBoundaryMode {
    #[default]
    Polyline,
    SphericalChaikin {
        #[serde(default = "default_chaikin_iterations")]
        iterations: u8,
        #[serde(default = "default_max_segment_angle_deg")]
        max_segment_angle_deg: f64,
    },
    EnclosingCap {
        #[serde(default)]
        margin_km: f64,
        #[serde(default = "default_cap_max_radius_deg")]
        max_radius_deg: f64,
        #[serde(default = "default_max_segment_angle_deg")]
        max_segment_angle_deg: f64,
    },
}

impl CloseBoundaryMode {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Polyline => Ok(()),
            Self::SphericalChaikin {
                iterations,
                max_segment_angle_deg,
            } => {
                if !(1..=6).contains(iterations) {
                    return Err(
                        "close spherical_chaikin iterations must be between 1 and 6".to_string()
                    );
                }
                validate_max_segment_angle(*max_segment_angle_deg)
            }
            Self::EnclosingCap {
                margin_km,
                max_radius_deg,
                max_segment_angle_deg,
            } => {
                if !margin_km.is_finite() || *margin_km < 0.0 {
                    return Err("close enclosing_cap margin_km must be finite and >= 0".to_string());
                }
                if !max_radius_deg.is_finite() || *max_radius_deg <= 0.0 || *max_radius_deg >= 90.0
                {
                    return Err(
                        "close enclosing_cap max_radius_deg must be finite and in (0, 90)"
                            .to_string(),
                    );
                }
                validate_max_segment_angle(*max_segment_angle_deg)
            }
        }
    }

    /// Compact adapter value carried through the flat engine namelist.
    pub fn to_engine_spec(&self) -> String {
        match self {
            Self::Polyline => "polyline".to_string(),
            Self::SphericalChaikin {
                iterations,
                max_segment_angle_deg,
            } => format!(
                "spherical_chaikin:iterations={iterations},max_segment_angle_deg={max_segment_angle_deg}"
            ),
            Self::EnclosingCap {
                margin_km,
                max_radius_deg,
                max_segment_angle_deg,
            } => format!(
                "enclosing_cap:margin_km={margin_km},max_radius_deg={max_radius_deg},max_segment_angle_deg={max_segment_angle_deg}"
            ),
        }
    }

    pub fn from_engine_spec(spec: &str) -> Result<Self, String> {
        let spec = spec.trim();
        if spec.is_empty() || spec.eq_ignore_ascii_case("polyline") {
            return Ok(Self::Polyline);
        }
        let (kind, values) = spec
            .split_once(':')
            .ok_or_else(|| format!("invalid close boundary engine spec {spec:?}"))?;
        let values = parse_values(values)?;
        let mode = match kind.trim().to_ascii_lowercase().as_str() {
            "spherical_chaikin" => Self::SphericalChaikin {
                iterations: parse_value(&values, "iterations")?,
                max_segment_angle_deg: parse_value(&values, "max_segment_angle_deg")?,
            },
            "enclosing_cap" => Self::EnclosingCap {
                margin_km: parse_value(&values, "margin_km")?,
                max_radius_deg: parse_value(&values, "max_radius_deg")?,
                max_segment_angle_deg: parse_value(&values, "max_segment_angle_deg")?,
            },
            other => return Err(format!("unsupported close boundary mode {other:?}")),
        };
        mode.validate()?;
        Ok(mode)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum CloseBoundaryGeometry {
    Polygon(Vec<GeometryPoint>),
    EnclosingCap {
        center: GeometryPoint,
        radius_km: f64,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct CloseBoundaryReport {
    pub input_points: usize,
    pub output_points: usize,
    pub input_area_km2: f64,
    pub output_area_km2: f64,
    pub area_delta_km2: f64,
    pub max_vertex_displacement_km: Option<f64>,
    pub radius_km: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CloseBoundaryTransform {
    pub geometry: CloseBoundaryGeometry,
    pub report: CloseBoundaryReport,
}

pub fn transform_close_boundary(
    points: &[GeometryPoint],
    mode: &CloseBoundaryMode,
) -> Result<CloseBoundaryTransform, String> {
    mode.validate()?;
    if matches!(mode, CloseBoundaryMode::Polyline) {
        let area = spherical_area(points);
        return Ok(CloseBoundaryTransform {
            geometry: CloseBoundaryGeometry::Polygon(points.to_vec()),
            report: CloseBoundaryReport {
                input_points: points.len(),
                output_points: points.len(),
                input_area_km2: area,
                output_area_km2: area,
                area_delta_km2: 0.0,
                max_vertex_displacement_km: Some(0.0),
                radius_km: None,
            },
        });
    }

    let source = canonical_local_ring(points)?;
    let input_area_km2 = spherical_area(&source);
    match mode {
        CloseBoundaryMode::Polyline => unreachable!(),
        CloseBoundaryMode::SphericalChaikin {
            iterations,
            max_segment_angle_deg,
        } => {
            let mut vectors = source.iter().copied().map(to_unit).collect::<Vec<_>>();
            for _ in 0..*iterations {
                vectors = chaikin_iteration(&vectors)?;
            }
            vectors = densify_geodesic_ring(&vectors, *max_segment_angle_deg)?;
            let output = vectors.into_iter().map(from_unit).collect::<Vec<_>>();
            validate_transformed_ring(&output)?;
            let output_area_km2 = spherical_area(&output);
            let max_vertex_displacement_km = source
                .iter()
                .map(|point| {
                    output
                        .iter()
                        .map(|candidate| angular_distance(to_unit(*point), to_unit(*candidate)))
                        .fold(f64::INFINITY, f64::min)
                        * EARTH_RADIUS_KM
                })
                .fold(0.0_f64, f64::max);
            Ok(CloseBoundaryTransform {
                geometry: CloseBoundaryGeometry::Polygon(output.clone()),
                report: CloseBoundaryReport {
                    input_points: points.len(),
                    output_points: output.len(),
                    input_area_km2,
                    output_area_km2,
                    area_delta_km2: output_area_km2 - input_area_km2,
                    max_vertex_displacement_km: Some(max_vertex_displacement_km),
                    radius_km: None,
                },
            })
        }
        CloseBoundaryMode::EnclosingCap {
            margin_km,
            max_radius_deg,
            max_segment_angle_deg,
        } => {
            let samples = densify_lonlat_ring(&source, *max_segment_angle_deg)?;
            let center_vector = normalized_mean(&samples)?;
            let center = from_unit(center_vector);
            let radius_rad = samples
                .iter()
                .map(|point| angular_distance(center_vector, to_unit(*point)))
                .fold(0.0_f64, f64::max)
                + lonlat_sampling_error_bound_rad(&source, *max_segment_angle_deg)
                + margin_km / EARTH_RADIUS_KM;
            let radius_deg = radius_rad.to_degrees();
            if radius_deg > *max_radius_deg {
                return Err(format!(
                    "close enclosing_cap radius {radius_deg:.6}° exceeds max_radius_deg {max_radius_deg}°"
                ));
            }
            let radius_km = radius_rad * EARTH_RADIUS_KM;
            let output_area_km2 =
                2.0 * PI * EARTH_RADIUS_KM * EARTH_RADIUS_KM * (1.0 - radius_rad.cos());
            Ok(CloseBoundaryTransform {
                geometry: CloseBoundaryGeometry::EnclosingCap { center, radius_km },
                report: CloseBoundaryReport {
                    input_points: points.len(),
                    output_points: 1,
                    input_area_km2,
                    output_area_km2,
                    area_delta_km2: output_area_km2 - input_area_km2,
                    max_vertex_displacement_km: None,
                    radius_km: Some(radius_km),
                },
            })
        }
    }
}

fn validate_max_segment_angle(value: f64) -> Result<(), String> {
    if !value.is_finite() || value <= 0.0 || value > 30.0 {
        return Err("close max_segment_angle_deg must be finite and in (0, 30]".to_string());
    }
    Ok(())
}

fn parse_values(values: &str) -> Result<BTreeMap<&str, &str>, String> {
    let mut parsed = BTreeMap::new();
    for item in values.split(',') {
        let (key, value) = item
            .split_once('=')
            .ok_or_else(|| format!("invalid close boundary option {item:?}"))?;
        parsed.insert(key.trim(), value.trim());
    }
    Ok(parsed)
}

fn parse_value<T>(values: &BTreeMap<&str, &str>, key: &str) -> Result<T, String>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    let value = values
        .get(key)
        .ok_or_else(|| format!("close boundary engine spec is missing {key}"))?;
    value
        .parse::<T>()
        .map_err(|err| format!("invalid close boundary {key}={value}: {err}"))
}

fn canonical_local_ring(points: &[GeometryPoint]) -> Result<Vec<GeometryPoint>, String> {
    if points.len() < 3 {
        return Err("close boundary requires at least three points".to_string());
    }
    if points.len() > MAX_CLOSE_BOUNDARY_POINTS {
        return Err(format!(
            "close transformed boundary supports at most {MAX_CLOSE_BOUNDARY_POINTS} input points"
        ));
    }
    let mut ring = Vec::with_capacity(points.len());
    for (index, point) in points.iter().copied().enumerate() {
        if !point.lon.is_finite() || !point.lat.is_finite() {
            return Err(format!("close boundary point {} is not finite", index + 1));
        }
        if !(-90.0..=90.0).contains(&point.lat) {
            return Err(format!(
                "close boundary point {} latitude must be in [-90, 90]",
                index + 1
            ));
        }
        let point = GeometryPoint::new(wrap_lon(point.lon), point.lat);
        if ring.last().is_some_and(|previous| {
            angular_distance(to_unit(*previous), to_unit(point)) < VECTOR_EPS
        }) {
            return Err(format!(
                "close boundary has duplicate consecutive point {}",
                index + 1
            ));
        }
        ring.push(point);
    }
    if ring.len() > 3
        && angular_distance(to_unit(ring[0]), to_unit(*ring.last().expect("non-empty")))
            < VECTOR_EPS
    {
        ring.pop();
    }
    if ring.len() < 3 {
        return Err("close boundary requires at least three unique points".to_string());
    }
    let mut seen = HashSet::with_capacity(ring.len());
    for (index, point) in ring.iter().enumerate() {
        let key = (point.lon.to_bits(), point.lat.to_bits());
        if !seen.insert(key) {
            return Err(format!("close boundary has duplicate vertex {}", index + 1));
        }
    }
    for index in 0..ring.len() {
        let angle = angular_distance(
            to_unit(ring[index]),
            to_unit(ring[(index + 1) % ring.len()]),
        );
        if PI - angle <= ANTIPODAL_EPS_RAD {
            return Err(format!(
                "close boundary edge {} has antipodal or near-antipodal endpoints",
                index + 1
            ));
        }
    }
    validate_transformed_ring(&ring)?;
    let center = normalized_mean(&ring)?;
    let max_radius = ring
        .iter()
        .map(|point| angular_distance(center, to_unit(*point)))
        .fold(0.0_f64, f64::max);
    if max_radius >= PI / 2.0 {
        return Err("close transformed boundary must fit inside one open hemisphere".to_string());
    }
    Ok(ring)
}

fn validate_transformed_ring(points: &[GeometryPoint]) -> Result<(), String> {
    let planar = unwrap_ring(points);
    if let Some(intersection) = area_judge_first_self_intersection_one_based(&planar) {
        return Err(format!(
            "close boundary is self-intersecting at edges {} and {}",
            intersection.first_segment_id, intersection.second_segment_id
        ));
    }
    if polygon_area(&planar) <= 1.0e-12 {
        return Err("close boundary has zero planar area".to_string());
    }
    Ok(())
}

fn unwrap_ring(points: &[GeometryPoint]) -> Vec<Point> {
    let mut out = Vec::with_capacity(points.len());
    let mut previous = wrap_lon(points[0].lon);
    out.push(Point::new(previous, points[0].lat));
    for point in &points[1..] {
        let lon = unwrap_near(point.lon, previous);
        out.push(Point::new(lon, point.lat));
        previous = lon;
    }
    out
}

fn chaikin_iteration(points: &[[f64; 3]]) -> Result<Vec<[f64; 3]>, String> {
    if points.len().saturating_mul(2) > MAX_CLOSE_BOUNDARY_POINTS {
        return Err(format!(
            "close boundary smoothing would exceed {MAX_CLOSE_BOUNDARY_POINTS} points"
        ));
    }
    let mut output = Vec::with_capacity(points.len() * 2);
    for index in 0..points.len() {
        let a = points[index];
        let b = points[(index + 1) % points.len()];
        output.push(slerp(a, b, 0.25)?);
        output.push(slerp(a, b, 0.75)?);
    }
    Ok(output)
}

fn densify_geodesic_ring(
    points: &[[f64; 3]],
    max_segment_angle_deg: f64,
) -> Result<Vec<[f64; 3]>, String> {
    let max_angle = max_segment_angle_deg.to_radians();
    let mut output = Vec::new();
    for index in 0..points.len() {
        let a = points[index];
        let b = points[(index + 1) % points.len()];
        let angle = angular_distance(a, b);
        let segments = (angle / max_angle).ceil().max(1.0) as usize;
        if output.len().saturating_add(segments) > MAX_CLOSE_BOUNDARY_POINTS {
            return Err(format!(
                "close boundary densification would exceed {MAX_CLOSE_BOUNDARY_POINTS} points"
            ));
        }
        for step in 0..segments {
            output.push(slerp(a, b, step as f64 / segments as f64)?);
        }
    }
    Ok(output)
}

fn densify_lonlat_ring(
    points: &[GeometryPoint],
    max_segment_angle_deg: f64,
) -> Result<Vec<GeometryPoint>, String> {
    let mut output = Vec::new();
    for index in 0..points.len() {
        let a = points[index];
        let b = points[(index + 1) % points.len()];
        let bx = unwrap_near(b.lon, a.lon);
        let span = (bx - a.lon).abs().max((b.lat - a.lat).abs());
        let segments = (span / max_segment_angle_deg).ceil().max(1.0) as usize;
        if output.len().saturating_add(segments) > MAX_CLOSE_BOUNDARY_POINTS {
            return Err(format!(
                "close boundary densification would exceed {MAX_CLOSE_BOUNDARY_POINTS} points"
            ));
        }
        for step in 0..segments {
            let t = step as f64 / segments as f64;
            output.push(GeometryPoint::new(
                wrap_lon(a.lon + (bx - a.lon) * t),
                a.lat + (b.lat - a.lat) * t,
            ));
        }
    }
    if output.is_empty() {
        return Err("close boundary densification produced no points".to_string());
    }
    Ok(output)
}

/// Conservative distance from any unsampled point on the current lon/lat
/// straight-segment boundary to its nearest sample. The spherical line element
/// is bounded by `|dlat| + |dlon|`, and samples include both endpoints through
/// the adjacent segment, so half of each subsegment path length is sufficient.
fn lonlat_sampling_error_bound_rad(points: &[GeometryPoint], max_segment_angle_deg: f64) -> f64 {
    let mut bound = 0.0_f64;
    for index in 0..points.len() {
        let a = points[index];
        let b = points[(index + 1) % points.len()];
        let bx = unwrap_near(b.lon, a.lon);
        let delta_lon = (bx - a.lon).abs();
        let delta_lat = (b.lat - a.lat).abs();
        let segments = (delta_lon.max(delta_lat) / max_segment_angle_deg)
            .ceil()
            .max(1.0);
        bound = bound.max(0.5 * (delta_lon + delta_lat).to_radians() / segments);
    }
    bound
}

fn normalized_mean(points: &[GeometryPoint]) -> Result<[f64; 3], String> {
    let sum = points.iter().fold([0.0; 3], |mut sum, point| {
        let vector = to_unit(*point);
        sum[0] += vector[0];
        sum[1] += vector[1];
        sum[2] += vector[2];
        sum
    });
    let norm = norm(sum);
    if norm <= VECTOR_EPS * points.len() as f64 {
        return Err(
            "close enclosing_cap center is degenerate because boundary directions cancel"
                .to_string(),
        );
    }
    Ok([sum[0] / norm, sum[1] / norm, sum[2] / norm])
}

fn slerp(a: [f64; 3], b: [f64; 3], t: f64) -> Result<[f64; 3], String> {
    let angle = angular_distance(a, b);
    if angle <= VECTOR_EPS {
        return Ok(a);
    }
    if PI - angle <= ANTIPODAL_EPS_RAD {
        return Err("close boundary contains an antipodal edge".to_string());
    }
    let sin_angle = angle.sin();
    let wa = ((1.0 - t) * angle).sin() / sin_angle;
    let wb = (t * angle).sin() / sin_angle;
    normalize([
        wa * a[0] + wb * b[0],
        wa * a[1] + wb * b[1],
        wa * a[2] + wb * b[2],
    ])
}

fn spherical_area(points: &[GeometryPoint]) -> f64 {
    spherical_polygon_area_km2(
        &points
            .iter()
            .map(|point| Point::new(point.lon, point.lat))
            .collect::<Vec<_>>(),
    )
}

fn to_unit(point: GeometryPoint) -> [f64; 3] {
    let lon = point.lon.to_radians();
    let lat = point.lat.to_radians();
    [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
}

fn from_unit(vector: [f64; 3]) -> GeometryPoint {
    GeometryPoint::new(
        wrap_lon(vector[1].atan2(vector[0]).to_degrees()),
        vector[2].clamp(-1.0, 1.0).asin().to_degrees(),
    )
}

fn angular_distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    norm(cross(a, b)).atan2(dot(a, b).clamp(-1.0, 1.0))
}

fn normalize(vector: [f64; 3]) -> Result<[f64; 3], String> {
    let length = norm(vector);
    if length <= VECTOR_EPS {
        return Err("close boundary spherical interpolation is degenerate".to_string());
    }
    Ok([vector[0] / length, vector[1] / length, vector[2] / length])
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn norm(vector: [f64; 3]) -> f64 {
    dot(vector, vector).sqrt()
}

fn wrap_lon(lon: f64) -> f64 {
    ((lon + 180.0).rem_euclid(360.0)) - 180.0
}

fn unwrap_near(lon: f64, anchor: f64) -> f64 {
    let mut lon = wrap_lon(lon);
    while lon - anchor > 180.0 {
        lon -= 360.0;
    }
    while lon - anchor < -180.0 {
        lon += 360.0;
    }
    lon
}
