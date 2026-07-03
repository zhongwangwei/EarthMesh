//! Unified mesh cell-width field `h(x)` for EarthMesh refinement planning.
//!
//! This crate implements the "continuous mesh-density field" layer recommended
//! by `docs/mesh_refinement_method_research_2026-07-02.md`:
//!
//! 1. Every refinement input — threshold criteria (LAI/slope/SST/EKE rasters)
//!    and user-specified regions (bbox/circle/polygon/corridor) — contributes
//!    its own target-cell-size field `h_i(lon, lat)` in meters.
//! 2. Fields compose by pointwise minimum (`min_with_*`).
//! 3. A spherical gradient limiter enforces `|∇h| <= g` (Persson 2004/2006),
//!    which mathematically bounds neighbor cell-size ratios by `1 + g` and, in
//!    cell units, guarantees every quantized refinement ring is at least
//!    `~0.7/g` rows wide — so Method-C nesting legality holds by construction.
//! 4. `level_map` quantizes `h` back to discrete power-of-two levels for the
//!    existing subdivision engines; `sample` feeds continuous targets to the
//!    spring relaxers ("split between levels, stretch within levels").
//!
//! Design constraints: zero dependencies, deterministic (fixed-order fast
//! sweeping, no threads), pure f64. All longitudes are degrees in [-180, 180)
//! (inputs are wrapped), latitudes are degrees in [-90, 90].

use std::f64::consts::PI;
use std::io;

/// Mean Earth radius in meters. Matches `earthmesh_core::EARTH_RADIUS_METERS`
/// (Fortran `erad = 6371229`); kept local so this crate stays dependency-free.
pub const EARTH_RADIUS_METERS: f64 = 6_371_229.0;

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

