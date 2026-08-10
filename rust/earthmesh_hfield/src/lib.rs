//! Unified mesh cell-width field `h(x)` for EarthMesh refinement planning.
//!
//! This crate implements the "continuous mesh-density field" layer recommended
//! by `docs/mesh_refinement_method_research_2026-07-02.md`:
//!
//! 1. Every refinement input — threshold criteria (LAI/slope/SST/EKE rasters)
//!    and user-specified regions (bbox/circle/polygon/corridor) — contributes
//!    its own target-cell-size field `h_i(lon, lat)` in meters.
//! 2. Fields compose by pointwise minimum (`min_with_*`).
//! 3. A spherical lat-lon fast-sweeping limiter reduces the discrete field
//!    toward `|∇h| <= g` (Persson 2004/2006). It constrains the raster stencil
//!    and closes the outer rows in the exact spherical metric; it is not an exact
//!    global Lipschitz extension between arbitrary samples, so realized mesh
//!    gradation and Method-C nesting still require downstream quality checks.
//! 4. `level_map` quantizes `h` back to discrete power-of-two levels for the
//!    existing subdivision engines; `sample` feeds continuous targets to the
//!    spring relaxers ("split between levels, stretch within levels").
//!
//! Design constraints: deterministic (fixed-order fast sweeping, no threads),
//! pure f64. All longitudes are degrees in [-180, 180) (inputs are wrapped),
//! latitudes are degrees in [-90, 90].

use std::f64::consts::{PI, SQRT_2};
use std::io;

/// Canonical EarthMesh radius (`erad = 6_371_229 m`). Re-exporting the core
/// constant keeps every spherical calculation on one source of truth.
pub use earthmesh_core::EARTH_RADIUS_METERS;

fn invalid(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg)
}

/// Wrap a longitude in degrees into [-180, 180).
pub fn wrap_lon_degrees(lon: f64) -> f64 {
    let mut x = (lon + 180.0) % 360.0;
    if x < 0.0 {
        x += 360.0;
    }
    x - 180.0
}