fn unwrap_near(lon: f64, anchor: f64) -> f64 {
    let mut l = lon;
    while l - anchor > 180.0 {
        l -= 360.0;
    }
    while l - anchor < -180.0 {
        l += 360.0;
    }
    l
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

/// Great-circle distance in meters from point `p` to the segment `a..b`.
fn point_segment_distance_m(p: [f64; 3], a: [f64; 3], b: [f64; 3]) -> f64 {
    let seg_angle = gc_angle(a, b);
    let end_dist = gc_angle(p, a).min(gc_angle(p, b));
    if seg_angle < 1e-12 {
        return end_dist * EARTH_RADIUS_METERS;
    }
    let n = cross3(a, b);
    let nn = norm3(n);
    if nn < 1e-15 {
        return end_dist * EARTH_RADIUS_METERS;
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
        return end_dist.min(PI / 2.0) * EARTH_RADIUS_METERS;
    }
    let f_hat = [f[0] / fn3, f[1] / fn3, f[2] / fn3];
    let within = gc_angle(a, f_hat) + gc_angle(f_hat, b) <= seg_angle + 1e-9;
    let angle = if within {
        sin_xt.abs().asin()
    } else {
        end_dist
    };
    angle * EARTH_RADIUS_METERS
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
    Circle {
        lon: f64,
        lat: f64,
        radius_m: f64,
    },
    /// Simple polygon (no self-intersection), vertices (lon, lat) in degrees.
    /// Longitudes are unwrapped around the first vertex, so polygons narrower
    /// than 180 degrees work across the dateline.
    Polygon {
        points: Vec<(f64, f64)>,
    },
    /// Polyline buffered by a constant great-circle radius.
    Corridor {
        points: Vec<(f64, f64)>,
        radius_m: f64,
    },
}

impl HRegion {
    pub fn contains(&self, lon_deg: f64, lat_deg: f64) -> bool {
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
            HRegion::Polygon { points } => {
                if points.len() < 3 {
                    return false;
                }
                let anchor = points[0].0;
                let x = unwrap_near(lon_deg, anchor);
                let y = lat_deg;
                let mut inside = false;
                let mut jj = points.len() - 1;
                for ii in 0..points.len() {
                    let xi = unwrap_near(points[ii].0, anchor);
                    let yi = points[ii].1;
                    let xj = unwrap_near(points[jj].0, anchor);
                    let yj = points[jj].1;
                    if (yi > y) != (yj > y) {
                        let x_int = xi + (y - yi) / (yj - yi) * (xj - xi);
                        if x < x_int {
                            inside = !inside;
                        }
                    }
                    jj = ii;
                }
                inside
            }
            HRegion::Corridor { points, radius_m } => {
                self.distance_m(lon_deg, lat_deg) <= *radius_m && !points.is_empty()
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
                    best = best.min(point_segment_distance_m(p, a, b));
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

impl HField {
    pub fn uniform(nlon: usize, nlat: usize, h_meters: f64) -> io::Result<Self> {
        if nlon < 4 || nlat < 2 {
            return Err(invalid(format!(
                "HField grid {nlon}x{nlat} too small (need >= 4x2)"
            )));
        }
        if !h_meters.is_finite() || h_meters <= 0.0 {
            return Err(invalid(format!(
                "HField uniform value {h_meters} must be positive and finite"
            )));
        }
        Ok(Self {
            nlon,
            nlat,
            values: vec![h_meters; nlon * nlat],
        })
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

    pub fn set(&mut self, ilon: usize, jlat: usize, h_meters: f64) {
        let k = self.idx(ilon, jlat);
        self.values[k] = h_meters;
    }

    pub fn values(&self) -> &[f64] {
        &self.values
    }

    /// Bilinear sample at (lon, lat) degrees; longitude wraps, latitude clamps.
    pub fn sample(&self, lon_deg: f64, lat_deg: f64) -> f64 {
        let dlon = self.dlon_degrees();
        let dlat = self.dlat_degrees();
        let x = (wrap_lon_degrees(lon_deg) + 180.0) / dlon - 0.5;
        let i0f = x.floor();
        let fx = x - i0f;
        let i0 = (i0f as i64).rem_euclid(self.nlon as i64) as usize;
        let i1 = (i0 + 1) % self.nlon;
        let y = ((lat_deg + 90.0) / dlat - 0.5).clamp(0.0, (self.nlat - 1) as f64);
        let j0 = y.floor() as usize;
        let j0 = j0.min(self.nlat - 1);
        let j1 = (j0 + 1).min(self.nlat - 1);
        let fy = y - j0 as f64;
        let v00 = self.get(i0, j0);
        let v10 = self.get(i1, j0);
        let v01 = self.get(i0, j1);
        let v11 = self.get(i1, j1);
        let v0 = v00 + (v10 - v00) * fx;
        let v1 = v01 + (v11 - v01) * fx;
        v0 + (v1 - v0) * fy
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

    /// Enforce `|∇h| <= g` on the sphere (g dimensionless, meters per meter):
    /// afterwards `h(x) <= h(y) + g * d(x, y)` for all x, y, i.e. the largest
    /// field below the input with bounded gradation (Persson 2004, Thm 2.1;
    /// the MESH-neighbor cell-size ratio is then bounded by `1 + g`, since one
    /// mesh cell of size h spans distance h and may change h by at most g*h).
    ///
    /// Raster resolution requirement: the per-mesh-cell ratio bound and the
    /// "quantized levels never jump by 2" property hold where the raster
    /// resolves the target sizes, i.e. raster spacing <= the local h (ideally
    /// <= h/2). For sub-raster targets (very fine refinement), build the field
    /// on a finer raster or a regional window.
    ///
    /// Solved by deterministic fast sweeping of the eikonal update with
    /// longitude periodicity and per-row `cos(lat)` metric. Returns the number
    /// of 4-ordering sweep rounds performed.
    pub fn limit_gradient(&mut self, g: f64) -> io::Result<usize> {
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
        let nlon = self.nlon;
        let nlat = self.nlat;
        let radius = EARTH_RADIUS_METERS;
        let dy = (PI / 180.0) * self.dlat_degrees() * radius;
        let dlon_rad = (PI / 180.0) * self.dlon_degrees();
        let dx: Vec<f64> = (0..nlat)
            .map(|j| (dlon_rad * radius * self.lat_center(j).to_radians().cos()).max(1e-9))
            .collect();

        let h_scale = self.values.iter().cloned().fold(0.0_f64, f64::max).max(1.0);
        let tol = 1e-12 * h_scale;
        let max_rounds = 256usize;

        let mut rounds = 0usize;
        while rounds < max_rounds {
            rounds += 1;
            let mut max_change = 0.0_f64;
            // Four deterministic sweep orderings (fast sweeping).
            for ordering in 0..4 {
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
                        let h_new = eikonal_update(a, dx[j], b, dy, g);
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
            if max_change <= tol {
                break;
            }
        }
        Ok(rounds)
    }

    /// Quantize to discrete refinement levels: `level = ceil(log2(h_base / h))`
    /// clamped to `[0, max_level]`. `h >= h_base` maps to level 0.
    pub fn level_map(&self, h_base_m: f64, max_level: u8) -> io::Result<Vec<u8>> {
        if !h_base_m.is_finite() || h_base_m <= 0.0 {
            return Err(invalid(format!(
                "base cell size {h_base_m} must be positive and finite"
            )));
        }
        Ok(self
            .values
            .iter()
            .map(|&h| {
                if !(h > 0.0) || h >= h_base_m {
                    0u8
                } else {
                    let raw = ((h_base_m / h).log2() - 1e-9).ceil();
                    if raw <= 0.0 {
                        0u8
                    } else if raw >= max_level as f64 {
                        max_level
                    } else {
                        raw as u8
                    }
                }
            })
            .collect())
    }

    /// Level at a sampled point (bilinear h, then quantized).
    pub fn level_at(&self, lon_deg: f64, lat_deg: f64, h_base_m: f64, max_level: u8) -> u8 {
        let h = self.sample(lon_deg, lat_deg);
        if !(h > 0.0) || h >= h_base_m {
            return 0;
        }
        let raw = ((h_base_m / h).log2() - 1e-9).ceil();
        if raw <= 0.0 {
            0
        } else if raw >= max_level as f64 {
            max_level
        } else {
            raw as u8
        }
    }
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
    fn bilinear_sample_wraps_dateline() {
        let mut f = HField::uniform(360, 180, 100_000.0).unwrap();
        // Cells straddling the dateline: i = 359 (lon 179.5) and i = 0 (lon -179.5).
        f.set(359, 90, 50_000.0);
        f.set(0, 90, 70_000.0);
        let v = f.sample(180.0, f.lat_center(90));
        // Midway between the two centers.
        assert!((v - 60_000.0).abs() < 1.0, "v={v}");
        // Determinism of wrapping on the other notation of the same meridian.
        let v2 = f.sample(-180.0, f.lat_center(90));
        assert!((v - v2).abs() < 1e-9);
    }

    #[test]
    fn gradient_limit_cone_matches_analytic_solution() {
        let mut f = HField::uniform(360, 180, 1_000_000.0).unwrap();
        let (si, sj) = (180usize, 90usize); // lon 0.5, lat 0.5
        f.set(si, sj, 10_000.0);
        let g = 0.2;
        f.limit_gradient(g).unwrap();

        let slon = f.lon_center(si);
        let slat = f.lat_center(sj);
        // Axis (due north) and diagonal (northeast) probes.
        for &(di, dj) in &[(0isize, 20isize), (20, 0), (14, 14), (-17, 9)] {
            let i = (si as isize + di).rem_euclid(360) as usize;
            let j = (sj as isize + dj) as usize;
            let d = great_circle_distance_m(f.lon_center(i), f.lat_center(j), slon, slat);
            let expected = (10_000.0 + g * d).min(1_000_000.0);
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
    fn level_rings_have_width_h_over_g() {
        let mut f = HField::uniform(360, 180, 1_000_000.0).unwrap();
        let (si, sj) = (180usize, 90usize);
        f.set(si, sj, 10_000.0);
        let g = 0.2;
        f.limit_gradient(g).unwrap();
        let h_base = 1_000_000.0;
        let levels = f.level_map(h_base, 10).unwrap();

        // Walk due north from the source; the level-2 ring spans
        // h in [h_base/4, h_base/2) whose cone width is (h_base/4)/g.
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
        let expected = (h_base / 4.0) / g;
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
    fn circle_region_center_keeps_target_and_grows_at_slope_g() {
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
        // 1500 km from the edge: expected 25km + 0.2 * 1000km = 225km.
        let d_total = 1_500_000.0;
        let probe_lat = d_total / meters_per_degree_lat();
        let got = f.sample(0.0, probe_lat);
        let expected = 25_000.0 + g * (d_total - 500_000.0);
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

        let poly = HRegion::Polygon {
            points: vec![(178.0, -3.0), (-178.0, -3.0), (-178.0, 3.0), (178.0, 3.0)],
        };
        assert!(poly.contains(179.5, 0.0));
        assert!(poly.contains(-179.5, 0.0));
        assert!(!poly.contains(170.0, 0.0));
        assert!(!poly.contains(179.5, 4.0));
    }

    #[test]
    fn corridor_distance_matches_cross_track_on_equator() {
        let corridor = HRegion::Corridor {
            points: vec![(0.0, 0.0), (10.0, 0.0)],
            radius_m: 100_000.0,
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
        f.set(0, 0, 100_000.0);
        f.set(1, 0, 50_000.0);
        f.set(2, 0, 49_000.0);
        f.set(3, 0, 25_000.0);
        let levels = f.level_map(100_000.0, 5).unwrap();
        assert_eq!(levels[0], 0);
        assert_eq!(levels[1], 1);
        assert_eq!(levels[2], 2);
        assert_eq!(levels[3], 2);
    }
}