fn lonlat_to_unit(lon_deg: f64, lat_deg: f64) -> [f64; 3] {
    let lon = lon_deg.to_radians();
    let lat = lat_deg.to_radians();
    [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
}

fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn norm3(a: [f64; 3]) -> f64 {
    dot3(a, a).sqrt()
}

/// Robust great-circle angle between two unit vectors (radians).
fn gc_angle(a: [f64; 3], b: [f64; 3]) -> f64 {
    norm3(cross3(a, b)).atan2(dot3(a, b))
}

/// Great-circle distance in meters between two (lon, lat) degree points.
pub fn great_circle_distance_m(lon1: f64, lat1: f64, lon2: f64, lat2: f64) -> f64 {
    gc_angle(lonlat_to_unit(lon1, lat1), lonlat_to_unit(lon2, lat2)) * EARTH_RADIUS_METERS
}

/// Great-circle distance in meters from point `p` to the segment `a..b`, plus
/// the clamped along-segment fraction used to interpolate endpoint metadata.
fn point_segment_distance_and_fraction_m(p: [f64; 3], a: [f64; 3], b: [f64; 3]) -> (f64, f64) {
    let seg_angle = gc_angle(a, b);
    let start_dist = gc_angle(p, a);
    let end_dist = gc_angle(p, b);
    let closest_endpoint = || {
        if start_dist <= end_dist {
            (start_dist * EARTH_RADIUS_METERS, 0.0)
        } else {
            (end_dist * EARTH_RADIUS_METERS, 1.0)
        }
    };
    if seg_angle < 1e-12 {
        return closest_endpoint();
    }
    let n = cross3(a, b);
    let nn = norm3(n);
    if nn < 1e-15 {
        return closest_endpoint();
    }
    let n_hat = [n[0] / nn, n[1] / nn, n[2] / nn];
    let sin_xt = dot3(p, n_hat).clamp(-1.0, 1.0);
    // Foot of `p` on the great circle through a, b.
    let f = [
        p[0] - sin_xt * n_hat[0],
        p[1] - sin_xt * n_hat[1],
        p[2] - sin_xt * n_hat[2],
    ];
    let fn3 = norm3(f);
    if fn3 < 1e-12 {
        // p is (anti)parallel to the circle normal: quarter turn to the circle.
        return closest_endpoint();
    }
    let f_hat = [f[0] / fn3, f[1] / fn3, f[2] / fn3];
    let within = gc_angle(a, f_hat) + gc_angle(f_hat, b) <= seg_angle + 1e-9;
    if within {
        (
            sin_xt.abs().asin() * EARTH_RADIUS_METERS,
            (gc_angle(a, f_hat) / seg_angle).clamp(0.0, 1.0),
        )
    } else {
        closest_endpoint()
    }
}

fn quantize_h_level(h: f64, h_base_m: f64, max_level: u8) -> io::Result<u8> {
    if !h_base_m.is_finite() || h_base_m <= 0.0 {
        return Err(invalid(format!(
            "base cell size {h_base_m} must be positive and finite"
        )));
    }
    if !h.is_finite() || h <= 0.0 {
        return Err(invalid(format!(
            "HField sample {h} must be positive and finite before quantization"
        )));
    }
    if h >= h_base_m {
        return Ok(0);
    }
    let raw = ((h_base_m / h).log2() - 1e-9).ceil();
    if raw <= 0.0 {
        Ok(0)
    } else if raw >= max_level as f64 {
        Ok(max_level)
    } else {
        Ok(raw as u8)
    }
}

fn point_on_minor_arc_unit(a: [f64; 3], b: [f64; 3], p: [f64; 3]) -> bool {
    let ab = gc_angle(a, b);
    if ab <= 1.0e-12 || (std::f64::consts::PI - ab).abs() <= 1.0e-10 {
        return false;
    }
    let normal = cross3(a, b);
    dot3(normal, p).abs() <= 1.0e-10 * norm3(normal).max(1.0)
        && gc_angle(a, p) + gc_angle(p, b) <= ab + 1.0e-10
}

fn spherical_polygon_contains(points: &[(f64, f64)], lon_deg: f64, lat_deg: f64) -> bool {
    if points.len() < 3
        || !lon_deg.is_finite()
        || !lat_deg.is_finite()
        || !(-90.0..=90.0).contains(&lat_deg)
    {
        return false;
    }
    let Some(area) = spherical_polygon_signed_area(points) else {
        return false;
    };
    if area.abs() <= 1.0e-14 {
        return false;
    }
    let desired_sign = if area.abs() <= std::f64::consts::TAU {
        area.signum()
    } else {
        -area.signum()
    };

    let here = lonlat_to_unit(lon_deg, lat_deg);
    let tangent = |point: [f64; 3]| -> Option<(f64, f64)> {
        let dot = dot3(here, point);
        let flat = [
            point[0] - here[0] * dot,
            point[1] - here[1] * dot,
            point[2] - here[2] * dot,
        ];
        let length = norm3(flat);
        if length <= 1.0e-12 {
            return None;
        }
        let east0 = [-here[1], here[0], 0.0];
        let east_len = (east0[0] * east0[0] + east0[1] * east0[1]).sqrt();
        let east = if east_len > 1.0e-12 {
            [east0[0] / east_len, east0[1] / east_len, 0.0]
        } else {
            [1.0, 0.0, 0.0]
        };
        let north = cross3(here, east);
        Some((dot3(flat, east) / length, dot3(flat, north) / length))
    };

    let mut turned = 0.0_f64;
    for i in 0..points.len() {
        let a_unit = lonlat_to_unit(points[i].0, points[i].1);
        let b_unit = lonlat_to_unit(
            points[(i + 1) % points.len()].0,
            points[(i + 1) % points.len()].1,
        );
        if dot3(here, a_unit) > 1.0 - 1.0e-12 || dot3(here, b_unit) > 1.0 - 1.0e-12 {
            return true;
        }
        if dot3(here, a_unit) < -1.0 + 1.0e-12 || dot3(here, b_unit) < -1.0 + 1.0e-12 {
            return false;
        }
        if point_on_minor_arc_unit(a_unit, b_unit, here) {
            return true;
        }
        let (Some(a), Some(b)) = (tangent(a_unit), tangent(b_unit)) else {
            return false;
        };
        turned += (a.0 * b.1 - a.1 * b.0).atan2(a.0 * b.0 + a.1 * b.1);
    }
    if desired_sign > 0.0 {
        turned > std::f64::consts::PI
    } else {
        turned < -std::f64::consts::PI
    }
}

fn spherical_polygon_signed_area(points: &[(f64, f64)]) -> Option<f64> {
    if points.len() < 3 {
        return None;
    }
    for &(lon, lat) in points {
        if !(lon.is_finite() && lat.is_finite()) || !(-90.0..=90.0).contains(&lat) {
            return None;
        }
    }
    let apex = lonlat_to_unit(points[0].0, points[0].1);
    let mut total = 0.0;
    for step in 1..points.len() - 1 {
        let b = lonlat_to_unit(points[step].0, points[step].1);
        let c = lonlat_to_unit(points[step + 1].0, points[step + 1].1);
        let triple = dot3(apex, cross3(b, c));
        let denominator = 1.0 + dot3(apex, b) + dot3(b, c) + dot3(c, apex);
        total += 2.0 * triple.atan2(denominator);
    }
    total.is_finite().then_some(total)
}

fn validate_lonlat(lon: f64, lat: f64, label: &str) -> io::Result<()> {
    if !lon.is_finite() || !lat.is_finite() || !(-90.0..=90.0).contains(&lat) {
        return Err(invalid(format!(
            "{label} ({lon}, {lat}) must be finite with latitude in [-90, 90]"
        )));
    }
    Ok(())
}

fn polygon_points_without_closure(points: &[(f64, f64)]) -> &[(f64, f64)] {
    if points.len() > 1 {
        let first = lonlat_to_unit(points[0].0, points[0].1);
        let last = lonlat_to_unit(points[points.len() - 1].0, points[points.len() - 1].1);
        if gc_angle(first, last) <= 1.0e-12 {
            return &points[..points.len() - 1];
        }
    }
    points
}

fn validate_minor_arc(a: (f64, f64), b: (f64, f64), label: &str) -> io::Result<()> {
    let angle = gc_angle(lonlat_to_unit(a.0, a.1), lonlat_to_unit(b.0, b.1));
    if angle <= 1.0e-12 {
        return Err(invalid(format!("{label} has coincident endpoints")));
    }
    if (PI - angle).abs() <= 1.0e-10 {
        return Err(invalid(format!("{label} has antipodal endpoints")));
    }
    Ok(())
}

fn arcs_intersect(a: [f64; 3], b: [f64; 3], c: [f64; 3], d: [f64; 3]) -> bool {
    let n1 = cross3(a, b);
    let n2 = cross3(c, d);
    let x = cross3(n1, n2);
    let xn = norm3(x);
    if xn <= 1.0e-14 {
        return point_on_minor_arc_unit(a, b, c)
            || point_on_minor_arc_unit(a, b, d)
            || point_on_minor_arc_unit(c, d, a)
            || point_on_minor_arc_unit(c, d, b);
    }
    let p = [x[0] / xn, x[1] / xn, x[2] / xn];
    let q = [-p[0], -p[1], -p[2]];
    (point_on_minor_arc_unit(a, b, p) && point_on_minor_arc_unit(c, d, p))
        || (point_on_minor_arc_unit(a, b, q) && point_on_minor_arc_unit(c, d, q))
}

fn corridor_radius_at_segment(radius_meters: &[f64], index: usize, t: f64) -> Option<f64> {
    let start = *radius_meters.get(index)?;
    let end = *radius_meters.get(index + 1)?;
    Some(start + t.clamp(0.0, 1.0) * (end - start))
}

/// A user-specified refinement region on the sphere (degrees).
#[derive(Clone, Debug)]
pub enum HRegion {
    /// Dateline-aware box: `west > east` means the box crosses the antimeridian.
    Bbox {
        west: f64,
        east: f64,
        south: f64,
        north: f64,
    },
    /// Great-circle disk. This metric is intentionally explicit and differs
    /// from the legacy Method-C polar-stereographic compatibility metric.
    Circle { lon: f64, lat: f64, radius_m: f64 },
    /// Simple spherical polygon (no self-intersection), vertices (lon, lat) in
    /// degrees and edges following great circles. The contained side is the
    /// smaller side of the ring, independent of vertex order.
    Polygon { points: Vec<(f64, f64)> },
    /// Polyline buffered by great-circle radii linearly interpolated between
    /// endpoints. `radius_meters` must contain one radius per point.
    Corridor {
        points: Vec<(f64, f64)>,
        radius_meters: Vec<f64>,
    },
}

impl HRegion {
    pub fn validate(&self) -> io::Result<()> {
        match self {
            HRegion::Bbox {
                west,
                east,
                south,
                north,
            } => {
                if !west.is_finite() || !east.is_finite() || west == east {
                    return Err(invalid(format!(
                        "bbox longitudes ({west}, {east}) must be finite and enclose a nonzero span"
                    )));
                }
                if !south.is_finite()
                    || !north.is_finite()
                    || !(-90.0..=90.0).contains(south)
                    || !(-90.0..=90.0).contains(north)
                    || south >= north
                {
                    return Err(invalid(format!(
                        "bbox latitudes south={south} north={north} must be finite, strictly ordered, and in [-90, 90]"
                    )));
                }
            }
            HRegion::Circle { lon, lat, radius_m } => {
                validate_lonlat(*lon, *lat, "circle center")?;
                if !radius_m.is_finite() || *radius_m <= 0.0 {
                    return Err(invalid(format!(
                        "circle radius {radius_m} must be positive and finite"
                    )));
                }
            }
            HRegion::Polygon { points } => {
                let points = polygon_points_without_closure(points);
                if points.len() < 3 {
                    return Err(invalid(format!(
                        "polygon must have at least 3 distinct points, got {}",
                        points.len()
                    )));
                }
                for (index, &(lon, lat)) in points.iter().enumerate() {
                    validate_lonlat(lon, lat, &format!("polygon point {index}"))?;
                }
                for i in 0..points.len() {
                    validate_minor_arc(
                        points[i],
                        points[(i + 1) % points.len()],
                        &format!("polygon edge {i}"),
                    )?;
                }
                let area = spherical_polygon_signed_area(points).ok_or_else(|| {
                    invalid("polygon spherical area could not be computed".into())
                })?;
                if area.abs() <= 1.0e-14 {
                    return Err(invalid("polygon spherical area must be nonzero".into()));
                }
                let units = points
                    .iter()
                    .map(|&(lon, lat)| lonlat_to_unit(lon, lat))
                    .collect::<Vec<_>>();
                let n = units.len();
                for i in 0..n {
                    for j in i + 1..n {
                        if i == j || (i + 1) % n == j || (j + 1) % n == i {
                            continue;
                        }
                        if arcs_intersect(
                            units[i],
                            units[(i + 1) % n],
                            units[j],
                            units[(j + 1) % n],
                        ) {
                            return Err(invalid(format!(
                                "polygon edges {i} and {j} self-intersect"
                            )));
                        }
                    }
                }
            }
            HRegion::Corridor {
                points,
                radius_meters,
            } => {
                if points.is_empty() || points.len() != radius_meters.len() {
                    return Err(invalid(format!(
                        "corridor must have one positive radius per point ({} points, {} radii)",
                        points.len(),
                        radius_meters.len()
                    )));
                }
                for (index, &(lon, lat)) in points.iter().enumerate() {
                    validate_lonlat(lon, lat, &format!("corridor point {index}"))?;
                }
                for (index, radius) in radius_meters.iter().enumerate() {
                    if !radius.is_finite() || *radius <= 0.0 {
                        return Err(invalid(format!(
                            "corridor radius {index} value {radius} must be positive and finite"
                        )));
                    }
                }
                for (index, segment) in points.windows(2).enumerate() {
                    validate_minor_arc(
                        segment[0],
                        segment[1],
                        &format!("corridor segment {index}"),
                    )?;
                }
            }
        }
        Ok(())
    }

    pub fn contains(&self, lon_deg: f64, lat_deg: f64) -> bool {
        if !lon_deg.is_finite() || !lat_deg.is_finite() || !(-90.0..=90.0).contains(&lat_deg) {
            return false;
        }
        match self {
            HRegion::Bbox {
                west,
                east,
                south,
                north,
            } => {
                if lat_deg < *south || lat_deg > *north {
                    return false;
                }
                if west.is_finite() && east.is_finite() && (*east - *west).abs() >= 360.0 - 1.0e-12
                {
                    return true;
                }
                let w = wrap_lon_degrees(*west);
                let e = wrap_lon_degrees(*east);
                let x = wrap_lon_degrees(lon_deg);
                if w <= e {
                    x >= w && x <= e
                } else {
                    x >= w || x <= e
                }
            }
            HRegion::Circle { lon, lat, radius_m } => {
                great_circle_distance_m(lon_deg, lat_deg, *lon, *lat) <= *radius_m
            }
            HRegion::Polygon { points } => spherical_polygon_contains(points, lon_deg, lat_deg),
            HRegion::Corridor {
                points,
                radius_meters,
            } => {
                if points.len() != radius_meters.len() || points.is_empty() {
                    return false;
                }
                let p = lonlat_to_unit(lon_deg, lat_deg);
                if points.len() == 1 {
                    return gc_angle(p, lonlat_to_unit(points[0].0, points[0].1))
                        * EARTH_RADIUS_METERS
                        <= radius_meters[0];
                }
                points.windows(2).enumerate().any(|(index, segment)| {
                    let (distance, t) = point_segment_distance_and_fraction_m(
                        p,
                        lonlat_to_unit(segment[0].0, segment[0].1),
                        lonlat_to_unit(segment[1].0, segment[1].1),
                    );
                    corridor_radius_at_segment(radius_meters, index, t)
                        .is_some_and(|radius| distance <= radius)
                })
            }
        }
    }

    /// Great-circle distance in meters from a point to the region's spine
    /// (corridor polyline) or center (circle). For bbox/polygon this returns
    /// 0.0 inside and `f64::INFINITY` outside (distance-to-boundary can be
    /// added later; the gradient limiter makes it unnecessary for sizing).
    pub fn distance_m(&self, lon_deg: f64, lat_deg: f64) -> f64 {
        match self {
            HRegion::Circle { lon, lat, .. } => {
                great_circle_distance_m(lon_deg, lat_deg, *lon, *lat)
            }
            HRegion::Corridor { points, .. } => {
                if points.is_empty() {
                    return f64::INFINITY;
                }
                let p = lonlat_to_unit(lon_deg, lat_deg);
                if points.len() == 1 {
                    return gc_angle(p, lonlat_to_unit(points[0].0, points[0].1))
                        * EARTH_RADIUS_METERS;
                }
                let mut best = f64::INFINITY;
                for w in points.windows(2) {
                    let a = lonlat_to_unit(w[0].0, w[0].1);
                    let b = lonlat_to_unit(w[1].0, w[1].1);
                    best = best.min(point_segment_distance_and_fraction_m(p, a, b).0);
                }
                best
            }
            _ => {
                if self.contains(lon_deg, lat_deg) {
                    0.0
                } else {
                    f64::INFINITY
                }
            }
        }
    }
}

/// A global lat-lon raster of target cell sizes in meters (cell-center
/// registered: centers at `-180 + (i + 0.5) * dlon`, `-90 + (j + 0.5) * dlat`).
#[derive(Clone, Debug)]
pub struct HField {
    nlon: usize,
    nlat: usize,
    values: Vec<f64>,
}

fn hfield_len(nlon: usize, nlat: usize) -> io::Result<usize> {
    if nlon < 4 || nlat < 2 {
        return Err(invalid(format!(
            "HField grid {nlon}x{nlat} too small (need >= 4x2)"
        )));
    }
    let len = nlon
        .checked_mul(nlat)
        .ok_or_else(|| invalid("HField dimensions overflow usize".into()))?;
    if len > isize::MAX as usize / std::mem::size_of::<f64>() {
        return Err(invalid(format!(
            "HField grid {nlon}x{nlat} exceeds the platform allocation limit"
        )));
    }
    Ok(len)
}

impl HField {
    pub fn uniform(nlon: usize, nlat: usize, h_meters: f64) -> io::Result<Self> {
        let len = hfield_len(nlon, nlat)?;
        if !h_meters.is_finite() || h_meters <= 0.0 {
            return Err(invalid(format!(
                "HField uniform value {h_meters} must be positive and finite"
            )));
        }
        let mut values = Vec::new();
        values
            .try_reserve_exact(len)
            .map_err(|error| invalid(format!("cannot allocate HField {nlon}x{nlat}: {error}")))?;
        values.resize(len, h_meters);
        Ok(Self { nlon, nlat, values })
    }

    /// Restore a persisted field after validating its exact shape and values.
    pub fn from_values(nlon: usize, nlat: usize, values: Vec<f64>) -> io::Result<Self> {
        let expected = hfield_len(nlon, nlat)?;
        if values.len() != expected {
            return Err(invalid(format!(
                "HField values length {} must equal {nlon}x{nlat}={expected}",
                values.len()
            )));
        }
        if values
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
        {
            return Err(invalid(
                "HField restored values must be positive and finite".into(),
            ));
        }
        Ok(Self { nlon, nlat, values })
    }

    pub fn nlon(&self) -> usize {
        self.nlon
    }

    pub fn nlat(&self) -> usize {
        self.nlat
    }

    pub fn dlon_degrees(&self) -> f64 {
        360.0 / self.nlon as f64
    }

    pub fn dlat_degrees(&self) -> f64 {
        180.0 / self.nlat as f64
    }

    pub fn lon_center(&self, ilon: usize) -> f64 {
        -180.0 + (ilon as f64 + 0.5) * self.dlon_degrees()
    }

    pub fn lat_center(&self, jlat: usize) -> f64 {
        -90.0 + (jlat as f64 + 0.5) * self.dlat_degrees()
    }

    fn idx(&self, ilon: usize, jlat: usize) -> usize {
        jlat * self.nlon + ilon
    }

    pub fn get(&self, ilon: usize, jlat: usize) -> f64 {
        self.values[self.idx(ilon, jlat)]
    }

    pub fn set(&mut self, ilon: usize, jlat: usize, h_meters: f64) -> io::Result<()> {
        if !h_meters.is_finite() || h_meters <= 0.0 {
            return Err(invalid(format!(
                "HField value {h_meters} must be positive and finite"
            )));
        }
        let k = self.idx(ilon, jlat);
        self.values[k] = h_meters;
        Ok(())
    }

    pub fn values(&self) -> &[f64] {
        &self.values
    }

    /// Bilinear sample at (lon, lat) degrees; longitude wraps, latitude clamps.
    /// Between either outer row and its geographic pole, values converge to
    /// the row midrange so the pole is unique and sampling remains continuous.
    pub fn sample(&self, lon_deg: f64, lat_deg: f64) -> f64 {
        if lat_deg >= 90.0 {
            return self.polar_row_midrange(self.nlat - 1);
        }
        if lat_deg <= -90.0 {
            return self.polar_row_midrange(0);
        }
        let dlon = self.dlon_degrees();
        let dlat = self.dlat_degrees();
        let x = (wrap_lon_degrees(lon_deg) + 180.0) / dlon - 0.5;
        let i0f = x.floor();
        let fx = x - i0f;
        let i0 = (i0f as i64).rem_euclid(self.nlon as i64) as usize;
        let i1 = (i0 + 1) % self.nlon;
        let row_sample = |j| {
            let v0 = self.get(i0, j);
            v0 + (self.get(i1, j) - v0) * fx
        };
        let south_center = self.lat_center(0);
        if lat_deg < south_center {
            let pole = self.polar_row_midrange(0);
            let fraction = ((lat_deg + 90.0) / (south_center + 90.0)).clamp(0.0, 1.0);
            return pole + (row_sample(0) - pole) * fraction;
        }
        let north_row = self.nlat - 1;
        let north_center = self.lat_center(north_row);
        if lat_deg > north_center {
            let pole = self.polar_row_midrange(north_row);
            let fraction = ((90.0 - lat_deg) / (90.0 - north_center)).clamp(0.0, 1.0);
            return pole + (row_sample(north_row) - pole) * fraction;
        }
        let y = ((lat_deg + 90.0) / dlat - 0.5).clamp(0.0, (self.nlat - 1) as f64);
        let j0 = y.floor() as usize;
        let j0 = j0.min(self.nlat - 1);
        let j1 = (j0 + 1).min(self.nlat - 1);
        let fy = y - j0 as f64;
        let v0 = row_sample(j0);
        let v1 = row_sample(j1);
        v0 + (v1 - v0) * fy
    }

    fn polar_row_midrange(&self, jlat: usize) -> f64 {
        let start = jlat * self.nlon;
        let (minimum, maximum) = self.values[start..start + self.nlon].iter().fold(
            (f64::INFINITY, f64::NEG_INFINITY),
            |(minimum, maximum), value| (minimum.min(*value), maximum.max(*value)),
        );
        minimum + 0.5 * (maximum - minimum)
    }

    /// Pointwise `h = min(h, f(lon, lat))` over cell centers. Non-finite or
    /// non-positive candidates are ignored so criterion closures can return
    /// e.g. `f64::INFINITY` for "no opinion here".
    pub fn min_with_fn<F: Fn(f64, f64) -> f64>(&mut self, f: F) {
        for j in 0..self.nlat {
            let lat = self.lat_center(j);
            for i in 0..self.nlon {
                let lon = self.lon_center(i);
                let candidate = f(lon, lat);
                if candidate.is_finite() && candidate > 0.0 {
                    let k = self.idx(i, j);
                    if candidate < self.values[k] {
                        self.values[k] = candidate;
                    }
                }
            }
        }
    }

    /// Impose `h <= h_inside` inside a region (sharp edge; the gradient
    /// limiter turns it into a slope-`g` transition afterwards).
    pub fn min_with_region(&mut self, region: &HRegion, h_inside_m: f64) -> io::Result<()> {
        region.validate()?;
        if !h_inside_m.is_finite() || h_inside_m <= 0.0 {
            return Err(invalid(format!(
                "region target size {h_inside_m} must be positive and finite"
            )));
        }
        self.min_with_fn(|lon, lat| {
            if region.contains(lon, lat) {
                h_inside_m
            } else {
                f64::INFINITY
            }
        });
        Ok(())
    }

    /// Pointwise minimum with another field of identical shape.
    pub fn min_with_field(&mut self, other: &HField) -> io::Result<()> {
        if other.nlon != self.nlon || other.nlat != self.nlat {
            return Err(invalid(format!(
                "field shape mismatch: {}x{} vs {}x{}",
                self.nlon, self.nlat, other.nlon, other.nlat
            )));
        }
        for (v, o) in self.values.iter_mut().zip(other.values.iter()) {
            if *o < *v {
                *v = *o;
            }
        }
        Ok(())
    }

    /// Reduce raster values toward `|∇h| <= g` (g dimensionless, meters per
    /// meter) with a spherical lat-lon fast-sweeping approximation. Grid-edge
    /// updates use physical row spacing and the outer rows additionally use
    /// their exact pairwise great-circle metric.
    ///
    /// This bounds the discrete update stencil; bilinear samples are not an
    /// exact global Lipschitz extension of every raster value. Downstream mesh
    /// quality checks remain authoritative for the realized cell-size ratio.
    /// The stencil uses `g / sqrt(2)` as a conservative per-direction budget so
    /// two bilinear derivatives cannot independently consume the full `g`.
    ///
    /// Raster resolution requirement: use spacing <= the local h (ideally
    /// <= h/2). For sub-raster targets, build the field on a finer raster or a
    /// regional window rather than treating this approximation as a proof of
    /// the realized mesh gradation.
    ///
    /// Solved by deterministic fast sweeping of the eikonal update with
    /// longitude periodicity, exact spherical metric closure on the top and
    /// bottom rows, and a per-row `cos(lat)` metric. Returns the number of
    /// 4-ordering sweep rounds performed.
    pub fn limit_gradient(&mut self, g: f64) -> io::Result<usize> {
        self.limit_gradient_with_max_rounds(g, 256)
    }

    fn limit_gradient_with_max_rounds(&mut self, g: f64, max_rounds: usize) -> io::Result<usize> {
        if !g.is_finite() || g <= 0.0 {
            return Err(invalid(format!(
                "gradation g {g} must be positive and finite"
            )));
        }
        for &v in &self.values {
            if !v.is_finite() || v <= 0.0 {
                return Err(invalid(
                    "gradient limiting requires all h values positive and finite".to_string(),
                ));
            }
        }
        // Bilinear sampling blends the two orthogonal raster derivatives. Give
        // each direction a g/sqrt(2) budget so their vector sum cannot spend
        // the full requested gradation twice at off-center sample locations.
        let stencil_g = g / SQRT_2;
        let nlon = self.nlon;
        let nlat = self.nlat;
        let radius = EARTH_RADIUS_METERS;
        let dy = (PI / 180.0) * self.dlat_degrees() * radius;
        let dlon_rad = (PI / 180.0) * self.dlon_degrees();
        let dx: Vec<f64> = (0..nlat)
            .map(|j| (dlon_rad * radius * self.lat_center(j).to_radians().cos()).max(1e-9))
            .collect();
        let polar_distances: Vec<f64> = (0..nlon)
            .map(|i| {
                great_circle_distance_m(
                    self.lon_center(0),
                    self.lat_center(0),
                    self.lon_center(i),
                    self.lat_center(0),
                )
            })
            .collect();

        let h_scale = self.values.iter().cloned().fold(0.0_f64, f64::max).max(1.0);
        let tol = 1e-12 * h_scale;
        let mut rounds = 0usize;
        let mut converged = false;
        while rounds < max_rounds {
            rounds += 1;
            let mut max_change = 0.0_f64;
            // Four deterministic sweep orderings (fast sweeping).
            for ordering in 0..4 {
                let south_polar_cap = self.values[..nlon]
                    .iter()
                    .copied()
                    .fold(f64::INFINITY, f64::min)
                    + stencil_g * dy;
                let north_start = (nlat - 1) * nlon;
                let north_polar_cap = self.values[north_start..north_start + nlon]
                    .iter()
                    .copied()
                    .fold(f64::INFINITY, f64::min)
                    + stencil_g * dy;
                for jj in 0..nlat {
                    let j = if ordering & 1 == 0 { jj } else { nlat - 1 - jj };
                    for ii in 0..nlon {
                        let i = if ordering & 2 == 0 { ii } else { nlon - 1 - ii };
                        let k = j * nlon + i;
                        let west = self.values[j * nlon + (i + nlon - 1) % nlon];
                        let east = self.values[j * nlon + (i + 1) % nlon];
                        let a = west.min(east);
                        let south = if j > 0 {
                            self.values[(j - 1) * nlon + i]
                        } else {
                            f64::INFINITY
                        };
                        let north = if j + 1 < nlat {
                            self.values[(j + 1) * nlon + i]
                        } else {
                            f64::INFINITY
                        };
                        let b = south.min(north);
                        let mut h_new = eikonal_update(a, dx[j], b, dy, stencil_g);
                        if j == 0 || j + 1 == nlat {
                            // A meridian continues through a pole at longitude
                            // +180 degrees. Without this edge the top/bottom
                            // rows behave like artificial circular boundaries
                            // and can violate the spherical Lipschitz bound.
                            for offset in [nlon / 2, nlon.div_ceil(2)] {
                                let opposite_i = (i + offset) % nlon;
                                let opposite = self.values[j * nlon + opposite_i];
                                let cross_pole_distance = great_circle_distance_m(
                                    self.lon_center(i),
                                    self.lat_center(j),
                                    self.lon_center(opposite_i),
                                    self.lat_center(j),
                                );
                                h_new = h_new.min(opposite + stencil_g * cross_pole_distance);
                            }
                            h_new = h_new.min(if j == 0 {
                                south_polar_cap
                            } else {
                                north_polar_cap
                            });
                        }
                        let h_cur = self.values[k];
                        if h_new < h_cur - tol {
                            self.values[k] = h_new;
                            let change = h_cur - h_new;
                            if change > max_change {
                                max_change = change;
                            }
                        }
                    }
                }
            }
            for start in [0, (nlat - 1) * nlon] {
                max_change = max_change.max(limit_metric_row(
                    &mut self.values[start..start + nlon],
                    &polar_distances,
                    stencil_g,
                ));
            }
            if max_change <= tol {
                converged = true;
                break;
            }
        }
        if converged {
            Ok(rounds)
        } else {
            Err(io::Error::other(format!(
                "gradient limiter did not converge within {max_rounds} rounds"
            )))
        }
    }

    /// Quantize to discrete refinement levels: `level = ceil(log2(h_base / h))`
    /// clamped to `[0, max_level]`. `h >= h_base` maps to level 0.
    pub fn level_map(&self, h_base_m: f64, max_level: u8) -> io::Result<Vec<u8>> {
        if !h_base_m.is_finite() || h_base_m <= 0.0 {
            return Err(invalid(format!(
                "base cell size {h_base_m} must be positive and finite"
            )));
        }
        let mut levels = Vec::with_capacity(self.values.len());
        for &h in &self.values {
            levels.push(quantize_h_level(h, h_base_m, max_level)?);
        }
        Ok(levels)
    }

    /// Level at a sampled point (bilinear h, then quantized).
    pub fn level_at(&self, lon_deg: f64, lat_deg: f64, h_base_m: f64, max_level: u8) -> u8 {
        self.try_level_at(lon_deg, lat_deg, h_base_m, max_level)
            .expect("HField::level_at requires positive finite field values and base size")
    }

    /// Fallible form of [`Self::level_at`] for callers that handle invalid
    /// persisted fields or bad base sizes without panicking.
    pub fn try_level_at(
        &self,
        lon_deg: f64,
        lat_deg: f64,
        h_base_m: f64,
        max_level: u8,
    ) -> io::Result<u8> {
        if !lon_deg.is_finite() || !lat_deg.is_finite() || !(-90.0..=90.0).contains(&lat_deg) {
            return Err(invalid(format!(
                "sample location ({lon_deg}, {lat_deg}) must be finite with latitude in [-90, 90]"
            )));
        }
        quantize_h_level(self.sample(lon_deg, lat_deg), h_base_m, max_level)
    }
}

fn limit_metric_row(row: &mut [f64], distances: &[f64], g: f64) -> f64 {
    // Local longitude edges overestimate polar great-circle distances, so the
    // two outer rows need their exact metric lower envelope.
    let source = row.to_vec();
    let len = row.len();
    let mut max_change = 0.0_f64;
    for (i, value) in row.iter_mut().enumerate() {
        let limited = source.iter().enumerate().fold(*value, |best, (j, source)| {
            best.min(source + g * distances[(i + len - j) % len])
        });
        if limited < *value {
            max_change = max_change.max(*value - limited);
            *value = limited;
        }
    }
    max_change
}

/// Local eikonal solver for `|∇h| = g` with axis spacings `dxa` (lon) and
/// `dyb` (lat); `a`/`b` are the best (smallest) upwind neighbor values, or
/// `f64::INFINITY` when the axis has no neighbor.
fn eikonal_update(a: f64, dxa: f64, b: f64, dyb: f64, g: f64) -> f64 {
    let ha = if a.is_finite() {
        a + g * dxa
    } else {
        f64::INFINITY
    };
    let hb = if b.is_finite() {
        b + g * dyb
    } else {
        f64::INFINITY
    };
    let mut h = ha.min(hb);
    if a.is_finite() && b.is_finite() {
        let ia = 1.0 / (dxa * dxa);
        let ib = 1.0 / (dyb * dyb);
        let s = ia + ib;
        let m = a * ia + b * ib;
        let c = a * a * ia + b * b * ib - g * g;
        let disc = m * m - s * c;
        if disc > 0.0 {
            let h2 = (m + disc.sqrt()) / s;
            if h2 >= a && h2 >= b && h2 < h {
                h = h2;
            }
        }
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meters_per_degree_lat() -> f64 {
        (PI / 180.0) * EARTH_RADIUS_METERS
    }

    #[test]
    fn persisted_values_restore_only_valid_fields() {
        let values = vec![42.0; 8];
        let field = HField::from_values(4, 2, values.clone()).unwrap();
        assert_eq!(field.values(), values);
        assert!(HField::from_values(4, 2, vec![42.0; 7]).is_err());
        assert!(HField::from_values(4, 2, vec![f64::NAN; 8]).is_err());
    }

    #[test]
    fn public_writes_and_quantization_reject_bad_h_values() {
        let mut field = HField::uniform(4, 2, 100.0).unwrap();
        assert!(field.set(0, 0, f64::NAN).is_err());
        assert!(field.set(0, 0, f64::INFINITY).is_err());
        assert!(field.set(0, 0, 0.0).is_err());

        let bad = HField {
            nlon: 1,
            nlat: 1,
            values: vec![f64::NAN],
        };
        assert!(bad.level_map(100.0, 3).is_err());
        assert!(bad.try_level_at(0.0, 0.0, 100.0, 3).is_err());
        assert!(field.try_level_at(f64::NAN, 0.0, 100.0, 3).is_err());
        assert!(field.try_level_at(0.0, 91.0, 100.0, 3).is_err());
    }

    #[test]
    fn limiter_reports_non_convergence() {
        let mut field = HField::uniform(60, 30, 1_000_000.0).unwrap();
        field.set(30, 15, 10_000.0).unwrap();
        let mut short = field.clone();
        let error = short.limit_gradient_with_max_rounds(0.2, 1).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert!(error.to_string().contains("did not converge"));
        field.limit_gradient(0.2).expect("normal cap converges");
    }

    #[test]
    fn constructors_reject_overflow_and_unallocatable_grids() {
        assert!(HField::uniform(usize::MAX, 2, 42.0)
            .unwrap_err()
            .to_string()
            .contains("overflow"));
        let too_many = isize::MAX as usize / std::mem::size_of::<f64>() / 2 + 1;
        assert!(HField::uniform(too_many, 2, 42.0)
            .unwrap_err()
            .to_string()
            .contains("allocation limit"));
        assert!(HField::from_values(too_many, 2, Vec::new())
            .unwrap_err()
            .to_string()
            .contains("allocation limit"));
    }

    #[test]
    fn bilinear_sample_wraps_dateline() {
        let mut f = HField::uniform(360, 180, 100_000.0).unwrap();
        // Cells straddling the dateline: i = 359 (lon 179.5) and i = 0 (lon -179.5).
        f.set(359, 90, 50_000.0).unwrap();
        f.set(0, 90, 70_000.0).unwrap();
        let v = f.sample(180.0, f.lat_center(90));
        // Midway between the two centers.
        assert!((v - 60_000.0).abs() < 1.0, "v={v}");
        // Determinism of wrapping on the other notation of the same meridian.
        let v2 = f.sample(-180.0, f.lat_center(90));
        assert!((v - v2).abs() < 1e-9);
    }

    #[test]
    fn gradient_limit_cone_matches_conservative_analytic_solution() {
        let mut f = HField::uniform(360, 180, 1_000_000.0).unwrap();
        let (si, sj) = (180usize, 90usize); // lon 0.5, lat 0.5
        f.set(si, sj, 10_000.0).unwrap();
        let g = 0.2;
        let raster_g = g / SQRT_2;
        f.limit_gradient(g).unwrap();

        let slon = f.lon_center(si);
        let slat = f.lat_center(sj);
        // Axis (due north) and diagonal (northeast) probes.
        for &(di, dj) in &[(0isize, 20isize), (20, 0), (14, 14), (-17, 9)] {
            let i = (si as isize + di).rem_euclid(360) as usize;
            let j = (sj as isize + dj) as usize;
            let d = great_circle_distance_m(f.lon_center(i), f.lat_center(j), slon, slat);
            let expected = (10_000.0 + raster_g * d).min(1_000_000.0);
            let got = f.get(i, j);
            let rel = (got - expected).abs() / expected;
            assert!(
                rel < 0.10,
                "probe ({di},{dj}): got {got}, expected {expected}, rel {rel}"
            );
        }
    }

    #[test]
    fn gradient_limit_bounds_neighbor_ratio_and_is_idempotent() {
        let mut f = HField::uniform(180, 90, 500_000.0).unwrap();
        // A rough multi-source field.
        f.min_with_region(
            &HRegion::Circle {
                lon: 10.0,
                lat: 0.0,
                radius_m: 300_000.0,
            },
            20_000.0,
        )
        .unwrap();
        f.min_with_region(
            &HRegion::Bbox {
                west: 170.0,
                east: -170.0,
                south: -10.0,
                north: 10.0,
            },
            60_000.0,
        )
        .unwrap();
        let g = 0.25;
        f.limit_gradient(g).unwrap();

        // Discrete bound: along every grid edge, h may grow by at most g*edge.
        let dy = (PI / 180.0) * f.dlat_degrees() * EARTH_RADIUS_METERS;
        for j in 0..f.nlat() {
            let dx = (PI / 180.0)
                * f.dlon_degrees()
                * EARTH_RADIUS_METERS
                * f.lat_center(j).to_radians().cos().max(1e-9);
            for i in 0..f.nlon() {
                let h = f.get(i, j);
                let east = f.get((i + 1) % f.nlon(), j);
                assert!(
                    (h - east).abs() <= g * dx + 1e-6,
                    "lon-neighbor bound violated at ({i},{j})"
                );
                if j + 1 < f.nlat() {
                    let north = f.get(i, j + 1);
                    assert!(
                        (h - north).abs() <= g * dy + 1e-6,
                        "lat-neighbor bound violated at ({i},{j})"
                    );
                }
            }
        }

        // Idempotence and monotonicity.
        let before = f.values().to_vec();
        f.limit_gradient(g).unwrap();
        for (x, y) in before.iter().zip(f.values().iter()) {
            assert!((x - y).abs() <= 1e-6, "limiter not idempotent");
        }
        assert!(f.values().iter().all(|&v| v <= 500_000.0 + 1e-6));
    }

    #[test]
    fn polar_rows_obey_spherical_lipschitz_and_pole_samples_are_unique() {
        let mut field = HField::uniform(8, 4, 10_000_000.0).unwrap();
        let top = field.nlat() - 1;
        field.set(0, top, 10_000.0).unwrap();
        let g = 0.2;

        field.limit_gradient(g).unwrap();

        let opposite = field.nlon() / 2;
        let spherical_distance = great_circle_distance_m(
            field.lon_center(0),
            field.lat_center(top),
            field.lon_center(opposite),
            field.lat_center(top),
        );
        let difference = (field.get(0, top) - field.get(opposite, top)).abs();
        assert!(
            difference <= g * spherical_distance + 1.0e-6,
            "cross-pole Lipschitz bound violated: |dh|={difference}, g*d={}",
            g * spherical_distance
        );

        let north = field.sample(0.0, 90.0);
        let distance_to_pole = 0.5 * field.dlat_degrees().to_radians() * EARTH_RADIUS_METERS;
        for i in 0..field.nlon() {
            assert!(
                (field.get(i, top) - north).abs() <= g * distance_to_pole + 1.0e-6,
                "north-pole sample violates the row-to-pole Lipschitz bound at longitude index {i}"
            );
        }
        for lon in [-180.0, -90.0, 45.0, 179.0] {
            assert_eq!(
                field.sample(lon, 90.0),
                north,
                "the geographic north pole must not depend on longitude"
            );
        }
        let south = field.sample(0.0, -90.0);
        for lon in [-180.0, -90.0, 45.0, 179.0] {
            assert_eq!(
                field.sample(lon, -90.0),
                south,
                "the geographic south pole must not depend on longitude"
            );
        }
    }

    #[test]
    fn near_pole_samples_obey_spherical_lipschitz_for_arbitrary_longitudes() {
        let mut field = HField::uniform(12, 6, 10_000_000.0).unwrap();
        field.set(0, 0, 10_000.0).unwrap();
        let g = 0.2;
        field.limit_gradient(g).unwrap();

        for lat in [-89.0, -85.0, -80.0, field.lat_center(0)] {
            for i in 0..2 * field.nlon() {
                for j in 0..2 * field.nlon() {
                    let lon_a = -180.0 + (i as f64 + 0.5) * field.dlon_degrees() / 2.0;
                    let lon_b = -180.0 + (j as f64 + 0.5) * field.dlon_degrees() / 2.0;
                    let difference = (field.sample(lon_a, lat) - field.sample(lon_b, lat)).abs();
                    let distance = great_circle_distance_m(lon_a, lat, lon_b, lat);
                    assert!(
                        difference <= g * distance + 1.0e-6,
                        "near-pole Lipschitz bound violated at latitude {lat}: |dh|={difference}, g*d={}",
                        g * distance
                    );
                }
            }
        }

        let pole = field.sample(0.0, -90.0);
        for lon in [-165.0, -45.0, 75.0] {
            assert!(
                (field.sample(lon, -90.0 + 1.0e-9) - pole).abs() <= 1.0e-3,
                "sampling must converge continuously to the geographic pole"
            );
        }
    }

    #[test]
    fn bilinear_samples_respect_requested_spherical_gradation() {
        let mut seed = 1_u64;
        let values = (0..12 * 6)
            .map(|_| {
                seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
                4_000_000.0 + ((seed >> 11) as f64 / (1_u64 << 53) as f64) * 8_000_000.0
            })
            .collect();
        let mut field = HField::from_values(12, 6, values).unwrap();
        let g = 0.2;
        field.limit_gradient(g).unwrap();

        let a = (-135.0, -15.0);
        let b = (-110.0, 10.0);
        let difference = (field.sample(a.0, a.1) - field.sample(b.0, b.1)).abs();
        let distance = great_circle_distance_m(a.0, a.1, b.0, b.1);
        assert!(
            difference <= g * distance + 1.0e-6,
            "bilinear samples exceed the requested spherical gradation: |dh|={difference}, g*d={}",
            g * distance
        );
    }

    #[test]
    fn odd_longitude_count_checks_both_cross_pole_neighbors() {
        let mut field = HField::uniform(5, 4, 10_000_000.0).unwrap();
        let top = field.nlat() - 1;
        field.set(0, top, 10_000.0).unwrap();
        let g = 0.2;

        field.limit_gradient(g).unwrap();

        for opposite in [field.nlon() / 2, field.nlon().div_ceil(2)] {
            let distance = great_circle_distance_m(
                field.lon_center(0),
                field.lat_center(top),
                field.lon_center(opposite),
                field.lat_center(top),
            );
            assert!(
                (field.get(0, top) - field.get(opposite, top)).abs() <= g * distance + 1.0e-6,
                "odd nlon missed cross-pole neighbor {opposite}"
            );
        }
    }

    #[test]
    fn level_rings_have_width_h_over_g() {
        let mut f = HField::uniform(360, 180, 1_000_000.0).unwrap();
        let (si, sj) = (180usize, 90usize);
        f.set(si, sj, 10_000.0).unwrap();
        let g = 0.2;
        f.limit_gradient(g).unwrap();
        let h_base = 1_000_000.0;
        let levels = f.level_map(h_base, 10).unwrap();

        // Walk due north from the source; the level-2 ring spans
        // h in [h_base/4, h_base/2). The raster cone uses g/sqrt(2) so
        // bilinear samples retain the requested two-dimensional budget.
        let level_at = |j: usize| levels[j * 360 + si];
        let mut first2: Option<usize> = None;
        let mut last2: Option<usize> = None;
        for j in sj..180 {
            if level_at(j) == 2 {
                if first2.is_none() {
                    first2 = Some(j);
                }
                last2 = Some(j);
            }
        }
        let (j0, j1) = (first2.expect("ring exists"), last2.unwrap());
        let measured = (j1 - j0 + 1) as f64 * meters_per_degree_lat();
        let expected = (h_base / 4.0) / (g / SQRT_2);
        let rel = (measured - expected).abs() / expected;
        assert!(
            rel < 0.20,
            "ring width {measured} vs {expected} (rel {rel})"
        );
        // Nesting sanity: adjacent rows never jump two levels, wherever the
        // raster resolves the target sizes (h >= one raster row). Deeper
        // levels need a finer raster for this guarantee -- see the
        // `limit_gradient` docs.
        for j in sj..179 {
            let (la, lb) = (level_at(j), level_at(j + 1));
            if la <= 3 && lb <= 3 {
                let d = (la as i16 - lb as i16).abs();
                assert!(d <= 1, "level jump {d} at row {j}");
            }
        }
    }

    #[test]
    fn circle_region_center_keeps_target_and_grows_at_conservative_raster_slope() {
        let mut f = HField::uniform(360, 180, 400_000.0).unwrap();
        f.min_with_region(
            &HRegion::Circle {
                lon: 0.0,
                lat: 0.0,
                radius_m: 500_000.0,
            },
            25_000.0,
        )
        .unwrap();
        let g = 0.2;
        f.limit_gradient(g).unwrap();
        let center = f.sample(0.0, 0.0);
        assert!((center - 25_000.0).abs() < 1.0, "center {center}");
        // The raster spends g/sqrt(2) per direction so off-center bilinear
        // samples cannot combine two full g gradients.
        let d_total = 1_500_000.0;
        let probe_lat = d_total / meters_per_degree_lat();
        let got = f.sample(0.0, probe_lat);
        let expected = 25_000.0 + (g / SQRT_2) * (d_total - 500_000.0);
        assert!(
            (got - expected).abs() / expected < 0.12,
            "got {got} expected {expected}"
        );
    }

    #[test]
    fn bbox_wraps_dateline_and_polygon_contains_works() {
        let bbox = HRegion::Bbox {
            west: 170.0,
            east: -170.0,
            south: -5.0,
            north: 5.0,
        };
        assert!(bbox.contains(175.0, 0.0));
        assert!(bbox.contains(-175.0, 0.0));
        assert!(bbox.contains(180.0, 4.9));
        assert!(!bbox.contains(0.0, 0.0));
        assert!(!bbox.contains(175.0, 6.0));

        for (west, east) in [
            (-180.0, 180.0),
            (10.0, 369.999_999_999_999_94),
            (180.0, -180.0),
        ] {
            let global_band = HRegion::Bbox {
                west,
                east,
                south: -5.0,
                north: 5.0,
            };
            for lon in [-179.9, -90.0, 0.0, 90.0, 179.9] {
                assert!(
                    global_band.contains(lon, 0.0),
                    "full-longitude bbox {west}..{east} missed {lon}"
                );
            }
            assert!(!global_band.contains(0.0, 6.0));
        }

        let poly = HRegion::Polygon {
            points: vec![(178.0, -3.0), (-178.0, -3.0), (-178.0, 3.0), (178.0, 3.0)],
        };
        assert!(poly.contains(179.5, 0.0));
        assert!(poly.contains(-179.5, 0.0));
        assert!(!poly.contains(170.0, 0.0));
        assert!(!poly.contains(179.5, 4.0));

        let north_cap = HRegion::Polygon {
            points: vec![(-120.0, 80.0), (0.0, 80.0), (120.0, 80.0)],
        };
        assert!(north_cap.contains(0.0, 89.0));
        assert!(!north_cap.contains(0.0, 70.0));
        assert!(
            !north_cap.contains(0.0, -80.0),
            "far side must not be inside"
        );

        let reversed_cap = HRegion::Polygon {
            points: vec![(120.0, 80.0), (0.0, 80.0), (-120.0, 80.0)],
        };
        assert!(reversed_cap.contains(0.0, 89.0));
        assert!(!reversed_cap.contains(0.0, -80.0));

        let bad = HRegion::Polygon {
            points: vec![(0.0, 0.0), (1.0, f64::NAN), (2.0, 0.0)],
        };
        assert!(!bad.contains(0.5, 0.1));
        assert!(!poly.contains(179.5, f64::NAN));

        let midlat = HRegion::Polygon {
            points: vec![(0.0, 0.0), (10.0, 0.0), (5.0, 8.0)],
        };
        assert!(midlat.contains(5.0, 0.0), "boundary edge counts inside");
        assert!(midlat.contains(5.0, 2.0));
        assert!(
            !midlat.contains(-120.0, -45.0),
            "ordinary far side stays outside"
        );
    }

    #[test]
    fn all_regions_reject_nonfinite_query_coordinates() {
        let regions = [
            HRegion::Bbox {
                west: -180.0,
                east: 180.0,
                south: -90.0,
                north: 90.0,
            },
            HRegion::Circle {
                lon: 0.0,
                lat: 0.0,
                radius_m: 1_000_000.0,
            },
            HRegion::Polygon {
                points: vec![(-10.0, -10.0), (10.0, -10.0), (0.0, 10.0)],
            },
            HRegion::Corridor {
                points: vec![(0.0, 0.0), (10.0, 0.0)],
                radius_meters: vec![100_000.0, 100_000.0],
            },
        ];
        for region in regions {
            for (lon, lat) in [
                (f64::NAN, 0.0),
                (f64::INFINITY, 0.0),
                (0.0, f64::NAN),
                (0.0, f64::NEG_INFINITY),
                (0.0, 91.0),
            ] {
                assert!(
                    !region.contains(lon, lat),
                    "{region:?} accepted ({lon}, {lat})"
                );
            }
        }
    }

    #[test]
    fn region_validation_rejects_degenerate_shapes_before_apply() {
        assert!(HRegion::Bbox {
            west: 0.0,
            east: 1.0,
            south: 5.0,
            north: 4.0,
        }
        .validate()
        .unwrap_err()
        .to_string()
        .contains("latitudes"));
        assert!(HRegion::Bbox {
            west: 0.0,
            east: 0.0,
            south: 0.0,
            north: 1.0,
        }
        .validate()
        .unwrap_err()
        .to_string()
        .contains("nonzero span"));
        assert!(HRegion::Circle {
            lon: 0.0,
            lat: 0.0,
            radius_m: 0.0,
        }
        .validate()
        .unwrap_err()
        .to_string()
        .contains("radius"));
        assert!(HRegion::Corridor {
            points: vec![(0.0, 0.0), (1.0, 0.0)],
            radius_meters: vec![10.0],
        }
        .validate()
        .unwrap_err()
        .to_string()
        .contains("one positive radius per point"));
        assert!(HRegion::Polygon {
            points: vec![(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)],
        }
        .validate()
        .unwrap_err()
        .to_string()
        .contains("area"));
        assert!(HRegion::Polygon {
            points: vec![(0.0, 0.0), (180.0, 0.0), (0.0, 1.0)],
        }
        .validate()
        .unwrap_err()
        .to_string()
        .contains("antipodal"));
        assert!(HRegion::Polygon {
            points: vec![(0.0, 0.0), (10.0, 10.0), (0.0, 10.0), (10.0, 0.0)],
        }
        .validate()
        .unwrap_err()
        .to_string()
        .contains("self-intersect"));

        let mut field = HField::uniform(4, 2, 100.0).unwrap();
        let before = field.values().to_vec();
        let error = field
            .min_with_region(
                &HRegion::Polygon {
                    points: vec![(0.0, 0.0), (1.0, 0.0), (1.0, 0.0)],
                },
                10.0,
            )
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(field.values(), before);
    }

    #[test]
    fn closed_polygon_rings_are_validated_without_breaking_containment() {
        let polygon = HRegion::Polygon {
            points: vec![(0.0, 0.0), (4.0, 0.0), (0.0, 4.0), (0.0, 0.0)],
        };
        polygon.validate().unwrap();
        assert!(polygon.contains(1.0, 1.0));
    }

    #[test]
    fn corridor_distance_matches_cross_track_on_equator() {
        let corridor = HRegion::Corridor {
            points: vec![(0.0, 0.0), (10.0, 0.0)],
            radius_meters: vec![100_000.0, 100_000.0],
        };
        // Cross-track: 3 degrees due north of the segment interior.
        let d = corridor.distance_m(5.0, 3.0);
        let expected = 3.0 * meters_per_degree_lat();
        assert!(
            (d - expected).abs() / expected < 0.01,
            "d {d} vs {expected}"
        );
        // Beyond the endpoint: distance to (10, 0).
        let d_end = corridor.distance_m(15.0, 0.0);
        let expected_end = 5.0 * meters_per_degree_lat();
        assert!(
            (d_end - expected_end).abs() / expected_end < 0.01,
            "d_end {d_end} vs {expected_end}"
        );
        assert!(corridor.contains(5.0, 0.5));
        assert!(!corridor.contains(5.0, 3.0));
    }

    #[test]
    fn corridor_interpolates_each_endpoint_radius_instead_of_using_the_maximum() {
        let corridor = HRegion::Corridor {
            points: vec![(0.0, 0.0), (10.0, 0.0)],
            radius_meters: vec![50_000.0, 500_000.0],
        };

        assert!(
            !corridor.contains(1.0, 2.0),
            "the narrow start must not inherit the far endpoint's maximum radius"
        );
        assert!(
            corridor.contains(9.0, 2.0),
            "the wide end must use its interpolated segment radius"
        );
    }

    #[test]
    fn limiter_is_deterministic() {
        let build = || {
            let mut f = HField::uniform(120, 60, 300_000.0).unwrap();
            f.min_with_region(
                &HRegion::Circle {
                    lon: 33.0,
                    lat: 21.0,
                    radius_m: 200_000.0,
                },
                15_000.0,
            )
            .unwrap();
            f.limit_gradient(0.2).unwrap();
            f
        };
        let a = build();
        let b = build();
        assert_eq!(
            a.values(),
            b.values(),
            "two identical runs must match bitwise"
        );
    }

    #[test]
    fn level_map_handles_exact_powers() {
        let mut f = HField::uniform(8, 4, 100_000.0).unwrap();
        f.set(0, 0, 100_000.0).unwrap();
        f.set(1, 0, 50_000.0).unwrap();
        f.set(2, 0, 49_000.0).unwrap();
        f.set(3, 0, 25_000.0).unwrap();
        let levels = f.level_map(100_000.0, 5).unwrap();
        assert_eq!(levels[0], 0);
        assert_eq!(levels[1], 1);
        assert_eq!(levels[2], 2);
        assert_eq!(levels[3], 2);
    }
}
